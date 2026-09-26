//! "Everywhere" search (design system `SearchResults`): answered from the index when it's
//! on, by a live walk of the indexed folders when it's off. "This folder" search is the
//! file list's own filter (app.rs).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use ef_config as config;
use ef_index::{KindFilter, Query};

use crate::indexer::{self, RootIndex};

/// Results shown at most; the index answers the rest instantly if the user narrows down.
pub const LIMIT: usize = 300;

#[derive(Debug, Clone)]
pub struct Hit {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    /// Parent folder, written the way people do (`~/Work/2026`).
    pub location: String,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Clone)]
pub struct Results {
    pub query: String,
    pub hits: Vec<Hit>,
    /// Matches before the display limit.
    pub total: usize,
    pub from_index: bool,
    pub index_updated: Option<SystemTime>,
}

fn hit(path: PathBuf, is_dir: bool) -> Hit {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let location = path.parent().map(config::tilde).unwrap_or_default();
    Hit { path, name, is_dir, location, size: 0, mtime: 0 }
}

/// Sizes and dates for the hits (≤ LIMIT `statx` calls).
fn fill_details(hits: &mut [Hit]) {
    use std::os::unix::fs::MetadataExt;
    for h in hits.iter_mut() {
        if let Ok(m) = std::fs::symlink_metadata(&h.path) {
            h.size = if h.is_dir { 0 } else { m.len() };
            h.mtime = m.mtime();
        }
    }
}

/// Search every indexed folder's index. Returns `None` if no index file exists yet.
pub fn from_index(roots: &[RootIndex], text: &str, hidden: bool) -> Option<Results> {
    let maps: Vec<_> = roots.iter().filter_map(|r| r.map.clone()).collect();
    if maps.is_empty() {
        return None;
    }
    let mut hits = Vec::new();
    let mut total = 0;
    for m in &maps {
        let q = Query { text, hidden, ..Query::new(text) };
        let all = m.search(&Query { limit: None, ..q.clone() });
        total += all.len();
        for &id in all.iter().take(LIMIT.saturating_sub(hits.len())) {
            hits.push(hit(m.path(id), m.kind(id) == ef_index::Kind::Dir));
        }
    }
    fill_details(&mut hits);
    let updated = roots.iter().filter_map(|r| r.updated).min();
    Some(Results { query: text.to_string(), hits, total, from_index: true, index_updated: updated })
}

/// Walk the indexed folders live (index off). Cancelled when `cancel` is set.
pub fn live(roots: &[PathBuf], exclude: &ef_config::SearchConfig, text: &str, hidden: bool, cancel: &Arc<AtomicBool>) -> Option<Results> {
    let q = Query { text, hidden, kind: KindFilter::Any, ..Query::new(text) };
    let mut hits = Vec::new();
    let mut total = 0;
    for root in roots {
        let found = indexer::background_pool().install(|| ef_index::live::search(root, &q, cancel, &|_| {})).ok()?;
        for p in found {
            if exclude.exclude_paths.iter().any(|e| p.starts_with(config::expand(e))) || in_skipped(&p, root, exclude) {
                continue;
            }
            total += 1;
            if hits.len() < LIMIT {
                let is_dir = p.is_dir();
                hits.push(hit(p, is_dir));
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
    }
    fill_details(&mut hits);
    Some(Results { query: text.to_string(), hits, total, from_index: false, index_updated: None })
}

/// Live search walks everything; skip what the index would skip (folders named in
/// Settings), so both modes show the same kind of results.
fn in_skipped(p: &Path, root: &Path, cfg: &ef_config::SearchConfig) -> bool {
    let Ok(rel) = p.strip_prefix(root) else { return false };
    let comps: Vec<_> = rel.components().collect();
    // Every component except the last: a skipped folder is still found by name.
    comps.iter().take(comps.len().saturating_sub(1)).any(|c| cfg.exclude_names.iter().any(|n| c.as_os_str() == n.as_str()))
}

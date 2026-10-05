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
    let maps: Vec<_> = roots.iter().filter_map(|r| Some((&r.root, r.map.clone()?))).collect();
    if maps.is_empty() {
        return None;
    }
    let searched: Vec<PathBuf> = roots.iter().map(|r| r.root.clone()).collect();
    let mut hits = Vec::new();
    let mut total = 0;
    for (root, m) in &maps {
        let q = Query { text, hidden, ..Query::new(text) };
        let all = m.search(&Query { limit: None, ..q.clone() });
        let nested = config::nested_roots(&searched, root);
        if nested.is_empty() {
            total += all.len();
            for &id in all.iter() {
                hits.push(hit(m.path(id), m.kind(id) == ef_index::Kind::Dir));
            }
        } else {
            for &id in &all {
                let p = m.path(id);
                if nested.iter().any(|n| p.starts_with(n)) {
                    continue;
                }
                total += 1;
                {
                    hits.push(hit(p, m.kind(id) == ef_index::Kind::Dir));
                }
            }
        }
    }
    rank_hits(&mut hits, text);
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
        let nested = config::nested_roots(roots, root);
        let found = indexer::background_pool().install(|| ef_index::live::search_with(root, &q, &ef_index::Options::from_search(exclude, root), cancel, &|_| {})).ok()?;
        for p in found {
            if nested.iter().any(|n| p.starts_with(n)) || exclude.exclude_paths.iter().any(|e| p.starts_with(config::expand(e))) || in_skipped(&p, root, exclude) {
                continue;
            }
            total += 1;
            {
                let is_dir = p.is_dir();
                hits.push(hit(p, is_dir));
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
    }
    rank_hits(&mut hits, text);
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

/// Merge offline phone names into Everywhere search without touching the UI thread.
pub fn with_phone(results: Option<Results>, text: &str, hidden: bool, enabled: bool) -> Option<Results> {
    if !enabled { return results; }
    let mut results = results?;
    let q = Query { text, hidden, limit: Some(LIMIT), ..Query::new(text) };
    let (total, entries) = ef_index::phone::results(&q);
    results.total += total;
    for entry in entries {
        let mut h = hit(entry.uri, entry.directory); h.size = entry.size; h.mtime = entry.modified;
        h.location = "Phone · cached; reconnect to open".into(); results.hits.push(h);
    }
    rank_hits(&mut results.hits, text);
    Some(results)
}

fn rank_hits(hits: &mut Vec<Hit>, text: &str) {
    use std::os::unix::ffi::OsStrExt;
    hits.sort_by_cached_key(|h| (ef_index::rank_name(text, h.path.file_name().unwrap_or_default().as_bytes()), h.path.clone()));
    hits.dedup_by(|a, b| a.path == b.path);
    hits.truncate(LIMIT);
}

pub fn combined(roots: &[RootIndex], cfg: &ef_config::SearchConfig, text: &str, hidden: bool, cancel: &Arc<AtomicBool>) -> Option<Results> {
    let roots: Vec<_> = roots.iter().cloned().map(|mut r| { if r.map.as_ref().is_some_and(|m| m.options() != &ef_index::Options::from_search(cfg, &r.root)) { r.map = None; } r }).collect();
    let mut result = from_index(&roots, text, hidden);
    let missing: Vec<_> = cfg.roots().into_iter().filter(|p| !roots.iter().any(|r| &r.root == p && r.map.is_some())).collect();
    if !missing.is_empty() {
        let live = live(&missing, cfg, text, hidden, cancel)?;
        if let Some(r) = &mut result { r.total += live.total; r.hits.extend(live.hits); r.from_index = false; rank_hits(&mut r.hits, text); }
        else { result = Some(live); }
    }
    result
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn exact_match_in_later_root_survives_global_limit() {
        let root = tempfile::tempdir().unwrap(); let a = root.path().join("a"); let b = root.path().join("b");
        std::fs::create_dir(&a).unwrap(); std::fs::create_dir(&b).unwrap();
        for i in 0..300 { std::fs::write(a.join(format!("some-report-{i}")), b"").unwrap(); }
        std::fs::write(b.join("report"), b"").unwrap();
        let maps: Vec<_> = [&a, &b].iter().enumerate().map(|(i, p)| {
            let file = root.path().join(format!("{i}.index")); ef_index::Index::build(p).unwrap().save(&file).unwrap();
            RootIndex { root: p.to_path_buf(), map: Some(Arc::new(ef_index::MappedIndex::open(&file).unwrap())), updated: None, error: None }
        }).collect();
        let found = from_index(&maps, "report", false).unwrap(); assert_eq!(found.total, 301); assert_eq!(found.hits.len(), 300); assert_eq!(found.hits[0].name, "report");
    }
}

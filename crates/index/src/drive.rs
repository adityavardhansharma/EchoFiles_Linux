//! Google Drive catalogs in search (only with Settings → Google Drive → Include Google Drive
//! in search index). Same matcher and ranking as the phone's cached files. Results point at
//! the mounted folder when the account is mounted, else `gdrive://<account>/<path>` (the UI
//! offers to connect; `ef` prints it as is).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::phone::Entry;
use crate::{fold, matcher::Matcher, Kind, Query};

struct Cached {
    /// Path inside Drive.
    path: String,
    directory: bool,
    size: u64,
    modified: i64,
    folded: Vec<u8>,
    hidden: bool,
}

type Stamp = (u64, Option<SystemTime>, u64, Option<SystemTime>);
type Parsed = Arc<Vec<Cached>>;
static PARSED: Mutex<Option<HashMap<String, (Stamp, Parsed)>>> = Mutex::new(None);

fn stamp(base: &Path, delta: &Path) -> Stamp {
    let m = |p: &Path| std::fs::metadata(p).ok().map(|m| (m.len(), m.modified().ok()));
    let (bl, bt) = m(base).unwrap_or((0, None));
    let (dl, dt) = m(delta).unwrap_or((0, None));
    (bl, bt, dl, dt)
}

/// Accounts with a catalog on disk.
pub fn accounts() -> Vec<String> {
    accounts_in(&ef_config::cloud_index_dir())
}

fn accounts_in(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<String> = rd
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".jsonl")).filter(|n| !n.ends_with(".delta")).map(str::to_string))
        .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
        .collect();
    v.sort();
    v
}

fn parsed(dir: &Path, account: &str) -> Parsed {
    let base = dir.join(format!("{account}.jsonl"));
    let delta = dir.join(format!("{account}.delta.jsonl"));
    let st = stamp(&base, &delta);
    let mut g = PARSED.lock().unwrap_or_else(|e| e.into_inner());
    let cache = g.get_or_insert_with(HashMap::new);
    if let Some((s, p)) = cache.get(account)
        && *s == st
    {
        return p.clone();
    }
    let cat = ef_cloud::catalog::load_from(&base, &delta);
    let v: Vec<Cached> = cat
        .entries()
        .map(|e| {
            let name = e.name();
            // Google Docs are `Name.link.html` in the mount; search their real name.
            let shown = name.strip_suffix(ef_cloud::drive::LINK_SUFFIX).unwrap_or(name);
            Cached {
                path: e.path.clone(),
                directory: e.dir,
                size: e.size.max(0) as u64,
                modified: e.modified,
                folded: fold::fold(shown),
                hidden: e.path.split('/').any(|p| p.starts_with('.')),
            }
        })
        .collect();
    let p: Parsed = Arc::new(v);
    cache.insert(account.to_string(), (st, p.clone()));
    p
}

/// Where a result opens: the mount when it's there, else the Drive URI.
pub fn location(account: &str, path: &str) -> PathBuf {
    let mp = ef_cloud::paths::mount_point(account);
    if ef_cloud::drive::is_mount_point(&mp) {
        mp.join(path)
    } else {
        PathBuf::from(format!("{}{account}/{path}", ef_cloud::uri::SCHEME))
    }
}

pub fn entries(q: &Query<'_>) -> Vec<Entry> {
    search_counted(&ef_config::cloud_index_dir(), q).1
}

pub fn search_counted(dir: &Path, q: &Query<'_>) -> (usize, Vec<Entry>) {
    let Some(matcher) = Matcher::new(q) else { return (0, Vec::new()) };
    let mut hits: Vec<(u64, Entry)> = Vec::new();
    for account in accounts_in(dir) {
        let files = parsed(dir, &account);
        let mounted = ef_cloud::drive::is_mount_point(&ef_cloud::paths::mount_point(&account));
        for c in files.iter() {
            if (!q.hidden && c.hidden) || !matcher.accepts(&c.folded, if c.directory { Kind::Dir } else { Kind::File }) {
                continue;
            }
            let uri = if mounted { ef_cloud::paths::mount_point(&account).join(&c.path) } else { PathBuf::from(format!("{}{account}/{}", ef_cloud::uri::SCHEME, c.path)) };
            hits.push((matcher.rank(&c.folded), Entry { uri, directory: c.directory, size: c.size, modified: c.modified }));
        }
    }
    let total = hits.len();
    let order = |a: &(u64, Entry), b: &(u64, Entry)| a.0.cmp(&b.0).then(a.1.uri.cmp(&b.1.uri));
    let limit = q.limit.unwrap_or(usize::MAX);
    if limit < hits.len() {
        hits.select_nth_unstable_by(limit, order);
        hits.truncate(limit);
    }
    hits.sort_by(order);
    (total, hits.into_iter().map(|(_, e)| e).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_catalogs_with_deltas() {
        let d = tempfile::tempdir().unwrap();
        let (base, delta) = (d.path().join("k3f9q2.jsonl"), d.path().join("k3f9q2.delta.jsonl"));
        std::fs::write(&base, concat!(
            "{\"i\":\"w\",\"path\":\"Work\",\"d\":true}\n",
            "{\"i\":\"r\",\"p\":\"w\",\"path\":\"Work/Annual Report.pdf\",\"s\":42,\"m\":123}\n",
            "{\"i\":\"b\",\"p\":\"w\",\"path\":\"Work/Budget 2026.link.html\",\"s\":-1}\n",
            "{\"i\":\"h\",\"path\":\".secret/report.txt\"}\n",
        )).unwrap();
        let q = Query { ext: Some("pdf"), ..Query::new("report annual") };
        let (n, hits) = search_counted(d.path(), &q);
        assert_eq!(n, 1);
        assert_eq!((hits[0].size, hits[0].modified), (42, 123));
        assert!(hits[0].uri.to_string_lossy().starts_with("gdrive://k3f9q2/Work/Annual"));
        assert_eq!(search_counted(d.path(), &Query::new("budget")).0, 1, "Google Docs by their real name");
        assert_eq!(search_counted(d.path(), &Query { hidden: true, ..Query::new("report") }).0, 2);
        // A delta line shows up without rebuilding (new file stamp).
        std::fs::write(&delta, "{\"op\":\"del\",\"i\":\"r\"}\n").unwrap();
        assert_eq!(search_counted(d.path(), &Query::new("report")).0, 0);
        assert_eq!(accounts_in(d.path()), vec!["k3f9q2".to_string()]);
    }
}

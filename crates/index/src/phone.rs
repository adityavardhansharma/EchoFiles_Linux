use std::{io::BufRead, path::{Path, PathBuf}};
use crate::{Kind, Query, fold, matcher::Matcher};

#[derive(Clone)]
pub struct Entry {
    pub uri: PathBuf,
    pub directory: bool,
    pub size: u64,
    pub modified: i64,
}

/// URI paths cannot be opened locally; the UI resolves them over the phone link.
pub fn search(q: &Query<'_>) -> Vec<PathBuf> { entries(q).into_iter().map(|e| e.uri).collect() }
pub fn entries(q: &Query<'_>) -> Vec<Entry> { search_in(&ef_config::state_dir().join("phone-index"), q) }
pub fn results(q: &Query<'_>) -> (usize, Vec<Entry>) { search_counted(&ef_config::state_dir().join("phone-index"), q) }

fn search_in(directory: &Path, q: &Query<'_>) -> Vec<Entry> {
    search_counted(directory, q).1
}

/// One validated cache line, with its name already folded for matching.
struct Cached {
    entry: Entry,
    folded: Vec<u8>,
    hidden: bool,
}

/// Parsed cache files, reused while their size and modification time are unchanged, so
/// each keystroke searches memory rather than re-reading every JSONL file.
type Parsed = std::sync::Arc<Vec<Cached>>;
/// Cache file → (length, modification time, parsed lines).
type ParsedFiles = std::collections::HashMap<PathBuf, (u64, std::time::SystemTime, Parsed)>;
static PARSED: std::sync::Mutex<Option<ParsedFiles>> = std::sync::Mutex::new(None);

fn parse(path: &Path) -> Vec<Cached> {
    let Ok(file) = std::fs::File::open(path) else { return Vec::new() };
    let mut out = Vec::new();
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let (Some(id), Some(path)) = (v["device"].as_str(), v["path"].as_str()) else { continue };
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') || path.starts_with('/') || Path::new(path).components().any(|c| matches!(c, std::path::Component::ParentDir)) { continue; }
        let directory = v["directory"].as_bool().unwrap_or(false);
        out.push(Cached {
            folded: fold::fold(path.rsplit('/').next().unwrap_or(path)),
            hidden: path.split('/').any(|p| p.starts_with('.')),
            entry: Entry { uri: PathBuf::from(format!("phone://{id}/{path}")), directory, size: v["size"].as_u64().unwrap_or(0), modified: v["modified"].as_i64().unwrap_or(0) / 1000 },
        });
    }
    out
}

fn cached_files(directory: &Path) -> Vec<Parsed> {
    let Ok(files) = std::fs::read_dir(directory) else { return Vec::new() };
    let mut paths: Vec<PathBuf> = files.flatten().map(|f| f.path()).filter(|p| p.extension().is_some_and(|ext| ext == "jsonl")).collect();
    paths.sort();
    let mut guard = PARSED.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(Default::default);
    cache.retain(|p, _| !p.starts_with(directory) || paths.contains(p));
    paths.into_iter().filter_map(|path| {
        let m = std::fs::metadata(&path).ok()?;
        let (len, mtime) = (m.len(), m.modified().ok()?);
        if let Some((l, t, parsed)) = cache.get(&path) && *l == len && *t == mtime {
            return Some(parsed.clone());
        }
        let parsed: Parsed = std::sync::Arc::new(parse(&path));
        cache.insert(path, (len, mtime, parsed.clone()));
        Some(parsed)
    }).collect()
}

fn search_counted(directory: &Path, q: &Query<'_>) -> (usize, Vec<Entry>) {
    let Some(matcher) = Matcher::new(q) else { return (0, Vec::new()) };
    let files = cached_files(directory);
    let mut seen = std::collections::HashSet::new();
    let mut hits: Vec<(u64, &Entry)> = Vec::new();
    for file in &files {
        for c in file.iter() {
            if (!q.hidden && c.hidden) || !matcher.accepts(&c.folded, if c.entry.directory { Kind::Dir } else { Kind::File }) { continue; }
            if seen.insert(&c.entry.uri) { hits.push((matcher.rank(&c.folded), &c.entry)); }
        }
    }
    let total = hits.len();
    let order = |a: &(u64, &Entry), b: &(u64, &Entry)| a.0.cmp(&b.0).then(a.1.uri.cmp(&b.1.uri));
    let limit = q.limit.unwrap_or(usize::MAX);
    if limit < hits.len() { hits.select_nth_unstable_by(limit, order); hits.truncate(limit); }
    hits.sort_by(order);
    (total, hits.into_iter().map(|(_, e)| e.clone()).collect())
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn offline_filters_rank_and_metadata() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("phone.jsonl"), concat!(
            "{\"device\":\"abc\",\"path\":\"Docs/Annual Report.PDF\",\"size\":42,\"modified\":123000}\n",
            "{\"device\":\"abc\",\"path\":\".private/Report.pdf\"}\n",
            "{\"device\":\"abc\",\"path\":\"Report folder\",\"directory\":true}\n",
            "{\"device\":\"abc\",\"path\":\"../../Report.pdf\"}\n",
            "incomplete line\n"
        )).unwrap();
        let q = Query { ext: Some("pdf"), ..Query::new("report annual") };
        let hits = search_in(dir.path(), &q);
        assert_eq!(hits.len(), 1); assert_eq!(hits[0].size, 42); assert_eq!(hits[0].modified, 123);
        assert_eq!(search_in(dir.path(), &Query { hidden: true, ext: Some("pdf"), ..Query::new("report") }).len(), 2);
        assert_eq!(search_in(dir.path(), &Query { kind: crate::KindFilter::Dirs, ..Query::new("report") }).len(), 1);
        assert!(search_in(dir.path(), &Query { limit: Some(0), ..Query::new("*") }).is_empty());
        let (total, hits) = search_counted(dir.path(), &Query { limit: Some(1), ..Query::new("report") });
        assert_eq!(total, 2); assert_eq!(hits.len(), 1);
    }
}

// ---------------------------------------------------------------------------------------
// The running app's phone link, for `ef phone`
// ---------------------------------------------------------------------------------------

/// A paired phone as the app sees it now.
#[derive(Clone, Debug)]
pub struct LinkPhone {
    pub id: String,
    pub name: String,
    /// Reachable over Wi-Fi now (files don't go over Bluetooth).
    pub online: bool,
    /// Runs EchoConnect, which answers photo and file requests.
    pub echoconnect: bool,
}

/// What `ef phone` needs from the app's phone connection. The app installs one at
/// startup ([`set_link`]); an `ef` running in its own process has none.
pub trait Link: Send + Sync {
    fn phones(&self) -> Vec<LinkPhone>;
    /// Send `kind` with `body` (a fresh `requestId` is added) and wait for the packet that
    /// answers it.
    fn ask(&self, phone: &str, kind: &str, body: serde_json::Value, wait: std::time::Duration) -> Result<serde_json::Value, String>;
    /// Send a request the phone answers with a file, save that file into `dir` and return
    /// its path. `wait` bounds the whole exchange; a late transfer is cancelled.
    fn fetch(&self, phone: &str, kind: &str, body: serde_json::Value, dir: &Path, wait: std::time::Duration) -> Result<PathBuf, String>;
}

static LINK: std::sync::OnceLock<Box<dyn Link>> = std::sync::OnceLock::new();

/// Called once by the app.
pub fn set_link(link: Box<dyn Link>) {
    let _ = LINK.set(link);
}

pub fn link() -> Option<&'static dyn Link> {
    LINK.get().map(|b| b.as_ref())
}

/// The full-storage file index's saved entries for one phone (`phone_index` on), as
/// (path, size, modified seconds, directory).
pub fn indexed_files(device: &str) -> Vec<(String, u64, i64, bool)> {
    let path = ef_config::state_dir().join("phone-index").join(format!("{device}.jsonl"));
    parse(&path).into_iter().filter_map(|c| {
        let uri = c.entry.uri.to_string_lossy().into_owned();
        let rel = uri.strip_prefix("phone://")?.split_once('/')?.1.to_owned();
        Some((rel, c.entry.size, c.entry.modified, c.entry.directory))
    }).collect()
}

use std::{collections::BTreeMap, io::BufRead, path::{Path, PathBuf}};
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

fn search_counted(directory: &Path, q: &Query<'_>) -> (usize, Vec<Entry>) {
    let Some(matcher) = Matcher::new(q) else { return (0, Vec::new()) };
    let mut entries = BTreeMap::new();
    let Ok(files) = std::fs::read_dir(directory) else { return (0, Vec::new()) };
    for file in files.flatten().filter(|f| f.path().extension().is_some_and(|ext| ext == "jsonl")) {
        if let Ok(file) = std::fs::File::open(file.path()) {
            for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
                let (Some(id), Some(path)) = (v["device"].as_str(), v["path"].as_str()) else { continue };
                if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') || path.starts_with('/') || Path::new(path).components().any(|c| matches!(c, std::path::Component::ParentDir)) { continue; }
                if !q.hidden && path.split('/').any(|p| p.starts_with('.')) { continue; }
                let directory = v["directory"].as_bool().unwrap_or(false);
                let name = fold::fold(path.rsplit('/').next().unwrap_or(path));
                if matcher.accepts(&name, if directory { Kind::Dir } else { Kind::File }) {
                    let uri = PathBuf::from(format!("phone://{id}/{path}"));
                    entries.insert(uri.clone(), (matcher.rank(&name), Entry { uri, directory, size: v["size"].as_u64().unwrap_or(0), modified: v["modified"].as_i64().unwrap_or(0) / 1000 }));
                }
            }
        }
    }
    let mut entries: Vec<_> = entries.into_values().collect();
    entries.sort_by(|a,b| a.0.cmp(&b.0).then(a.1.uri.cmp(&b.1.uri)));
    (entries.len(), entries.into_iter().take(q.limit.unwrap_or(usize::MAX)).map(|(_,e)| e).collect())
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

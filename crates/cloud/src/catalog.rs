//! The saved Drive index for one account (only with Settings → Google Drive → Include
//! Google Drive in search index): `<id>.jsonl` from the last full crawl plus
//! `<id>.delta.jsonl`, an append-only list of changes since. Reads apply the delta in order;
//! the next full crawl writes a new base and empties the delta.
//!
//! Base lines are entries: `{"i": id, "p": parent id, "path": "Work/a.pdf", "d": false,
//! "s": size, "m": unix seconds, "t": mime, "h": md5}`. Delta lines add `"op"`: `put` (an
//! entry), `del` (`"i"`, or `"path"`; a folder takes everything under it), or `folder`
//! (`"path"` and `"entries"`: that folder's direct children are exactly these). Metadata
//! only — never contents.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use serde_json::{json, Value};

use crate::drive::Entry as DriveEntry;
use crate::paths;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub parent: String,
    /// `/`-separated path inside Drive.
    pub path: String,
    pub dir: bool,
    /// -1 when Drive has no size (Google Docs).
    pub size: i64,
    pub modified: i64,
    pub mime: String,
    pub md5: String,
}

impl Entry {
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    pub fn dirname(&self) -> &str {
        self.path.rsplit_once('/').map_or("", |(d, _)| d)
    }

    fn key(&self) -> &str {
        if self.id.is_empty() { &self.path } else { &self.id }
    }

    pub fn to_json(&self) -> Value {
        let mut v = json!({ "i": self.id, "p": self.parent, "path": self.path, "d": self.dir, "s": self.size, "m": self.modified });
        if !self.mime.is_empty() {
            v["t"] = json!(self.mime);
        }
        if !self.md5.is_empty() {
            v["h"] = json!(self.md5);
        }
        v
    }

    pub fn from_json(v: &Value) -> Option<Entry> {
        let path = v.get("path")?.as_str()?.trim_matches('/').to_string();
        if path.is_empty() || path.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
            return None;
        }
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Some(Entry {
            id: s("i"),
            parent: s("p"),
            dir: v.get("d").and_then(Value::as_bool).unwrap_or(false),
            size: v.get("s").and_then(Value::as_i64).unwrap_or(-1),
            modified: v.get("m").and_then(Value::as_i64).unwrap_or(0),
            mime: s("t"),
            md5: s("h"),
            path,
        })
    }

    /// From an `operations/list` / `lsjson` item (parent filled in later).
    pub fn from_drive(e: &DriveEntry, md5: String) -> Entry {
        Entry { id: e.id.clone(), parent: String::new(), path: e.path.trim_matches('/').to_string(), dir: e.dir, size: e.size, modified: e.modified, mime: e.mime.clone(), md5 }
    }
}

/// Entries by key with path lookups — what a catalog is once read.
#[derive(Debug, Default, Clone)]
pub struct Catalog {
    by_key: HashMap<String, Entry>,
}

impl Catalog {
    pub fn from_entries(entries: impl IntoIterator<Item = Entry>) -> Catalog {
        let mut c = Catalog::default();
        for e in entries {
            c.by_key.insert(e.key().to_string(), e);
        }
        c
    }

    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.by_key.values()
    }

    pub fn by_id(&self, id: &str) -> Option<&Entry> {
        self.by_key.get(id)
    }

    /// Folder ID → path, for live search results that only know their parent's ID.
    pub fn folder_paths(&self) -> HashMap<String, String> {
        self.by_key.values().filter(|e| e.dir && !e.id.is_empty()).map(|e| (e.id.clone(), e.path.clone())).collect()
    }

    fn id_of(&self, path: &str) -> String {
        self.by_key.values().find(|e| e.path == path).map(|e| e.id.clone()).unwrap_or_default()
    }

    pub fn put(&mut self, mut e: Entry) {
        if e.parent.is_empty() {
            e.parent = self.id_of(e.dirname());
        }
        // A rename keeps the ID; drop the old path's entry if it was keyed by path.
        self.by_key.retain(|_, x| !(x.id.is_empty() && x.path == e.path));
        self.by_key.insert(e.key().to_string(), e);
    }

    /// Remove by ID or path; a folder takes everything under it.
    pub fn remove(&mut self, id: &str, path: &str) {
        let path = if path.is_empty() { self.by_key.get(id).map(|e| e.path.clone()).unwrap_or_default() } else { path.to_string() };
        let under = format!("{path}/");
        self.by_key.retain(|k, e| k != id && (path.is_empty() || (e.path != path && !e.path.starts_with(&under))));
    }

    /// `dir`'s direct children are exactly `entries` (a fresh listing of one folder).
    pub fn replace_folder(&mut self, dir: &str, entries: Vec<Entry>) {
        let dir = dir.trim_matches('/');
        let gone: Vec<(String, String)> = self.by_key.values().filter(|e| e.dirname() == dir && !entries.iter().any(|n| n.key() == e.key() || n.path == e.path)).map(|e| (e.id.clone(), e.path.clone())).collect();
        for (id, path) in gone {
            self.remove(&id, &path);
        }
        let parent = self.id_of(dir);
        for mut e in entries {
            if e.parent.is_empty() {
                e.parent = parent.clone();
            }
            // Keep the hash a crawl found; a folder listing doesn't ask for it.
            if e.md5.is_empty()
                && let Some(old) = self.by_key.get(e.key())
            {
                e.md5 = old.md5.clone();
            }
            self.put(e);
        }
    }

    /// Fill parent IDs from paths (a crawl lists paths and IDs, not parents).
    pub fn link_parents(&mut self) {
        let ids: HashMap<String, String> = self.by_key.values().filter(|e| e.dir).map(|e| (e.path.clone(), e.id.clone())).collect();
        for e in self.by_key.values_mut() {
            if e.parent.is_empty() {
                e.parent = ids.get(e.dirname()).cloned().unwrap_or_default();
            }
        }
    }

    fn apply(&mut self, v: &Value) {
        match v.get("op").and_then(Value::as_str) {
            Some("put") => {
                if let Some(e) = Entry::from_json(v) {
                    self.put(e);
                }
            }
            Some("del") => {
                let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                self.remove(&s("i"), &s("path"));
            }
            Some("folder") => {
                let dir = v.get("path").and_then(Value::as_str).unwrap_or("");
                let entries = v.get("entries").and_then(Value::as_array).map(|a| a.iter().filter_map(Entry::from_json).collect()).unwrap_or_default();
                self.replace_folder(dir, entries);
            }
            _ => {}
        }
    }
}

fn lines(path: &Path) -> impl Iterator<Item = Value> {
    std::fs::File::open(path).ok().into_iter().flat_map(|f| io::BufReader::new(f).lines().map_while(Result::ok)).filter_map(|l| serde_json::from_str::<Value>(&l).ok())
}

/// Read a catalog: base, then the delta in order.
pub fn load_from(base: &Path, delta: &Path) -> Catalog {
    let mut c = Catalog::from_entries(lines(base).filter_map(|v| Entry::from_json(&v)));
    for v in lines(delta) {
        c.apply(&v);
    }
    c
}

pub fn load(account: &str) -> Catalog {
    load_from(&paths::catalog(account), &paths::delta(account))
}

pub fn exists(account: &str) -> bool {
    paths::catalog(account).exists()
}

/// Write a fresh base (after a full crawl) and empty the delta. Atomic: a failed write
/// leaves the old catalog.
pub fn write_base(account: &str, c: &Catalog) -> io::Result<()> {
    let base = paths::catalog(account);
    write_base_to(&base, &paths::delta(account), c)
}

pub fn write_base_to(base: &Path, delta: &Path, c: &Catalog) -> io::Result<()> {
    if let Some(dir) = base.parent() {
        paths::private_dir(dir)?;
    }
    let tmp = base.with_extension("jsonl.tmp");
    {
        let f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        let mut w = io::BufWriter::new(f);
        for e in c.entries() {
            serde_json::to_writer(&mut w, &e.to_json())?;
            w.write_all(b"\n")?;
        }
        w.flush()?;
    }
    std::fs::rename(&tmp, base)?;
    let _ = std::fs::remove_file(delta);
    Ok(())
}

/// One change, appended to the delta (only if the account has a catalog).
pub fn append(account: &str, change: &Value) -> io::Result<()> {
    append_to(&paths::catalog(account), &paths::delta(account), change)
}

pub fn append_to(base: &Path, delta: &Path, change: &Value) -> io::Result<()> {
    if !base.exists() {
        return Ok(());
    }
    let mut f = std::fs::OpenOptions::new().append(true).create(true).mode(0o600).open(delta)?;
    let mut line = serde_json::to_vec(change)?;
    line.push(b'\n');
    f.write_all(&line)
}

pub fn put_change(e: &Entry) -> Value {
    let mut v = e.to_json();
    v["op"] = json!("put");
    v
}

pub fn del_change(id: &str, path: &str) -> Value {
    json!({ "op": "del", "i": id, "path": path })
}

pub fn folder_change(dir: &str, entries: &[Entry]) -> Value {
    json!({ "op": "folder", "path": dir.trim_matches('/'), "entries": entries.iter().map(Entry::to_json).collect::<Vec<_>>() })
}

/// Delete an account's catalog (switch off, sign out).
pub fn remove(account: &str) {
    let _ = std::fs::remove_file(paths::catalog(account));
    let _ = std::fs::remove_file(paths::delta(account));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: &str, path: &str, dir: bool) -> Entry {
        Entry { id: id.into(), parent: String::new(), path: path.into(), dir, size: 1, modified: 0, mime: String::new(), md5: String::new() }
    }

    #[test]
    fn base_delta_and_folders() {
        let d = tempfile::tempdir().unwrap();
        let (base, delta) = (d.path().join("a.jsonl"), d.path().join("a.delta.jsonl"));
        let mut c = Catalog::from_entries([e("w", "Work", true), e("r", "Work/report.pdf", false), e("o", "Work/old.txt", false), e("x", "Work/Sub", true), e("y", "Work/Sub/deep.txt", false)]);
        c.link_parents();
        assert_eq!(c.by_id("r").unwrap().parent, "w");
        write_base_to(&base, &delta, &c).unwrap();
        append_to(&base, &delta, &put_change(&e("n", "Work/new.pdf", false))).unwrap();
        append_to(&base, &delta, &del_change("x", "")).unwrap();
        append_to(&base, &delta, &folder_change("Work", &[e("r", "Work/report.pdf", false), e("n", "Work/new.pdf", false), e("z", "Work/zeta.md", false)])).unwrap();
        let c = load_from(&base, &delta);
        let mut paths: Vec<&str> = c.entries().map(|e| e.path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, ["Work", "Work/new.pdf", "Work/report.pdf", "Work/zeta.md"], "old.txt gone by folder listing, Sub and its child gone by del");
        assert_eq!(c.by_id("z").unwrap().parent, "w");
        assert_eq!(c.folder_paths().get("w").map(String::as_str), Some("Work"));
        // A full crawl empties the delta.
        write_base_to(&base, &delta, &c).unwrap();
        assert!(!delta.exists());
        // No catalog, no delta written (index off).
        let none = d.path().join("none.jsonl");
        append_to(&none, &d.path().join("none.delta.jsonl"), &del_change("a", "")).unwrap();
        assert!(!d.path().join("none.delta.jsonl").exists());
        // Bad lines and escapes are skipped.
        std::fs::write(&base, "{\"i\":\"a\",\"path\":\"../x\"}\nnot json\n{\"i\":\"b\",\"path\":\"ok.txt\"}\n").unwrap();
        assert_eq!(load_from(&base, &delta).len(), 1);
        use std::os::unix::fs::PermissionsExt;
        write_base_to(&base, &delta, &c).unwrap();
        assert_eq!(std::fs::metadata(&base).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

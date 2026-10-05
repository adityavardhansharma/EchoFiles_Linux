//! Live search: a parallel walk that matches names as it reads them. No index needed, and
//! always current. It's for places the index doesn't cover (USB sticks, network shares,
//! `/tmp`), for before the first crawl finishes, and for confirming indexed results.
//!
//! It uses the same [`Query`] and the same matching and ranking as [`Index::search`], so the
//! two give the same set of results for the same folder. Differences:
//! - `Query::within` is ignored: the walk starts at the `root` path it's given.
//! - Ties in ranking are broken by path, not by index id.
//! - Hidden folders are skipped without being read, unless `Query::hidden` is set.
//!
//! [`Index::search`]: crate::Index::search

use std::ffi::OsString;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use rustix::fs::{AtFlags, FileType, RawDir};

use crate::matcher::{self, Matcher};
use crate::{Kind, Query, fold, open_dir};

struct Walk<'a> {
    m: &'a Matcher,
    opts: &'a crate::Options,
    hidden: bool,
    cancel: &'a AtomicBool,
    on_hits: &'a (dyn Fn(&[PathBuf]) + Sync),
    /// (rank key, path bytes): bytes compare with one memcmp, where `Path` compares
    /// component by component.
    hits: Mutex<Vec<(u64, Vec<u8>)>>,
}

/// Search below `root`, best first. `on_hits` is called from worker threads with each
/// folder's hits as soon as that folder is read (unranked), so a UI can show results while
/// the walk runs. Setting `cancel` stops the walk; the call then returns
/// `ErrorKind::Interrupted`.
pub fn search(root: &Path, q: &Query, cancel: &AtomicBool, on_hits: &(dyn Fn(&[PathBuf]) + Sync)) -> io::Result<Vec<PathBuf>> {
    search_with(root, q, &crate::Options::everything(), cancel, on_hits)
}

pub fn search_with(root: &Path, q: &Query, opts: &crate::Options, cancel: &AtomicBool, on_hits: &(dyn Fn(&[PathBuf]) + Sync)) -> io::Result<Vec<PathBuf>> {
    if opts.excludes_path(root) { return Ok(Vec::new()); }
    drop(open_dir(root.as_os_str().as_bytes(), true)?);
    let Some(m) = Matcher::new(q) else {
        return Ok(Vec::new());
    };
    let w = Walk { m: &m, opts, hidden: q.hidden, cancel, on_hits, hits: Mutex::new(Vec::new()) };
    let path = root.as_os_str().as_bytes().to_vec();
    rayon::scope(|s| walk(path, true, &w, s));
    if cancel.load(Ordering::Relaxed) {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "search cancelled"));
    }
    let mut hits = w.hits.into_inner().unwrap();
    matcher::keep_best(&mut hits, q.limit);
    hits.sort_unstable();
    Ok(hits.into_iter().map(|(_, p)| PathBuf::from(OsString::from_vec(p))).collect())
}

fn walk<'s>(path: Vec<u8>, root: bool, w: &'s Walk<'s>, scope: &rayon::Scope<'s>) {
    if w.cancel.load(Ordering::Relaxed) {
        return;
    }
    let p = Path::new(std::ffi::OsStr::from_bytes(&path));
    if w.opts.excludes_path(p) { return; }
    if !root && w.opts.skip_cache_tagged && std::fs::symlink_metadata(p.join("CACHEDIR.TAG")).is_ok() { return; }
    let Ok(fd) = open_dir(&path, root) else { return };
    let mut buf: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); 64 * 1024];
    let mut raw = RawDir::new(&fd, &mut buf);
    let mut folded = Vec::new();
    let mut keys: Vec<u64> = Vec::new();
    let mut found: Vec<PathBuf> = Vec::new();
    let mut children = Vec::new();
    while let Some(entry) = raw.next() {
        let Ok(e) = entry else { break };
        let name = e.file_name().to_bytes();
        if name == b"." || name == b".." || (!w.hidden && name[0] == b'.') {
            continue;
        }
        let mut t = e.file_type();
        if t == FileType::Unknown {
            t = rustix::fs::statat(&fd, e.file_name(), AtFlags::SYMLINK_NOFOLLOW)
                .map(|st| FileType::from_raw_mode(st.st_mode))
                .unwrap_or(FileType::Unknown);
        }
        let kind = Kind::from_file_type(t);
        let full = || {
            let mut p = path.clone();
            p.push(b'/');
            p.extend_from_slice(name);
            p
        };
        if w.opts.excludes_path(Path::new(std::ffi::OsStr::from_bytes(&full()))) { continue; }
        folded.clear();
        fold::fold_into(name, &mut folded);
        if w.m.accepts(&folded, kind) {
            keys.push(w.m.rank(&folded));
            found.push(PathBuf::from(OsString::from_vec(full())));
        }
        if kind == Kind::Dir && !w.opts.excludes(name) {
            children.push(full());
        }
    }
    drop(fd);
    if !found.is_empty() {
        (w.on_hits)(&found);
        let batch = keys.into_iter().zip(found.into_iter().map(|p| p.into_os_string().into_vec()));
        w.hits.lock().unwrap().extend(batch);
    }
    for p in children {
        scope.spawn(move |s| walk(p, false, w, s));
    }
}

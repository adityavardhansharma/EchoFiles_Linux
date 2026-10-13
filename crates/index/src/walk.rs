//! The live walk behind `ef find`/`ef fd` for folders the index doesn't cover (or when asked
//! to be exact): one rayon task per folder, raw `getdents64` listings into a 64 KiB buffer,
//! names matched as they are read, `stat` only when a filter needs it.
//!
//! Compared with a `walkdir`/`ignore`-style walker it avoids a `PathBuf` and a heap
//! `DirEntry` per entry (paths are built only for entries that are kept or descended
//! into), checks ignore rules against the listing it already has instead of probing for
//! every rule file, and `stat`s relative to the open folder.

use std::ffi::OsStr;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rustix::fd::{AsFd, BorrowedFd, OwnedFd};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RawDir};

use crate::filter::{FType, Hit, Probe};
use crate::find::Search;
use crate::fold;
use crate::gitignore::{DirRules, Stack, Verdict};

/// (device, inode) of the folders above, for loop detection when following links.
pub(crate) struct Ancestors {
    id: (u64, u64),
    parent: Option<Arc<Ancestors>>,
}

pub(crate) struct Walk<'a> {
    pub s: &'a Search,
    pub root: u16,
    pub root_dev: u64,
    /// One batch per folder; flattened once at the end (no repeated copying).
    pub hits: Mutex<Vec<Vec<Hit>>>,
    /// Fold names (word patterns or word ranking need it).
    pub fold: bool,
    pub found: &'a AtomicUsize,
    pub stop: &'a AtomicBool,
    pub errors: &'a Mutex<Vec<String>>,
}

fn open_dir(path: &[u8], follow: bool) -> std::io::Result<OwnedFd> {
    let mut flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    if !follow {
        flags |= OFlags::NOFOLLOW;
    }
    Ok(rustix::fs::open(OsStr::from_bytes(path), flags, Mode::empty())?)
}

fn join(dir: &[u8], name: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(dir.len() + 1 + name.len());
    p.extend_from_slice(dir);
    if p.last() != Some(&b'/') {
        p.push(b'/');
    }
    p.extend_from_slice(name);
    p
}

impl<'a> Walk<'a> {
    pub fn into_hits(self) -> Vec<Hit> {
        let batches = self.hits.into_inner().unwrap();
        let mut out = Vec::with_capacity(batches.iter().map(Vec::len).sum());
        for mut b in batches {
            out.append(&mut b);
        }
        out
    }

    pub fn new(s: &'a Search, root: u16, root_dev: u64, found: &'a AtomicUsize, stop: &'a AtomicBool, errors: &'a Mutex<Vec<String>>) -> Walk<'a> {
        Walk { s, root, root_dev, hits: Mutex::new(Vec::new()), fold: s.filters.needs_fold(s.rank), found, stop, errors }
    }

    fn error(&self, path: &[u8], e: impl std::fmt::Display) {
        if self.s.show_errors {
            self.errors.lock().unwrap().push(format!("{}: {e}", String::from_utf8_lossy(path)));
        }
    }

    fn halted(&self) -> bool {
        self.stop.load(Ordering::Relaxed) || self.s.cancel.load(Ordering::Relaxed)
    }

    /// Walk the contents of folder `path` (whose own depth is `depth`). `stack` holds the
    /// rule files of the folders above it; its own are read here.
    pub fn dir<'s>(&'s self, path: Vec<u8>, depth: u32, stack: Arc<Stack>, anc: Option<Arc<Ancestors>>, is_root: bool, scope: &rayon::Scope<'s>) {
        if self.halted() {
            return;
        }
        let f = &self.s.filters;
        let fd = match open_dir(&path, is_root || f.follow) {
            Ok(fd) => fd,
            Err(e) => {
                self.error(&path, e);
                return;
            }
        };
        // With -L, remember the folders on the way down; a symlink back to one of them is a
        // loop (checked when the link is listed, as fd does — only links are checked).
        let mut anc = anc;
        if f.follow
            && let Ok(st) = rustix::fs::fstat(&fd)
        {
            anc = Some(Arc::new(Ancestors { id: (st.st_dev, st.st_ino), parent: anc }));
        }
        // Read the whole listing first: rule files and `.git` must be known before any
        // entry is judged.
        thread_local! {
            static BUF: std::cell::RefCell<Vec<MaybeUninit<u8>>> = std::cell::RefCell::new(vec![MaybeUninit::uninit(); 64 * 1024]);
        }
        let mut buf = BUF.with(|b| std::mem::take(&mut *b.borrow_mut()));
        if buf.is_empty() {
            buf = vec![MaybeUninit::uninit(); 64 * 1024];
        }
        let mut raw = RawDir::new(&fd, &mut buf);
        let mut names: Vec<u8> = Vec::new();
        let mut ents: Vec<(u32, u32, FType)> = Vec::new();
        while let Some(entry) = raw.next() {
            let e = match entry {
                Ok(e) => e,
                Err(err) => {
                    self.error(&path, err);
                    break;
                }
            };
            let name = e.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            let start = names.len() as u32;
            names.extend_from_slice(name);
            ents.push((start, names.len() as u32, FType::from_file_type(e.file_type())));
        }
        BUF.with(|b| *b.borrow_mut() = buf);
        let name_of = |e: &(u32, u32, FType)| &names[e.0 as usize..e.1 as usize];
        let g = stack.globals();
        let stack = if g.opts.any() {
            let has = |n: &[u8]| ents.iter().any(|e| name_of(e) == n);
            match DirRules::load(&path, &has, &g.opts) {
                Some(r) => stack.push(r),
                None => stack,
            }
        } else {
            stack
        };
        let d = depth + 1;
        if f.max_depth.is_some_and(|m| d > m) {
            return;
        }
        let mut hits = Vec::new();
        let mut children = Vec::new();
        let mut scratch = Vec::new();
        for e in &ents {
            self.visit(Some(fd.as_fd()), &path, name_of(e), e.2, d, &stack, &anc, &mut hits, &mut children, &mut scratch);
        }
        drop(fd);
        self.flush(hits, children, d, &stack, &anc, scope);
    }

    fn flush<'s>(&'s self, mut hits: Vec<Hit>, children: Vec<Vec<u8>>, d: u32, stack: &Arc<Stack>, anc: &Option<Arc<Ancestors>>, scope: &rayon::Scope<'s>) {
        if !hits.is_empty() {
            match &self.s.sink {
                Some(sink) => sink.take(std::mem::take(&mut hits)),
                None => self.hits.lock().unwrap().push(std::mem::take(&mut hits)),
            }
        }
        for c in children {
            let st = stack.clone();
            let an = anc.clone();
            scope.spawn(move |s| self.dir(c, d, st, an, false, s));
        }
    }

    /// One entry named in an index but missing from it (an excluded path): judge and walk it
    /// as if it had been listed in `parent`.
    pub fn entry<'s>(&'s self, parent: &[u8], name: &[u8], parent_depth: u32, stack: Arc<Stack>, scope: &rayon::Scope<'s>) {
        let abs = join(parent, name);
        let Ok(st) = rustix::fs::statat(rustix::fs::CWD, OsStr::from_bytes(&abs), AtFlags::SYMLINK_NOFOLLOW) else { return };
        let d = parent_depth + 1;
        if self.s.filters.max_depth.is_some_and(|m| d > m) {
            return;
        }
        let (mut hits, mut children, mut scratch) = (Vec::new(), Vec::new(), Vec::new());
        self.visit(None, parent, name, FType::from_mode(st.st_mode), d, &stack, &None, &mut hits, &mut children, &mut scratch);
        self.flush(hits, children, d, &stack, &None, scope);
    }

    /// Judge one listed entry at depth `d`: keep it as a hit, and/or queue it to descend.
    #[allow(clippy::too_many_arguments)]
    fn visit(&self, fd: Option<BorrowedFd>, path: &[u8], name: &[u8], listed: FType, d: u32, stack: &Arc<Stack>, anc: &Option<Arc<Ancestors>>, hits: &mut Vec<Hit>, children: &mut Vec<Vec<u8>>, folded: &mut Vec<u8>) {
        let f = &self.s.filters;
        let g = stack.globals();
        let ignoring = g.opts.any() || g.overrides.is_some() || !g.explicit.is_empty();
        let hidden = name[0] == b'.';
        if hidden && !f.hidden && !ignoring {
            return;
        }
        let abs = join(path, name);
        let stat = |follow: bool| {
            let flags = if follow { AtFlags::empty() } else { AtFlags::SYMLINK_NOFOLLOW };
            match fd {
                Some(fd) => rustix::fs::statat(fd, OsStr::from_bytes(name), flags),
                None => rustix::fs::statat(rustix::fs::CWD, OsStr::from_bytes(&abs), flags),
            }
        };
        let mut t = listed;
        if t == FType::Unknown {
            t = stat(false).map(|s| FType::from_mode(s.st_mode)).unwrap_or(FType::Unknown);
        }
        let mut is_dir = t == FType::Dir;
        if f.follow && t == FType::Symlink
            && let Ok(target) = stat(true)
            && FileType::from_raw_mode(target.st_mode) == FileType::Directory
        {
            // A link back to a folder above: fd reports the loop and drops the entry.
            let id = (target.st_dev, target.st_ino);
            if std::iter::successors(anc.as_deref(), |a| a.parent.as_deref()).any(|a| a.id == id) {
                self.error(&join(path, name), "file system loop found");
                return;
            }
            is_dir = true;
        }
        if ignoring {
            let v = stack.check(&abs, is_dir);
            if v.is_ignore() || (hidden && !f.hidden && !matches!(v, Verdict::Whitelist(..))) {
                return;
            }
        }
        if is_dir && !f.ignore_contain.is_empty() {
            let marked = f.ignore_contain.iter().any(|m| rustix::fs::statat(rustix::fs::CWD, OsStr::from_bytes(&join(&abs, m)), AtFlags::SYMLINK_NOFOLLOW).is_ok());
            if marked {
                return;
            }
        }
        let mut matched = false;
        let mut hit = None;
        if d >= f.min_depth {
            let fold_it = self.fold;
            if fold_it {
                folded.clear();
                fold::fold_into(name, folded);
            }
            let fo = fold_it.then_some(folded.as_slice());
            if f.name_ok(name, fo, &abs) {
                let mut probe = Probe::new(fd, name, &abs);
                if let Some((ft, meta)) = f.rest_ok(t, &mut probe) {
                    matched = true;
                    let rank = if self.s.rank { f.rank(name, fo) } else { 0 };
                    hit = Some((ft, meta, rank));
                    if let Some(max) = self.s.max_results
                        && self.found.fetch_add(1, Ordering::Relaxed) + 1 >= max
                    {
                        self.stop.store(true, Ordering::Relaxed);
                    }
                }
            }
        }
        let descend = is_dir
            && f.max_depth.is_none_or(|m| d < m)
            && !(matched && f.prune)
            && (!f.one_file_system || stat(f.follow).is_ok_and(|s| s.st_dev == self.root_dev));
        match (hit, descend) {
            (Some((ft, meta, rank)), true) => {
                children.push(abs.clone());
                hits.push(Hit { path: abs, root: self.root, depth: d, ftype: ft, meta: meta.map(Box::new), rank });
            }
            (Some((ft, meta, rank)), false) => hits.push(Hit { path: abs, root: self.root, depth: d, ftype: ft, meta: meta.map(Box::new), rank }),
            (None, true) => children.push(abs),
            (None, false) => {}
        }
    }
}

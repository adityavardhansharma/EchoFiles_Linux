//! File-name index for EchoFiles search (build plan §2.8).
//!
//! One parallel crawl (names + `d_type` only, no `statx`) produces a struct-of-arrays index
//! in **depth-first preorder**: every folder's subtree is one contiguous id range
//! `[id, end[id])`. That gives the app two things cheaply:
//!
//! - "Search in this folder" scans only that folder's slice of the name arena, so a search
//!   in a 100-file folder costs the same whatever the size of the whole index.
//! - [`Index::rescan`] re-crawls one folder and splices the fresh subtree in place, which is
//!   how inotify events and idle re-crawls keep the index current.
//!
//! Names are matched in their folded form (see [`fold`]): case-, accent- and
//! width-insensitive. A query is split on whitespace into terms that must all occur in the
//! name, in any order; `"double quotes"` keep a phrase together. Hits are ranked: exact name,
//! then prefix, then word start, then anywhere; shorter names first within each group.

pub mod corpus;
pub mod fold;
mod store;

use std::ffi::OsStr;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use memchr::memmem;
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RawDir};

/// Id of the indexed root folder. Its subtree is the whole index.
pub const ROOT: u32 = 0;
const NONE: u32 = u32::MAX;

const KIND_MASK: u8 = 0b11;
const HIDDEN: u8 = 0b100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    File,
    Dir,
    Symlink,
    Other,
}

impl Kind {
    fn from_bits(b: u8) -> Kind {
        match b & KIND_MASK {
            0 => Kind::File,
            1 => Kind::Dir,
            2 => Kind::Symlink,
            _ => Kind::Other,
        }
    }

    fn from_file_type(t: FileType) -> Kind {
        match t {
            FileType::RegularFile => Kind::File,
            FileType::Directory => Kind::Dir,
            FileType::Symlink => Kind::Symlink,
            _ => Kind::Other,
        }
    }
}

/// Which entry kinds a query returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum KindFilter {
    #[default]
    Any,
    /// Everything that is not a directory.
    Files,
    Dirs,
}

/// A search. `Query::new("report")` searches the whole index for visible entries.
#[derive(Clone, Debug)]
pub struct Query<'a> {
    pub text: &'a str,
    /// Only descendants of this entry (from [`Index::lookup`]); [`ROOT`] for everything.
    pub within: u32,
    pub kind: KindFilter,
    /// Name must end in `.ext` (case-insensitive, without the dot: `"pdf"`, `"tar.gz"`).
    pub ext: Option<&'a str>,
    /// Include entries with a dot-named component below `within`.
    pub hidden: bool,
    /// Return only the best `limit` hits.
    pub limit: Option<usize>,
}

impl<'a> Query<'a> {
    pub fn new(text: &'a str) -> Query<'a> {
        Query { text, within: ROOT, kind: KindFilter::Any, ext: None, hidden: false, limit: None }
    }
}

/// Crawl counters for the last build or rescan.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Folders that could not be opened (permissions, removed mid-crawl). Indexed as empty.
    pub unreadable: u32,
}

pub struct Index {
    root: PathBuf,
    parent: Vec<u32>,
    /// One past the last id in each entry's subtree (`id + 1` for files and empty folders).
    end: Vec<u32>,
    /// Kind in the low bits, [`HIDDEN`] if the entry's own name starts with a dot.
    flags: Vec<u8>,
    /// `names[off[i]..off[i + 1] - 1]` is entry `i`'s name; each name is NUL-terminated.
    off: Vec<u32>,
    names: Vec<u8>,
    /// The same layout for the folded names that queries run against.
    foff: Vec<u32>,
    folded: Vec<u8>,
    stats: Stats,
}

// ---------------------------------------------------------------------------------------
// Crawl
// ---------------------------------------------------------------------------------------

/// One folder's children, gathered by one rayon task.
struct DirBatch {
    /// Crawl id of this folder (crawl ids number folders only).
    dir: u32,
    flags: Vec<u8>,
    ends: Vec<u32>,
    names: Vec<u8>,
    /// Crawl id of each child folder, [`NONE`] for other kinds.
    sub: Vec<u32>,
}

struct Crawl {
    next_dir: AtomicU32,
    unreadable: AtomicU32,
    batches: Mutex<Vec<DirBatch>>,
}

fn open_dir(path: &[u8], follow: bool) -> io::Result<rustix::fd::OwnedFd> {
    let mut flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    if !follow {
        flags |= OFlags::NOFOLLOW;
    }
    Ok(rustix::fs::open(OsStr::from_bytes(path), flags, Mode::empty())?)
}

fn walk<'s>(path: Vec<u8>, dir: u32, crawl: &'s Crawl, scope: &rayon::Scope<'s>) {
    let Ok(fd) = open_dir(&path, dir == 0) else {
        crawl.unreadable.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let mut buf: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); 64 * 1024];
    let mut raw = RawDir::new(&fd, &mut buf);
    let mut b = DirBatch { dir, flags: Vec::new(), ends: Vec::new(), names: Vec::new(), sub: Vec::new() };
    let mut children = Vec::new();
    while let Some(entry) = raw.next() {
        let Ok(e) = entry else { break };
        let name = e.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        let mut t = e.file_type();
        if t == FileType::Unknown {
            // Some filesystems don't fill d_type; one dirfd-relative stat resolves it.
            t = rustix::fs::statat(&fd, e.file_name(), AtFlags::SYMLINK_NOFOLLOW)
                .map(|st| FileType::from_raw_mode(st.st_mode))
                .unwrap_or(FileType::Unknown);
        }
        let kind = Kind::from_file_type(t);
        let hidden = if name[0] == b'.' { HIDDEN } else { 0 };
        b.flags.push(kind as u8 | hidden);
        b.names.extend_from_slice(name);
        b.ends.push(b.names.len() as u32);
        if kind == Kind::Dir {
            let child = crawl.next_dir.fetch_add(1, Ordering::Relaxed);
            b.sub.push(child);
            let mut p = path.clone();
            p.push(b'/');
            p.extend_from_slice(name);
            children.push((p, child));
        } else {
            b.sub.push(NONE);
        }
    }
    drop(fd);
    if !b.flags.is_empty() {
        crawl.batches.lock().unwrap().push(b);
    }
    for (p, child) in children {
        scope.spawn(move |s| walk(p, child, crawl, s));
    }
}

impl Index {
    /// Crawl `root` in parallel (one rayon task per folder). Symlinks are indexed but not
    /// followed; `root` itself may be a symlink to a folder.
    pub fn build(root: &Path) -> io::Result<Index> {
        // Surface a missing or unreadable root as an error instead of an empty index.
        drop(open_dir(root.as_os_str().as_bytes(), true)?);
        let crawl = Crawl { next_dir: AtomicU32::new(1), unreadable: AtomicU32::new(0), batches: Mutex::new(Vec::new()) };
        let path = root.as_os_str().as_bytes().to_vec();
        rayon::scope(|s| walk(path, 0, &crawl, s));
        let ndirs = crawl.next_dir.load(Ordering::Relaxed) as usize;
        let batches = crawl.batches.into_inner().unwrap();
        let mut idx = assemble(root.to_path_buf(), &batches, ndirs);
        idx.stats.unreadable = crawl.unreadable.load(Ordering::Relaxed);
        Ok(idx)
    }
}

/// Lay the crawl batches out in depth-first preorder, folding names on the way.
fn assemble(root: PathBuf, batches: &[DirBatch], ndirs: usize) -> Index {
    let mut by_dir = vec![NONE; ndirs];
    for (i, b) in batches.iter().enumerate() {
        by_dir[b.dir as usize] = i as u32;
    }
    let n = 1 + batches.iter().map(|b| b.flags.len()).sum::<usize>();
    let bytes = batches.iter().map(|b| b.names.len()).sum::<usize>() + n;
    let mut idx = Index {
        root,
        parent: Vec::with_capacity(n),
        end: Vec::with_capacity(n),
        flags: Vec::with_capacity(n),
        off: Vec::with_capacity(n + 1),
        names: Vec::with_capacity(bytes),
        foff: Vec::with_capacity(n + 1),
        folded: Vec::with_capacity(bytes),
        stats: Stats::default(),
    };
    idx.push(NONE, Kind::Dir as u8, b"");

    // (batch, next child, entry id)
    let mut stack: Vec<(u32, usize, u32)> = vec![(by_dir[0], 0, ROOT)];
    while let Some(top) = stack.last_mut() {
        let (bi, k, id) = *top;
        let Some(b) = batches.get(bi as usize).filter(|b| k < b.flags.len()) else {
            idx.end[id as usize] = idx.parent.len() as u32;
            stack.pop();
            continue;
        };
        top.1 += 1;
        let start = if k == 0 { 0 } else { b.ends[k - 1] as usize };
        let child = idx.push(id, b.flags[k], &b.names[start..b.ends[k] as usize]);
        if b.sub[k] != NONE {
            stack.push((by_dir[b.sub[k] as usize], 0, child));
        }
    }
    idx.off.push(idx.names.len() as u32);
    idx.foff.push(idx.folded.len() as u32);
    idx
}

// ---------------------------------------------------------------------------------------
// Access
// ---------------------------------------------------------------------------------------

impl Index {
    fn push(&mut self, parent: u32, flags: u8, name: &[u8]) -> u32 {
        let id = self.parent.len() as u32;
        self.parent.push(parent);
        self.end.push(id + 1);
        self.flags.push(flags);
        self.off.push(self.names.len() as u32);
        self.names.extend_from_slice(name);
        self.names.push(0);
        self.foff.push(self.folded.len() as u32);
        fold::fold_into(name, &mut self.folded);
        self.folded.push(0);
        id
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Number of entries, including the root.
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() <= 1
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Approximate heap bytes held by the index.
    pub fn heap_bytes(&self) -> usize {
        self.parent.capacity() * 4
            + self.end.capacity() * 4
            + self.flags.capacity()
            + self.off.capacity() * 4
            + self.names.capacity()
            + self.foff.capacity() * 4
            + self.folded.capacity()
    }

    /// Entry `id`'s own name as stored on disk (empty for the root).
    pub fn name(&self, id: u32) -> &[u8] {
        let i = id as usize;
        &self.names[self.off[i] as usize..self.off[i + 1] as usize - 1]
    }

    fn folded_name(&self, i: usize) -> &[u8] {
        &self.folded[self.foff[i] as usize..self.foff[i + 1] as usize - 1]
    }

    pub fn kind(&self, id: u32) -> Kind {
        Kind::from_bits(self.flags[id as usize])
    }

    pub fn parent(&self, id: u32) -> Option<u32> {
        Some(self.parent[id as usize]).filter(|&p| p != NONE)
    }

    /// Number of entries below `id` (all depths).
    pub fn descendants(&self, id: u32) -> usize {
        (self.end[id as usize] - id - 1) as usize
    }

    /// Direct children of `id`, in on-disk order.
    pub fn children(&self, id: u32) -> impl Iterator<Item = u32> + '_ {
        let end = self.end[id as usize];
        let mut c = id + 1;
        std::iter::from_fn(move || {
            (c < end).then(|| {
                let this = c;
                c = self.end[c as usize];
                this
            })
        })
    }

    /// Full path of entry `id`.
    pub fn path(&self, id: u32) -> PathBuf {
        let mut chain = Vec::new();
        let mut i = id;
        while i != ROOT && i != NONE {
            chain.push(i);
            i = self.parent[i as usize];
        }
        let mut p = self.root.clone();
        for &k in chain.iter().rev() {
            p.push(OsStr::from_bytes(self.name(k)));
        }
        p
    }

    /// The entry at `path`, if it is indexed.
    pub fn lookup(&self, path: &Path) -> Option<u32> {
        match self.locate(path)? {
            (id, true) => Some(id),
            _ => None,
        }
    }

    /// The deepest indexed entry on the way to `path`, and whether it is `path` itself.
    /// `None` if `path` is outside the root.
    fn locate(&self, path: &Path) -> Option<(u32, bool)> {
        let rel = path.strip_prefix(&self.root).ok()?;
        let mut id = ROOT;
        for comp in rel.components() {
            let Component::Normal(name) = comp else {
                return Some((id, false));
            };
            match self.children(id).find(|&c| self.name(c) == name.as_bytes()) {
                Some(c) => id = c,
                None => return Some((id, false)),
            }
        }
        Some((id, true))
    }
}

// ---------------------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------------------

/// Whitespace-separated terms, `"quoted phrases"` kept whole, each folded.
fn terms(text: &str) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for (i, part) in text.split('"').enumerate() {
        if i % 2 == 1 {
            if !part.is_empty() {
                out.push(fold::fold(part));
            }
        } else {
            out.extend(part.split_whitespace().map(fold::fold));
        }
    }
    out.retain(|t| !t.is_empty());
    out
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b >= 0x80
}

impl Index {
    /// Ids of matching entries, best first. An empty query (no terms, no extension) matches
    /// nothing.
    pub fn search(&self, q: &Query) -> Vec<u32> {
        let terms = terms(q.text);
        let ext = q.ext.map(|e| {
            let mut n = vec![b'.'];
            n.extend(fold::fold(e.trim_start_matches('.')));
            n
        });
        // The longest needle is the most selective one to scan the arena with.
        let Some(scan) = terms.iter().chain(ext.as_ref()).max_by_key(|t| t.len()) else {
            return Vec::new();
        };
        if terms.iter().chain(ext.as_ref()).any(|t| t.contains(&0)) {
            return Vec::new();
        }
        let within = q.within as usize;
        if within >= self.len() {
            return Vec::new();
        }

        let (first, last) = (within + 1, self.end[within] as usize);
        if first >= last {
            return Vec::new();
        }
        let lo = self.foff[first] as usize;
        let hay = &self.folded[lo..self.foff[last] as usize];
        let finder = memmem::Finder::new(scan);
        let mut hits = Vec::new();
        let mut cur = first;
        let mut prev = usize::MAX;
        for pos in finder.find_iter(hay) {
            let abs = (lo + pos) as u32;
            if abs >= self.foff[cur + 1] {
                cur += self.foff[cur + 1..=last].partition_point(|&o| o <= abs);
            }
            if cur == prev {
                continue;
            }
            prev = cur;
            if self.matches(cur, &terms, ext.as_deref(), q) {
                hits.push(cur as u32);
            }
        }
        self.rank(&mut hits, &terms, q.limit);
        hits
    }

    fn matches(&self, i: usize, terms: &[Vec<u8>], ext: Option<&[u8]>, q: &Query) -> bool {
        let flags = self.flags[i];
        let ok_kind = match q.kind {
            KindFilter::Any => true,
            KindFilter::Files => flags & KIND_MASK != Kind::Dir as u8,
            KindFilter::Dirs => flags & KIND_MASK == Kind::Dir as u8,
        };
        if !ok_kind {
            return false;
        }
        let name = self.folded_name(i);
        if let Some(ext) = ext {
            // A name that is only the extension (".pdf") is a dotfile, not a PDF.
            if name.len() <= ext.len() || !name.ends_with(ext) {
                return false;
            }
        }
        if !terms.iter().all(|t| memmem::find(name, t).is_some()) {
            return false;
        }
        if !q.hidden {
            let mut a = i as u32;
            while a != q.within && a != NONE {
                if self.flags[a as usize] & HIDDEN != 0 {
                    return false;
                }
                a = self.parent[a as usize];
            }
        }
        true
    }

    /// Sort hits by (match quality, name length, tree order), keeping the best `limit`.
    fn rank(&self, hits: &mut Vec<u32>, terms: &[Vec<u8>], limit: Option<usize>) {
        let phrase = terms.join(&b' ');
        let lead = terms.first().map(Vec::as_slice).unwrap_or(b"");
        let key = |&id: &u32| -> u64 {
            let name = self.folded_name(id as usize);
            let stem = memchr::memrchr(b'.', name).map_or(name, |p| &name[..p]);
            let class: u64 = if lead.is_empty() {
                3
            } else if name == phrase.as_slice() || stem == phrase.as_slice() {
                0
            } else if name.starts_with(lead) {
                1
            } else if memmem::find_iter(name, lead).any(|p| !is_word_byte(name[p - 1])) {
                2
            } else {
                3
            };
            class << 56 | (name.len().min(0xFF_FFFF) as u64) << 32 | id as u64
        };
        let mut keyed: Vec<u64> = hits.iter().map(key).collect();
        if let Some(k) = limit.filter(|&k| k < keyed.len()) {
            if k == 0 {
                hits.clear();
                return;
            }
            keyed.select_nth_unstable(k - 1);
            keyed.truncate(k);
        }
        keyed.sort_unstable();
        hits.clear();
        hits.extend(keyed.iter().map(|&k| k as u32));
    }
}

// ---------------------------------------------------------------------------------------
// Updates
// ---------------------------------------------------------------------------------------

impl Index {
    /// Bring `path`'s folder up to date: re-crawl the deepest indexed folder on the way to
    /// `path` and splice it in place. If that folder is gone, its parent is re-crawled
    /// instead, up to the root. Call it with the folder an inotify event names (for a
    /// create, delete or rename, that's the parent folder of the changed entry).
    pub fn rescan(&mut self, path: &Path) -> io::Result<Stats> {
        let (mut id, _) = self
            .locate(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path is outside the indexed root"))?;
        while self.kind(id) != Kind::Dir {
            id = self.parent[id as usize];
        }
        loop {
            match Index::build(&self.path(id)) {
                Ok(sub) => {
                    let stats = sub.stats;
                    self.splice(id, &sub);
                    return Ok(stats);
                }
                Err(e) if id == ROOT => return Err(e),
                Err(_) => id = self.parent[id as usize],
            }
        }
    }

    /// Replace everything below folder `id` with `sub`'s entries (`sub` is a fresh crawl
    /// rooted at `id`'s path).
    fn splice(&mut self, id: u32, sub: &Index) {
        let (a, b) = (id as usize + 1, self.end[id as usize] as usize);
        let added = sub.len() - 1;
        let delta = added as i64 - (b - a) as i64;
        let shift = |v: u32, from: u32| if v >= from { (v as i64 + delta) as u32 } else { v };
        let old_end = b as u32;

        // Ids: entries after the range move by `delta`; the new entries sit at `id + k`.
        for p in self.parent.iter_mut() {
            *p = if *p == NONE { NONE } else { shift(*p, old_end) };
        }
        for e in self.end.iter_mut() {
            *e = shift(*e, old_end);
        }
        self.end[id as usize] = id + 1 + added as u32;
        let base = id;
        self.parent.splice(a..b, sub.parent[1..].iter().map(|&p| p + base));
        self.end.splice(a..b, sub.end[1..].iter().map(|&e| e + base));
        self.flags.splice(a..b, sub.flags[1..].iter().copied());

        splice_arena(&mut self.off, &mut self.names, a, b, &sub.off, &sub.names);
        splice_arena(&mut self.foff, &mut self.folded, a, b, &sub.foff, &sub.folded);
    }
}

/// Replace entries `a..b` of an offset + arena pair with entries `1..` of another.
fn splice_arena(off: &mut Vec<u32>, arena: &mut Vec<u8>, a: usize, b: usize, soff: &[u32], sarena: &[u8]) {
    let (lo, hi) = (off[a] as usize, off[b] as usize);
    let new = &sarena[soff[1] as usize..];
    let delta = new.len() as i64 - (hi - lo) as i64;
    for o in off[b..].iter_mut() {
        *o = (*o as i64 + delta) as u32;
    }
    let rebase = soff[1] as i64 - lo as i64;
    let n = soff.len() - 1;
    off.splice(a..b, soff[1..n].iter().map(|&o| (o as i64 - rebase) as u32));
    arena.splice(lo..hi, new.iter().copied());
}

#[doc(hidden)]
impl Index {
    /// Structural invariants, for tests.
    pub fn check(&self) {
        let n = self.len();
        assert_eq!(self.end.len(), n);
        assert_eq!(self.flags.len(), n);
        assert_eq!(self.off.len(), n + 1);
        assert_eq!(self.foff.len(), n + 1);
        assert_eq!(self.end[0] as usize, n);
        assert_eq!(*self.off.last().unwrap() as usize, self.names.len());
        assert_eq!(*self.foff.last().unwrap() as usize, self.folded.len());
        for i in 1..n {
            let p = self.parent[i] as usize;
            assert!(p < i, "parent {p} of {i} must precede it");
            assert!(self.end[p] as usize >= self.end[i] as usize && self.end[i] as usize > i);
            assert_eq!(self.names[self.off[i + 1] as usize - 1], 0);
            assert_eq!(self.folded[self.foff[i + 1] as usize - 1], 0);
            let mut f = Vec::new();
            fold::fold_into(self.name(i as u32), &mut f);
            assert_eq!(f, self.folded_name(i));
        }
    }
}

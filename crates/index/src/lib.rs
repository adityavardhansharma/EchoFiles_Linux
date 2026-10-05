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

pub use ef_config as config;
pub mod corpus;
pub mod fold;
pub mod live;
mod matcher;
mod store;
mod view;

pub use store::MappedIndex;

use std::ffi::OsStr;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use rustix::fs::{AtFlags, FileType, Mode, OFlags, RawDir};

/// Id of the indexed root folder. Its subtree is the whole index.
pub const ROOT: u32 = 0;
pub(crate) const NONE: u32 = u32::MAX;

const KIND_MASK: u8 = 0b11;
pub(crate) const HIDDEN: u8 = 0b100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    File,
    Dir,
    Symlink,
    Other,
}

impl Kind {
    pub(crate) fn from_bits(b: u8) -> Kind {
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
    /// Folders indexed by name only, their contents skipped (see [`Options`]).
    pub excluded: u32,
}

/// What a crawl skips. Excluded folders are still indexed themselves (so searching for
/// `node_modules` finds them); only their contents are left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Folder names whose contents are skipped, matched exactly.
    pub exclude_names: Vec<Vec<u8>>,
    /// Skip the contents of folders holding a `CACHEDIR.TAG` file — the standard marker
    /// cargo `target/`, pytest, mypy and other tools put in their cache folders.
    pub skip_cache_tagged: bool,
    /// Absolute paths left out entirely — the entry itself and everything below it (a
    /// folder the user doesn't want in search at all).
    pub exclude_paths: Vec<Vec<u8>>,
}

impl Options {
    /// Index everything.
    pub fn everything() -> Options {
        Options { exclude_names: Vec::new(), skip_cache_tagged: false, exclude_paths: Vec::new() }
    }

    /// Skip folders people don't search by name: package and toolchain stores, VCS
    /// internals, caches. On a typical laptop these hold most of the files.
    pub fn recommended() -> Options {
        Options {
            exclude_names: config::DEFAULT_EXCLUDE_NAMES.iter().map(|n| n.as_bytes().to_vec()).collect(),
            skip_cache_tagged: true,
            exclude_paths: Vec::new(),
        }
    }

    fn excludes(&self, name: &[u8]) -> bool {
        self.exclude_names.iter().any(|n| n == name)
    }

    /// Names of `dir`'s children that are excluded by path (usually none: one prefix check
    /// per excluded path).
    fn excluded_children<'p>(&'p self, dir: &[u8]) -> Vec<&'p [u8]> {
        self.exclude_paths
            .iter()
            .filter_map(|p| {
                let rest = p.strip_prefix(dir)?.strip_prefix(b"/")?;
                (!rest.is_empty() && !rest.contains(&b'/')).then_some(rest)
            })
            .collect()
    }

    /// Whether `path` is an excluded path or inside one.
    pub fn excludes_path(&self, path: &Path) -> bool {
        self.exclude_paths.iter().any(|p| path.starts_with(Path::new(OsStr::from_bytes(p))))
    }
}

impl Options {
    /// Crawl options for one indexed folder from the user's search settings.
    pub fn from_search(cfg: &config::SearchConfig, root: &Path) -> Options {
        Options {
            exclude_names: cfg.exclude_names.iter().map(|n| n.trim().as_bytes().to_vec()).filter(|n| !n.is_empty()).collect(),
            skip_cache_tagged: cfg.skip_cache_folders,
            exclude_paths: cfg.exclude_paths_in(root).iter().map(|p| p.as_os_str().as_bytes().to_vec()).collect(),
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Options::everything()
    }
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
    opts: Options,
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

struct Crawl<'o> {
    next_dir: AtomicU32,
    unreadable: AtomicU32,
    excluded: AtomicU32,
    batches: Mutex<Vec<DirBatch>>,
    opts: &'o Options,
    /// Apply the `CACHEDIR.TAG` rule to the crawl root too (rescans of a subfolder).
    tag_root: bool,
}

fn open_dir(path: &[u8], follow: bool) -> io::Result<rustix::fd::OwnedFd> {
    let mut flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    if !follow {
        flags |= OFlags::NOFOLLOW;
    }
    Ok(rustix::fs::open(OsStr::from_bytes(path), flags, Mode::empty())?)
}

fn walk<'s>(path: Vec<u8>, dir: u32, crawl: &'s Crawl<'s>, scope: &rayon::Scope<'s>) {
    if crawl.opts.excludes_path(Path::new(OsStr::from_bytes(&path))) { return; }
    let Ok(fd) = open_dir(&path, dir == 0) else {
        crawl.unreadable.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let mut buf: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); 64 * 1024];
    let mut raw = RawDir::new(&fd, &mut buf);
    let mut b = DirBatch { dir, flags: Vec::new(), ends: Vec::new(), names: Vec::new(), sub: Vec::new() };
    let mut children = Vec::new();
    let mut tagged = false;
    let skip = crawl.opts.excluded_children(&path);
    while let Some(entry) = raw.next() {
        let Ok(e) = entry else { break };
        let name = e.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        tagged |= name == b"CACHEDIR.TAG";
        if !skip.is_empty() && skip.contains(&name) {
            crawl.excluded.fetch_add(1, Ordering::Relaxed);
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
        if kind == Kind::Dir && crawl.opts.excludes(name) {
            crawl.excluded.fetch_add(1, Ordering::Relaxed);
            b.sub.push(NONE);
        } else if kind == Kind::Dir {
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
    if tagged && crawl.opts.skip_cache_tagged && (dir != 0 || crawl.tag_root) {
        // A cache folder: keep the folder itself, drop what's inside.
        crawl.excluded.fetch_add(1, Ordering::Relaxed);
        return;
    }
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
        Index::build_with(root, &Options::everything())
    }

    /// Crawl `root`, skipping what `opts` excludes. The options are kept for [`rescan`]
    /// and saved with the index.
    ///
    /// [`rescan`]: Index::rescan
    pub fn build_with(root: &Path, opts: &Options) -> io::Result<Index> {
        Index::crawl(root, opts, false)
    }

    fn crawl(root: &Path, opts: &Options, tag_root: bool) -> io::Result<Index> {
        // Surface a missing or unreadable root as an error instead of an empty index.
        drop(open_dir(root.as_os_str().as_bytes(), true)?);
        let crawl = Crawl {
            next_dir: AtomicU32::new(1),
            unreadable: AtomicU32::new(0),
            excluded: AtomicU32::new(0),
            batches: Mutex::new(Vec::new()),
            opts,
            tag_root,
        };
        let path = root.as_os_str().as_bytes().to_vec();
        rayon::scope(|s| walk(path, 0, &crawl, s));
        let ndirs = crawl.next_dir.load(Ordering::Relaxed) as usize;
        let batches = crawl.batches.into_inner().unwrap();
        let mut idx = assemble(root.to_path_buf(), &batches, ndirs);
        idx.stats.unreadable = crawl.unreadable.load(Ordering::Relaxed);
        idx.stats.excluded = crawl.excluded.load(Ordering::Relaxed);
        idx.opts = opts.clone();
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
        opts: Options::everything(),
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

    pub(crate) fn view(&self) -> view::View<'_> {
        view::View {
            root: &self.root,
            parent: &self.parent,
            end: &self.end,
            flags: &self.flags,
            foff: &self.foff,
            folded: &self.folded,
            names: view::Names::Owned { off: &self.off, names: &self.names },
        }
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

    pub fn options(&self) -> &Options {
        &self.opts
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
        self.view().name(id)
    }

    pub fn kind(&self, id: u32) -> Kind {
        self.view().kind(id)
    }

    pub fn parent(&self, id: u32) -> Option<u32> {
        self.view().parent(id)
    }

    /// Number of entries below `id` (all depths).
    pub fn descendants(&self, id: u32) -> usize {
        self.view().descendants(id)
    }

    /// Direct children of `id`, in on-disk order.
    pub fn children(&self, id: u32) -> impl Iterator<Item = u32> + '_ {
        self.view().children(id)
    }

    /// Full path of entry `id`.
    pub fn path(&self, id: u32) -> PathBuf {
        self.view().path(id)
    }

    /// The entry at `path`, if it is indexed.
    pub fn lookup(&self, path: &Path) -> Option<u32> {
        self.view().lookup(path)
    }

    /// Ids of matching entries, best first. An empty query (no terms, no extension) matches
    /// nothing. Large scopes are split into chunks searched in parallel, each keeping only
    /// its best `limit` hits.
    pub fn search(&self, q: &Query) -> Vec<u32> {
        self.view().search(q)
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
            .view()
            .locate(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path is outside the indexed root"))?;
        while self.kind(id) != Kind::Dir {
            id = self.parent[id as usize];
        }
        // Changes inside an excluded folder don't concern the index.
        if self.opts.excludes_path(path) || (id != ROOT && self.opts.excludes(self.name(id))) {
            return Ok(Stats::default());
        }
        loop {
            match Index::crawl(&self.path(id), &self.opts, true) {
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
            assert_eq!(f, self.view().folded_name(i));
        }
    }
}

/// Cached phone file names remain searchable when the phone is offline.
pub mod phone;

/// The common cross-root ranking key, also used when merging live and mapped results.
pub fn rank_name(text: &str, name: &[u8]) -> u64 {
    matcher::Matcher::new(&Query::new(text)).map_or(u64::MAX, |m| { let mut folded = Vec::new(); fold::fold_into(name, &mut folded); m.rank(&folded) })
}

pub fn rank_paths(paths: &mut Vec<PathBuf>, q: &Query) {
    paths.sort_by_cached_key(|p| (rank_name(q.text, p.file_name().unwrap_or_default().as_bytes()), p.clone()));
    paths.dedup();
    if let Some(limit) = q.limit { paths.truncate(limit); }
}

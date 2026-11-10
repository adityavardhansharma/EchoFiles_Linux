//! The search engine behind `ef find` and `ef fd`: every fd filter, answered from the index
//! where it can be and from the disk where it must be.
//!
//! For each search root:
//! - **Index**: the saved tree is walked in preorder without touching the disk. Ignore
//!   rules, hidden entries, depth and prune are applied per folder, so a rejected folder's
//!   whole subtree is skipped by jumping to its end id. With a literal in the pattern only
//!   the names containing it are visited (one SIMD scan of the folded name arena), and
//!   only their ancestors are judged. Folders the index skipped (`node_modules`, caches,
//!   excluded paths) are walked live when the search would show their contents — so an
//!   indexed answer has the same entries as a live one, as of the last index update.
//! - **Live**: a parallel walk ([`crate::walk`]) for folders outside the index, `-L`, or
//!   `--live`.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;

use crate::filter::{FType, Filters, Hit, Probe};
use crate::gitignore::{DirRules, Globals, IgnoreOptions, Stack, Verdict};
use crate::view::View;
use crate::walk::Walk;
use crate::{Kind, MappedIndex, NONE, Options};

/// Receives results while the search runs (from many threads), instead of collecting them:
/// output starts before the walk ends and nothing is kept.
pub trait Sink: Send + Sync {
    fn take(&self, hits: Vec<Hit>);
}

/// One search: filters plus how the walk behaves.
#[derive(Default)]
pub struct Search {
    /// Stream results here instead of returning them in [`Outcome::hits`].
    pub sink: Option<Arc<dyn Sink>>,
    pub filters: Filters,
    pub ignore: IgnoreOptions,
    /// `--exclude` globs (gitignore syntax, relative to each search root).
    pub excludes: Vec<String>,
    pub ignore_files: Vec<PathBuf>,
    /// Stop after this many hits (only when results aren't ranked).
    pub max_results: Option<usize>,
    pub show_errors: bool,
    /// Set to stop the search early (from another thread).
    pub cancel: Arc<AtomicBool>,
    pub threads: Option<usize>,
    /// Compute rank keys (only ranked output needs them).
    pub rank: bool,
}

/// Where one root's results came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Index,
    Live,
}

/// One place to search.
pub struct RootSpec<'a> {
    /// The path walked and reported (absolute; may run through symlinks).
    pub walk: PathBuf,
    /// The same folder with symlinks resolved, for finding it in the index.
    pub lookup: PathBuf,
    /// The index to answer from, if any.
    pub index: Option<&'a MappedIndex>,
    /// Leave out what the index leaves out (Settings → Search exclusions), also where the
    /// folder is read live: what the app's search shows. `None`: every entry, like fd.
    pub exclusions: Option<Options>,
    /// Ranked search shown only in part: keep just the best this many hits from the index
    /// (counting the rest in [`Outcome::dropped`]) instead of building every path.
    pub keep_best: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct RootReport {
    pub path: PathBuf,
    pub source: Source,
    /// Folders the index had skipped that were walked live for this search.
    pub holes: usize,
}

#[derive(Default)]
pub struct Outcome {
    pub hits: Vec<Hit>,
    pub roots: Vec<RootReport>,
    pub errors: Vec<String>,
    /// Indexed hits that no longer exist on disk (dropped).
    pub vanished: usize,
    /// Matches counted but not kept (see [`RootSpec::keep_best`]).
    pub dropped: usize,
}

struct Hole {
    /// Folder whose contents to walk, or (with `entry`) the parent of a missing entry.
    path: Vec<u8>,
    entry: Option<Vec<u8>>,
    depth: u32,
    stack: Arc<Stack>,
}

/// Search `roots` (absolute paths; the index for each, if one should be used).
pub fn run(s: &Search, roots: &[RootSpec]) -> Outcome {
    let found = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let errors = Mutex::new(Vec::new());
    let mut out = Outcome::default();
    let mut work = || {
        for (i, spec) in roots.iter().enumerate() {
            let root = &spec.walk;
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let root_abs = root.as_os_str().as_bytes().to_vec();
            // fd skips a search root that itself holds an `--ignore-contain` marker.
            if s.filters.ignore_contain.iter().any(|m| std::fs::symlink_metadata(root.join(OsStr::from_bytes(m))).is_ok()) {
                out.roots.push(RootReport { path: root.clone(), source: Source::Live, holes: 0 });
                continue;
            }
            let globals = Arc::new(Globals::new(s.ignore.clone(), root, &s.excludes, &s.ignore_files));
            let base = Stack::root(root, globals.clone(), None);
            let root_dev = rustix::fs::stat(root.as_path()).map(|st| st.st_dev).unwrap_or(0);
            let mut walk = Walk::new(s, i as u16, root_dev, &found, &stop, &errors);
            walk.skip = spec.exclusions.as_ref();
            let scope_id = spec.index.and_then(|m| m.lookup(&spec.lookup).filter(|&id| m.kind(id) == Kind::Dir));
            let (source, holes) = match (spec.index, scope_id) {
                (Some(m), Some(id)) => {
                    let mut hits = Vec::new();
                    let mut holes = Vec::new();
                    let vanished = AtomicUsize::new(0);
                    let dropped = AtomicUsize::new(0);
                    let tally = Tally { vanished: &vanished, dropped: &dropped, keep: spec.keep_best, holes: spec.exclusions.is_none() };
                    index_walk(s, m.view(), m.options(), id, &root_abs, base, i as u16, &found, &stop, &tally, &mut hits, &mut holes);
                    out.vanished += vanished.into_inner();
                    out.dropped += dropped.into_inner();
                    if spec.exclusions.is_some() {
                        // What the index skipped stays skipped.
                        holes.clear();
                    }
                    let n = holes.len();
                    walk.hits.lock().unwrap().push(std::mem::take(&mut hits));
                    rayon::scope(|sc| {
                        for h in holes {
                            match h.entry {
                                Some(name) => {
                                    let w = &walk;
                                    sc.spawn(move |sc| w.entry(&h.path, &name, h.depth, h.stack, sc));
                                }
                                None => {
                                    let w = &walk;
                                    sc.spawn(move |sc| w.dir(h.path, h.depth, h.stack, None, false, sc));
                                }
                            }
                        }
                    });
                    (Source::Index, n)
                }
                _ => {
                    rayon::scope(|sc| walk.dir(root_abs.clone(), 0, base, None, true, sc));
                    (Source::Live, 0)
                }
            };
            let mut hits = walk.into_hits();
            out.hits.append(&mut hits);
            out.roots.push(RootReport { path: root.clone(), source, holes });
        }
    };
    match s.threads {
        Some(n) => match rayon::ThreadPoolBuilder::new().num_threads(n.max(1)).build() {
            Ok(pool) => pool.install(work),
            Err(_) => work(),
        },
        None => work(),
    }
    out.errors = errors.into_inner().unwrap();
    if let Some(max) = s.max_results {
        out.hits.truncate(max);
    }
    out
}

/// Drop indexed hits that are gone from disk; fill in metadata the filters didn't fetch.
///
/// Hits come in index (preorder) order, so siblings are adjacent. A folder with many hits
/// is listed once (one `getdents64` instead of a path lookup and `stat` per hit); the
/// rest are `lstat`ed. Returns how many were dropped.
fn verify(s: &Search, hits: &mut Vec<Hit>) -> usize {
    let before = hits.len();
    let want = s.filters.want_meta;
    let mut i = 0;
    while i < hits.len() {
        let dir_of = |h: &Hit| memchr::memrchr(b'/', &h.path).unwrap_or(0);
        let d = dir_of(&hits[i]);
        let mut j = i + 1;
        while j < hits.len() && dir_of(&hits[j]) == d && hits[j].path[..d] == hits[i].path[..d] {
            j += 1;
        }
        let group = &mut hits[i..j];
        if !want && group.len() >= 8 {
            let names = list_names(&group[0].path[..d.max(1)]);
            for h in group.iter_mut() {
                if h.meta.is_none() && names.binary_search_by(|n| n.as_slice().cmp(h.name())).is_err() {
                    h.path.clear();
                }
            }
        } else {
            for h in group.iter_mut() {
                if h.meta.is_none() {
                    let mut p = Probe::new(None, &[], &h.path);
                    match p.lstat() {
                        Some(m) => {
                            if want {
                                h.meta = Some(Box::new(m));
                            }
                        }
                        None => h.path.clear(),
                    }
                }
            }
        }
        i = j;
    }
    hits.retain(|h| !h.path.is_empty());
    before - hits.len()
}

/// A folder's entry names, sorted (empty if it can't be read).
fn list_names(dir: &[u8]) -> Vec<Vec<u8>> {
    let Ok(fd) = rustix::fs::open(
        OsStr::from_bytes(dir),
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    ) else {
        return Vec::new();
    };
    let mut buf: Vec<std::mem::MaybeUninit<u8>> = vec![std::mem::MaybeUninit::uninit(); 32 * 1024];
    let mut raw = rustix::fs::RawDir::new(&fd, &mut buf);
    let mut names = Vec::new();
    while let Some(Ok(e)) = raw.next() {
        names.push(e.file_name().to_bytes().to_vec());
    }
    names.sort_unstable();
    names
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

/// A folder on the way down the index, accepted for the search.
#[derive(Clone)]
struct DirState {
    id: u32,
    end: u32,
    abs: Vec<u8>,
    depth: u32,
    stack: Arc<Stack>,
    /// Whether its children are visited (not past max depth, not pruned).
    open: bool,
}

struct IndexWalk<'a> {
    s: &'a Search,
    v: View<'a>,
    opts: &'a Options,
    /// Look for folders the index skipped (to read them live); off when they stay out.
    holes: bool,
}

impl IndexWalk<'_> {
    fn has_child(&self, id: u32, name: &[u8]) -> bool {
        self.v.children(id).any(|c| self.v.name(c) == name)
    }

    /// The rule files of indexed folder `id`, found from its indexed children.
    fn rules(&self, id: u32, abs: &[u8], stack: &Arc<Stack>) -> Arc<Stack> {
        let g = stack.globals();
        if !g.opts.any() {
            return stack.clone();
        }
        let mut names: Vec<&[u8]> = Vec::new();
        for c in self.v.children(id) {
            let n = self.v.name(c);
            if n.first() == Some(&b'.') && (n == b".git" || n.ends_with(b"ignore")) {
                names.push(n);
            }
        }
        if names.is_empty() {
            return stack.clone();
        }
        let has = |n: &[u8]| names.contains(&n);
        match DirRules::load(abs, &has, &g.opts) {
            Some(r) => stack.push(r),
            None => stack.clone(),
        }
    }

    /// Hidden, ignore rules, `--ignore-contain`, max depth. `None` if `id` is out (and, for
    /// a folder, everything below it).
    /// The entry's path comes back only if it had to be built.
    fn admit(&self, p: &DirState, id: u32) -> Option<(Option<Vec<u8>>, bool)> {
        let f = &self.s.filters;
        let name = self.v.name(id);
        let is_dir = self.v.kind(id) == Kind::Dir;
        let hidden = name.first() == Some(&b'.');
        let g = p.stack.globals();
        let ignoring = g.opts.any() || g.overrides.is_some() || !g.explicit.is_empty();
        if hidden && !f.hidden && !ignoring {
            return None;
        }
        if f.max_depth.is_some_and(|m| p.depth + 1 > m) {
            return None;
        }
        if !ignoring && (!is_dir || f.ignore_contain.is_empty()) {
            return Some((None, is_dir));
        }
        let abs = join(&p.abs, name);
        if ignoring {
            let v = p.stack.check(&abs, is_dir);
            if v.is_ignore() || (hidden && !f.hidden && !matches!(v, Verdict::Whitelist(..))) {
                return None;
            }
        }
        if is_dir && !f.ignore_contain.is_empty() {
            let marked = if self.v.end[id as usize] == id + 1 {
                f.ignore_contain.iter().any(|m| std::fs::symlink_metadata(OsStr::from_bytes(&join(&abs, m))).is_ok())
            } else {
                f.ignore_contain.iter().any(|m| self.has_child(id, m))
            };
            if marked {
                return None;
            }
        }
        Some((Some(abs), is_dir))
    }

    /// Whether the index left out this folder's contents (excluded name or cache folder).
    fn is_hole(&self, id: u32, abs: &[u8]) -> bool {
        if self.v.end[id as usize] != id + 1 {
            return false;
        }
        let name = self.v.name(id);
        self.opts.exclude_names.iter().any(|n| n == name)
            || (self.opts.skip_cache_tagged && std::fs::symlink_metadata(OsStr::from_bytes(&join(abs, b"CACHEDIR.TAG"))).is_ok())
    }

    /// Excluded paths directly inside `abs`: missing from the index entirely.
    fn missing_children(&self, abs: &[u8]) -> Vec<Vec<u8>> {
        self.opts
            .exclude_paths
            .iter()
            .filter_map(|p| {
                let rest = p.strip_prefix(abs)?.strip_prefix(b"/")?;
                (!rest.is_empty() && !rest.contains(&b'/')).then(|| rest.to_vec())
            })
            .collect()
    }

    fn open_dir(&self, p: &DirState, id: u32, abs: Vec<u8>, matched: bool, holes: &mut Vec<Hole>) -> DirState {
        let f = &self.s.filters;
        let depth = p.depth + 1;
        let open = f.max_depth.is_none_or(|m| depth < m) && !(matched && f.prune);
        let stack = if open { self.rules(id, &abs, &p.stack) } else { p.stack.clone() };
        if open && self.holes {
            for name in self.missing_children(&abs) {
                holes.push(Hole { path: abs.clone(), entry: Some(name), depth, stack: stack.clone() });
            }
            if self.is_hole(id, &abs) {
                holes.push(Hole { path: abs.clone(), entry: None, depth, stack: stack.clone() });
            }
        }
        DirState { id, end: self.v.end[id as usize], abs, depth, stack, open }
    }
}

#[allow(clippy::too_many_arguments)]
fn index_walk(
    s: &Search,
    v: View,
    opts: &Options,
    scope: u32,
    scope_abs: &[u8],
    base: Arc<Stack>,
    root: u16,
    found: &AtomicUsize,
    stop: &AtomicBool,
    tally: &Tally,
    hits: &mut Vec<Hit>,
    holes: &mut Vec<Hole>,
) {
    let w = IndexWalk { s, v, opts, holes: tally.holes };
    let f = &s.filters;
    let root_stack = w.rules(scope, scope_abs, &base);
    let states = [DirState { id: scope, end: v.end[scope as usize], abs: scope_abs.to_vec(), depth: 0, stack: root_stack, open: true }];
    for name in w.missing_children(scope_abs).into_iter().filter(|_| w.holes) {
        holes.push(Hole { path: scope_abs.to_vec(), entry: Some(name), depth: 0, stack: states[0].stack.clone() });
    }
    if w.holes && w.is_hole(scope, scope_abs) {
        holes.push(Hole { path: scope_abs.to_vec(), entry: None, depth: 0, stack: states[0].stack.clone() });
    }
    let (first, last) = (scope + 1, v.end[scope as usize]);
    if first >= last {
        return;
    }
    // Candidates: names containing the longest literal every hit must have, else all.
    let literal: Option<Vec<u8>> = f
        .patterns
        .iter()
        .filter_map(|p| p.prefilter().map(<[u8]>::to_vec))
        .chain(
            f.exts
                .iter()
                .filter(|e| e.is_ascii())
                .map(|e| [b".".as_slice(), &e.to_ascii_lowercase()].concat())
                .take((f.exts.len() == 1) as usize),
        )
        .max_by_key(Vec::len);
    let finder = literal.as_ref().map(memchr::memmem::Finder::new);
    // Split the id range into chunks searched in parallel; each finds its own candidates
    // (names containing the literal, one SIMD scan of its slice of the folded arena) and
    // rebuilds the folder states it needs from the scope down, so chunks are independent.
    let threads = rayon::current_num_threads().max(1);
    let span = (last - first) as usize;
    let per = span.div_ceil(threads * 4).max(4096);
    let ranges: Vec<(u32, u32)> = (first..last).step_by(per).map(|a| (a, (a as usize + per).min(last as usize) as u32)).collect();
    let root_state = &states[0];
    let parts: Vec<(Vec<Hit>, Vec<Hole>)> = ranges
        .into_par_iter()
        .map(|(a, b)| {
            let mut hits = Vec::new();
            let mut holes = Vec::new();
            let mut st = vec![root_state.clone()];
            match &finder {
                Some(finder) => {
                    let lo = v.foff[a as usize] as usize;
                    let hay = &v.folded[lo..v.foff[b as usize] as usize];
                    let foff = v.foff;
                    let mut cur = a as usize;
                    let mut prev = usize::MAX;
                    let ids = finder.find_iter(hay).filter_map(move |pos| {
                        let at = (lo + pos) as u32;
                        if at >= foff[cur + 1] {
                            cur += foff[cur + 1..=b as usize].partition_point(|&o| o <= at);
                        }
                        (cur != prev && cur < b as usize).then(|| {
                            prev = cur;
                            cur as u32
                        })
                    });
                    walk_chunk(&w, ids, &mut st, root, found, stop, tally, &mut hits, &mut holes);
                }
                None => walk_chunk(&w, a..b, &mut st, root, found, stop, tally, &mut hits, &mut holes),
            }
            if tally.keep.is_none() {
                // The index may be stale: check this chunk's hits against the disk now, in
                // parallel with the other chunks.
                tally.vanished.fetch_add(verify(s, &mut hits), Ordering::Relaxed);
                if let Some(sink) = &s.sink {
                    sink.take(std::mem::take(&mut hits));
                }
            }
            (hits, holes)
        })
        .collect();
    for (mut h, mut o) in parts {
        hits.append(&mut h);
        holes.append(&mut o);
    }
    if let Some(k) = tally.keep {
        tally.dropped.fetch_add(keep_best(hits, k), Ordering::Relaxed);
        // A few hundred hits: one parallel `lstat` each, keeping what it returns.
        let before = hits.len();
        hits.par_iter_mut().for_each(|h| {
            if h.meta.is_none() {
                match Probe::new(None, &[], &h.path).lstat() {
                    Some(m) => h.meta = Some(Box::new(m)),
                    None => h.path.clear(),
                }
            }
        });
        hits.retain(|h| !h.path.is_empty());
        tally.vanished.fetch_add(before - hits.len(), Ordering::Relaxed);
        if let Some(sink) = &s.sink {
            sink.take(std::mem::take(hits));
        }
    }
}

/// Counters shared by the chunks of one indexed root.
struct Tally<'a> {
    vanished: &'a AtomicUsize,
    dropped: &'a AtomicUsize,
    keep: Option<usize>,
    holes: bool,
}

/// Keep the best `k` hits (by rank, then path); returns how many were removed.
fn keep_best(hits: &mut Vec<Hit>, k: usize) -> usize {
    if hits.len() <= k {
        return 0;
    }
    let n = hits.len();
    if k == 0 {
        hits.clear();
        return n;
    }
    hits.select_nth_unstable_by(k - 1, |a, b| (a.rank, &a.path).cmp(&(b.rank, &b.path)));
    hits.truncate(k);
    n - k
}

/// What [`hit_for`] decided about one entry.
enum Judged {
    No,
    /// A match, but ranked below the hits already kept.
    Counted,
    Hit(Hit),
}

/// Judge and collect one entry whose parent folder state is `p`.
///
/// `abs` is the entry's path if the caller already built it; otherwise it is built only
/// when a filter needs it or the hit is kept (most matches of a broad ranked search are
/// counted, never turned into a path). Hits ranked worse than `cutoff` are only counted.
fn hit_for(w: &IndexWalk, c: u32, abs: Option<&[u8]>, parent: &[u8], d: u32, root: u16, cutoff: u64) -> Judged {
    let f = &w.s.filters;
    if d < f.min_depth {
        return Judged::No;
    }
    let v = &w.v;
    let name = v.name(c);
    let folded = v.folded_name(c as usize);
    let listed = FType::from_kind(v.kind(c));
    let built;
    let abs: &[u8] = match abs {
        Some(a) => a,
        None if f.needs_path() || f.needs_stat() || listed == FType::Unknown => {
            built = join(parent, name);
            &built
        }
        None => &[],
    };
    if !f.name_ok(name, Some(folded), abs) {
        return Judged::No;
    }
    let mut probe = Probe::new(None, name, abs);
    let Some((ft, meta)) = f.rest_ok(listed, &mut probe) else { return Judged::No };
    let rank = if w.s.rank { f.rank(name, Some(folded)) } else { 0 };
    if rank > cutoff {
        return Judged::Counted;
    }
    let path = if abs.is_empty() { join(parent, name) } else { abs.to_vec() };
    Judged::Hit(Hit { path, root, depth: d, ftype: ft, meta: meta.map(Box::new), rank })
}

#[allow(clippy::too_many_arguments)]
fn walk_chunk(
    w: &IndexWalk,
    candidates: impl Iterator<Item = u32>,
    states: &mut Vec<DirState>,
    root: u16,
    found: &AtomicUsize,
    stop: &AtomicBool,
    tally: &Tally,
    hits: &mut Vec<Hit>,
    holes: &mut Vec<Hole>,
) {
    let (s, v, f) = (w.s, &w.v, &w.s.filters);
    let mut next = 0;
    // With `keep`, hits past the best `k` so far are only counted.
    let mut cutoff = u64::MAX;
    for c in candidates {
        if c < next {
            continue;
        }
        if stop.load(Ordering::Relaxed) || s.cancel.load(Ordering::Relaxed) {
            return;
        }
        while states.last().is_some_and(|t| t.end <= c) {
            states.pop();
        }
        // Judge the folders between the innermost accepted one and `c`.
        let top = states.last().unwrap().id;
        let mut chain = Vec::new();
        let mut a = v.parent[c as usize];
        while a != top && a != NONE {
            chain.push(a);
            a = v.parent[a as usize];
        }
        let mut rejected = false;
        for &a in chain.iter().rev() {
            let p = states.last().unwrap();
            if !p.open {
                next = p.end;
                rejected = true;
                break;
            }
            match w.admit(p, a) {
                Some((abs, _)) => {
                    let abs = abs.unwrap_or_else(|| join(&p.abs, v.name(a)));
                    // A pruned ancestor (matched in another chunk) must stay closed here too.
                    let matched = f.prune && !matches!(hit_for(w, a, Some(&abs), &p.abs, p.depth + 1, root, u64::MAX), Judged::No);
                    let st = w.open_dir(p, a, abs, matched, holes);
                    states.push(st);
                }
                None => {
                    next = v.end[a as usize];
                    rejected = true;
                    break;
                }
            }
        }
        if rejected {
            continue;
        }
        let p = states.last().unwrap();
        if !p.open {
            next = p.end;
            continue;
        }
        let Some((abs, is_dir)) = w.admit(p, c) else {
            next = v.end[c as usize];
            continue;
        };
        let d = p.depth + 1;
        let abs = if is_dir { Some(abs.unwrap_or_else(|| join(&p.abs, v.name(c)))) } else { abs };
        let matched = match hit_for(w, c, abs.as_deref(), &p.abs, d, root, cutoff) {
            Judged::No => false,
            Judged::Counted => {
                tally.dropped.fetch_add(1, Ordering::Relaxed);
                true
            }
            Judged::Hit(h) => {
                hits.push(h);
                if let Some(k) = tally.keep
                    && hits.len() >= 2 * k.max(1)
                {
                    tally.dropped.fetch_add(keep_best(hits, k), Ordering::Relaxed);
                    cutoff = hits.iter().map(|h| h.rank).max().unwrap_or(u64::MAX);
                }
                if let Some(max) = s.max_results
                    && found.fetch_add(1, Ordering::Relaxed) + 1 >= max
                {
                    stop.store(true, Ordering::Relaxed);
                    return;
                }
                true
            }
        };
        if is_dir {
            let abs = abs.unwrap();
            let st = w.open_dir(p, c, abs, matched, holes);
            if !st.open {
                next = st.end;
            }
            states.push(st);
        }
    }
}

/// The deepest indexed root that contains `path`, if its index file opens and was built
/// with the current settings.
pub fn index_for<'a>(path: &Path, indexes: &'a [(PathBuf, Arc<MappedIndex>)]) -> Option<&'a MappedIndex> {
    indexes.iter().filter(|(root, _)| path.starts_with(root)).max_by_key(|(root, _)| root.as_os_str().len()).map(|(_, m)| &**m)
}

//! A borrowed, read-only view of an index: plain slices, whether they live in a heap-built
//! [`Index`](crate::Index) or in a memory-mapped file ([`MappedIndex`](crate::MappedIndex)).
//! All lookup and search code lives here, so both give identical answers.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use crate::matcher::{self, Matcher};
use crate::{HIDDEN, Kind, NONE, Query, ROOT};

/// Folders with at least this many entries below them are searched on all cores.
const PARALLEL_MIN: usize = 50_000;

/// Where original (on-disk) names come from.
#[derive(Clone, Copy)]
pub(crate) enum Names<'a> {
    /// Heap index: every name stored, `names[off[i]..off[i + 1] - 1]`.
    Owned { off: &'a [u32], names: &'a [u8] },
    /// File format: a name equal to its folded form isn't stored again. `ooff[i]` is
    /// [`NONE`] for those, else the start of a NUL-terminated name in `orig`.
    Deduped { ooff: &'a [u32], orig: &'a [u8] },
}

#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    pub root: &'a Path,
    pub parent: &'a [u32],
    pub end: &'a [u32],
    pub flags: &'a [u8],
    pub foff: &'a [u32],
    pub folded: &'a [u8],
    pub names: Names<'a>,
}

impl<'a> View<'a> {
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    pub fn folded_name(&self, i: usize) -> &'a [u8] {
        &self.folded[self.foff[i] as usize..self.foff[i + 1] as usize - 1]
    }

    pub fn name(&self, id: u32) -> &'a [u8] {
        let i = id as usize;
        match self.names {
            Names::Owned { off, names } => &names[off[i] as usize..off[i + 1] as usize - 1],
            Names::Deduped { ooff, orig } => match ooff[i] {
                NONE => self.folded_name(i),
                o => {
                    let s = &orig[o as usize..];
                    &s[..memchr::memchr(0, s).unwrap_or(s.len())]
                }
            },
        }
    }

    pub fn kind(&self, id: u32) -> Kind {
        Kind::from_bits(self.flags[id as usize])
    }

    pub fn parent(&self, id: u32) -> Option<u32> {
        Some(self.parent[id as usize]).filter(|&p| p != NONE)
    }

    pub fn descendants(&self, id: u32) -> usize {
        (self.end[id as usize] - id - 1) as usize
    }

    pub fn children(self, id: u32) -> impl Iterator<Item = u32> + 'a {
        let end = self.end[id as usize];
        let ends = self.end;
        let mut c = id + 1;
        std::iter::from_fn(move || {
            (c < end).then(|| {
                let this = c;
                c = ends[c as usize];
                this
            })
        })
    }

    pub fn path(&self, id: u32) -> PathBuf {
        let mut chain = Vec::new();
        let mut i = id;
        while i != ROOT && i != NONE {
            chain.push(i);
            i = self.parent[i as usize];
        }
        let mut p = self.root.to_path_buf();
        for &k in chain.iter().rev() {
            p.push(OsStr::from_bytes(self.name(k)));
        }
        p
    }

    pub fn lookup(&self, path: &Path) -> Option<u32> {
        match self.locate(path)? {
            (id, true) => Some(id),
            _ => None,
        }
    }

    /// The deepest indexed entry on the way to `path`, and whether it is `path` itself.
    /// `None` if `path` is outside the root.
    pub fn locate(&self, path: &Path) -> Option<(u32, bool)> {
        let rel = path.strip_prefix(self.root).ok()?;
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

    /// Ids of matching entries, best first. See [`Index::search`](crate::Index::search).
    pub fn search(&self, q: &Query) -> Vec<u32> {
        let Some(m) = Matcher::new(q) else {
            return Vec::new();
        };
        let within = q.within as usize;
        if within >= self.len() {
            return Vec::new();
        }
        let (first, last) = (within + 1, self.end[within] as usize);
        if first >= last {
            return Vec::new();
        }
        let mut keys = if last - first < PARALLEL_MIN {
            let mut v = self.scan(first, last, &m, q);
            matcher::keep_best(&mut v, q.limit);
            v
        } else {
            use rayon::prelude::*;
            let chunks = rayon::current_num_threads() * 4;
            let step = (last - first).div_ceil(chunks);
            (first..last)
                .step_by(step)
                .collect::<Vec<_>>()
                .into_par_iter()
                .map(|a| {
                    let mut v = self.scan(a, (a + step).min(last), &m, q);
                    matcher::keep_best(&mut v, q.limit);
                    v
                })
                .reduce(Vec::new, |mut a, mut b| {
                    a.append(&mut b);
                    a
                })
        };
        matcher::keep_best(&mut keys, q.limit);
        keys.sort_unstable();
        keys.into_iter().map(|k| k as u32).collect()
    }

    /// Rank keys (`Matcher::rank` | id) of the hits among entries `first..last`.
    fn scan(&self, first: usize, last: usize, m: &Matcher, q: &Query) -> Vec<u64> {
        let lo = self.foff[first] as usize;
        let hay = &self.folded[lo..self.foff[last] as usize];
        let mut keys = Vec::new();
        let mut cur = first;
        let mut prev = usize::MAX;
        for pos in m.scan_finder().find_iter(hay) {
            if pos >= hay.len() { break; }
            let abs = (lo + pos) as u32;
            if abs >= self.foff[cur + 1] {
                cur += self.foff[cur + 1..=last].partition_point(|&o| o <= abs);
            }
            if cur == prev {
                continue;
            }
            prev = cur;
            let name = self.folded_name(cur);
            if m.accepts(name, Kind::from_bits(self.flags[cur])) && (q.hidden || !self.hidden_below(cur, q.within)) {
                keys.push(m.rank(name) | cur as u64);
            }
        }
        keys
    }

    /// Whether `i` or any of its ancestors below `within` is dot-named.
    fn hidden_below(&self, i: usize, within: u32) -> bool {
        let mut a = i as u32;
        while a != within && a != NONE {
            if self.flags[a as usize] & HIDDEN != 0 {
                return true;
            }
            a = self.parent[a as usize];
        }
        false
    }
}

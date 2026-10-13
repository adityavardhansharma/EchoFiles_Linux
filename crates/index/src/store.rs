//! On-disk format, designed to be searched in place through `mmap`:
//!
//! ```text
//! header (64 B): magic "EFIDX003", n, folded_len, orig_len, root_len, excl_len,
//!                skip_cache_tagged, reserved                      — all u64
//! root path, exclude names (NUL-separated), zero padding to 4 bytes
//! parent [u32; n] · end [u32; n] · foff [u32; n + 1] · ooff [u32; n]
//! flags [u8; n] · folded [u8; folded_len] · orig [u8; orig_len]
//! ```
//!
//! The folded arena is stored ready to search, so opening an index costs a validation pass
//! and no folding. Original names equal to their folded form (most lowercase ASCII names)
//! are not stored twice: their `ooff` is `NONE`.
//!
//! A [`MappedIndex`] keeps all of this in the page cache, not on the heap: the kernel can
//! drop those pages under memory pressure and read them back on the next search.

use std::ffi::OsStr;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::view::{Names, View};
use crate::{Index, Kind, NONE, Options, Query, Stats};

const MAGIC: &[u8; 8] = b"EFIDX003";
const HEADER: usize = 64;

#[cfg(not(target_endian = "little"))]
compile_error!("the index file format assumes a little-endian target");

fn pad4(n: usize) -> usize {
    n.next_multiple_of(4)
}

impl Index {
    /// Write the index atomically (temp file + rename), so readers — including processes
    /// that have the old file mapped — never see a half-written index.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        self.save_if_changed(path).map(drop)
    }

    /// [`save`](Index::save), skipped when the file already holds exactly these bytes (a
    /// periodic rebuild of an unchanged tree then costs no disk write or fsync). Returns
    /// whether the file was written.
    pub fn save_if_changed(&self, path: &Path) -> io::Result<bool> {
        let n = self.len();
        let v = self.view();
        let mut ooff = Vec::with_capacity(n);
        let mut orig = Vec::new();
        for i in 0..n {
            let name = v.name(i as u32);
            if name == v.folded_name(i) {
                ooff.push(NONE);
            } else {
                ooff.push(orig.len() as u32);
                orig.extend_from_slice(name);
                orig.push(0);
            }
        }
        let root = self.root.as_os_str().as_bytes();
        // Names never contain '/', paths always start with it, so one list holds both.
        let excl = self.opts.exclude_names.iter().chain(&self.opts.exclude_paths).cloned().collect::<Vec<_>>().join(&0u8);

        let parent = path.parent().ok_or_else(|| io::Error::other("missing parent"))?;
        let mut f: Vec<u8> = Vec::new();
        f.write_all(MAGIC)?;
        for x in [n, self.folded.len(), orig.len(), root.len(), excl.len(), self.opts.skip_cache_tagged as usize, 0] {
            f.write_all(&(x as u64).to_le_bytes())?;
        }
        f.write_all(root)?;
        f.write_all(&excl)?;
        f.write_all(&[0u8; 3][..pad4(root.len() + excl.len()) - (root.len() + excl.len())])?;
        for a in [&self.parent, &self.end, &self.foff, &ooff] {
            f.write_all(u32_bytes(a))?;
        }
        f.write_all(&self.flags)?;
        f.write_all(&self.folded)?;
        f.write_all(&orig)?;
        if same_contents(path, &f) {
            return Ok(false);
        }
        write_atomically(path, parent, &f)?;
        Ok(true)
    }

    /// Read a saved index onto the heap (for an index that will be updated with
    /// [`rescan`](Index::rescan)). To only search, [`MappedIndex::open`] is cheaper.
    pub fn load(path: &Path) -> io::Result<Index> {
        let m = MappedIndex::open(path)?;
        let v = m.view();
        let n = v.len();
        let mut off = Vec::with_capacity(n + 1);
        let mut names = Vec::new();
        for i in 0..n {
            off.push(names.len() as u32);
            names.extend_from_slice(v.name(i as u32));
            names.push(0);
        }
        off.push(names.len() as u32);
        Ok(Index {
            root: m.root.clone(),
            parent: v.parent.to_vec(),
            end: v.end.to_vec(),
            flags: v.flags.to_vec(),
            off,
            names,
            foff: v.foff.to_vec(),
            folded: v.folded.to_vec(),
            stats: Stats::default(),
            opts: m.opts.clone(),
        })
    }
}

/// Write `bytes` to a fresh private file next to `path`, flush it, and rename it over
/// `path`; then flush the folder so the rename survives a crash.
fn write_atomically(path: &Path, parent: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    for _ in 0..16 {
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = parent.join(format!(".{name}.{}.{n}.tmp", std::process::id()));
        let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(rustix::fs::OFlags::CLOEXEC.bits() as i32).open(&tmp) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        let done = file.write_all(bytes).and_then(|_| file.sync_all()).and_then(|_| std::fs::rename(&tmp, path));
        if let Err(e) = done {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        return std::fs::File::open(parent)?.sync_all();
    }
    Err(io::Error::other("couldn't create a temporary file"))
}

/// A read-only memory mapping of a whole file, unmapped on drop.
struct Map {
    ptr: *mut std::ffi::c_void,
    len: usize,
}

// SAFETY: the mapping is read-only and never written through; it may be read from any
// thread.
unsafe impl Send for Map {}
unsafe impl Sync for Map {}

impl Map {
    fn of(file: &std::fs::File) -> io::Result<Map> {
        let len = file.metadata()?.len() as usize;
        if len == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "empty file"));
        }
        // SAFETY: a fresh private read-only mapping of an open file; no existing memory is
        // affected.
        let ptr = unsafe { rustix::mm::mmap(std::ptr::null_mut(), len, rustix::mm::ProtFlags::READ, rustix::mm::MapFlags::SHARED, file, 0)? };
        Ok(Map { ptr, len })
    }
}

impl std::ops::Deref for Map {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        // SAFETY: `ptr` maps `len` readable bytes for as long as `self` lives.
        unsafe { std::slice::from_raw_parts(self.ptr.cast::<u8>(), self.len) }
    }
}

impl Drop for Map {
    fn drop(&mut self) {
        // SAFETY: unmapping exactly the range `of` mapped; no slices outlive `self`.
        unsafe {
            let _ = rustix::mm::munmap(self.ptr, self.len);
        }
    }
}

/// A saved index searched in place, without copying it onto the heap. Read-only: the
/// indexer rewrites the file (atomically) and readers reopen it.
pub struct MappedIndex {
    map: Map,
    root: PathBuf,
    opts: Options,
    n: usize,
    parent_at: usize,
    folded_len: usize,
    orig_len: usize,
}

impl MappedIndex {
    pub fn open(path: &Path) -> io::Result<MappedIndex> {
        let file = std::fs::File::open(path)?;
        // The indexer only ever replaces index files by rename, never truncates or writes
        // them in place, so the mapped bytes can't change under us.
        let map = Map::of(&file)?;
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "not an EchoFiles index");
        if map.len() < HEADER || &map[..8] != MAGIC {
            return Err(bad());
        }
        let rd = |at: usize| u64::from_le_bytes(map[at..at + 8].try_into().unwrap()) as usize;
        let (n, folded_len, orig_len, root_len, excl_len, skip) = (rd(8), rd(16), rd(24), rd(32), rd(40), rd(48));
        let parent_at = root_len.checked_add(excl_len).and_then(|v| v.checked_add(3)).map(|v| v & !3).and_then(|v| v.checked_add(HEADER)).ok_or_else(bad)?;
        let need = n
            .checked_mul(17)
            .and_then(|v| parent_at.checked_add(4).and_then(|p| v.checked_add(p)))
            .and_then(|v| v.checked_add(folded_len))
            .and_then(|v| v.checked_add(orig_len));
        if n == 0 || need != Some(map.len()) {
            return Err(bad());
        }
        let root = PathBuf::from(OsStr::from_bytes(&map[HEADER..HEADER + root_len]));
        let excl = &map[HEADER + root_len..HEADER + root_len + excl_len];
        let all: Vec<Vec<u8>> = if excl.is_empty() { Vec::new() } else { excl.split(|&b| b == 0).map(<[u8]>::to_vec).collect() };
        let (exclude_paths, exclude_names) = all.into_iter().partition(|e| e.first() == Some(&b'/'));
        let m = MappedIndex {
            map,
            root,
            opts: Options { exclude_names, skip_cache_tagged: skip != 0, exclude_paths },
            n,
            parent_at,
            folded_len,
            orig_len,
        };
        m.validate().then_some(m).ok_or_else(bad)
    }

    /// One linear pass proving every later slice and lookup is in bounds, so a corrupt file
    /// is rejected here instead of crashing a search.
    fn validate(&self) -> bool {
        let v = self.view();
        let n = self.n;
        let tree = v.parent[0] == NONE
            && v.end[0] as usize == n
            && (1..n).all(|i| {
                let (p, e) = (v.parent[i] as usize, v.end[i] as usize);
                p < i && e > i && e <= v.end[p] as usize
            });
        let folded_ok = v.foff[0] == 0
            && v.foff[n] as usize == v.folded.len()
            && v.foff.windows(2).all(|w| w[0] < w[1])
            && (0..n).all(|i| v.folded[v.foff[i + 1] as usize - 1] == 0);
        let orig_ok = match v.names {
            Names::Deduped { ooff, orig } => {
                orig.last().is_none_or(|&b| b == 0) && ooff.iter().all(|&o| o == NONE || (o as usize) < orig.len())
            }
            Names::Owned { .. } => false,
        };
        tree && folded_ok && orig_ok
    }

    pub(crate) fn view(&self) -> View<'_> {
        let n = self.n;
        let at = self.parent_at;
        let u32s = |from: usize, len: usize| -> &[u32] {
            let bytes = &self.map[from..from + len * 4];
            // SAFETY: `from` is a multiple of 4 inside a page-aligned mapping, the range was
            // bounds-checked in `open`, and every bit pattern is a valid u32 (little-endian
            // enforced above).
            unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<u32>(), len) }
        };
        let flags_at = at + 4 * (4 * n + 1);
        let folded_at = flags_at + n;
        View {
            root: &self.root,
            parent: u32s(at, n),
            end: u32s(at + 4 * n, n),
            foff: u32s(at + 8 * n, n + 1),
            flags: &self.map[flags_at..folded_at],
            folded: &self.map[folded_at..folded_at + self.folded_len],
            names: Names::Deduped {
                ooff: u32s(at + 4 * (3 * n + 1), n),
                orig: &self.map[folded_at + self.folded_len..folded_at + self.folded_len + self.orig_len],
            },
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn options(&self) -> &Options {
        &self.opts
    }

    /// Number of entries, including the root.
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n <= 1
    }

    /// Size of the mapped file (page cache, not heap).
    pub fn mapped_bytes(&self) -> usize {
        self.map.len()
    }

    pub fn name(&self, id: u32) -> &[u8] {
        self.view().name(id)
    }

    pub fn kind(&self, id: u32) -> Kind {
        self.view().kind(id)
    }

    pub fn path(&self, id: u32) -> PathBuf {
        self.view().path(id)
    }

    pub fn lookup(&self, path: &Path) -> Option<u32> {
        self.view().lookup(path)
    }

    /// Same results as [`Index::search`] on the index this file was saved from.
    pub fn search(&self, q: &Query) -> Vec<u32> {
        self.view().search(q)
    }

    /// See [`Index::search_counted`](crate::Index::search_counted).
    pub fn search_counted(&self, q: &Query) -> (usize, Vec<u32>) {
        self.view().search_counted(q)
    }

    /// Number of entries below `id` (all depths).
    pub fn descendants(&self, id: u32) -> usize {
        self.view().descendants(id)
    }
}

/// Whether the file at `path` holds exactly `bytes` (compared in chunks, no second copy).
fn same_contents(path: &Path, bytes: &[u8]) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    if file.metadata().map(|m| m.len()).ok() != Some(bytes.len() as u64) { return false; }
    let mut buf = vec![0u8; 1 << 20];
    let mut at = 0;
    while at < bytes.len() {
        let want = buf.len().min(bytes.len() - at);
        if file.read_exact(&mut buf[..want]).is_err() || buf[..want] != bytes[at..at + want] { return false; }
        at += want;
    }
    true
}

fn u32_bytes(v: &[u32]) -> &[u8] {
    // SAFETY: u32 has no padding and any byte pattern is a valid u8; little-endian is
    // enforced above.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), std::mem::size_of_val(v)) }
}

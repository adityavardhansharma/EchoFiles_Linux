//! Prototype file-name index (build plan §2.8) — measures whether an EchoFiles index beats
//! `find`/`fd` for agents. Names only: no `statx` during the crawl.
//!
//! Layout (one file, little-endian): magic, counts, `parent: [u32]`, `is_dir: [u8]`,
//! `off: [u32; n+1]`, `names` (NUL-separated), `lower` (ASCII-lowercased copy of `names`, same
//! offsets). Queries run `memmem` over `lower` in one pass; NUL separators stop matches from
//! spanning two names.

use std::io::{self, Read, Write};
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use rustix::fs::{Mode, OFlags, RawDir};

const MAGIC: &[u8; 8] = b"EFIDX001";
const NO_PARENT: u32 = u32::MAX;

pub struct Index {
    pub root: PathBuf,
    pub parent: Vec<u32>,
    pub is_dir: Vec<u8>,
    pub off: Vec<u32>,
    pub names: Vec<u8>,
    pub lower: Vec<u8>,
}

/// One directory's entries, gathered by one task and merged at the end.
struct Batch {
    ids: Vec<u32>,
    parents: Vec<u32>,
    dirs: Vec<u8>,
    names: Vec<u8>,
    ends: Vec<u32>,
}

struct Ctx {
    next: AtomicU32,
    batches: Mutex<Vec<Batch>>,
}

fn walk<'s>(path: Vec<u8>, id: u32, ctx: &'s Ctx, scope: &rayon::Scope<'s>) {
    let Ok(fd) = rustix::fs::open(
        Path::new(std::ffi::OsStr::from_bytes_compat(&path)),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    ) else {
        return;
    };
    let mut buf: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); 64 * 1024];
    let mut raw = RawDir::new(&fd, &mut buf);
    let mut b = Batch { ids: Vec::new(), parents: Vec::new(), dirs: Vec::new(), names: Vec::new(), ends: Vec::new() };
    let mut children = Vec::new();
    while let Some(Ok(e)) = raw.next() {
        let name = e.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        let child = ctx.next.fetch_add(1, Ordering::Relaxed);
        let dir = e.file_type() == rustix::fs::FileType::Directory;
        b.ids.push(child);
        b.parents.push(id);
        b.dirs.push(dir as u8);
        b.names.extend_from_slice(name);
        b.ends.push(b.names.len() as u32);
        if dir {
            let mut p = path.clone();
            p.push(b'/');
            p.extend_from_slice(name);
            children.push((p, child));
        }
    }
    drop(fd);
    if !b.ids.is_empty() {
        ctx.batches.lock().unwrap().push(b);
    }
    for (p, child) in children {
        scope.spawn(move |s| walk(p, child, ctx, s));
    }
}

trait FromBytes {
    fn from_bytes_compat(b: &[u8]) -> &Self;
}
impl FromBytes for std::ffi::OsStr {
    fn from_bytes_compat(b: &[u8]) -> &Self {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::OsStr::from_bytes(b)
    }
}

impl Index {
    /// Crawl `root` in parallel (one rayon task per directory; symlinks are not followed).
    pub fn build(root: &Path) -> Index {
        use std::os::unix::ffi::OsStrExt;
        let ctx = Ctx { next: AtomicU32::new(1), batches: Mutex::new(Vec::new()) };
        let root_bytes = root.as_os_str().as_bytes().to_vec();
        rayon::scope(|s| walk(root_bytes, 0, &ctx, s));
        let n = ctx.next.load(Ordering::Relaxed) as usize;
        let batches = ctx.batches.into_inner().unwrap();

        // Scatter every batch into id order.
        let mut parent = vec![NO_PARENT; n];
        let mut is_dir = vec![0u8; n];
        let mut spans: Vec<(u32, u32, u32)> = vec![(0, 0, 0); n]; // (batch, start, end)
        is_dir[0] = 1;
        for (bi, b) in batches.iter().enumerate() {
            let mut start = 0u32;
            for k in 0..b.ids.len() {
                let id = b.ids[k] as usize;
                parent[id] = b.parents[k];
                is_dir[id] = b.dirs[k];
                spans[id] = (bi as u32, start, b.ends[k]);
                start = b.ends[k];
            }
        }
        let total: usize = batches.iter().map(|b| b.names.len()).sum::<usize>() + n;
        let mut names = Vec::with_capacity(total);
        let mut off = Vec::with_capacity(n + 1);
        for (id, &(bi, a, e)) in spans.iter().enumerate() {
            off.push(names.len() as u32);
            if id > 0 {
                names.extend_from_slice(&batches[bi as usize].names[a as usize..e as usize]);
            }
            names.push(0);
        }
        off.push(names.len() as u32);
        let lower = names.to_ascii_lowercase();
        Index { root: root.to_path_buf(), parent, is_dir, off, names, lower }
    }

    pub fn len(&self) -> usize {
        self.parent.len()
    }

    pub fn name(&self, i: usize) -> &[u8] {
        &self.names[self.off[i] as usize..self.off[i + 1] as usize - 1]
    }

    pub fn path(&self, mut i: usize) -> PathBuf {
        use std::os::unix::ffi::OsStrExt;
        let mut parts = Vec::new();
        while i != 0 && i != NO_PARENT as usize {
            parts.push(i);
            i = self.parent[i] as usize;
        }
        let mut p = self.root.clone();
        for &k in parts.iter().rev() {
            p.push(std::ffi::OsStr::from_bytes(self.name(k)));
        }
        p
    }

    /// Entries whose lowercased name contains `needle` (and, with `suffix`, ends with it).
    pub fn find(&self, needle: &str, suffix: bool) -> Vec<u32> {
        let needle = needle.to_ascii_lowercase();
        let finder = memchr::memmem::Finder::new(needle.as_bytes());
        let mut hits = Vec::new();
        let mut last = u32::MAX;
        for pos in finder.find_iter(&self.lower) {
            let i = self.off.partition_point(|&o| o as usize <= pos) - 1;
            if i as u32 == last || i == 0 {
                continue;
            }
            if suffix && pos + needle.len() != self.off[i + 1] as usize - 1 {
                continue;
            }
            last = i as u32;
            hits.push(i as u32);
        }
        hits
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        use std::os::unix::ffi::OsStrExt;
        let mut f = io::BufWriter::new(std::fs::File::create(path)?);
        let root = self.root.as_os_str().as_bytes();
        f.write_all(MAGIC)?;
        for v in [self.len() as u64, self.names.len() as u64, root.len() as u64] {
            f.write_all(&v.to_le_bytes())?;
        }
        f.write_all(root)?;
        f.write_all(as_bytes(&self.parent))?;
        f.write_all(&self.is_dir)?;
        f.write_all(as_bytes(&self.off))?;
        f.write_all(&self.names)?;
        f.write_all(&self.lower)?;
        f.flush()
    }

    pub fn load(path: &Path) -> io::Result<Index> {
        use std::os::unix::ffi::OsStrExt;
        let mut data = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut data)?;
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "not an EchoFiles index");
        if data.len() < 32 || &data[..8] != MAGIC {
            return Err(bad());
        }
        let rd = |at: usize| u64::from_le_bytes(data[at..at + 8].try_into().unwrap()) as usize;
        let (n, nl, rl) = (rd(8), rd(16), rd(24));
        let mut at = 32;
        let mut take = |len: usize| {
            let s = at;
            at += len;
            s..at
        };
        let root = PathBuf::from(std::ffi::OsStr::from_bytes(&data[take(rl)]));
        let parent = from_bytes(&data[take(n * 4)]);
        let is_dir = data[take(n)].to_vec();
        let off = from_bytes(&data[take((n + 1) * 4)]);
        let names = data[take(nl)].to_vec();
        let lower = data[take(nl)].to_vec();
        Ok(Index { root, parent, is_dir, off, names, lower })
    }
}

fn as_bytes(v: &[u32]) -> &[u8] {
    // SAFETY: u32 has no padding; little-endian target (x86_64).
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 4) }
}

fn from_bytes(b: &[u8]) -> Vec<u32> {
    b.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()
}

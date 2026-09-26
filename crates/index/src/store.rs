//! On-disk format (little-endian): magic, `n`, name-arena length, root length, root path,
//! `parent: [u32; n]`, `end: [u32; n]`, `flags: [u8; n]`, `off: [u32; n + 1]`, `names`.
//!
//! The folded arena is not stored: it's rebuilt on load (cheap for ASCII names) so the file
//! is about half the size of names + folded.

use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::{Index, Stats, fold};

const MAGIC: &[u8; 8] = b"EFIDX002";

#[cfg(not(target_endian = "little"))]
compile_error!("the index file format assumes a little-endian target");

impl Index {
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("tmp");
        let mut f = io::BufWriter::new(std::fs::File::create(&tmp)?);
        let root = self.root.as_os_str().as_bytes();
        f.write_all(MAGIC)?;
        for v in [self.len(), self.names.len(), root.len()] {
            f.write_all(&(v as u64).to_le_bytes())?;
        }
        f.write_all(root)?;
        f.write_all(u32_bytes(&self.parent))?;
        f.write_all(u32_bytes(&self.end))?;
        f.write_all(&self.flags)?;
        f.write_all(u32_bytes(&self.off))?;
        f.write_all(&self.names)?;
        f.into_inner().map_err(|e| e.into_error())?;
        // Readers never see a half-written index.
        std::fs::rename(&tmp, path)
    }

    pub fn load(path: &Path) -> io::Result<Index> {
        let mut data = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut data)?;
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "not an EchoFiles index");
        if data.len() < 32 || &data[..8] != MAGIC {
            return Err(bad());
        }
        let rd = |at: usize| u64::from_le_bytes(data[at..at + 8].try_into().unwrap()) as usize;
        let (n, nl, rl) = (rd(8), rd(16), rd(24));
        let need = n.checked_mul(13).and_then(|v| v.checked_add(4 + nl + rl + 32));
        if n == 0 || need != Some(data.len()) {
            return Err(bad());
        }
        let mut at = 32;
        let mut take = |len: usize| {
            at += len;
            &data[at - len..at]
        };
        let root = PathBuf::from(OsStr::from_bytes(take(rl)));
        let parent = u32_vec(take(n * 4));
        let end = u32_vec(take(n * 4));
        let flags = take(n).to_vec();
        let off = u32_vec(take((n + 1) * 4));
        let names = take(nl).to_vec();
        let tree_ok = parent[0] == crate::NONE
            && end[0] as usize == n
            && (1..n).all(|i| {
                let (p, e) = (parent[i] as usize, end[i] as usize);
                p < i && e > i && e <= end[p] as usize
            });
        if !tree_ok || off[0] != 0 || off[n] as usize != names.len() {
            return Err(bad());
        }

        let mut foff = Vec::with_capacity(n + 1);
        let mut folded = Vec::with_capacity(names.len());
        for w in off.windows(2) {
            let (a, b) = (w[0] as usize, w[1] as usize);
            if a >= b || b > names.len() {
                return Err(bad());
            }
            foff.push(folded.len() as u32);
            fold::fold_into(&names[a..b - 1], &mut folded);
            folded.push(0);
        }
        foff.push(folded.len() as u32);
        Ok(Index { root, parent, end, flags, off, names, foff, folded, stats: Stats::default() })
    }
}

fn u32_bytes(v: &[u32]) -> &[u8] {
    // SAFETY: u32 has no padding and any byte pattern is a valid u8; little-endian is
    // enforced above.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), std::mem::size_of_val(v)) }
}

fn u32_vec(b: &[u8]) -> Vec<u32> {
    b.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()
}

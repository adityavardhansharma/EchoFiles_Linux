//! Sort keys and order permutations (build plan §2.3).
//!
//! A sort never moves entries: it produces a `Vec<u32>` permutation over a [`Listing`].
//! Name keys are computed once — casefolded, with digit runs encoded so `file2 < file10`,
//! the order Windows Explorer uses.

use std::cmp::Ordering;

use rayon::prelude::*;

use crate::listing::{Kind, Listing};

/// Above this many entries the sort runs on the rayon pool.
const PARALLEL_SORT_MIN: usize = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Size,
    Modified,
    Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortSpec {
    pub by: SortBy,
    pub descending: bool,
    pub folders_first: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self { by: SortBy::Name, descending: false, folders_first: true }
    }
}

/// Natural-order name keys for a whole listing, stored in one arena.
pub struct NameKeys {
    bytes: Vec<u8>,
    off: Vec<u32>,
}

impl NameKeys {
    pub fn build(listing: &Listing) -> Self {
        let n = listing.len();
        // Build per-chunk arenas in parallel, then stitch them together.
        let parts: Vec<(Vec<u8>, Vec<u32>)> = (0..n)
            .collect::<Vec<_>>()
            .par_chunks(4096)
            .map(|chunk| {
                let mut bytes = Vec::with_capacity(chunk.len() * 20);
                let mut ends = Vec::with_capacity(chunk.len());
                for &i in chunk {
                    push_key(&mut bytes, listing.name_bytes(i));
                    ends.push(bytes.len() as u32);
                }
                (bytes, ends)
            })
            .collect();
        let total: usize = parts.iter().map(|p| p.0.len()).sum();
        let mut bytes = Vec::with_capacity(total);
        let mut off = Vec::with_capacity(n + 1);
        off.push(0);
        for (b, ends) in parts {
            let base = bytes.len() as u32;
            bytes.extend_from_slice(&b);
            off.extend(ends.into_iter().map(|e| base + e));
        }
        Self { bytes, off }
    }

    pub fn get(&self, i: usize) -> &[u8] {
        &self.bytes[self.off[i] as usize..self.off[i + 1] as usize]
    }

    /// First 8 key bytes packed big-endian: comparing prefixes orders like comparing keys,
    /// except on ties.
    fn prefix(&self, i: usize) -> u64 {
        let k = self.get(i);
        let mut buf = [0u8; 8];
        let m = k.len().min(8);
        buf[..m].copy_from_slice(&k[..m]);
        u64::from_be_bytes(buf)
    }
}

/// Append the natural sort key for `name` to `out`.
///
/// Letters are casefolded (ASCII fast path; full Unicode lowercase for non-ASCII UTF-8).
/// A run of digits becomes `0x1F, len, digits…` with leading zeros stripped, so shorter
/// numbers sort first and runs sort before letters.
pub fn push_key(out: &mut Vec<u8>, name: &[u8]) {
    let mut i = 0;
    while i < name.len() {
        let b = name[i];
        if b.is_ascii_digit() {
            let start = i;
            while i < name.len() && name[i].is_ascii_digit() {
                i += 1;
            }
            let digits = &name[start..i];
            let trimmed = match digits.iter().position(|&d| d != b'0') {
                Some(p) => &digits[p..],
                None => &digits[digits.len() - 1..],
            };
            out.push(0x1F);
            out.push(trimmed.len().min(255) as u8);
            out.extend_from_slice(trimmed);
        } else if b.is_ascii() {
            out.push(b.to_ascii_lowercase());
            i += 1;
        } else {
            // Lowercase the next UTF-8 character; invalid bytes pass through unchanged.
            let rest = &name[i..];
            let len = utf8_len(b).min(rest.len());
            match std::str::from_utf8(&rest[..len]) {
                Ok(s) => {
                    for c in s.chars().flat_map(char::to_lowercase) {
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                    }
                    i += len;
                }
                Err(_) => {
                    out.push(b);
                    i += 1;
                }
            }
        }
    }
}

fn utf8_len(b: u8) -> usize {
    match b {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

/// Order of entries under `spec`, optionally skipping hidden files.
pub fn order(listing: &Listing, keys: &NameKeys, spec: SortSpec, show_hidden: bool) -> Vec<u32> {
    let visible = |i: usize| show_hidden || !listing.is_hidden(i);
    let parallel = listing.len() >= PARALLEL_SORT_MIN;
    if spec.by == SortBy::Name {
        // Fast path: sort (group, prefix, index) triples; full keys only break prefix ties.
        let mut v: Vec<(u64, u32)> = (0..listing.len())
            .filter(|&i| visible(i))
            .map(|i| {
                let group = u64::from(spec.folders_first && listing.kind[i] != Kind::Dir);
                (group << 63 | keys.prefix(i) >> 1, i as u32)
            })
            .collect();
        let cmp = |a: &(u64, u32), b: &(u64, u32)| {
            a.0.cmp(&b.0).then_with(|| {
                let (x, y) = (a.1 as usize, b.1 as usize);
                keys.get(x).cmp(keys.get(y)).then_with(|| listing.name_bytes(x).cmp(listing.name_bytes(y)))
            })
        };
        if parallel { v.par_sort_unstable_by(cmp) } else { v.sort_unstable_by(cmp) }
        let mut out: Vec<u32> = v.into_iter().map(|(_, i)| i).collect();
        if spec.descending {
            // Keep folders first when reversing the name order.
            let split = if spec.folders_first { out.iter().position(|&i| listing.kind[i as usize] != Kind::Dir).unwrap_or(out.len()) } else { out.len() };
            out[..split].reverse();
            out[split..].reverse();
        }
        return out;
    }
    let mut idx: Vec<u32> = (0..listing.len() as u32).filter(|&i| visible(i as usize)).collect();
    let cmp = |a: &u32, b: &u32| compare(listing, keys, spec, *a as usize, *b as usize);
    if parallel { idx.par_sort_unstable_by(cmp) } else { idx.sort_unstable_by(cmp) }
    idx
}

fn compare(l: &Listing, keys: &NameKeys, spec: SortSpec, a: usize, b: usize) -> Ordering {
    if spec.folders_first {
        let (da, db) = (l.kind[a] == Kind::Dir, l.kind[b] == Kind::Dir);
        if da != db {
            return db.cmp(&da);
        }
    }
    let primary = match spec.by {
        SortBy::Name => Ordering::Equal,
        SortBy::Size => l.size[a].cmp(&l.size[b]),
        SortBy::Modified => l.mtime[a].cmp(&l.mtime[b]),
        SortBy::Kind => extension(l.name_bytes(a)).cmp(extension(l.name_bytes(b))),
    };
    let ord = primary
        .then_with(|| keys.get(a).cmp(keys.get(b)))
        .then_with(|| l.name_bytes(a).cmp(l.name_bytes(b)));
    if spec.descending { ord.reverse() } else { ord }
}

fn extension(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&c| c == b'.') {
        Some(p) if p > 0 => &name[p + 1..],
        _ => b"",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> Vec<u8> {
        let mut v = Vec::new();
        push_key(&mut v, s.as_bytes());
        v
    }

    #[test]
    fn natural_order() {
        let mut names = vec!["file10.txt", "File2.txt", "file1.txt", "file02b", "Äpfel", "apple", "zeta"];
        names.sort_by_key(|n| key(n));
        assert_eq!(names, ["apple", "file1.txt", "File2.txt", "file02b", "file10.txt", "zeta", "Äpfel"]);
    }

    #[test]
    fn leading_zeros_tie() {
        assert_eq!(key("img002"), key("IMG2"));
    }
}

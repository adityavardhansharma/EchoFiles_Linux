//! "Did you mean …?" — names in an index close to a query that found nothing.
//!
//! Every name in scope is compared, in parallel, word by word: `UserServce` finds
//! `UserService.kt`, `invoce` finds `invoice-2023.pdf`. A banded edit distance with an
//! early exit keeps it to a few milliseconds per million names.

use rayon::prelude::*;

use crate::fold;
use crate::store::MappedIndex;

/// Edit distance between `a` and `b`, or `None` if it is more than `max`.
pub fn distance(a: &[u8], b: &[u8], max: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        let mut best = cur[0];
        for (j, &cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
            best = best.min(cur[j + 1]);
        }
        if best > max {
            return None;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    Some(prev[b.len()]).filter(|&d| d <= max)
}

fn allowed(len: usize) -> usize {
    match len {
        0..=3 => 0,
        4..=5 => 1,
        6..=9 => 2,
        _ => 3,
    }
}

/// How close a folded name is to the folded query: the best distance of the query to the
/// whole stem or to any run of the name's words.
fn closeness(q: &[u8], name: &[u8], max: usize) -> Option<usize> {
    let stem = memchr::memrchr(b'.', name).filter(|&i| i > 0).map_or(name, |i| &name[..i]);
    let mut best = distance(q, stem, max);
    // A query with an extension ("seach.rs") is compared with the whole name.
    if let Some(d) = distance(q, name, max) {
        best = Some(best.map_or(d, |b| b.min(d)));
    }
    let words: Vec<&[u8]> = stem.split(|b| !b.is_ascii_alphanumeric() && *b < 0x80).filter(|w| !w.is_empty()).collect();
    for i in 0..words.len() {
        // Single words, and pairs joined without the separator ("user service" ~ "userservice").
        let mut joined = Vec::new();
        for w in words.iter().skip(i).take(2) {
            joined.extend_from_slice(w);
            if let Some(d) = distance(q, &joined, max) {
                best = Some(best.map_or(d, |b| b.min(d)));
            }
        }
    }
    best
}

/// Up to `k` distinct names below entry `within` close to `query`, closest first.
pub fn similar(m: &MappedIndex, within: u32, query: &str, k: usize) -> Vec<(String, usize)> {
    let q = fold::fold(query.trim());
    let max = allowed(q.len());
    if max == 0 {
        return Vec::new();
    }
    let v = m.view();
    let (first, last) = (within as usize + 1, v.end[within as usize] as usize);
    if first >= last {
        return Vec::new();
    }
    let step = (last - first).div_ceil(rayon::current_num_threads() * 4).max(4096);
    let mut found: Vec<(usize, u32)> = (first..last)
        .step_by(step)
        .collect::<Vec<_>>()
        .into_par_iter()
        .flat_map_iter(|a| {
            let q = &q;
            (a..(a + step).min(last)).filter_map(move |i| {
                let name = v.folded_name(i);
                if name.len() + max < q.len() {
                    return None;
                }
                closeness(q, name, max).filter(|&d| d > 0 || name != q.as_slice()).map(|d| (d, i as u32))
            })
        })
        .collect();
    found.sort_by_key(|&(d, i)| (d, v.folded_name(i as usize).len(), i));
    let mut out: Vec<(String, usize)> = Vec::new();
    for (d, i) in found {
        let name = String::from_utf8_lossy(v.name(i)).into_owned();
        if !out.iter().any(|(n, _)| *n == name) {
            out.push((name, d));
        }
        if out.len() == k {
            break;
        }
    }
    out
}

/// [`similar`] for names that aren't in an index (a live search's listing).
pub fn similar_names<'a>(names: impl Iterator<Item = &'a [u8]>, query: &str, k: usize) -> Vec<(String, usize)> {
    let q = fold::fold(query.trim());
    let max = allowed(q.len());
    if max == 0 {
        return Vec::new();
    }
    let mut found: Vec<(usize, usize, String)> = Vec::new();
    let mut folded = Vec::new();
    for name in names {
        folded.clear();
        fold::fold_into(name, &mut folded);
        if folded.len() + max < q.len() {
            continue;
        }
        if let Some(d) = closeness(&q, &folded, max).filter(|&d| d > 0 || folded != q) {
            found.push((d, name.len(), String::from_utf8_lossy(name).into_owned()));
        }
    }
    found.sort();
    found.dedup_by(|a, b| a.2 == b.2);
    found.into_iter().take(k).map(|(d, _, n)| (n, d)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances() {
        assert_eq!(distance(b"kitten", b"sitting", 3), Some(3));
        assert_eq!(distance(b"kitten", b"sitting", 2), None);
        assert_eq!(closeness(b"userservce", b"userservice.kt", 2), Some(1));
        assert_eq!(closeness(b"invoce", b"invoice-2023.pdf", 2), Some(1));
        assert_eq!(closeness(b"userservice", b"user_service.rs", 2), Some(0));
        assert_eq!(closeness(b"seach.rs", b"search.rs", 2), Some(1));
    }
}

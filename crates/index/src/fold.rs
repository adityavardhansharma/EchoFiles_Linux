//! Search folding: the form both names and queries are matched in.
//!
//! - ASCII: lowercase (the fast path; most names never leave it).
//! - Unicode: compatibility decomposition (NFKD), lowercase, accents dropped. So
//!   "resume" finds "Résumé" whether the name is stored composed (NFC) or decomposed (NFD,
//!   as macOS and some Windows tools write it), and "report" finds fullwidth "Ｒｅｐｏｒｔ".
//! - Only the combining *diacritic* blocks are dropped (U+0300–036F and friends): marks that
//!   carry meaning in other scripts (Devanagari vowel signs, kana voicing marks) are kept.
//! - `ß`/`ẞ` → `ss`, final `ς` → `σ`: the full case-folding rules `to_lowercase` misses.
//! - Bytes that are not UTF-8 (legal in Linux names) are kept as they are; ASCII runs around
//!   them still fold.
//!
//! Folding never produces a NUL byte, so NUL-separated arenas keep matches inside one name.

use crate::fold_tables::{CCC, DATA, DECOMP, FOLD, FOLD_BYTES, SLOW};

/// Appends the folded form of `name` to `out`.
pub fn fold_into(name: &[u8], out: &mut Vec<u8>) {
    if name.is_ascii() {
        out.extend(name.iter().map(u8::to_ascii_lowercase));
        return;
    }
    for chunk in name.utf8_chunks() {
        fold_str(chunk.valid(), out);
        out.extend_from_slice(chunk.invalid());
    }
}

/// The folded form of `s`.
pub fn fold(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    fold_into(s.as_bytes(), &mut out);
    out
}

fn fold_str(s: &str, out: &mut Vec<u8>) {
    if s.is_ascii() {
        out.extend(s.bytes().map(|b| b.to_ascii_lowercase()));
        return;
    }
    // Fast path: every character folds on its own (one table lookup each).
    let start = out.len();
    let mut buf = [0u8; 4];
    for c in s.chars() {
        if c.is_ascii() {
            out.push((c as u8).to_ascii_lowercase());
            continue;
        }
        let cp = c as u32;
        let (a, b) = fold_pages()[(cp >> 8) as usize];
        let page = &FOLD[a as usize..b as usize];
        if let Ok(i) = page.binary_search_by_key(&cp, |e| e.0) {
            let (_, at, len) = page[i];
            out.extend_from_slice(&FOLD_BYTES[at as usize..at as usize + len as usize]);
        } else if slow_pages()[(cp >> 8) as usize].0 != slow_pages()[(cp >> 8) as usize].1 && SLOW.binary_search(&cp).is_ok() {
            out.truncate(start);
            return fold_slow(s, out);
        } else {
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
}

/// Full NFKD with canonical reordering, for strings with combining marks that are kept.
fn fold_slow(s: &str, out: &mut Vec<u8>) {
    // NFKD: full compatibility decomposition, then canonical ordering (stable sort of each
    // run of combining marks by combining class).
    let mut d: Vec<(char, u8)> = Vec::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            d.push((c, 0));
        } else {
            decompose(c, &mut |x| d.push((x, ccc(x))));
        }
    }
    let mut i = 0;
    while i < d.len() {
        if d[i].1 == 0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < d.len() && d[i].1 != 0 {
            i += 1;
        }
        d[start..i].sort_by_key(|x| x.1);
    }
    let mut buf = [0u8; 4];
    for &(c, _) in &d {
        if c.is_ascii() {
            out.push((c as u8).to_ascii_lowercase());
            continue;
        }
        if is_diacritic(c) {
            continue;
        }
        for l in c.to_lowercase() {
            match l {
                'ß' => out.extend_from_slice(b"ss"),
                'ς' => out.extend_from_slice('σ'.encode_utf8(&mut buf).as_bytes()),
                // Lowercasing can reintroduce a mark (`İ` → `i` + U+0307).
                l if is_diacritic(l) => {}
                l => out.extend_from_slice(l.encode_utf8(&mut buf).as_bytes()),
            }
        }
    }
}

/// Per 256-code-point page: the range of `table` entries on that page.
fn pages<T>(table: &'static [(u32, T)]) -> Vec<(u32, u32)>
where
    T: 'static,
{
    let mut p = vec![(0u32, 0u32); 0x1100];
    let mut i = 0;
    while i < table.len() {
        let page = (table[i].0 >> 8) as usize;
        let start = i;
        while i < table.len() && (table[i].0 >> 8) as usize == page {
            i += 1;
        }
        p[page] = (start as u32, i as u32);
    }
    p
}

fn fold_pages() -> &'static [(u32, u32)] {
    static P: std::sync::OnceLock<Vec<(u32, u32)>> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let keys: &'static [(u32, ())] = Box::leak(FOLD.iter().map(|e| (e.0, ())).collect::<Vec<_>>().into_boxed_slice());
        pages(keys)
    })
}

fn slow_pages() -> &'static [(u32, u32)] {
    static P: std::sync::OnceLock<Vec<(u32, u32)>> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let keys: &'static [(u32, ())] = Box::leak(SLOW.iter().map(|&e| (e, ())).collect::<Vec<_>>().into_boxed_slice());
        pages(keys)
    })
}

fn decomp_pages() -> &'static [(u32, u32)] {
    static P: std::sync::OnceLock<Vec<(u32, u32)>> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let keys: &'static [(u32, ())] = Box::leak(DECOMP.iter().map(|e| (e.0, ())).collect::<Vec<_>>().into_boxed_slice());
        pages(keys)
    })
}

fn ccc_pages() -> &'static [(u32, u32)] {
    static P: std::sync::OnceLock<Vec<(u32, u32)>> = std::sync::OnceLock::new();
    P.get_or_init(|| pages(&CCC))
}

/// Canonical combining class (0 for most characters).
fn ccc(c: char) -> u8 {
    let cp = c as u32;
    let Some(&(a, b)) = ccc_pages().get((cp >> 8) as usize) else { return 0 };
    let page = &CCC[a as usize..b as usize];
    page.binary_search_by_key(&cp, |e| e.0).map_or(0, |i| page[i].1)
}

/// Full compatibility decomposition of one character.
fn decompose(c: char, emit: &mut impl FnMut(char)) {
    let cp = c as u32;
    // Hangul syllables: by formula (Unicode §3.12).
    if (0xAC00..=0xD7A3).contains(&cp) {
        let s = cp - 0xAC00;
        let (l, v, t) = (0x1100 + s / 588, 0x1161 + (s % 588) / 28, 0x11A7 + s % 28);
        for x in [l, v] {
            emit(char::from_u32(x).unwrap());
        }
        if t != 0x11A7 {
            emit(char::from_u32(t).unwrap());
        }
        return;
    }
    if let Some(&(a, b)) = decomp_pages().get((cp >> 8) as usize) {
        let page = &DECOMP[a as usize..b as usize];
        if let Ok(i) = page.binary_search_by_key(&cp, |e| e.0) {
            let (_, start, len) = page[i];
            for &x in &DATA[start as usize..start as usize + len as usize] {
                emit(char::from_u32(x).unwrap());
            }
            return;
        }
    }
    emit(c);
}

fn is_diacritic(c: char) -> bool {
    matches!(c, '\u{300}'..='\u{36F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}' | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(s: &str) -> String {
        String::from_utf8(fold(s)).unwrap()
    }

    #[test]
    fn ascii_lowercases() {
        assert_eq!(f("Report FINAL (2).PDF"), "report final (2).pdf");
    }

    #[test]
    fn accents_fold_in_nfc_and_nfd() {
        assert_eq!(f("Résumé"), "resume");
        assert_eq!(f("Re\u{301}sume\u{301}"), "resume");
        assert_eq!(f("naïve café"), "naive cafe");
    }

    #[test]
    fn special_case_rules() {
        assert_eq!(f("Straße"), "strasse");
        assert_eq!(f("STRASSE"), "strasse");
        assert_eq!(f("İstanbul"), "istanbul");
        assert_eq!(f("ΣΊΣΥΦΟΣ"), f("σίσυφος"));
        assert_eq!(f("Ｒｅｐｏｒｔ"), "report");
    }

    #[test]
    fn cjk_and_emoji_pass_through() {
        assert_eq!(f("日本語メモ"), "日本語メモ");
        assert_eq!(f("🎉 Party"), "🎉 party");
    }

    #[test]
    fn meaningful_marks_are_kept() {
        assert_ne!(f("ガ"), f("カ"));
        assert_ne!(f("कि"), f("क"));
    }

    /// The tables give exactly what the `unicode-normalization` crate gives.
    #[test]
    fn matches_reference_nfkd() {
        use unicode_normalization::UnicodeNormalization;
        let reference = |s: &str| -> Vec<u8> {
            let mut out = Vec::new();
            let mut buf = [0u8; 4];
            for c in s.nfkd() {
                if is_diacritic(c) {
                    continue;
                }
                for l in c.to_lowercase() {
                    match l {
                        'ß' => out.extend_from_slice(b"ss"),
                        'ς' => out.extend_from_slice('σ'.encode_utf8(&mut buf).as_bytes()),
                        l if is_diacritic(l) => {}
                        l => out.extend_from_slice(l.encode_utf8(&mut buf).as_bytes()),
                    }
                }
            }
            out
        };
        for cp in 0x80u32..=0x10FFFF {
            let Some(c) = char::from_u32(cp) else { continue };
            let s = format!("a{c}b");
            assert_eq!(fold(&s), reference(&s), "U+{cp:04X}");
        }
        // Runs of combining marks in every order, mixed with letters that decompose.
        let marks = ['\u{301}', '\u{316}', '\u{5B0}', '\u{93C}', '\u{94D}', '\u{3099}', '\u{20D2}', '\u{1AB0}', '\u{E3A}'];
        let bases = ['e', 'é', 'ǖ', 'क', 'か', '가', 'Å', 'ﬁ', 'Ǆ'];
        let mut x: u64 = 0x2545_F491_4F6C_DD1D;
        for _ in 0..20_000 {
            let mut s = String::new();
            for _ in 0..6 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                s.push(if x.is_multiple_of(3) { bases[(x >> 8) as usize % bases.len()] } else { marks[(x >> 8) as usize % marks.len()] });
            }
            assert_eq!(fold(&s), reference(&s), "{s:?}");
        }
    }

    #[test]
    fn invalid_utf8_is_kept() {
        let mut out = Vec::new();
        fold_into(b"BAD\xff\xfeName", &mut out);
        assert_eq!(out, b"bad\xff\xfename");
    }
}

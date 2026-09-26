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

use unicode_normalization::UnicodeNormalization;

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
    let mut buf = [0u8; 4];
    for c in s.nfkd() {
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

    #[test]
    fn invalid_utf8_is_kept() {
        let mut out = Vec::new();
        fold_into(b"BAD\xff\xfeName", &mut out);
        assert_eq!(out, b"bad\xff\xfename");
    }
}

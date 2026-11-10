//! Name patterns: EchoFiles word search, regex, glob and fixed strings, each either on the
//! name or on the full path, with fd's smart case.

use memchr::memmem::Finder;
use regex::bytes::Regex;
use regex_syntax::hir::{Hir, HirKind};

use crate::glob::{Glob, GlobOptions};
use crate::matcher::Matcher;
use crate::{Kind, Query, fold};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syntax {
    /// Whitespace-separated words that must all appear, ignoring case and accents;
    /// `"quoted phrases"`; `*` matches everything. Ranked best first.
    Words,
    Regex,
    Glob,
    /// The pattern as a literal substring.
    Fixed,
    /// The pattern as the whole name (or path), literally (fd's `--exact`).
    Exact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Case {
    /// Case-insensitive unless the pattern has an uppercase letter.
    #[default]
    Smart,
    Sensitive,
    Insensitive,
}

enum Engine {
    Words(Matcher),
    Regex(Regex),
    Glob(Glob),
    Fixed(Finder<'static>),
    Exact(Vec<u8>),
    /// The empty pattern: matches everything.
    All,
}

pub struct Pattern {
    engine: Engine,
    pub syntax: Syntax,
    pub full_path: bool,
    pub case_insensitive: bool,
    pub source: String,
    /// A lowercase ASCII literal every matching name contains (in its folded form too).
    prefilter: Option<Vec<u8>>,
}

/// fd's smart-case test: whether the pattern, read as the regex fd builds from it, has an
/// uppercase letter in a literal or a character class (`\S` and `\p{Lu}` don't count).
pub fn needs_case(pattern: &str, syntax: Syntax) -> bool {
    match syntax {
        Syntax::Regex => regex_syntax::ParserBuilder::new().utf8(false).build().parse(pattern).is_ok_and(|h| hir_upper(&h)),
        // Globs, literal text and words become literals (and classes) of their own characters.
        _ => pattern.chars().any(char::is_uppercase),
    }
}

fn hir_upper(h: &Hir) -> bool {
    use regex_syntax::hir::Class;
    match h.kind() {
        HirKind::Literal(l) => match std::str::from_utf8(&l.0) {
            Ok(s) => s.chars().any(char::is_uppercase),
            Err(_) => l.0.iter().any(|&b| char::from(b).is_uppercase()),
        },
        HirKind::Class(Class::Unicode(c)) => c.ranges().iter().any(|r| r.start().is_uppercase() || r.end().is_uppercase()),
        HirKind::Class(Class::Bytes(c)) => {
            c.ranges().iter().any(|r| char::from(r.start()).is_uppercase() || char::from(r.end()).is_uppercase())
        }
        HirKind::Capture(c) => hir_upper(&c.sub),
        HirKind::Repetition(r) => hir_upper(&r.sub),
        HirKind::Concat(v) | HirKind::Alternation(v) => v.iter().any(hir_upper),
        _ => false,
    }
}

pub fn is_regex_like(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '^' | '$' | '(' | ')' | '|' | '+' | '\\' | '{' | '}')) || s.contains(".*") || s.contains(".+")
}

pub fn is_glob_like(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '*' | '?' | '[')) && s.trim() != "*"
}

/// The longest literal run every match of `hir` must contain.
fn required_literal(hir: &Hir) -> Option<Vec<u8>> {
    match hir.kind() {
        HirKind::Literal(l) => Some(l.0.to_vec()),
        HirKind::Capture(c) => required_literal(&c.sub),
        HirKind::Repetition(r) if r.min >= 1 => required_literal(&r.sub),
        HirKind::Concat(parts) => {
            let mut best: Option<Vec<u8>> = None;
            let mut run: Vec<u8> = Vec::new();
            let consider = |v: Vec<u8>, best: &mut Option<Vec<u8>>| {
                if best.as_ref().is_none_or(|b| v.len() > b.len()) {
                    *best = Some(v);
                }
            };
            for p in parts {
                if let HirKind::Literal(l) = p.kind() {
                    run.extend_from_slice(&l.0);
                    continue;
                }
                if !run.is_empty() {
                    consider(std::mem::take(&mut run), &mut best);
                }
                if let Some(v) = required_literal(p) {
                    consider(v, &mut best);
                }
            }
            if !run.is_empty() {
                consider(run, &mut best);
            }
            best
        }
        _ => None,
    }
}

impl Pattern {
    pub fn new(source: &str, syntax: Syntax, case: Case, full_path: bool) -> Result<Pattern, String> {
        let ci = match case {
            Case::Sensitive => false,
            Case::Insensitive => true,
            Case::Smart => !needs_case(source, syntax),
        };
        let lit_ok = |l: &[u8]| l.is_ascii() && l.len() >= 2;
        let (engine, prefilter) = if source.is_empty() {
            (Engine::All, None)
        } else {
            match syntax {
                Syntax::Words => {
                    let m = Matcher::new(&Query::new(source)).ok_or_else(|| "the pattern has no words to search for".to_string())?;
                    let lit = m.scan_finder().needle().to_vec();
                    // Folded needle: valid against the folded arena even when it isn't ASCII.
                    let pre = (!lit.is_empty() && !full_path).then_some(lit);
                    (Engine::Words(m), pre)
                }
                Syntax::Regex => {
                    let re = regex::bytes::RegexBuilder::new(source)
                        .case_insensitive(ci)
                        .unicode(true)
                        .build()
                        .map_err(|e| format!("invalid regex: {e}"))?;
                    let lit = regex_syntax::ParserBuilder::new()
                        .utf8(false)
                        .build()
                        .parse(source)
                        .ok()
                        .and_then(|h| required_literal(&h))
                        .map(|l| l.to_ascii_lowercase());
                    (Engine::Regex(re), lit.filter(|l| lit_ok(l) && !l.contains(&b'/') && !full_path))
                }
                Syntax::Glob => {
                    let g = Glob::new(source, GlobOptions { case_insensitive: ci, braces: true, nonempty_trailing_recursive: true })
                        .map_err(|e| format!("invalid glob: {e}"))?;
                    let lit = g.required_literal().map(|l| l.to_ascii_lowercase());
                    (Engine::Glob(g), lit.filter(|l| lit_ok(l) && !full_path))
                }
                Syntax::Exact => {
                    let needle = if ci { source.to_lowercase().into_bytes() } else { source.as_bytes().to_vec() };
                    let pre = Some(needle.to_ascii_lowercase()).filter(|l| lit_ok(l) && !full_path);
                    (Engine::Exact(needle), pre)
                }
                Syntax::Fixed => {
                    let needle = if ci { source.to_lowercase().into_bytes() } else { source.as_bytes().to_vec() };
                    let pre = Some(needle.to_ascii_lowercase()).filter(|l| lit_ok(l) && !full_path);
                    (Engine::Fixed(Finder::new(&needle).into_owned()), pre)
                }
            }
        };
        Ok(Pattern { engine, syntax, full_path, case_insensitive: ci, source: source.to_string(), prefilter })
    }

    pub fn matches_all(&self) -> bool {
        matches!(self.engine, Engine::All) || matches!(&self.engine, Engine::Words(_) if self.source.trim() == "*")
    }

    /// A lowercase ASCII literal every matching *name* contains; it is also contained in
    /// the folded form of the name, so it can drive a scan of the folded name arena.
    pub fn prefilter(&self) -> Option<&[u8]> {
        self.prefilter.as_deref()
    }

    /// `subject` is the name, or the absolute path when [`full_path`](Pattern::full_path).
    /// `folded` is the folded subject if the caller has it already.
    pub fn is_match(&self, subject: &[u8], folded: Option<&[u8]>) -> bool {
        match &self.engine {
            Engine::All => true,
            Engine::Words(m) => {
                let owned;
                let f = match folded {
                    Some(f) => f,
                    None => {
                        owned = fold_bytes(subject);
                        &owned
                    }
                };
                m.accepts(f, Kind::File)
            }
            Engine::Regex(r) => r.is_match(subject),
            Engine::Glob(g) => g.matches(subject),
            Engine::Exact(n) => {
                if self.case_insensitive {
                    lowercase(subject) == *n
                } else {
                    subject == n.as_slice()
                }
            }
            Engine::Fixed(f) => {
                if self.case_insensitive {
                    f.find(&lowercase(subject)).is_some()
                } else {
                    f.find(subject).is_some()
                }
            }
        }
    }

    /// Rank key for a hit's name (smaller is better): match class in the top byte (exact,
    /// prefix, word start, anywhere), then name length.
    pub fn rank(&self, name: &[u8], folded: Option<&[u8]>) -> u64 {
        let len = (name.len().min(0xFF_FFFF) as u64) << 32;
        match &self.engine {
            Engine::Words(m) => match folded {
                Some(f) => m.rank(f),
                None => m.rank(&fold_bytes(name)),
            },
            Engine::All => 3 << 56 | len,
            _ => {
                let pos = match &self.engine {
                    Engine::Regex(r) if !self.full_path => r.find(name).map(|m| (m.start(), m.end())),
                    Engine::Fixed(f) if !self.full_path => {
                        let hay = if self.case_insensitive { lowercase(name) } else { name.to_vec() };
                        f.find(&hay).map(|s| (s, s + f.needle().len()))
                    }
                    _ => None,
                };
                let class = match pos {
                    Some((0, e)) if e == name.len() => 0,
                    Some((0, _)) => 1,
                    Some((s, _)) if !name[s - 1].is_ascii_alphanumeric() => 2,
                    _ => 3,
                };
                class << 56 | len
            }
        }
    }
}

fn fold_bytes(b: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(b.len());
    fold::fold_into(b, &mut v);
    v
}

fn lowercase(b: &[u8]) -> Vec<u8> {
    if b.is_ascii() {
        return b.to_ascii_lowercase();
    }
    String::from_utf8_lossy(b).to_lowercase().into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_case_and_syntaxes() {
        let p = Pattern::new("bench.*rs", Syntax::Regex, Case::Smart, false).unwrap();
        assert!(p.is_match(b"Bench_x.rs", None));
        assert_eq!(p.prefilter(), Some(&b"bench"[..]));
        let p = Pattern::new("Bench", Syntax::Regex, Case::Smart, false).unwrap();
        assert!(!p.is_match(b"bench", None));
        let p = Pattern::new(r"\Sx", Syntax::Regex, Case::Smart, false).unwrap();
        assert!(p.case_insensitive);
        let p = Pattern::new("test_*.py", Syntax::Glob, Case::Smart, false).unwrap();
        assert!(p.is_match(b"TEST_a.py", None));
        assert!(!p.is_match(b"xtest_a.py", None));
        let p = Pattern::new("résumé", Syntax::Words, Case::Smart, false).unwrap();
        assert!(p.is_match("Resume final.pdf".as_bytes(), None));
        let p = Pattern::new("a.b", Syntax::Fixed, Case::Smart, false).unwrap();
        assert!(p.is_match(b"xA.Bx", None));
        assert!(!p.is_match(b"axb", None));
        let p = Pattern::new("", Syntax::Regex, Case::Smart, false).unwrap();
        assert!(p.matches_all());
    }

    #[test]
    fn literals() {
        let lit = |s: &str| {
            Pattern::new(s, Syntax::Regex, Case::Smart, false).unwrap().prefilter().map(|l| String::from_utf8(l.to_vec()).unwrap())
        };
        assert_eq!(lit("^foo(bar)+baz$").as_deref(), Some("foo"));
        assert_eq!(lit("a|b"), None);
        assert_eq!(lit("x?yy"), Some("yy".into()));
        assert_eq!(lit("Cargo\\.toml").as_deref(), Some("cargo.toml"));
    }

    #[test]
    fn ranks() {
        let p = Pattern::new("main", Syntax::Regex, Case::Smart, false).unwrap();
        assert!(p.rank(b"main", None) < p.rank(b"main.rs", None));
        assert!(p.rank(b"main.rs", None) < p.rank(b"x_main.rs", None));
        assert!(p.rank(b"x_main.rs", None) < p.rank(b"domain.rs", None));
    }
}

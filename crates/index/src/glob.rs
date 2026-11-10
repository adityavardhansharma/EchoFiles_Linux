//! Glob patterns, matched without a regex engine.
//!
//! Syntax: `*` (any run within one path component), `?` (one character), `[abc]`, `[a-z]`,
//! `[!x]`/`[^x]`, `\` escapes, `**` as a whole component (any number of components), and —
//! for `fd --glob` style patterns, not gitignore — `{a,b}` alternatives.
//!
//! A pattern is split into components on `/` and matched against the components of a path
//! with the classic "back up to the last star" algorithm, used twice: once over components
//! (`**` is the star) and once over the characters of one component (`*` is the star). Each
//! is linear in practice and never exponential. Common shapes skip all of that: a literal
//! name is a comparison, `*.rs` an `ends_with`, `foo*` a `starts_with`, `*foo*` a substring
//! search.

/// One character position: a Unicode scalar value, or a byte that isn't valid UTF-8
/// (`0x110000 + byte`, so it can only match itself).
type Unit = u32;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tok {
    Lit(Unit),
    Any,
    Star,
    Class { negated: bool, ranges: Vec<(Unit, Unit)> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Comp {
    /// `**`: zero or more whole components.
    Recursive,
    Segment(Vec<Tok>),
}

#[derive(Clone, Debug)]
enum Fast {
    /// No shortcut; run the general matcher.
    None,
    /// The whole (single-component) pattern is this literal.
    Exact(Vec<u8>),
    Suffix(Vec<u8>),
    Prefix(Vec<u8>),
    Contains(Box<memchr::memmem::Finder<'static>>),
    /// `*`: anything (within one component).
    All,
}

/// A compiled glob. Clone is cheap enough to keep one per rule.
#[derive(Clone, Debug)]
pub struct Glob {
    alts: Vec<Alt>,
    case_insensitive: bool,
    /// The pattern's text as written (for messages and `ef why`).
    pub source: String,
}

#[derive(Clone, Debug)]
struct Alt {
    comps: Vec<Comp>,
    fast: Fast,
    /// A trailing `/**` must match at least one component (gitignore: "everything inside").
    nonempty_tail: bool,
}

/// How a pattern is read.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlobOptions {
    pub case_insensitive: bool,
    /// `{a,b}` alternatives (fd's `--glob`, not gitignore).
    pub braces: bool,
    /// gitignore: `dir/**` doesn't match `dir` itself.
    pub nonempty_trailing_recursive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobError(pub String);

impl std::fmt::Display for GlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GlobError {}

/// Splits bytes into [`Unit`]s.
fn units(s: &[u8], lower: bool, out: &mut Vec<Unit>) {
    out.clear();
    for chunk in s.utf8_chunks() {
        for c in chunk.valid().chars() {
            if lower {
                if c.is_ascii() {
                    out.push(c.to_ascii_lowercase() as Unit);
                } else {
                    out.extend(c.to_lowercase().map(|l| l as Unit));
                }
            } else {
                out.push(c as Unit);
            }
        }
        out.extend(chunk.invalid().iter().map(|&b| 0x110000 + b as Unit));
    }
}

fn lower_bytes(s: &[u8]) -> Vec<u8> {
    if s.is_ascii() {
        return s.to_ascii_lowercase();
    }
    let mut out = Vec::with_capacity(s.len());
    for chunk in s.utf8_chunks() {
        let mut buf = [0u8; 4];
        for c in chunk.valid().chars() {
            for l in c.to_lowercase() {
                out.extend_from_slice(l.encode_utf8(&mut buf).as_bytes());
            }
        }
        out.extend_from_slice(chunk.invalid());
    }
    out
}

/// Expand `{a,b}` (nested allowed) into plain alternatives. Escaped braces stay literal.
fn expand_braces(p: &[u8]) -> Result<Vec<Vec<u8>>, GlobError> {
    // Find the first top-level unescaped '{' and its matching '}'.
    let mut i = 0;
    let mut in_class = false;
    while i < p.len() {
        match p[i] {
            b'\\' => i += 1,
            b'[' if !in_class => in_class = true,
            b']' if in_class => in_class = false,
            b'{' if !in_class => {
                let mut depth = 0;
                let mut j = i;
                let mut parts = Vec::new();
                let mut start = i + 1;
                let mut cls = false;
                while j < p.len() {
                    match p[j] {
                        b'\\' => j += 1,
                        b'[' if !cls => cls = true,
                        b']' if cls => cls = false,
                        b'{' if !cls => depth += 1,
                        b'}' if !cls => {
                            depth -= 1;
                            if depth == 0 {
                                parts.push(&p[start..j]);
                                break;
                            }
                        }
                        b',' if !cls && depth == 1 => {
                            parts.push(&p[start..j]);
                            start = j + 1;
                        }
                        _ => {}
                    }
                    j += 1;
                }
                if j >= p.len() {
                    return Err(GlobError("unclosed '{' in glob".into()));
                }
                let (head, tail) = (&p[..i], &p[j + 1..]);
                let mut out = Vec::new();
                for part in parts {
                    for rest in expand_braces(&[part, tail].concat())? {
                        out.push([head, &rest].concat());
                        if out.len() > 4096 {
                            return Err(GlobError("glob has too many '{…}' combinations".into()));
                        }
                    }
                }
                return Ok(out);
            }
            _ => {}
        }
        i += 1;
    }
    Ok(vec![p.to_vec()])
}

fn parse_segment(seg: &[u8], lower: bool) -> Result<Vec<Tok>, GlobError> {
    let mut u = Vec::new();
    units(seg, false, &mut u);
    let low = |c: Unit| -> Unit {
        if !lower {
            return c;
        }
        match char::from_u32(c) {
            Some(ch) => {
                let mut l = ch.to_lowercase();
                // Characters whose lowercase is several characters only match themselves.
                if l.len() == 1 { l.next().unwrap() as Unit } else { c }
            }
            None => c,
        }
    };
    let mut toks = Vec::new();
    let mut i = 0;
    while i < u.len() {
        let c = u[i];
        match char::from_u32(c) {
            Some('*') => {
                if toks.last() != Some(&Tok::Star) {
                    toks.push(Tok::Star);
                }
            }
            Some('?') => toks.push(Tok::Any),
            Some('\\') => {
                i += 1;
                // A trailing backslash matches a backslash.
                toks.push(Tok::Lit(low(*u.get(i).unwrap_or(&('\\' as Unit)))));
            }
            Some('[') => {
                let mut j = i + 1;
                let negated = matches!(u.get(j).and_then(|&c| char::from_u32(c)), Some('!' | '^'));
                if negated {
                    j += 1;
                }
                let mut ranges = Vec::new();
                let mut first = true;
                let mut closed = false;
                while j < u.len() {
                    let mut c = u[j];
                    if c == ']' as Unit && !first {
                        closed = true;
                        break;
                    }
                    first = false;
                    if c == '\\' as Unit && j + 1 < u.len() {
                        j += 1;
                        c = u[j];
                    }
                    if j + 2 < u.len() && u[j + 1] == '-' as Unit && u[j + 2] != ']' as Unit {
                        let mut hi = u[j + 2];
                        let mut skip = 2;
                        if hi == '\\' as Unit && j + 3 < u.len() {
                            hi = u[j + 3];
                            skip = 3;
                        }
                        if hi < c {
                            return Err(GlobError(format!("invalid range in glob: {}-{}", fmt_unit(c), fmt_unit(hi))));
                        }
                        ranges.push((c, hi));
                        j += skip + 1;
                    } else {
                        ranges.push((c, c));
                        j += 1;
                    }
                }
                if !closed {
                    return Err(GlobError("unclosed '[' in glob".into()));
                }
                if lower {
                    // Add the lowercase form of every ASCII letter range.
                    let extra: Vec<(Unit, Unit)> = ranges
                        .iter()
                        .filter_map(|&(a, b)| {
                            let (lo, hi) = (a.max('A' as Unit), b.min('Z' as Unit));
                            (lo <= hi).then(|| (lo + 32, hi + 32))
                        })
                        .collect();
                    ranges.extend(extra);
                    for r in ranges.iter_mut() {
                        if r.0 == r.1 {
                            let l = low(r.0);
                            *r = (l, l);
                        }
                    }
                }
                toks.push(Tok::Class { negated, ranges });
                i = j;
            }
            _ => toks.push(Tok::Lit(low(c))),
        }
        i += 1;
    }
    Ok(toks)
}

fn fmt_unit(c: Unit) -> String {
    char::from_u32(c).map_or_else(|| format!("\\x{:02x}", c - 0x110000), String::from)
}

fn lit_bytes(toks: &[Tok]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for t in toks {
        let Tok::Lit(c) = t else { return None };
        match char::from_u32(*c) {
            Some(ch) => out.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes()),
            None => out.push((*c - 0x110000) as u8),
        }
    }
    Some(out)
}

fn fast_path(comps: &[Comp]) -> Fast {
    let [Comp::Segment(t)] = comps else { return Fast::None };
    match t.as_slice() {
        [Tok::Star] => Fast::All,
        [Tok::Star, rest @ .., Tok::Star] if !rest.is_empty() => match lit_bytes(rest) {
            Some(l) => Fast::Contains(Box::new(memchr::memmem::Finder::new(&l).into_owned())),
            None => Fast::None,
        },
        [Tok::Star, rest @ ..] => lit_bytes(rest).map_or(Fast::None, Fast::Suffix),
        [rest @ .., Tok::Star] => lit_bytes(rest).map_or(Fast::None, Fast::Prefix),
        all => lit_bytes(all).map_or(Fast::None, Fast::Exact),
    }
}

impl Glob {
    pub fn new(pattern: &str, opts: GlobOptions) -> Result<Glob, GlobError> {
        Glob::from_bytes(pattern.as_bytes(), opts)
    }

    pub fn from_bytes(pattern: &[u8], opts: GlobOptions) -> Result<Glob, GlobError> {
        let expanded = if opts.braces { expand_braces(pattern)? } else { vec![pattern.to_vec()] };
        let mut alts = Vec::new();
        for p in expanded {
            let mut comps = Vec::new();
            for seg in split_unescaped(&p) {
                if seg == b"**" {
                    if comps.last() != Some(&Comp::Recursive) {
                        comps.push(Comp::Recursive);
                    }
                } else {
                    comps.push(Comp::Segment(parse_segment(seg, opts.case_insensitive)?));
                }
            }
            let nonempty_tail = opts.nonempty_trailing_recursive && comps.len() > 1 && comps.last() == Some(&Comp::Recursive);
            alts.push(Alt { fast: fast_path(&comps), comps, nonempty_tail });
        }
        Ok(Glob { alts, case_insensitive: opts.case_insensitive, source: String::from_utf8_lossy(pattern).into_owned() })
    }

    /// Whether the pattern has a `/` in it, i.e. is matched against paths, not names.
    pub fn has_separator(&self) -> bool {
        self.alts.iter().any(|a| a.comps.len() > 1)
    }

    /// Match a name or a `/`-separated path.
    pub fn matches(&self, path: &[u8]) -> bool {
        let lowered;
        let path = if self.case_insensitive && self.alts.iter().any(|a| !matches!(a.fast, Fast::None | Fast::All)) {
            lowered = lower_bytes(path);
            lowered.as_slice()
        } else {
            path
        };
        let mut scratch = Scratch::default();
        self.alts.iter().any(|a| a.matches(path, self.case_insensitive, &mut scratch))
    }

    /// A literal every match must contain (for prefiltering), lowercase if the glob is
    /// case-insensitive. `None` if there is no useful one.
    pub fn required_literal(&self) -> Option<Vec<u8>> {
        if self.alts.len() != 1 {
            return None;
        }
        let mut best: Vec<u8> = Vec::new();
        for c in &self.alts[0].comps {
            let Comp::Segment(toks) = c else { continue };
            let mut run = Vec::new();
            for t in toks.iter().chain(std::iter::once(&Tok::Star)) {
                match t {
                    Tok::Lit(u) => match char::from_u32(*u) {
                        Some(ch) => run.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes()),
                        None => run.push((*u - 0x110000) as u8),
                    },
                    _ => {
                        if run.len() > best.len() {
                            best = std::mem::take(&mut run);
                        }
                        run.clear();
                    }
                }
            }
        }
        (!best.is_empty()).then_some(best)
    }
}

/// Split on unescaped `/` (a `/` inside `[...]` still splits, as in git).
fn split_unescaped(p: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < p.len() {
        match p[i] {
            b'\\' => i += 1,
            b'/' => {
                out.push(&p[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&p[start..]);
    out
}

#[derive(Default)]
struct Scratch {
    units: Vec<Unit>,
}

impl Alt {
    fn matches(&self, path: &[u8], ci: bool, s: &mut Scratch) -> bool {
        match &self.fast {
            Fast::All => return !path.contains(&b'/'),
            Fast::Exact(l) => return path == l.as_slice(),
            Fast::Suffix(l) => return path.ends_with(l) && !path[..path.len() - l.len()].contains(&b'/'),
            Fast::Prefix(l) => return path.starts_with(l) && !path[l.len()..].contains(&b'/'),
            Fast::Contains(f) => return !path.contains(&b'/') && f.find(path).is_some(),
            Fast::None => {}
        }
        let parts: Vec<&[u8]> = path.split(|&b| b == b'/').collect();
        let (mut pi, mut si) = (0usize, 0usize);
        let mut star: Option<(usize, usize)> = None;
        let comps = &self.comps;
        while si < parts.len() {
            if pi < comps.len() {
                match &comps[pi] {
                    Comp::Recursive => {
                        star = Some((pi, si));
                        pi += 1;
                        continue;
                    }
                    Comp::Segment(t) => {
                        if seg_match(t, parts[si], ci, s) {
                            pi += 1;
                            si += 1;
                            continue;
                        }
                    }
                }
            }
            match star {
                Some((sp, ss)) => {
                    pi = sp + 1;
                    si = ss + 1;
                    star = Some((sp, ss + 1));
                }
                None => return false,
            }
        }
        if self.nonempty_tail && pi + 1 == comps.len() && comps[pi] == Comp::Recursive {
            // `dir/**` reached the end of the path exactly at `dir`: nothing inside matched.
            // (Matching is greedy, so the tail only ends up empty when it has to.)
            return false;
        }
        while pi < comps.len() && comps[pi] == Comp::Recursive {
            pi += 1;
        }
        pi == comps.len()
    }
}

fn seg_match(toks: &[Tok], name: &[u8], ci: bool, s: &mut Scratch) -> bool {
    // Pure-literal segments compare bytes directly.
    if toks.iter().all(|t| matches!(t, Tok::Lit(c) if *c < 0x80)) {
        return toks.len() == name.len()
            && toks.iter().zip(name).all(|(t, &b)| matches!(t, Tok::Lit(c) if *c == if ci { b.to_ascii_lowercase() } else { b } as Unit));
    }
    units(name, ci, &mut s.units);
    let u = &s.units;
    let (mut ti, mut ui) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while ui < u.len() {
        if ti < toks.len() {
            match &toks[ti] {
                Tok::Star => {
                    star = Some((ti, ui));
                    ti += 1;
                    continue;
                }
                Tok::Any => {
                    ti += 1;
                    ui += 1;
                    continue;
                }
                Tok::Lit(c) if *c == u[ui] => {
                    ti += 1;
                    ui += 1;
                    continue;
                }
                Tok::Class { negated, ranges } if ranges.iter().any(|&(a, b)| a <= u[ui] && u[ui] <= b) != *negated => {
                    ti += 1;
                    ui += 1;
                    continue;
                }
                _ => {}
            }
        }
        match star {
            Some((st, su)) => {
                ti = st + 1;
                ui = su + 1;
                star = Some((st, su + 1));
            }
            None => return false,
        }
    }
    while ti < toks.len() && toks[ti] == Tok::Star {
        ti += 1;
    }
    ti == toks.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(p: &str) -> Glob {
        Glob::new(p, GlobOptions { braces: true, ..Default::default() }).unwrap()
    }

    fn gi(p: &str) -> Glob {
        Glob::new(p, GlobOptions { braces: true, case_insensitive: true, ..Default::default() }).unwrap()
    }

    #[test]
    fn basics() {
        assert!(g("*.rs").matches(b"main.rs"));
        assert!(!g("*.rs").matches(b"main.rsx"));
        assert!(!g("*.rs").matches(b"src/main.rs"));
        assert!(g("test_*.py").matches(b"test_a.py"));
        assert!(!g("test_*.py").matches(b"atest_a.py"));
        assert!(g("a?c").matches(b"abc"));
        assert!(g("a?c").matches("aéc".as_bytes()));
        assert!(!g("a?c").matches(b"ac"));
        assert!(g("[a-c]x").matches(b"bx"));
        assert!(!g("[!a-c]x").matches(b"bx"));
        assert!(g("[^a-c]x").matches(b"dx"));
        assert!(g("[]]").matches(b"]"));
        assert!(g("\\*").matches(b"*"));
        assert!(!g("\\*").matches(b"a"));
        assert!(g("*a*b*c*").matches(b"xxaxxbxxcxx"));
        assert!(!g("*a*b*c*").matches(b"xxcxxbxxaxx"));
        assert!(g("*").matches(b""));
    }

    #[test]
    fn braces_and_case() {
        assert!(g("*.{rs,toml}").matches(b"Cargo.toml"));
        assert!(g("{a,b{c,d}}x").matches(b"bdx"));
        assert!(!g("*.{rs,toml}").matches(b"x.md"));
        assert!(gi("*.RS").matches(b"MAIN.rs"));
        assert!(gi("[A-C]*").matches(b"bob"));
        assert!(gi("résumé*").matches("RÉSUMÉ.pdf".as_bytes()));
        assert!(!g("*.RS").matches(b"main.rs"));
    }

    #[test]
    fn recursive() {
        assert!(g("src/**/*.rs").matches(b"src/main.rs"));
        assert!(g("src/**/*.rs").matches(b"src/a/b/main.rs"));
        assert!(!g("src/**/*.rs").matches(b"lib/a/main.rs"));
        assert!(g("**/target").matches(b"target"));
        assert!(g("**/target").matches(b"a/b/target"));
        assert!(g("a/**").matches(b"a/b/c"));
        assert!(g("**").matches(b"a/b/c"));
        assert!(g("/home/**/x").matches(b"/home/me/x"));
        let git = Glob::new("a/**", GlobOptions { nonempty_trailing_recursive: true, ..Default::default() }).unwrap();
        assert!(git.matches(b"a/b"));
        assert!(!git.matches(b"a"));
        assert!(g("a/**/b/**/c").matches(b"a/x/b/y/z/c"));
        assert!(!g("a/**/b/**/c").matches(b"a/x/y/z/c"));
    }

    #[test]
    fn non_utf8_and_literals() {
        assert!(g("bad*").matches(b"bad\xff\xfe"));
        assert!(g("?").matches(b"\xff"));
        assert_eq!(g("*report*.pdf").required_literal().unwrap(), b"report");
    }
}

//! Turning hits into output: fd-style paths (relative, `./`, trailing `/`), colours from
//! `LS_COLORS`, hyperlinks, `--format`, `-0`, and EchoFiles' JSON, budget and summary.

use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;
use std::io::Write;
use std::path::Path;

use crate::filter::{FType, Hit};

/// How one search root's hits are shown.
pub struct RootView {
    /// The absolute path walked (hit paths start with it).
    pub walk: Vec<u8>,
    /// What replaces it in output (`src`, `./src`, `/abs/src`, or empty for the cwd).
    pub shown: Vec<u8>,
}

pub struct Style {
    pub trailing_slash: bool,
    pub separator: Option<String>,
    pub color: Option<LsColors>,
    pub hyperlink: Option<String>,
    pub format: Option<Template>,
    /// Output goes to a terminal: escape control characters in names (as fd does).
    pub tty: bool,
}

/// Display path of a hit (without colour), e.g. `src/main.rs`.
pub fn display(h: &Hit, roots: &[RootView], slash: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(h.path.len() + 2);
    display_into(h, roots, slash, &mut out);
    out
}

/// [`display`], appended to `out`.
pub fn display_into(h: &Hit, roots: &[RootView], slash: bool, out: &mut Vec<u8>) {
    // Phone results (and anything else from outside the roots) print as they are.
    let Some(r) = roots.get(h.root as usize) else {
        out.extend_from_slice(&h.path);
        return;
    };
    let rel = h.path.strip_prefix(r.walk.as_slice()).unwrap_or(&h.path);
    let rel = rel.strip_prefix(b"/").unwrap_or(rel);
    let start = out.len();
    if !r.shown.is_empty() {
        out.extend_from_slice(&r.shown);
        if out.last() != Some(&b'/') && !rel.is_empty() {
            out.push(b'/');
        }
    }
    out.extend_from_slice(rel);
    if slash && h.ftype == FType::Dir && out.len() > start && out.last() != Some(&b'/') {
        out.push(b'/');
    }
}

// ---------------------------------------------------------------------------------------
// Colours
// ---------------------------------------------------------------------------------------

/// One `LS_COLORS` style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
struct Sgr {
    fg: Option<Color>,
    bg: Option<Color>,
    /// Underline colour (`58;…`): parsed and compared, never printed (as in fd).
    ul: Option<Color>,
    bold: bool,
    dimmed: bool,
    italic: bool,
    underline: bool,
    blink: bool,
    reverse: bool,
    hidden: bool,
    strike: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Color {
    /// 30–37 / 40–47.
    Basic(u8),
    Fixed(u8),
    Rgb(u8, u8, u8),
}

impl Sgr {
    /// An SGR sequence like `01;38;5;81`. `None` for `0`, empty, or a part that isn't a
    /// number from 0 to 255 (which disables the entry, as in GNU ls and fd).
    fn parse(code: &str) -> Option<Sgr> {
        if code.is_empty() || code == "0" || code == "00" {
            return None;
        }
        let mut parts = std::collections::VecDeque::new();
        for c in code.split(';') {
            parts.push_back(c.parse::<u8>().ok()?);
        }
        let mut s = Sgr::default();
        let color = |parts: &mut std::collections::VecDeque<u8>| -> Option<Color> {
            match (parts.pop_front(), parts.pop_front()) {
                (Some(5), Some(n)) => Some(Color::Fixed(n)),
                (Some(2), Some(r)) => match (parts.pop_front(), parts.pop_front()) {
                    (Some(g), Some(b)) => Some(Color::Rgb(r, g, b)),
                    _ => None,
                },
                _ => None,
            }
        };
        while let Some(p) = parts.pop_front() {
            match p {
                0 => {
                    let (fg, bg, ul) = (s.fg, s.bg, s.ul);
                    s = Sgr { fg, bg, ul, ..Sgr::default() };
                }
                1 => s.bold = true,
                2 => s.dimmed = true,
                3 => s.italic = true,
                4 => s.underline = true,
                5 | 6 => s.blink = true,
                7 => s.reverse = true,
                8 => s.hidden = true,
                9 => s.strike = true,
                22 => {
                    s.bold = false;
                    s.dimmed = false;
                }
                23 => s.italic = false,
                24 => s.underline = false,
                25 => s.blink = false,
                27 => s.reverse = false,
                28 => s.hidden = false,
                29 => s.strike = false,
                30..=37 => s.fg = Some(Color::Basic(p - 30)),
                38 => match color(&mut parts) {
                    Some(c) => s.fg = Some(c),
                    None => break,
                },
                39 => s.fg = None,
                40..=47 => s.bg = Some(Color::Basic(p - 40)),
                48 => match color(&mut parts) {
                    Some(c) => s.bg = Some(c),
                    None => break,
                },
                49 => s.bg = None,
                58 => match color(&mut parts) {
                    Some(c) => s.ul = Some(c),
                    None => break,
                },
                59 => s.ul = None,
                // Bright colours become their 256-colour equivalents.
                90..=97 => s.fg = Some(Color::Fixed(p - 90 + 8)),
                100..=107 => s.bg = Some(Color::Fixed(p - 100 + 8)),
                _ => {}
            }
        }
        Some(s)
    }

    fn plain(&self) -> bool {
        self.fg.is_none() && self.bg.is_none() && !(self.bold || self.dimmed || self.italic || self.underline || self.blink || self.reverse || self.hidden || self.strike)
    }

    /// `text` wrapped in this style's codes (attributes, then background, then
    /// foreground, then `ESC[0m`); a plain style writes the text alone.
    fn paint(&self, text: &[u8], out: &mut Vec<u8>) {
        if self.plain() {
            out.extend_from_slice(text);
            return;
        }
        let mut codes: Vec<String> = Vec::new();
        for (on, c) in [(self.bold, "1"), (self.dimmed, "2"), (self.italic, "3"), (self.underline, "4"), (self.blink, "5"), (self.reverse, "7"), (self.hidden, "8"), (self.strike, "9")] {
            if on {
                codes.push(c.into());
            }
        }
        let color = |c: Color, base: u8, ext: u8| match c {
            Color::Basic(n) => format!("{}", base + n),
            Color::Fixed(n) => format!("{ext};5;{n}"),
            Color::Rgb(r, g, b) => format!("{ext};2;{r};{g};{b}"),
        };
        if let Some(bg) = self.bg {
            codes.push(color(bg, 40, 48));
        }
        if let Some(fg) = self.fg {
            codes.push(color(fg, 30, 38));
        }
        out.extend_from_slice(b"\x1b[");
        out.extend_from_slice(codes.join(";").as_bytes());
        out.push(b'm');
        out.extend_from_slice(text);
        out.extend_from_slice(b"\x1b[0m");
    }
}

/// fd's palette for when `LS_COLORS` isn't set (copied from fd 10.5, MIT/Apache-2.0),
/// byte for byte: its stray newlines disable its first and last entries, as in fd.
const FD_DEFAULT: &str = include_str!("fd_ls_colors.txt");

/// What `lscolors` starts from before reading a palette.
const BASE: &str = "rs=0:lc=\x1b[:rc=m:cl=\x1b[K:ex=01;32:sg=30;43:su=37;41:di=01;34:st=37;44:ow=34;42:tw=30;42:ln=01;36:bd=01;33:cd=01;33:do=01;35:pi=33:so=01;35:";

const INDICATORS: [&str; 24] = ["no", "fi", "di", "ln", "pi", "so", "do", "bd", "cd", "or", "su", "sg", "st", "ow", "tw", "ex", "mi", "ca", "mh", "lc", "rc", "ec", "rs", "cl"];

/// `LS_COLORS` colouring, matching fd (which uses the `lscolors` crate): file-type
/// indicators with ls's fallbacks, and suffix rules where later entries win and matching
/// ignores ASCII case unless two spellings of one suffix have different styles.
pub struct LsColors {
    indicators: HashMap<&'static str, Sgr>,
    /// `fi` falls back to `no` unless `fi` was set to nothing.
    file_normal_fallback: bool,
    /// (suffix, style, case-sensitive), latest entry first.
    suffixes: Vec<(Vec<u8>, Option<Sgr>, bool)>,
}

impl LsColors {
    /// The palette from `LS_COLORS` (`spec`), or fd's when it isn't set.
    pub fn parse(spec: Option<&str>) -> LsColors {
        let mut c = LsColors { indicators: HashMap::new(), file_normal_fallback: true, suffixes: Vec::new() };
        c.add(BASE);
        c.add(spec.unwrap_or(FD_DEFAULT));
        c.finish();
        c
    }

    fn add(&mut self, input: &str) {
        for entry in input.split(':') {
            let mut parts = entry.split('=');
            let (Some(key), Some(code)) = (parts.next(), parts.next()) else { continue };
            let style = Sgr::parse(code);
            if let Some(suffix) = key.strip_prefix('*') {
                self.suffixes.push((suffix.as_bytes().to_vec(), style, false));
            } else if let Some(k) = INDICATORS.iter().find(|&&k| k == key) {
                match style {
                    Some(st) => {
                        self.indicators.insert(k, st);
                    }
                    None => {
                        self.indicators.remove(k);
                        if *k == "fi" {
                            self.file_normal_fallback = false;
                        }
                    }
                }
            }
        }
    }

    /// Latest first, and mark suffixes whose spellings disagree as case-sensitive.
    fn finish(&mut self) {
        self.suffixes.reverse();
        let lower: Vec<Vec<u8>> = self.suffixes.iter().map(|s| s.0.to_ascii_lowercase()).collect();
        let first_exact = |key: &[u8]| self.suffixes.iter().position(|s| s.0 == key).unwrap();
        let first_lower = |key: &[u8]| lower.iter().position(|l| l == key).unwrap();
        let mut sensitive: Vec<Vec<u8>> = Vec::new();
        for (i, s) in self.suffixes.iter().enumerate() {
            if first_exact(&s.0) != i {
                continue;
            }
            let j = first_lower(&lower[i]);
            if self.suffixes[i].1 != self.suffixes[j].1 && !sensitive.contains(&lower[i]) {
                sensitive.push(lower[i].clone());
            }
        }
        for (i, s) in self.suffixes.iter_mut().enumerate() {
            s.2 = sensitive.contains(&lower[i]);
        }
    }

    fn suffix_style(&self, name: &[u8]) -> Option<&Sgr> {
        let exact = self.suffixes.iter().position(|(sfx, _, _)| name.ends_with(sfx));
        let folded = self.suffixes.iter().position(|(sfx, _, cs)| !cs && name.len() >= sfx.len() && name[name.len() - sfx.len()..].eq_ignore_ascii_case(sfx));
        let i = match (exact, folded) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => return None,
        };
        self.suffixes[i].1.as_ref()
    }

    fn indicator(&self, key: &str) -> Option<&Sgr> {
        let fallback = match key {
            "su" | "sg" | "ex" | "mh" => "fi",
            "tw" | "ow" | "st" => "di",
            "or" => "ln",
            "mi" => "or",
            k => k,
        };
        self.indicators.get(key).or_else(|| self.indicators.get(fallback)).or_else(|| {
            if key == "fi" && !self.file_normal_fallback { None } else { self.indicators.get("no") }
        })
    }

    fn has(&self, key: &str) -> bool {
        self.indicators.contains_key(key)
    }

    fn style_for(&self, h: &Hit) -> Option<&Sgr> {
        let path = std::ffi::OsStr::from_bytes(&h.path);
        // A file or folder hit that is really a link was followed (-L): colour it by its
        // target, as fd does.
        let meta = || std::fs::metadata(path).ok();
        let key = match h.ftype {
            FType::File => {
                let m = h.meta.as_deref().map(|m| (m.mode, m.nlink)).or_else(|| meta().map(|m| (std::os::unix::fs::MetadataExt::mode(&m), std::os::unix::fs::MetadataExt::nlink(&m))));
                let (mode, nlink) = m.unwrap_or((0, 1));
                if self.has("su") && mode & 0o4000 != 0 {
                    "su"
                } else if self.has("sg") && mode & 0o2000 != 0 {
                    "sg"
                } else if self.has("ex") && mode & 0o111 != 0 {
                    "ex"
                } else if self.has("mh") && nlink > 1 {
                    "mh"
                } else {
                    "fi"
                }
            }
            FType::Dir => {
                let mode = h.meta.as_deref().map(|m| m.mode).or_else(|| meta().map(|m| std::os::unix::fs::MetadataExt::mode(&m))).unwrap_or(0);
                if self.has("tw") && mode & 0o1002 == 0o1002 {
                    "tw"
                } else if self.has("ow") && mode & 0o002 != 0 {
                    "ow"
                } else if self.has("st") && mode & 0o1000 != 0 {
                    "st"
                } else {
                    "di"
                }
            }
            FType::Symlink => {
                if self.has("or") && std::fs::metadata(path).is_err() {
                    "or"
                } else {
                    "ln"
                }
            }
            FType::Fifo => "pi",
            FType::Socket => "so",
            FType::Block => "bd",
            FType::Char => "cd",
            FType::Unknown => "mi",
        };
        if key == "fi"
            && std::str::from_utf8(h.name()).is_ok()
            && let Some(s) = self.suffix_style(h.name())
        {
            return Some(s);
        }
        self.indicator(key)
    }

    /// fd's layout: the parent folders in the folder style, the name in its own style, and
    /// a folder's trailing separator in the folder style again.
    pub fn paint(&self, h: &Hit, raw: &[u8], sep: Option<&str>, trailing: bool, tty: bool, out: &mut Vec<u8>) {
        let dir = self.indicator("di").copied().unwrap_or_default();
        let text = String::from_utf8_lossy(raw);
        let mut offset = std::path::Path::new(text.as_ref()).parent().map_or(0, |p| p.as_os_str().len());
        offset += text[offset..].bytes().take_while(|&b| b == b'/').count();
        let mut tmp = Vec::new();
        if offset > 0 {
            let parent = &text[..offset];
            let parent = match sep {
                Some(s) => parent.replace('/', s),
                None => parent.to_string(),
            };
            sanitize(&parent, tty, &mut tmp);
            dir.paint(&tmp, out);
            tmp.clear();
        }
        sanitize(&text[offset..], tty, &mut tmp);
        self.style_for(h).copied().unwrap_or_default().paint(&tmp, out);
        if trailing && h.ftype == FType::Dir {
            dir.paint(sep.unwrap_or("/").as_bytes(), out);
        }
    }
}

/// The host name, as `gethostname` reports it (for `file://host/…` links).
pub fn hostname() -> String {
    rustix::system::uname().nodename().to_string_lossy().into_owned()
}

fn file_url(host: &str, abs: &[u8]) -> Vec<u8> {
    let mut out = format!("file://{host}").into_bytes();
    for &b in abs {
        if b.is_ascii_alphanumeric() || b"/:-._~".contains(&b) {
            out.push(b);
        } else {
            out.extend_from_slice(format!("%{b:02X}").as_bytes());
        }
    }
    out
}

/// fd's rule for names written to a terminal: control, bidi and invisible characters are
/// shown as `\xNN` / `\u{NNNN}` so a file name can't drive the terminal.
pub fn sanitize(s: &str, tty: bool, out: &mut Vec<u8>) {
    fn needs_escape(c: char) -> bool {
        c != '\t'
            && (c.is_control()
                || matches!(c, '\u{00AD}' | '\u{180E}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FEFF}' | '\u{FFF9}'..='\u{FFFB}' | '\u{E0000}'..='\u{E007F}'))
    }
    if !tty || !s.chars().any(needs_escape) {
        out.extend_from_slice(s.as_bytes());
        return;
    }
    for c in s.chars() {
        if needs_escape(c) {
            let v = c as u32;
            let e = if v <= 0xFF { format!("\\x{v:02X}") } else { format!("\\u{{{v:04X}}}") };
            out.extend_from_slice(e.as_bytes());
        } else {
            out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
        }
    }
}

/// One output line for a hit (no terminator), the way fd writes it: raw bytes when piped
/// and plain; otherwise as UTF-8 (lossy), with `--path-separator` applied, coloured,
/// formatted, sanitized for terminals, and wrapped in a hyperlink.
pub fn line(h: &Hit, roots: &[RootView], style: &Style, out: &mut Vec<u8>) {
    if style.color.is_none() && style.hyperlink.is_none() && style.separator.is_none() && style.format.is_none() && !style.tty {
        display_into(h, roots, style.trailing_slash, out);
        return;
    }
    let raw = display(h, roots, false);
    if let Some(host) = &style.hyperlink {
        out.extend_from_slice(b"\x1b]8;;");
        out.extend_from_slice(&file_url(host, &h.path));
        out.extend_from_slice(b"\x1b\\");
    }
    let sep = style.separator.as_deref();
    if let Some(t) = &style.format {
        sanitize(&String::from_utf8_lossy(&t.generate(&raw, sep)), style.tty, out);
    } else if let Some(c) = &style.color {
        c.paint(h, &raw, sep, style.trailing_slash, style.tty, out);
    } else {
        let text = String::from_utf8_lossy(&raw);
        match sep {
            Some(s) => sanitize(&text.replace('/', s), style.tty, out),
            None => sanitize(&text, style.tty, out),
        }
        if style.trailing_slash && h.ftype == FType::Dir {
            out.extend_from_slice(sep.unwrap_or("/").as_bytes());
        }
    }
    if style.hyperlink.is_some() {
        out.extend_from_slice(b"\x1b]8;;\x1b\\");
    }
}

// ---------------------------------------------------------------------------------------
// --format, -x and -X templates
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tok {
    Text(Vec<u8>),
    /// `{}`
    Path,
    /// `{/}`
    Base,
    /// `{//}`
    Parent,
    /// `{.}`
    NoExt,
    /// `{/.}`
    BaseNoExt,
}

/// An fd template: `{}` path, `{/}` name, `{//}` parent, `{.}` path without extension,
/// `{/.}` name without extension; `{{` and `}}` are literal braces, and a placeholder
/// followed by `}` is literal text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    toks: Vec<Tok>,
}

impl Template {
    pub fn parse(fmt: &str) -> Template {
        const PATS: [(&str, Option<Tok>); 7] = [("{{", None), ("}}", None), ("{//}", Some(Tok::Parent)), ("{/.}", Some(Tok::BaseNoExt)), ("{/}", Some(Tok::Base)), ("{.}", Some(Tok::NoExt)), ("{}", Some(Tok::Path))];
        let b = fmt.as_bytes();
        let mut toks = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        let mut i = 0;
        while i < b.len() {
            let Some((pat, tok)) = PATS.iter().find(|(p, _)| b[i..].starts_with(p.as_bytes())) else {
                buf.push(b[i]);
                i += 1;
                continue;
            };
            let end = i + pat.len();
            match tok {
                // `{{` or `}}`: one brace.
                None => {
                    buf.push(b[i]);
                    i = end;
                }
                // `{}}`: the placeholder's text, the extra `}` dropped.
                Some(_) if b.get(end) == Some(&b'}') => {
                    buf.extend_from_slice(pat.as_bytes());
                    i = end + 1;
                }
                Some(t) => {
                    if !buf.is_empty() {
                        toks.push(Tok::Text(std::mem::take(&mut buf)));
                    }
                    toks.push(t.clone());
                    i = end;
                }
            }
        }
        if !buf.is_empty() {
            toks.push(Tok::Text(buf));
        }
        Template { toks }
    }

    /// Just `{}`.
    pub fn placeholder() -> Template {
        Template { toks: vec![Tok::Path] }
    }

    pub fn has_tokens(&self) -> bool {
        self.toks.iter().any(|t| !matches!(t, Tok::Text(_)))
    }

    /// The template for `path`; `sep` replaces `/` inside placeholders only.
    pub fn generate(&self, path: &[u8], sep: Option<&str>) -> Vec<u8> {
        use std::ffi::{OsStr, OsString};
        use std::path::{Component, Path, PathBuf};
        let p = Path::new(OsStr::from_bytes(path));
        let basename = |p: &Path| -> OsString { p.file_name().unwrap_or(p.as_os_str()).to_os_string() };
        let dirname = |p: &Path| -> OsString {
            p.parent().map(|d| if d.as_os_str().is_empty() { OsString::from(".") } else { d.as_os_str().to_os_string() }).unwrap_or_else(|| p.as_os_str().to_os_string())
        };
        let no_ext = |p: &Path| -> OsString {
            let joined = PathBuf::from(dirname(p)).join(p.file_stem().unwrap_or(p.as_os_str()));
            joined.strip_prefix(".").map(Path::to_path_buf).unwrap_or(joined).into_os_string()
        };
        let replace = |o: &OsStr| -> Vec<u8> {
            let Some(sep) = sep else { return o.as_bytes().to_vec() };
            let mut out = Vec::new();
            let mut comps = Path::new(o).components().peekable();
            while let Some(c) = comps.next() {
                match c {
                    Component::RootDir => out.extend_from_slice(sep.as_bytes()),
                    c => {
                        out.extend_from_slice(c.as_os_str().as_bytes());
                        if comps.peek().is_some() {
                            out.extend_from_slice(sep.as_bytes());
                        }
                    }
                }
            }
            out
        };
        let mut out = Vec::new();
        for t in &self.toks {
            match t {
                Tok::Text(b) => out.extend_from_slice(b),
                Tok::Path => out.extend(replace(p.as_os_str())),
                Tok::Base => out.extend(replace(&basename(p))),
                Tok::Parent => out.extend(replace(&dirname(p))),
                Tok::NoExt => out.extend(replace(&no_ext(p))),
                Tok::BaseNoExt => out.extend(replace(&no_ext(Path::new(&basename(p))))),
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------
// Summaries, budgets and JSON (EchoFiles additions)
// ---------------------------------------------------------------------------------------

/// Where the hits are, grouped by their first folders below the root, and by extension.
pub fn summary(hits: &[Hit], roots: &[RootView]) -> (Vec<(String, usize)>, Vec<(String, usize)>) {
    let mut by_dir: HashMap<Vec<u8>, usize> = HashMap::new();
    let mut by_ext: HashMap<String, usize> = HashMap::new();
    for h in hits {
        // Group by the first two folders below the search root, shown as they print.
        let key = match roots.get(h.root as usize) {
            Some(r) => {
                let rel = h.path.strip_prefix(r.walk.as_slice()).unwrap_or(&h.path);
                let rel = rel.strip_prefix(b"/").unwrap_or(rel);
                let comps: Vec<&[u8]> = rel.split(|&b| b == b'/').collect();
                let keep = 2.min(comps.len().saturating_sub(1));
                let mut k = r.shown.clone();
                if keep > 0 {
                    if !k.is_empty() && k.last() != Some(&b'/') {
                        k.push(b'/');
                    }
                    k.extend_from_slice(&comps[..keep].join(&b'/'));
                } else if k.is_empty() {
                    k.push(b'.');
                }
                k
            }
            None => h.path.split(|&b| b == b'/').take(3).collect::<Vec<_>>().join(&b'/'),
        };
        *by_dir.entry(key).or_default() += 1;
        let name = h.name();
        let ext = if h.ftype == FType::Dir {
            "(folders)".to_string()
        } else {
            match memchr::memrchr(b'.', name) {
                Some(i) if i > 0 => String::from_utf8_lossy(&name[i..]).to_lowercase(),
                _ => "(no extension)".into(),
            }
        };
        *by_ext.entry(ext).or_default() += 1;
    }
    let mut dirs: Vec<(String, usize)> = by_dir.into_iter().map(|(k, v)| (String::from_utf8_lossy(&k).into_owned(), v)).collect();
    dirs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut exts: Vec<(String, usize)> = by_ext.into_iter().collect();
    exts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    (dirs, exts)
}

pub fn write_summary(breakdown: &(Vec<(String, usize)>, Vec<(String, usize)>), total: usize, out: &mut Vec<u8>) {
    let (dirs, exts) = breakdown;
    let _ = writeln!(out, "# {total} results. By folder:");
    for (d, n) in dirs.iter().take(10) {
        let _ = writeln!(out, "#   {n:>7}  {d}");
    }
    if dirs.len() > 10 {
        let _ = writeln!(out, "#   … {} more folders", dirs.len() - 10);
    }
    let _ = writeln!(out, "# By type:");
    for (e, n) in exts.iter().take(8) {
        let _ = writeln!(out, "#   {n:>7}  {e}");
    }
}

pub fn json_str(s: &[u8]) -> String {
    let text = String::from_utf8_lossy(s);
    serde_json::to_string(&text).unwrap_or_else(|_| "\"\"".into())
}

/// Content facts for agents: binary or text, line count, and tags that say "don't read
/// this" (lockfile, minified, generated, large).
pub struct FileFacts {
    pub lines: Option<u64>,
    pub tags: Vec<&'static str>,
}

const LOCKFILES: &[&str] = &[
    "cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "poetry.lock", "gemfile.lock", "composer.lock", "go.sum", "flake.lock", "uv.lock",
    "pipfile.lock", "bun.lockb", "mix.lock", "pubspec.lock", "podfile.lock", "packages.lock.json",
];
const GENERATED_DIRS: &[&str] = &["/target/", "/dist/", "/build/", "/node_modules/", "/.next/", "/__pycache__/", "/out/", "/.gradle/", "/vendor/"];

pub fn facts(h: &Hit) -> Option<FileFacts> {
    if h.ftype != FType::File {
        return None;
    }
    let name = String::from_utf8_lossy(h.name()).to_lowercase();
    let path = String::from_utf8_lossy(&h.path);
    let mut tags = Vec::new();
    if LOCKFILES.contains(&name.as_str()) {
        tags.push("lockfile");
    }
    if name.contains(".min.") {
        tags.push("minified");
    }
    if GENERATED_DIRS.iter().any(|d| path.contains(d)) {
        tags.push("build-output");
    }
    let size = h.meta.as_ref().map_or(0, |m| m.size);
    if size > 1_000_000 {
        tags.push("large");
    }
    // Read at most 4 MiB: enough to count lines of any file an agent would read whole.
    let mut f = std::fs::File::open(Path::new(&*path)).ok()?;
    let mut head = vec![0u8; 8192];
    let n = std::io::Read::read(&mut f, &mut head).ok()?;
    head.truncate(n);
    let binary = memchr::memchr(0, &head).is_some() || std::str::from_utf8(&head[..n.saturating_sub(4)]).is_err() && !looks_text(&head);
    if binary {
        tags.push("binary");
        return Some(FileFacts { lines: None, tags });
    }
    let lower = String::from_utf8_lossy(&head[..n.min(1024)]).to_lowercase();
    if ["@generated", "do not edit", "auto-generated", "autogenerated", "generated by"].iter().any(|m| lower.contains(m)) {
        tags.push("generated");
    }
    let mut lines = memchr::memchr_iter(b'\n', &head).count() as u64;
    let mut read = n as u64;
    if size <= 4 << 20 {
        let mut buf = vec![0u8; 1 << 16];
        while let Ok(k) = std::io::Read::read(&mut f, &mut buf) {
            if k == 0 {
                break;
            }
            lines += memchr::memchr_iter(b'\n', &buf[..k]).count() as u64;
            read += k as u64;
        }
        if read > 0 && !head.ends_with(b"\n") && read == n as u64 {
            lines += 1;
        }
        if lines > 0 && read / lines > 500 && !tags.contains(&"minified") {
            tags.push("minified");
        }
        Some(FileFacts { lines: Some(lines), tags })
    } else {
        Some(FileFacts { lines: None, tags })
    }
}

fn looks_text(b: &[u8]) -> bool {
    // Latin-1 and other 8-bit text: few control bytes.
    let ctl = b.iter().filter(|&&c| c < 0x20 && !matches!(c, b'\n' | b'\r' | b'\t' | 0x0c)).count();
    ctl * 100 < b.len().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(fmt: &str, path: &str, sep: Option<&str>) -> String {
        String::from_utf8(Template::parse(fmt).generate(path.as_bytes(), sep)).unwrap()
    }

    /// The cases fd's own tests and docs cover.
    #[test]
    fn templates_match_fd() {
        assert_eq!(g("{}", "./a/b.txt", None), "./a/b.txt");
        assert_eq!(g("{/}", "./a/b.txt", None), "b.txt");
        assert_eq!(g("{//}", "./a/b.txt", None), "./a");
        assert_eq!(g("{//}", "b.txt", None), ".");
        assert_eq!(g("{.}", "./a/b.txt", None), "a/b");
        assert_eq!(g("{.}", ".foo", None), ".foo");
        assert_eq!(g("{/.}", "a/b.tar.gz", None), "b.tar");
        assert_eq!(g("{{}}", "x", None), "{}");
        assert_eq!(g("{}}", "x", None), "{}");
        assert_eq!(g("a{{b{}c}}", "x", None), "a{bxc}");
        assert_eq!(g("{}|{//}|{.}", "./a/b.c", Some("\\")), ".\\a\\b.c|.\\a|a\\b");
        assert_eq!(g("{}", "/abs/x", Some("#")), "#abs#x");
        assert!(!Template::parse("plain").has_tokens());
        assert!(Template::parse("x{/.}").has_tokens());
    }

    #[test]
    fn sanitizes_only_for_terminals() {
        let mut out = Vec::new();
        sanitize("a\u{1b}[31mb\u{202E}c\td", true, &mut out);
        assert_eq!(String::from_utf8(out).unwrap(), "a\\x1B[31mb\\u{202E}c\td");
        let mut out = Vec::new();
        sanitize("a\u{1b}b", false, &mut out);
        assert_eq!(out, "a\u{1b}b".as_bytes());
    }
}

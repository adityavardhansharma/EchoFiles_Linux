//! Turning hits into output: fd-style paths (relative, `./`, trailing `/`), colours from
//! `LS_COLORS`, hyperlinks, `-l`, `--format`, `-0`, and EchoFiles' JSON, budget and summary.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use crate::filter::{FType, Hit, Meta};
use crate::units;

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

pub fn with_separator(p: Vec<u8>, sep: &Option<String>) -> Vec<u8> {
    match sep {
        Some(s) if s != "/" => {
            let mut out = Vec::with_capacity(p.len());
            for &b in &p {
                if b == b'/' {
                    out.extend_from_slice(s.as_bytes());
                } else {
                    out.push(b);
                }
            }
            out
        }
        _ => p,
    }
}

// ---------------------------------------------------------------------------------------
// Colours
// ---------------------------------------------------------------------------------------

pub struct LsColors {
    types: HashMap<&'static str, String>,
    exts: Vec<(Vec<u8>, String)>,
}

impl LsColors {
    pub fn parse(spec: &str) -> LsColors {
        let mut c = LsColors { types: HashMap::new(), exts: Vec::new() };
        for (k, v) in [("di", "01;34"), ("ln", "01;36"), ("ex", "01;32"), ("so", "01;35"), ("pi", "33"), ("bd", "01;33"), ("cd", "01;33"), ("or", "01;31")] {
            c.types.insert(k, v.to_string());
        }
        for item in spec.split(':') {
            let Some((k, v)) = item.split_once('=') else { continue };
            if let Some(ext) = k.strip_prefix("*") {
                c.exts.push((ext.to_ascii_lowercase().into_bytes(), v.to_string()));
                continue;
            }
            for key in ["di", "ln", "ex", "so", "pi", "bd", "cd", "or", "fi"] {
                if k == key {
                    c.types.insert(key, v.to_string());
                }
            }
        }
        c
    }

    fn code(&self, h: &Hit, name: &[u8]) -> Option<&str> {
        let key = match h.ftype {
            FType::Dir => "di",
            FType::Symlink => {
                if std::fs::metadata(std::ffi::OsStr::new(std::str::from_utf8(&h.path).unwrap_or(""))).is_err() {
                    "or"
                } else {
                    "ln"
                }
            }
            FType::Socket => "so",
            FType::Fifo => "pi",
            FType::Block => "bd",
            FType::Char => "cd",
            _ => {
                if h.meta.as_ref().is_some_and(|m| m.executable()) {
                    "ex"
                } else {
                    let lower = name.to_ascii_lowercase();
                    if let Some((_, v)) = self.exts.iter().rev().find(|(e, _)| lower.ends_with(e)) {
                        return Some(v);
                    }
                    "fi"
                }
            }
        };
        self.types.get(key).map(String::as_str)
    }

    /// Parent folders in the folder colour, the name in its own.
    pub fn paint(&self, h: &Hit, text: &[u8], out: &mut Vec<u8>) {
        let trimmed = text.strip_suffix(b"/").unwrap_or(text);
        let split = memchr::memrchr(b'/', trimmed).map_or(0, |i| i + 1);
        let (dir, name) = text.split_at(split);
        if !dir.is_empty() {
            match self.types.get("di") {
                Some(c) => {
                    out.extend_from_slice(format!("\x1b[{c}m").as_bytes());
                    out.extend_from_slice(dir);
                    out.extend_from_slice(b"\x1b[0m");
                }
                None => out.extend_from_slice(dir),
            }
        }
        match self.code(h, h.name()) {
            Some(c) if !c.is_empty() => {
                out.extend_from_slice(format!("\x1b[{c}m").as_bytes());
                out.extend_from_slice(name);
                out.extend_from_slice(b"\x1b[0m");
            }
            _ => out.extend_from_slice(name),
        }
    }
}

pub fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname").or_else(|_| std::fs::read_to_string("/proc/sys/kernel/hostname")).map(|s| s.trim().to_string()).unwrap_or_default()
}

fn file_url(host: &str, abs: &[u8]) -> Vec<u8> {
    let mut out = format!("file://{host}").into_bytes();
    for &b in abs {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            out.push(b);
        } else {
            out.extend_from_slice(format!("%{b:02X}").as_bytes());
        }
    }
    out
}

/// One output line for a hit (no terminator).
pub fn line(h: &Hit, roots: &[RootView], style: &Style, out: &mut Vec<u8>) {
    if style.color.is_none() && style.hyperlink.is_none() && style.separator.is_none() {
        display_into(h, roots, style.trailing_slash, out);
        return;
    }
    let text = with_separator(display(h, roots, style.trailing_slash), &style.separator);
    if let Some(host) = &style.hyperlink {
        out.extend_from_slice(b"\x1b]8;;");
        out.extend_from_slice(&file_url(host, &h.path));
        out.extend_from_slice(b"\x1b\\");
    }
    match &style.color {
        Some(c) => c.paint(h, &text, out),
        None => out.extend_from_slice(&text),
    }
    if style.hyperlink.is_some() {
        out.extend_from_slice(b"\x1b]8;;\x1b\\");
    }
}

// ---------------------------------------------------------------------------------------
// -l and --format
// ---------------------------------------------------------------------------------------

fn mode_string(m: &Meta) -> String {
    let t = match m.ftype {
        FType::Dir => 'd',
        FType::Symlink => 'l',
        FType::Socket => 's',
        FType::Fifo => 'p',
        FType::Block => 'b',
        FType::Char => 'c',
        _ => '-',
    };
    let mut s = String::with_capacity(10);
    s.push(t);
    let bits = m.mode;
    for (i, c) in ['r', 'w', 'x', 'r', 'w', 'x', 'r', 'w', 'x'].iter().enumerate() {
        let on = bits & (0o400 >> i) != 0;
        let special = match i {
            2 => bits & 0o4000 != 0,
            5 => bits & 0o2000 != 0,
            8 => bits & 0o1000 != 0,
            _ => false,
        };
        s.push(match (special, on, i) {
            (true, true, 8) => 't',
            (true, false, 8) => 'T',
            (true, true, _) => 's',
            (true, false, _) => 'S',
            (false, true, _) => *c,
            (false, false, _) => '-',
        });
    }
    s
}

/// `ls -l` date: `Oct 13 01:33` within six months, else `Oct 13  2024`.
fn ls_date(t: std::time::SystemTime, now: std::time::SystemTime) -> String {
    const MON: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let u = units::to_unix(t);
    let l = u + units::tz().offset_at(u);
    let (y, m, d) = units::civil_from_days(l.div_euclid(86_400));
    let recent = (units::to_unix(now) - u).abs() < 182 * 86_400;
    let s = l.rem_euclid(86_400);
    if recent {
        format!("{} {d:>2} {:02}:{:02}", MON[(m - 1) as usize], s / 3600, s / 60 % 60)
    } else {
        format!("{} {d:>2}  {y}", MON[(m - 1) as usize])
    }
}

/// `ls -l`-style lines, columns aligned across all hits.
pub fn list_details(hits: &[Hit], roots: &[RootView], style: &Style, out: &mut Vec<u8>) {
    let now = std::time::SystemTime::now();
    let rows: Vec<(String, String, String, String, String, String)> = hits
        .iter()
        .map(|h| match &h.meta {
            Some(m) => (mode_string(m), m.nlink.to_string(), units::user_name(m.uid), units::group_name(m.gid), m.size.to_string(), ls_date(m.mtime, now)),
            None => ("?---------".into(), "?".into(), "?".into(), "?".into(), "?".into(), "?".into()),
        })
        .collect();
    let w = |f: fn(&(String, String, String, String, String, String)) -> usize| rows.iter().map(f).max().unwrap_or(0);
    let (wl, wu, wg, ws) = (w(|r| r.1.len()), w(|r| r.2.len()), w(|r| r.3.len()), w(|r| r.4.len()));
    for (h, r) in hits.iter().zip(&rows) {
        out.extend_from_slice(format!("{} {:>wl$} {:<wu$} {:<wg$} {:>ws$} {} ", r.0, r.1, r.2, r.3, r.4, r.5).as_bytes());
        line(h, roots, style, out);
        if h.ftype == FType::Symlink
            && let Ok(t) = std::fs::read_link(std::ffi::OsStr::new(&String::from_utf8_lossy(&h.path).into_owned()))
        {
            out.extend_from_slice(b" -> ");
            out.extend_from_slice(t.as_os_str().as_encoded_bytes());
        }
        out.push(b'\n');
    }
}

/// fd placeholders: `{}` path, `{/}` name, `{//}` parent, `{.}` path without extension,
/// `{/.}` name without extension; `{{`/`}}` are literal braces.
pub fn expand(template: &str, path: &[u8]) -> Vec<u8> {
    let name_start = memchr::memrchr(b'/', path).map_or(0, |i| i + 1);
    let name = &path[name_start..];
    let parent: &[u8] = if name_start == 0 { b"." } else if name_start == 1 { b"/" } else { &path[..name_start - 1] };
    let strip_ext = |p: &[u8], from: usize| -> Vec<u8> {
        match memchr::memrchr(b'.', &p[from..]) {
            Some(i) if i > 0 => p[..from + i].to_vec(),
            _ => p.to_vec(),
        }
    };
    let mut out = Vec::new();
    let t = template.as_bytes();
    let mut i = 0;
    while i < t.len() {
        let rest = &t[i..];
        let (rep, n): (Option<Vec<u8>>, usize) = if rest.starts_with(b"{{") {
            (Some(b"{".to_vec()), 2)
        } else if rest.starts_with(b"}}") {
            (Some(b"}".to_vec()), 2)
        } else if rest.starts_with(b"{//}") {
            (Some(parent.to_vec()), 4)
        } else if rest.starts_with(b"{/.}") {
            (Some(strip_ext(name, 0)), 4)
        } else if rest.starts_with(b"{/}") {
            (Some(name.to_vec()), 3)
        } else if rest.starts_with(b"{.}") {
            // fd drops a leading `./` here (only here).
            let p = strip_ext(path, name_start);
            (Some(p.strip_prefix(b"./").map(<[u8]>::to_vec).unwrap_or(p)), 3)
        } else if rest.starts_with(b"{}") {
            (Some(path.to_vec()), 2)
        } else {
            (None, 1)
        };
        match rep {
            Some(r) => out.extend_from_slice(&r),
            None => out.push(t[i]),
        }
        i += n;
    }
    out
}

pub fn has_placeholder(s: &str) -> bool {
    ["{}", "{/}", "{//}", "{.}", "{/.}"].iter().any(|p| s.contains(p))
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

pub fn write_summary(hits: &[Hit], roots: &[RootView], out: &mut Vec<u8>) {
    let (dirs, exts) = summary(hits, roots);
    let _ = writeln!(out, "# {} results. By folder:", hits.len());
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

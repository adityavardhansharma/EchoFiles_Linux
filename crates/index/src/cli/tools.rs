//! Commands beyond fd: `why`, `map`, `recent`, `related`, `wait`, `mark`/`changed`,
//! `batch` and `dupes`.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use super::Ctx;
use crate::config;
use crate::filter::{FType, Hit};
use crate::gitignore::{DirRules, Globals, IgnoreOptions, Stack, Verdict};
use crate::units;

use super::args::{self, Flavor, Sort};
use super::find::{self as efind, Indexes, build_search, project_root};
use super::output::{self, RootView};

fn fail(ctx: &Ctx, msg: impl std::fmt::Display) -> u8 {
    errln!(ctx, "ef: {msg}");
    1
}

/// The default place for project-scoped commands: the project around the current folder,
/// else the current folder.
fn here_or_project(ctx: &Ctx) -> PathBuf {
    project_root(&ctx.cwd).unwrap_or_else(|| ctx.cwd.clone())
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// Positional arguments, skipping `--flag value` pairs for the listed value flags.
fn positional<'a>(args: &'a [String], valued: &[&str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if valued.contains(&a.as_str()) {
            i += 2;
            continue;
        }
        if !a.starts_with('-') {
            out.push(a.as_str());
        }
        i += 1;
    }
    out
}

fn display_paths(hits: &[Hit], views: &[RootView]) -> Vec<String> {
    hits.iter().map(|h| String::from_utf8_lossy(&output::display(h, views, false)).into_owned()).collect()
}

// ---------------------------------------------------------------------------------------
// why
// ---------------------------------------------------------------------------------------

/// `ef why PATH [--in ROOT]`: whether PATH shows up in searches, and if not, exactly why.
pub fn why(args: &[String], ctx: &Ctx) -> u8 {
    let settings = &*ctx.settings;
    let Some(target) = positional(args, &["--in"]).first().map(|p| ctx.abs(p)) else {
        return fail(ctx, "usage: ef why PATH [--in FOLDER]");
    };
    let abs = target.clone();
    let Some(parent) = abs.parent().and_then(|p| std::fs::canonicalize(p).ok()) else {
        return fail(ctx, format!("{}: no such folder", abs.display()));
    };
    let path = parent.join(abs.file_name().unwrap_or_default());
    let meta = std::fs::symlink_metadata(&path);
    let say = |s: String| outln!(ctx, "{s}");
    match &meta {
        Ok(m) => say(format!(
            "{}: {}",
            config::tilde(&path),
            if m.is_dir() {
                "folder"
            } else if m.file_type().is_symlink() {
                "symlink"
            } else {
                "file"
            }
        )),
        Err(_) => {
            say(format!("{}: doesn't exist", config::tilde(&path)));
            return 1;
        }
    }
    let is_dir = meta.as_ref().is_ok_and(|m| m.is_dir());
    // The root searches start from: --in, the indexed folder holding it, or the project.
    let idx = ctx.indexes.current(settings);
    let index = crate::find::index_for(&path, &idx);
    let root = match value(args, "--in") {
        Some(r) => std::fs::canonicalize(ctx.abs(r)).unwrap_or_else(|_| ctx.abs(r)),
        None => index
            .map(|m| std::fs::canonicalize(m.root()).unwrap_or_else(|_| m.root().to_path_buf()))
            .or_else(|| project_root(&parent))
            .unwrap_or(parent.clone()),
    };
    let Ok(rel) = path.strip_prefix(&root) else {
        say(format!("not inside {}", config::tilde(&root)));
        return 1;
    };
    let mut verdicts = Vec::new();
    // Hidden.
    if let Some(c) = rel.components().find(|c| c.as_os_str().as_bytes().starts_with(b".")) {
        verdicts
            .push(format!("hidden: '{}' starts with a dot — searches skip it unless you add -H (or -u)", c.as_os_str().to_string_lossy()));
    }
    // Ignore rules, folder by folder from the root down.
    let globals = Arc::new(Globals::new(IgnoreOptions::default(), &root, &[], &[]));
    let mut stack = Stack::root(&root, globals.clone(), None);
    let mut dir = root.clone();
    let comps: Vec<_> = rel.components().collect();
    for (k, c) in comps.iter().enumerate() {
        if let Some(r) = DirRules::load_from_disk(&dir, &globals.opts) {
            stack = stack.push(r);
        }
        let here = dir.join(c.as_os_str());
        let last = k + 1 == comps.len();
        let d = if last { is_dir } else { true };
        match stack.check(here.as_os_str().as_bytes(), d) {
            Verdict::Ignore(set, rule) => {
                let whom = if last { "it".to_string() } else { format!("its folder {}", config::tilde(&here)) };
                verdicts.push(format!(
                    "ignored: {whom} matches '{}' ({}:{}) — add -I (or -u) to include ignored files",
                    rule.pattern(),
                    config::tilde(&set.source),
                    rule.line
                ));
                break;
            }
            Verdict::Whitelist(set, rule) if last => {
                verdicts.push(format!("re-included by '!{}' ({}:{})", rule.pattern(), config::tilde(&set.source), rule.line))
            }
            _ => {}
        }
        dir = here;
    }
    // The index.
    match index {
        None if settings.search.index => {
            verdicts.push("not in an indexed folder: `ef find` reads it live when you pass a folder (--in or a path)".into())
        }
        None => verdicts.push("the search index is off: searches read the disk live".into()),
        Some(m) => {
            if m.lookup(&path).is_some() {
                verdicts.push(format!(
                    "indexed (index of {} saved {})",
                    config::tilde(m.root()),
                    Indexes::age(m.root()).map(|d| units::ago(SystemTime::now() - d, SystemTime::now())).unwrap_or_default()
                ));
            } else {
                let opts = m.options();
                let mut reason = None;
                let mut p = path.clone();
                while let Some(up) = p.parent() {
                    if opts.excludes_path(&p) {
                        reason = Some(format!("{} is excluded in Settings → Search", config::tilde(&p)));
                    } else if p != path && p.file_name().is_some_and(|n| opts.exclude_names.iter().any(|e| e.as_slice() == n.as_bytes())) {
                        reason = Some(format!(
                            "the index skips the contents of '{}' folders (Settings → Search)",
                            p.file_name().unwrap().to_string_lossy()
                        ));
                    } else if p != path && opts.skip_cache_tagged && p.join("CACHEDIR.TAG").exists() {
                        reason =
                            Some(format!("{} is a cache folder (it has CACHEDIR.TAG); the index skips its contents", config::tilde(&p)));
                    }
                    if reason.is_some() || up == m.root() {
                        break;
                    }
                    p = up.to_path_buf();
                }
                verdicts.push(match reason {
                    Some(r) => format!("not in the index: {r}. Searches still read it live when they'd show it."),
                    None => {
                        "not in the index yet: it's newer than the last index update (EchoFiles updates it within a minute while running)"
                            .into()
                    }
                });
            }
        }
    }
    if verdicts.iter().all(|v| v.starts_with("indexed") || v.starts_with("re-included")) {
        verdicts.insert(0, "shows up in searches".into());
    }
    for v in verdicts {
        say(format!("  {v}"));
    }
    0
}

// ---------------------------------------------------------------------------------------
// map
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Node {
    files: u64,
    dirs: u64,
    bytes: u64,
    children: HashMap<Vec<u8>, Node>,
}

/// `ef map [PATH] [--depth N] [--lines N] [-H] [-I]`: the shape of a folder — biggest
/// subfolders with file counts and sizes, junk (ignored files) left out.
pub fn map(args: &[String], ctx: &Ctx) -> u8 {
    let depth: usize = value(args, "--depth").and_then(|v| v.parse().ok()).unwrap_or(2);
    let max_lines: usize = value(args, "--lines").and_then(|v| v.parse().ok()).unwrap_or(60);
    let root = positional(args, &["--depth", "--lines"]).first().map(|p| ctx.abs(p)).unwrap_or_else(|| here_or_project(ctx));
    let mut fargs: Vec<String> = vec!["".into(), root.to_string_lossy().into_owned()];
    for f in ["-H", "-I", "-u", "--live"] {
        if flag(args, f) {
            fargs.push(f.into());
        }
    }
    let mut a = match args::parse(Flavor::Ef, &fargs) {
        Ok(a) => a,
        Err(e) => return fail(ctx, e),
    };
    a.json = true; // metadata on every hit
    let s = match build_search(&a) {
        Ok(s) => s,
        Err(e) => return fail(ctx, e),
    };
    let ran = match efind::run(&a, ctx, &s) {
        Ok(r) => r,
        Err(e) => return fail(ctx, e),
    };
    let base = std::fs::canonicalize(&root).unwrap_or(root.clone());
    let base_b = base.as_os_str().as_bytes();
    let mut tree = Node::default();
    for h in &ran.outcome.hits {
        let rel = h.path.strip_prefix(base_b).unwrap_or(&h.path);
        let comps: Vec<&[u8]> = rel.split(|&b| b == b'/').filter(|c| !c.is_empty()).collect();
        let size = if h.ftype == FType::Dir { 0 } else { h.meta.as_ref().map_or(0, |m| m.size) };
        let mut n = &mut tree;
        let last = comps.len().saturating_sub(1);
        for (k, c) in comps.iter().enumerate() {
            if h.ftype == FType::Dir {
                n.dirs += u64::from(k == last);
            } else {
                n.files += 1;
            }
            n.bytes += size;
            if k == last && h.ftype != FType::Dir {
                break;
            }
            n = n.children.entry(c.to_vec()).or_default();
        }
    }
    let mut out = String::new();
    out.push_str(&format!("{}  — {} files, {}\n", config::tilde(&base), tree.files, units::human_size(tree.bytes)));
    let mut lines = 1;
    fn walk(n: &Node, prefix: &str, level: usize, depth: usize, out: &mut String, lines: &mut usize, max: usize) {
        let mut kids: Vec<(&Vec<u8>, &Node)> = n.children.iter().collect();
        kids.sort_by(|a, b| b.1.bytes.cmp(&a.1.bytes).then(b.1.files.cmp(&a.1.files)).then(a.0.cmp(b.0)));
        let show = if level == 0 { 25 } else { 8 };
        let total = kids.len();
        for (i, (name, k)) in kids.into_iter().enumerate() {
            if *lines >= max {
                return;
            }
            if i == show {
                let rest = n.children.len() - show;
                out.push_str(&format!("{prefix}… {rest} more folders\n"));
                *lines += 1;
                break;
            }
            let last = i + 1 == total.min(show);
            out.push_str(&format!(
                "{prefix}{}{}/  {} files, {}\n",
                if last { "└─ " } else { "├─ " },
                String::from_utf8_lossy(name),
                k.files,
                units::human_size(k.bytes)
            ));
            *lines += 1;
            if level + 1 < depth {
                let p = format!("{prefix}{}", if last { "   " } else { "│  " });
                walk(k, &p, level + 1, depth, out, lines, max);
            }
        }
    }
    walk(&tree, "", 0, depth, &mut out, &mut lines, max_lines);
    let loose = tree.files - tree.children.values().map(|c| c.files).sum::<u64>();
    if loose > 0 {
        out.push_str(&format!("   ({loose} files directly in {})\n", config::tilde(&base)));
    }
    out!(ctx, "{out}");
    if !flag(args, "-I") && !flag(args, "-u") {
        errln!(ctx, "ef: ignored files (.gitignore etc.) and hidden folders left out; -u includes them");
    }
    0
}

// ---------------------------------------------------------------------------------------
// recent
// ---------------------------------------------------------------------------------------

/// `ef recent [PATH] [--within 1d] [-n 20]`: files changed recently, newest first.
/// Any `ef find` option works too (`--ext rs`, `-H`, …).
pub fn recent(args: &[String], ctx: &Ctx) -> u8 {
    let within = value(args, "--within").unwrap_or("1d").to_string();
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--within" {
            i += 2;
            continue;
        }
        rest.push(args[i].clone());
        i += 1;
    }
    let mut a = match args::parse(Flavor::Ef, &rest) {
        Ok(a) => a,
        Err(e) => return fail(ctx, e),
    };
    // `ef recent ~/Documents`: a lone positional is the folder, not a pattern.
    if let Some(p) = a.pattern.take() {
        if ctx.abs(&p).is_dir() {
            a.paths.insert(0, p);
        } else {
            a.pattern = Some(p);
        }
    }
    if a.paths.is_empty() && !a.here && !a.project {
        a.paths.push(here_or_project(ctx).to_string_lossy().into_owned());
    }
    a.newer = Some(within);
    if a.types.any() {
        a.types.file = true;
    }
    a.sort = Some(Sort::Modified);
    if a.max_results.is_none() {
        a.max_results = Some(20);
    }
    let s = match build_search(&a) {
        Ok(s) => s,
        Err(e) => return fail(ctx, e),
    };
    let mut ran = match efind::run(&a, ctx, &s) {
        Ok(r) => r,
        Err(e) => return fail(ctx, e),
    };
    efind::sort_hits(&mut ran.outcome.hits, Sort::Modified);
    let total = ran.outcome.hits.len();
    ran.outcome.hits.truncate(a.max_results.unwrap_or(20));
    if a.json {
        let mut out = Vec::new();
        super::agent::write_json(&a, &ran, ctx, &mut out);
        let _ = ctx.write_out(&out);
        return 0;
    }
    let now = SystemTime::now();
    let paths = display_paths(&ran.outcome.hits, &ran.views);
    let width = paths.iter().map(|p| p.chars().count()).max().unwrap_or(0).min(80);
    for (h, p) in ran.outcome.hits.iter().zip(paths) {
        let m = h.meta.as_deref().copied().unwrap();
        outln!(ctx, "{p:<width$}  {:>9}  {}", units::ago(m.mtime, now), units::human_size(m.size));
    }
    if total > ran.outcome.hits.len() {
        errln!(ctx, "ef: {total} files changed in that time; showing the newest {}", ran.outcome.hits.len());
    } else if total == 0 {
        errln!(ctx, "ef: nothing changed there in that time (try --within 1w, or -H/-I for hidden and ignored files)");
    }
    0
}

// ---------------------------------------------------------------------------------------
// related
// ---------------------------------------------------------------------------------------

/// The part of a name that identifies "the same thing": `parser` for `parser.rs`,
/// `test_parser.py`, `parser_test.go`, `Parser.spec.ts`, `ParserTests.cs`.
fn base_stem(name: &str) -> String {
    let lower = name.to_lowercase();
    let stem = lower.split('.').next().unwrap_or(&lower).to_string();
    let mut s = stem.as_str();
    for p in ["test_", "tests_", "spec_"] {
        if let Some(r) = s.strip_prefix(p) {
            s = r;
        }
    }
    for suf in ["_test", "_tests", "_spec", "-test", "-spec", "tests", "test", "spec"] {
        if let Some(r) = s.strip_suffix(suf).filter(|r| !r.is_empty()) {
            s = r;
            break;
        }
    }
    s.trim_end_matches(['_', '-']).to_string()
}

fn relation(file: &Path, other: &Path) -> &'static str {
    let on = other.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let fname = file.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let in_tests = other.components().any(|c| matches!(c.as_os_str().to_str(), Some("tests" | "test" | "__tests__" | "spec")));
    if on.contains("test") || on.contains("spec") || in_tests {
        return "test";
    }
    let ext = |n: &str| n.rsplit('.').next().unwrap_or("").to_string();
    let (a, b) = (ext(&fname), ext(&on));
    let header = ["h", "hpp", "hh", "hxx"];
    let source = ["c", "cc", "cpp", "cxx", "m", "mm"];
    if (header.contains(&a.as_str()) && source.contains(&b.as_str())) || (source.contains(&a.as_str()) && header.contains(&b.as_str())) {
        return "header/source";
    }
    if b == "md" || b == "txt" || b == "rst" {
        return "docs";
    }
    if ["css", "scss", "less", "sass"].contains(&b.as_str()) {
        return "styles";
    }
    if other.parent() == file.parent() {
        return "same folder";
    }
    "same name"
}

/// `ef related FILE`: its tests, header/source pair, styles, docs and same-named files in
/// the project, closest first.
pub fn related(args: &[String], ctx: &Ctx) -> u8 {
    let Some(file) = positional(args, &["--in"]).first().map(|p| ctx.abs(p)) else {
        return fail(ctx, "usage: ef related FILE [--in FOLDER]");
    };
    let file = std::fs::canonicalize(&file).unwrap_or(file);
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = base_stem(&name);
    if stem.len() < 2 {
        return fail(ctx, format!("'{name}' is too short a name to find relatives of"));
    }
    let root = value(args, "--in")
        .map(|r| ctx.abs(r))
        .or_else(|| project_root(file.parent().unwrap_or(Path::new("/"))))
        .unwrap_or_else(|| file.parent().unwrap_or(Path::new("/")).to_path_buf());
    let fargs = vec![stem.clone(), root.to_string_lossy().into_owned(), "--files".into()];
    let a = match args::parse(Flavor::Ef, &fargs) {
        Ok(a) => a,
        Err(e) => return fail(ctx, e),
    };
    let s = match build_search(&a) {
        Ok(s) => s,
        Err(e) => return fail(ctx, e),
    };
    let ran = match efind::run(&a, ctx, &s) {
        Ok(r) => r,
        Err(e) => return fail(ctx, e),
    };
    let mut found: Vec<(u8, usize, PathBuf, &'static str)> = Vec::new();
    for h in &ran.outcome.hits {
        let p = PathBuf::from(OsStr::from_bytes(&h.path));
        if p == file {
            continue;
        }
        let on = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if base_stem(&on) != stem {
            continue;
        }
        let rel = relation(&file, &p);
        let order = match rel {
            "test" => 0,
            "header/source" => 1,
            "same folder" => 2,
            "styles" => 3,
            "docs" => 4,
            _ => 5,
        };
        // Folder distance: steps up from the file's folder to a common ancestor and down.
        let common = file.components().zip(p.components()).take_while(|(a, b)| a == b).count();
        let dist = file.components().count() + p.components().count() - 2 * common;
        found.push((order, dist, p, rel));
    }
    found.sort();
    if found.is_empty() {
        errln!(ctx, "ef: nothing related to {name} in {}", config::tilde(&root));
        return 1;
    }
    for (_, _, p, rel) in found.iter().take(30) {
        outln!(ctx, "{}  ({rel})", p.display());
    }
    0
}

// ---------------------------------------------------------------------------------------
// wait
// ---------------------------------------------------------------------------------------

/// `ef wait PATTERN [PATH] [--timeout 2m]`: block until something matching appears (read
/// live, never from the index), then print it.
pub fn wait(args: &[String], ctx: &Ctx) -> u8 {
    let timeout = value(args, "--timeout").map(|t| units::parse_duration(t).ok_or(t)).unwrap_or(Ok(Duration::from_secs(120)));
    let Ok(timeout) = timeout else { return fail(ctx, "--timeout: try 30s, 5m or 1h") };
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--timeout" {
            i += 2;
            continue;
        }
        rest.push(args[i].clone());
        i += 1;
    }
    let mut a = match args::parse(Flavor::Ef, &rest) {
        Ok(a) => a,
        Err(e) => return fail(ctx, e),
    };
    if a.pattern.is_none() {
        return fail(ctx, "usage: ef wait PATTERN [FOLDER] [--timeout 2m] [any ef find option]");
    }
    if a.paths.is_empty() {
        a.here = true;
    }
    a.live = true;
    a.hidden = true;
    let s = match build_search(&a) {
        Ok(s) => s,
        Err(e) => return fail(ctx, e),
    };
    let start = Instant::now();
    let mut pause = Duration::from_millis(100);
    loop {
        let ran = match efind::run(&a, ctx, &s) {
            Ok(r) => r,
            Err(e) => return fail(ctx, e),
        };
        if !ran.outcome.hits.is_empty() {
            for p in display_paths(&ran.outcome.hits, &ran.views) {
                outln!(ctx, "{p}");
            }
            return 0;
        }
        if start.elapsed() >= timeout {
            errln!(ctx, "ef: nothing matching appeared within {}", humanize(timeout));
            return 1;
        }
        std::thread::sleep(pause.min(timeout.saturating_sub(start.elapsed())));
        pause = (pause * 3 / 2).min(Duration::from_secs(1));
    }
}

fn humanize(d: Duration) -> String {
    let s = d.as_secs();
    if s < 120 {
        format!("{s} s")
    } else if s < 7200 {
        format!("{} min", s / 60)
    } else {
        format!("{} h", s / 3600)
    }
}

// ---------------------------------------------------------------------------------------
// mark / changed
// ---------------------------------------------------------------------------------------

fn marks_dir() -> PathBuf {
    config::state_dir().join("marks")
}

/// One snapshot line: kind, size, mtime (ns), path; NUL-terminated.
/// (path, kind, size, modified in ns) for every entry below a root.
type Snapshot = Vec<(Vec<u8>, char, u64, u128)>;

fn snapshot(root: &Path, ctx: &Ctx, all: bool) -> Result<Snapshot, String> {
    let mut fargs = vec![String::new(), root.to_string_lossy().into_owned(), "--live".into(), "--json".into()];
    if all {
        fargs.push("-u".into());
    }
    let a = args::parse(Flavor::Ef, &fargs)?;
    let s = build_search(&a)?;
    let ran = efind::run(&a, ctx, &s)?;
    Ok(ran
        .outcome
        .hits
        .iter()
        .map(|h| {
            let m = h.meta.as_deref().copied();
            let kind = match h.ftype {
                FType::Dir => 'd',
                FType::Symlink => 'l',
                _ => 'f',
            };
            let mt = m.map_or(0, |m| m.mtime.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
            (h.path.clone(), kind, m.map_or(0, |m| m.size), mt)
        })
        .collect())
}

/// `ef mark [PATH] [-u]`: remember the current state of a folder; prints a mark id for
/// `ef changed --since ID`.
pub fn mark(args: &[String], ctx: &Ctx) -> u8 {
    let root = positional(args, &[]).first().map(|p| ctx.abs(p)).unwrap_or_else(|| here_or_project(ctx));
    let root = std::fs::canonicalize(&root).unwrap_or(root);
    let all = flag(args, "-u");
    let snap = match snapshot(&root, ctx, all) {
        Ok(s) => s,
        Err(e) => return fail(ctx, e),
    };
    let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let id = format!("m{:x}", t);
    let mut buf = Vec::new();
    buf.extend_from_slice(root.as_os_str().as_bytes());
    buf.extend_from_slice(if all { b"\0u\0" } else { b"\0-\0" });
    for (p, k, size, mt) in &snap {
        buf.extend_from_slice(format!("{k} {size} {mt} ").as_bytes());
        buf.extend_from_slice(p);
        buf.push(0);
    }
    if let Err(e) = config::atomic_write(&marks_dir().join(&id), &buf) {
        return fail(ctx, format!("couldn't save the mark: {e}"));
    }
    // Keep the 50 newest marks.
    if let Ok(rd) = std::fs::read_dir(marks_dir()) {
        let mut all: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        all.sort();
        for old in all.iter().take(all.len().saturating_sub(50)) {
            let _ = std::fs::remove_file(old);
        }
    }
    outln!(ctx, "{id}");
    errln!(ctx, "ef: marked {} ({} entries). Later: ef changed --since {id}", config::tilde(&root), snap.len());
    0
}

/// `ef changed --since ID [--json]`: what was added (+), removed (-) or modified (~)
/// since `ef mark`.
pub fn changed(args: &[String], ctx: &Ctx) -> u8 {
    let Some(id) = value(args, "--since").or_else(|| positional(args, &[]).first().copied()) else {
        return fail(ctx, "usage: ef changed --since MARK-ID   (make one with `ef mark`)");
    };
    if !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return fail(ctx, "that's not a mark id");
    }
    let Ok(buf) = std::fs::read(marks_dir().join(id)) else {
        return fail(ctx, format!("no mark '{id}' (make one with `ef mark`)"));
    };
    let mut parts = buf.split(|&b| b == 0);
    let root = PathBuf::from(OsStr::from_bytes(parts.next().unwrap_or_default()));
    let all = parts.next() == Some(b"u");
    let mut before: HashMap<Vec<u8>, (char, u64, u128)> = HashMap::new();
    for rec in parts.filter(|r| !r.is_empty()) {
        let mut f = rec.splitn(4, |&b| b == b' ');
        let (Some(k), Some(sz), Some(mt), Some(p)) = (f.next(), f.next(), f.next(), f.next()) else { continue };
        let num = |b: &[u8]| std::str::from_utf8(b).ok().and_then(|s| s.parse::<u128>().ok()).unwrap_or(0);
        before.insert(p.to_vec(), (k.first().copied().unwrap_or(b'f') as char, num(sz) as u64, num(mt)));
    }
    let now = match snapshot(&root, ctx, all) {
        Ok(s) => s,
        Err(e) => return fail(ctx, e),
    };
    let mut changes: Vec<(char, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (p, k, size, mt) in &now {
        seen.insert(p.clone());
        let show = String::from_utf8_lossy(p).into_owned();
        match before.get(p) {
            None => changes.push(('+', show)),
            Some((bk, bs, bm)) if *k != 'd' && (bk != k || bs != size || bm != mt) => changes.push(('~', show)),
            _ => {}
        }
    }
    for p in before.keys() {
        if !seen.contains(p) {
            changes.push(('-', String::from_utf8_lossy(p).into_owned()));
        }
    }
    changes.sort_by(|a, b| a.1.cmp(&b.1));
    if flag(args, "--json") {
        let list: Vec<String> = changes
            .iter()
            .map(|(c, p)| {
                format!(
                    "{{\"change\":\"{}\",\"path\":{}}}",
                    match c {
                        '+' => "added",
                        '-' => "removed",
                        _ => "modified",
                    },
                    output::json_str(p.as_bytes())
                )
            })
            .collect();
        outln!(ctx, "{{\"root\":{},\"changes\":[{}]}}", output::json_str(root.as_os_str().as_bytes()), list.join(","));
    } else {
        for (c, p) in &changes {
            outln!(ctx, "{c} {p}");
        }
        if changes.is_empty() {
            errln!(ctx, "ef: nothing changed in {} since {id}", config::tilde(&root));
        }
    }
    0
}

// ---------------------------------------------------------------------------------------
// batch
// ---------------------------------------------------------------------------------------

/// Split a command line like a shell would (quotes and backslashes, no expansion).
fn shell_words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut any = false;
    let mut chars = line.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                any = true;
            }
            (None, '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                    any = true;
                }
            }
            (None, c) if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `ef batch` — several searches in one call: one `ef` command per line on stdin (or
/// `-c 'find x' -c 'recent'`), run in parallel, answered in order under `### <line>`.
pub fn batch(args: &[String], ctx: &Ctx) -> u8 {
    let _settings = &*ctx.settings;
    let mut lines: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-c" {
            if let Some(l) = args.get(i + 1) {
                lines.push(l.clone());
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    if lines.is_empty() {
        let mut s = String::new();
        let _ = std::io::stdin().lock().read_to_string(&mut s);
        lines = s.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(String::from).collect();
    }
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ef"));
    let results: Vec<(String, std::io::Result<std::process::Output>)> = std::thread::scope(|s| {
        let handles: Vec<_> = lines
            .iter()
            .map(|l| {
                let exe = &exe;
                s.spawn(move || {
                    let mut words = shell_words(l);
                    if words.first().is_some_and(|w| w == "ef") {
                        words.remove(0);
                    }
                    (l.clone(), std::process::Command::new(exe).args(&words).stdin(std::process::Stdio::null()).output())
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut out: Vec<u8> = Vec::new();
    let mut failed = false;
    for (line, r) in results {
        let _ = writeln!(out, "### {line}");
        match r {
            Ok(o) => {
                let _ = out.write_all(&o.stdout);
                let _ = out.write_all(&o.stderr);
                failed |= !o.status.success();
            }
            Err(e) => {
                let _ = writeln!(out, "ef: {e}");
                failed = true;
            }
        }
    }
    let _ = ctx.write_out(&out);
    if failed { 1 } else { 0 }
}

// ---------------------------------------------------------------------------------------
// dupes
// ---------------------------------------------------------------------------------------

/// Two independent 64-bit FNV-style hashes of a file's bytes (or its first `limit`).
fn hash_file(path: &[u8], limit: Option<u64>) -> Option<(u64, u64)> {
    let mut f = std::fs::File::open(OsStr::from_bytes(path)).ok()?;
    let mut buf = vec![0u8; 1 << 16];
    let (mut a, mut b) = (0xcbf2_9ce4_8422_2325u64, 0x8422_2325_cbf2_9ce4u64);
    let mut read = 0u64;
    loop {
        let want = limit.map_or(buf.len(), |l| (l - read).min(buf.len() as u64) as usize);
        if want == 0 {
            break;
        }
        let n = f.read(&mut buf[..want]).ok()?;
        if n == 0 {
            break;
        }
        for chunk in buf[..n].chunks(8) {
            let mut w = [0u8; 8];
            w[..chunk.len()].copy_from_slice(chunk);
            let v = u64::from_le_bytes(w);
            a = (a ^ v).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(29);
            b = (b ^ v.rotate_left(17)).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(31);
        }
        read += n as u64;
    }
    Some((a ^ read, b))
}

/// `ef dupes [PATH] [--min-size 1k]`: files with identical contents, biggest waste first.
pub fn dupes(args: &[String], ctx: &Ctx) -> u8 {
    use rayon::prelude::*;
    let min = value(args, "--min-size").and_then(|v| units::SizeFilter::parse(&format!("+{v}"))).map_or(1, |s| match s {
        units::SizeFilter::AtLeast(n) => n.max(1),
        _ => 1,
    });
    let root = positional(args, &["--min-size"]).first().map(|p| ctx.abs(p)).unwrap_or_else(|| here_or_project(ctx));
    let mut fargs = vec![String::new(), root.to_string_lossy().into_owned(), "-t".into(), "f".into(), "--json".into()];
    for f in ["-H", "-I", "-u"] {
        if flag(args, f) {
            fargs.push(f.into());
        }
    }
    let a = match args::parse(Flavor::Ef, &fargs) {
        Ok(a) => a,
        Err(e) => return fail(ctx, e),
    };
    let s = match build_search(&a) {
        Ok(s) => s,
        Err(e) => return fail(ctx, e),
    };
    let ran = match efind::run(&a, ctx, &s) {
        Ok(r) => r,
        Err(e) => return fail(ctx, e),
    };
    let mut by_size: HashMap<u64, Vec<&Hit>> = HashMap::new();
    for h in &ran.outcome.hits {
        if let Some(m) = h.meta.as_deref()
            && m.size >= min
        {
            by_size.entry(m.size).or_default().push(h);
        }
    }
    let candidates: Vec<(u64, Vec<&Hit>)> = by_size.into_iter().filter(|(_, v)| v.len() > 1).collect();
    // First 64 KiB, then (for files still colliding) the whole file.
    let mut groups: Vec<(u64, Vec<Vec<u8>>)> = candidates
        .par_iter()
        .flat_map_iter(|(size, hs)| {
            let mut by_head: HashMap<(u64, u64), Vec<&Hit>> = HashMap::new();
            for h in hs {
                if let Some(k) = hash_file(&h.path, Some(64 * 1024)) {
                    by_head.entry(k).or_default().push(h);
                }
            }
            let mut out = Vec::new();
            for (_, hs) in by_head.into_iter().filter(|(_, v)| v.len() > 1) {
                let mut by_full: HashMap<(u64, u64), Vec<Vec<u8>>> = HashMap::new();
                for h in hs {
                    let k = if *size <= 64 * 1024 { Some((0, 0)) } else { hash_file(&h.path, None) };
                    if let Some(k) = k {
                        by_full.entry(k).or_default().push(h.path.clone());
                    }
                }
                out.extend(by_full.into_values().filter(|v| v.len() > 1).map(|v| (*size, v)));
            }
            out
        })
        .collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.0 * (g.1.len() as u64 - 1)));
    let waste: u64 = groups.iter().map(|(s, v)| s * (v.len() as u64 - 1)).sum();
    for (size, paths) in &groups {
        outln!(ctx, "{} × {}:", paths.len(), units::human_size(*size));
        let mut ps: Vec<String> = paths.iter().map(|p| String::from_utf8_lossy(p).into_owned()).collect();
        ps.sort();
        for p in ps {
            outln!(ctx, "  {p}");
        }
    }
    errln!(ctx, "ef: {} sets of duplicates, {} reclaimable", groups.len(), units::human_size(waste));
    0
}

// ---------------------------------------------------------------------------------------
// status / index
// ---------------------------------------------------------------------------------------

pub fn status(ctx: &Ctx) -> u8 {
    let settings = &*ctx.settings;
    outln!(ctx, "Index: {}", if settings.search.index { "on" } else { "off (searches read the disk)" });
    outln!(
        ctx,
        "EchoFiles app: {}",
        if ctx.app_running {
            "running — it keeps the index current and answers ef from memory"
        } else {
            "not running — ef reads small folders live and marks indexed answers as possibly stale"
        }
    );
    for root in settings.search.roots() {
        let file = config::index_file_for(&root);
        match crate::MappedIndex::open(&file) {
            Ok(idx) => {
                let age = std::fs::metadata(&file).and_then(|m| m.modified()).map(|t| units::ago(t, SystemTime::now())).unwrap_or_default();
                outln!(
                    ctx,
                    "  {}: {} entries, {:.1} MB, saved {age}",
                    config::tilde(&root),
                    idx.len() - 1,
                    idx.mapped_bytes() as f64 / 1e6
                );
            }
            Err(_) => outln!(ctx, "  {}: not indexed yet", config::tilde(&root)),
        }
    }
    0
}

/// `ef index [PATH] [--all]`: re-index the folders from Settings (or PATH).
pub fn index(args: &[String], ctx: &Ctx) -> u8 {
    let settings = &*ctx.settings;
    let targets: Vec<PathBuf> = match args.first().filter(|a| !a.starts_with("--")) {
        Some(p) => vec![ctx.abs(p)],
        None => settings.search.roots(),
    };
    for root in targets {
        let t = Instant::now();
        let opts = if flag(args, "--all") { crate::Options::everything() } else { crate::Options::from_search(&settings.search, &root) };
        let idx = match crate::Index::build_with(&root, &opts) {
            Ok(i) => i,
            Err(e) => return fail(ctx, format!("Couldn't index {}: {e}", config::tilde(&root))),
        };
        let file = config::index_file_for(&root);
        if let Err(e) = std::fs::create_dir_all(config::index_dir()).and_then(|_| idx.save(&file)) {
            return fail(ctx, format!("Couldn't save the index to {}: {e}", file.display()));
        }
        errln!(
            ctx,
            "Indexed {} entries in {} ({} folders' contents skipped) in {:.0} ms",
            idx.len() - 1,
            config::tilde(&root),
            idx.stats().excluded,
            t.elapsed().as_secs_f64() * 1000.0
        );
    }
    0
}

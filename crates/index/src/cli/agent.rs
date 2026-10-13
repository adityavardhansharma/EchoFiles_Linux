//! What `ef find` adds for people and agents on top of fd's output: where answers came
//! from and how fresh they are, why nothing matched (and what would), "did you mean",
//! JSON with metadata and file facts, and phone results.

use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::time::SystemTime;

#[allow(unused_imports)]
use super::find::Indexes as _Indexes;
use crate::config::{self, Settings};
use crate::filter::{FType, Hit};
use crate::find::{Search, Source};
use crate::pattern::Syntax;
use crate::units;

use super::Ctx;
use super::args::{FindArgs, Flavor};
use super::find::{Indexes, Ran, build_search, run};
use super::output::{self, json_str};

/// Phone files (cached while the phone is away) for searches of "everywhere".
pub fn add_phone_hits(a: &FindArgs, ran: &mut Ran) {
    let plain = matches!(a.syntax, None | Some(Syntax::Words))
        && a.sizes.is_empty()
        && a.newer.is_none()
        && a.older.is_none()
        && a.owner.is_none()
        && !a.full_path;
    let Some(text) = a.pattern.as_deref().filter(|_| plain) else { return };
    let kind = if a.types.dir && !a.types.file {
        crate::KindFilter::Dirs
    } else if a.types.file || a.types.not_dir {
        crate::KindFilter::Files
    } else {
        crate::KindFilter::Any
    };
    let ext = a.exts.first().map(String::as_str);
    let q = crate::Query { text, within: crate::ROOT, kind, ext, hidden: a.hidden, limit: None };
    let ranker = crate::Ranker::new(text);
    for e in crate::phone::entries(&q) {
        let path = e.uri.as_os_str().as_bytes().to_vec();
        let name = e.uri.file_name().map(|n| n.as_bytes().to_vec()).unwrap_or_default();
        ran.outcome.hits.push(Hit {
            rank: ranker.rank(&name),
            path,
            root: u16::MAX,
            depth: 0,
            ftype: if e.directory { FType::Dir } else { FType::File },
            meta: Some(Box::new(crate::filter::Meta {
                ftype: if e.directory { FType::Dir } else { FType::File },
                size: e.size,
                mtime: units::from_unix(e.modified),
                mode: 0o644,
                uid: 0,
                gid: 0,
                nlink: 1,
                dev: 0,
                ino: 0,
            })),
        });
    }
}

fn source_text(settings: &Settings, ran: &Ran) -> String {
    let mut parts = Vec::new();
    for r in &ran.outcome.roots {
        let what = match r.source {
            Source::Index => {
                let holes = if r.holes > 0 { format!(", {} skipped folders read live", r.holes) } else { String::new() };
                format!("{} (index{holes})", config::tilde(&r.path))
            }
            Source::Live => format!("{} (read live)", config::tilde(&r.path)),
        };
        parts.push(what);
    }
    let _ = settings;
    if parts.len() > 3 {
        let n = parts.len() - 2;
        parts.truncate(2);
        parts.push(format!("{n} more"));
    }
    parts.join(", ")
}

/// Lines on stderr: truncation, staleness, and — when nothing matched — why, and what
/// would have.
pub fn notes(a: &FindArgs, ctx: &Ctx, s: &Search, ran: &Ran, shown: usize) {
    let settings = &*ctx.settings;
    let mut lines: Vec<String> = Vec::new();
    let total = ran.total;
    if a.verbose {
        lines.push(format!("{total} results from {}", source_text(settings, ran)));
    }
    if let Some(n) = &ran.stale_note {
        lines.push(format!("answered from the {n}"));
    }
    if ran.outcome.vanished > 0 {
        lines.push(format!("{} indexed results were deleted since the last index update and were left out", ran.outcome.vanished));
    }
    if shown < total && a.budget.is_none() && !a.count {
        lines.push(format!("showing the best {shown} of {total} (--limit to change)"));
    }
    if total == 0 && !a.count && a.flavor == Flavor::Ef {
        lines.push(format!("no matches in {}", source_text(settings, ran)));
        lines.extend(why_nothing(a, ctx, s));
    }
    for l in lines {
        errln!(ctx, "ef: {l}");
    }
}

/// Re-run relaxed versions of a search that found nothing, and fuzzy-match the name.
/// Why a search found nothing: counts for relaxed versions of it, and close names.
pub fn why_nothing(a: &FindArgs, ctx: &Ctx, _s: &Search) -> Vec<String> {
    let mut out = Vec::new();
    let t0 = std::time::Instant::now();
    let count = |f: &dyn Fn(&mut FindArgs)| -> Option<usize> {
        if t0.elapsed().as_millis() > 1500 {
            return None;
        }
        let mut b = a.clone();
        f(&mut b);
        b.count = true;
        b.json = false;
        let s = build_search(&b).ok()?;
        run(&b, ctx, &s).ok().map(|r| r.total)
    };
    // (what was relaxed, how to relax it)
    type Try = (String, Box<dyn Fn(&mut FindArgs)>);
    let mut tries: Vec<Try> = Vec::new();
    if !a.hidden {
        tries.push(("include hidden files (-H)".into(), Box::new(|b: &mut FindArgs| b.hidden = true)));
    }
    if !a.no_ignore {
        tries.push(("include files hidden by .gitignore/.ignore (-I)".into(), Box::new(|b: &mut FindArgs| b.no_ignore = true)));
    }
    if !a.hidden || !a.no_ignore {
        tries.push((
            "include both (-u)".into(),
            Box::new(|b: &mut FindArgs| {
                b.hidden = true;
                b.no_ignore = true;
            }),
        ));
    }
    if !a.exts.is_empty() {
        tries.push(("drop the extension filter".into(), Box::new(|b: &mut FindArgs| b.exts.clear())));
    }
    if !a.types.any() {
        tries.push(("drop the type filter".into(), Box::new(|b: &mut FindArgs| b.types = Default::default())));
    }
    if !a.sizes.is_empty() || a.newer.is_some() || a.older.is_some() || a.owner.is_some() {
        tries.push((
            "drop the size/date/owner filters".into(),
            Box::new(|b: &mut FindArgs| {
                b.sizes.clear();
                b.newer = None;
                b.older = None;
                b.owner = None;
            }),
        ));
    }
    if a.max_depth.is_some() || a.min_depth.is_some() {
        tries.push((
            "drop the depth limits".into(),
            Box::new(|b: &mut FindArgs| {
                b.max_depth = None;
                b.min_depth = None;
            }),
        ));
    }
    if matches!(a.syntax, Some(Syntax::Regex | Syntax::Glob | Syntax::Fixed)) {
        tries.push(("search the words instead (no --regex/--glob/-F)".into(), Box::new(|b: &mut FindArgs| b.syntax = Some(Syntax::Words))));
    } else if a.pattern.as_deref().is_some_and(crate::pattern::is_glob_like) {
        tries.push(("read the pattern as a glob (-g)".into(), Box::new(|b: &mut FindArgs| b.syntax = Some(Syntax::Glob))));
    } else if a.pattern.as_deref().is_some_and(crate::pattern::is_regex_like) {
        tries.push(("read the pattern as a regex (--regex)".into(), Box::new(|b: &mut FindArgs| b.syntax = Some(Syntax::Regex))));
    }
    if !a.paths.is_empty() || a.here || a.project {
        tries.push((
            "search everywhere that's indexed (no path)".into(),
            Box::new(|b: &mut FindArgs| {
                b.paths.clear();
                b.here = false;
                b.project = false;
            }),
        ));
    }
    for (what, f) in &tries {
        if let Some(n) = count(f.as_ref())
            && n > 0
        {
            out.push(format!("{n} would match if you {what}"));
        }
    }
    // Did you mean …?
    if let Some(p) = a.pattern.as_deref().filter(|p| !p.contains('/') && p.len() >= 4)
        && matches!(a.syntax, None | Some(Syntax::Words | Syntax::Fixed))
    {
        let idx = ctx.indexes.current(&ctx.settings);
        let mut names: Vec<(String, usize)> = Vec::new();
        let mut covered = false;
        for (root, m) in &idx {
            let within = if a.paths.is_empty() && !a.here && !a.project {
                Some(0)
            } else {
                a.paths
                    .iter()
                    .find_map(|t| std::fs::canonicalize(ctx.abs(t)).ok().filter(|c| c.starts_with(root)).and_then(|c| m.lookup(&c)))
            };
            if let Some(w) = within {
                covered = true;
                names.extend(crate::suggest::similar(m, w, p, 5));
            }
        }
        if !covered && t0.elapsed().as_millis() < 1500 {
            // Not indexed: list the same places (same hidden/ignore rules) and compare.
            let mut b = a.clone();
            b.pattern = None;
            b.and.clear();
            b.exts.clear();
            b.types = Default::default();
            b.max_results = Some(500_000);
            b.sort = Some(super::args::Sort::None);
            if let Ok(s) = build_search(&b)
                && let Ok(r) = run(&b, ctx, &s)
            {
                names.extend(crate::suggest::similar_names(r.outcome.hits.iter().map(|h| h.name()), p, 5));
            }
        }
        names.sort_by_key(|n| n.1);
        names.dedup_by(|x, y| x.0 == y.0);
        if !names.is_empty() {
            let list: Vec<String> = names.iter().take(5).map(|n| format!("\"{}\"", n.0)).collect();
            out.push(format!("did you mean: {}", list.join(", ")));
        }
    }
    out
}

pub fn write_json(a: &FindArgs, ran: &Ran, ctx: &Ctx, out: &mut Vec<u8>) {
    let hits = &ran.outcome.hits;
    let budget = a.budget.map(|t| t * 4);
    let fresh = ctx.app_running;
    let _ = write!(out, "{{\"total\":{},\"sources\":[", ran.total);
    for (i, r) in ran.outcome.roots.iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        let (from, age) = match r.source {
            Source::Index => ("index", ran.index_roots.get(i).and_then(|r| r.as_deref()).and_then(Indexes::age).map(|d| d.as_secs())),
            Source::Live => ("live", None),
        };
        let _ = write!(out, "{{\"root\":{},\"from\":\"{from}\"", json_str(r.path.as_os_str().as_bytes()));
        if from == "index" {
            let _ = write!(out, ",\"watched\":{fresh},\"skipped_folders_read_live\":{}", r.holes);
            if let Some(s) = age {
                let _ = write!(out, ",\"saved_seconds_ago\":{s}");
            }
        }
        out.push(b'}');
    }
    out.extend_from_slice(b"],\"results\":[");
    let start = out.len();
    let mut shown = 0;
    for (i, h) in hits.iter().enumerate() {
        let before = out.len();
        if i > 0 {
            out.push(b',');
        }
        let p = output::display(h, &ran.views, false);
        let _ = write!(out, "{{\"path\":{},\"type\":\"{}\"", json_str(&p), h.ftype.name());
        if let Some(m) = &h.meta {
            if h.ftype != FType::Dir {
                let _ = write!(out, ",\"size\":{}", m.size);
            }
            let _ = write!(out, ",\"modified\":\"{}\"", units::format_rfc3339(m.mtime));
        }
        if a.details
            && i < 200
            && let Some(f) = output::facts(h)
        {
            if let Some(l) = f.lines {
                let _ = write!(out, ",\"lines\":{l}");
            }
            if !f.tags.is_empty() {
                let tags: Vec<String> = f.tags.iter().map(|t| format!("\"{t}\"")).collect();
                let _ = write!(out, ",\"tags\":[{}]", tags.join(","));
            }
        }
        out.push(b'}');
        if budget.is_some_and(|b| out.len() - start > b * 3 / 4) {
            out.truncate(before);
            break;
        }
        shown += 1;
    }
    let _ = write!(out, "],\"shown\":{shown}");
    if shown < ran.total {
        let (dirs, exts) = ran.breakdown.clone().unwrap_or_else(|| output::summary(hits, &ran.views));
        let j = |v: &[(String, usize)]| {
            v.iter()
                .take(10)
                .map(|(k, n)| format!("{{\"{}\":{n}}}", k.replace('\\', "\\\\").replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(",")
        };
        let _ = write!(out, ",\"by_folder\":[{}],\"by_type\":[{}]", j(&dirs), j(&exts));
    }
    if !ran.outcome.errors.is_empty() {
        let errs: Vec<String> = ran.outcome.errors.iter().map(|e| json_str(e.as_bytes())).collect();
        let _ = write!(out, ",\"errors\":[{}]", errs.join(","));
    }
    out.extend_from_slice(b"}\n");
}

/// `path  [12 kB · 340 lines · lockfile]` for `--details` in plain output.
pub fn detail_suffix(h: &Hit) -> String {
    let mut parts = Vec::new();
    if let Some(m) = &h.meta
        && h.ftype != FType::Dir
    {
        parts.push(units::human_size(m.size));
    }
    if let Some(f) = output::facts(h) {
        if let Some(l) = f.lines {
            parts.push(format!("{l} lines"));
        }
        parts.extend(f.tags.iter().map(|t| t.to_string()));
    }
    if let Some(m) = &h.meta {
        parts.push(units::ago(m.mtime, SystemTime::now()));
    }
    if parts.is_empty() { String::new() } else { format!("  [{}]", parts.join(" · ")) }
}

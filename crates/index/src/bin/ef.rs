//! `ef` — EchoFiles from the terminal and for AI agents. Uses the same index as the app
//! (Settings → Search); `--in` scans live when its folder is outside the index.
//! Commands can be turned off in Settings → AI agents.
//!
//! ```text
//! ef find <text> [--ext EXT] [--dirs|--files] [--hidden] [--in PATH] [--limit N] [--count]
//! ef index [PATH] [--all]     re-index the folders from Settings (or PATH)
//! ef status                   what's indexed, how big, how fresh
//! ```
//!
//! `EF_TIMING=1` prints timings to stderr.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::{Instant, SystemTime};

use ef_index::config::{self, Settings};
use ef_index::{KindFilter, MappedIndex, Options, Query};

const OFF: &str = "EchoFiles commands are turned off. Turn them on in EchoFiles → Settings → AI agents.";

fn usage() -> ExitCode {
    eprintln!("usage: ef find <text> [--ext EXT] [--dirs|--files] [--hidden] [--in PATH] [--limit N] [--count]\n       ef index [PATH] [--all]\n       ef status");
    ExitCode::from(2)
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn age(path: &Path) -> String {
    let secs = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map_or(0, |d| d.as_secs());
    match secs {
        0..=59 => format!("{secs} s ago"),
        60..=3599 => format!("{} min ago", secs / 60),
        _ => format!("{} h ago", secs / 3600),
    }
}

fn main() -> ExitCode {
    let t0 = Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let settings = match Settings::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if !settings.agents.cli {
        eprintln!("{OFF}");
        return ExitCode::from(3);
    }
    let roots = settings.search.roots();
    match args.first().map(String::as_str) {
        Some("find") => {
            let Some(text) = args.get(1).filter(|a| !a.starts_with("--")) else { return usage() };
            let ext = value(&args, "--ext").map(|e| e.trim_start_matches('.').to_string());
            let limit: Option<usize> = value(&args, "--limit").and_then(|v| v.parse().ok());
            let within = value(&args, "--in").map(|p| {
                let path = config::expand(p);
                std::fs::canonicalize(&path).unwrap_or(path)
            });
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            let q = Query { text, within: ef_index::ROOT,
                kind: if flag(&args, "--dirs") { KindFilter::Dirs } else if flag(&args, "--files") { KindFilter::Files } else { KindFilter::Any },
                ext: ext.as_deref(), hidden: flag(&args, "--hidden"), limit: None };
            let mut hits = Vec::new();
            let targets = within.clone().map(|p| vec![p]).unwrap_or_else(|| roots.clone());
            let mut any_index = false;
            for root in &targets {
                // Explicit scopes may fall inside excluded index subtrees; a live walk is
                // complete and honours the user's requested location in that case.
                let indexed = if within.is_none() && settings.search.index {
                    MappedIndex::open(&config::index_file_for(root)).ok().filter(|m| m.options() == &Options::from_search(&settings.search, root))
                } else { None };
                let found = if let Some(idx) = indexed {
                    any_index = true;
                    idx.search(&q).into_iter().map(|id| idx.path(id)).collect()
                } else {
                    let opts = if within.is_some() { Options::everything() } else { Options::from_search(&settings.search, root) };
                    match ef_index::live::search_with(root, &q, &opts, &AtomicBool::new(false), &|_| {}) {
                        Ok(paths) => { any_index = true; paths },
                        Err(e) => { eprintln!("Couldn't search {}: {e}", root.display()); return ExitCode::FAILURE; },
                    }
                };
                let nested = config::nested_roots(&targets, root);
                hits.extend(found.into_iter().filter(|p| !nested.iter().any(|n| p.starts_with(n))));
            }
            if within.is_none() && settings.phone.phone_index { hits.extend(ef_index::phone::search(&q)); }
            ef_index::rank_paths(&mut hits, &Query { limit, ..q });
            let total = hits.len();
            if !flag(&args, "--count") { for path in hits { let _ = writeln!(out, "{}", path.display()); } }
            if flag(&args, "--count") {
                let _ = writeln!(out, "{total}");
            }
            let _ = out.flush();
            if !any_index && within.is_none() {
                eprintln!("No index yet. Turn on the search index in EchoFiles → Settings → Search, or run `ef index`.");
                return ExitCode::FAILURE;
            }
            if std::env::var_os("EF_TIMING").is_some() {
                eprintln!("{total} hits in {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
            }
            ExitCode::SUCCESS
        }
        Some("index") => {
            let targets: Vec<PathBuf> = match args.get(1).filter(|a| !a.starts_with("--")) {
                Some(p) => vec![config::expand(p)],
                None => roots.clone(),
            };
            for root in targets {
                let t = Instant::now();
                let opts = if flag(&args, "--all") { Options::everything() } else { Options::from_search(&settings.search, &root) };
                let idx = match ef_index::Index::build_with(&root, &opts) {
                    Ok(i) => i,
                    Err(e) => {
                        eprintln!("Couldn't index {}: {e}", config::tilde(&root));
                        return ExitCode::FAILURE;
                    }
                };
                let file = config::index_file_for(&root);
                if let Err(e) = std::fs::create_dir_all(config::index_dir()).and_then(|_| idx.save(&file)) {
                    eprintln!("Couldn't save the index to {}: {e}", file.display());
                    return ExitCode::FAILURE;
                }
                eprintln!(
                    "Indexed {} entries in {} ({} folders' contents skipped) in {:.0} ms",
                    idx.len() - 1,
                    config::tilde(&root),
                    idx.stats().excluded,
                    t.elapsed().as_secs_f64() * 1000.0
                );
            }
            ExitCode::SUCCESS
        }
        Some("status") => {
            println!("Index: {}", if settings.search.index { "on" } else { "off (the app searches live)" });
            for root in &roots {
                let file = config::index_file_for(root);
                match MappedIndex::open(&file) {
                    Ok(idx) => println!("  {}: {} entries, {:.1} MB, updated {}", config::tilde(root), idx.len() - 1, idx.mapped_bytes() as f64 / 1e6, age(&file)),
                    Err(_) => println!("  {}: not indexed yet", config::tilde(root)),
                }
            }
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}

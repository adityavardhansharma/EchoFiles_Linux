//! Prototype agent CLI.
//!
//! `ef index <root> [index-file]`       crawl and save
//! `ef find <pattern> [--ext] [--limit N] [--count]`  query the saved index
//!
//! Timings go to stderr with `EF_TIMING=1`; results go to stdout, one path per line.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

fn index_path() -> PathBuf {
    let home = std::env::var_os("HOME").expect("HOME");
    PathBuf::from(home).join(".cache/echofiles/home.efidx")
}

fn main() {
    let t0 = Instant::now();
    let timing = std::env::var_os("EF_TIMING").is_some();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("index") => {
            let root = PathBuf::from(args.get(1).expect("usage: ef index <root> [file]"));
            let out = args.get(2).map(PathBuf::from).unwrap_or_else(index_path);
            let idx = ef_index::Index::build(&root).unwrap();
            let crawl = t0.elapsed();
            std::fs::create_dir_all(out.parent().unwrap()).unwrap();
            idx.save(&out).unwrap();
            let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
            eprintln!(
                "indexed {} entries under {} in {:.0} ms (save {:.0} ms, file {:.1} MB)",
                idx.len(),
                root.display(),
                crawl.as_secs_f64() * 1000.0,
                (t0.elapsed() - crawl).as_secs_f64() * 1000.0,
                size as f64 / 1e6
            );
        }
        Some("find") => {
            let pattern = args.get(1).expect("usage: ef find <pattern> [--ext] [--limit N] [--count]");
            let ext = args.iter().any(|a| a == "--ext");
            let count_only = args.iter().any(|a| a == "--count");
            let limit = args.iter().position(|a| a == "--limit").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
            let idx = ef_index::Index::load(&index_path()).expect("run `ef index ~` first");
            let loaded = t0.elapsed();
            let mut q = ef_index::Query::new(if ext { "" } else { pattern });
            q.ext = ext.then_some(pattern.as_str());
            q.hidden = true;
            let hits = idx.search(&q);
            let queried = t0.elapsed();
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            if count_only {
                let _ = writeln!(out, "{}", hits.len());
            } else {
                for &h in hits.iter().take(limit) {
                    let _ = writeln!(out, "{}", idx.path(h).display());
                }
            }
            let _ = out.flush();
            if timing {
                eprintln!(
                    "load {:.1} ms · query {:.1} ms · output {:.1} ms · {} hits",
                    loaded.as_secs_f64() * 1000.0,
                    (queried - loaded).as_secs_f64() * 1000.0,
                    (t0.elapsed() - queried).as_secs_f64() * 1000.0,
                    hits.len()
                );
            }
        }
        _ => eprintln!("usage: ef index <root> | ef find <pattern> [--ext] [--limit N] [--count]"),
    }
}

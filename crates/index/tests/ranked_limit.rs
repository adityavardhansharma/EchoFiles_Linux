//! A ranked search cut to N (`ef find -n N`, the app's search box) keeps only the best N
//! hits while walking the index. It must return exactly the first N of the full ranked
//! answer, with the same total count, for every query shape.

use std::sync::Arc;

use ef_index::cli::args::{self, Flavor, Sort};
use ef_index::cli::{self, Ctx, find, query};
use ef_index::corpus::{self, Layout};

fn ctx(root: &std::path::Path, index_file: &std::path::Path) -> Ctx {
    let mut settings = ef_config::Settings::default();
    settings.search.roots = vec![root.display().to_string()];
    settings.search.index_drives = false;
    let opts = ef_index::Options::from_search(&settings.search, root);
    ef_index::Index::build_with(root, &opts).unwrap().save(index_file).unwrap();
    let map = Arc::new(ef_index::MappedIndex::open(index_file).unwrap());
    Ctx {
        cwd: root.to_path_buf(),
        out: cli::stream(std::io::sink()),
        err: cli::stream(std::io::sink()),
        out_tty: false,
        ls_colors: None,
        no_color: true,
        settings: Arc::new(settings),
        indexes: Arc::new(find::Indexes::preloaded(vec![(std::fs::canonicalize(root).unwrap(), map)])),
        served: true,
        app_running: true,
    }
}

fn ranked(ctx: &Ctx, text: &str, limit: Option<usize>, hidden: bool, settings_mode: bool) -> (usize, Vec<Vec<u8>>) {
    let mut a = args::parse(Flavor::Ef, &[]).unwrap();
    query::apply(text, &mut a).unwrap();
    a.hidden = hidden;
    a.no_ignore = settings_mode;
    a.respect_settings = settings_mode;
    a.sort = Some(Sort::Rank);
    a.max_results = limit;
    let s = find::build_search(&a).unwrap();
    let mut ran = find::run(&a, ctx, &s).unwrap();
    find::sort_hits(&mut ran.outcome.hits, Sort::Rank);
    let mut paths: Vec<Vec<u8>> = ran.outcome.hits.iter().map(|h| h.path.clone()).collect();
    if let Some(n) = limit {
        paths.truncate(n);
    }
    (ran.total, paths)
}

#[test]
fn best_n_is_a_prefix_of_the_full_answer() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("files");
    corpus::generate(&root, 6_000, Layout::Tree).unwrap();
    // Ignore rules and a skipped folder, so every path through the walk is exercised.
    std::fs::write(root.join("Documents/.gitignore"), "*.txt\n!report*.txt\n").unwrap();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(root.join("Projects/node_modules/pkg")).unwrap();
    std::fs::write(root.join("Projects/node_modules/pkg/report.js"), b"").unwrap();
    let ctx = ctx(&root, &dir.path().join("index"));
    let mut checked = 0;
    for text in ["report", "e", "r", "ext:pdf", "report ext:pdf", "re:^rep", "glob:*report*", "type:folder part", "size:<1k invoice"] {
        for hidden in [false, true] {
            for settings_mode in [false, true] {
                let (total, all) = ranked(&ctx, text, None, hidden, settings_mode);
                for n in [1, 7, 50, 300] {
                    let (t, best) = ranked(&ctx, text, Some(n), hidden, settings_mode);
                    assert_eq!(t, total, "total for {text:?} hidden={hidden} settings={settings_mode} n={n}");
                    assert_eq!(best, all[..n.min(all.len())], "best {n} for {text:?} hidden={hidden} settings={settings_mode}");
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 144);
}

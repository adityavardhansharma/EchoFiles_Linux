use std::{fs, sync::{Arc, atomic::AtomicBool}};
use ef_index::{Index, MappedIndex, Options, Query, KindFilter};
#[test]
fn malformed_headers_are_errors() {
    let root = tempfile::tempdir().unwrap(); fs::write(root.path().join("x"), b"x").unwrap();
    let file = root.path().join("snapshot"); Index::build(root.path()).unwrap().save(&file).unwrap();
    let original = fs::read(&file).unwrap();
    for at in [8, 16, 24, 32, 40] {
        for value in [u64::MAX, u64::MAX - 1, u64::MAX - 3, 1 << 63] {
            let mut bytes = original.clone(); bytes[at..at+8].copy_from_slice(&value.to_le_bytes()); fs::write(&file, bytes).unwrap();
            assert!(MappedIndex::open(&file).is_err());
        }
    }
}
#[test]
fn wildcard_and_exclusion_parity() {
    let root = tempfile::tempdir().unwrap(); fs::create_dir(root.path().join("cache")).unwrap();
    fs::write(root.path().join("cache/CACHEDIR.TAG"), b"Signature: 8a477f597d28d172789f06886806bc55\n").unwrap(); fs::write(root.path().join("cache/secret"), b"").unwrap(); fs::write(root.path().join("report"), b"").unwrap();
    let cfg = ef_config::SearchConfig { roots: vec![root.path().display().to_string()], ..Default::default() };
    let options = Options::from_search(&cfg, root.path()); let ix = Index::build_with(root.path(), &options).unwrap();
    for kind in [KindFilter::Any, KindFilter::Dirs, KindFilter::Files] {
        let q = Query { kind, hidden: true, ..Query::new("*") };
        let mut indexed: Vec<_> = ix.search(&q).into_iter().map(|id| ix.path(id)).collect(); indexed.sort();
        let mut live = ef_index::live::search_with(root.path(), &q, &options, &AtomicBool::new(false), &|_| {}).unwrap(); live.sort();
        assert_eq!(indexed, live); assert!(!indexed.is_empty());
    }
    let cfg = ef_config::SearchConfig { exclude_paths: vec![root.path().display().to_string()], ..cfg };
    let options = Options::from_search(&cfg, root.path());
    assert!(Index::build_with(root.path(), &options).unwrap().search(&Query::new("*")).is_empty());
}
#[test]
fn simultaneous_publication_keeps_mappings_immutable() {
    let root = tempfile::tempdir().unwrap(); fs::write(root.path().join("one"), b"").unwrap();
    let ix = Arc::new(Index::build(root.path()).unwrap()); let file = root.path().join("snapshot"); ix.save(&file).unwrap(); let old = MappedIndex::open(&file).unwrap();
    std::thread::scope(|scope| { for _ in 0..8 { let ix = &ix; let file = &file; scope.spawn(move || for _ in 0..10 { ix.save(file).unwrap(); }); } });
    assert_eq!(old.search(&Query::new("one")).len(), 1); assert_eq!(MappedIndex::open(&file).unwrap().search(&Query::new("one")).len(), 1);
}

#[test]
fn cli_wildcard_scoped_overlap_and_global_limit() {
    let root = tempfile::tempdir().unwrap(); let parent = root.path().join("files"); let child = parent.join("skip"); fs::create_dir_all(&child).unwrap();
    for i in 0..305 { fs::write(parent.join(format!("some-report-{i}")), b"").unwrap(); } fs::write(child.join("report"), b"").unwrap();
    let mut cfg = ef_config::Settings::default(); cfg.search.roots = vec![parent.display().to_string(), child.display().to_string()]; cfg.search.exclude_names = vec!["skip".into()];
    cfg.save_to(&root.path().join("config/echofiles/settings.toml")).unwrap();
    let command = |args: &[&str]| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_ef")).args(args).env("XDG_CONFIG_HOME", root.path().join("config")).env("XDG_CACHE_HOME", root.path().join("cache")).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr)); String::from_utf8(out.stdout).unwrap()
    };
    command(&["index"]);
    assert_eq!(command(&["find", "*", "--limit", "2", "--count"]).trim(), "2");
    assert_eq!(command(&["find", "report", "--in", child.to_str().unwrap(), "--count"]).trim(), "1");
    assert_eq!(command(&["find", "report", "--limit", "1"]).trim(), child.join("report").to_str().unwrap());
}

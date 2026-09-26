//! `cargo bench -p echofiles-index` — corpora are generated on first run under
//! `~/.cache/echofiles-bench/index/` (flat and tree, 100 to 100k files; see `corpus.rs`).
//!
//! `walk_*` benches are the no-index baseline: what an in-app search costs today, walking the
//! folder with `readdir` and matching each name.

use std::path::Path;

use ef_index::corpus::{self, Layout};
use ef_index::{Index, KindFilter, Query};

const SIZES: &[usize] = &[100, 1_000, 10_000, 100_000];

fn main() {
    divan::main();
}

fn index(n: usize, layout: Layout) -> Index {
    Index::build(&corpus::cached(n, layout)).unwrap()
}

fn search(b: divan::Bencher, n: usize, layout: Layout, make: impl Fn(&Index) -> Query<'static>) {
    let idx = index(n, layout);
    let q = make(&idx);
    b.bench_local(|| idx.search(divan::black_box(&q)));
}

// --- Building ---------------------------------------------------------------------------

#[divan::bench(args = SIZES)]
fn build_flat(n: usize) -> Index {
    Index::build(&corpus::cached(n, Layout::Flat)).unwrap()
}

#[divan::bench(args = SIZES)]
fn build_tree(n: usize) -> Index {
    Index::build(&corpus::cached(n, Layout::Tree)).unwrap()
}

#[divan::bench(args = SIZES)]
fn load_from_disk(b: divan::Bencher, n: usize) {
    let file = std::env::temp_dir().join(format!("ef-bench-{n}.efidx"));
    index(n, Layout::Tree).save(&file).unwrap();
    b.bench_local(|| Index::load(&file).unwrap());
}

// --- Searching the whole index ------------------------------------------------------------

/// A similar-name family: ~1 in 4 files match.
#[divan::bench(args = SIZES)]
fn common_word(b: divan::Bencher, n: usize) {
    search(b, n, Layout::Flat, |_| Query::new("report"));
}

#[divan::bench(args = SIZES)]
fn no_match(b: divan::Bencher, n: usize) {
    search(b, n, Layout::Flat, |_| Query::new("zqxj"));
}

#[divan::bench(args = SIZES)]
fn two_terms(b: divan::Bencher, n: usize) {
    search(b, n, Layout::Flat, |_| Query::new("report final"));
}

/// Accent-insensitive: matches NFC and NFD "Résumé".
#[divan::bench(args = SIZES)]
fn accented(b: divan::Bencher, n: usize) {
    search(b, n, Layout::Flat, |_| Query::new("résumé"));
}

#[divan::bench(args = SIZES)]
fn cjk(b: divan::Bencher, n: usize) {
    search(b, n, Layout::Flat, |_| Query::new("日本語"));
}

#[divan::bench(args = SIZES)]
fn shell_chars(b: divan::Bencher, n: usize) {
    search(b, n, Layout::Flat, |_| Query::new("[draft] (v2)"));
}

#[divan::bench(args = SIZES)]
fn extension_only(b: divan::Bencher, n: usize) {
    search(b, n, Layout::Flat, |_| Query { ext: Some("pdf"), ..Query::new("") });
}

/// One letter: nearly everything matches, so this is the ranking worst case.
#[divan::bench(args = SIZES)]
fn one_letter_top50(b: divan::Bencher, n: usize) {
    search(b, n, Layout::Flat, |_| Query { limit: Some(50), hidden: true, ..Query::new("e") });
}

#[divan::bench(args = SIZES)]
fn one_letter_all(b: divan::Bencher, n: usize) {
    search(b, n, Layout::Flat, |_| Query { hidden: true, ..Query::new("e") });
}

// --- Searching one folder of a 100k-file tree ------------------------------------------

/// A leaf folder of ~100 files inside the 100k tree.
#[divan::bench]
fn in_folder_100_of_100k(b: divan::Bencher) {
    search(b, 100_000, Layout::Tree, |idx| {
        let dir = idx.root().join(corpus::folder_of(0, Layout::Tree));
        Query { within: idx.lookup(&dir).unwrap(), ..Query::new("report") }
    });
}

/// A top-level folder (~12.5k files) inside the 100k tree.
#[divan::bench]
fn in_folder_12k_of_100k(b: divan::Bencher) {
    search(b, 100_000, Layout::Tree, |idx| {
        let dir = idx.root().join("Documents");
        Query { within: idx.lookup(&dir).unwrap(), kind: KindFilter::Files, ..Query::new("report") }
    });
}

// --- Keeping it fresh --------------------------------------------------------------------

#[divan::bench]
fn rescan_leaf_folder_in_100k(b: divan::Bencher) {
    let mut idx = index(100_000, Layout::Tree);
    let dir = idx.root().join(corpus::folder_of(0, Layout::Tree));
    b.bench_local(|| idx.rescan(&dir).unwrap());
}

// --- No-index baseline -------------------------------------------------------------------

fn walk(dir: &Path, needle: &[u8], hits: &mut usize) {
    use std::os::unix::ffi::OsStrExt;
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name();
        let mut f = Vec::new();
        ef_index::fold::fold_into(name.as_bytes(), &mut f);
        if memchr::memmem::find(&f, needle).is_some() {
            *hits += 1;
        }
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            walk(&e.path(), needle, hits);
        }
    }
}

#[divan::bench(args = SIZES)]
fn walk_flat_no_index(n: usize) -> usize {
    let mut hits = 0;
    walk(&corpus::cached(n, Layout::Flat), b"report", &mut hits);
    hits
}

#[divan::bench(args = SIZES)]
fn walk_tree_no_index(n: usize) -> usize {
    let mut hits = 0;
    walk(&corpus::cached(n, Layout::Tree), b"report", &mut hits);
    hits
}

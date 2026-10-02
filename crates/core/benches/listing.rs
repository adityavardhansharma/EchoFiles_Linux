//! `cargo bench -p echofiles-core` — needs a corpus: `cargo run -p echofiles-core --release
//! --bin mkcorpus -- ~/.cache/echofiles-bench/d100k 100000` (and d10k with 10000).

use std::path::PathBuf;

use ef_core::{listing, sort};

fn corpus(n: &str) -> PathBuf {
    if n == "d100k" && let Some(path) = std::env::var_os("ECHOFILES_BENCH_100K") {
        return PathBuf::from(path);
    }
    let home = std::env::var_os("HOME").expect("HOME");
    PathBuf::from(home).join(".cache/echofiles-bench").join(n)
}

fn main() {
    divan::main();
}

#[divan::bench(args = ["d10k", "d100k"])]
fn names_only(n: &str) -> listing::Listing {
    let dir = corpus(n);
    let fd = listing::open_dir(&dir).unwrap();
    listing::read_names(&dir, &fd).unwrap()
}

#[divan::bench(args = ["d10k", "d100k"])]
fn full_listing(n: &str) -> listing::Listing {
    listing::list(&corpus(n)).unwrap()
}

#[divan::bench(args = ["d10k", "d100k"])]
fn sort_by_name(bencher: divan::Bencher, n: &str) {
    let l = listing::list(&corpus(n)).unwrap();
    bencher.bench_local(|| {
        let keys = sort::NameKeys::build(&l);
        sort::order(&l, &keys, sort::SortSpec::default(), true)
    });
}

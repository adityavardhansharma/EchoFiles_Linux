//! Indexed and live search results must both equal a brute-force walk of the same folders,
//! for every query shape, on folders of 100 to 100k files full of near-identical and
//! special-character names.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use ef_index::corpus::{self, Layout};
use ef_index::{Index, Kind, KindFilter, Query, fold, live};

/// Every entry below the root, found with `std::fs` alone.
struct Entry {
    path: PathBuf,
    dir: bool,
}

fn walk(dir: &Path, out: &mut Vec<Entry>) {
    for e in fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let dir = e.file_type().unwrap().is_dir();
        out.push(Entry { path: e.path(), dir });
        if dir {
            walk(&e.path(), out);
        }
    }
}

fn fold_raw(name: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    fold::fold_into(name, &mut v);
    v
}

fn oracle(all: &[Entry], scope: &Path, q: &Query) -> Vec<PathBuf> {
    let terms: Vec<Vec<u8>> = q.text.split_whitespace().map(fold::fold).collect();
    let ext = q.ext.map(|e| [b".".as_slice(), &fold::fold(e)].concat());
    if terms.is_empty() && ext.is_none() {
        return Vec::new();
    }
    let mut out: Vec<PathBuf> = all
        .iter()
        .filter(|e| {
            let Ok(rel) = e.path.strip_prefix(scope) else { return false };
            if rel.as_os_str().is_empty() {
                return false;
            }
            let name = fold_raw(e.path.file_name().unwrap().as_bytes());
            let hidden = rel.components().any(|c| c.as_os_str().as_bytes().starts_with(b"."));
            (q.hidden || !hidden)
                && match q.kind {
                    KindFilter::Any => true,
                    KindFilter::Files => !e.dir,
                    KindFilter::Dirs => e.dir,
                }
                && ext.as_ref().is_none_or(|x| name.len() > x.len() && name.ends_with(x))
                && terms.iter().all(|t| memchr::memmem::find(&name, t).is_some())
        })
        .map(|e| e.path.clone())
        .collect();
    out.sort();
    out
}

fn results(idx: &Index, q: &Query) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = idx.search(q).into_iter().map(|id| idx.path(id)).collect();
    v.sort();
    v
}

const TEXTS: &[&str] = &[
    "report",
    "REPORT",
    "Report Final",
    "final report",
    "rep ort",
    "reportage",
    "résumé",
    "RESUME",
    "re\u{301}sume\u{301}",
    "strasse",
    "STRAßE",
    "日本語",
    "メモ",
    "🎉",
    "👨\u{200d}👩\u{200d}👧",
    "Ｒｅｐｏｒｔ",
    "σίσυφος",
    "ΣΊΣΥΦΟΣ",
    "istanbul",
    "हिंदी",
    "cafe",
    "[draft]",
    "(v2) {braces}",
    "it's",
    "$home &",
    "back\\slash",
    "star*q?",
    "-dash",
    "#hash %",
    "semi;colon:",
    "tab\there",
    "new\nline",
    "latin1",
    "IMG_2026",
    ".pdf",
    ".tar.gz",
    "e",
    "7",
    "long",
    "zqxj",
    "reports",
];

fn live_results(scope: &Path, q: &Query) -> Vec<PathBuf> {
    let streamed = std::sync::Mutex::new(Vec::new());
    let mut v = live::search(scope, q, &AtomicBool::new(false), &|batch| streamed.lock().unwrap().extend_from_slice(batch)).unwrap();
    v.sort();
    // Everything returned was also streamed, and nothing else (without a limit).
    let mut s = streamed.into_inner().unwrap();
    s.sort();
    assert_eq!(s, v, "streamed hits differ from the final result for {q:?}");
    v
}

fn check_corpus(root: &Path, scopes: &[PathBuf]) {
    let idx = Index::build(root).unwrap();
    idx.check();
    let mut all = Vec::new();
    walk(root, &mut all);
    assert_eq!(idx.len(), all.len() + 1, "every entry is indexed once");

    if all.len() >= 1_000 {
        for text in ["résumé", "strasse", "日本語", "🎉", "[draft]", "new\nline", "latin1", "Ｒｅｐｏｒｔ", "हिंदी"] {
            let q = Query { hidden: true, ..Query::new(text) };
            assert!(!oracle(&all, root, &q).is_empty(), "corpus has no {text:?} names");
        }
    }

    for scope in std::iter::once(root.to_path_buf()).chain(scopes.iter().cloned()) {
        let within = idx.lookup(&scope).unwrap();
        for &text in TEXTS {
            for (hidden, kind, ext) in [
                (false, KindFilter::Any, None),
                (true, KindFilter::Any, None),
                (false, KindFilter::Files, Some("pdf")),
                (true, KindFilter::Dirs, None),
            ] {
                let q = Query { within, hidden, kind, ext, ..Query::new(text) };
                let want = oracle(&all, &scope, &q);
                assert_eq!(results(&idx, &q), want, "indexed {q:?} in {}", scope.display());
                assert_eq!(live_results(&scope, &q), want, "live {q:?} in {}", scope.display());
            }
        }
        let q = Query { within, ext: Some("TAR.GZ"), ..Query::new("") };
        let want = oracle(&all, &scope, &Query { ext: Some("tar.gz"), ..q.clone() });
        assert_eq!(results(&idx, &q), want);
        assert_eq!(live_results(&scope, &q), want);
    }
}

fn flat(n: usize) {
    let tmp = tempfile::tempdir().unwrap();
    corpus::generate(tmp.path(), n, Layout::Flat).unwrap();
    check_corpus(tmp.path(), &[]);
}

#[test]
fn folder_with_100_files() {
    flat(100);
}

#[test]
fn folder_with_1k_files() {
    flat(1_000);
}

#[test]
fn folder_with_10k_files() {
    flat(10_000);
}

#[test]
fn folder_with_100k_files() {
    flat(100_000);
}

#[test]
fn tree_with_10k_files_and_scoped_search() {
    let tmp = tempfile::tempdir().unwrap();
    corpus::generate(tmp.path(), 10_000, Layout::Tree).unwrap();
    let root = tmp.path();
    check_corpus(
        root,
        &[
            root.join("Documents"),
            root.join(corpus::folder_of(0, Layout::Tree)),
            // Searching inside a hidden folder shows its (hidden-relative-to-root) files.
            root.join(".cache"),
            root.join("Ünïcödé Ordner"),
        ],
    );
}

fn touch(root: &Path, rel: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::File::create(p).unwrap();
}

fn names(idx: &Index, q: &Query) -> Vec<String> {
    idx.search(q).into_iter().map(|id| String::from_utf8_lossy(idx.name(id)).into_owned()).collect()
}

#[test]
fn ranking_puts_exact_then_prefix_then_word_then_anywhere() {
    let tmp = tempfile::tempdir().unwrap();
    for f in ["myreport.txt", "annual report.pdf", "reportage.txt", "report.pdf", "Report Final.pdf", "a/report"] {
        touch(tmp.path(), f);
    }
    let idx = Index::build(tmp.path()).unwrap();
    assert_eq!(
        names(&idx, &Query::new("report")),
        ["report", "report.pdf", "reportage.txt", "Report Final.pdf", "annual report.pdf", "myreport.txt"]
    );
    let top = names(&idx, &Query { limit: Some(3), ..Query::new("report") });
    assert_eq!(top, ["report", "report.pdf", "reportage.txt"]);
    assert!(names(&idx, &Query { limit: Some(0), ..Query::new("report") }).is_empty());
}

#[test]
fn quoted_phrase_keeps_word_order() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "annual report.pdf");
    touch(tmp.path(), "report annual.pdf");
    let idx = Index::build(tmp.path()).unwrap();
    assert_eq!(names(&idx, &Query::new("annual report")).len(), 2);
    assert_eq!(names(&idx, &Query::new("\"annual report\"")), ["annual report.pdf"]);
    assert!(names(&idx, &Query::new("\"\"")).is_empty());
    assert!(names(&idx, &Query::new("   ")).is_empty());
}

#[test]
fn similar_names_are_all_found_and_distinct() {
    let tmp = tempfile::tempdir().unwrap();
    let family = ["report.pdf", "Report.pdf", "REPORT.PDF", "report (1).pdf", "report (2).pdf", "report copy.pdf", "report.pdf.bak"];
    for f in family {
        touch(tmp.path(), f);
    }
    let idx = Index::build(tmp.path()).unwrap();
    let mut got = names(&idx, &Query::new("report"));
    got.sort();
    let mut want: Vec<String> = family.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(got, want);
    // `.pdf.bak` is not a PDF.
    assert_eq!(names(&idx, &Query { ext: Some("pdf"), ..Query::new("report") }).len(), 6);
}

#[test]
fn non_utf8_names_round_trip_to_real_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let raw = b"caf\xe9 \xff menu.txt";
    fs::File::create(tmp.path().join(OsStr::from_bytes(raw))).unwrap();
    let idx = Index::build(tmp.path()).unwrap();
    let hits = idx.search(&Query::new("MENU"));
    assert_eq!(hits.len(), 1);
    assert_eq!(idx.name(hits[0]), raw);
    assert!(idx.path(hits[0]).exists());
}

#[test]
fn symlinks_are_indexed_not_followed() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "real/inside.txt");
    std::os::unix::fs::symlink(tmp.path(), tmp.path().join("real/loop")).unwrap();
    let idx = Index::build(tmp.path()).unwrap();
    assert_eq!(idx.len(), 4);
    let link = idx.lookup(&tmp.path().join("real/loop")).unwrap();
    assert_eq!(idx.kind(link), Kind::Symlink);
}

#[test]
fn missing_root_is_an_error() {
    assert!(Index::build(Path::new("/definitely/not/here")).is_err());
}

#[test]
fn rescan_matches_a_fresh_build() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    corpus::generate(root, 3_000, Layout::Tree).unwrap();
    let mut idx = Index::build(root).unwrap();
    let leaf = root.join(corpus::folder_of(0, Layout::Tree));
    let q = |t: &'static str| Query { hidden: true, ..Query::new(t) };

    // Add files and a nested folder, then rescan the leaf folder.
    touch(&leaf, "new report 1.pdf");
    touch(&leaf, "new folder/deeper/new report 2.pdf");
    idx.rescan(&leaf).unwrap();
    idx.check();
    assert_eq!(results(&idx, &q("new report")), results(&Index::build(root).unwrap(), &q("new report")));
    assert_eq!(idx.search(&q("new report")).len(), 2);

    // Delete a folder: rescanning the gone path falls back to its parent.
    fs::remove_dir_all(leaf.join("new folder")).unwrap();
    idx.rescan(&leaf.join("new folder")).unwrap();
    idx.check();
    assert_eq!(idx.search(&q("new report")).len(), 1);

    // Rename a whole top-level folder and rescan the root.
    fs::rename(root.join("Documents"), root.join("Papers")).unwrap();
    idx.rescan(root).unwrap();
    idx.check();
    let fresh = Index::build(root).unwrap();
    assert_eq!(idx.len(), fresh.len());
    for t in ["report", "résumé", "papers", "documents"] {
        assert_eq!(results(&idx, &q(t)), results(&fresh, &q(t)), "{t}");
    }

    // Shrinking splice: empty a big folder.
    let photos = root.join("Photos 2026");
    for e in fs::read_dir(&photos).unwrap() {
        fs::remove_dir_all(e.unwrap().path()).unwrap();
    }
    idx.rescan(&photos).unwrap();
    idx.check();
    assert_eq!(idx.descendants(idx.lookup(&photos).unwrap()), 0);
    assert_eq!(idx.len(), Index::build(root).unwrap().len());

    assert!(idx.rescan(Path::new("/elsewhere")).is_err());
}

#[test]
fn save_and_load_give_the_same_answers() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("c");
    corpus::generate(&root, 5_000, Layout::Tree).unwrap();
    let idx = Index::build(&root).unwrap();
    let file = tmp.path().join("i.efidx");
    idx.save(&file).unwrap();
    let back = Index::load(&file).unwrap();
    back.check();
    assert_eq!(back.root(), idx.root());
    for &t in TEXTS {
        let q = Query { hidden: true, ..Query::new(t) };
        assert_eq!(back.search(&q), idx.search(&q), "{t}");
    }

    // Truncated or foreign files are rejected, not trusted.
    let bytes = fs::read(&file).unwrap();
    fs::write(&file, &bytes[..bytes.len() - 1]).unwrap();
    assert!(Index::load(&file).is_err());
    fs::write(&file, b"EFIDX001 old prototype format").unwrap();
    assert!(Index::load(&file).is_err());
}

#[test]
fn parallel_search_with_limit_equals_the_top_of_the_full_ranking() {
    // 100k entries is above the parallel threshold, so chunks rank separately.
    let tmp = tempfile::tempdir().unwrap();
    corpus::generate(tmp.path(), 100_000, Layout::Flat).unwrap();
    let idx = Index::build(tmp.path()).unwrap();
    for text in ["e", "report", "résumé"] {
        let all = idx.search(&Query { hidden: true, ..Query::new(text) });
        assert!(all.len() > 100, "{text}");
        for k in [1, 50, 1000] {
            let top = idx.search(&Query { hidden: true, limit: Some(k), ..Query::new(text) });
            assert_eq!(top, all[..k], "{text} top {k}");
        }
    }
}

#[test]
fn live_search_ranks_like_the_index() {
    let tmp = tempfile::tempdir().unwrap();
    for f in ["myreport.txt", "annual report.pdf", "reportage.txt", "report.pdf", "Report Final.pdf", "a/report"] {
        touch(tmp.path(), f);
    }
    let q = Query::new("report");
    let got: Vec<String> = live::search(tmp.path(), &q, &AtomicBool::new(false), &|_| {})
        .unwrap()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(got, ["report", "report.pdf", "reportage.txt", "Report Final.pdf", "annual report.pdf", "myreport.txt"]);
    let top = live::search(tmp.path(), &Query { limit: Some(2), ..q }, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(top.len(), 2);
}

#[test]
fn live_search_can_be_cancelled_and_needs_a_real_root() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "a/report.txt");
    let err = live::search(tmp.path(), &Query::new("report"), &AtomicBool::new(true), &|_| {}).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::Interrupted);
    assert!(live::search(Path::new("/definitely/not/here"), &Query::new("x"), &AtomicBool::new(false), &|_| {}).is_err());
    // Symlink loops don't hang it.
    std::os::unix::fs::symlink(tmp.path(), tmp.path().join("a/loop")).unwrap();
    assert_eq!(live::search(tmp.path(), &Query::new("report"), &AtomicBool::new(false), &|_| {}).unwrap().len(), 1);
}

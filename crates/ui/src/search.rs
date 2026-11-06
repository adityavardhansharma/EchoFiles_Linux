//! "Everywhere" search (design system `SearchResults`), run by the same engine as `ef`:
//! answered from the index where there is one and read live elsewhere, with the search-box
//! language (`report ext:pdf size:>1m modified:<7d in:~/Work`, see `ef_index::cli::query`).
//! "This folder" search is the file list's own filter (app.rs).
//!
//! Like before, it shows everything Settings → Search indexes: hidden files follow the
//! "show hidden" toggle, `.gitignore` rules don't hide anything (`ignored:no` turns them
//! on), and folders excluded in Settings stay out even where a folder is read live.

use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use ef_config::{self as config, Settings};
use ef_index::cli::args::{self, Flavor, Sort};
use ef_index::cli::{self, Ctx, agent, find, query};
use ef_index::filter::FType;
use ef_index::find::Source;

/// Results shown at most; the index answers the rest instantly if the user narrows down.
pub const LIMIT: usize = 300;

#[derive(Debug, Clone)]
pub struct Hit {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    /// Parent folder, written the way people do (`~/Work/2026`).
    pub location: String,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Clone)]
pub struct Results {
    pub query: String,
    pub hits: Vec<Hit>,
    /// Matches before the display limit.
    pub total: usize,
    pub from_index: bool,
    pub index_updated: Option<SystemTime>,
    /// A problem with the query, or — when nothing matched — what would have.
    pub note: Option<String>,
}

impl Results {
    fn empty(text: &str, note: String) -> Results {
        Results { query: text.to_string(), hits: Vec::new(), total: 0, from_index: false, index_updated: None, note: Some(note) }
    }
}

/// "Google Drive › Work", or "Google Drive · offline" for a Drive result that isn't mounted.
pub fn drive_location(settings: &Settings, path: &std::path::Path) -> Option<String> {
    let text = path.to_string_lossy();
    let (uri, mounted) = match ef_cloud::DriveUri::from_mount_path(path) {
        Some(u) => (u, true),
        None => (ef_cloud::DriveUri::parse(&text)?, false),
    };
    let name = settings.cloud.accounts.iter().find(|a| a.id == uri.account).map_or("Google Drive", |a| a.name.as_str());
    let parent = uri.parent().map(|p| p.path).unwrap_or_default();
    let mut s = name.to_string();
    if !mounted {
        s.push_str(" · offline");
    } else if !parent.is_empty() {
        s.push_str(" › ");
        s.push_str(&parent.replace('/', " › "));
    }
    Some(s)
}

fn hit(path: PathBuf, is_dir: bool) -> Hit {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let location = path.parent().map(config::tilde).unwrap_or_default();
    Hit { path, name, is_dir, location, size: 0, mtime: 0 }
}

/// Sizes and dates for hits the search didn't stat (read live, ≤ LIMIT `statx` calls).
fn fill_details(hits: &mut [Hit]) {
    use std::os::unix::fs::MetadataExt;
    for h in hits.iter_mut() {
        if h.mtime == 0
            && let Ok(m) = std::fs::symlink_metadata(&h.path)
        {
            h.size = if h.is_dir { 0 } else { m.len() };
            h.mtime = m.mtime();
        }
    }
}

/// The search-box text as `ef find` options with the app's defaults.
fn query_args(text: &str, hidden: bool) -> Result<args::FindArgs, String> {
    let mut a = args::parse(Flavor::Ef, &[])?;
    a.hidden = hidden;
    a.no_ignore = true;
    a.respect_settings = true;
    a.sort = Some(Sort::Rank);
    // Only the best LIMIT are shown: the index counts the rest without building them.
    a.max_results = Some(LIMIT);
    query::apply(text, &mut a)?;
    Ok(a)
}

fn context(settings: &Settings, indexes: Arc<find::Indexes>) -> Ctx {
    Ctx {
        cwd: config::home(),
        out: cli::stream(std::io::sink()),
        err: cli::stream(std::io::sink()),
        out_tty: false,
        ls_colors: None,
        no_color: true,
        settings: Arc::new(settings.clone()),
        indexes,
        served: true,
        // The app's indexer keeps the index current while the app runs.
        app_running: true,
    }
}

/// Search everywhere Settings → Search covers (or where `in:` says). `None` if the search
/// was cancelled.
pub fn run(settings: &Settings, text: &str, hidden: bool, cancel: &Arc<AtomicBool>) -> Option<Results> {
    run_with(settings, cli::shared_indexes(), text, hidden, cancel)
}

fn run_with(settings: &Settings, indexes: Arc<find::Indexes>, text: &str, hidden: bool, cancel: &Arc<AtomicBool>) -> Option<Results> {
    let a = match query_args(text, hidden) {
        Ok(a) => a,
        Err(e) => return Some(Results::empty(text, e)),
    };
    let ctx = context(settings, indexes);
    let mut s = match find::build_search(&a) {
        Ok(s) => s,
        Err(e) => return Some(Results::empty(text, e)),
    };
    s.cancel = cancel.clone();
    let mut ran = match find::run(&a, &ctx, &s) {
        Ok(r) => r,
        Err(e) => return Some(Results::empty(text, e)),
    };
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    if ran.everywhere && settings.phone.phone_index {
        agent::add_phone_hits(&a, &mut ran);
    }
    // Your own search: Drive's index switch alone decides (ef's Drive switch is for agents).
    if ran.everywhere && settings.cloud.index {
        agent::add_drive_hits(&a, &mut ran);
    }
    let hits = &mut ran.outcome.hits;
    hits.sort_unstable_by(|x, y| x.path.cmp(&y.path));
    hits.dedup_by(|x, y| x.path == y.path);
    let total = hits.len() + ran.outcome.dropped;
    find::sort_hits(hits, Sort::Rank);
    hits.truncate(LIMIT);
    let mut shown: Vec<Hit> = hits
        .iter()
        .map(|h| {
            let path = PathBuf::from(std::ffi::OsStr::from_bytes(&h.path));
            let mut out = hit(path, h.ftype == FType::Dir);
            if h.path.starts_with(b"phone://") {
                out.location = "Phone · cached; reconnect to open".into();
            } else if let Some(loc) = drive_location(settings, &out.path) {
                out.location = loc;
                if let Some(stem) = out.name.strip_suffix(ef_cloud::drive::LINK_SUFFIX) {
                    out.name = stem.to_string();
                }
            }
            if let Some(m) = &h.meta {
                out.size = if out.is_dir { 0 } else { m.size };
                out.mtime = ef_index::units::to_unix(m.mtime);
            }
            out
        })
        .collect();
    fill_details(&mut shown);
    let from_index = !ran.outcome.roots.is_empty() && ran.outcome.roots.iter().all(|r| r.source == Source::Index);
    let index_updated = ran.index_roots.iter().flatten().filter_map(|r| find::Indexes::age(r)).max().map(|d| SystemTime::now() - d);
    let note = (total == 0).then(|| agent::why_nothing(&a, &ctx, &s)).filter(|n| !n.is_empty()).map(|n| n.join(" · "));
    Some(Results { query: text.to_string(), hits: shown, total, from_index, index_updated, note })
}

#[cfg(test)]
mod review_tests {
    use super::*;

    fn indexed(dirs: &[&std::path::Path], tmp: &std::path::Path) -> (Settings, Arc<find::Indexes>) {
        let mut settings = Settings::default();
        settings.search.roots = dirs.iter().map(|d| d.display().to_string()).collect();
        // Only these folders, not the drives of the machine running the tests.
        settings.search.index_drives = false;
        let maps = dirs
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let file = tmp.join(format!("{i}.index"));
                ef_index::Index::build_with(p, &ef_index::Options::from_search(&settings.search, p)).unwrap().save(&file).unwrap();
                (std::fs::canonicalize(p).unwrap(), Arc::new(ef_index::MappedIndex::open(&file).unwrap()))
            })
            .collect();
        (settings, Arc::new(find::Indexes::preloaded(maps)))
    }

    #[test]
    fn exact_match_in_later_root_survives_global_limit() {
        let root = tempfile::tempdir().unwrap();
        let (a, b) = (root.path().join("a"), root.path().join("b"));
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        for i in 0..300 {
            std::fs::write(a.join(format!("some-report-{i}")), b"").unwrap();
        }
        std::fs::write(b.join("report"), b"").unwrap();
        let (settings, idx) = indexed(&[&a, &b], root.path());
        let found = run_with(&settings, idx, "report", false, &Arc::default()).unwrap();
        assert!(found.from_index);
        assert_eq!(found.total, 301);
        assert_eq!(found.hits.len(), 300);
        assert_eq!(found.hits[0].name, "report");
    }

    #[test]
    fn filters_settings_exclusions_and_notes() {
        let root = tempfile::tempdir().unwrap();
        let docs = root.path().join("docs");
        std::fs::create_dir_all(docs.join("node_modules/pkg")).unwrap();
        std::fs::write(docs.join("node_modules/pkg/report.pdf"), b"x").unwrap();
        std::fs::write(docs.join("report.pdf"), vec![0u8; 2000]).unwrap();
        std::fs::write(docs.join("report.txt"), b"x").unwrap();
        std::fs::write(docs.join(".report.pdf"), b"x").unwrap();
        let (settings, idx) = indexed(&[&docs], root.path());
        let names = |text: &str, hidden: bool| -> Vec<String> {
            let mut v: Vec<String> = run_with(&settings, idx.clone(), text, hidden, &Arc::default()).unwrap().hits.into_iter().map(|h| h.name).collect();
            v.sort();
            v
        };
        // Settings exclude node_modules contents; hidden follows the toggle.
        assert_eq!(names("report", false), ["report.pdf", "report.txt"]);
        assert_eq!(names("report", true), [".report.pdf", "report.pdf", "report.txt"]);
        // The search-box language.
        assert_eq!(names("report ext:pdf", false), ["report.pdf"]);
        assert_eq!(names("ext:pdf size:>1k", false), ["report.pdf"]);
        assert_eq!(names("report type:folder", false), Vec::<String>::new());
        // Read live (index off) gives the same answers.
        let mut live = settings.clone();
        live.search.index = false;
        let r = run_with(&live, Arc::new(find::Indexes::default()), "report", false, &Arc::default()).unwrap();
        assert!(!r.from_index);
        assert_eq!(r.total, 2);
        // A bad filter explains itself; nothing found says what would match.
        let bad = run_with(&settings, idx.clone(), "size:>huge", false, &Arc::default()).unwrap();
        assert!(bad.note.unwrap().contains("size"));
        let none = run_with(&settings, idx.clone(), "report ext:md", false, &Arc::default()).unwrap();
        assert!(none.note.unwrap().contains("extension"));
        // Cancelled searches return nothing.
        let cancel = Arc::new(AtomicBool::new(true));
        assert!(run_with(&settings, idx, "report", false, &cancel).is_none());
    }
}

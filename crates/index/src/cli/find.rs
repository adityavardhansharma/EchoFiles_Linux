//! `ef find` and `ef fd`: build the search from the arguments, pick index or live per
//! root, run it, rank, and print or execute. Plain fd-style output streams while the
//! search runs.

use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::config::{self, Settings};
use crate::filter::Hit;
use crate::find::{self as engine, Outcome, RootSpec, Search, Sink, Source};
use crate::gitignore::IgnoreOptions;
use crate::pattern::{Pattern, Syntax};
use crate::units::{self, OwnerFilter};
use crate::{MappedIndex, Options};

use super::Ctx;
use super::args::{FindArgs, Flavor, Sort, When};
use super::output::{self, RootView, Style};
use super::{agent, exec};

pub fn err(ctx: &Ctx, flavor: Flavor, msg: impl std::fmt::Display) -> u8 {
    match flavor {
        Flavor::Fd => errln!(ctx, "[ef error]: {msg}"),
        Flavor::Ef => errln!(ctx, "ef: {msg}"),
    }
    1
}

/// `/a/./b/` → `/a/b` (lexically; `..` is kept, as fd does).
fn lexical_abs(p: &Path, cwd: &Path) -> PathBuf {
    let joined = if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) };
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The innermost folder at or above `from` that looks like a project root.
pub fn project_root(from: &Path) -> Option<PathBuf> {
    const MARKERS: &[&str] = &[".git", "Cargo.toml", "package.json", "pyproject.toml", "go.mod", "build.gradle", "build.gradle.kts", "pom.xml", "Gemfile", "composer.json", "mix.exs", "deno.json", "Makefile", "CMakeLists.txt", ".hg"];
    let mut best = None;
    for dir in from.ancestors() {
        if MARKERS.iter().any(|m| dir.join(m).exists()) {
            best = Some(dir.to_path_buf());
            // `.git` marks the outermost root of a repository; stop there.
            if dir.join(".git").exists() {
                return best;
            }
        }
    }
    best
}

/// Whether the EchoFiles app (which keeps the index current) is running.
pub fn app_running() -> bool {
    std::os::unix::net::UnixStream::connect(config::socket_path()).is_ok()
}

/// The open index files, reopened only when the file on disk changes (the app's indexer
/// replaces them by rename). In the app these stay open between `ef` runs.
#[derive(Default)]
pub struct Indexes {
    slots: Mutex<Vec<Slot>>,
    /// Fixed maps instead of the files from Settings (tests).
    preloaded: Option<Vec<(PathBuf, Arc<MappedIndex>)>>,
}

struct Slot {
    file: PathBuf,
    ident: (u64, u64, u64, i64),
    canon: PathBuf,
    opts: Options,
    map: Arc<MappedIndex>,
}

impl Indexes {
    /// (indexed folder with symlinks resolved, its index) for every usable index.
    pub fn current(&self, settings: &Settings) -> Vec<(PathBuf, Arc<MappedIndex>)> {
        if let Some(p) = &self.preloaded {
            return p.clone();
        }
        if !settings.search.index {
            return Vec::new();
        }
        // A query that panicked while holding the lock leaves valid slots behind.
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        for root in settings.search.roots() {
            let file = config::index_file_for(&root);
            let Ok(m) = std::fs::metadata(&file) else { continue };
            let ident = (m.dev(), m.ino(), m.len(), m.mtime_nsec() + m.mtime() * 1_000_000_000);
            let opts = Options::from_search(&settings.search, &root);
            if let Some(slot) = slots.iter().find(|s| s.file == file && s.ident == ident) {
                if slot.opts == opts {
                    out.push((slot.canon.clone(), slot.map.clone()));
                }
                continue;
            }
            let Ok(map) = MappedIndex::open(&file) else { continue };
            if map.options() != &opts {
                continue;
            }
            let canon = std::fs::canonicalize(&root).unwrap_or(root);
            let map = Arc::new(map);
            slots.retain(|s| s.file != file);
            slots.push(Slot { file, ident, canon: canon.clone(), opts, map: map.clone() });
            out.push((canon, map));
        }
        out
    }

    /// These maps, whatever Settings says (for tests).
    pub fn preloaded(maps: Vec<(PathBuf, Arc<MappedIndex>)>) -> Indexes {
        Indexes { slots: Mutex::default(), preloaded: Some(maps) }
    }

    /// How long ago the index file for `root` was last written.
    pub fn age(root: &Path) -> Option<Duration> {
        let file = config::index_file_for(root);
        std::fs::metadata(file).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok())
    }
}

/// Everything the engine needs, built from the arguments.
pub fn build_search(a: &FindArgs) -> Result<Search, String> {
    let mut s = Search::default();
    let f = &mut s.filters;
    let syntax = a.syntax.unwrap_or(match a.flavor {
        // `ef find '*.rs'`: a pattern that looks like a glob is one.
        Flavor::Ef if a.pattern.as_deref().is_some_and(crate::pattern::is_glob_like) => Syntax::Glob,
        Flavor::Ef => Syntax::Words,
        Flavor::Fd => Syntax::Regex,
    });
    let mut full_path = a.full_path;
    if let Some(p) = &a.pattern
        && p.contains('/')
        && !full_path
    {
        match a.flavor {
            Flavor::Ef => full_path = true,
            Flavor::Fd => {
                return Err(format!(
                    "The search pattern '{p}' contains a path-separation character ('/') and will not lead to any search results.\n\n\
                     If you want to search for all files inside the '{p}' directory, use a match-all pattern:\n\n  ef fd . '{p}'\n\n\
                     Instead, if you want your pattern to match the full file path, use:\n\n  ef fd --full-path '{p}'"
                ));
            }
        }
    }
    // Smart case is decided once for all patterns (with `--and`), as fd does.
    let case = match a.case {
        crate::pattern::Case::Smart if a.pattern.iter().chain(&a.and).any(|p| crate::pattern::needs_case(p, syntax)) => crate::pattern::Case::Sensitive,
        crate::pattern::Case::Smart => crate::pattern::Case::Insensitive,
        c => c,
    };
    for p in a.pattern.iter().chain(&a.and) {
        let text = if a.flavor == Flavor::Ef && p.trim() == "*" && syntax == Syntax::Words { "" } else { p.as_str() };
        let pat = Pattern::new(text, syntax, case, full_path).map_err(|e| {
            if syntax == Syntax::Regex {
                format!("{e}\n\nNote: search for literal text with '--fixed-strings' (-F), or for a glob with '--glob' (-g).")
            } else {
                e
            }
        })?;
        if !pat.matches_all() || full_path {
            f.patterns.push(pat);
        }
    }
    f.exts = a.exts.iter().map(|e| e.to_ascii_lowercase().into_bytes()).collect();
    f.ext_matches_bare_dotfile = a.flavor == Flavor::Fd;
    f.types = a.types;
    f.sizes = a.sizes.clone();
    let now = SystemTime::now();
    if let Some(t) = &a.newer {
        f.newer = Some(units::parse_time(t, now).ok_or_else(|| format!("--changed-within: can't read '{t}' (try 2d, 10h, 2024-01-05)"))?);
    }
    if let Some(t) = &a.older {
        f.older = Some(units::parse_time(t, now).ok_or_else(|| format!("--changed-before: can't read '{t}' (try 2d, 10h, 2024-01-05)"))?);
    }
    if let Some(o) = &a.owner {
        f.owner = Some(OwnerFilter::parse(o)?);
    }
    f.min_depth = a.min_depth.unwrap_or(0);
    f.max_depth = a.max_depth;
    f.hidden = a.hidden;
    f.follow = a.follow;
    f.prune = a.prune;
    f.one_file_system = a.one_fs;
    f.ignore_contain = a.ignore_contain.iter().map(|s| s.as_bytes().to_vec()).collect();
    f.want_meta = a.json || a.details || matches!(a.sort, Some(Sort::Size | Sort::Modified));
    s.ignore = IgnoreOptions {
        vcs: !(a.no_ignore || a.no_ignore_vcs),
        dot: !a.no_ignore,
        parents: !a.no_ignore_parent,
        require_git: !a.no_require_git,
        global: !(a.no_ignore || a.no_global_ignore_file),
        case_insensitive: false,
    };
    s.excludes = a.excludes.clone();
    s.ignore_files = a.ignore_files.clone();
    s.show_errors = a.show_errors;
    s.threads = a.threads;
    // Results are ranked in `ef find`, so stopping early would keep arbitrary ones.
    let ranked = a.flavor == Flavor::Ef && matches!(a.sort, None | Some(Sort::Rank));
    s.rank = ranked;
    if a.quiet {
        s.max_results = Some(1);
    } else if !ranked {
        s.max_results = a.max_results;
    }
    Ok(s)
}

/// The search roots: (as typed, walked path, path for index lookup).
fn roots(a: &FindArgs, settings: &Settings, cwd: &Path) -> Result<(Vec<(String, PathBuf, PathBuf)>, bool), String> {
    let cwd = &cwd.to_path_buf();
    let mut typed: Vec<String> = a.paths.clone();
    let mut everywhere = false;
    if typed.is_empty() {
        if a.flavor == Flavor::Fd || a.here {
            typed.push(".".into());
        } else if a.project {
            let p = project_root(cwd).ok_or("no project folder (with .git, Cargo.toml, package.json, …) at or above the current folder")?;
            typed.push(p.to_string_lossy().into_owned());
        } else {
            everywhere = true;
            typed = settings.search.roots().iter().map(|r| r.to_string_lossy().into_owned()).collect();
        }
    }
    let mut out = Vec::new();
    for t in typed {
        let p = config::expand(&t);
        let lex = lexical_abs(&p, cwd);
        let canon = match std::fs::canonicalize(&lex) {
            Ok(c) if c.is_dir() => c,
            Ok(_) | Err(_) if everywhere => continue,
            Ok(_) => return Err(format!("Search path '{t}' is not a directory.")),
            Err(_) => return Err(format!("Search path '{t}' is not a directory.")),
        };
        let walk = if a.flavor == Flavor::Fd { lex } else { canon.clone() };
        out.push((t, walk, canon));
    }
    Ok((out, everywhere))
}

pub struct Ran {
    pub outcome: Outcome,
    pub views: Vec<RootView>,
    /// Hits before the result limit.
    pub total: usize,
    pub everywhere: bool,
    /// Where all the matches are (by folder, by type), taken before `-n` cut them.
    pub breakdown: Option<(Vec<(String, usize)>, Vec<(String, usize)>)>,
    /// The indexed folder each root was answered from (`None` when read live).
    pub index_roots: Vec<Option<PathBuf>>,
    pub stale_note: Option<String>,
}

/// The roots and how their hits print, before searching.
pub struct Plan {
    pub roots: Vec<(String, PathBuf, PathBuf)>,
    pub views: Vec<RootView>,
    pub everywhere: bool,
}

pub fn plan(a: &FindArgs, ctx: &Ctx) -> Result<Plan, String> {
    let cwd = &ctx.cwd;
    let (roots, everywhere) = roots(a, &ctx.settings, cwd)?;
    let fd_default = a.flavor == Flavor::Fd && a.paths.is_empty();
    let strip = match a.strip_cwd_prefix {
        Some(When::Always) => true,
        Some(When::Never) => false,
        _ => !(a.print0 || a.exec.is_some()),
    };
    let views = roots
        .iter()
        .map(|(typed, walk, _)| {
            let walk_b = walk.as_os_str().as_bytes().to_vec();
            let shown = if a.absolute || (a.flavor == Flavor::Ef && !a.relative) {
                walk_b.clone()
            } else if a.flavor == Flavor::Ef {
                relative_to(walk, cwd)
            } else if fd_default {
                if strip { Vec::new() } else { b".".to_vec() }
            } else {
                let t = typed.as_bytes();
                if t.len() > 1 { t.strip_suffix(b"/").unwrap_or(t).to_vec() } else { t.to_vec() }
            };
            RootView { walk: walk_b, shown }
        })
        .collect();
    Ok(Plan { roots, views, everywhere })
}

/// Run the search described by `a` (no printing, unless `s.sink` streams).
pub fn run(a: &FindArgs, ctx: &Ctx, s: &Search) -> Result<Ran, String> {
    let p = plan(a, ctx)?;
    run_planned(a, ctx, s, p)
}

pub fn run_planned(a: &FindArgs, ctx: &Ctx, s: &Search, p: Plan) -> Result<Ran, String> {
    let indexes = if a.live || a.follow { Vec::new() } else { ctx.indexes.current(&ctx.settings) };
    let running = !indexes.is_empty() && ctx.app_running;
    let mut specs = Vec::new();
    let mut stale_note = None;
    for (_, walk, canon) in &p.roots {
        let mut index = engine::index_for(canon, &indexes);
        if let Some(m) = index
            && !a.indexed
            && !running
        {
            // Without the app watching, a small folder is cheaper to read than to trust.
            let small = m.lookup(canon).is_none_or(|id| m.descendants(id) < 50_000);
            if small {
                index = None;
            } else {
                let age = Indexes::age(m.root()).map(|d| units::ago(SystemTime::now() - d, SystemTime::now())).unwrap_or_default();
                stale_note = Some(format!("index of {} (saved {age}; EchoFiles isn't running, so it may miss recent changes — add --live to read the disk)", config::tilde(m.root())));
            }
        }
        let exclusions = a.respect_settings.then(|| Options::from_search(&ctx.settings.search, canon));
        // Ranked and cut to `-n`: only the best that many are needed from this root — unless
        // another searched root overlaps it (its hits would be removed afterwards).
        let ranked = a.flavor == Flavor::Ef && matches!(a.sort, None | Some(Sort::Rank));
        let overlaps = p.roots.iter().filter(|(_, w, _)| w.starts_with(walk) || walk.starts_with(w)).count() > 1;
        // Summaries describe every match, so they need them all.
        let summarised = a.json || a.summary || a.budget.is_some();
        let keep_best = a.max_results.filter(|_| ranked && !overlaps && !a.quiet && !a.count && !summarised);
        specs.push(RootSpec { walk: walk.clone(), lookup: canon.clone(), index, exclusions, keep_best });
    }
    let mut outcome = engine::run(s, &specs);
    let index_roots = specs.iter().zip(&outcome.roots).map(|(sp, r)| (r.source == Source::Index).then(|| sp.index.map(|m| m.root().to_path_buf())).flatten()).collect();
    // Each root inside another searched root is searched on its own.
    if specs.len() > 1 {
        let walks: Vec<Vec<u8>> = specs.iter().map(|r| r.walk.as_os_str().as_bytes().to_vec()).collect();
        outcome.hits.retain(|h| {
            !walks.iter().enumerate().any(|(j, w)| {
                j != h.root as usize && w.len() > walks[h.root as usize].len() && w.starts_with(&walks[h.root as usize]) && h.path.starts_with(w) && h.path.get(w.len()) == Some(&b'/')
            })
        });
    }
    let total = outcome.hits.len() + outcome.dropped;
    Ok(Ran { outcome, views: p.views, total, everywhere: p.everywhere, breakdown: None, index_roots, stale_note })
}

fn relative_to(p: &Path, base: &Path) -> Vec<u8> {
    let pc: Vec<_> = p.components().collect();
    let bc: Vec<_> = base.components().collect();
    let common = pc.iter().zip(&bc).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in common..bc.len() {
        out.push("..");
    }
    for c in &pc[common..] {
        out.push(c.as_os_str());
    }
    if out.as_os_str().is_empty() { b".".to_vec() } else { out.as_os_str().as_bytes().to_vec() }
}

pub fn sort_hits(hits: &mut Vec<Hit>, sort: Sort) {
    // Sort a permutation, not the (large) hits themselves.
    let mut order: Vec<u32> = (0..hits.len() as u32).collect();
    let h = &*hits;
    let size = |i: u32| h[i as usize].meta.as_ref().map_or(0, |m| m.size);
    let time = |i: u32| h[i as usize].meta.as_ref().map(|m| m.mtime);
    match sort {
        Sort::Rank => order.sort_unstable_by(|&a, &b| h[a as usize].rank.cmp(&h[b as usize].rank).then_with(|| h[a as usize].path.cmp(&h[b as usize].path))),
        Sort::Path => order.sort_unstable_by(|&a, &b| h[a as usize].path.cmp(&h[b as usize].path)),
        Sort::Name => order.sort_unstable_by(|&a, &b| h[a as usize].name().cmp(h[b as usize].name()).then_with(|| h[a as usize].path.cmp(&h[b as usize].path))),
        Sort::Size => order.sort_by(|&a, &b| size(b).cmp(&size(a))),
        Sort::Modified => order.sort_by(|&a, &b| time(b).cmp(&time(a))),
        Sort::None => return,
    }
    let mut slots: Vec<Option<Hit>> = std::mem::take(hits).into_iter().map(Some).collect();
    *hits = order.into_iter().map(|i| slots[i as usize].take().unwrap()).collect();
}

fn use_color(w: When, ctx: &Ctx) -> bool {
    match w {
        When::Always => true,
        When::Never => false,
        When::Auto => !ctx.no_color && ctx.out_tty,
    }
}

/// Prints hits as they arrive from the search threads (fd's behaviour: output starts
/// before the walk ends, and nothing is kept).
struct Streamer {
    views: Vec<RootView>,
    style: Style,
    end: u8,
    max: Option<usize>,
    count_only: bool,
    printed: AtomicUsize,
    out: super::Stream,
    served: bool,
    broken: std::sync::atomic::AtomicBool,
}

impl Sink for Streamer {
    fn take(&self, hits: Vec<Hit>) {
        let n = hits.len();
        let before = self.printed.fetch_add(n, Ordering::Relaxed);
        let keep = match self.max {
            Some(m) if before >= m => 0,
            Some(m) => n.min(m - before),
            None => n,
        };
        if self.count_only || keep == 0 || self.broken.load(Ordering::Relaxed) {
            std::mem::forget(hits);
            return;
        }
        let mut buf = Vec::with_capacity(keep * 64);
        for h in &hits[..keep] {
            output::line(h, &self.views, &self.style, &mut buf);
            buf.push(self.end);
        }
        let ok = {
            let mut o = self.out.lock().unwrap();
            o.write_all(&buf).and_then(|_| o.flush())
        };
        if ok.is_err() {
            self.broken.store(true, Ordering::Relaxed);
        }
        if !self.served {
            // Freeing is wasted work in a process that is about to exit.
            std::mem::forget(hits);
        }
    }
}

fn style(a: &FindArgs, ctx: &Ctx) -> Style {
    let color = use_color(a.color, ctx) && !a.json;
    Style {
        trailing_slash: a.flavor == Flavor::Fd && a.format.is_none(),
        separator: a.path_separator.clone(),
        color: color.then(|| output::LsColors::parse(ctx.ls_colors.as_deref())),
        hyperlink: match a.hyperlink {
            When::Always => Some(output::hostname()),
            When::Auto if ctx.out_tty => Some(output::hostname()),
            _ => None,
        },
        format: a.format.as_deref().map(output::Template::parse),
        tty: ctx.out_tty,
    }
}

pub fn main(flavor: Flavor, argv: &[String], ctx: &Ctx) -> u8 {
    let a = match super::args::parse(flavor, argv).and_then(|mut a| {
        // `ef find 'report ext:pdf size:>1m'`: the search-box language.
        if flavor == Flavor::Ef
            && let Some(p) = a.pattern.clone().filter(|p| super::query::has_filters(p))
        {
            super::query::apply(&p, &mut a)?;
        }
        Ok(a)
    }) {
        Ok(a) => a,
        Err(e) => {
            err(ctx, flavor, e);
            // Usage errors exit 2, like fd (clap).
            return 2;
        }
    };
    if a.help {
        super::help(ctx, flavor);
        return 0;
    }
    if a.version {
        outln!(ctx, "ef {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    // fd's -l: GNU `ls -lhd` on all results at once, coloured like the rest of the output.
    let mut a = a;
    if a.list_details {
        let color = if use_color(a.color, ctx) { "--color=always" } else { "--color=never" };
        a.exec = Some(super::args::Exec { batch: true, cmd: ["ls", "-l", "-h", "-d", color].map(String::from).to_vec() });
    }
    let a = a;
    // `--base-directory`: a new working directory for this run only.
    let based;
    let ctx = match &a.base_dir {
        Some(dir) => {
            let d = ctx.abs(&dir.to_string_lossy());
            if !d.is_dir() {
                return err(ctx, flavor, format!("Could not set '{}' as the base directory: not a directory", dir.display()));
            }
            based = Ctx {
                cwd: d,
                out: ctx.out.clone(),
                err: ctx.err.clone(),
                out_tty: ctx.out_tty,
                ls_colors: ctx.ls_colors.clone(),
                no_color: ctx.no_color,
                settings: ctx.settings.clone(),
                indexes: ctx.indexes.clone(),
                served: ctx.served,
                app_running: ctx.app_running,
            };
            &based
        }
        None => ctx,
    };
    let mut s = match build_search(&a) {
        Ok(s) => s,
        Err(e) => return err(ctx, flavor, e),
    };
    let t0 = std::time::Instant::now();
    let plan = match plan(&a, ctx) {
        Ok(p) => p,
        Err(e) => return err(ctx, flavor, e),
    };
    let sort = a.sort.unwrap_or(match flavor {
        Flavor::Ef => Sort::Rank,
        Flavor::Fd => Sort::None,
    });
    // Unsorted plain output (fd's way) streams; so does counting.
    let streams = sort == Sort::None
        && (flavor == Flavor::Fd || plan.roots.len() == 1)
        && !(plan.everywhere && ctx.settings.phone.phone_index)
        && !a.json
        && !a.list_details
        && !a.details
        && a.budget.is_none()
        && !a.summary
        && a.exec.is_none()
        && !a.quiet;
    let stream = streams.then(|| {
        Arc::new(Streamer {
            views: plan.views.iter().map(|v| RootView { walk: v.walk.clone(), shown: v.shown.clone() }).collect(),
            style: style(&a, ctx),
            end: if a.print0 { 0 } else { b'\n' },
            max: a.max_results,
            count_only: a.count,
            printed: AtomicUsize::new(0),
            out: ctx.out.clone(),
            served: ctx.served,
            broken: Default::default(),
        })
    });
    if let Some(st) = &stream {
        s.sink = Some(st.clone());
    }
    let ran = run_planned(&a, ctx, &s, plan);
    s.sink = None;
    let mut ran = match ran {
        Ok(r) => r,
        Err(e) => return err(ctx, flavor, e),
    };
    for e in &ran.outcome.errors {
        errln!(ctx, "[ef error]: {e}");
    }
    if let Some(st) = stream {
        let printed = st.printed.load(Ordering::Relaxed);
        ran.total = printed;
        if a.count {
            outln!(ctx, "{}", a.max_results.map_or(printed, |m| printed.min(m)));
        }
        let shown = a.max_results.map_or(printed, |m| printed.min(m));
        if flavor == Flavor::Ef || a.verbose {
            agent::notes(&a, ctx, &s, &ran, shown);
        }
        if std::env::var_os("EF_TIMING").is_some() {
            errln!(ctx, "{printed} hits in {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
        }
        return finish(ctx, ran, if ran_errors(&a) { 1 } else { 0 });
    }
    if flavor == Flavor::Ef && ran.everywhere && ctx.settings.phone.phone_index {
        agent::add_phone_hits(&a, &mut ran);
    }
    if flavor == Flavor::Ef {
        ran.outcome.hits.sort_unstable_by(|x, y| x.path.cmp(&y.path));
        ran.outcome.hits.dedup_by(|x, y| x.path == y.path);
        ran.total = ran.outcome.hits.len() + ran.outcome.dropped;
    }
    sort_hits(&mut ran.outcome.hits, sort);
    if a.quiet {
        let code = if ran.outcome.hits.is_empty() { 1 } else { 0 };
        return finish(ctx, ran, code);
    }
    if a.json || a.summary || a.budget.is_some() {
        ran.breakdown = Some(output::summary(&ran.outcome.hits, &ran.views));
    }
    if let Some(n) = a.max_results {
        ran.outcome.hits.truncate(n);
    }
    if a.count {
        // With --limit, the count is of what would be shown.
        outln!(ctx, "{}", ran.outcome.hits.len());
        return finish(ctx, ran, 0);
    }
    if let Some(x) = &a.exec {
        // Commands get real paths: fd doesn't apply --path-separator to them.
        let paths: Vec<Vec<u8>> = ran.outcome.hits.iter().map(|h| output::display(h, &ran.views, false)).collect();
        let code = exec::run(x, &paths, a.path_separator.as_deref(), a.threads, a.batch_size, a.print0, ctx);
        return finish(ctx, ran, code);
    }
    let style = style(&a, ctx);
    let mut out: Vec<u8> = Vec::new();
    let mut shown = ran.outcome.hits.len();
    if a.json {
        agent::write_json(&a, &ran, ctx, &mut out);
    } else if a.format.is_some() {
        for h in &ran.outcome.hits {
            output::line(h, &ran.views, &style, &mut out);
            out.push(if a.print0 { 0 } else { b'\n' });
        }
    } else {
        let budget = a.budget.map(|t| t * 4);
        for (k, h) in ran.outcome.hits.iter().enumerate() {
            // Hand big outputs to stdout as they're built.
            if out.len() > 1 << 20 && budget.is_none() {
                if ctx.write_out(&out).is_err() {
                    return finish(ctx, ran, 0);
                }
                out.clear();
            }
            let before = out.len();
            output::line(h, &ran.views, &style, &mut out);
            if a.details && k < 200 {
                out.extend_from_slice(agent::detail_suffix(h).as_bytes());
            }
            out.push(if a.print0 { 0 } else { b'\n' });
            if let Some(b) = budget
                && out.len() > b * 3 / 4
            {
                out.truncate(before);
                shown = k;
                break;
            }
        }
        if (shown < ran.outcome.hits.len() || a.summary) && !a.print0 {
            if shown < ran.outcome.hits.len() {
                let _ = writeln!(out, "# … showing {shown} of {} — narrow the search, or see where they are:", ran.total);
            }
            let b = ran.breakdown.take().unwrap_or_else(|| output::summary(&ran.outcome.hits, &ran.views));
            output::write_summary(&b, ran.total, &mut out);
        }
    }
    if ctx.write_out(&out).is_err() {
        return finish(ctx, ran, 0);
    }
    drop(out);
    if (flavor == Flavor::Ef && !a.json) || a.verbose {
        agent::notes(&a, ctx, &s, &ran, shown);
    }
    if std::env::var_os("EF_TIMING").is_some() {
        errln!(ctx, "{} hits in {:.1} ms", ran.total, t0.elapsed().as_secs_f64() * 1000.0);
    }
    let code = if ran.outcome.errors.is_empty() || !a.show_errors { 0 } else { 1 };
    finish(ctx, ran, code)
}

fn ran_errors(_a: &FindArgs) -> bool {
    false
}

/// End a run: in a process about to exit, freeing every result path one by one is wasted
/// work, so skip it; in the app, free it after the caller has its answer.
fn finish(ctx: &Ctx, ran: Ran, code: u8) -> u8 {
    if ctx.served {
        std::thread::spawn(move || drop(ran));
    } else {
        std::mem::forget(ran);
    }
    code
}


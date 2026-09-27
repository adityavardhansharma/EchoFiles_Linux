//! The search index inside the app (docs/threading-freshness-and-next-optimizations.md,
//! Part 2): saved index files open instantly, crawls run on a low-priority pool, and a
//! watcher on the folders where new files land triggers a re-crawl within seconds. A full
//! re-crawl every minute catches everything else.

use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Instant, SystemTime};

use ef_config::{self as config, SearchConfig};
use ef_index::{Index, MappedIndex, Options};
use rustix::fd::OwnedFd;
use rustix::fs::inotify;

/// Background work (crawls, live "everywhere" search) runs here: few threads, low CPU and
/// idle I/O priority, so it never delays the folder you're opening.
pub fn background_pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .thread_name(|i| format!("ef-bg-{i}"))
            .start_handler(|_| lower_priority())
            .build()
            .expect("background pool")
    })
}

fn lower_priority() {
    // SAFETY: plain syscalls on the calling thread only.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 10);
        // ioprio_set(IOPRIO_WHO_PROCESS, this thread, IOPRIO_CLASS_IDLE)
        libc::syscall(libc::SYS_ioprio_set, 1, 0, 3 << 13);
    }
}

/// One indexed folder as the app sees it.
#[derive(Clone)]
pub struct RootIndex {
    pub root: PathBuf,
    pub map: Option<Arc<MappedIndex>>,
    pub updated: Option<SystemTime>,
    pub error: Option<String>,
}

impl std::fmt::Debug for RootIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RootIndex({}, {} entries)", self.root.display(), self.entries())
    }
}

impl RootIndex {
    pub fn entries(&self) -> usize {
        self.map.as_ref().map_or(0, |m| m.len().saturating_sub(1))
    }

    pub fn bytes(&self) -> u64 {
        self.map.as_ref().map_or(0, |m| m.mapped_bytes() as u64)
    }
}

/// Open whatever index files already exist (fast: map + validate).
pub fn open_existing(cfg: &SearchConfig) -> Vec<RootIndex> {
    cfg.roots()
        .into_iter()
        .map(|root| {
            let file = config::index_file_for(&root);
            let updated = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
            match MappedIndex::open(&file) {
                Ok(m) => RootIndex { root, map: Some(Arc::new(m)), updated, error: None },
                Err(_) => RootIndex { root, map: None, updated: None, error: None },
            }
        })
        .collect()
}

/// Crawl every indexed folder, save, and map the new files. Runs on the background pool.
pub fn build_all(cfg: &SearchConfig) -> Vec<RootIndex> {
    let cfg = cfg.clone();
    background_pool().install(move || {
        cfg.roots()
            .into_iter()
            .map(|root| {
                let file = config::index_file_for(&root);
                let built = Index::build_with(&root, &Options::from_search(&cfg, &root))
                    .and_then(|idx| std::fs::create_dir_all(config::index_dir()).and_then(|_| idx.save(&file)))
                    .and_then(|_| MappedIndex::open(&file));
                match built {
                    Ok(m) => RootIndex { root, map: Some(Arc::new(m)), updated: Some(SystemTime::now()), error: None },
                    Err(e) => RootIndex { root: root.clone(), map: None, updated: None, error: Some(format!("Couldn't index {}: {e}", config::tilde(&root))) },
                }
            })
            .collect()
    })
}

/// Delete the index files (search index turned off).
pub fn remove_files(cfg: &SearchConfig) {
    for root in cfg.roots() {
        let _ = std::fs::remove_file(config::index_file_for(&root));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexState {
    Off,
    Opening,
    Building,
    Ready,
}

// ---------------------------------------------------------------------------------------
// Watcher
// ---------------------------------------------------------------------------------------

/// A change in a watched folder.
#[derive(Debug, Clone)]
pub struct Change {
    pub folder: PathBuf,
    /// Something was created, deleted or renamed (the index only stores names).
    pub names_changed: bool,
    /// At least one changed entry isn't a dotfile.
    pub visible: bool,
}

/// inotify on a handful of folders (not recursive): where new files land, plus the folder
/// that's open.
pub struct Watcher {
    fd: Arc<OwnedFd>,
    watches: Arc<Mutex<Vec<(i32, PathBuf)>>>,
    current: Mutex<Vec<i32>>,
}

impl Watcher {
    pub fn new(on_change: impl Fn(Change) + Send + 'static) -> Option<Watcher> {
        let fd = Arc::new(inotify::init(inotify::CreateFlags::CLOEXEC).ok()?);
        let watches: Arc<Mutex<Vec<(i32, PathBuf)>>> = Arc::default();
        let (rfd, rw) = (fd.clone(), watches.clone());
        std::thread::Builder::new()
            .name("ef-watch".into())
            .spawn(move || {
                let mut buf = vec![MaybeUninit::<u8>::uninit(); 16 * 1024];
                loop {
                    let mut reader = inotify::Reader::new(&*rfd, &mut buf);
                    let mut changed: Vec<Change> = Vec::new();
                    loop {
                        match reader.next() {
                            Ok(ev) => {
                                let wd = ev.wd();
                                let names = !ev.events().contains(inotify::ReadFlags::CLOSE_WRITE);
                                let visible = ev.file_name().is_some_and(|n| n.to_bytes().first() != Some(&b'.'));
                                if let Some((_, p)) = rw.lock().unwrap().iter().find(|(w, _)| *w == wd) {
                                    match changed.iter_mut().find(|c| &c.folder == p) {
                                        Some(c) => {
                                            c.names_changed |= names;
                                            c.visible |= visible;
                                        }
                                        None => changed.push(Change { folder: p.clone(), names_changed: names, visible }),
                                    }
                                }
                                if reader.is_buffer_empty() {
                                    break;
                                }
                            }
                            Err(rustix::io::Errno::INTR) => continue,
                            Err(_) => return,
                        }
                    }
                    for c in changed {
                        on_change(c);
                    }
                }
            })
            .ok()?;
        Some(Watcher { fd, watches, current: Mutex::new(Vec::new()) })
    }

    fn add(&self, path: &Path) -> Option<i32> {
        let mask = inotify::WatchFlags::CREATE
            | inotify::WatchFlags::DELETE
            | inotify::WatchFlags::MOVED_FROM
            | inotify::WatchFlags::MOVED_TO
            | inotify::WatchFlags::CLOSE_WRITE
            | inotify::WatchFlags::ONLYDIR;
        let wd = inotify::add_watch(&*self.fd, path, mask).ok()?;
        let mut w = self.watches.lock().unwrap();
        if !w.iter().any(|(x, _)| *x == wd) {
            w.push((wd, path.to_path_buf()));
        }
        Some(wd)
    }

    /// The fixed set: where new files usually land.
    pub fn watch_landing_folders(&self) {
        let home = config::home();
        for p in [home.clone(), home.join("Downloads"), home.join("Desktop"), home.join("Documents"), home.join("Pictures"), home.join("Videos"), home.join("Music")] {
            if p.is_dir() {
                self.add(&p);
            }
        }
    }

    /// Follow the folders that are open (every visible pane), dropping the ones no longer
    /// shown unless they're landing folders.
    pub fn watch_open(&self, paths: &[PathBuf]) {
        let mut cur = self.current.lock().unwrap();
        let landing = |wd: i32| {
            let home = config::home();
            self.watches.lock().unwrap().iter().any(|(w, p)| {
                *w == wd && (p == &home || ["Downloads", "Desktop", "Documents", "Pictures", "Videos", "Music"].iter().any(|n| p == &home.join(n)))
            })
        };
        let wanted: Vec<i32> = paths.iter().filter_map(|p| self.add(p)).collect();
        for old in cur.drain(..) {
            if !wanted.contains(&old) && !landing(old) {
                let _ = inotify::remove_watch(&*self.fd, old);
                self.watches.lock().unwrap().retain(|(w, _)| *w != old);
            }
        }
        *cur = wanted;
    }
}

/// How long ago, in words ("20 s ago").
pub fn ago(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return "never".into() };
    let secs = SystemTime::now().duration_since(t).map_or(0, |d| d.as_secs());
    match secs {
        0..=4 => "just now".into(),
        5..=59 => format!("{secs} s ago"),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86_400),
    }
}

/// Debounce helper: true once `since` is at least `ms` old.
pub fn settled(since: Option<Instant>, ms: u64) -> bool {
    since.is_some_and(|t| t.elapsed().as_millis() as u64 >= ms)
}

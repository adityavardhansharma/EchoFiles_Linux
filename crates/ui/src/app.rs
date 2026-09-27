//! Application state and update logic. Views live in `view.rs` (files) and `settings.rs`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use ef_config::{self as config, Scope, Settings};
use ef_core::fmt::DateFormatter;
use ef_core::listing;
use ef_core::sort::{self, NameKeys, SortBy, SortSpec};
use ef_core::volume::{self, FsInfo};
use ef_core::Listing;
use ef_disks::Volume;
use ef_theme::Palette;
use iced::futures::channel::mpsc;
use iced::keyboard::{self, key::Named, Key};
use iced::{window, Subscription, Task};

use crate::file_list::{self, Action};
use crate::indexer::{self, IndexState, RootIndex, Watcher};
use crate::search::{self, Results};
use crate::settings::{SettingsMsg, SettingsUi};
use crate::style::Icons;
use crate::system::{self, Request};

/// A directory as far as it has been loaded. Shared with the loader thread through `Arc`.
pub struct Loaded {
    pub listing: Listing,
    pub keys: Arc<NameKeys>,
    pub metadata_ready: bool,
    /// Dotfiles in the listing, counted once on the loader thread (not per frame).
    pub hidden: usize,
    pub names_ms: f64,
    pub meta_ms: f64,
    pub sort_ms: f64,
}

impl std::fmt::Debug for Loaded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Loaded({} entries, metadata: {})", self.listing.len(), self.metadata_ready)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded { generation: u64, loaded: Arc<Loaded>, order: Arc<Vec<u32>> },
    LoadFailed { generation: u64, error: String },
    Reordered { generation: u64, order: Arc<Vec<u32>> },
    List(Action),
    Navigate(PathBuf),
    Back,
    Forward,
    Up,
    Reload,
    ToggleHidden,
    Volumes(Result<Vec<Volume>, String>),
    DriveClicked(usize),
    Key(keyboard::Event),
    Escape,
    Tick,
    Frame(Instant),
    Search(String),
    SearchSubmit,
    FocusSearch,
    SetScope(Scope),
    SearchDone { generation: u64, results: Option<Results> },
    ResultClick(usize),
    ResultOpen(usize),
    ResultReveal(usize),
    DismissNotice,
    ShowSkeleton(u64),
    IndexOpened(Vec<RootIndex>),
    IndexBuilt(Vec<RootIndex>),
    IndexTick,
    FsChanged(indexer::Change),
    Request(Request),
    WindowOpened(window::Id),
    CloseRequested(window::Id),
    WindowClosed(window::Id),
    Settings(SettingsMsg),
}

pub const SEARCH_ID: &str = "search";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Files,
    Settings,
}

struct Bench {
    frames: Vec<f64>,
    last: Option<Instant>,
    started: bool,
}

pub struct App {
    pub(crate) window: Option<window::Id>,
    pub(crate) mode: Mode,
    pub(crate) settings: Settings,
    pub(crate) settings_ui: SettingsUi,
    pub(crate) palette: Palette,
    pub(crate) icons: Icons,
    pub(crate) dates: DateFormatter,
    theme_stamp: Option<SystemTime>,
    pub(crate) location: PathBuf,
    pub(crate) back: Vec<PathBuf>,
    pub(crate) forward: Vec<PathBuf>,
    pub(crate) generation: u64,
    nav_started: Instant,
    pub(crate) first_paint_ms: f64,
    pub(crate) loaded: Option<Arc<Loaded>>,
    pub(crate) order: Arc<Vec<u32>>,
    pub(crate) show_hidden: bool,
    pub(crate) selected: Vec<u64>,
    pub(crate) cursor: Option<usize>,
    anchor: Option<usize>,
    /// Select this name once the next listing arrives ("Show in folder").
    reveal: Option<OsString>,
    pub(crate) volumes: Vec<Volume>,
    pub(crate) volume_fs: Vec<Option<FsInfo>>,
    pub(crate) notice: Option<String>,
    typeahead: (String, Instant),
    bench: Option<Bench>,
    first_frame_logged: bool,
    pub(crate) sort: SortSpec,
    pub(crate) query: String,
    pub(crate) pending: bool,
    pub(crate) skeleton: bool,
    pub(crate) fs: Option<FsInfo>,
    // search
    pub(crate) scope: Scope,
    pub(crate) results: Option<Results>,
    pub(crate) result_cursor: Option<usize>,
    pub(crate) searching: bool,
    search_generation: u64,
    search_cancel: Arc<AtomicBool>,
    // index
    pub(crate) index_state: IndexState,
    pub(crate) roots: Vec<RootIndex>,
    pub(crate) index_error: Option<String>,
    index_dirty: Option<Instant>,
    watcher: Option<Arc<Watcher>>,
}

fn theme_stamp() -> Option<SystemTime> {
    let dir = ef_theme::omarchy_theme_dir()?;
    std::fs::metadata(dir.join("colors.toml")).and_then(|m| m.modified()).ok()
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Error text per the design system: name the thing, the reason, and the fix.
fn describe(dir: &Path, e: &std::io::Error) -> String {
    let name = config::tilde(dir);
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("Couldn't open {name} — you don't have permission to read it."),
        std::io::ErrorKind::NotFound => format!("{name} no longer exists."),
        _ => format!("Couldn't open {name}: {e}"),
    }
}

/// Keep entries whose name contains `query`, ignoring ASCII case.
fn filter(l: &Listing, order: Vec<u32>, query: &str) -> Vec<u32> {
    if query.is_empty() {
        return order;
    }
    let q = query.as_bytes();
    order
        .into_iter()
        .filter(|&i| {
            let n = l.name_bytes(i as usize);
            n.len() >= q.len() && n.windows(q.len()).any(|w| w.eq_ignore_ascii_case(q))
        })
        .collect()
}

/// Stream of folder changes from the watcher thread.
static CHANGES: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<indexer::Change>>>> = OnceLock::new();

fn changes() -> impl iced::futures::Stream<Item = indexer::Change> {
    let rx = CHANGES.get().and_then(|m| m.lock().unwrap().take());
    iced::futures::stream::unfold(rx, |rx| async move {
        use iced::futures::StreamExt;
        let mut rx = rx?;
        let item = rx.next().await?;
        Some((item, Some(rx)))
    })
}

/// Run blocking work on a plain thread and deliver its result as a message.
fn background<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, to: impl Fn(T) -> Message + Send + 'static) -> Task<Message> {
    Task::perform(
        async move {
            let (tx, rx) = iced::futures::channel::oneshot::channel();
            std::thread::spawn(move || {
                let _ = tx.send(work());
            });
            rx.await
        },
        move |r| match r {
            Ok(v) => to(v),
            Err(_) => Message::Tick,
        },
    )
}

pub fn window_settings() -> window::Settings {
    window::Settings {
        size: iced::Size::new(1280.0, 800.0),
        exit_on_close_request: false,
        platform_specific: window::settings::PlatformSpecific { application_id: "echofiles".into(), ..Default::default() },
        ..Default::default()
    }
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let background_launch = args.iter().any(|a| a == "--background");
        let (settings, settings_error) = match Settings::load() {
            Ok(s) => (s, None),
            Err(e) => (Settings::default(), Some(format!("Couldn't read your settings, so defaults are used: {e}"))),
        };
        let palette = ef_theme::load_active();
        let icons = Icons::new(&palette);
        let bench_dir = std::env::var_os("ECHOFILES_BENCH").map(PathBuf::from);
        let path_arg = args.iter().find(|a| !a.starts_with("--")).map(|a| config::expand(a));
        let start = bench_dir
            .clone()
            .or(path_arg)
            .or_else(|| (settings.general.open_to == config::OpenTo::Last).then(system::last_folder).flatten())
            .unwrap_or_else(config::home);

        let (tx, rx) = mpsc::unbounded();
        CHANGES.get_or_init(|| Mutex::new(Some(rx)));
        let watcher = Watcher::new(move |c| {
            let _ = tx.unbounded_send(c);
        })
        .map(Arc::new);
        if let Some(w) = &watcher {
            w.watch_landing_folders();
            w.watch_current(&start);
        }

        let mut app = Self {
            window: None,
            mode: if args.iter().any(|a| a == "--settings") { Mode::Settings } else { Mode::Files },
            settings_ui: SettingsUi::new(settings_error),
            palette,
            icons,
            dates: DateFormatter::new(),
            theme_stamp: theme_stamp(),
            location: start.clone(),
            back: Vec::new(),
            forward: Vec::new(),
            generation: 0,
            nav_started: Instant::now(),
            first_paint_ms: 0.0,
            loaded: None,
            order: Arc::new(Vec::new()),
            show_hidden: settings.general.show_hidden,
            selected: Vec::new(),
            cursor: None,
            anchor: None,
            reveal: None,
            volumes: Vec::new(),
            volume_fs: Vec::new(),
            notice: None,
            typeahead: (String::new(), Instant::now()),
            bench: bench_dir.map(|_| Bench { frames: Vec::with_capacity(700), last: None, started: false }),
            first_frame_logged: false,
            sort: SortSpec::default(),
            query: String::new(),
            pending: true,
            skeleton: false,
            fs: volume::fs_info(&start),
            scope: settings.search.default_scope,
            results: None,
            result_cursor: None,
            searching: false,
            search_generation: 0,
            search_cancel: Arc::new(AtomicBool::new(false)),
            index_state: if settings.search.index { IndexState::Opening } else { IndexState::Off },
            roots: Vec::new(),
            index_error: None,
            index_dirty: None,
            watcher,
            settings,
        };
        let mut tasks = vec![app.load(start)];
        tasks.push(background(|| ef_disks::windows_volumes().map_err(|e| e.to_string()), Message::Volumes));
        if app.settings.search.index {
            let cfg = app.settings.search.clone();
            tasks.push(background(move || indexer::open_existing(&cfg), Message::IndexOpened));
        }
        if !background_launch {
            let (_, open) = window::open(window_settings());
            tasks.push(open.map(Message::WindowOpened));
        }
        (app, Task::batch(tasks))
    }

    pub fn title(&self, _window: window::Id) -> String {
        match self.mode {
            Mode::Settings => "Settings — EchoFiles".into(),
            Mode::Files => {
                let crumbs = self.crumbs();
                let last = crumbs.last().map(|c| c.1.clone()).unwrap_or_else(|| "EchoFiles".into());
                format!("{last} — EchoFiles")
            }
        }
    }

    // ------------------------------------------------------------------ loading

    /// Start listing `dir` on the rayon pool: names first, then metadata (build plan §2.2).
    pub(crate) fn load(&mut self, dir: PathBuf) -> Task<Message> {
        self.generation += 1;
        self.nav_started = Instant::now();
        let generation = self.generation;
        let show_hidden = self.show_hidden;
        let spec = self.sort;
        self.pending = true;
        self.fs = volume::fs_info(&dir);
        if let Some(w) = &self.watcher {
            w.watch_current(&dir);
        }
        let (tx, rx) = mpsc::unbounded();
        let timer_tx = tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            let _ = timer_tx.unbounded_send(Message::ShowSkeleton(generation));
        });
        rayon::spawn(move || {
            let send = |m: Message| {
                let _ = tx.unbounded_send(m);
            };
            let t0 = Instant::now();
            let fd = match listing::open_dir(&dir) {
                Ok(fd) => fd,
                Err(e) => return send(Message::LoadFailed { generation, error: describe(&dir, &e) }),
            };
            let names = match listing::read_names(&dir, &fd) {
                Ok(l) => l,
                Err(e) => return send(Message::LoadFailed { generation, error: describe(&dir, &e) }),
            };
            let names_ms = ms(t0);
            let hidden = (0..names.len()).filter(|&i| names.is_hidden(i)).count();
            let t1 = Instant::now();
            let keys = Arc::new(NameKeys::build(&names));
            let order = Arc::new(sort::order(&names, &keys, spec, show_hidden));
            let sort_ms = ms(t1);
            let mut full = names.clone();
            send(Message::Loaded {
                generation,
                loaded: Arc::new(Loaded { listing: names, keys: keys.clone(), metadata_ready: false, hidden, names_ms, meta_ms: 0.0, sort_ms }),
                order: order.clone(),
            });
            let t2 = Instant::now();
            listing::fill_metadata(&mut full, &fd);
            let meta_ms = ms(t2);
            let order = if matches!(spec.by, SortBy::Size | SortBy::Modified) { Arc::new(sort::order(&full, &keys, spec, show_hidden)) } else { order };
            send(Message::Loaded { generation, loaded: Arc::new(Loaded { listing: full, keys, metadata_ready: true, hidden, names_ms, meta_ms, sort_ms }), order });
        });
        Task::run(rx, |m| m)
    }

    /// Go somewhere new: clears the search, which belonged to the old folder.
    fn go(&mut self, dir: PathBuf, record: bool) -> Task<Message> {
        if dir == self.location && self.results.is_none() {
            return Task::none();
        }
        if record && dir != self.location {
            self.back.push(std::mem::replace(&mut self.location, dir.clone()));
            self.forward.clear();
        } else {
            self.location = dir.clone();
        }
        self.notice = None;
        self.clear_search();
        let d = dir.clone();
        std::thread::spawn(move || system::save_last_folder(&d));
        self.load(dir)
    }

    fn clear_search(&mut self) {
        self.query.clear();
        self.results = None;
        self.result_cursor = None;
        self.searching = false;
        self.search_cancel.store(true, Ordering::Relaxed);
    }

    pub(crate) fn reorder(&self) -> Task<Message> {
        let Some(loaded) = self.loaded.clone() else { return Task::none() };
        let generation = self.generation;
        let show_hidden = self.show_hidden;
        let spec = self.sort;
        let query = if self.scope == Scope::Folder { self.query.clone() } else { String::new() };
        background(
            move || {
                let order = sort::order(&loaded.listing, &loaded.keys, spec, show_hidden);
                filter(&loaded.listing, order, &query)
            },
            move |order| Message::Reordered { generation, order: Arc::new(order) },
        )
    }

    fn entry_at(&self, pos: usize) -> Option<usize> {
        self.order.get(pos).map(|&i| i as usize)
    }

    fn set_bit(&mut self, i: usize, on: bool) {
        if let Some(w) = self.selected.get_mut(i / 64) {
            if on { *w |= 1 << (i % 64) } else { *w &= !(1 << (i % 64)) }
        }
    }

    fn clear_selection(&mut self) {
        self.selected.iter_mut().for_each(|w| *w = 0);
    }

    /// (files, folders, bytes of the files) selected.
    pub(crate) fn selection_stats(&self) -> (usize, usize, u64) {
        let Some(l) = self.loaded.as_ref().map(|l| &l.listing) else { return (0, 0, 0) };
        let (mut files, mut dirs, mut bytes) = (0, 0, 0);
        for (wi, &w) in self.selected.iter().enumerate() {
            let mut bits = w;
            while bits != 0 {
                let i = wi * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if l.is_dir(i) {
                    dirs += 1;
                } else {
                    files += 1;
                    bytes += l.size[i];
                }
            }
        }
        (files, dirs, bytes)
    }

    fn open_path(&mut self, path: PathBuf) -> Task<Message> {
        if path.is_dir() {
            return self.go(path, true);
        }
        if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
            self.notice = Some(format!("Couldn't open {}: {e}", config::tilde(&path)));
        }
        Task::none()
    }

    fn open(&mut self, pos: usize) -> Task<Message> {
        let (Some(i), Some(l)) = (self.entry_at(pos), self.loaded.clone()) else { return Task::none() };
        self.open_path(l.listing.path(i))
    }

    fn jump_to_prefix(&mut self, c: &str) {
        let now = Instant::now();
        if now.duration_since(self.typeahead.1) > Duration::from_millis(900) {
            self.typeahead.0.clear();
        }
        self.typeahead.0.push_str(&c.to_lowercase());
        self.typeahead.1 = now;
        let Some(l) = self.loaded.as_ref().map(|l| &l.listing) else { return };
        let prefix = self.typeahead.0.as_bytes();
        if let Some(pos) = self.order.iter().position(|&i| {
            let n = l.name_bytes(i as usize);
            n.len() >= prefix.len() && n[..prefix.len()].eq_ignore_ascii_case(prefix)
        }) {
            self.select_only(pos);
        }
    }

    fn select_only(&mut self, pos: usize) {
        self.clear_selection();
        if let Some(i) = self.entry_at(pos) {
            self.set_bit(i, true);
        }
        self.cursor = Some(pos);
        self.anchor = Some(pos);
    }

    // ------------------------------------------------------------------ search

    pub(crate) fn everywhere(&self) -> bool {
        self.scope == Scope::Everywhere && !self.query.trim().is_empty()
    }

    fn run_search(&mut self) -> Task<Message> {
        self.search_cancel.store(true, Ordering::Relaxed);
        self.search_generation += 1;
        let generation = self.search_generation;
        let text = self.query.trim().to_string();
        if text.is_empty() {
            self.results = None;
            self.searching = false;
            return Task::none();
        }
        self.searching = true;
        let hidden = self.show_hidden;
        if self.settings.search.index && self.roots.iter().any(|r| r.map.is_some()) {
            let roots = self.roots.clone();
            return background(move || search::from_index(&roots, &text, hidden), move |results| Message::SearchDone { generation, results });
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.search_cancel = cancel.clone();
        let cfg = self.settings.search.clone();
        let roots = cfg.roots();
        background(move || search::live(&roots, &cfg, &text, hidden, &cancel), move |results| Message::SearchDone { generation, results })
    }

    // ------------------------------------------------------------------ index

    pub(crate) fn build_index(&mut self) -> Task<Message> {
        if !self.settings.search.index || self.index_state == IndexState::Building {
            return Task::none();
        }
        self.index_state = IndexState::Building;
        self.index_dirty = None;
        let cfg = self.settings.search.clone();
        background(move || indexer::build_all(&cfg), Message::IndexBuilt)
    }

    /// Settings that change what's indexed: rebuild soon (debounced).
    pub(crate) fn index_settings_changed(&mut self) {
        if self.settings.search.index {
            self.index_dirty = Some(Instant::now());
        }
    }

    pub(crate) fn set_index_enabled(&mut self, on: bool) -> Task<Message> {
        self.settings.search.index = on;
        if on {
            self.index_state = IndexState::Opening;
            let cfg = self.settings.search.clone();
            background(move || indexer::open_existing(&cfg), Message::IndexOpened)
        } else {
            self.index_state = IndexState::Off;
            self.roots.clear();
            self.index_error = None;
            let cfg = self.settings.search.clone();
            std::thread::spawn(move || indexer::remove_files(&cfg));
            Task::none()
        }
    }

    // ------------------------------------------------------------------ windows

    pub(crate) fn show_window(&mut self) -> Task<Message> {
        match self.window {
            Some(id) => window::gain_focus(id),
            None => {
                let (_, open) = window::open(window_settings());
                open.map(Message::WindowOpened)
            }
        }
    }

    // ------------------------------------------------------------------ update

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded { generation, loaded, order } if generation == self.generation => {
                let first = !loaded.metadata_ready;
                if first {
                    self.pending = false;
                    self.skeleton = false;
                    self.first_paint_ms = ms(self.nav_started);
                    // Keep the cursor on the same name across a refresh of the same folder.
                    let keep = self.loaded.as_ref().filter(|l| l.listing.dir == loaded.listing.dir).and_then(|l| {
                        let pos = self.cursor?;
                        let i = *self.order.get(pos)? as usize;
                        Some(l.listing.name(i).to_os_string())
                    });
                    self.selected = vec![0; loaded.listing.len().div_ceil(64)];
                    self.cursor = None;
                    self.anchor = None;
                    let want = self.reveal.take().or(keep);
                    self.loaded = Some(loaded);
                    self.order = order;
                    if let Some(name) = want {
                        let l = &self.loaded.as_ref().unwrap().listing;
                        if let Some(pos) = self.order.iter().position(|&i| l.name(i as usize) == name) {
                            self.select_only(pos);
                        }
                    }
                    if self.scope == Scope::Folder && !self.query.is_empty() {
                        return self.reorder();
                    }
                    return Task::none();
                }
                if std::env::var_os("ECHOFILES_TIMING").is_some() || self.bench.is_some() {
                    eprintln!(
                        "listing {}: {} entries · names {:.1} ms · sort {:.1} ms · metadata {:.1} ms · first paint {:.1} ms after navigation",
                        loaded.listing.dir.display(),
                        loaded.listing.len(),
                        loaded.names_ms,
                        loaded.sort_ms,
                        loaded.meta_ms,
                        self.first_paint_ms
                    );
                }
                let keep_order = self.scope == Scope::Folder && !self.query.is_empty();
                self.loaded = Some(loaded);
                if keep_order {
                    return self.reorder();
                }
                self.order = order;
                Task::none()
            }
            Message::Reordered { generation, order } if generation == self.generation => {
                self.order = order;
                self.cursor = None;
                Task::none()
            }
            Message::Loaded { .. } | Message::Reordered { .. } => Task::none(),
            Message::LoadFailed { generation, error } => {
                if generation == self.generation {
                    self.notice = Some(error);
                    self.pending = false;
                    self.skeleton = false;
                    if let Some(prev) = self.back.pop() {
                        self.location = prev;
                    }
                }
                Task::none()
            }
            Message::List(Action::Cursor { pos, extend, toggle }) => {
                if toggle {
                    if let Some(i) = self.entry_at(pos) {
                        let on = !file_list::is_selected(&self.selected, i);
                        self.set_bit(i, on);
                    }
                    self.cursor = Some(pos);
                    self.anchor = Some(pos);
                } else if extend {
                    let anchor = self.anchor.unwrap_or(pos);
                    self.clear_selection();
                    for p in anchor.min(pos)..=anchor.max(pos) {
                        if let Some(i) = self.entry_at(p) {
                            self.set_bit(i, true);
                        }
                    }
                    self.cursor = Some(pos);
                } else {
                    self.select_only(pos);
                }
                Task::none()
            }
            Message::List(Action::SelectAll) => {
                for p in 0..self.order.len() {
                    let i = self.order[p] as usize;
                    self.set_bit(i, true);
                }
                Task::none()
            }
            Message::List(Action::Open(pos)) => self.open(pos),
            Message::List(Action::Sort(by)) => {
                if self.sort.by == by {
                    self.sort.descending = !self.sort.descending;
                } else {
                    self.sort = SortSpec { by, descending: matches!(by, SortBy::Size | SortBy::Modified), ..self.sort };
                }
                self.reorder()
            }
            Message::Search(q) => {
                self.query = q;
                match self.scope {
                    Scope::Folder => self.reorder(),
                    Scope::Everywhere => self.run_search(),
                }
            }
            Message::SearchSubmit => match (self.everywhere(), self.result_cursor) {
                (true, Some(i)) => self.update(Message::ResultOpen(i)),
                _ => Task::none(),
            },
            Message::SetScope(s) => {
                self.scope = s;
                self.results = None;
                self.result_cursor = None;
                let t = self.reorder();
                if s == Scope::Everywhere { Task::batch([t, self.run_search()]) } else { t }
            }
            Message::SearchDone { generation, results } => {
                if generation == self.search_generation {
                    self.searching = false;
                    if let Some(r) = results {
                        self.result_cursor = (!r.hits.is_empty()).then_some(0);
                        self.results = Some(r);
                    }
                }
                Task::none()
            }
            Message::ResultClick(i) => {
                self.result_cursor = Some(i);
                Task::none()
            }
            Message::ResultOpen(i) => match self.results.as_ref().and_then(|r| r.hits.get(i)).map(|h| h.path.clone()) {
                Some(p) => self.open_path(p),
                None => Task::none(),
            },
            Message::ResultReveal(i) => {
                let Some(h) = self.results.as_ref().and_then(|r| r.hits.get(i)) else { return Task::none() };
                let (Some(parent), Some(name)) = (h.path.parent().map(Path::to_path_buf), h.path.file_name().map(|n| n.to_os_string())) else { return Task::none() };
                self.reveal = Some(name);
                self.scope = Scope::Folder;
                self.go(parent, true)
            }
            Message::FocusSearch => iced::widget::operation::focus(SEARCH_ID),
            Message::DismissNotice => {
                self.notice = None;
                Task::none()
            }
            Message::Escape => {
                if self.mode == Mode::Settings {
                    self.mode = Mode::Files;
                    return Task::none();
                }
                if !self.query.is_empty() {
                    self.clear_search();
                    return self.reorder();
                }
                self.clear_selection();
                Task::none()
            }
            Message::Settings(m) => self.settings_update(m),
            Message::ShowSkeleton(generation) => {
                if generation == self.generation && self.pending {
                    self.skeleton = true;
                }
                Task::none()
            }
            Message::Navigate(dir) => {
                self.mode = Mode::Files;
                self.go(dir, true)
            }
            Message::Back => match self.back.pop() {
                Some(prev) => {
                    self.forward.push(self.location.clone());
                    self.go(prev, false)
                }
                None => Task::none(),
            },
            Message::Forward => match self.forward.pop() {
                Some(next) => {
                    self.back.push(self.location.clone());
                    self.go(next, false)
                }
                None => Task::none(),
            },
            Message::Up => match self.location.parent() {
                Some(parent) => self.go(parent.to_path_buf(), true),
                None => Task::none(),
            },
            Message::Reload => {
                let dir = self.location.clone();
                self.load(dir)
            }
            Message::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                let t = self.reorder();
                if self.everywhere() { Task::batch([t, self.run_search()]) } else { t }
            }
            Message::Volumes(result) => {
                match result {
                    Ok(v) => {
                        self.volume_fs = v.iter().map(|v| v.mount_points.first().and_then(|m| volume::fs_info(Path::new(m)))).collect();
                        self.volumes = v;
                    }
                    Err(e) => self.notice = Some(e),
                }
                Task::none()
            }
            Message::DriveClicked(i) => {
                let Some(v) = self.volumes.get(i) else { return Task::none() };
                self.mode = Mode::Files;
                match v.mount_points.first() {
                    Some(mp) => {
                        let mp = PathBuf::from(mp);
                        self.go(mp, true)
                    }
                    None => {
                        self.notice = Some(format!(
                            "{} isn't mounted yet. Mounting from EchoFiles is coming next — for now, run: udisksctl mount -b {}",
                            crate::view::drive_name(v, &self.volumes),
                            v.device
                        ));
                        Task::none()
                    }
                }
            }
            Message::Key(keyboard::Event::KeyPressed { key, modifiers, text, .. }) => {
                if self.mode == Mode::Settings {
                    return match key.as_ref() {
                        Key::Character("q") if modifiers.control() => iced::exit(),
                        _ => Task::none(),
                    };
                }
                if self.everywhere() {
                    let n = self.results.as_ref().map_or(0, |r| r.hits.len());
                    match key.as_ref() {
                        Key::Named(Named::ArrowDown) if n > 0 => {
                            self.result_cursor = Some(self.result_cursor.map_or(0, |c| (c + 1).min(n - 1)));
                            return Task::none();
                        }
                        Key::Named(Named::ArrowUp) if n > 0 => {
                            self.result_cursor = Some(self.result_cursor.map_or(0, |c| c.saturating_sub(1)));
                            return Task::none();
                        }
                        Key::Named(Named::Enter) => {
                            return match self.result_cursor {
                                Some(i) if modifiers.alt() => self.update(Message::ResultReveal(i)),
                                Some(i) => self.update(Message::ResultOpen(i)),
                                None => Task::none(),
                            };
                        }
                        _ => {}
                    }
                }
                match key.as_ref() {
                    Key::Named(Named::Backspace) => self.update(Message::Up),
                    Key::Named(Named::ArrowLeft) if modifiers.alt() => self.update(Message::Back),
                    Key::Named(Named::ArrowRight) if modifiers.alt() => self.update(Message::Forward),
                    Key::Named(Named::ArrowUp) if modifiers.alt() => self.update(Message::Up),
                    Key::Named(Named::F5) => self.update(Message::Reload),
                    Key::Character(",") if modifiers.control() => self.update(Message::Settings(SettingsMsg::Open)),
                    Key::Character("f") | Key::Character("l") if modifiers.control() => self.update(Message::FocusSearch),
                    Key::Character("/") => self.update(Message::FocusSearch),
                    Key::Character("h") if modifiers.control() => self.update(Message::ToggleHidden),
                    Key::Character("q") if modifiers.control() => iced::exit(),
                    Key::Character("e") if modifiers.control() => {
                        let s = if self.scope == Scope::Folder { Scope::Everywhere } else { Scope::Folder };
                        self.update(Message::SetScope(s))
                    }
                    _ => {
                        if !modifiers.control() && !modifiers.alt() {
                            if let Some(t) = text.as_ref().filter(|t| t.chars().all(|c| !c.is_control())) {
                                self.jump_to_prefix(t);
                            }
                        }
                        Task::none()
                    }
                }
            }
            Message::Key(_) => Task::none(),
            Message::Tick => {
                let stamp = theme_stamp();
                if stamp != self.theme_stamp {
                    self.theme_stamp = stamp;
                    self.palette = ef_theme::load_active();
                    self.icons = Icons::new(&self.palette);
                }
                if indexer::settled(self.index_dirty, 1500) && self.index_state != IndexState::Building {
                    return self.build_index();
                }
                Task::none()
            }
            Message::IndexOpened(roots) => {
                if !self.settings.search.index {
                    return Task::none();
                }
                let stale = roots.iter().any(|r| r.map.is_none() || r.updated.is_none_or(|t| t.elapsed().map_or(true, |d| d > Duration::from_secs(60))));
                self.roots = roots;
                self.index_state = if self.roots.iter().any(|r| r.map.is_some()) { IndexState::Ready } else { IndexState::Building };
                if stale {
                    self.index_state = IndexState::Ready;
                    return self.build_index();
                }
                Task::none()
            }
            Message::IndexBuilt(roots) => {
                if !self.settings.search.index {
                    // A build still running when the index was turned off saved its files
                    // after `remove_files` ran; delete them again.
                    let files: Vec<_> = roots.iter().map(|r| ef_config::index_file_for(&r.root)).collect();
                    drop(roots);
                    std::thread::spawn(move || {
                        for f in files {
                            let _ = std::fs::remove_file(f);
                        }
                    });
                    return Task::none();
                }
                self.index_error = roots.iter().find_map(|r| r.error.clone());
                self.roots = roots;
                self.index_state = IndexState::Ready;
                if self.everywhere() {
                    return self.run_search();
                }
                Task::none()
            }
            Message::IndexTick => self.build_index(),
            Message::FsChanged(change) => {
                if change.names_changed && change.visible {
                    self.index_settings_changed();
                }
                if change.folder == self.location && (change.visible || self.show_hidden) && self.mode == Mode::Files {
                    let dir = self.location.clone();
                    return self.load(dir);
                }
                Task::none()
            }
            Message::Request(req) => {
                if let Request::Open(Some(p)) = &req {
                    self.mode = Mode::Files;
                    let t = self.go(p.clone(), true);
                    return Task::batch([t, self.show_window()]);
                }
                if matches!(req, Request::Settings) {
                    self.mode = Mode::Settings;
                }
                self.show_window()
            }
            Message::WindowOpened(id) => {
                self.window = Some(id);
                Task::none()
            }
            Message::CloseRequested(id) => {
                if self.settings.general.background {
                    // Stay running: index fresh, next window instant.
                    self.window = None;
                    window::close(id)
                } else {
                    iced::exit()
                }
            }
            Message::WindowClosed(id) => {
                if self.window == Some(id) {
                    self.window = None;
                }
                if self.window.is_none() && !self.settings.general.background {
                    return iced::exit();
                }
                Task::none()
            }
            Message::Frame(now) => self.frame(now),
        }
    }

    fn frame(&mut self, now: Instant) -> Task<Message> {
        if !self.first_frame_logged {
            self.first_frame_logged = true;
            if std::env::var_os("ECHOFILES_TIMING").is_some() || self.bench.is_some() {
                eprintln!("startup: first frame {:.1} ms after process start", crate::since_start_ms());
            }
        }
        let ready = self.loaded.as_ref().is_some_and(|l| l.metadata_ready);
        let n = self.order.len();
        let Some(b) = self.bench.as_mut() else { return Task::none() };
        if !ready || n == 0 {
            return Task::none();
        }
        if !b.started {
            b.started = true;
            b.last = Some(now);
            self.cursor = Some(0);
            return Task::none();
        }
        if let Some(last) = b.last {
            b.frames.push(now.duration_since(last).as_secs_f64() * 1000.0);
        }
        b.last = Some(now);
        self.cursor = Some((self.cursor.unwrap_or(0) + 3) % n);
        if b.frames.len() >= 600 {
            let mut f = b.frames.clone();
            f.sort_by(|a, b| a.total_cmp(b));
            let pct = |p: f64| f[((f.len() - 1) as f64 * p) as usize];
            let over = f.iter().filter(|&&x| x > 17.5).count();
            eprintln!(
                "scroll bench: {} frames over {} rows · p50 {:.2} ms · p95 {:.2} ms · p99 {:.2} ms · max {:.2} ms · {} frames > 17.5 ms",
                f.len(),
                n,
                pct(0.5),
                pct(0.95),
                pct(0.99),
                f[f.len() - 1],
                over
            );
            return iced::exit();
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            keyboard::listen().map(Message::Key),
            // Esc works even while the search field has focus (it captures other keys).
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key: Key::Named(Named::Escape), .. }) => Some(Message::Escape),
                _ => None,
            }),
            iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            window::close_requests().map(Message::CloseRequested),
            window::close_events().map(Message::WindowClosed),
            Subscription::run(system::requests).map(Message::Request),
            Subscription::run(changes).map(Message::FsChanged),
        ];
        if self.settings.search.index {
            subs.push(iced::time::every(Duration::from_secs(60)).map(|_| Message::IndexTick));
        }
        if self.bench.is_some() || !self.first_frame_logged {
            subs.push(window::frames().map(Message::Frame));
        }
        Subscription::batch(subs)
    }

    pub fn view(&self, _window: window::Id) -> iced::Element<'_, Message> {
        match self.mode {
            Mode::Files => self.files_view(),
            Mode::Settings => self.settings_view(),
        }
    }
}

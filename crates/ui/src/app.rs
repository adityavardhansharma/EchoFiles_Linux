//! Application state and update loop. The folder views live in `pane.rs`; what the app
//! does to files in `actions.rs`; drives in `drives.rs`; network places in `network.rs`; menus, dialogs, toasts and the
//! command palette in `overlay.rs`; the preview pane in `preview.rs`; the files screen in
//! `view.rs` and Settings in `settings.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use ef_config::{self as config, Scope, Settings};
use ef_core::fmt::DateFormatter;
use ef_core::ops::Undo;
use ef_core::sort::{SortBy, SortSpec};
use ef_core::volume::{self, FsInfo};
use ef_disks::Volume;
use ef_theme::Palette;
use iced::futures::channel::mpsc;
use iced::keyboard::{self, key::Named, Key};
use iced::{window, Point, Subscription, Task};

use crate::actions::{Clip, FileMsg, Rename, Transfer};
use crate::drives::{DriveMsg, DriveState};
use crate::file_list::{self, Action};
use crate::indexer::{self, IndexState, RootIndex, Watcher};
use crate::network::{NetMsg, NetState};
use crate::overlay::{Command, Dialog, Menu, MenuFor, Toast, UiMsg};
use crate::pane::{Loaded, Pane};
use crate::preview::PreviewData;
use crate::search::{self, Results};
use crate::settings::{SettingsMsg, SettingsUi};
use crate::style::Icons;
use crate::system::{self, Request};
use crate::thumbs::{self, Pixels, Thumbs};

#[derive(Debug, Clone)]
pub enum Message {
    Loaded { generation: u64, loaded: Arc<Loaded>, order: Arc<Vec<u32>> },
    LoadFailed { generation: u64, error: String },
    Reordered { generation: u64, order: Arc<Vec<u32>> },
    ShowSkeleton(u64),
    List(u64, Action),
    Navigate(PathBuf),
    OpenTab(PathBuf),
    Back,
    Forward,
    Up,
    Reload,
    ToggleHidden,
    // tabs, panes and layout
    NewTab,
    CloseTab(usize),
    SelectTab(usize),
    CycleTab(bool),
    ToggleDual,
    SwitchPane,
    SetGrid(bool),
    TogglePreview,
    ToggleSidebar,
    SidebarResize(Option<f32>),
    ShowDrives,
    // search
    Search(String),
    SearchSubmit,
    FocusSearch,
    SetScope(Scope),
    SearchDone { generation: u64, results: Option<Results> },
    ResultClick(usize),
    ResultOpen(usize),
    ResultReveal(usize),
    // path bar
    EditPath(bool),
    PathDraft(String),
    PathSubmit,
    // subsystems
    File(FileMsg),
    Drive(DriveMsg),
    Net(NetMsg),
    /// Fold or unfold a sidebar section.
    Fold(crate::view::Fold, bool),
    Ui(UiMsg),
    Thumb(PathBuf, Option<Pixels>),
    Preview(PathBuf, Arc<PreviewData>),
    DismissNotice,
    Escape,
    Key(keyboard::Event),
    Tick,
    Frame(Instant),
    MouseUp,
    MouseMove(Point),
    IndexOpened(Vec<RootIndex>),
    IndexBuilt(Vec<RootIndex>),
    IndexTick,
    FsChanged(indexer::Change),
    Request(Request),
    WindowOpened(window::Id),
    CloseRequested(window::Id),
    WindowClosed(window::Id),
    Settings(SettingsMsg),
    Resized(iced::Size),
}

pub const SEARCH_ID: &str = "search";
pub const PATH_ID: &str = "path";
pub const RENAME_ID: &str = "rename";

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

/// One tab: a pane, or two side by side.
pub struct Tab {
    pub panes: Vec<Pane>,
    pub active: usize,
}

/// Files being dragged inside the window.
pub struct Drag {
    pub paths: Vec<PathBuf>,
    pub from: u64,
    pub at: Point,
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
    pub(crate) tabs: Vec<Tab>,
    pub(crate) tab: usize,
    pub(crate) show_hidden: bool,
    pub(crate) volumes: Vec<Volume>,
    pub(crate) volume_fs: Vec<Option<FsInfo>>,
    pub(crate) drive_state: HashMap<String, DriveState>,
    pub(crate) net: NetState,
    pub(crate) notice: Option<String>,
    bench: Option<Bench>,
    first_frame_logged: bool,
    // index
    pub(crate) index_state: IndexState,
    pub(crate) roots: Vec<RootIndex>,
    pub(crate) index_error: Option<String>,
    index_dirty: Option<Instant>,
    watcher: Option<Arc<Watcher>>,
    // files
    pub(crate) clip: Option<Clip>,
    pub(crate) undo: Vec<Undo>,
    pub(crate) rename: Option<Rename>,
    pub(crate) transfers: Vec<Transfer>,
    pub(crate) drag: Option<Drag>,
    pub(crate) sidebar_drag: Option<(f32, f32)>,
    pub(crate) drop_place: Option<PathBuf>,
    // overlays
    pub(crate) menu: Option<Menu>,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) command: Option<Command>,
    pub(crate) path_edit: Option<String>,
    // preview and thumbnails
    pub(crate) thumbs: Thumbs,
    pub(crate) preview: Option<(PathBuf, Arc<PreviewData>)>,
    pub(crate) preview_want: Option<PathBuf>,
    pub(crate) view_modes: HashMap<PathBuf, bool>,
    /// Recently visited folders, newest last (command palette "Go to").
    pub(crate) recent: Vec<PathBuf>,
    pub(crate) animations: bool,
    pub(crate) mouse: Point,
    pub(crate) modifiers: keyboard::Modifiers,
    pub(crate) window_size: iced::Size,
    /// Background size of the selected folders: (what was measured, totals, cancel).
    pub(crate) sel_size: Option<(Vec<PathBuf>, Arc<ef_core::ops::DirSize>, Arc<AtomicBool>)>,
}

fn theme_stamp() -> Option<SystemTime> {
    let dir = ef_theme::omarchy_theme_dir()?;
    std::fs::metadata(dir.join("colors.toml")).and_then(|m| m.modified()).ok()
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
pub fn background<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, to: impl Fn(T) -> Message + Send + 'static) -> Task<Message> {
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
        // ECHOFILES_APP_ID lets a development build run beside the installed one.
        platform_specific: window::settings::PlatformSpecific { application_id: std::env::var("ECHOFILES_APP_ID").unwrap_or_else(|_| "echofiles".into()), ..Default::default() },
        ..Default::default()
    }
}

fn view_modes_file() -> PathBuf {
    config::state_dir().join("view-modes")
}

fn load_view_modes() -> HashMap<PathBuf, bool> {
    std::fs::read_to_string(view_modes_file())
        .map(|t| t.lines().filter_map(|l| l.split_once('\t')).map(|(m, p)| (PathBuf::from(p), m == "grid")).collect())
        .unwrap_or_default()
}

/// Hyprland `animations:enabled = false` turns every EchoFiles animation off too.
fn animations_enabled() -> bool {
    let Ok(out) = std::process::Command::new("hyprctl").args(["-j", "getoption", "animations:enabled"]).output() else { return true };
    !String::from_utf8_lossy(&out.stdout).contains("\"int\": 0")
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
        let launch = args.iter().find(|a| !a.starts_with("--")).map(|a| system::request_for(a));
        let path_arg = match &launch {
            Some(Request::Open(Some(p))) => Some(p.clone()),
            _ => None,
        };
        // A network folder from last time may not be connected now (and asking a gone
        // server can stall), so those start at home.
        let last = (settings.general.open_to == config::OpenTo::Last).then(system::last_folder).flatten().filter(|p| !ef_net::gvfs::is_network_path(p));
        let start = bench_dir.clone().or(path_arg).or(last).unwrap_or_else(config::home);

        let (tx, rx) = mpsc::unbounded();
        CHANGES.get_or_init(|| Mutex::new(Some(rx)));
        let watcher = Watcher::new(move |c| {
            let _ = tx.unbounded_send(c);
        })
        .map(Arc::new);
        if let Some(w) = &watcher {
            w.watch_landing_folders();
            w.watch_open(std::slice::from_ref(&start));
        }

        let pane = Pane::new(start.clone(), settings.search.default_scope);
        let mut app = Self {
            window: None,
            mode: if args.iter().any(|a| a == "--settings") { Mode::Settings } else { Mode::Files },
            settings_ui: SettingsUi::new(settings_error),
            palette,
            icons,
            dates: DateFormatter::new(),
            theme_stamp: theme_stamp(),
            tabs: vec![Tab { panes: vec![pane], active: 0 }],
            tab: 0,
            show_hidden: settings.general.show_hidden,
            volumes: Vec::new(),
            volume_fs: Vec::new(),
            drive_state: HashMap::new(),
            net: NetState { recent: crate::network::load_recent(), ..Default::default() },
            notice: None,
            bench: bench_dir.map(|_| Bench { frames: Vec::with_capacity(700), last: None, started: false }),
            first_frame_logged: false,
            index_state: if settings.search.index { IndexState::Opening } else { IndexState::Off },
            roots: Vec::new(),
            index_error: None,
            index_dirty: None,
            watcher,
            clip: None,
            undo: Vec::new(),
            rename: None,
            transfers: Vec::new(),
            drag: None,
            sidebar_drag: None,
            drop_place: None,
            menu: None,
            dialog: None,
            toasts: Vec::new(),
            command: None,
            path_edit: None,
            thumbs: Thumbs::default(),
            preview: None,
            preview_want: None,
            view_modes: load_view_modes(),
            recent: Vec::new(),
            animations: true,
            mouse: Point::ORIGIN,
            modifiers: keyboard::Modifiers::default(),
            window_size: iced::Size::new(1280.0, 800.0),
            sel_size: None,
            settings,
        };
        let mut tasks = vec![app.load_active(start)];
        tasks.push(background(|| ef_disks::windows_volumes().map_err(|e| e.to_string()), |r| Message::Drive(DriveMsg::Volumes(r))));
        tasks.push(background(animations_enabled, |on| Message::Drive(DriveMsg::Animations(on))));
        tasks.push(crate::network::boot_tasks());
        if let Some(Request::Connect(uri)) = launch {
            match ef_net::Address::parse(&uri, ef_net::Protocol::Smb) {
                Ok(address) => tasks.push(app.net_update(NetMsg::Connect { address, save: false })),
                Err(e) => app.toast_error(format!("Can't open {uri}"), e),
            }
        }
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
            Mode::Files => format!("{} — EchoFiles", self.pane_title(self.pane())),
        }
    }

    /// A pane's name for tabs and the window title; network roots use the name people
    /// gave the place, not GVfs' folder name.
    pub(crate) fn pane_title(&self, pane: &Pane) -> String {
        if !pane.special()
            && let Some((m, name)) = self.net_place_at(&pane.location)
            && pane.location == m.root
        {
            return name;
        }
        pane.title()
    }

    // ------------------------------------------------------------------ panes

    pub(crate) fn pane(&self) -> &Pane {
        let t = &self.tabs[self.tab];
        &t.panes[t.active]
    }

    pub(crate) fn pane_mut(&mut self) -> &mut Pane {
        let t = &mut self.tabs[self.tab];
        &mut t.panes[t.active]
    }

    pub(crate) fn pane_by_id(&self, id: u64) -> Option<&Pane> {
        self.tabs.iter().flat_map(|t| t.panes.iter()).find(|p| p.id == id)
    }

    pub(crate) fn pane_by_id_mut(&mut self, id: u64) -> Option<&mut Pane> {
        self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()).find(|p| p.id == id)
    }

    /// The other pane of a split tab.
    pub(crate) fn other_pane(&self) -> Option<&Pane> {
        let t = &self.tabs[self.tab];
        (t.panes.len() == 2).then(|| &t.panes[1 - t.active])
    }

    pub(crate) fn dual(&self) -> bool {
        self.tabs[self.tab].panes.len() == 2
    }

    fn all_panes(&self) -> impl Iterator<Item = &Pane> {
        self.tabs.iter().flat_map(|t| t.panes.iter())
    }

    /// Watch every folder on screen in this tab.
    fn watch_visible(&self) {
        if let Some(w) = &self.watcher {
            let open: Vec<PathBuf> = self.tabs[self.tab].panes.iter().map(|p| p.location.clone()).collect();
            w.watch_open(&open);
        }
    }

    /// The folder's remembered view, else Settings → Appearance → Folders open as.
    fn grid_for(&self, dir: &Path) -> bool {
        if let Some(&g) = self.view_modes.get(dir) {
            return g;
        }
        match self.settings.appearance.view {
            config::DefaultView::List => return false,
            config::DefaultView::Grid => return true,
            config::DefaultView::Auto => {}
        }
        let home = config::home();
        let name = dir.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        dir == home.join("Pictures") || dir == home.join("Videos") || matches!(name.as_str(), "dcim" | "camera" | "screenshots" | "photos" | "wallpapers")
    }

    /// The default view changed: forget per-folder choices and re-apply it everywhere.
    pub(crate) fn apply_default_view(&mut self) {
        self.view_modes.clear();
        let _ = std::fs::remove_file(view_modes_file());
        let grids: Vec<(u64, bool)> = self.all_panes().map(|p| (p.id, self.grid_for(&p.location))).collect();
        for (id, g) in grids {
            if let Some(p) = self.pane_by_id_mut(id) {
                p.grid = g;
            }
        }
    }

    /// (Re)load the active pane's folder.
    pub(crate) fn load_active(&mut self, dir: PathBuf) -> Task<Message> {
        let hidden = self.show_hidden;
        let grid = self.grid_for(&dir);
        let pane = self.pane_mut();
        pane.grid = grid;
        let t = pane.load(dir, hidden);
        self.watch_visible();
        t
    }

    fn reload_pane(&mut self, id: u64) -> Task<Message> {
        let hidden = self.show_hidden;
        match self.pane_by_id_mut(id) {
            Some(p) => {
                let dir = p.location.clone();
                p.load(dir, hidden)
            }
            None => Task::none(),
        }
    }

    /// Reload every pane showing `dir` (after we changed something there).
    pub(crate) fn refresh(&mut self, dir: &Path) -> Task<Message> {
        let ids: Vec<u64> = self.all_panes().filter(|p| p.location == dir && !p.special()).map(|p| p.id).collect();
        Task::batch(ids.into_iter().map(|id| self.reload_pane(id)))
    }

    /// Go somewhere new in the active pane: clears the search, which belonged to the old folder.
    pub(crate) fn go(&mut self, dir: PathBuf, record: bool) -> Task<Message> {
        self.mode = Mode::Files;
        self.rename = None;
        self.path_edit = None;
        let pane = self.pane_mut();
        if dir == pane.location && pane.results.is_none() && !pane.special() {
            return Task::none();
        }
        if record && (dir != pane.location || pane.special()) {
            let old = std::mem::replace(&mut pane.location, dir.clone());
            pane.back.push(old);
            pane.forward.clear();
        } else {
            pane.location = dir.clone();
        }
        pane.drives = false;
        pane.shares = None;
        pane.clear_search();
        self.notice = None;
        self.recent.retain(|p| p != &dir);
        self.recent.push(dir.clone());
        if self.recent.len() > 30 {
            self.recent.remove(0);
        }
        let d = dir.clone();
        std::thread::spawn(move || system::save_last_folder(&d));
        self.load_active(dir)
    }

    pub(crate) fn open_path(&mut self, path: PathBuf) -> Task<Message> {
        if path.is_dir() {
            return self.go(path, true);
        }
        if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
            self.toast_error(format!("Couldn't open {}", path.file_name().unwrap_or_default().to_string_lossy()), e.to_string());
        }
        Task::none()
    }

    fn open(&mut self, pos: usize) -> Task<Message> {
        let pane = self.pane();
        let (Some(i), Some(l)) = (pane.entry_at(pos), pane.loaded.clone()) else { return Task::none() };
        // A folder in the Trash can't usefully be browsed into and "opened" as a file.
        self.open_path(l.listing.path(i))
    }

    pub(crate) fn everywhere(&self) -> bool {
        self.pane().everywhere()
    }

    fn run_search(&mut self) -> Task<Message> {
        let hidden = self.show_hidden;
        let use_index = self.settings.search.index && self.roots.iter().any(|r| r.map.is_some());
        let roots = self.roots.clone();
        let cfg = self.settings.search.clone();
        let pane = self.pane_mut();
        pane.search_cancel.store(true, Ordering::Relaxed);
        pane.search_generation = crate::pane::next_id();
        let generation = pane.search_generation;
        let text = pane.query.trim().to_string();
        if text.is_empty() {
            pane.results = None;
            pane.searching = false;
            return Task::none();
        }
        pane.searching = true;
        if use_index {
            return background(move || search::from_index(&roots, &text, hidden), move |results| Message::SearchDone { generation, results });
        }
        let cancel = Arc::new(AtomicBool::new(false));
        pane.search_cancel = cancel.clone();
        let dirs = cfg.roots();
        background(move || search::live(&dirs, &cfg, &text, hidden, &cancel), move |results| Message::SearchDone { generation, results })
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

    /// Something modal is up: lists don't take keys.
    pub(crate) fn modal(&self) -> bool {
        self.menu.is_some() || self.dialog.is_some() || self.command.is_some()
    }

    // ------------------------------------------------------------------ thumbnails

    fn start_thumbs(&self, jobs: Vec<(PathBuf, i64)>) -> Task<Message> {
        Task::batch(jobs.into_iter().map(|(p, m)| {
            let q = p.clone();
            background(move || thumbs::load(&q, m), move |px| Message::Thumb(p.clone(), px))
        }))
    }

    fn want_thumbs(&mut self, pane: u64, first: usize, last: usize) -> Task<Message> {
        let Some(p) = self.pane_by_id(pane) else { return Task::none() };
        let Some(l) = p.loaded.clone() else { return Task::none() };
        if !p.grid || !l.metadata_ready {
            return Task::none();
        }
        let items: Vec<(PathBuf, i64)> = (first..last.min(p.order.len()))
            .map(|pos| p.order[pos] as usize)
            .filter(|&i| !l.listing.is_dir(i))
            .map(|i| (l.listing.path(i), l.listing.mtime[i]))
            .collect();
        let jobs = self.thumbs.want(items);
        self.start_thumbs(jobs)
    }

    // ------------------------------------------------------------------ update

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let t = self.handle(message);
        let p = self.sync_preview();
        self.sync_selection_size();
        for tr in &mut self.transfers {
            if tr.running() {
                tr.sample();
            }
        }
        Task::batch([t, p])
    }

    /// Count the selected folders' sizes in the background for the status bar.
    fn sync_selection_size(&mut self) {
        let pane = self.pane();
        let dirs: Vec<PathBuf> = match pane.loaded.as_ref() {
            Some(l) if self.mode == Mode::Files => pane.selected_entries().into_iter().filter(|&i| l.listing.is_dir(i)).take(10_000).map(|i| l.listing.path(i)).collect(),
            _ => Vec::new(),
        };
        if self.sel_size.as_ref().map(|s| &s.0) == Some(&dirs) || (dirs.is_empty() && self.sel_size.is_none()) {
            return;
        }
        if let Some((_, _, cancel)) = self.sel_size.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        if dirs.is_empty() {
            return;
        }
        let size = Arc::new(ef_core::ops::DirSize::default());
        let cancel = Arc::new(AtomicBool::new(false));
        let (s2, c2, d2) = (size.clone(), cancel.clone(), dirs.clone());
        std::thread::spawn(move || ef_core::ops::measure(&d2, &s2, &c2));
        self.sel_size = Some((dirs, size, cancel));
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded { generation, loaded, order } => {
                let hidden = self.show_hidden;
                let timing = std::env::var_os("ECHOFILES_TIMING").is_some() || self.bench.is_some();
                let Some(pane) = self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()).find(|p| p.generation == generation) else { return Task::none() };
                let ready = loaded.metadata_ready;
                if ready && timing {
                    eprintln!(
                        "listing {}: {} entries · names {:.1} ms · sort {:.1} ms · metadata {:.1} ms · first paint {:.1} ms after navigation",
                        loaded.listing.dir.display(),
                        loaded.listing.len(),
                        loaded.names_ms,
                        loaded.sort_ms,
                        loaded.meta_ms,
                        pane.first_paint_ms
                    );
                }
                pane.apply_loaded(loaded, order);
                let id = pane.id;
                let t = if pane.scope == Scope::Folder && !pane.query.is_empty() { pane.reorder(hidden) } else { Task::none() };
                self.refresh_cut();
                let r = self.resume_rename(id);
                Task::batch([t, r])
            }
            Message::Reordered { generation, order } => {
                if let Some(pane) = self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()).find(|p| p.generation == generation) {
                    pane.order = order;
                    pane.cursor = None;
                }
                Task::none()
            }
            Message::LoadFailed { generation, error } => {
                let hidden = self.show_hidden;
                let mut task = Task::none();
                if let Some(pane) = self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()).find(|p| p.generation == generation) {
                    pane.pending = false;
                    pane.skeleton = false;
                    if let Some(prev) = pane.back.pop() {
                        // What's on screen may be the folder that just failed (a reload, a
                        // server that went away): show the one we're back at for real.
                        let shown_failed = pane.loaded.is_some() && pane.location != prev;
                        pane.location = prev.clone();
                        if shown_failed {
                            task = pane.load(prev, hidden);
                        }
                    }
                    // The facts shown belong to where the pane is back at, not the failed place.
                    pane.fs = volume::fs_info(&pane.location);
                    self.notice = Some(error);
                }
                task
            }
            Message::ShowSkeleton(generation) => {
                if let Some(pane) = self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()).find(|p| p.generation == generation)
                    && pane.pending {
                        pane.skeleton = true;
                    }
                Task::none()
            }
            Message::List(id, action) => self.list_action(id, action),
            Message::Navigate(dir) => self.go(dir, true),
            Message::OpenTab(dir) => {
                let scope = self.settings.search.default_scope;
                self.tabs.insert(self.tab + 1, Tab { panes: vec![Pane::new(dir.clone(), scope)], active: 0 });
                self.tab += 1;
                self.mode = Mode::Files;
                self.load_active(dir)
            }
            Message::Back => {
                let pane = self.pane_mut();
                match pane.back.pop() {
                    Some(prev) => {
                        let cur = pane.location.clone();
                        pane.forward.push(cur);
                        self.go(prev, false)
                    }
                    None => Task::none(),
                }
            }
            Message::Forward => {
                let pane = self.pane_mut();
                match pane.forward.pop() {
                    Some(next) => {
                        let cur = pane.location.clone();
                        pane.back.push(cur);
                        self.go(next, false)
                    }
                    None => Task::none(),
                }
            }
            Message::Up => match self.pane().location.parent() {
                Some(parent) => {
                    // Leaving a folder selects it in its parent.
                    let name = self.pane().location.file_name().map(|n| n.to_os_string());
                    let parent = parent.to_path_buf();
                    let t = self.go(parent, true);
                    if let Some(n) = name {
                        self.pane_mut().reveal = vec![n];
                    }
                    t
                }
                None => Task::none(),
            },
            Message::Reload => {
                let id = self.pane().id;
                self.reload_pane(id)
            }
            Message::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                let hidden = self.show_hidden;
                let mut tasks: Vec<Task<Message>> = self.tabs.iter().flat_map(|t| t.panes.iter()).map(|p| p.reorder(hidden)).collect();
                if self.everywhere() {
                    tasks.push(self.run_search());
                }
                Task::batch(tasks)
            }
            // ---- tabs and layout
            Message::NewTab => {
                let dir = self.pane().location.clone();
                self.handle(Message::OpenTab(dir))
            }
            Message::CloseTab(i) => {
                if self.tabs.len() == 1 {
                    return Task::none();
                }
                self.tabs.remove(i);
                if self.tab >= self.tabs.len() || self.tab > i {
                    self.tab = self.tab.saturating_sub(1).min(self.tabs.len() - 1);
                }
                self.watch_visible();
                Task::none()
            }
            Message::SelectTab(i) => {
                if i < self.tabs.len() && i != self.tab {
                    self.tab = i;
                    self.rename = None;
                    self.mode = Mode::Files;
                    self.watch_visible();
                    // Background tabs aren't watched: catch up on what changed meanwhile.
                    let ids: Vec<u64> = self.tabs[i].panes.iter().filter(|p| !p.special()).map(|p| p.id).collect();
                    return Task::batch(ids.into_iter().map(|id| self.reload_pane(id)));
                }
                Task::none()
            }
            Message::CycleTab(back) => {
                let n = self.tabs.len();
                let i = if back { (self.tab + n - 1) % n } else { (self.tab + 1) % n };
                self.handle(Message::SelectTab(i))
            }
            Message::ToggleDual => {
                self.rename = None;
                let scope = self.settings.search.default_scope;
                let tab = &mut self.tabs[self.tab];
                if tab.panes.len() == 2 {
                    tab.panes.remove(1 - tab.active);
                    tab.active = 0;
                    self.watch_visible();
                    Task::none()
                } else {
                    let dir = tab.panes[0].location.clone();
                    tab.panes.push(Pane::new(dir.clone(), scope));
                    tab.active = 1;
                    self.load_active(dir)
                }
            }
            Message::SwitchPane => {
                let tab = &mut self.tabs[self.tab];
                if tab.panes.len() == 2 {
                    tab.active = 1 - tab.active;
                    self.rename = None;
                }
                Task::none()
            }
            Message::SetGrid(on) => {
                let pane = self.pane_mut();
                pane.grid = on;
                let dir = pane.location.clone();
                self.view_modes.insert(dir, on);
                let text: String = self.view_modes.iter().map(|(p, g)| format!("{}\t{}\n", if *g { "grid" } else { "list" }, p.display())).collect();
                std::thread::spawn(move || {
                    let f = view_modes_file();
                    if let Some(d) = f.parent() {
                        let _ = std::fs::create_dir_all(d);
                    }
                    let _ = std::fs::write(f, text);
                });
                Task::none()
            }
            Message::TogglePreview => {
                self.settings.appearance.preview = !self.settings.appearance.preview;
                self.persist_settings()
            }
            Message::ToggleSidebar => {
                self.settings.sidebar.hidden = !self.settings.sidebar.hidden;
                self.persist_settings()
            }
            Message::SidebarResize(Some(x)) => {
                // NaN starts a drag at the edge; later values are the pointer's x (the
                // sidebar starts at the window's left edge, so x is the new width).
                if x.is_nan() {
                    let w = self.settings.sidebar.width as f32;
                    self.sidebar_drag = Some((w, w));
                } else if self.sidebar_drag.is_some() {
                    self.settings.sidebar.width = x.clamp(180.0, 360.0) as u16;
                }
                Task::none()
            }
            Message::SidebarResize(None) => {
                if self.sidebar_drag.take().is_some() {
                    return self.persist_settings();
                }
                Task::none()
            }
            Message::ShowDrives => {
                self.mode = Mode::Files;
                let pane = self.pane_mut();
                pane.shares = None;
                if !pane.drives {
                    pane.drives = true;
                    pane.clear_search();
                }
                
                background(|| ef_disks::windows_volumes().map_err(|e| e.to_string()), |r| Message::Drive(DriveMsg::Volumes(r)))
            }
            // ---- search
            Message::Search(q) => {
                let hidden = self.show_hidden;
                let pane = self.pane_mut();
                pane.query = q;
                match pane.scope {
                    Scope::Folder => pane.reorder(hidden),
                    Scope::Everywhere => self.run_search(),
                }
            }
            Message::SearchSubmit => match (self.everywhere(), self.pane().result_cursor) {
                (true, Some(i)) => self.handle(Message::ResultOpen(i)),
                _ => Task::none(),
            },
            Message::SetScope(s) => {
                let hidden = self.show_hidden;
                let pane = self.pane_mut();
                pane.scope = s;
                pane.results = None;
                pane.result_cursor = None;
                let t = pane.reorder(hidden);
                if s == Scope::Everywhere { Task::batch([t, self.run_search()]) } else { t }
            }
            Message::SearchDone { generation, results } => {
                if let Some(pane) = self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()).find(|p| p.search_generation == generation) {
                    pane.searching = false;
                    if let Some(r) = results {
                        pane.result_cursor = (!r.hits.is_empty()).then_some(0);
                        pane.results = Some(r);
                    }
                }
                Task::none()
            }
            Message::ResultClick(i) => {
                self.pane_mut().result_cursor = Some(i);
                Task::none()
            }
            Message::ResultOpen(i) => match self.pane().results.as_ref().and_then(|r| r.hits.get(i)).map(|h| h.path.clone()) {
                Some(p) => self.open_path(p),
                None => Task::none(),
            },
            Message::ResultReveal(i) => {
                let Some(h) = self.pane().results.as_ref().and_then(|r| r.hits.get(i)) else { return Task::none() };
                let (Some(parent), Some(name)) = (h.path.parent().map(Path::to_path_buf), h.path.file_name().map(|n| n.to_os_string())) else { return Task::none() };
                self.pane_mut().scope = Scope::Folder;
                let t = self.go(parent, true);
                self.pane_mut().reveal = vec![name];
                t
            }
            Message::FocusSearch => iced::widget::operation::focus(SEARCH_ID),
            // ---- path bar
            Message::EditPath(on) => {
                if on {
                    let loc = self.pane().location.clone();
                    let s = self.net_uri_for(&loc).unwrap_or_else(|| loc.to_string_lossy().into_owned());
                    self.path_edit = Some(s);
                    Task::batch([iced::widget::operation::focus(PATH_ID), iced::widget::operation::select_all(PATH_ID)])
                } else {
                    self.path_edit = None;
                    Task::none()
                }
            }
            Message::PathDraft(s) => {
                self.path_edit = Some(s);
                Task::none()
            }
            Message::PathSubmit => {
                let Some(raw) = self.path_edit.clone() else { return Task::none() };
                if let Request::Connect(uri) = system::request_for(raw.trim()) {
                    self.path_edit = None;
                    return match ef_net::Address::parse(&uri, ef_net::Protocol::Smb) {
                        Ok(a) => self.connect(a, false),
                        Err(e) => {
                            self.toast_error("Can't go there".into(), e);
                            Task::none()
                        }
                    };
                }
                match self.resolve_typed_path(raw.trim()) {
                    Ok(p) if p.is_dir() => {
                        self.path_edit = None;
                        self.go(p, true)
                    }
                    Ok(p) if p.exists() => {
                        self.path_edit = None;
                        let (parent, name) = (p.parent().map(Path::to_path_buf), p.file_name().map(|n| n.to_os_string()));
                        match (parent, name) {
                            (Some(dir), Some(n)) => {
                                let t = self.go(dir, true);
                                self.pane_mut().reveal = vec![n];
                                t
                            }
                            _ => Task::none(),
                        }
                    }
                    Ok(p) if ef_net::gvfs::is_network_path(&p) => {
                        self.toast_error("That network place isn't connected".into(), "Connect it from the Network section, or type its address (smb://, sftp://, ftp://).".into());
                        Task::none()
                    }
                    Ok(p) => {
                        self.toast_error("No such folder".into(), format!("{} doesn't exist. Check the spelling.", p.display()));
                        Task::none()
                    }
                    Err(e) => {
                        self.toast_error("Can't go there".into(), e);
                        Task::none()
                    }
                }
            }
            // ---- subsystems
            Message::File(m) => self.file_update(m),
            Message::Drive(m) => self.drive_update(m),
            Message::Net(m) => self.net_update(m),
            Message::Fold(section, open) => {
                match section {
                    crate::view::Fold::Windows => self.settings.sidebar.windows_open = open,
                    crate::view::Fold::Network => self.settings.sidebar.network_open = open,
                }
                self.persist_settings()
            }
            Message::Ui(m) => self.ui_update(m),
            Message::Thumb(path, px) => {
                let jobs = self.thumbs.done(path, px);
                self.start_thumbs(jobs)
            }
            Message::Preview(path, data) => {
                if self.preview_want.as_ref() == Some(&path) {
                    self.preview = Some((path, data));
                }
                Task::none()
            }
            Message::DismissNotice => {
                self.notice = None;
                Task::none()
            }
            Message::Escape => self.escape(),
            Message::Settings(m) => self.settings_update(m),
            Message::Key(keyboard::Event::KeyPressed { key, modifiers, text, .. }) => self.key(key, modifiers, text),
            Message::Key(keyboard::Event::ModifiersChanged(m)) => {
                self.modifiers = m;
                Task::none()
            }
            Message::Key(_) => Task::none(),
            Message::Resized(size) => {
                self.window_size = size;
                Task::none()
            }
            Message::Tick => {
                let stamp = theme_stamp();
                if stamp != self.theme_stamp {
                    self.theme_stamp = stamp;
                    self.palette = ef_theme::load_active();
                    self.icons = Icons::new(&self.palette);
                }
                self.expire_toasts();
                if indexer::settled(self.index_dirty, 1500) && self.index_state != IndexState::Building {
                    return self.build_index();
                }
                Task::none()
            }
            Message::MouseMove(p) => {
                self.mouse = p;
                if let Some(d) = &mut self.drag {
                    d.at = p;
                }
                if self.sidebar_drag.is_some() {
                    return self.handle(Message::SidebarResize(Some(p.x)));
                }
                Task::none()
            }
            Message::MouseUp => {
                let t = if self.sidebar_drag.is_some() { self.handle(Message::SidebarResize(None)) } else { Task::none() };
                // A drop on a list was handled already (widget messages come first); a drop on
                // a sidebar place lands here.
                let place = self.drop_place.take();
                match (self.drag.take(), place) {
                    (Some(d), Some(dest)) => Task::batch([t, self.drop_paths(d.paths, dest)]),
                    _ => t,
                }
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
                if !(change.visible || self.show_hidden) || self.mode != Mode::Files {
                    return Task::none();
                }
                let ids: Vec<u64> = self.tabs[self.tab].panes.iter().filter(|p| p.location == change.folder && !p.special()).map(|p| p.id).collect();
                Task::batch(ids.into_iter().map(|id| self.reload_pane(id)))
            }
            Message::Request(req) => {
                if let Request::Open(Some(p)) = &req {
                    let t = self.go(p.clone(), true);
                    return Task::batch([t, self.show_window()]);
                }
                if let Request::Connect(uri) = &req {
                    let t = match ef_net::Address::parse(uri, ef_net::Protocol::Smb) {
                        Ok(a) => self.connect(a, false),
                        Err(e) => {
                            self.toast_error(format!("Can't open {uri}"), e);
                            Task::none()
                        }
                    };
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

    /// Save settings changed outside the Settings screen (sidebar, preview).
    pub(crate) fn persist_settings(&self) -> Task<Message> {
        let s = self.settings.clone();
        std::thread::spawn(move || {
            let _ = s.save();
        });
        Task::none()
    }

    /// Typed paths: `~/x`, absolute paths, and Windows paths (`D:\Work`) when the drive's
    /// letter is known and it's mounted.
    fn resolve_typed_path(&self, raw: &str) -> Result<PathBuf, String> {
        let b = raw.as_bytes();
        if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
            let letter = (b[0] as char).to_ascii_uppercase();
            let v = self.volumes.iter().find(|v| v.letter == Some(letter)).ok_or(format!("No drive {letter}: is known yet. Mount it once so EchoFiles can learn the letters."))?;
            let mp = v.mount_points.first().ok_or(format!("{letter}: isn't mounted. Click it in the sidebar to mount it."))?;
            let rest = raw[2..].trim_start_matches(['\\', '/']).replace('\\', "/");
            return Ok(Path::new(mp).join(rest));
        }
        let p = config::expand(raw);
        if p.is_absolute() { Ok(p) } else { Ok(self.pane().location.join(p)) }
    }

    fn list_action(&mut self, id: u64, action: Action) -> Task<Message> {
        // Anything done in a pane makes it the active one.
        if let Some(ti) = self.tabs.iter().position(|t| t.panes.iter().any(|p| p.id == id)) {
            let t = &mut self.tabs[ti];
            if let Some(pi) = t.panes.iter().position(|p| p.id == id)
                && t.active != pi && self.tab == ti {
                    t.active = pi;
                    if self.rename.as_ref().is_some_and(|r| r.pane != id) {
                        self.rename = None;
                    }
                }
        }
        match action {
            Action::Focus => {
                self.menu = None;
                Task::none()
            }
            Action::Cursor { pos, extend, toggle } => {
                let Some(pane) = self.pane_by_id_mut(id) else { return Task::none() };
                if toggle {
                    if let Some(i) = pane.entry_at(pos) {
                        let on = !file_list::is_selected(&pane.selected, i);
                        pane.set_bit(i, on);
                    }
                    pane.cursor = Some(pos);
                    pane.anchor = Some(pos);
                } else if extend {
                    let anchor = pane.anchor.unwrap_or(pos);
                    pane.clear_selection();
                    for p in anchor.min(pos)..=anchor.max(pos) {
                        if let Some(i) = pane.entry_at(p) {
                            pane.set_bit(i, true);
                        }
                    }
                    pane.cursor = Some(pos);
                } else {
                    pane.select_only(pos);
                }
                Task::none()
            }
            Action::Marquee(selected) => {
                if let Some(pane) = self.pane_by_id_mut(id)
                    && selected.len() == pane.selected.len() {
                        pane.selected = selected;
                        pane.cursor = None;
                        pane.anchor = None;
                    }
                Task::none()
            }
            Action::ClearSelection => {
                if let Some(pane) = self.pane_by_id_mut(id) {
                    pane.clear_selection();
                    pane.cursor = None;
                }
                Task::none()
            }
            Action::SelectAll => {
                if let Some(pane) = self.pane_by_id_mut(id) {
                    pane.select_all();
                }
                Task::none()
            }
            Action::Open(pos) => self.open(pos),
            Action::Sort(by) => {
                let hidden = self.show_hidden;
                let Some(pane) = self.pane_by_id_mut(id) else { return Task::none() };
                if pane.sort.by == by {
                    pane.sort.descending = !pane.sort.descending;
                } else {
                    pane.sort = SortSpec { by, descending: matches!(by, SortBy::Size | SortBy::Modified), ..pane.sort };
                }
                pane.reorder(hidden)
            }
            Action::Context { pos, at } => {
                self.rename = None;
                self.open_menu(MenuFor::Files { pane: id, on_item: pos.is_some() }, at);
                Task::none()
            }
            Action::Middle(pos) => {
                let Some(pane) = self.pane_by_id(id) else { return Task::none() };
                let Some(l) = pane.loaded.clone() else { return Task::none() };
                let Some(i) = pane.entry_at(pos) else { return Task::none() };
                let path = l.listing.path(i);
                if path.is_dir() { self.handle(Message::OpenTab(path)) } else { Task::none() }
            }
            Action::DragStart => {
                let Some(pane) = self.pane_by_id(id) else { return Task::none() };
                let paths = pane.targets();
                if !paths.is_empty() {
                    self.drag = Some(Drag { paths, from: id, at: self.mouse });
                }
                Task::none()
            }
            Action::Drop { pos } => {
                let Some(d) = self.drag.take() else { return Task::none() };
                let Some(pane) = self.pane_by_id(id) else { return Task::none() };
                let dest = match pos.and_then(|p| pane.entry_at(p)) {
                    Some(i) => pane.loaded.as_ref().map(|l| l.listing.path(i)).unwrap_or_else(|| pane.location.clone()),
                    None => pane.location.clone(),
                };
                if pos.is_none() && d.from == id {
                    return Task::none(); // dropped back where it came from
                }
                self.drop_paths(d.paths, dest)
            }
            Action::Spring(pos) => {
                let Some(pane) = self.pane_by_id(id) else { return Task::none() };
                let Some(i) = pane.entry_at(pos) else { return Task::none() };
                let Some(path) = pane.loaded.as_ref().map(|l| l.listing.path(i)) else { return Task::none() };
                self.go(path, true)
            }
            Action::EditorBlur => self.file_update(FileMsg::RenameCommit),
            Action::Visible { first, last } => self.want_thumbs(id, first, last),
        }
    }

    fn escape(&mut self) -> Task<Message> {
        if self.menu.take().is_some() {
            return Task::none();
        }
        if self.command.take().is_some() {
            return Task::none();
        }
        if let Some(d) = self.dialog.take() {
            return self.dialog_cancelled(d);
        }
        if self.net.form.is_some() {
            return self.net_update(NetMsg::Close);
        }
        if self.rename.take().is_some() {
            return Task::none();
        }
        if self.path_edit.take().is_some() {
            return Task::none();
        }
        if self.drag.take().is_some() {
            return Task::none();
        }
        if self.mode == Mode::Settings {
            self.mode = Mode::Files;
            return Task::none();
        }
        let hidden = self.show_hidden;
        let pane = self.pane_mut();
        if !pane.query.is_empty() {
            pane.clear_search();
            return pane.reorder(hidden);
        }
        if pane.special() {
            pane.drives = false;
            pane.shares = None;
            return Task::none();
        }
        pane.clear_selection();
        Task::none()
    }

    fn key(&mut self, key: Key, modifiers: keyboard::Modifiers, text: Option<iced::advanced::graphics::core::SmolStr>) -> Task<Message> {
        let ctrl = modifiers.control();
        let shift = modifiers.shift();
        let alt = modifiers.alt();
        if matches!(key.as_ref(), Key::Character("q")) && ctrl {
            return iced::exit();
        }
        if self.mode == Mode::Settings {
            return Task::none();
        }
        let tab = |shift: bool| if shift { iced::widget::operation::focus_previous() } else { iced::widget::operation::focus_next() };
        if self.dialog.is_some() {
            return match key.as_ref() {
                Key::Named(Named::Enter) => self.ui_update(UiMsg::DialogDefault),
                Key::Named(Named::Tab) => tab(shift),
                _ => Task::none(),
            };
        }
        if self.net.form.is_some() {
            return match key.as_ref() {
                Key::Named(Named::Enter) => self.net_update(NetMsg::Submit),
                Key::Named(Named::Tab) => tab(shift),
                _ => Task::none(),
            };
        }
        if self.command.is_some() {
            return match key.as_ref() {
                Key::Named(Named::ArrowDown) => self.ui_update(UiMsg::CommandMove(1)),
                Key::Named(Named::ArrowUp) => self.ui_update(UiMsg::CommandMove(-1)),
                _ => Task::none(),
            };
        }
        if self.menu.is_some() {
            return match key.as_ref() {
                Key::Named(Named::ArrowDown) => self.ui_update(UiMsg::MenuMove(1)),
                Key::Named(Named::ArrowUp) => self.ui_update(UiMsg::MenuMove(-1)),
                Key::Named(Named::Enter) => self.ui_update(UiMsg::MenuActivate),
                _ => Task::none(),
            };
        }
        if self.everywhere() {
            let n = self.pane().results.as_ref().map_or(0, |r| r.hits.len());
            match key.as_ref() {
                Key::Named(Named::ArrowDown) if n > 0 => {
                    let pane = self.pane_mut();
                    pane.result_cursor = Some(pane.result_cursor.map_or(0, |c| (c + 1).min(n - 1)));
                    return Task::none();
                }
                Key::Named(Named::ArrowUp) if n > 0 => {
                    let pane = self.pane_mut();
                    pane.result_cursor = Some(pane.result_cursor.map_or(0, |c| c.saturating_sub(1)));
                    return Task::none();
                }
                Key::Named(Named::Enter) => {
                    return match self.pane().result_cursor {
                        Some(i) if alt => self.handle(Message::ResultReveal(i)),
                        Some(i) => self.handle(Message::ResultOpen(i)),
                        None => Task::none(),
                    };
                }
                _ => {}
            }
        }
        let dual = self.dual();
        let f = |m: FileMsg| Message::File(m);
        let msg = match key.as_ref() {
            Key::Named(Named::Backspace) => Some(Message::Up),
            Key::Named(Named::ArrowLeft) if alt => Some(Message::Back),
            Key::Named(Named::ArrowRight) if alt => Some(Message::Forward),
            Key::Named(Named::ArrowUp) if alt => Some(Message::Up),
            Key::Named(Named::Enter) if alt => Some(f(FileMsg::Properties)),
            Key::Named(Named::Enter) if ctrl => Some(f(FileMsg::OpenInTab)),
            Key::Named(Named::F2) => Some(f(FileMsg::StartRename)),
            Key::Named(Named::F3) => Some(Message::ToggleDual),
            Key::Named(Named::F5) if dual => Some(f(FileMsg::ToOther(ef_core::ops::Kind::Copy))),
            Key::Named(Named::F6) if dual => Some(f(FileMsg::ToOther(ef_core::ops::Kind::Move))),
            Key::Named(Named::F5) => Some(Message::Reload),
            Key::Named(Named::Tab) if ctrl => Some(Message::CycleTab(shift)),
            Key::Named(Named::Tab) if dual => Some(Message::SwitchPane),
            Key::Named(Named::Delete) if shift => Some(f(FileMsg::AskDelete)),
            Key::Named(Named::Delete) => Some(f(FileMsg::Trash)),
            Key::Named(Named::Space) => Some(Message::TogglePreview),
            Key::Named(Named::F10) if shift => Some(f(FileMsg::MenuAtCursor)),
            Key::Named(Named::ContextMenu) => Some(f(FileMsg::MenuAtCursor)),
            Key::Character("r") if ctrl => Some(Message::Reload),
            Key::Character("c") if ctrl && shift => Some(f(FileMsg::CopyPath)),
            Key::Character("c") if ctrl => Some(f(FileMsg::Copy)),
            Key::Character("x") if ctrl => Some(f(FileMsg::Cut)),
            Key::Character("v") if ctrl => Some(f(FileMsg::Paste)),
            Key::Character("z") if ctrl => Some(f(FileMsg::Undo)),
            Key::Character("n") | Key::Character("N") if ctrl && shift => Some(f(FileMsg::NewFolder)),
            Key::Character("t") if ctrl => Some(Message::NewTab),
            Key::Character("w") if ctrl => Some(Message::CloseTab(self.tab)),
            Key::Character("k") if ctrl => Some(Message::Ui(UiMsg::OpenCommand)),
            Key::Character("l") if ctrl => Some(Message::EditPath(true)),
            Key::Character("b") if ctrl => Some(Message::ToggleSidebar),
            Key::Character("d") | Key::Character("D") if ctrl && shift => Some(Message::ShowDrives),
            Key::Character("s") | Key::Character("S") if ctrl && shift => Some(Message::Net(NetMsg::Open(None))),
            Key::Character("1") if ctrl => Some(Message::SetGrid(false)),
            Key::Character("2") if ctrl => Some(Message::SetGrid(true)),
            Key::Character(",") if ctrl => Some(Message::Settings(SettingsMsg::Open)),
            Key::Character("f") if ctrl => Some(Message::FocusSearch),
            Key::Character("/") => Some(Message::FocusSearch),
            Key::Character("h") if ctrl => Some(Message::ToggleHidden),
            Key::Character("e") if ctrl => {
                let s = if self.pane().scope == Scope::Folder { Scope::Everywhere } else { Scope::Folder };
                Some(Message::SetScope(s))
            }
            _ => None,
        };
        if let Some(m) = msg {
            return self.handle(m);
        }
        if !ctrl && !alt && let Some(t) = text.as_ref().filter(|t| t.chars().all(|c| !c.is_control())) {
            self.pane_mut().jump_to_prefix(t);
        }
        Task::none()
    }

    fn frame(&mut self, now: Instant) -> Task<Message> {
        if !self.first_frame_logged {
            self.first_frame_logged = true;
            if std::env::var_os("ECHOFILES_TIMING").is_some() || self.bench.is_some() {
                eprintln!("startup: first frame {:.1} ms after process start", crate::since_start_ms());
            }
        }
        let ready = self.pane().loaded.as_ref().is_some_and(|l| l.metadata_ready);
        let n = self.pane().order.len();
        let Some(b) = self.bench.as_mut() else { return Task::none() };
        if !ready || n == 0 {
            return Task::none();
        }
        if !b.started {
            b.started = true;
            b.last = Some(now);
            self.pane_mut().cursor = Some(0);
            return Task::none();
        }
        if let Some(last) = b.last {
            b.frames.push(now.duration_since(last).as_secs_f64() * 1000.0);
        }
        b.last = Some(now);
        let done = b.frames.len() >= 600;
        let frames = done.then(|| b.frames.clone());
        let pane = self.pane_mut();
        pane.cursor = Some((pane.cursor.unwrap_or(0) + 3) % n);
        if let Some(mut f) = frames {
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

    /// Anything moving on screen: menus, dialogs, toasts entering, transfers running.
    fn animating(&self) -> bool {
        let fresh = |t: Instant| t.elapsed() < Duration::from_millis(400);
        self.transfers.iter().any(|t| t.running())
            || self.preview_counting()
            || self.props_counting()
            || self.sel_size.as_ref().is_some_and(|s| !s.1.done.load(Ordering::Relaxed))
            || self.drag.is_some()
            || (self.animations && (self.menu.as_ref().is_some_and(|m| fresh(m.opened)) || self.dialog.as_ref().is_some_and(|d| fresh(d.opened())) || self.toasts.iter().any(|t| fresh(t.at)) || self.command.as_ref().is_some_and(|c| fresh(c.opened)) || self.net.form.as_ref().is_some_and(|f| fresh(f.opened))))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            keyboard::listen().map(Message::Key),
            // Esc works even while a text field has focus (it captures other keys).
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key: Key::Named(Named::Escape), .. }) => Some(Message::Escape),
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => Some(Message::MouseUp),
                _ => None,
            }),
            iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            window::close_requests().map(Message::CloseRequested),
            window::close_events().map(Message::WindowClosed),
            window::resize_events().map(|(_, s)| Message::Resized(s)),
            Subscription::run(system::requests).map(Message::Request),
            Subscription::run(changes).map(Message::FsChanged),
            Subscription::run(crate::drives::prompts).map(|p| Message::Drive(DriveMsg::Prompt(p))),
            Subscription::run(crate::network::events).map(Message::Net),
        ];
        if self.drag.is_some() || self.sidebar_drag.is_some() {
            subs.push(iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => Some(Message::MouseMove(position)),
                _ => None,
            }));
        }
        if self.settings.search.index {
            subs.push(iced::time::every(Duration::from_secs(60)).map(|_| Message::IndexTick));
        }
        if self.bench.is_some() || !self.first_frame_logged || self.animating() {
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

    // ------------------------------------------------------------------ preview data

    /// What the preview pane shows: the cursor item, else the single selected item.
    pub(crate) fn preview_target(&self) -> Option<PathBuf> {
        let pane = self.pane();
        let l = pane.loaded.as_ref()?;
        let i = pane.cursor.and_then(|c| pane.entry_at(c)).or_else(|| pane.selected_entries().first().copied())?;
        Some(l.listing.path(i))
    }

    fn sync_preview(&mut self) -> Task<Message> {
        if !self.settings.appearance.preview || self.mode != Mode::Files {
            return Task::none();
        }
        let want = self.preview_target();
        if want == self.preview_want {
            return Task::none();
        }
        self.preview_want = want.clone();
        if let Some((_, d)) = &self.preview {
            d.cancel.store(true, Ordering::Relaxed);
        }
        let Some(path) = want else {
            self.preview = None;
            return Task::none();
        };
        let mut tasks = Vec::new();
        if thumbs::thumbnailable(&path) && self.thumbs.get(&path).is_none() {
            let mtime = std::fs::metadata(&path).map(|m| std::os::unix::fs::MetadataExt::mtime(&m)).unwrap_or(0);
            let jobs = self.thumbs.want(vec![(path.clone(), mtime)]);
            tasks.push(self.start_thumbs(jobs));
        }
        let p = path.clone();
        tasks.push(background(move || Arc::new(crate::preview::gather(&p)), move |d| Message::Preview(path.clone(), d)));
        Task::batch(tasks)
    }
}

/// `volume::fs_info` is re-exported for views that only have a path.
pub fn fs_of(p: &Path) -> Option<FsInfo> {
    volume::fs_info(p)
}

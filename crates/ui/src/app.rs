//! Application state, update and view.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use ef_core::fmt::{self, DateFormatter};
use ef_core::listing;
use ef_core::sort::{self, NameKeys, SortBy, SortSpec};
use ef_core::volume::{self, FsInfo};
use ef_core::Listing;
use ef_disks::Volume;
use ef_theme::Palette;
use iced::futures::channel::mpsc;
use iced::keyboard::{self, key::Named, Key};
use iced::widget::{button, column, container, row, svg, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Subscription, Task};

use crate::file_list::{self, Action, FileList};
use crate::style::{self, color, Icons};

/// A directory as far as it has been loaded. Shared with the loader thread through `Arc`.
pub struct Loaded {
    pub listing: Listing,
    pub keys: Arc<NameKeys>,
    pub metadata_ready: bool,
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
    ToggleHidden,
    Volumes(Result<Vec<Volume>, String>),
    DriveClicked(usize),
    Key(keyboard::Event),
    ThemeTick,
    Frame(Instant),
    Search(String),
    FocusSearch,
    ShowSkeleton(u64),
}

const SEARCH_ID: &str = "search";

struct Bench {
    frames: Vec<f64>,
    last: Option<Instant>,
    started: bool,
}

pub struct App {
    palette: Palette,
    icons: Icons,
    dates: DateFormatter,
    theme_stamp: Option<SystemTime>,
    location: PathBuf,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    generation: u64,
    nav_started: Instant,
    first_paint_ms: f64,
    loaded: Option<Arc<Loaded>>,
    order: Arc<Vec<u32>>,
    show_hidden: bool,
    selected: Vec<u64>,
    cursor: Option<usize>,
    anchor: Option<usize>,
    volumes: Vec<Volume>,
    notice: Option<String>,
    typeahead: (String, Instant),
    bench: Option<Bench>,
    first_frame_logged: bool,
    sort: SortSpec,
    query: String,
    /// A navigation is in flight; the old listing stays up for 150 ms, then a skeleton.
    pending: bool,
    skeleton: bool,
    fs: Option<FsInfo>,
    volume_fs: Vec<Option<FsInfo>>,
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| "/".into())
}

fn theme_stamp() -> Option<SystemTime> {
    let dir = ef_theme::omarchy_theme_dir()?;
    std::fs::metadata(dir.join("colors.toml")).and_then(|m| m.modified()).ok()
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let palette = ef_theme::load_active();
        let icons = Icons::new(&palette);
        let bench_dir = std::env::var_os("ECHOFILES_BENCH").map(PathBuf::from);
        let start = bench_dir.clone().or_else(|| std::env::args_os().nth(1).map(PathBuf::from)).unwrap_or_else(home);
        let mut app = Self {
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
            show_hidden: false,
            selected: Vec::new(),
            cursor: None,
            anchor: None,
            volumes: Vec::new(),
            notice: None,
            typeahead: (String::new(), Instant::now()),
            bench: bench_dir.map(|_| Bench { frames: Vec::with_capacity(700), last: None, started: false }),
            first_frame_logged: false,
            sort: SortSpec::default(),
            query: String::new(),
            pending: true,
            skeleton: false,
            fs: volume::fs_info(&start),
            volume_fs: Vec::new(),
        };
        let load = app.load(start);
        let probe = Task::perform(
            async {
                let (tx, rx) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(ef_disks::windows_volumes().map_err(|e| e.to_string()));
                });
                rx.await.unwrap_or_else(|_| Err("drive probe stopped".into()))
            },
            Message::Volumes,
        );
        (app, Task::batch([load, probe]))
    }

    pub fn title(&self) -> String {
        let name = self.location.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "/".into());
        format!("{name} — EchoFiles")
    }

    /// Start listing `dir` on the rayon pool: names first, then metadata (build plan §2.2).
    fn load(&mut self, dir: PathBuf) -> Task<Message> {
        self.generation += 1;
        self.nav_started = Instant::now();
        let generation = self.generation;
        let show_hidden = self.show_hidden;
        let spec = self.sort;
        self.pending = true;
        self.fs = volume::fs_info(&dir);
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
            let t1 = Instant::now();
            let keys = Arc::new(NameKeys::build(&names));
            let order = Arc::new(sort::order(&names, &keys, spec, show_hidden));
            let sort_ms = ms(t1);
            let mut full = names.clone();
            send(Message::Loaded {
                generation,
                loaded: Arc::new(Loaded { listing: names, keys: keys.clone(), metadata_ready: false, names_ms, meta_ms: 0.0, sort_ms }),
                order: order.clone(),
            });
            let t2 = Instant::now();
            listing::fill_metadata(&mut full, &fd);
            let meta_ms = ms(t2);
            let order = if matches!(spec.by, SortBy::Size | SortBy::Modified) {
                Arc::new(sort::order(&full, &keys, spec, show_hidden))
            } else {
                order
            };
            send(Message::Loaded {
                generation,
                loaded: Arc::new(Loaded { listing: full, keys, metadata_ready: true, names_ms, meta_ms, sort_ms }),
                order,
            });
        });
        Task::run(rx, |m| m)
    }

    fn navigate(&mut self, dir: PathBuf, record: bool) -> Task<Message> {
        if dir == self.location {
            return Task::none();
        }
        if record {
            self.back.push(std::mem::replace(&mut self.location, dir.clone()));
            self.forward.clear();
        } else {
            self.location = dir.clone();
        }
        self.notice = None;
        self.query.clear();
        self.load(dir)
    }

    fn reorder(&self) -> Task<Message> {
        let Some(loaded) = self.loaded.clone() else { return Task::none() };
        let generation = self.generation;
        let show_hidden = self.show_hidden;
        let spec = self.sort;
        let query = self.query.clone();
        Task::perform(
            async move {
                let (tx, rx) = iced::futures::channel::oneshot::channel();
                rayon::spawn(move || {
                    let order = sort::order(&loaded.listing, &loaded.keys, spec, show_hidden);
                    let _ = tx.send(filter(&loaded.listing, order, &query));
                });
                rx.await.unwrap_or_default()
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

    fn selection_stats(&self) -> (usize, u64) {
        let Some(l) = self.loaded.as_ref().map(|l| &l.listing) else { return (0, 0) };
        let mut count = 0;
        let mut bytes = 0;
        for (wi, &w) in self.selected.iter().enumerate() {
            let mut bits = w;
            while bits != 0 {
                let i = wi * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                count += 1;
                if !l.is_dir(i) {
                    bytes += l.size[i];
                }
            }
        }
        (count, bytes)
    }

    fn open(&mut self, pos: usize) -> Task<Message> {
        let (Some(i), Some(l)) = (self.entry_at(pos), self.loaded.clone()) else { return Task::none() };
        let path = l.listing.path(i);
        if path.is_dir() {
            return self.navigate(path, true);
        }
        match std::process::Command::new("xdg-open").arg(&path).spawn() {
            Ok(_) => {}
            Err(e) => self.notice = Some(format!("Couldn't open “{}”: {e}", l.listing.name(i).to_string_lossy())),
        }
        Task::none()
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

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded { generation, loaded, order } if generation == self.generation => {
                let first = !loaded.metadata_ready;
                if first {
                    self.pending = false;
                    self.skeleton = false;
                    self.first_paint_ms = ms(self.nav_started);
                    self.selected = vec![0; loaded.listing.len().div_ceil(64)];
                    self.cursor = None;
                    self.anchor = None;
                } else if std::env::var_os("ECHOFILES_TIMING").is_some() || self.bench.is_some() {
                    eprintln!(
                        "listing {}: {} entries · names {:.1} ms · sort {:.1} ms · metadata {:.1} ms · first paint {:.1} ms after navigation",
                        loaded.listing.dir.display(), loaded.listing.len(), loaded.names_ms, loaded.sort_ms, loaded.meta_ms, self.first_paint_ms
                    );
                }
                self.loaded = Some(loaded);
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
                self.reorder()
            }
            Message::FocusSearch => iced::widget::operation::focus(SEARCH_ID),
            Message::ShowSkeleton(generation) => {
                if generation == self.generation && self.pending {
                    self.skeleton = true;
                }
                Task::none()
            }
            Message::Navigate(dir) => self.navigate(dir, true),
            Message::Back => match self.back.pop() {
                Some(prev) => {
                    self.forward.push(std::mem::replace(&mut self.location, prev.clone()));
                    self.load(prev)
                }
                None => Task::none(),
            },
            Message::Forward => match self.forward.pop() {
                Some(next) => {
                    self.back.push(std::mem::replace(&mut self.location, next.clone()));
                    self.load(next)
                }
                None => Task::none(),
            },
            Message::Up => match self.location.parent() {
                Some(parent) => self.navigate(parent.to_path_buf(), true),
                None => Task::none(),
            },
            Message::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.reorder()
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
                match v.mount_points.first() {
                    Some(mp) => {
                        let mp = PathBuf::from(mp);
                        self.navigate(mp, true)
                    }
                    None => {
                        self.notice = Some(format!(
                            "{} isn't mounted yet. Mounting from EchoFiles arrives in M2 — for now run: udisksctl mount -b {}",
                            drive_name(v, &self.volumes),
                            v.device
                        ));
                        Task::none()
                    }
                }
            }
            Message::Key(keyboard::Event::KeyPressed { key, modifiers, text, .. }) => match key.as_ref() {
                Key::Named(Named::Backspace) => self.update(Message::Up),
                Key::Named(Named::ArrowLeft) if modifiers.alt() => self.update(Message::Back),
                Key::Named(Named::ArrowRight) if modifiers.alt() => self.update(Message::Forward),
                Key::Named(Named::ArrowUp) if modifiers.alt() => self.update(Message::Up),
                Key::Named(Named::F5) => {
                    let dir = self.location.clone();
                    self.load(dir)
                }
                Key::Named(Named::Escape) => {
                    if !self.query.is_empty() {
                        return self.update(Message::Search(String::new()));
                    }
                    self.clear_selection();
                    Task::none()
                }
                Key::Character("f") if modifiers.control() => self.update(Message::FocusSearch),
                Key::Character("l") if modifiers.control() => self.update(Message::FocusSearch),
                Key::Character("/") => self.update(Message::FocusSearch),
                Key::Character("h") if modifiers.control() => self.update(Message::ToggleHidden),
                Key::Character("q") if modifiers.control() => iced::exit(),
                _ => {
                    if !modifiers.control() && !modifiers.alt() {
                        if let Some(t) = text.as_ref().filter(|t| t.chars().all(|c| !c.is_control())) {
                            self.jump_to_prefix(t);
                        }
                    }
                    Task::none()
                }
            },
            Message::Key(_) => Task::none(),
            Message::ThemeTick => {
                let stamp = theme_stamp();
                if stamp != self.theme_stamp {
                    self.theme_stamp = stamp;
                    self.palette = ef_theme::load_active();
                    self.icons = Icons::new(&self.palette);
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
        // Scroll three rows per frame by moving the keyboard cursor, like holding ↓.
        self.cursor = Some((self.cursor.unwrap_or(0) + 3) % n);
        if b.frames.len() >= 600 {
            let mut f = b.frames.clone();
            f.sort_by(|a, b| a.total_cmp(b));
            let pct = |p: f64| f[((f.len() - 1) as f64 * p) as usize];
            let over = f.iter().filter(|&&x| x > 17.5).count();
            eprintln!(
                "scroll bench: {} frames over {} rows · p50 {:.2} ms · p95 {:.2} ms · p99 {:.2} ms · max {:.2} ms · {} frames > 17.5 ms",
                f.len(), n, pct(0.5), pct(0.95), pct(0.99), f[f.len() - 1], over
            );
            return iced::exit();
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![keyboard::listen().map(Message::Key), iced::time::every(Duration::from_secs(1)).map(|_| Message::ThemeTick)];
        if self.bench.is_some() || !self.first_frame_logged {
            subs.push(iced::window::frames().map(Message::Frame));
        }
        Subscription::batch(subs)
    }

    // ------------------------------------------------------------------ view

    pub fn view(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let list: Element<'_, Message> = match &self.loaded {
            Some(loaded) => {
                let empty = (!self.skeleton && self.order.is_empty()).then(|| {
                    if !self.query.is_empty() {
                        (format!("No matches for “{}”", self.query), "Nothing in this folder matches. Press Esc to clear the search.".to_string())
                    } else if loaded.listing.is_empty() {
                        ("This folder is empty".to_string(), "Drop files here, or paste with Ctrl+V.".to_string())
                    } else {
                        ("Only hidden files here".to_string(), "Press Ctrl+H to show them.".to_string())
                    }
                });
                FileList::new(
                    file_list::Model {
                        listing: &loaded.listing,
                        order: &self.order,
                        selected: &self.selected,
                        cursor: self.cursor,
                        generation: self.generation,
                        metadata_ready: loaded.metadata_ready,
                        sort: self.sort,
                        skeleton: self.skeleton,
                        empty,
                    },
                    p,
                    &self.icons,
                    &self.dates,
                    Message::List,
                )
                .into()
            }
            None => container(Space::new()).width(Length::Fill).height(Length::Fill).into(),
        };

        let mut center = column![].width(Length::Fill).height(Length::Fill);
        if let Some(n) = &self.notice {
            center = center.push(self.banner(n));
        }
        center = center.push(list);

        let body = row![self.sidebar(), vline(p.line), center].height(Length::Fill);
        let root = column![self.toolbar(), hline(p.line), body, hline(p.line), self.status_bar()];
        container(root)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style { background: Some(Background::Color(color(p.bg))), ..Default::default() })
            .into()
    }

    fn glyph<'a>(&self, name: &str, size: f32, tint: Color) -> Element<'a, Message> {
        svg(self.icons.glyph(name)).width(size).height(size).style(move |_, _| svg::Style { color: Some(tint) }).into()
    }

    fn glyph_button<'a>(&'a self, name: &str, label: &str, msg: Option<Message>, active: bool) -> Element<'a, Message> {
        let p = self.palette.clone();
        let enabled = msg.is_some();
        let tint = if active {
            color(p.accent_ink)
        } else if enabled {
            color(p.ink_muted)
        } else {
            Color { a: 0.45, ..color(p.ink_muted) }
        };
        let b = button(container(self.glyph(name, style::GLYPH, tint)).center(28.0))
            .padding(0)
            .on_press_maybe(msg)
            .style(move |_, status| {
                let bg = match status {
                    button::Status::Hovered => Some(Background::Color(color(p.state_hover))),
                    button::Status::Pressed => Some(Background::Color(color(p.state_press))),
                    _ if active => Some(Background::Color(color(p.state_active))),
                    _ => None,
                };
                button::Style { background: bg, text_color: color(p.ink), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() }
            });
        iced::widget::tooltip(b, self.tooltip(label), iced::widget::tooltip::Position::Bottom).gap(6).into()
    }

    fn tooltip<'a>(&self, label: &str) -> Element<'a, Message> {
        let p = self.palette.clone();
        container(text(label.to_string()).size(style::LABEL).font(style::FONT).color(color(p.ink)))
            .padding([3, 8])
            .style(move |_| container::Style {
                background: Some(Background::Color(color(p.bg_raised))),
                border: Border { color: color(p.line_strong), width: 1.0, radius: 0.0.into() },
                ..Default::default()
            })
            .into()
    }

    /// Breadcrumb segments: Home or the drive's name first, never raw mount paths.
    fn crumbs(&self) -> Vec<(Option<&'static str>, String, PathBuf)> {
        let home = home();
        let (mut base, mut out) = if let Ok(rest) = self.location.strip_prefix(&home) {
            (rest.to_path_buf(), vec![(Some("home"), "Home".to_string(), home.clone())])
        } else if let Some((v, mp)) = self.volumes.iter().find_map(|v| {
            v.mount_points.iter().find(|m| self.location.starts_with(m.as_str())).map(|m| (v, m.clone()))
        }) {
            let rest = self.location.strip_prefix(&mp).unwrap_or(Path::new("")).to_path_buf();
            (rest, vec![(Some("drive"), drive_name(v, &self.volumes), PathBuf::from(mp))])
        } else {
            (self.location.strip_prefix("/").unwrap_or(&self.location).to_path_buf(), vec![(Some("drive"), "Computer".to_string(), PathBuf::from("/"))])
        };
        let mut acc = out[0].2.clone();
        for c in base.components() {
            acc.push(c);
            out.push((None, c.as_os_str().to_string_lossy().into_owned(), acc.clone()));
        }
        base.clear();
        out
    }

    fn toolbar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let crumbs = self.crumbs();
        let n = crumbs.len();
        let mut trail = row![].spacing(0).align_y(Alignment::Center);
        for (idx, (icon, label, target)) in crumbs.into_iter().enumerate() {
            if idx > 0 {
                trail = trail.push(self.glyph("chevron-right", 12.0, color(p.ink_faint)));
            }
            let last = idx == n - 1;
            let ink = if last { color(p.ink_strong) } else { color(p.ink_muted) };
            let mut content = row![].spacing(style::SPACE_1 + 2.0).align_y(Alignment::Center);
            if let Some(icon) = icon {
                content = content.push(self.glyph(icon, 14.0, ink));
            }
            content = content.push(text(label).size(style::BODY).font(if last { style::FONT_BOLD } else { style::FONT }).color(ink).wrapping(text::Wrapping::None));
            let pal = p.clone();
            trail = trail.push(
                button(content)
                    .padding([3, 6])
                    .on_press_maybe((!last).then_some(Message::Navigate(target)))
                    .style(move |_, status| button::Style {
                        background: matches!(status, button::Status::Hovered | button::Status::Pressed).then(|| Background::Color(color(pal.state_hover))),
                        text_color: ink,
                        border: Border { radius: 2.0.into(), ..Border::default() },
                        ..Default::default()
                    }),
            );
        }
        let crumb_box = container(trail).width(Length::Fill).clip(true);

        let pal = p.clone();
        let search = text_input("Search this folder", &self.query)
            .id(SEARCH_ID)
            .on_input(Message::Search)
            .size(style::BODY)
            .font(style::FONT)
            .padding([5, 10])
            .width(240)
            .style(move |_, status| {
                let focused = matches!(status, text_input::Status::Focused { .. });
                text_input::Style {
                    background: Background::Color(color(pal.bg_deep)),
                    border: Border { color: if focused { color(pal.focus_ring) } else { color(pal.line_strong) }, width: 1.0, radius: 4.0.into() },
                    icon: color(pal.ink_muted),
                    placeholder: color(pal.ink_muted),
                    value: color(pal.ink),
                    selection: color(pal.accent_soft),
                }
            });

        let row = row![
            self.glyph_button("arrow-left", "Back  Alt+←", (!self.back.is_empty()).then_some(Message::Back), false),
            self.glyph_button("arrow-right", "Forward  Alt+→", (!self.forward.is_empty()).then_some(Message::Forward), false),
            self.glyph_button("arrow-up", "Parent folder  Alt+↑", self.location.parent().map(|_| Message::Up), false),
            container(Space::new()).width(1).height(18).style({
                let c = color(p.line);
                move |_| container::Style { background: Some(Background::Color(c)), ..Default::default() }
            }),
            crumb_box,
            search,
            self.glyph_button(
                if self.show_hidden { "eye" } else { "eye-off" },
                if self.show_hidden { "Hide hidden files  Ctrl+H" } else { "Show hidden files  Ctrl+H" },
                Some(Message::ToggleHidden),
                self.show_hidden,
            ),
        ]
        .spacing(style::SPACE_1)
        .align_y(Alignment::Center);
        container(row).height(style::TOOLBAR).padding([0, 12]).align_y(Alignment::Center).width(Length::Fill).into()
    }

    fn section<'a>(&self, title: &str, mark: Option<Color>, count: Option<usize>) -> Element<'a, Message> {
        let p = &self.palette;
        let mut r = row![].spacing(style::SPACE_3).align_y(Alignment::Center);
        if let Some(c) = mark {
            r = r.push(container(Space::new()).width(6).height(6).style(move |_| container::Style { background: Some(Background::Color(c)), ..Default::default() }));
        }
        r = r.push(text(title.to_uppercase()).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted)).width(Length::Fill));
        if let Some(n) = count {
            r = r.push(text(n.to_string()).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)));
        }
        container(r).padding([4, 12]).into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let home = home();
        let places = [
            ("home", "Home", home.clone()),
            ("file", "Documents", home.join("Documents")),
            ("download", "Downloads", home.join("Downloads")),
            ("image", "Pictures", home.join("Pictures")),
            ("music", "Music", home.join("Music")),
            ("video", "Videos", home.join("Videos")),
            ("trash", "Trash", home.join(".local/share/Trash/files")),
        ];
        let mut col = column![self.section("Linux", Some(color(p.world_linux)), None)].spacing(1);
        for (icon, label, path) in places {
            if path.is_dir() {
                let active = self.location == path;
                col = col.push(self.side_item(icon, label.to_string(), active, Message::Navigate(path)));
            }
        }
        col = col.push(Space::new().height(style::SPACE_5));
        col = col.push(self.section("Windows", Some(color(p.world_windows)), (!self.volumes.is_empty()).then_some(self.volumes.len())));
        if self.volumes.is_empty() {
            col = col.push(container(text("Looking for drives…").size(style::META).font(style::FONT).color(color(p.ink_muted))).padding([4, 12]));
        }
        for (i, v) in self.volumes.iter().enumerate() {
            col = col.push(self.drive_item(i, v));
        }
        container(col)
            .width(style::SIDEBAR)
            .height(Length::Fill)
            .padding([12, 8])
            .style(move |_| container::Style { background: Some(Background::Color(color(p.bg_sunken))), ..Default::default() })
            .into()
    }

    fn row_button<'a>(&self, content: Element<'a, Message>, active: bool, msg: Message) -> Element<'a, Message> {
        let p = self.palette.clone();
        button(content)
            .width(Length::Fill)
            .padding([0, 12])
            .on_press(msg)
            .style(move |_, status| {
                let bg = match status {
                    button::Status::Hovered => Some(Background::Color(color(p.state_hover))),
                    button::Status::Pressed => Some(Background::Color(color(p.state_press))),
                    _ if active => Some(Background::Color(color(p.state_active))),
                    _ => None,
                };
                button::Style { background: bg, text_color: color(p.ink), border: Border { radius: 2.0.into(), ..Border::default() }, ..Default::default() }
            })
            .into()
    }

    fn side_item<'a>(&'a self, icon: &str, label: String, active: bool, msg: Message) -> Element<'a, Message> {
        let p = &self.palette;
        let tint = if active { color(p.accent_ink) } else { color(p.ink_muted) };
        let ink = if active { color(p.ink_strong) } else { color(p.ink) };
        let r = row![
            self.glyph(icon, 16.0, tint),
            text(label).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(ink).width(Length::Fill).wrapping(text::Wrapping::None)
        ]
        .spacing(style::SPACE_4)
        .align_y(Alignment::Center);
        self.row_button(container(r).height(style::ROW).align_y(Alignment::Center).into(), active, msg)
    }

    fn pill<'a>(&self, label: &str, tone: Option<ef_theme::Semantic>) -> Element<'a, Message> {
        let p = &self.palette;
        let (bg, fg) = match tone {
            Some(t) => (color(t.soft), color(t.ink)),
            None => (color(p.state_hover), color(p.ink_muted)),
        };
        container(
            row![
                container(Space::new()).width(6).height(6).style(move |_| container::Style { background: Some(Background::Color(fg)), ..Default::default() }),
                text(label.to_string()).size(style::LABEL).font(style::FONT).color(fg).wrapping(text::Wrapping::None),
            ]
            .spacing(5)
            .align_y(Alignment::Center),
        )
        .padding([1, 6])
        .style(move |_| container::Style { background: Some(Background::Color(bg)), border: Border { radius: 2.0.into(), ..Border::default() }, ..Default::default() })
        .into()
    }

    fn usage_bar<'a>(&self, used: f32) -> Element<'a, Message> {
        let p = &self.palette;
        let fill = if used >= 0.97 { color(p.danger.base) } else if used >= 0.9 { color(p.warning.base) } else { color(p.world_windows) };
        let track = color(p.line);
        let used = (used.clamp(0.0, 1.0) * 1000.0) as u16;
        row![
            container(Space::new()).width(Length::FillPortion(used.max(1))).height(3).style(move |_| container::Style { background: Some(Background::Color(fill)), border: Border { radius: 1.0.into(), ..Border::default() }, ..Default::default() }),
            container(Space::new()).width(Length::FillPortion((1000 - used).max(1))).height(3).style(move |_| container::Style { background: Some(Background::Color(track)), ..Default::default() }),
        ]
        .into()
    }

    fn drive_item<'a>(&'a self, i: usize, v: &Volume) -> Element<'a, Message> {
        let p = &self.palette;
        let active = v.mount_points.iter().any(|m| self.location.starts_with(m.as_str()));
        let fs = self.volume_fs.get(i).cloned().flatten();
        let ink = if active { color(p.ink_strong) } else if v.is_mounted() { color(p.ink) } else { color(p.ink_muted) };
        let head = row![
            text(drive_name(v, &self.volumes)).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(ink).width(Length::Fill).wrapping(text::Wrapping::None)
        ]
        .align_y(Alignment::Center);
        let mut meta = String::new();
        match &fs {
            Some(f) => {
                fmt::size(f.free, &mut meta);
                meta.push_str(" free of ");
                fmt::size(f.total, &mut meta);
            }
            None => {
                meta.push_str("NTFS");
            }
        }
        let mut lines = column![head].spacing(3);
        if let Some(f) = &fs {
            lines = lines.push(self.usage_bar(f.used_fraction()));
        }
        let mut meta_row = row![text(meta).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None).width(Length::Fill)]
            .align_y(Alignment::Center);
        if !v.is_mounted() {
            meta_row = meta_row.push(self.pill("Not mounted", None));
        }
        lines = lines.push(meta_row);
        let tint = if active { color(p.accent_ink) } else { color(p.ink_muted) };
        let r = row![container(self.glyph("drive", 16.0, tint)).padding([2, 0]), lines].spacing(style::SPACE_4).align_y(Alignment::Start);
        self.row_button(container(r).padding([6, 0]).into(), active, Message::DriveClicked(i))
    }

    fn banner<'a>(&'a self, message: &'a str) -> Element<'a, Message> {
        let p = &self.palette;
        let w = p.warning;
        let icon = self.glyph("alert", 16.0, color(w.ink));
        container(
            row![icon, text(message).size(style::META).font(style::FONT).color(color(p.ink)).width(Length::Fill)]
                .spacing(style::SPACE_4)
                .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([10, 16])
        .style(move |_| container::Style { background: Some(Background::Color(color(w.soft))), ..Default::default() })
        .into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mut left = String::new();
        fmt::count(self.order.len(), &mut left);
        left.push_str(if self.order.len() == 1 { " item" } else { " items" });
        let (sel, bytes) = self.selection_stats();
        if sel > 0 {
            left.push_str("  ·  ");
            fmt::count(sel, &mut left);
            left.push_str(" selected · ");
            fmt::size(bytes, &mut left);
        }
        if !self.show_hidden {
            if let Some(l) = &self.loaded {
                let hidden = (0..l.listing.len()).filter(|&i| l.listing.is_hidden(i)).count();
                if hidden > 0 && self.query.is_empty() {
                    left.push_str("  ·  ");
                    fmt::count(hidden, &mut left);
                    left.push_str(" hidden");
                }
            }
        }
        let mut right = String::new();
        if self.loaded.as_ref().is_some_and(|l| !l.metadata_ready) {
            right.push_str("Reading details…  ·  ");
        }
        if std::env::var_os("ECHOFILES_TIMING").is_some() {
            if let Some(l) = self.loaded.as_ref().filter(|l| l.metadata_ready) {
                right.push_str(&format!("names {:.1} · sort {:.1} · details {:.1} · paint {:.1} ms  ·  ", l.names_ms, l.sort_ms, l.meta_ms, self.first_paint_ms));
            }
        }
        if let Some(f) = &self.fs {
            right.push_str(f.fs_type);
            right.push_str(" · ");
            fmt::size(f.free, &mut right);
            right.push_str(" free");
        }
        let row = row![
            text(left).size(style::META).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None),
            Space::new().width(Length::Fill),
            text(right).size(style::META).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None),
        ]
        .align_y(Alignment::Center);
        container(row)
            .height(style::STATUSBAR)
            .padding([0, 16])
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .style(move |_| container::Style { background: Some(Background::Color(color(p.bg_deep))), ..Default::default() })
            .into()
    }
}

/// Windows' own name for a volume: its label, or "Local Disk" when it has none. Duplicate
/// labels get their size so the two `AVS` partitions can be told apart (drive letters: M2).
pub fn drive_name(v: &Volume, all: &[Volume]) -> String {
    let base = if v.label.is_empty() { "Local Disk".to_string() } else { v.label.clone() };
    let dup = all.iter().filter(|o| o.label == v.label).count() > 1;
    if dup {
        let mut s = format!("{base} · ");
        fmt::size(v.size, &mut s);
        s
    } else {
        base
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

fn hline<'a>(c: ef_theme::Rgb) -> Element<'a, Message> {
    container(Space::new()).width(Length::Fill).height(1).style(move |_| container::Style { background: Some(Background::Color(color(c))), ..Default::default() }).into()
}

fn vline<'a>(c: ef_theme::Rgb) -> Element<'a, Message> {
    container(Space::new()).width(1).height(Length::Fill).style(move |_| container::Style { background: Some(Background::Color(color(c))), ..Default::default() }).into()
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Error text per the design system: name the thing, the reason, and the fix.
fn describe(dir: &Path, e: &std::io::Error) -> String {
    let name = dir.display();
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("Couldn't open “{name}” — you don't have permission to read it."),
        std::io::ErrorKind::NotFound => format!("“{name}” no longer exists."),
        _ => format!("Couldn't open “{name}”: {e}"),
    }
}


//! Everything that floats over the files: the right-click menu (design system
//! `ContextMenu`), dialogs (`Dialog`, `ConflictDialog`), toasts (`Toast`, `TransferToast`)
//! and the Ctrl+K command palette (`CommandPalette`).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use ef_config::{self as config, Scope};
use ef_core::fmt;
use ef_core::ops::{Kind, Phase, Resolution};
use ef_core::trash;
use iced::widget::{button, column, container, mouse_area, pin, row, scrollable, span, stack, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Point, Shadow, Task, Vector};

use crate::actions::{place_name, FileMsg};
use crate::app::{App, Message};
use crate::drives::DriveMsg;
use crate::settings::{Page, SettingsMsg};
use crate::style::{self, color};
use crate::widgets::{self as w, Variant};

pub const COMMAND_ID: &str = "command";
pub const PASSWORD_ID: &str = "password";
const MENU_W: f32 = 272.0;
const ITEM_H: f32 = 28.0;
const SEP_H: f32 = 9.0;
const HEAD_H: f32 = 24.0;

// ------------------------------------------------------------------------------ types

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub icon: &'static str,
    pub label: String,
    pub kbd: Option<&'static str>,
    pub hint: Option<String>,
    pub msg: Option<Message>,
    pub danger: bool,
    pub submenu: Vec<MenuItem>,
    /// A toggle that is on: a trailing check (View menu).
    pub checked: bool,
}

pub(crate) fn item(icon: &'static str, label: impl Into<String>, kbd: Option<&'static str>, msg: Message) -> Entry {
    Entry::Item(MenuItem { icon, label: label.into(), kbd, hint: None, msg: Some(msg), danger: false, submenu: Vec::new(), checked: false })
}

#[derive(Debug, Clone)]
pub enum Entry {
    Item(MenuItem),
    Sep,
    Heading(String),
}

impl Entry {
    fn height(&self) -> f32 {
        match self {
            Entry::Item(_) => ITEM_H,
            Entry::Sep => SEP_H,
            Entry::Heading(_) => HEAD_H,
        }
    }
}

pub struct Menu {
    pub at: Point,
    pub entries: Vec<Entry>,
    pub opened: Instant,
    pub hover: Option<usize>,
    pub sub: Option<usize>,
    /// The open submenu's items.
    pub sub_entries: Vec<Entry>,
}

/// What was right-clicked.
#[derive(Debug, Clone)]
pub enum MenuFor {
    Files { pane: u64, on_item: bool },
    Place { path: PathBuf, pinned: bool },
    Drive(usize),
    Tab(usize),
    /// Path segments folded into "…".
    Crumbs(Vec<(String, PathBuf)>),
    /// The toolbar's View dropdown.
    View,
    /// A place in the sidebar's Network section, by key.
    Network(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Toasts are for news and problems only; finished actions (trash, copy, rename…) don't
/// announce themselves — the files on screen already show it, and Ctrl+Z undoes.
pub enum Tone {
    Accent,
    Danger,
}

pub struct Toast {
    pub id: u64,
    pub tone: Tone,
    pub title: String,
    pub body: Option<String>,
    pub action: Option<(String, Message)>,
    pub at: Instant,
    pub hovered: bool,
}

pub enum Dialog {
    Delete { paths: Vec<PathBuf>, reason: Option<String>, opened: Instant },
    Conflict { id: u64, index: usize, all: bool, opened: Instant },
    BadNames { id: u64, opened: Instant },
    Password { prompt: ef_disks::polkit::Prompt, value: String, opened: Instant },
    WriteSystem { drive: usize, opened: Instant },
    /// A server wants a user name and password (design system `SignInDialog`).
    Login { ask: ef_net::LoginAsk, user: String, domain: String, password: String, guest: bool, remember: bool, opened: Instant },
    /// A server connection has a question, e.g. an unknown SSH host key.
    Question { ask: ef_net::QuestionAsk, opened: Instant },
    EmptyTrash { opened: Instant },
    /// Properties (Alt+Enter): facts only, closed with × or Esc.
    Properties { paths: Vec<PathBuf>, data: Option<std::sync::Arc<crate::preview::PreviewData>>, total: Option<std::sync::Arc<ef_core::ops::DirSize>>, cancel: std::sync::Arc<std::sync::atomic::AtomicBool>, opened: Instant },
}

impl Dialog {
    pub fn opened(&self) -> Instant {
        match self {
            Dialog::Delete { opened, .. }
            | Dialog::Conflict { opened, .. }
            | Dialog::BadNames { opened, .. }
            | Dialog::Password { opened, .. }
            | Dialog::WriteSystem { opened, .. }
            | Dialog::Login { opened, .. }
            | Dialog::Question { opened, .. }
            | Dialog::EmptyTrash { opened }
            | Dialog::Properties { opened, .. } => *opened,
        }
    }
}

/// One command palette result.
#[derive(Debug, Clone)]
pub struct Found {
    pub group: &'static str,
    pub icon: &'static str,
    pub label: String,
    pub hint: Option<String>,
    pub kbd: Option<&'static str>,
    pub msg: Message,
    pub matched: Vec<usize>,
    pub score: i32,
}

pub struct Command {
    pub query: String,
    pub cursor: usize,
    pub opened: Instant,
}

#[derive(Debug, Clone)]
pub enum UiMsg {
    CloseMenu,
    Pick(Box<Message>),
    MenuHover(usize),
    MenuMove(i32),
    MenuActivate,
    OpenMenu(MenuFor, Point),
    CloseDialog,
    DialogDefault,
    ConflictAll(bool),
    PasswordDraft(String),
    PasswordSubmit,
    ToastDismiss(u64),
    ToastHover(u64, bool),
    OpenCommand,
    CommandQuery(String),
    CommandMove(i32),
    CommandRun(Option<usize>),
    /// The pointer entered a drop target during a drag…
    DropHover(Option<PathBuf>),
    /// …or left one (only clears if it's still the current target).
    DropLeave(PathBuf),
}

/// Subsequence match: (score, matched char indices). Word starts and runs score higher.
pub fn fuzzy(query: &str, target: &str) -> Option<(i32, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let t: Vec<char> = target.chars().collect();
    let mut qi = 0;
    let mut score = 0;
    let mut matched = Vec::new();
    let mut last: Option<usize> = None;
    for (i, c) in t.iter().enumerate() {
        if qi < q.len() && c.to_lowercase().next() == Some(q[qi]) {
            score += 1;
            if i == 0 || !t[i - 1].is_alphanumeric() {
                score += 4;
            }
            if last == Some(i.wrapping_sub(1)) {
                score += 3;
            }
            matched.push(i);
            last = Some(i);
            qi += 1;
        }
    }
    (qi == q.len()).then(|| (score * 10 - t.len() as i32, matched))
}

// ------------------------------------------------------------------------------ logic

impl App {
    pub(crate) fn push_toast(&mut self, tone: Tone, title: String, body: Option<String>, action: Option<(String, Message)>) {
        self.toasts.push(Toast { id: crate::pane::next_id(), tone, title, body, action, at: Instant::now(), hovered: false });
    }

    /// Success toasts leave after 4 s (not while hovered); errors stay until dismissed.
    /// Finished transfers leave after 6 s.
    pub(crate) fn expire_toasts(&mut self) {
        self.toasts.retain(|t| t.tone == Tone::Danger || t.hovered || t.at.elapsed() < Duration::from_secs(if t.action.is_some() { 8 } else { 4 }));
        self.transfers.retain(|t| t.finished.is_none_or(|f| f.elapsed() < Duration::from_secs(6)));
    }

    pub(crate) fn dialog_cancelled(&mut self, d: Dialog) -> Task<Message> {
        match d {
            Dialog::Conflict { id, .. } | Dialog::BadNames { id, .. } => {
                self.transfers.retain(|t| t.id != id);
            }
            Dialog::Password { prompt, .. } => prompt.answer(None),
            Dialog::Login { ask, .. } => ask.answer(None),
            Dialog::Question { ask, .. } => ask.answer(None),
            Dialog::Properties { cancel, data, .. } => {
                cancel.store(true, Ordering::Relaxed);
                if let Some(d) = data {
                    d.cancel.store(true, Ordering::Relaxed);
                }
            }
            _ => {}
        }
        Task::none()
    }

    pub(crate) fn open_menu(&mut self, target: MenuFor, at: Point) {
        let entries = self.menu_entries(target);
        if !entries.is_empty() {
            self.menu = Some(Menu { at, entries, opened: Instant::now(), hover: None, sub: None, sub_entries: Vec::new() });
        }
    }

    /// Places to send files to (Move to ▸ / Copy to ▸).
    fn destinations(&self, kind: Kind, sources: &[PathBuf]) -> Vec<MenuItem> {
        let home = config::home();
        let mut out: Vec<(String, PathBuf, &'static str)> = Vec::new();
        if let Some(o) = self.other_pane() {
            out.push((format!("Other pane · {}", place_name(&o.location)), o.location.clone(), "columns"));
        }
        for (n, p, i) in [("Home", home.clone(), "home"), ("Documents", home.join("Documents"), "file"), ("Downloads", home.join("Downloads"), "download"), ("Pictures", home.join("Pictures"), "image")] {
            if p.is_dir() {
                out.push((n.to_string(), p, i));
            }
        }
        for s in &self.settings.sidebar.pinned {
            let p = config::expand(s);
            out.push((place_name(&p), p, "pin"));
        }
        for v in &self.volumes {
            if let Some(mp) = v.mount_points.first() {
                out.push((crate::drives::display_name(v, &self.volumes), PathBuf::from(mp), "drive"));
            }
        }
        out.into_iter()
            .filter(|(_, p, _)| !sources.iter().all(|s| s.parent() == Some(p.as_path())))
            .map(|(label, p, icon)| MenuItem {
                icon,
                label,
                kbd: None,
                hint: None,
                msg: Some(Message::File(FileMsg::Start { kind, sources: sources.to_vec(), dest: p })),
                danger: false,
                submenu: Vec::new(),
                checked: false,
            })
            .collect()
    }

    fn menu_entries(&self, target: MenuFor) -> Vec<Entry> {
        let f = Message::File;
        match target {
            MenuFor::Files { pane, on_item } => {
                let Some(p) = self.pane_by_id(pane) else { return Vec::new() };
                let in_trash = trash::is_trash_files(&p.location);
                let windows = crate::app::fs_of(&p.location).is_some_and(|f| f.fs_type.starts_with("ntfs"));
                let mut e = Vec::new();
                if on_item {
                    let targets = p.targets();
                    let one_dir = targets.len() == 1 && targets[0].is_dir();
                    if in_trash {
                        e.push(item("undo", "Restore", None, f(FileMsg::Restore)));
                        e.push(Entry::Sep);
                        e.push(Entry::Item(MenuItem { icon: "trash", label: "Delete permanently".into(), kbd: Some("Shift+Del"), hint: None, msg: Some(f(FileMsg::AskDelete)), danger: true, submenu: Vec::new(), checked: false }));
                        e.push(Entry::Sep);
                        e.push(item("info", "Properties", Some("Alt+Enter"), f(FileMsg::Properties)));
                        return e;
                    }
                    e.push(item("external", "Open", Some("Enter"), f(FileMsg::Open)));
                    if one_dir {
                        e.push(item("plus", "Open in new tab", Some("Ctrl+Enter"), f(FileMsg::OpenInTab)));
                        e.push(item("columns", "Open in other pane", Some("F3"), f(FileMsg::OpenInOther)));
                    } else {
                        e.push(item("external", "Open with…", None, f(FileMsg::OpenWith)));
                    }
                    e.push(Entry::Sep);
                    e.push(item("cut", "Cut", Some("Ctrl+X"), f(FileMsg::Cut)));
                    e.push(item("copy", "Copy", Some("Ctrl+C"), f(FileMsg::Copy)));
                    e.push(item("paste", "Paste", Some("Ctrl+V"), f(FileMsg::Paste)));
                    if one_dir && self.clip.is_some() {
                        e.push(item("paste", "Paste into folder", None, f(FileMsg::PasteInto(targets[0].clone()))));
                    }
                    let moves = self.destinations(Kind::Move, &targets);
                    if !moves.is_empty() {
                        e.push(Entry::Item(MenuItem { icon: "move", label: "Move to".into(), kbd: None, hint: None, msg: None, danger: false, submenu: moves, checked: false }));
                        e.push(Entry::Item(MenuItem { icon: "copy", label: "Copy to".into(), kbd: None, hint: None, msg: None, danger: false, submenu: self.destinations(Kind::Copy, &targets), checked: false }));
                    }
                    if targets.len() == 1 {
                        e.push(item("rename", "Rename", Some("F2"), f(FileMsg::StartRename)));
                    }
                    e.push(item("link", "Copy path", Some("Ctrl+Shift+C"), f(FileMsg::CopyPath)));
                    if windows {
                        let hint = targets.first().and_then(|t| self.windows_path(t));
                        e.push(Entry::Item(MenuItem { icon: "link", label: "Copy as Windows path".into(), kbd: None, hint, msg: Some(f(FileMsg::CopyWindowsPath)), danger: false, submenu: Vec::new(), checked: false }));
                    }
                    if one_dir {
                        let pinned = self.settings.sidebar.pinned.iter().any(|s| config::expand(s) == targets[0]);
                        e.push(if pinned { item("pin", "Unpin from sidebar", None, f(FileMsg::Unpin(targets[0].clone()))) } else { item("pin", "Pin to sidebar", None, f(FileMsg::Pin(targets[0].clone()))) });
                    }
                    if let Some(pid) = self.phone_id().filter(|i| self.phone_online(i))
                        && targets.iter().all(|t| t.is_file())
                    {
                        e.push(item("phone", format!("Send to {}", self.phone_name(&pid)), None, Message::Phone(crate::phone::PhoneMsg::SendPaths(targets.clone()))));
                    }
                    e.push(Entry::Sep);
                    e.push(item("trash", "Move to Trash", Some("Del"), f(FileMsg::Trash)));
                    e.push(Entry::Item(MenuItem { icon: "trash", label: "Delete permanently".into(), kbd: Some("Shift+Del"), hint: None, msg: Some(f(FileMsg::AskDelete)), danger: true, submenu: Vec::new(), checked: false }));
                    e.push(Entry::Sep);
                    e.push(item("info", "Properties", Some("Alt+Enter"), f(FileMsg::Properties)));
                } else {
                    if in_trash {
                        e.push(Entry::Item(MenuItem { icon: "trash", label: "Empty Trash".into(), kbd: None, hint: None, msg: Some(f(FileMsg::AskEmptyTrash)), danger: true, submenu: Vec::new(), checked: false }));
                        return e;
                    }
                    e.push(item("folder-plus", "New folder", Some("Ctrl+Shift+N"), f(FileMsg::NewFolder)));
                    e.push(item("file-plus", "New file", None, f(FileMsg::NewFile)));
                    e.push(item("paste", "Paste", Some("Ctrl+V"), f(FileMsg::Paste)));
                    e.push(item("check", "Select all", Some("Ctrl+A"), f(FileMsg::SelectAll)));
                    e.push(Entry::Sep);
                    e.push(if p.grid { item("list", "View as list", Some("Ctrl+1"), Message::SetGrid(false)) } else { item("grid", "View as grid", Some("Ctrl+2"), Message::SetGrid(true)) });
                    e.push(item(if self.show_hidden { "eye-off" } else { "eye" }, if self.show_hidden { "Hide hidden files" } else { "Show hidden files" }, Some("Ctrl+H"), Message::ToggleHidden));
                    e.push(Entry::Sep);
                    e.push(item("terminal", "Open terminal here", None, f(FileMsg::OpenTerminal)));
                    e.push(item("link", "Copy path", None, f(FileMsg::CopyPath)));
                    let here = p.location.clone();
                    let pinned = self.settings.sidebar.pinned.iter().any(|s| config::expand(s) == here);
                    if !pinned && here != config::home() {
                        e.push(item("pin", "Pin to sidebar", None, f(FileMsg::Pin(here))));
                    }
                    e.push(item("info", "Properties", Some("Alt+Enter"), f(FileMsg::Properties)));
                }
                e
            }
            MenuFor::Place { path, pinned } => {
                let mut e = vec![item("external", "Open", None, Message::Navigate(path.clone())), item("plus", "Open in new tab", None, Message::OpenTab(path.clone()))];
                if path == trash::home_trash().join("files") {
                    e.push(Entry::Sep);
                    e.push(Entry::Item(MenuItem { icon: "trash", label: "Empty Trash".into(), kbd: None, hint: None, msg: Some(f(FileMsg::AskEmptyTrash)), danger: true, submenu: Vec::new(), checked: false }));
                    return e;
                }
                if self.clip.is_some() {
                    e.push(item("paste", "Paste into folder", None, f(FileMsg::PasteInto(path.clone()))));
                }
                e.push(Entry::Sep);
                e.push(if pinned { item("pin", "Unpin from sidebar", None, f(FileMsg::Unpin(path))) } else { item("pin", "Pin to sidebar", None, f(FileMsg::Pin(path))) });
                e
            }
            MenuFor::Drive(i) => {
                let Some(v) = self.volumes.get(i) else { return Vec::new() };
                let d = Message::Drive;
                let mut e = Vec::new();
                if v.locked {
                    e.push(Entry::Heading("Locked with BitLocker".into()));
                    return e;
                }
                match v.mount_points.first() {
                    Some(mp) => {
                        e.push(item("external", "Open", None, Message::Navigate(PathBuf::from(mp))));
                        e.push(item("plus", "Open in new tab", None, Message::OpenTab(PathBuf::from(mp))));
                        if v.system && self.drive_state.get(&v.device).is_none_or(|s| s.read_only) {
                            e.push(Entry::Sep);
                            e.push(Entry::Item(MenuItem { icon: "unlock", label: "Allow writing…".into(), kbd: None, hint: Some("unsafe".into()), msg: Some(d(DriveMsg::AskWrite(i))), danger: true, submenu: Vec::new(), checked: false }));
                        }
                        e.push(Entry::Sep);
                        e.push(item("arrow-up", "Unmount", None, d(DriveMsg::Unmount(i))));
                    }
                    None => e.push(item("drive", "Mount and open", None, d(DriveMsg::Click(i)))),
                }
                e.push(item("sliders", "All drives", Some("Ctrl+Shift+D"), Message::ShowDrives));
                e
            }
            MenuFor::Network(key) => self.net_menu(&key),
            MenuFor::Crumbs(segs) => segs.into_iter().map(|(label, p)| item("folder", label, None, Message::Navigate(p))).collect(),
            MenuFor::View => {
                let grid = self.pane().grid;
                let toggle = |icon, label: &str, kbd, msg, on| Entry::Item(MenuItem { icon, label: label.into(), kbd: Some(kbd), hint: None, msg: Some(msg), danger: false, submenu: Vec::new(), checked: on });
                vec![
                    Entry::Heading("Layout".into()),
                    toggle("list", "List", "Ctrl+1", Message::SetGrid(false), !grid),
                    toggle("grid", "Grid", "Ctrl+2", Message::SetGrid(true), grid),
                    Entry::Sep,
                    Entry::Heading("Show".into()),
                    toggle("columns", "Dual pane", "F3", Message::ToggleDual, self.dual()),
                    toggle("sidebar", "Preview pane", "Space", Message::TogglePreview, self.settings.appearance.preview),
                    toggle("eye", "Hidden files", "Ctrl+H", Message::ToggleHidden, self.show_hidden),
                ]
            }
            MenuFor::Tab(i) => {
                let mut e = vec![item("plus", "New tab", Some("Ctrl+T"), Message::NewTab)];
                if self.tabs.len() > 1 {
                    e.push(item("close", "Close tab", Some("Ctrl+W"), Message::CloseTab(i)));
                }
                e
            }
        }
    }

    fn menu_items(&self) -> Vec<(usize, &MenuItem)> {
        self.menu.as_ref().map(|m| m.entries.iter().enumerate().filter_map(|(i, e)| if let Entry::Item(it) = e { Some((i, it)) } else { None }).collect()).unwrap_or_default()
    }

    /// Everything the palette can do, filtered by `query`.
    pub(crate) fn command_results(&self, query: &str) -> Vec<Found> {
        let q = query.trim();
        let mut out = Vec::new();
        // Paths: `/`, `~` or `X:\` switch to path completion.
        if q.starts_with('/') || q.starts_with('~') {
            let full = config::expand(q);
            let (dir, prefix) = if q.ends_with('/') { (full.clone(), String::new()) } else { (full.parent().map(PathBuf::from).unwrap_or_default(), full.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()) };
            if full.is_dir() {
                out.push(Found { group: "Go to", icon: "folder", label: config::tilde(&full), hint: None, kbd: Some("Enter"), msg: Message::Navigate(full.clone()), matched: Vec::new(), score: 1000 });
            }
            if let Ok(rd) = std::fs::read_dir(&dir) {
                let mut dirs: Vec<PathBuf> = rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with(&prefix) && (self.show_hidden || !n.to_string_lossy().starts_with('.')))).collect();
                dirs.sort();
                for p in dirs.into_iter().take(12) {
                    if p != full {
                        out.push(Found { group: "Go to", icon: "folder", label: config::tilde(&p), hint: None, kbd: None, msg: Message::Navigate(p), matched: Vec::new(), score: 0 });
                    }
                }
            }
            return out;
        }
        let f = Message::File;
        let mut add = |group: &'static str, icon: &'static str, label: String, hint: Option<String>, kbd: Option<&'static str>, msg: Message| {
            if let Some((score, matched)) = fuzzy(q, &label) {
                out.push(Found { group, icon, label, hint, kbd, msg, matched, score });
            }
        };
        let actions: Vec<(&'static str, &str, Option<&'static str>, Message)> = vec![
            ("folder-plus", "New folder", Some("Ctrl+Shift+N"), f(FileMsg::NewFolder)),
            ("file-plus", "New file", None, f(FileMsg::NewFile)),
            ("paste", "Paste", Some("Ctrl+V"), f(FileMsg::Paste)),
            ("undo", "Undo", Some("Ctrl+Z"), f(FileMsg::Undo)),
            ("rename", "Rename", Some("F2"), f(FileMsg::StartRename)),
            ("trash", "Move to Trash", Some("Del"), f(FileMsg::Trash)),
            ("check", "Select all", Some("Ctrl+A"), f(FileMsg::SelectAll)),
            ("plus", "New tab", Some("Ctrl+T"), Message::NewTab),
            ("close", "Close tab", Some("Ctrl+W"), Message::CloseTab(self.tab)),
            ("columns", "Toggle dual pane", Some("F3"), Message::ToggleDual),
            ("sidebar", "Toggle preview pane", Some("Space"), Message::TogglePreview),
            ("sidebar", "Toggle sidebar", Some("Ctrl+B"), Message::ToggleSidebar),
            ("eye", "Toggle hidden files", Some("Ctrl+H"), Message::ToggleHidden),
            ("grid", "View as grid", Some("Ctrl+2"), Message::SetGrid(true)),
            ("list", "View as list", Some("Ctrl+1"), Message::SetGrid(false)),
            ("drive", "Show all drives", Some("Ctrl+Shift+D"), Message::ShowDrives),
            ("network", "Connect to server…", Some("Ctrl+Shift+S"), Message::Net(crate::network::NetMsg::Open(None))),
            ("terminal", "Open terminal here", None, f(FileMsg::OpenTerminal)),
            ("link", "Copy path", Some("Ctrl+Shift+C"), f(FileMsg::CopyPath)),
            ("search", "Search everywhere", Some("Ctrl+E"), Message::SetScope(Scope::Everywhere)),
            ("trash", "Empty Trash", None, f(FileMsg::AskEmptyTrash)),
            ("refresh", "Reload", Some("Ctrl+R"), Message::Reload),
        ];
        for (icon, label, kbd, msg) in actions {
            add("Actions", icon, label.to_string(), None, kbd, msg);
        }
        {
            use crate::phone::{PhoneMsg, PhonePage, Want};
            let ph = Message::Phone;
            match self.phone_id() {
                None => add("Phone", "phone", "Connect phone…".into(), None, None, ph(PhoneMsg::Dialog(true))),
                Some(id) => {
                    let name = self.phone_name(&id);
                    add("Phone", "phone", format!("Open {name}"), None, None, ph(PhoneMsg::Select(id.clone())));
                    add("Phone", "image", format!("Photos on {name}"), None, None, ph(PhoneMsg::Open(PhonePage::Photos)));
                    add("Phone", "folder", format!("Files on {name}"), None, None, ph(PhoneMsg::Files(Want::Files)));
                    add("Phone", "message", format!("Messages on {name}"), None, None, ph(PhoneMsg::Open(PhonePage::Messages)));
                    add("Phone", "bell", format!("Notifications from {name}"), None, None, ph(PhoneMsg::Open(PhonePage::Notifications)));
                    add("Phone", "phone-ring", format!("Ring {name}"), None, None, ph(PhoneMsg::Ring));
                    add("Phone", "upload", format!("Send files to {name}…"), None, None, ph(PhoneMsg::SendFiles));
                    add("Phone", "plus", "Connect another phone…".into(), None, None, ph(PhoneMsg::Dialog(true)));
                }
            }
        }
        for (i, v) in self.volumes.iter().enumerate() {
            let name = crate::drives::display_name(v, &self.volumes);
            let mut size = String::new();
            fmt::size(v.size, &mut size);
            if v.locked {
                continue;
            }
            if v.is_mounted() {
                add("Actions", "drive", format!("Unmount {name}"), None, None, Message::Drive(DriveMsg::Unmount(i)));
                if v.system && self.drive_state.get(&v.device).is_none_or(|s| s.read_only) {
                    add("Actions", "lock", format!("Mount {name} read-write"), Some("unsafe".into()), None, Message::Drive(DriveMsg::AskWrite(i)));
                }
            } else {
                add("Actions", "drive", format!("Mount {name}"), Some(format!("{size} NTFS")), None, Message::Drive(DriveMsg::Click(i)));
            }
        }
        for place in self.net_places() {
            let (icon, label) = match place.mount {
                Some(_) => ("network", format!("Open {}", place.name)),
                None => ("network", format!("Connect to {}", place.name)),
            };
            let hint = place.address.as_ref().map(|a| format!("{} · {}", a.protocol.label(), a.who_where()));
            if place.mount.is_some() {
                add("Actions", "arrow-up", format!("Disconnect {}", place.name), hint.clone(), None, Message::Net(crate::network::NetMsg::Disconnect(place.key.clone())));
            }
            add("Go to", icon, label, hint, None, Message::Net(crate::network::NetMsg::Click(place.key.clone())));
        }
        let home = config::home();
        let mut places: Vec<(&'static str, PathBuf)> = vec![("home", home.clone())];
        for n in ["Documents", "Downloads", "Pictures", "Music", "Videos", "Desktop"] {
            places.push(("folder", home.join(n)));
        }
        places.push(("trash", trash::home_trash().join("files")));
        for s in &self.settings.sidebar.pinned {
            places.push(("pin", config::expand(s)));
        }
        for p in self.recent.iter().rev() {
            places.push(("history", p.clone()));
        }
        for v in &self.volumes {
            if let Some(mp) = v.mount_points.first() {
                places.push(("drive", PathBuf::from(mp)));
            }
        }
        let mut seen = std::collections::HashSet::new();
        for (icon, p) in places {
            if !p.is_dir() || !seen.insert(p.clone()) {
                continue;
            }
            let label = self.volumes.iter().find(|v| v.mount_points.first().is_some_and(|m| *m == p)).map(|v| crate::drives::display_name(v, &self.volumes)).unwrap_or_else(|| config::tilde(&p));
            add("Go to", icon, label, (icon == "history").then(|| "recent".to_string()), None, Message::Navigate(p));
        }
        for (page, icon, label) in [(Page::General, "sliders", "Settings: General"), (Page::Search, "search", "Settings: Search & index"), (Page::Agents, "terminal", "Settings: AI agents"), (Page::Phone, "phone", "Settings: Phone"), (Page::Appearance, "image", "Settings: Appearance"), (Page::About, "info", "Settings: About & shortcuts")] {
            add("Settings", icon, label.to_string(), None, None, Message::Settings(SettingsMsg::OpenPage(page)));
        }
        if !q.is_empty() {
            // Groups stay in order; within a group the best match leads.
            let rank = |g: &str| match g {
                "Actions" => 0,
                "Phone" => 1,
                "Go to" => 2,
                _ => 3,
            };
            out.sort_by_key(|a| (rank(a.group), -a.score));
        }
        out.truncate(60);
        out
    }

    pub(crate) fn ui_update(&mut self, msg: UiMsg) -> Task<Message> {
        match msg {
            UiMsg::CloseMenu => {
                self.menu = None;
                Task::none()
            }
            UiMsg::Pick(m) => {
                self.menu = None;
                self.command = None;
                self.update(*m)
            }
            UiMsg::MenuHover(i) => {
                if let Some(m) = &mut self.menu {
                    m.hover = Some(i);
                    m.sub_entries = match m.entries.get(i) {
                        Some(Entry::Item(it)) => it.submenu.iter().cloned().map(Entry::Item).collect(),
                        _ => Vec::new(),
                    };
                    m.sub = (!m.sub_entries.is_empty()).then_some(i);
                }
                Task::none()
            }
            UiMsg::MenuMove(d) => {
                let items: Vec<usize> = self.menu_items().into_iter().filter(|(_, it)| it.msg.is_some() || !it.submenu.is_empty()).map(|(i, _)| i).collect();
                if let Some(m) = &mut self.menu {
                    if items.is_empty() {
                        return Task::none();
                    }
                    let cur = m.hover.and_then(|h| items.iter().position(|&x| x == h));
                    let next = match cur {
                        None => if d > 0 { 0 } else { items.len() - 1 },
                        Some(c) => (c as i32 + d).rem_euclid(items.len() as i32) as usize,
                    };
                    m.hover = Some(items[next]);
                    m.sub = None;
                }
                Task::none()
            }
            UiMsg::MenuActivate => {
                let Some(m) = &self.menu else { return Task::none() };
                let Some(Entry::Item(it)) = m.hover.and_then(|h| m.entries.get(h)) else { return Task::none() };
                match &it.msg {
                    Some(msg) => {
                        let msg = msg.clone();
                        self.menu = None;
                        self.update(msg)
                    }
                    None => {
                        let h = m.hover;
                        match h {
                            Some(h) => self.ui_update(UiMsg::MenuHover(h)),
                            None => Task::none(),
                        }
                    }
                }
            }
            UiMsg::OpenMenu(target, at) => {
                self.open_menu(target, at);
                Task::none()
            }
            UiMsg::CloseDialog => match self.dialog.take() {
                Some(d) => self.dialog_cancelled(d),
                None => Task::none(),
            },
            UiMsg::DialogDefault => match &self.dialog {
                Some(Dialog::Conflict { all, .. }) => {
                    let all = *all;
                    self.file_update(FileMsg::Resolve(Resolution::KeepBoth, all))
                }
                Some(Dialog::BadNames { .. }) => self.file_update(FileMsg::FixNames(true)),
                Some(Dialog::Password { .. }) => self.ui_update(UiMsg::PasswordSubmit),
                Some(Dialog::Login { .. }) => self.net_update(crate::network::NetMsg::LoginSubmit),
                Some(Dialog::Properties { .. }) => self.ui_update(UiMsg::CloseDialog),
                // Destructive dialogs never act on Enter.
                _ => Task::none(),
            },
            UiMsg::ConflictAll(on) => {
                if let Some(Dialog::Conflict { all, .. }) = &mut self.dialog {
                    *all = on;
                }
                Task::none()
            }
            UiMsg::PasswordDraft(v) => {
                if let Some(Dialog::Password { value, .. }) = &mut self.dialog {
                    *value = v;
                }
                Task::none()
            }
            UiMsg::PasswordSubmit => {
                if let Some(Dialog::Password { prompt, value, .. }) = self.dialog.take() {
                    prompt.answer(Some(value));
                }
                Task::none()
            }
            UiMsg::ToastDismiss(id) => {
                // Dismissing a phone's "wants to send" notice declines the file.
                let offer = self.toasts.iter().find(|t| t.id == id).and_then(|t| match &t.action {
                    Some((_, Message::Phone(crate::phone::PhoneMsg::AcceptOffer(x)))) => Some(*x),
                    _ => None,
                });
                self.toasts.retain(|t| t.id != id);
                match offer {
                    Some(x) => self.phone_update(crate::phone::PhoneMsg::DeclineOffer(x)),
                    None => Task::none(),
                }
            }
            UiMsg::ToastHover(id, on) => {
                if let Some(t) = self.toasts.iter_mut().find(|t| t.id == id) {
                    t.hovered = on;
                    if !on {
                        t.at = Instant::now() - Duration::from_secs(2);
                    }
                }
                Task::none()
            }
            UiMsg::OpenCommand => {
                self.menu = None;
                self.command = Some(Command { query: String::new(), cursor: 0, opened: Instant::now() });
                iced::widget::operation::focus(COMMAND_ID)
            }
            UiMsg::CommandQuery(q) => {
                if let Some(c) = &mut self.command {
                    c.query = q;
                    c.cursor = 0;
                }
                Task::none()
            }
            UiMsg::CommandMove(d) => {
                let n = self.command.as_ref().map(|c| self.command_results(&c.query).len()).unwrap_or(0);
                if let Some(c) = &mut self.command
                    && n > 0 {
                        c.cursor = (c.cursor as i32 + d).rem_euclid(n as i32) as usize;
                    }
                Task::none()
            }
            UiMsg::DropHover(p) => {
                if self.drag.is_some() {
                    self.drop_place = p;
                }
                Task::none()
            }
            UiMsg::DropLeave(p) => {
                if self.drop_place.as_ref() == Some(&p) {
                    self.drop_place = None;
                }
                Task::none()
            }
            UiMsg::CommandRun(i) => {
                let Some(c) = &self.command else { return Task::none() };
                let results = self.command_results(&c.query);
                let Some(found) = results.get(i.unwrap_or(c.cursor)) else { return Task::none() };
                let msg = found.msg.clone();
                self.command = None;
                self.update(msg)
            }
        }
    }

    // ------------------------------------------------------------------ views

    fn t(&self, opened: Instant, ms: u64) -> f32 {
        if !self.animations {
            return 1.0;
        }
        (opened.elapsed().as_secs_f32() * 1000.0 / ms as f32).min(1.0)
    }

    fn menu_panel<'a>(&'a self, entries: &'a [Entry], hover: Option<usize>, sub_level: bool) -> Element<'a, Message> {
        let p = &self.palette;
        let mut col = column![].spacing(0);
        for (i, e) in entries.iter().enumerate() {
            match e {
                Entry::Sep => col = col.push(container(w::hline::<Message>(p.line)).padding([4, 0])),
                Entry::Heading(h) => col = col.push(container(text(h.to_uppercase()).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted))).height(HEAD_H).padding([0, 10]).align_y(Alignment::Center)),
                Entry::Item(it) => {
                    let active = hover == Some(i) && !sub_level;
                    let enabled = it.msg.is_some() || !it.submenu.is_empty();
                    let ink = if it.danger { color(p.danger.ink) } else if active { color(p.accent_ink) } else { color(p.ink) };
                    let glyph = if it.danger { color(p.danger.ink) } else if active { color(p.accent_ink) } else { color(p.ink_muted) };
                    let mut r = row![w::glyph(&self.icons, it.icon, 16.0, glyph), text(it.label.clone()).size(style::BODY).font(style::FONT).color(ink).wrapping(text::Wrapping::None).width(Length::Fill)]
                        .spacing(style::SPACE_3 + 2.0)
                        .align_y(Alignment::Center);
                    if let Some(h) = &it.hint {
                        r = r.push(container(text(h.clone()).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None)).max_width(110).clip(true));
                    }
                    if let Some(k) = it.kbd {
                        r = r.push(w::kbd_chip(p, k));
                    }
                    if !it.submenu.is_empty() {
                        r = r.push(w::glyph(&self.icons, "chevron-right", 12.0, color(p.ink_muted)));
                    }
                    if it.checked {
                        r = r.push(w::glyph(&self.icons, "check", 14.0, color(p.accent_ink)));
                    } else if entries.iter().any(|e| matches!(e, Entry::Item(i) if i.checked)) {
                        r = r.push(Space::new().width(14));
                    }
                    let pal = p.clone();
                    let msg = match (&it.msg, it.submenu.is_empty()) {
                        (Some(m), _) => Some(Message::Ui(UiMsg::Pick(Box::new(m.clone())))),
                        (None, false) => Some(Message::Ui(UiMsg::MenuHover(i))),
                        _ => None,
                    };
                    let b = button(container(r).height(ITEM_H).align_y(Alignment::Center)).padding([0, 10]).width(Length::Fill).on_press_maybe(msg).style(move |_, status| {
                        let hot = active || matches!(status, button::Status::Hovered | button::Status::Pressed);
                        button::Style {
                            background: hot.then(|| Background::Color(color(pal.state_hover))),
                            border: Border { color: if hot { color(pal.line) } else { Color::TRANSPARENT }, width: 1.0, radius: 0.0.into() },
                            text_color: color(pal.ink),
                            ..Default::default()
                        }
                    });
                    let el: Element<'a, Message> = if enabled { b.into() } else { container(b).style(|_| container::Style { text_color: None, ..Default::default() }).into() };
                    col = col.push(if sub_level { el } else { mouse_area(el).on_enter(Message::Ui(UiMsg::MenuHover(i))).into() });
                }
            }
        }
        let pal = p.clone();
        container(col)
            .width(MENU_W)
            .padding(4)
            .style(move |_| container::Style {
                background: Some(Background::Color(color(pal.bg_raised))),
                border: Border { color: color(pal.line_strong), width: 1.0, radius: 0.0.into() },
                shadow: Shadow { color: Color { a: 0.35, ..Color::BLACK }, offset: Vector::new(0.0, 6.0), blur_radius: 18.0 },
                ..Default::default()
            })
            .into()
    }

    /// Where a panel of `size` fits inside the window, near `at`.
    fn fit(&self, at: Point, w: f32, h: f32) -> Point {
        let size = self.window_size;
        let x = if at.x + w > size.width - 4.0 { (at.x - w).max(4.0) } else { at.x };
        let y = if at.y + h > size.height - 4.0 { (size.height - h - 4.0).max(4.0) } else { at.y };
        Point::new(x, y)
    }

    pub(crate) fn menu_layer(&self) -> Option<Element<'_, Message>> {
        let m = self.menu.as_ref()?;
        let h: f32 = m.entries.iter().map(Entry::height).sum::<f32>() + 8.0;
        let t = self.t(m.opened, 140);
        let at = self.fit(m.at, MENU_W, h);
        let mut layers = vec![
            // Clicking anywhere else closes the menu (and doesn't click through).
            mouse_area(container(Space::new()).width(Length::Fill).height(Length::Fill)).on_press(Message::Ui(UiMsg::CloseMenu)).on_right_press(Message::Ui(UiMsg::CloseMenu)).into(),
            pin(self.menu_panel(&m.entries, m.hover, false)).x(at.x).y(at.y - 4.0 * (1.0 - t)).into(),
        ];
        if let Some(si) = m.sub.filter(|_| !m.sub_entries.is_empty()) {
            let y: f32 = m.entries[..si].iter().map(Entry::height).sum::<f32>() + at.y;
            let sh = m.sub_entries.len() as f32 * ITEM_H + 8.0;
            let sx = if at.x + MENU_W * 2.0 > self.window_size.width { at.x - MENU_W + 4.0 } else { at.x + MENU_W - 4.0 };
            let sp = self.fit(Point::new(sx, y), MENU_W, sh);
            layers.push(pin(self.menu_panel(&m.sub_entries, None, true)).x(sp.x).y(sp.y).into());
        }
        Some(stack(layers).width(Length::Fill).height(Length::Fill).into())
    }

    fn dialog_frame<'a>(&'a self, icon: &'static str, title: String, body: Element<'a, Message>, footer: Vec<Element<'a, Message>>, danger: bool, opened: Instant) -> Element<'a, Message> {
        self.dialog_frame_with(icon, title, body, footer, danger, false, opened)
    }

    /// `closable` adds a × in the corner (Properties, which has nothing to decide).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dialog_frame_with<'a>(&'a self, icon: &'static str, title: String, body: Element<'a, Message>, footer: Vec<Element<'a, Message>>, danger: bool, closable: bool, opened: Instant) -> Element<'a, Message> {
        let p = &self.palette;
        let t = self.t(opened, 180);
        let mut head = row![
            iced::widget::svg(self.icons.color(icon)).width(32).height(32),
            text(title).size(16).font(style::FONT_BOLD).color(color(p.ink_strong)).width(Length::Fill),
        ]
        .spacing(style::SPACE_4)
        .align_y(Alignment::Center);
        if closable {
            head = head.push(self.tip(w::icon_button(p, &self.icons, "close", Some(Message::Ui(UiMsg::CloseDialog)), false), "Close", Some("Esc")));
        }
        let mut foot = row![Space::new().width(Length::Fill)].spacing(style::SPACE_3).align_y(Alignment::Center);
        for b in footer {
            foot = foot.push(b);
        }
        let edge = if danger { color(p.danger.base) } else { color(p.accent) };
        let pal = p.clone();
        let panel = container(column![head, body, foot].spacing(style::SPACE_5))
            .width(540.0 * (0.98 + 0.02 * t))
            .padding(20)
            .style(move |_| container::Style {
                background: Some(Background::Color(color(pal.bg_raised))),
                border: Border { color: edge, width: 2.0, radius: 0.0.into() },
                shadow: Shadow { color: Color { a: 0.45, ..Color::BLACK }, offset: Vector::new(0.0, 12.0), blur_radius: 32.0 },
                ..Default::default()
            });
        let scrim = color(p.scrim);
        let back = mouse_area(container(Space::new()).width(Length::Fill).height(Length::Fill).style(move |_| container::Style { background: Some(Background::Color(Color { a: scrim.a * t, ..scrim })), ..Default::default() }));
        stack![back, container(iced::widget::opaque(panel)).center(Length::Fill)].into()
    }

    fn body_text<'a>(&self, s: String) -> Element<'a, Message> {
        text(s).size(style::BODY).font(style::FONT).color(color(self.palette.ink)).into()
    }

    fn btn<'a>(&self, label: &str, icon: Option<&str>, kbd: Option<&str>, v: Variant, msg: Message) -> Element<'a, Message> {
        w::text_button(&self.palette, &self.icons, label, icon, kbd, v, Some(msg))
    }

    pub(crate) fn dialog_layer(&self) -> Option<Element<'_, Message>> {
        let d = self.dialog.as_ref()?;
        let p = &self.palette;
        let cancel = || self.btn("Cancel", None, Some("Esc"), Variant::Ghost, Message::Ui(UiMsg::CloseDialog));
        Some(match d {
            Dialog::Delete { paths, reason, opened } => {
                let what = if paths.len() == 1 { format!("“{}”", paths[0].file_name().unwrap_or_default().to_string_lossy()) } else { format!("{} items", paths.len()) };
                let mut body = String::new();
                if let Some(r) = reason {
                    body.push_str(r);
                    body.push(' ');
                }
                body.push_str("Deleting permanently can't be undone.");
                let mut foot = vec![cancel()];
                if reason.is_none() {
                    foot.push(self.btn("Move to Trash", Some("trash"), None, Variant::Secondary, Message::Ui(UiMsg::Pick(Box::new(Message::File(FileMsg::Trash))))));
                }
                let paths = paths.clone();
                foot.push(self.btn("Delete permanently", Some("trash"), None, Variant::Danger, Message::Ui(UiMsg::Pick(Box::new(Message::File(FileMsg::Delete(paths)))))));
                self.dialog_frame("trash-full", format!("Delete {what} permanently?"), self.body_text(body), foot, true, *opened)
            }
            Dialog::EmptyTrash { opened } => {
                let n = std::fs::read_dir(trash::home_trash().join("files")).map(|r| r.count()).unwrap_or(0);
                let foot = vec![cancel(), self.btn("Empty Trash", Some("trash"), None, Variant::Danger, Message::Ui(UiMsg::Pick(Box::new(Message::File(FileMsg::EmptyTrash)))))];
                self.dialog_frame("trash-full", "Empty the Trash?".into(), self.body_text(format!("{n} {} will be deleted permanently. This can't be undone.", if n == 1 { "item" } else { "items" })), foot, true, *opened)
            }
            Dialog::Conflict { id, index, all, opened } => {
                let t = self.transfers.iter().find(|t| t.id == *id)?;
                let plan = t.plan.as_ref()?;
                let c = plan.conflicts.get(*index)?;
                let name = c.dst.file_name().unwrap_or_default().to_string_lossy().into_owned();
                let remaining = plan.conflicts.len() - index;
                let card = |label: &str, size: u64, mtime: i64, newer: bool, dir: bool| -> Element<'_, Message> {
                    let mut s = String::new();
                    if dir {
                        s.push_str("Folder");
                    } else {
                        fmt::size(size, &mut s);
                    }
                    let mut d = String::new();
                    self.dates.format(mtime, &mut d);
                    let mut col = column![
                        text(label.to_uppercase()).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted)),
                        text(s).size(style::BODY).font(style::FONT).color(color(p.ink)),
                        text(d).size(style::META).font(style::FONT).color(color(p.ink_muted)),
                    ]
                    .spacing(3);
                    if newer {
                        col = col.push(text("Newer").size(style::LABEL).font(style::FONT_BOLD).color(color(p.success.ink)));
                    }
                    let pal = p.clone();
                    container(col).padding(10).width(Length::Fill).style(move |_| container::Style { background: Some(Background::Color(color(pal.bg_deep))), border: Border { color: color(pal.line), width: 1.0, radius: 2.0.into() }, ..Default::default() }).into()
                };
                let mut body = column![
                    self.body_text(format!("A {} with this name is already in {}. Which one do you want to keep?", if c.dirs { "folder" } else { "file" }, place_name(&plan.dest))),
                    row![card("Already there", c.dst_size, c.dst_mtime, c.dst_mtime > c.src_mtime, c.dirs), card(if t.kind == Kind::Copy { "Copying" } else { "Moving" }, c.src_size, c.src_mtime, c.src_mtime > c.dst_mtime, c.dirs)].spacing(style::SPACE_3),
                ]
                .spacing(style::SPACE_4);
                if remaining > 1 {
                    body = body.push(w::checkbox(p, &self.icons, &format!("Do this for all {remaining} conflicts"), *all, |on| Message::Ui(UiMsg::ConflictAll(on))));
                }
                let all = *all;
                let foot = vec![
                    cancel(),
                    self.btn("Skip", None, None, Variant::Secondary, Message::File(FileMsg::Resolve(Resolution::Skip, all))),
                    self.btn(if c.dirs { "Merge" } else { "Replace" }, None, None, Variant::Secondary, Message::File(FileMsg::Resolve(Resolution::Replace, all))),
                    self.btn("Keep both", None, Some("Enter"), Variant::Primary, Message::File(FileMsg::Resolve(Resolution::KeepBoth, all))),
                ];
                self.dialog_frame(if c.dirs { "folder" } else { "file" }, format!("“{name}” already exists"), body.into(), foot, false, *opened)
            }
            Dialog::BadNames { id, opened } => {
                let t = self.transfers.iter().find(|t| t.id == *id)?;
                let plan = t.plan.as_ref()?;
                let mut body = column![self.body_text(format!("{} Windows doesn't allow. EchoFiles can rename them as they're copied:", if plan.bad_names.len() == 1 { "One name uses characters".to_string() } else { format!("{} names use characters", plan.bad_names.len()) }))].spacing(style::SPACE_3);
                for (path, _why, fixed) in plan.bad_names.iter().take(6) {
                    body = body.push(text(format!("{}  →  {fixed}", path.file_name().unwrap_or_default().to_string_lossy())).size(style::META).font(style::FONT).color(color(p.ink_muted)));
                }
                if plan.bad_names.len() > 6 {
                    body = body.push(text(format!("…and {} more", plan.bad_names.len() - 6)).size(style::META).font(style::FONT).color(color(p.ink_muted)));
                }
                let foot = vec![cancel(), self.btn("Rename and continue", None, Some("Enter"), Variant::Primary, Message::File(FileMsg::FixNames(true)))];
                self.dialog_frame("file-error", "Some names won't work on Windows".into(), body.into(), foot, false, *opened)
            }
            Dialog::Password { prompt, value, opened } => {
                let field = text_input("Password", value)
                    .id(PASSWORD_ID)
                    .secure(true)
                    .on_input(|v| Message::Ui(UiMsg::PasswordDraft(v)))
                    .on_submit(Message::Ui(UiMsg::PasswordSubmit))
                    .size(style::BODY)
                    .font(style::FONT)
                    .padding([6, 10])
                    .style(w::field_style(p, prompt.retry));
                let mut body = column![self.body_text(format!("{}. Enter the password for {}.", prompt.message.trim_end_matches('.'), prompt.user)), field].spacing(style::SPACE_3);
                if prompt.retry {
                    body = body.push(row![w::glyph(&self.icons, "error", 14.0, color(p.danger.ink)), text("That password didn't work. Try again.").size(style::META).font(style::FONT).color(color(p.danger.ink))].spacing(6).align_y(Alignment::Center));
                }
                let foot = vec![cancel(), self.btn("Authenticate", Some("key"), Some("Enter"), Variant::Primary, Message::Ui(UiMsg::PasswordSubmit))];
                self.dialog_frame("lock", "Password needed".into(), body.into(), foot, false, *opened)
            }
            Dialog::Properties { .. } => self.properties_dialog()?,
            Dialog::Login { ask, user, domain, password, guest, remember, opened } => self.login_dialog(ask, user, domain, password, *guest, *remember, *opened),
            Dialog::Question { ask, opened } => self.question_dialog(ask, *opened),
            Dialog::WriteSystem { drive, opened } => {
                let name = self.volumes.get(*drive).map(|v| crate::drives::display_name(v, &self.volumes)).unwrap_or_default();
                let foot = vec![cancel(), self.btn("Allow writing", Some("unlock"), None, Variant::Danger, Message::Ui(UiMsg::Pick(Box::new(Message::Drive(DriveMsg::AllowWrite(*drive))))))];
                self.dialog_frame(
                    "shield",
                    format!("Allow writing to {name}?"),
                    self.body_text("This is the drive Windows runs from. Changing files here while Windows is hibernated or using Fast Startup can corrupt it — only continue if Windows was shut down with Restart. EchoFiles goes back to read-only the next time it mounts the drive.".into()),
                    foot,
                    true,
                    *opened,
                )
            }
        })
    }

    pub(crate) fn toast_layer(&self) -> Option<Element<'_, Message>> {
        // Transfers show once they're really moving bytes (not while a dialog asks about them)
        // and only if they take a moment — quick ones just end in a toast.
        let transfers: Vec<_> = self.transfers.iter().filter(|t| t.plan.is_some() && t.progress.phase() != Phase::Planning && (t.started.elapsed() > Duration::from_millis(400) || t.finished.is_some())).collect();
        if self.toasts.is_empty() && transfers.is_empty() {
            return None;
        }
        let p = &self.palette;
        let mut col = column![].spacing(style::SPACE_3).align_x(Alignment::End);
        for t in transfers {
            col = col.push(self.transfer_toast(t));
        }
        let hidden = self.toasts.len().saturating_sub(3);
        if hidden > 0 {
            col = col.push(text(format!("+{hidden} more")).size(style::META).font(style::FONT).color(color(p.ink_muted)));
        }
        for t in self.toasts.iter().skip(hidden) {
            let edge = match t.tone {
                Tone::Accent => color(p.accent),
                Tone::Danger => color(p.danger.base),
            };
            let (icon, tint) = match t.tone {
                Tone::Accent => ("info", color(p.accent_ink)),
                Tone::Danger => ("error", color(p.danger.ink)),
            };
            let mut head = row![w::glyph(&self.icons, icon, 16.0, tint), text(t.title.clone()).size(style::BODY).font(style::FONT_BOLD).color(color(p.ink_strong)).width(Length::Fill)].spacing(style::SPACE_3).align_y(Alignment::Center);
            if let Some((label, msg)) = &t.action {
                let undo = label == "Undo";
                head = head.push(w::text_button(p, &self.icons, label, undo.then_some("undo"), undo.then_some("Ctrl+Z"), Variant::Secondary, Some(Message::Ui(UiMsg::Pick(Box::new(msg.clone()))))));
            }
            head = head.push(w::icon_button(p, &self.icons, "close", Some(Message::Ui(UiMsg::ToastDismiss(t.id))), false));
            let mut c = column![head].spacing(4);
            if let Some(b) = &t.body {
                c = c.push(container(text(b.clone()).size(style::META).font(style::FONT).color(color(p.ink_muted))).padding(Padding { left: 24.0, ..Padding::ZERO }));
            }
            let ti = self.t(t.at, 240);
            let pal = p.clone();
            let card = container(c).width(380).padding([8, 10]).style(move |_| container::Style {
                background: Some(Background::Color(color(pal.bg_raised))),
                border: Border { color: edge, width: 2.0, radius: 0.0.into() },
                shadow: Shadow { color: Color { a: 0.3, ..Color::BLACK }, offset: Vector::new(0.0, 6.0), blur_radius: 16.0 },
                ..Default::default()
            });
            let id = t.id;
            col = col.push(container(mouse_area(card).on_enter(Message::Ui(UiMsg::ToastHover(id, true))).on_exit(Message::Ui(UiMsg::ToastHover(id, false)))).padding(Padding { top: 12.0 * (1.0 - ti), ..Padding::ZERO }));
        }
        Some(container(col).width(Length::Fill).height(Length::Fill).align_x(Alignment::End).align_y(Alignment::End).padding(Padding { right: 16.0, bottom: 36.0, left: 16.0, top: 16.0 }).into())
    }

    fn transfer_toast<'a>(&'a self, t: &'a crate::actions::Transfer) -> Element<'a, Message> {
        let p = &self.palette;
        let pr = &t.progress;
        let total = pr.bytes_total.load(Ordering::Relaxed);
        let done = pr.bytes_done.load(Ordering::Relaxed);
        let files_total = pr.files_total.load(Ordering::Relaxed);
        let files_done = pr.files_done.load(Ordering::Relaxed);
        let finished = t.outcome.is_some();
        let ok = t.outcome.as_ref().is_some_and(|o| o.errors.is_empty() && !o.cancelled);
        let frac = if finished { 1.0 } else if total > 0 { done as f32 / total as f32 } else if files_total > 0 { files_done as f32 / files_total as f32 } else { 0.0 };
        let paused = pr.paused.load(Ordering::Relaxed);
        let phase = pr.phase();
        let mut meta = String::new();
        if finished {
            match &t.outcome {
                Some(o) if o.cancelled => meta.push_str("Cancelled"),
                Some(o) if !o.errors.is_empty() => meta.push_str(&format!("{} failed", o.errors.len())),
                Some(o) if o.synced => meta.push_str("Flushed to disk — safe to unplug or reboot"),
                _ => meta.push_str("Done"),
            }
        } else if phase == Phase::Planning {
            meta.push_str("Checking what's there…");
        } else if phase == Phase::Syncing {
            meta.push_str("Flushing to disk…");
        } else {
            fmt::size(done, &mut meta);
            meta.push_str(" of ");
            fmt::size(total, &mut meta);
            if t.rate > 0.0 && !paused {
                meta.push_str(" · ");
                fmt::size(t.rate as u64, &mut meta);
                meta.push_str("/s");
                let left = (total.saturating_sub(done)) as f64 / t.rate;
                if left > 1.0 {
                    meta.push_str(&format!(" · about {} left", if left < 60.0 { format!("{} s", left as u64) } else { format!("{} min", (left / 60.0).ceil() as u64) }));
                }
            }
            if paused {
                meta.push_str(" · paused");
            }
        }
        let current = if finished { String::new() } else { pr.current().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() };
        let tint = if finished && ok { color(p.success.ink) } else { color(p.accent_ink) };
        let mut head = row![
            w::glyph(&self.icons, if finished && ok { "check" } else if t.kind == Kind::Copy { "copy" } else { "move" }, 16.0, tint),
            text(t.title()).size(style::BODY).font(style::FONT_BOLD).color(color(p.ink_strong)).width(Length::Fill).wrapping(text::Wrapping::None),
        ]
        .spacing(style::SPACE_3)
        .align_y(Alignment::Center);
        if finished {
            head = head.push(w::icon_button(p, &self.icons, "close", Some(Message::File(FileMsg::Dismiss(t.id))), false));
        } else {
            head = head.push(w::icon_button(p, &self.icons, if paused { "play" } else { "pause" }, Some(Message::File(FileMsg::Pause(t.id))), paused));
            head = head.push(w::icon_button(p, &self.icons, "close", Some(Message::File(FileMsg::Cancel(t.id))), false));
        }
        let fill = if finished && ok { p.success.base } else if finished { p.danger.base } else { p.accent };
        let bar = w::progress(p, frac, fill, phase == Phase::Planning && !finished);
        let mut c = column![head].spacing(6);
        if !current.is_empty() {
            c = c.push(text(current).size(style::META).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None));
        }
        c = c.push(bar);
        c = c.push(text(meta).size(style::META).font(style::FONT).color(color(p.ink_muted)));
        let edge = if finished && ok { color(p.success.base) } else { color(p.accent) };
        let pal = p.clone();
        container(c)
            .width(380)
            .padding([8, 10])
            .clip(true)
            .style(move |_| container::Style {
                background: Some(Background::Color(color(pal.bg_raised))),
                border: Border { color: edge, width: 2.0, radius: 0.0.into() },
                shadow: Shadow { color: Color { a: 0.3, ..Color::BLACK }, offset: Vector::new(0.0, 6.0), blur_radius: 16.0 },
                ..Default::default()
            })
            .into()
    }

    pub(crate) fn command_layer(&self) -> Option<Element<'_, Message>> {
        let c = self.command.as_ref()?;
        let p = &self.palette;
        let results = self.command_results(&c.query);
        let field = row![
            text(">").size(16).font(style::FONT_BOLD).color(color(p.accent_ink)),
            text_input("Type a command, a folder, or a path (/ or ~)", &c.query)
                .id(COMMAND_ID)
                .on_input(|q| Message::Ui(UiMsg::CommandQuery(q)))
                .on_submit(Message::Ui(UiMsg::CommandRun(None)))
                .size(15)
                .font(style::FONT)
                .padding([6, 4])
                .style({
                    let pal = p.clone();
                    move |_, _| text_input::Style {
                        background: Background::Color(Color::TRANSPARENT),
                        border: Border::default(),
                        icon: color(pal.ink_muted),
                        placeholder: color(pal.ink_muted),
                        value: color(pal.ink_strong),
                        selection: color(pal.accent_soft),
                    }
                }),
        ]
        .spacing(style::SPACE_3)
        .align_y(Alignment::Center);
        let mut list = column![].spacing(1);
        let mut group = "";
        for (i, f) in results.iter().enumerate() {
            if f.group != group {
                group = f.group;
                list = list.push(container(text(group.to_uppercase()).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted))).padding([6, 10]));
            }
            let active = i == c.cursor;
            let chars: Vec<char> = f.label.chars().collect();
            let mut spans: Vec<iced::widget::text::Span<'_, (), iced::Font>> = Vec::new();
            let mut buf = String::new();
            let mut hl = false;
            for (ci, ch) in chars.iter().enumerate() {
                let m = f.matched.contains(&ci);
                if m != hl && !buf.is_empty() {
                    spans.push(span(std::mem::take(&mut buf)).color(if hl { color(p.accent_ink) } else { color(p.ink) }).font(if hl { style::FONT_BOLD } else { style::FONT }));
                }
                hl = m;
                buf.push(*ch);
            }
            if !buf.is_empty() {
                spans.push(span(buf).color(if hl { color(p.accent_ink) } else { color(p.ink) }).font(if hl { style::FONT_BOLD } else { style::FONT }));
            }
            let label: Element<'_, Message> = iced::widget::rich_text(spans).size(style::BODY).width(Length::Fill).into();
            let mut r = row![w::glyph(&self.icons, f.icon, 16.0, color(if active { p.accent_ink } else { p.ink_muted })), label].spacing(style::SPACE_3 + 2.0).align_y(Alignment::Center);
            if let Some(h) = &f.hint {
                r = r.push(text(h.clone()).size(style::META).font(style::FONT).color(color(p.ink_muted)));
            }
            if let Some(k) = f.kbd {
                r = r.push(w::kbd_chip(p, k));
            }
            let pal = p.clone();
            list = list.push(
                button(container(r).height(30).align_y(Alignment::Center))
                    .padding([0, 10])
                    .width(Length::Fill)
                    .on_press(Message::Ui(UiMsg::CommandRun(Some(i))))
                    .style(move |_, status| {
                        let hot = active || matches!(status, button::Status::Hovered);
                        button::Style {
                            background: hot.then(|| Background::Color(color(pal.state_hover))),
                            border: Border { color: if active { color(pal.line) } else { Color::TRANSPARENT }, width: 1.0, radius: 2.0.into() },
                            text_color: color(pal.ink),
                            ..Default::default()
                        }
                    }),
            );
        }
        if results.is_empty() {
            list = list.push(container(text("Nothing matches. Try a folder name, a path starting with / or ~, or an action like “new folder”.").size(style::META).font(style::FONT).color(color(p.ink_muted))).padding(12));
        }
        let t = self.t(c.opened, 180);
        let pal = p.clone();
        let panel = container(column![container(field).padding([4, 10]), w::hline(p.line), scrollable(list).height(Length::Shrink)].spacing(4))
            .width(620)
            .max_height(460)
            .padding(6)
            .style(move |_| container::Style {
                background: Some(Background::Color(color(pal.bg_raised))),
                border: Border { color: color(pal.accent), width: 2.0, radius: 0.0.into() },
                shadow: Shadow { color: Color { a: 0.45, ..Color::BLACK }, offset: Vector::new(0.0, 12.0), blur_radius: 32.0 },
                ..Default::default()
            });
        let back = mouse_area(container(Space::new()).width(Length::Fill).height(Length::Fill)).on_press(Message::Escape);
        Some(stack![back, container(iced::widget::opaque(panel)).width(Length::Fill).align_x(Alignment::Center).padding(Padding { top: 72.0 + 8.0 * (1.0 - t), ..Padding::ZERO })].into())
    }
}

//! Network places in EchoFiles (design system `NetworkItem`, `ConnectDialog`,
//! `SignInDialog`, `SharesPage`): Windows shares, SSH and FTP servers through GVfs.
//!
//! A connection becomes a folder under `$XDG_RUNTIME_DIR/gvfs`, so once it's open every
//! other part of EchoFiles treats it like any folder. This module owns what's special:
//! saved servers in the sidebar, the Connect to Server dialog, sign-in and host-key
//! questions, the share picker for a bare SMB server, and friendly names instead of GVfs'
//! mount folder names.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use ef_config::{self as config, Server};
use ef_net::gvfs;
use ef_net::{Address, Ask, Login, LoginAsk, Mount, Nearby, Protocol, QuestionAsk, Remember};
use iced::futures::channel::mpsc;
use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input, Space};
use iced::{Alignment, Background, Border, Element, Length, Point, Task};

use crate::app::{background, App, Message};
use crate::overlay::{Dialog, UiMsg};
use crate::style::{self, color};
use crate::widgets::{self as w, Variant};

pub const CONNECT_ID: &str = "connect-address";
pub const LOGIN_USER_ID: &str = "login-user";
pub const LOGIN_DOMAIN_ID: &str = "login-domain";
pub const LOGIN_PASSWORD_ID: &str = "login-password";
const RECENT_MAX: usize = 8;

// ------------------------------------------------------------------------------ state

/// The Connect to Server dialog.
pub struct ConnectForm {
    pub protocol: Protocol,
    pub text: String,
    /// Add the server to the sidebar once connected.
    pub save: bool,
    /// Connecting: the operation's id.
    pub busy: Option<u64>,
    pub error: Option<String>,
    pub opened: Instant,
}

/// The shares of an SMB server, shown in a pane (a server address without a share).
#[derive(Debug, Clone)]
pub struct SharesPage {
    pub address: Address,
    pub shares: Option<Result<Vec<String>, String>>,
}

#[derive(Default)]
pub struct NetState {
    pub mounts: Vec<Mount>,
    /// What GVfs can speak here; empty until known.
    pub protocols: Vec<Protocol>,
    /// GVfs is missing or not running.
    pub unavailable: Option<String>,
    /// Connections in progress: (operation, address).
    pub connecting: Vec<(u64, Address)>,
    /// Connections whose dialog was cancelled while they were still going.
    pub abandoned: HashSet<u64>,
    pub form: Option<ConnectForm>,
    pub nearby: Option<Result<Vec<Nearby>, String>>,
    pub scanning: bool,
    /// Recently used addresses, newest first.
    pub recent: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginField {
    User,
    Domain,
    Password,
}

#[derive(Debug, Clone)]
pub enum NetMsg {
    Mounts(Result<Vec<Mount>, String>),
    Protocols(Result<Vec<Protocol>, String>),
    /// Some app connected or disconnected a place.
    Changed,
    Ask(Ask),
    // Connect to Server
    Open(Option<String>),
    Close,
    SetProtocol(Protocol),
    Draft(String),
    SaveToggle(bool),
    Submit,
    Pick(Address),
    Scan,
    Scanned(Result<Vec<Nearby>, String>),
    // connections
    Connect { address: Address, save: bool },
    Connected { op: u64, address: Address, save: bool, result: Result<Mount, ef_net::Error> },
    Shares { op: u64, address: Address, result: Result<Vec<String>, ef_net::Error> },
    // sidebar
    Click(String),
    Menu(String, Point),
    Disconnect(String),
    Disconnected { name: String, result: Result<(), ef_net::Error> },
    Save(String),
    Forget(String),
    CopyAddress(String),
    // sign-in and questions
    LoginDraft(LoginField, String),
    LoginGuest(bool),
    LoginRemember(bool),
    LoginSubmit,
    Answer(usize),
}

enum Event {
    Ask(Ask),
    Changed,
}

static EVENTS: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<Event>>>> = OnceLock::new();
static EVENT_TX: OnceLock<mpsc::UnboundedSender<Event>> = OnceLock::new();

fn event_tx() -> mpsc::UnboundedSender<Event> {
    EVENT_TX
        .get_or_init(|| {
            let (tx, rx) = mpsc::unbounded();
            let _ = EVENTS.set(Mutex::new(Some(rx)));
            let changed = tx.clone();
            gvfs::watch(move || {
                let _ = changed.unbounded_send(Event::Changed);
            });
            tx
        })
        .clone()
}

/// Questions from GVfs and connect/disconnect news, as a subscription stream (taken once).
pub fn events() -> impl iced::futures::Stream<Item = NetMsg> {
    let _ = event_tx();
    let rx = EVENTS.get().and_then(|m| m.lock().unwrap().take());
    iced::futures::stream::unfold(rx, |rx| async move {
        use iced::futures::StreamExt;
        let mut rx = rx?;
        let item = rx.next().await?;
        let msg = match item {
            Event::Ask(a) => NetMsg::Ask(a),
            Event::Changed => NetMsg::Changed,
        };
        Some((msg, Some(rx)))
    })
}

fn asker() -> impl Fn(Ask) + Send + Sync + 'static {
    let tx = event_tx();
    move |a| {
        let _ = tx.unbounded_send(Event::Ask(a));
    }
}

fn recent_file() -> PathBuf {
    config::state_dir().join("recent-servers")
}

pub fn load_recent() -> Vec<String> {
    std::fs::read_to_string(recent_file()).map(|t| t.lines().filter(|l| !l.trim().is_empty()).map(str::to_string).take(RECENT_MAX).collect()).unwrap_or_default()
}

/// Work to start with the app: what GVfs can do and what's already connected.
pub fn boot_tasks() -> Task<Message> {
    Task::batch([
        background(|| gvfs::protocols().map_err(|e| e.to_string()), |r| Message::Net(NetMsg::Protocols(r))),
        background(|| gvfs::mounts().map_err(|e| e.to_string()), |r| Message::Net(NetMsg::Mounts(r))),
    ])
}

pub fn protocol_icon(p: Protocol) -> &'static str {
    match p {
        Protocol::Smb => "network",
        Protocol::Sftp | Protocol::Ftp | Protocol::Ftps => "server",
    }
}

/// "Windows share “Media” on nas.local · port 4445 · as alice"
fn describe(a: &Address) -> String {
    let mut s = match (&a.share, a.protocol) {
        (Some(share), _) => format!("Windows share “{share}” on {}", a.host),
        (None, Protocol::Smb) => format!("The shares on {}", a.host),
        (None, p) => format!("{} {}", p.describe(), a.host),
    };
    if let Some(port) = a.port.filter(|&p| p != a.protocol.default_port()) {
        s.push_str(&format!(" · port {port}"));
    }
    if !a.path.is_empty() {
        s.push_str(&format!(" · /{}", a.path));
    }
    if let Some(u) = &a.user {
        s.push_str(" · as ");
        if let Some(d) = &a.domain {
            s.push_str(&format!("{d}\\"));
        }
        s.push_str(u);
    }
    s
}

/// One row of the sidebar's Network section.
pub struct Place<'a> {
    /// Saved address, or the connection's address or folder for unsaved ones.
    pub key: String,
    pub name: String,
    pub address: Option<Address>,
    pub mount: Option<&'a Mount>,
    pub saved: bool,
}

fn same_server(a: &Address, b: &Address) -> bool {
    a.protocol == b.protocol
        && a.host.eq_ignore_ascii_case(&b.host)
        && a.port.unwrap_or(a.protocol.default_port()) == b.port.unwrap_or(b.protocol.default_port())
        && a.user == b.user
        && a.share.as_deref().map(str::to_lowercase) == b.share.as_deref().map(str::to_lowercase)
}

impl App {
    // ------------------------------------------------------------------ lookups

    /// Saved servers first (in their order), then connections made elsewhere.
    pub(crate) fn net_places(&self) -> Vec<Place<'_>> {
        let mut out: Vec<Place<'_>> = Vec::new();
        let mut used: Vec<usize> = Vec::new();
        for s in &self.settings.network.servers {
            let address = Address::parse(&s.uri, Protocol::Smb).ok();
            let mi = address.as_ref().and_then(|a| self.net.mounts.iter().position(|m| m.serves(a)));
            if let Some(i) = mi {
                used.push(i);
            }
            let name = s.name.clone().or_else(|| address.as_ref().map(Address::short_name)).unwrap_or_else(|| s.uri.clone());
            out.push(Place { key: s.uri.clone(), name, address, mount: mi.map(|i| &self.net.mounts[i]), saved: true });
        }
        for (i, m) in self.net.mounts.iter().enumerate() {
            if used.contains(&i) {
                continue;
            }
            let key = m.address.as_ref().map(Address::uri).unwrap_or_else(|| m.root.to_string_lossy().into_owned());
            let name = m.address.as_ref().map(Address::short_name).unwrap_or_else(|| m.name.clone());
            out.push(Place { key, name, address: m.address.clone(), mount: Some(m), saved: false });
        }
        out
    }

    /// The connection holding `path`, with the name people know it by.
    pub(crate) fn net_place_at(&self, path: &Path) -> Option<(&Mount, String)> {
        let m = self.net.mounts.iter().filter(|m| path.starts_with(&m.root)).max_by_key(|m| m.root.as_os_str().len())?;
        // The saved server's name (or its address as typed, keeping "Media" rather than
        // GVfs' "media"); then the connection's own address.
        let saved = self.settings.network.servers.iter().find_map(|s| {
            let b = Address::parse(&s.uri, Protocol::Smb).ok()?;
            m.serves(&b).then(|| s.name.clone().unwrap_or_else(|| b.display_name()))
        });
        let typed = || self.net.recent.iter().filter_map(|u| Address::parse(u, Protocol::Smb).ok()).find(|b| m.serves(b)).map(|b| b.display_name());
        let name = saved.or_else(typed).or_else(|| m.address.as_ref().map(Address::display_name)).unwrap_or_else(|| m.name.clone());
        Some((m, name))
    }

    /// `smb://nas/Media/Photos` for a folder inside a connection (the path bar's text).
    pub(crate) fn net_uri_for(&self, path: &Path) -> Option<String> {
        let (m, _) = self.net_place_at(path)?;
        let mut a = m.address.clone()?;
        let rest = path.strip_prefix(&m.root).ok()?;
        a.path = rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
        Some(a.uri())
    }

    fn place_by_key(&self, key: &str) -> Option<(Option<Address>, Option<Mount>, String)> {
        self.net_places().into_iter().find(|p| p.key == key).map(|p| (p.address, p.mount.cloned(), p.name))
    }

    fn is_connecting(&self, a: &Address) -> bool {
        self.net.connecting.iter().any(|(_, c)| same_server(c, a))
    }

    // ------------------------------------------------------------------ actions

    fn refresh_mounts(&self) -> Task<Message> {
        background(|| gvfs::mounts().map_err(|e| e.to_string()), |r| Message::Net(NetMsg::Mounts(r)))
    }

    fn remember_recent(&mut self, a: &Address) {
        // One entry per server (and share); the folder inside doesn't make a new one.
        let uri = a.server().uri();
        self.net.recent.retain(|u| u != &uri);
        self.net.recent.insert(0, uri);
        self.net.recent.truncate(RECENT_MAX);
        let text = self.net.recent.join("\n");
        std::thread::spawn(move || {
            let f = recent_file();
            if let Some(d) = f.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(f, text);
        });
    }

    fn save_server(&mut self, a: &Address) -> Task<Message> {
        let server = a.server();
        let known = self.settings.network.servers.iter().any(|s| Address::parse(&s.uri, Protocol::Smb).is_ok_and(|b| same_server(&b, &server)));
        if known {
            return Task::none();
        }
        self.settings.network.servers.push(Server { uri: server.uri(), name: None });
        self.persist_settings()
    }

    /// Some pane is showing a folder inside `root`.
    fn all_panes_at(&self, root: &Path) -> bool {
        self.tabs.iter().flat_map(|t| t.panes.iter()).any(|p| !p.special() && p.location.starts_with(root))
    }

    /// Panes inside `root` go home, so nothing holds a closing connection open.
    fn leave(&mut self, root: &Path) -> Task<Message> {
        let home = config::home();
        let show_hidden = self.show_hidden;
        let mut tasks = Vec::new();
        for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()) {
            if pane.location.starts_with(root) && !pane.special() {
                pane.location = home.clone();
                pane.back.retain(|p| !p.starts_with(root));
                pane.forward.retain(|p| !p.starts_with(root));
                tasks.push(pane.load(home.clone(), show_hidden));
            }
        }
        Task::batch(tasks)
    }

    /// Connect to `address` (or show it, if it's already connected) and open it.
    pub(crate) fn connect(&mut self, address: Address, save: bool) -> Task<Message> {
        if address.protocol == Protocol::Smb && address.share.is_none() {
            self.net.form = None;
            let save_task = if save { self.save_server(&address) } else { Task::none() };
            return Task::batch([save_task, self.open_shares(address)]);
        }
        if let Some(m) = self.net.mounts.iter().find(|m| m.serves(&address)).cloned() {
            self.net.form = None;
            self.remember_recent(&address);
            let save_task = if save { self.save_server(&address) } else { Task::none() };
            return Task::batch([save_task, self.go(m.path_for(&address), true)]);
        }
        if self.is_connecting(&address) {
            return Task::none();
        }
        let op = gvfs::next_op();
        self.net.connecting.push((op, address.clone()));
        if let Some(f) = &mut self.net.form {
            f.busy = Some(op);
            f.error = None;
        }
        let a = address.clone();
        let ask = asker();
        background(move || gvfs::mount(&a, op, ask), move |result| Message::Net(NetMsg::Connected { op, address: address.clone(), save, result }))
    }

    /// Show an SMB server's shares in the active pane.
    fn open_shares(&mut self, address: Address) -> Task<Message> {
        let server = Address { share: None, path: String::new(), ..address };
        self.mode = crate::app::Mode::Files;
        let pane = self.pane_mut();
        pane.drives = false;
        pane.shares = Some(SharesPage { address: server.clone(), shares: None });
        if self.is_connecting(&server) {
            return Task::none();
        }
        let op = gvfs::next_op();
        self.net.connecting.push((op, server.clone()));
        let a = server.clone();
        let ask = asker();
        background(move || gvfs::shares(&a, op, ask), move |result| Message::Net(NetMsg::Shares { op, address: server.clone(), result }))
    }

    fn close_questions_for(&mut self, op: u64) {
        let ours = match &self.dialog {
            Some(Dialog::Login { ask, .. }) => ask.op == op,
            Some(Dialog::Question { ask, .. }) => ask.op == op,
            _ => false,
        };
        if ours {
            self.dialog = None;
        }
    }

    pub(crate) fn net_update(&mut self, msg: NetMsg) -> Task<Message> {
        match msg {
            NetMsg::Mounts(Ok(mounts)) => {
                // Places that went away (disconnected elsewhere, server gone): leave them.
                let gone: Vec<(PathBuf, String)> = self
                    .net
                    .mounts
                    .iter()
                    .filter(|o| !mounts.iter().any(|m| m.root == o.root))
                    .map(|o| (o.root.clone(), self.net_place_at(&o.root).map(|(_, n)| n).unwrap_or_else(|| o.name.clone())))
                    .collect();
                self.net.mounts = mounts;
                self.net.unavailable = None;
                let mut tasks = Vec::new();
                for (root, name) in gone {
                    // Still open somewhere: the server went away (a Disconnect leaves first).
                    if self.all_panes_at(&root) {
                        self.push_toast(crate::overlay::Tone::Accent, format!("Lost the connection to {name}"), Some("The server stopped answering, so its folders were closed. Reconnect it from the Network section.".into()), None);
                    }
                    tasks.push(self.leave(&root));
                }
                Task::batch(tasks)
            }
            NetMsg::Mounts(Err(e)) => {
                self.net.unavailable = Some(e);
                Task::none()
            }
            NetMsg::Protocols(Ok(p)) => {
                self.net.protocols = p;
                Task::none()
            }
            NetMsg::Protocols(Err(e)) => {
                self.net.unavailable = Some(e);
                Task::none()
            }
            NetMsg::Changed => self.refresh_mounts(),
            NetMsg::Ask(Ask::Login(ask)) => {
                self.menu = None;
                let user = if ask.default_user.is_empty() {
                    self.net.connecting.iter().find(|(o, _)| *o == ask.op).and_then(|(_, a)| a.user.clone()).unwrap_or_default()
                } else {
                    ask.default_user.clone()
                };
                let focus = if ask.need_user && user.is_empty() { LOGIN_USER_ID } else { LOGIN_PASSWORD_ID };
                let domain = ask.default_domain.clone();
                self.dialog = Some(Dialog::Login { ask, user, domain, password: String::new(), guest: false, remember: false, opened: Instant::now() });
                iced::widget::operation::focus(focus)
            }
            NetMsg::Ask(Ask::Question(ask)) => {
                self.menu = None;
                self.dialog = Some(Dialog::Question { ask, opened: Instant::now() });
                Task::none()
            }
            NetMsg::Ask(Ask::Withdrawn(op)) => {
                self.close_questions_for(op);
                Task::none()
            }
            NetMsg::Open(prefill) => {
                self.menu = None;
                self.command = None;
                let protocol = self.net.form.as_ref().map_or(Protocol::Smb, |f| f.protocol);
                let mut form = ConnectForm { protocol, text: String::new(), save: true, busy: None, error: None, opened: Instant::now() };
                if let Some(t) = prefill {
                    if let Ok(a) = Address::parse(&t, protocol) {
                        form.protocol = a.protocol;
                    }
                    form.text = t;
                }
                self.net.form = Some(form);
                let scan = if self.net.scanning || self.net.nearby.as_ref().is_some_and(|n| n.as_ref().is_ok_and(|v| !v.is_empty())) { Task::none() } else { self.net_update(NetMsg::Scan) };
                Task::batch([scan, iced::widget::operation::focus(CONNECT_ID), iced::widget::operation::move_cursor_to_end(CONNECT_ID)])
            }
            NetMsg::Close => {
                if let Some(op) = self.net.form.take().and_then(|f| f.busy) {
                    self.net.abandoned.insert(op);
                }
                Task::none()
            }
            NetMsg::SetProtocol(p) => {
                if let Some(f) = &mut self.net.form {
                    // A typed scheme follows the choice.
                    if let Some((scheme, rest)) = f.text.split_once("://")
                        && Protocol::from_scheme(scheme).is_some()
                    {
                        f.text = format!("{}://{rest}", p.scheme());
                    }
                    f.protocol = p;
                    f.error = None;
                }
                iced::widget::operation::focus(CONNECT_ID)
            }
            NetMsg::Draft(t) => {
                if let Some(f) = &mut self.net.form {
                    if let Some((scheme, _)) = t.split_once("://") {
                        if let Some(p) = Protocol::from_scheme(scheme) {
                            f.protocol = p;
                        }
                    } else if t.starts_with("\\\\") {
                        f.protocol = Protocol::Smb;
                    }
                    f.text = t;
                    f.error = None;
                }
                Task::none()
            }
            NetMsg::SaveToggle(on) => {
                if let Some(f) = &mut self.net.form {
                    f.save = on;
                }
                Task::none()
            }
            NetMsg::Submit => {
                let Some(f) = &self.net.form else { return Task::none() };
                if f.busy.is_some() {
                    return Task::none();
                }
                match Address::parse(&f.text, f.protocol) {
                    Ok(a) => {
                        let save = f.save;
                        self.connect(a, save)
                    }
                    // The line under the field already says what's wrong.
                    Err(_) => iced::widget::operation::focus(CONNECT_ID),
                }
            }
            NetMsg::Pick(a) => {
                let save = self.net.form.as_ref().is_some_and(|f| f.save);
                if let Some(f) = &mut self.net.form {
                    f.text = a.uri();
                    f.protocol = a.protocol;
                }
                self.connect(a, save)
            }
            NetMsg::Scan => {
                self.net.scanning = true;
                background(|| ef_net::discover::scan(Duration::from_secs(3)), |r| Message::Net(NetMsg::Scanned(r)))
            }
            NetMsg::Scanned(r) => {
                self.net.scanning = false;
                self.net.nearby = Some(r);
                Task::none()
            }
            NetMsg::Connect { address, save } => self.connect(address, save),
            NetMsg::Connected { op, address, save, result } => {
                self.net.connecting.retain(|(o, _)| *o != op);
                self.close_questions_for(op);
                let from_form = self.net.form.as_ref().is_some_and(|f| f.busy == Some(op));
                if self.net.abandoned.remove(&op) {
                    return self.refresh_mounts();
                }
                match result {
                    Ok(m) => {
                        if from_form {
                            self.net.form = None;
                        }
                        self.remember_recent(&address);
                        if !self.net.mounts.iter().any(|x| x.root == m.root) {
                            self.net.mounts.push(m.clone());
                        }
                        let save_task = if save { self.save_server(&address) } else { Task::none() };
                        Task::batch([save_task, self.go(m.path_for(&address), true), self.refresh_mounts()])
                    }
                    Err(ef_net::Error::Cancelled) => {
                        if let Some(f) = self.net.form.as_mut().filter(|_| from_form) {
                            f.busy = None;
                        }
                        Task::none()
                    }
                    Err(e) => {
                        let why = gvfs::explain(&e, &address);
                        match self.net.form.as_mut().filter(|_| from_form) {
                            Some(f) => {
                                f.busy = None;
                                f.error = Some(why);
                            }
                            None => self.toast_error(format!("Couldn't connect to {}", address.display_name()), why),
                        }
                        Task::none()
                    }
                }
            }
            NetMsg::Shares { op, address, result } => {
                self.net.connecting.retain(|(o, _)| *o != op);
                self.close_questions_for(op);
                let shares = match result {
                    Ok(s) => {
                        self.remember_recent(&address);
                        Ok(s)
                    }
                    Err(e) => Err(gvfs::explain(&e, &address)),
                };
                for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()) {
                    if let Some(page) = pane.shares.as_mut().filter(|pg| same_server(&pg.address, &address)) {
                        page.shares = Some(shares.clone());
                    }
                }
                Task::none()
            }
            NetMsg::Click(key) => {
                let Some((address, mount, _)) = self.place_by_key(&key) else { return Task::none() };
                match (mount, address) {
                    (Some(m), Some(a)) => self.go(m.path_for(&Address { path: String::new(), ..a }), true),
                    (Some(m), None) => self.go(m.landing.clone(), true),
                    (None, Some(a)) => self.connect(a, false),
                    (None, None) => Task::none(),
                }
            }
            NetMsg::Menu(key, at) => {
                self.open_menu(crate::overlay::MenuFor::Network(key), at);
                Task::none()
            }
            NetMsg::Disconnect(key) => {
                let Some((_, Some(m), name)) = self.place_by_key(&key) else { return Task::none() };
                let leave = self.leave(&m.root);
                let unmount = background(move || gvfs::unmount(&m, false), move |result| Message::Net(NetMsg::Disconnected { name: name.clone(), result }));
                Task::batch([leave, unmount])
            }
            NetMsg::Disconnected { name, result } => {
                if let Err(e) = result {
                    let why = match e {
                        ef_net::Error::Cancelled => "Files there are still open. Close them, then try again.".to_string(),
                        other => other.to_string(),
                    };
                    self.toast_error(format!("Couldn't disconnect {name}"), why);
                }
                self.refresh_mounts()
            }
            NetMsg::Save(key) => match Address::parse(&key, Protocol::Smb) {
                Ok(a) => self.save_server(&a),
                Err(_) => Task::none(),
            },
            NetMsg::Forget(key) => {
                self.settings.network.servers.retain(|s| s.uri != key);
                self.persist_settings()
            }
            NetMsg::CopyAddress(uri) => iced::clipboard::write(uri),
            NetMsg::LoginDraft(field, v) => {
                if let Some(Dialog::Login { user, domain, password, .. }) = &mut self.dialog {
                    match field {
                        LoginField::User => *user = v,
                        LoginField::Domain => *domain = v,
                        LoginField::Password => *password = v,
                    }
                }
                Task::none()
            }
            NetMsg::LoginGuest(on) => {
                if let Some(Dialog::Login { guest, .. }) = &mut self.dialog {
                    *guest = on;
                }
                if on { Task::none() } else { iced::widget::operation::focus(LOGIN_PASSWORD_ID) }
            }
            NetMsg::LoginRemember(on) => {
                if let Some(Dialog::Login { remember, .. }) = &mut self.dialog {
                    *remember = on;
                }
                Task::none()
            }
            NetMsg::LoginSubmit => {
                if let Some(Dialog::Login { ask, user, domain, password, guest, remember, .. }) = &self.dialog {
                    if !*guest && ask.need_user && user.trim().is_empty() {
                        return iced::widget::operation::focus(LOGIN_USER_ID);
                    }
                    let login = Login {
                        user: user.trim().to_string(),
                        domain: domain.trim().to_string(),
                        password: password.clone(),
                        anonymous: *guest,
                        remember: if *remember { Remember::Forever } else { Remember::Session },
                    };
                    ask.answer(Some(login));
                    self.dialog = None;
                }
                Task::none()
            }
            NetMsg::Answer(i) => {
                if let Some(Dialog::Question { ask, .. }) = self.dialog.take() {
                    ask.answer(Some(i));
                }
                Task::none()
            }
        }
    }

    // ------------------------------------------------------------------ sidebar

    /// Section head with the Connect button at its end.
    fn net_section(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mark = color(p.world_network);
        let connected = self.net.mounts.len();
        let mut r = row![
            container(Space::new()).width(6).height(6).style(move |_| container::Style { background: Some(Background::Color(mark)), ..Default::default() }),
            text("NETWORK").size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted)).width(Length::Fill),
        ]
        .spacing(style::SPACE_3)
        .align_y(Alignment::Center);
        if connected > 0 {
            r = r.push(text(connected.to_string()).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)));
        }
        let add = self.tip(w::icon_button(p, &self.icons, "plus", Some(Message::Net(NetMsg::Open(None))), false), "Connect to server", Some("Ctrl+Shift+S"));
        r = r.push(add);
        container(r).padding(iced::Padding { left: 12.0, right: 4.0, top: 0.0, bottom: 0.0 }).height(24).align_y(Alignment::Center).into()
    }

    /// The Network section: saved servers and live connections (design system `NetworkItem`).
    pub(crate) fn network_section(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mut col = column![self.net_section()].spacing(1);
        let places = self.net_places();
        if places.is_empty() {
            let hint = text("Windows shares, SSH and FTP servers you connect to appear here.").size(style::META).font(style::FONT).color(color(p.ink_muted));
            col = col.push(container(hint).padding([2, 12]));
        }
        for place in places {
            col = col.push(self.net_item(place));
        }
        col = col.push(self.side_button("plus", "Connect to server…", Message::Net(NetMsg::Open(None)), self.net.form.is_some()));
        col.into()
    }

    fn net_item(&self, place: Place<'_>) -> Element<'_, Message> {
        let p = &self.palette;
        let pane = self.pane();
        let connecting = place.address.as_ref().is_some_and(|a| self.is_connecting(a));
        let active = match (place.mount, &pane.shares) {
            (_, Some(pg)) => place.address.as_ref().is_some_and(|a| a.share.is_none() && same_server(a, &pg.address)),
            (Some(m), None) => !pane.special() && pane.location.starts_with(&m.root),
            (None, None) => false,
        };
        let connected = place.mount.is_some();
        let ink = if active { color(p.ink_strong) } else if connected { color(p.ink) } else { color(p.ink_muted) };
        let head = text(place.name.clone()).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(ink).wrapping(text::Wrapping::None);
        // Connected: tinted icon and a disconnect button. Not connected: all muted. The
        // server itself is always on the second line.
        let meta_row: Element<'_, Message> = if connecting {
            w::pill(p, "Connecting…", Some(p.info))
        } else {
            let meta = match &place.address {
                Some(a) => format!("{} · {}", a.protocol.label(), a.who_where()),
                None => place.mount.map(|m| m.kind.clone()).unwrap_or_default(),
            };
            text(meta).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None).into()
        };
        let lines = column![head, meta_row].spacing(3).width(Length::Fill).clip(true);
        let icon = place.address.as_ref().map_or("network", |a| protocol_icon(a.protocol));
        let tint = if active { color(p.accent_ink) } else if connected { color(p.world_network) } else { color(p.ink_muted) };
        let mut r = row![container(w::glyph(&self.icons, icon, 16.0, tint)).padding([2, 0]), lines].spacing(style::SPACE_4).align_y(Alignment::Start);
        if connected {
            // Inside the row, so the highlight covers it; a click here doesn't open the place.
            let eject = self.tip(w::icon_button(p, &self.icons, "arrow-up", Some(Message::Net(NetMsg::Disconnect(place.key.clone()))), false), "Disconnect", None);
            r = r.push(eject);
        }
        let body = w::row_button(p, container(r).padding(iced::Padding { top: 6.0, bottom: 6.0, left: 0.0, right: 0.0 }).into(), active, Some(Message::Net(NetMsg::Click(place.key.clone()))));
        let tip = match (&place.address, place.mount) {
            (Some(a), Some(_)) => format!("{} · connected", a.uri()),
            (Some(a), None) => format!("{} · click to connect", a.uri()),
            (None, Some(m)) => m.name.clone(),
            (None, None) => place.key.clone(),
        };
        let item = self.tip(body, &tip, None);
        let key = place.key.clone();
        w::context_area(item, move |at| Message::Net(NetMsg::Menu(key.clone(), at)))
    }

    /// Right-click menu entries for a network place.
    pub(crate) fn net_menu(&self, key: &str) -> Vec<crate::overlay::Entry> {
        use crate::overlay::item;
        let Some(place) = self.net_places().into_iter().find(|p| p.key == key) else { return Vec::new() };
        let n = |m: NetMsg| Message::Net(m);
        let mut e = Vec::new();
        match place.mount {
            Some(m) => {
                let landing = m.landing.clone();
                e.push(item("external", "Open", None, n(NetMsg::Click(key.to_string()))));
                e.push(item("plus", "Open in new tab", None, Message::OpenTab(landing)));
                e.push(item("arrow-up", "Disconnect", None, n(NetMsg::Disconnect(key.to_string()))));
            }
            None => e.push(item("network", "Connect", None, n(NetMsg::Click(key.to_string())))),
        }
        if let Some(a) = &place.address {
            e.push(item("link", "Copy address", None, n(NetMsg::CopyAddress(a.uri()))));
            e.push(item("rename", "Edit address…", None, n(NetMsg::Open(Some(a.uri())))));
        }
        if place.saved {
            e.push(item("close", "Remove from sidebar", None, n(NetMsg::Forget(key.to_string()))));
        } else if let Some(a) = &place.address {
            e.push(item("pin", "Keep in sidebar", None, n(NetMsg::Save(a.server().uri()))));
        }
        e
    }

    // ------------------------------------------------------------------ Connect to Server

    fn list_row<'a>(&'a self, icon: &'static str, title: String, sub: String, msg: Message) -> Element<'a, Message> {
        let p = &self.palette;
        let r = row![
            w::glyph(&self.icons, icon, 16.0, color(p.world_network)),
            column![
                text(title).size(style::BODY).font(style::FONT).color(color(p.ink)).wrapping(text::Wrapping::None),
                text(sub).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None),
            ]
            .spacing(2)
            .width(Length::Fill),
            w::glyph(&self.icons, "chevron-right", 12.0, color(p.ink_faint)),
        ]
        .spacing(style::SPACE_4)
        .align_y(Alignment::Center);
        w::row_button(p, container(r).padding([5, 0]).clip(true).into(), false, Some(msg))
    }

    fn group_head<'a>(&self, title: &str, trailing: Option<Element<'a, Message>>) -> Element<'a, Message> {
        let p = &self.palette;
        let mut r = row![text(title.to_uppercase()).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted)).width(Length::Fill)].align_y(Alignment::Center);
        if let Some(t) = trailing {
            r = r.push(t);
        }
        container(r).height(24).align_y(Alignment::Center).into()
    }

    /// Connect to Server (design system `ConnectDialog`).
    pub(crate) fn connect_layer(&self) -> Option<Element<'_, Message>> {
        let f = self.net.form.as_ref()?;
        // While the server asks its own question, that dialog stands alone.
        if f.busy.is_some() && matches!(self.dialog, Some(Dialog::Login { .. } | Dialog::Question { .. })) {
            return None;
        }
        let p = &self.palette;
        let busy = f.busy.is_some();
        let options: Vec<(&str, Message)> = Protocol::ALL.iter().map(|&pr| (pr.label(), Message::Net(NetMsg::SetProtocol(pr)))).collect();
        let selected = Protocol::ALL.iter().position(|&x| x == f.protocol).unwrap_or(0);
        let proto_row = row![
            w::segmented(p, &options, selected),
            text(f.protocol.describe()).size(style::META).font(style::FONT).color(color(p.ink_muted)),
        ]
        .spacing(style::SPACE_4)
        .align_y(Alignment::Center);

        let placeholder = match f.protocol {
            Protocol::Smb => "nas.local/Media   or   \\\\nas\\Media",
            Protocol::Sftp => "me@server.local:/home/me",
            Protocol::Ftp | Protocol::Ftps => "ftp.example.com/pub",
        };
        let parsed = (!f.text.trim().is_empty()).then(|| Address::parse(&f.text, f.protocol));
        let invalid = f.error.is_some() || matches!(parsed, Some(Err(_)));
        let has_scheme = f.text.contains("://") || f.text.starts_with("\\\\");
        // The scheme chip is always the first child (empty once a scheme is typed) so the
        // field keeps its place in the widget tree, and with it focus and the cursor.
        // Same widget either way (a container holding text): swapping widget kinds here
        // resets the field beside it and drops focus mid-typing.
        let pal = p.clone();
        let chip = container(text(if has_scheme { String::new() } else { format!("{}://", f.protocol.scheme()) }).size(style::BODY).font(style::FONT).color(color(p.ink_muted)))
            .padding(if has_scheme { iced::Padding::ZERO } else { iced::Padding::from([6, 8]) })
            .style(move |_| if has_scheme { container::Style::default() } else { container::Style { background: Some(Background::Color(color(pal.bg_deep))), border: Border { color: color(pal.line_strong), width: 1.0, radius: 2.0.into() }, ..Default::default() } });
        let mut field_row = row![chip].align_y(Alignment::Center);
        let mut input = text_input(placeholder, &f.text).id(CONNECT_ID).size(style::BODY).font(style::FONT).padding([6, 10]).style(w::field_style(p, invalid));
        if !busy {
            input = input.on_input(|t| Message::Net(NetMsg::Draft(t))).on_submit(Message::Net(NetMsg::Submit));
        }
        field_row = field_row.push(input);

        let feedback: Element<'_, Message> = match &parsed {
            None => text("Type a server name or IP, or paste an address like smb://, sftp:// or \\\\server\\share.").size(style::META).font(style::FONT).color(color(p.ink_muted)).into(),
            Some(Ok(a)) => row![w::glyph(&self.icons, "check", 14.0, color(p.success.ink)), text(describe(a)).size(style::META).font(style::FONT).color(color(p.ink_muted))].spacing(6).align_y(Alignment::Center).into(),
            Some(Err(e)) => row![w::glyph(&self.icons, "alert", 14.0, color(p.warning.ink)), text(e.clone()).size(style::META).font(style::FONT).color(color(p.warning.ink))].spacing(6).align_y(Alignment::Center).into(),
        };

        let mut body = column![proto_row, column![field_row, feedback].spacing(6)].spacing(style::SPACE_4);
        if !self.net.protocols.is_empty() && !self.net.protocols.contains(&f.protocol) {
            let hint = match f.protocol {
                Protocol::Smb => "Windows shares need the gvfs-smb package: sudo pacman -S gvfs-smb",
                _ => "This computer's GVfs can't speak this protocol. Install the gvfs package.",
            };
            body = body.push(row![w::glyph(&self.icons, "alert", 14.0, color(p.warning.ink)), text(hint).size(style::META).font(style::FONT).color(color(p.warning.ink))].spacing(6).align_y(Alignment::Center));
        }
        if let Some(u) = &self.net.unavailable {
            body = body.push(row![w::glyph(&self.icons, "alert", 14.0, color(p.warning.ink)), text(u.clone()).size(style::META).font(style::FONT).color(color(p.warning.ink))].spacing(6).align_y(Alignment::Center));
        }
        body = body.push(w::checkbox(p, &self.icons, "Add to sidebar", f.save, |on| Message::Net(NetMsg::SaveToggle(on))));

        // Recent and nearby servers: one click connects.
        let mut lists = column![].spacing(2);
        let mut recent: Vec<Address> = Vec::new();
        for a in self.net.recent.iter().filter_map(|u| Address::parse(u, Protocol::Smb).ok()) {
            if recent.len() < 4 && !recent.iter().any(|r| same_server(r, &a)) {
                recent.push(a);
            }
        }
        if !recent.is_empty() {
            lists = lists.push(self.group_head("Recent", None));
            for a in recent {
                lists = lists.push(self.list_row(protocol_icon(a.protocol), a.display_name(), a.uri(), Message::Net(NetMsg::Pick(a.clone()))));
            }
            lists = lists.push(Space::new().height(style::SPACE_3));
        }
        let rescan: Element<'_, Message> = if self.net.scanning {
            text("Looking…").size(style::LABEL).font(style::FONT).color(color(p.ink_muted)).into()
        } else {
            self.tip(w::icon_button(p, &self.icons, "refresh", Some(Message::Net(NetMsg::Scan)), false), "Look again", None)
        };
        lists = lists.push(self.group_head("On this network", Some(rescan)));
        match &self.net.nearby {
            None => lists = lists.push(text("Looking for servers nearby…").size(style::META).font(style::FONT).color(color(p.ink_muted))),
            Some(Err(e)) => lists = lists.push(text(e.clone()).size(style::META).font(style::FONT).color(color(p.ink_muted))),
            Some(Ok(v)) if v.is_empty() => lists = lists.push(text("No servers are announcing themselves here. Type an address above.").size(style::META).font(style::FONT).color(color(p.ink_muted))),
            Some(Ok(v)) => {
                for n in v.iter().take(8) {
                    lists = lists.push(self.list_row(protocol_icon(n.address.protocol), n.name.clone(), format!("{} · {}", n.address.protocol.label(), n.address.host_port()), Message::Net(NetMsg::Pick(n.address.clone()))));
                }
            }
        }
        body = body.push(container(scrollable(lists).height(Length::Shrink)).max_height(220));

        if let Some(e) = &f.error {
            let pal = p.clone();
            body = body.push(
                container(row![w::glyph(&self.icons, "error", 14.0, color(p.danger.ink)), text(e.clone()).size(style::META).font(style::FONT).color(color(p.ink))].spacing(6).align_y(Alignment::Center))
                    .padding([8, 10])
                    .width(Length::Fill)
                    .style(move |_| container::Style { background: Some(Background::Color(color(pal.danger.soft))), border: Border { color: color(pal.danger.base), width: 1.0, radius: 2.0.into() }, ..Default::default() }),
            );
        }
        let can = matches!(parsed, Some(Ok(_))) && !busy;
        let foot = vec![
            w::text_button(p, &self.icons, "Cancel", None, Some("Esc"), Variant::Ghost, Some(Message::Net(NetMsg::Close))),
            w::text_button(p, &self.icons, if busy { "Connecting…" } else { "Connect" }, Some(if busy { "sync" } else { "network" }), (!busy).then_some("Enter"), Variant::Primary, can.then_some(Message::Net(NetMsg::Submit))),
        ];
        Some(self.dialog_frame_with("network", "Connect to server".into(), body.into(), foot, false, false, f.opened))
    }

    // ------------------------------------------------------------------ sign-in and questions

    fn labeled<'a>(&self, label: &str, field: Element<'a, Message>) -> Element<'a, Message> {
        let p = &self.palette;
        column![text(label.to_string()).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted)), field].spacing(4).width(Length::Fill).into()
    }

    /// Sign in to a server (design system `SignInDialog`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn login_dialog<'a>(&'a self, ask: &'a LoginAsk, user: &'a str, domain: &'a str, password: &'a str, guest: bool, remember: bool, opened: Instant) -> Element<'a, Message> {
        let p = &self.palette;
        let address = self.net.connecting.iter().find(|(o, _)| *o == ask.op).map(|(_, a)| a.clone());
        let title = match &address {
            Some(a) => format!("Sign in to {}", a.display_name()),
            None => "Sign in to the server".to_string(),
        };
        // GVfs' second line names what's asking ("Enter user and password for share …").
        let detail = ask.message.lines().filter(|l| !l.trim().is_empty() && !l.contains("Authentication Required")).collect::<Vec<_>>().join(" ");
        let detail = detail.trim_end_matches(':').to_string();
        let mut body = column![].spacing(style::SPACE_4);
        if !detail.is_empty() {
            body = body.push(text(detail).size(style::BODY).font(style::FONT).color(color(p.ink)));
        }
        if ask.can_anonymous {
            let opts = [("Registered user", Message::Net(NetMsg::LoginGuest(false))), ("Guest", Message::Net(NetMsg::LoginGuest(true)))];
            body = body.push(w::segmented(p, &opts, usize::from(guest)));
        }
        if guest {
            body = body.push(text("Connect without an account. Guests usually see only public folders.").size(style::META).font(style::FONT).color(color(p.ink_muted)));
        } else {
            let field = |id: &'static str, value: &'a str, ph: &'static str, f: LoginField, secure: bool| -> Element<'a, Message> {
                text_input(ph, value)
                    .id(id)
                    .secure(secure)
                    .on_input(move |v| Message::Net(NetMsg::LoginDraft(f, v)))
                    .on_submit(Message::Net(NetMsg::LoginSubmit))
                    .size(style::BODY)
                    .font(style::FONT)
                    .padding([6, 10])
                    .style(w::field_style(p, ask.retry && f == LoginField::Password))
                    .into()
            };
            let mut who = row![].spacing(style::SPACE_3);
            if ask.need_user {
                who = who.push(self.labeled("User name", field(LOGIN_USER_ID, user, "user", LoginField::User, false)));
            }
            if ask.need_domain {
                who = who.push(container(self.labeled("Domain", field(LOGIN_DOMAIN_ID, domain, "WORKGROUP", LoginField::Domain, false))).width(170));
            }
            if ask.need_user || ask.need_domain {
                body = body.push(who);
            } else if !user.is_empty() {
                body = body.push(text(format!("User: {user}")).size(style::META).font(style::FONT).color(color(p.ink_muted)));
            }
            if ask.need_password {
                body = body.push(self.labeled("Password", field(LOGIN_PASSWORD_ID, password, "", LoginField::Password, true)));
            }
            if ask.can_remember {
                body = body.push(w::checkbox(p, &self.icons, "Remember password in the keyring", remember, |on| Message::Net(NetMsg::LoginRemember(on))));
            }
        }
        if ask.retry {
            body = body.push(row![w::glyph(&self.icons, "error", 14.0, color(p.danger.ink)), text("That didn't work. Check the user name and password and try again.").size(style::META).font(style::FONT).color(color(p.danger.ink))].spacing(6).align_y(Alignment::Center));
        }
        let foot = vec![
            w::text_button(p, &self.icons, "Cancel", None, Some("Esc"), Variant::Ghost, Some(Message::Ui(UiMsg::CloseDialog))),
            w::text_button(p, &self.icons, if guest { "Connect as guest" } else { "Sign in" }, Some("key"), Some("Enter"), Variant::Primary, Some(Message::Net(NetMsg::LoginSubmit))),
        ];
        self.dialog_frame_with("lock", title, body.into(), foot, false, false, opened)
    }

    /// A question from the server connection, e.g. an unknown SSH host key.
    pub(crate) fn question_dialog<'a>(&'a self, ask: &'a QuestionAsk, opened: Instant) -> Element<'a, Message> {
        let p = &self.palette;
        let lines: Vec<&str> = ask.message.lines().filter(|l| !l.trim().is_empty()).collect();
        let (title, rest) = match lines.split_first() {
            Some((first, rest)) if first.len() < 80 && !rest.is_empty() => (first.to_string(), rest.join("\n")),
            _ => ("Check this server".to_string(), ask.message.trim().to_string()),
        };
        let pal = p.clone();
        let body = container(text(rest).size(style::META).font(style::FONT).color(color(p.ink)))
            .padding(10)
            .width(Length::Fill)
            .style(move |_| container::Style { background: Some(Background::Color(color(pal.bg_deep))), border: Border { color: color(pal.line), width: 1.0, radius: 2.0.into() }, ..Default::default() });
        // Cancel-like choices first and quiet, the way every other dialog puts them; the
        // first real choice is the primary button.
        let (mut foot, mut rest) = (Vec::new(), Vec::new());
        for (i, c) in ask.choices.iter().enumerate() {
            let msg = Some(Message::Net(NetMsg::Answer(i)));
            if c.to_lowercase().contains("cancel") {
                foot.push(w::text_button(p, &self.icons, c, None, None, Variant::Ghost, msg));
            } else {
                let v = if rest.is_empty() { Variant::Primary } else { Variant::Secondary };
                rest.push(w::text_button(p, &self.icons, c, None, None, v, msg));
            }
        }
        rest.reverse();
        foot.extend(rest);
        self.dialog_frame_with("shield", title, body.into(), foot, false, false, opened)
    }

    // ------------------------------------------------------------------ shares page

    /// An SMB server's shares as cards (design system `SharesPage`).
    pub(crate) fn shares_view<'a>(&'a self, page: &'a SharesPage) -> Element<'a, Message> {
        let p = &self.palette;
        let a = &page.address;
        let head = column![
            text(format!("Shares on {}", a.host_port())).size(22).font(style::FONT_BOLD).color(color(p.ink_strong)),
            text("Pick a share to open it. It's added to the Network section while it's connected.").size(style::META).font(style::FONT).color(color(p.ink_muted)),
        ]
        .spacing(6);
        let content: Element<'a, Message> = match &page.shares {
            None => row![w::glyph(&self.icons, "sync", 16.0, color(p.info.ink)), text("Asking the server for its shares…").size(style::BODY).font(style::FONT).color(color(p.ink_muted))].spacing(style::SPACE_3).align_y(Alignment::Center).into(),
            Some(Err(e)) => {
                let retry = w::text_button(p, &self.icons, "Try again", Some("refresh"), None, Variant::Secondary, Some(Message::Net(NetMsg::Connect { address: a.clone(), save: false })));
                column![row![w::glyph(&self.icons, "error", 16.0, color(p.danger.ink)), text(e.clone()).size(style::BODY).font(style::FONT).color(color(p.ink))].spacing(style::SPACE_3).align_y(Alignment::Center), retry].spacing(style::SPACE_4).into()
            }
            Some(Ok(v)) if v.is_empty() => text("This server doesn't share any folders with you.").size(style::BODY).font(style::FONT).color(color(p.ink_muted)).into(),
            Some(Ok(v)) => {
                let mut grid = column![].spacing(style::SPACE_4);
                let mut it = v.iter().peekable();
                while it.peek().is_some() {
                    let mut r = row![].spacing(style::SPACE_4);
                    for _ in 0..4 {
                        if let Some(s) = it.next() {
                            r = r.push(self.share_card(a, s));
                        }
                    }
                    grid = grid.push(r);
                }
                grid.into()
            }
        };
        w::fill(scrollable(container(column![head, content, Space::new().height(24)].spacing(style::SPACE_5)).padding([24, 28])).height(Length::Fill), p.bg).width(Length::Fill).height(Length::Fill).into()
    }

    fn share_card<'a>(&'a self, server: &Address, share: &str) -> Element<'a, Message> {
        let p = &self.palette;
        let target = Address { share: Some(share.to_string()), path: String::new(), ..server.clone() };
        let connected = self.net.mounts.iter().any(|m| m.serves(&target));
        let connecting = self.is_connecting(&target);
        let mut sub = row![].spacing(style::SPACE_3).align_y(Alignment::Center);
        if connecting {
            sub = sub.push(w::pill(p, "Connecting…", Some(p.info)));
        } else if connected {
            sub = sub.push(w::pill(p, "Connected", Some(p.success)));
        } else {
            sub = sub.push(text("Windows share").size(style::META).font(style::FONT).color(color(p.ink_muted)));
        }
        let body = column![
            iced::widget::svg(self.icons.color("folder-share")).width(40).height(40),
            text(share.to_string()).size(15).font(style::FONT_BOLD).color(color(p.ink_strong)).wrapping(text::Wrapping::None),
            sub,
        ]
        .spacing(style::SPACE_3);
        let pal = p.clone();
        let card = button(container(body).padding(16).width(200).clip(true))
            .padding(0)
            .on_press(Message::Net(NetMsg::Connect { address: target, save: false }))
            .style(move |_, status| button::Style {
                background: Some(Background::Color(color(pal.bg_raised))),
                border: Border { color: if matches!(status, button::Status::Hovered) { color(pal.world_network) } else { color(pal.line) }, width: 1.0, radius: 2.0.into() },
                text_color: color(pal.ink),
                ..Default::default()
            });
        mouse_area(card).into()
    }
}

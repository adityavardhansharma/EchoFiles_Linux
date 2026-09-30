//! Network mounts through GVfs, the GNOME virtual file system daemon that Nautilus uses.
//!
//! GVfs speaks SMB (libsmbclient), SFTP (OpenSSH) and FTP and keeps the connections; its
//! FUSE bridge exposes every mount as an ordinary folder under `$XDG_RUNTIME_DIR/gvfs`, so
//! listing, previews and the copy engine work on network files unchanged.
//!
//! EchoFiles asks for mounts over the session bus (`org.gtk.vfs.MountTracker`) and serves
//! its own `org.gtk.vfs.MountOperation` object, so log-in and host-key questions show in
//! EchoFiles' dialogs rather than a terminal or nowhere.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_channel::oneshot;
use zbus::blocking::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::address::{Address, Protocol};

const DAEMON: &str = "org.gtk.vfs.Daemon";
const TRACKER_PATH: &str = "/org/gtk/vfs/mounttracker";
const TRACKER: &str = "org.gtk.vfs.MountTracker";

/// GVfs mount types that are network places.
const NETWORK_TYPES: [&str; 8] = ["smb-share", "smb-server", "sftp", "ftp", "ftps", "ftpis", "afp-volume", "nfs"];

// ------------------------------------------------------------------------------ questions

/// How long GVfs (and the keyring) keep a password.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remember {
    Never = 0,
    /// Until logout.
    Session = 1,
    /// In the keyring.
    Forever = 2,
}

/// The answer to a log-in question.
#[derive(Clone, Debug)]
pub struct Login {
    pub user: String,
    pub domain: String,
    pub password: String,
    pub anonymous: bool,
    pub remember: Remember,
}

type Reply<T> = Arc<Mutex<Option<oneshot::Sender<Option<T>>>>>;

/// The server wants a user name and password. Answer with [`LoginAsk::answer`]; `None`
/// cancels the connection.
#[derive(Clone)]
pub struct LoginAsk {
    /// The connection attempt this belongs to.
    pub op: u64,
    /// GVfs' sentence, e.g. "Enter password for share “media” on “nas”".
    pub message: String,
    pub default_user: String,
    pub default_domain: String,
    pub need_user: bool,
    pub need_domain: bool,
    pub need_password: bool,
    pub can_remember: bool,
    pub can_anonymous: bool,
    /// An earlier answer in this attempt didn't work.
    pub retry: bool,
    reply: Reply<Login>,
}

impl LoginAsk {
    pub fn answer(&self, login: Option<Login>) {
        if let Some(tx) = self.reply.lock().unwrap().take() {
            let _ = tx.send(login);
        }
    }
}

impl std::fmt::Debug for LoginAsk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginAsk").field("op", &self.op).field("message", &self.message).field("retry", &self.retry).finish()
    }
}

/// A multiple-choice question, e.g. SFTP's "The identity of the remote computer is
/// unknown… Log In Anyway / Cancel Login".
#[derive(Clone)]
pub struct QuestionAsk {
    pub op: u64,
    pub message: String,
    pub choices: Vec<String>,
    reply: Reply<usize>,
}

impl QuestionAsk {
    pub fn answer(&self, choice: Option<usize>) {
        if let Some(tx) = self.reply.lock().unwrap().take() {
            let _ = tx.send(choice);
        }
    }
}

impl std::fmt::Debug for QuestionAsk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuestionAsk").field("op", &self.op).field("message", &self.message).field("choices", &self.choices).finish()
    }
}

/// Something GVfs needs from the person while connecting.
#[derive(Clone, Debug)]
pub enum Ask {
    Login(LoginAsk),
    Question(QuestionAsk),
    /// GVfs gave up waiting on the open question (the connection timed out).
    Withdrawn(u64),
}

type OnAsk = Arc<dyn Fn(Ask) + Send + Sync>;

// GAskPasswordFlags
const NEED_PASSWORD: u32 = 1 << 0;
const NEED_USERNAME: u32 = 1 << 1;
const NEED_DOMAIN: u32 = 1 << 2;
const SAVING_SUPPORTED: u32 = 1 << 3;
const ANONYMOUS_SUPPORTED: u32 = 1 << 4;

struct MountOp {
    op: u64,
    on_ask: OnAsk,
    asked: Mutex<u32>,
    cancelled: Arc<AtomicBool>,
}

#[zbus::interface(name = "org.gtk.vfs.MountOperation")]
impl MountOp {
    /// → (handled, aborted, password, username, domain, anonymous, password_save)
    async fn ask_password(&self, message_string: String, default_user: String, default_domain: String, flags_as_int: u32) -> (bool, bool, String, String, String, bool, u32) {
        let retry = {
            let mut n = self.asked.lock().unwrap();
            *n += 1;
            *n > 1
        };
        let (tx, rx) = oneshot::channel();
        (self.on_ask)(Ask::Login(LoginAsk {
            op: self.op,
            message: message_string,
            default_user,
            default_domain,
            need_user: flags_as_int & NEED_USERNAME != 0,
            need_domain: flags_as_int & NEED_DOMAIN != 0,
            need_password: flags_as_int & NEED_PASSWORD != 0,
            can_remember: flags_as_int & SAVING_SUPPORTED != 0,
            can_anonymous: flags_as_int & ANONYMOUS_SUPPORTED != 0,
            retry,
            reply: Arc::new(Mutex::new(Some(tx))),
        }));
        match rx.await {
            Ok(Some(l)) => (true, false, l.password, l.user, l.domain, l.anonymous, l.remember as u32),
            _ => {
                self.cancelled.store(true, Ordering::Relaxed);
                (true, true, String::new(), String::new(), String::new(), false, 0)
            }
        }
    }

    /// → (handled, aborted, choice)
    async fn ask_question(&self, message_string: String, choices: Vec<String>) -> (bool, bool, u32) {
        let (tx, rx) = oneshot::channel();
        (self.on_ask)(Ask::Question(QuestionAsk { op: self.op, message: message_string, choices, reply: Arc::new(Mutex::new(Some(tx))) }));
        match rx.await {
            Ok(Some(c)) => (true, false, c as u32),
            _ => {
                self.cancelled.store(true, Ordering::Relaxed);
                (true, true, 0)
            }
        }
    }

    /// Files are still open on the mount being unmounted. Decline; the caller reports it
    /// and offers to disconnect anyway.
    async fn show_processes(&self, _message_string: String, _choices: Vec<String>, _processes: Vec<i32>) -> (bool, bool, u32) {
        (true, true, 0)
    }

    async fn show_unmount_progress(&self, _message_string: String, _time_left: i64, _bytes_left: i64) {}

    async fn aborted(&self) {
        (self.on_ask)(Ask::Withdrawn(self.op));
    }
}

static NEXT_OP: AtomicU64 = AtomicU64::new(1);

/// A fresh operation id, so the UI can match questions to the connection they're about.
pub fn next_op() -> u64 {
    NEXT_OP.fetch_add(1, Ordering::Relaxed)
}

/// A private session-bus connection serving one mount operation object.
fn op_connection(op: u64, on_ask: OnAsk, cancelled: Arc<AtomicBool>) -> Result<(Connection, String), Error> {
    let path = format!("/org/echofiles/MountOp/{op}");
    let conn = zbus::blocking::connection::Builder::session()
        .and_then(|b| b.serve_at(path.as_str(), MountOp { op, on_ask, asked: Mutex::new(0), cancelled }))
        .and_then(|b| b.build())
        .map_err(|e| Error::Unavailable(format!("couldn't reach the session bus: {e}")))?;
    Ok((conn, path))
}

// ------------------------------------------------------------------------------ mounts

/// A connected network place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    /// GVfs' name for it: "media on nas.local".
    pub name: String,
    /// GVfs type: `smb-share`, `sftp`, `ftp`…
    pub kind: String,
    /// Where it is on disk (the FUSE folder).
    pub root: PathBuf,
    /// Where to land when opening it (SFTP: the home folder on the server).
    pub landing: PathBuf,
    /// The address it answers to.
    pub address: Option<Address>,
    dbus_id: String,
    object: String,
}

impl Mount {
    pub fn protocol(&self) -> Option<Protocol> {
        self.address.as_ref().map(|a| a.protocol)
    }

    /// The mount is the connection `a` asks for (same server, port, user and share).
    pub fn serves(&self, a: &Address) -> bool {
        let Some(m) = &self.address else { return false };
        m.protocol == a.protocol
            && m.host.eq_ignore_ascii_case(&a.host)
            && m.port.unwrap_or(m.protocol.default_port()) == a.port.unwrap_or(a.protocol.default_port())
            && (a.user.is_none() || m.user.is_none() || m.user == a.user)
            && match (&m.share, &a.share) {
                (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
                (None, None) => true,
                _ => false,
            }
    }

    /// The folder `a` points at inside this mount.
    pub fn path_for(&self, a: &Address) -> PathBuf {
        if a.path.is_empty() { self.landing.clone() } else { self.root.join(&a.path) }
    }
}

type Spec = (Vec<u8>, HashMap<String, OwnedValue>);
type RawMount = (String, OwnedObjectPath, String, String, String, String, String, String, bool, Vec<u8>, Spec, Vec<u8>);

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

fn spec_item(spec: &Spec, key: &str) -> Option<String> {
    let v = spec.1.get(key)?;
    match &**v {
        Value::Array(a) => {
            let bytes: Vec<u8> = a.iter().filter_map(|b| if let Value::U8(b) = b { Some(*b) } else { None }).collect();
            Some(cstr(&bytes)).filter(|s| !s.is_empty())
        }
        Value::Str(s) => Some(s.to_string()),
        _ => None,
    }
}

fn address_of(spec: &Spec) -> Option<Address> {
    let kind = spec_item(spec, "type")?;
    let protocol = match kind.as_str() {
        "smb-share" | "smb-server" => Protocol::Smb,
        "sftp" => Protocol::Sftp,
        "ftp" => Protocol::Ftp,
        "ftps" | "ftpis" => Protocol::Ftps,
        _ => return None,
    };
    let host = spec_item(spec, "host").or_else(|| spec_item(spec, "server"))?;
    Some(Address {
        protocol,
        host,
        port: spec_item(spec, "port").and_then(|p| p.parse().ok()),
        user: spec_item(spec, "user"),
        domain: spec_item(spec, "domain"),
        share: spec_item(spec, "share"),
        path: String::new(),
    })
}

fn mount_of(raw: RawMount) -> Option<Mount> {
    let (dbus_id, object, name, _stable, _x, _icon, _sym, _enc, _visible, fuse, spec, default) = raw;
    let kind = spec_item(&spec, "type")?;
    if !NETWORK_TYPES.contains(&kind.as_str()) {
        return None;
    }
    let root = PathBuf::from(cstr(&fuse));
    if root.as_os_str().is_empty() {
        return None;
    }
    let prefix = cstr(&spec.0);
    let default = cstr(&default);
    // `default_location` is a path on the server, below the mount prefix.
    let rel = default.strip_prefix(prefix.trim_end_matches('/')).unwrap_or(&default).trim_start_matches('/');
    let landing = if rel.is_empty() { root.clone() } else { root.join(rel) };
    Some(Mount { name, kind, landing, root, address: address_of(&spec), dbus_id, object: object.to_string() })
}

/// Why a connection didn't happen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The person cancelled a question.
    Cancelled,
    /// GVfs (or one of its backends) isn't installed or running.
    Unavailable(String),
    Failed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Cancelled => f.write_str("cancelled"),
            Error::Unavailable(m) | Error::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

fn failed(e: zbus::Error) -> Error {
    match e {
        zbus::Error::MethodError(name, msg, _) => {
            let name = name.as_str();
            if name.contains("ServiceUnknown") || name.contains("NameHasNoOwner") {
                return Error::Unavailable("GVfs isn't running. Install the gvfs package (and gvfs-smb for Windows shares).".into());
            }
            Error::Failed(msg.unwrap_or_else(|| name.to_string()))
        }
        other => Error::Failed(other.to_string()),
    }
}

fn session() -> Result<Connection, Error> {
    Connection::session().map_err(|e| Error::Unavailable(format!("couldn't reach the session bus: {e}")))
}

/// Every connected network place, in GVfs' order.
pub fn mounts() -> Result<Vec<Mount>, Error> {
    let conn = session()?;
    let reply = conn.call_method(Some(DAEMON), TRACKER_PATH, Some(TRACKER), "ListMounts2", &(true,)).map_err(failed)?;
    let raw: Vec<RawMount> = reply.body().deserialize().map_err(|e| Error::Failed(e.to_string()))?;
    // Connected places without a folder: the FUSE bridge isn't up. Start it and ask again.
    let headless = raw.iter().any(|m| m.9.iter().all(|&b| b == 0) && spec_item(&m.10, "type").is_some_and(|t| NETWORK_TYPES.contains(&t.as_str()) && t != "smb-server"));
    if headless && !is_mount_point(&fuse_root()) && ensure_fuse().is_ok() {
        let reply = conn.call_method(Some(DAEMON), TRACKER_PATH, Some(TRACKER), "ListMounts2", &(true,)).map_err(failed)?;
        let raw: Vec<RawMount> = reply.body().deserialize().map_err(|e| Error::Failed(e.to_string()))?;
        return Ok(raw.into_iter().filter_map(mount_of).collect());
    }
    Ok(raw.into_iter().filter_map(mount_of).collect())
}

/// Which protocols this computer's GVfs can speak (gvfs-smb is a separate package).
pub fn protocols() -> Result<Vec<Protocol>, Error> {
    let conn = session()?;
    let reply = conn.call_method(Some(DAEMON), TRACKER_PATH, Some(TRACKER), "ListMountTypes", &()).map_err(failed)?;
    let types: Vec<String> = reply.body().deserialize().map_err(|e| Error::Failed(e.to_string()))?;
    Ok(Protocol::ALL
        .into_iter()
        .filter(|p| {
            let t = match p {
                Protocol::Smb => "smb-share",
                Protocol::Sftp => "sftp",
                Protocol::Ftp => "ftp",
                Protocol::Ftps => "ftps",
            };
            types.iter().any(|x| x == t)
        })
        .collect())
}

fn bytestring(s: &str) -> Value<'static> {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    Value::from(b)
}

fn spec_for(a: &Address) -> (Vec<u8>, HashMap<String, Value<'static>>) {
    let mut items: HashMap<String, Value<'static>> = HashMap::new();
    let (kind, host_key) = match (a.protocol, &a.share) {
        (Protocol::Smb, Some(_)) => ("smb-share", "server"),
        (Protocol::Smb, None) => ("smb-server", "server"),
        (Protocol::Sftp, _) => ("sftp", "host"),
        (Protocol::Ftp, _) => ("ftp", "host"),
        (Protocol::Ftps, _) => ("ftps", "host"),
    };
    items.insert("type".into(), bytestring(kind));
    // GVfs' SMB backend names mounts in lower case; match it so lookups agree.
    let host = if a.protocol == Protocol::Smb { a.host.to_lowercase() } else { a.host.clone() };
    items.insert(host_key.into(), bytestring(&host));
    if let Some(share) = &a.share {
        items.insert("share".into(), bytestring(&share.to_lowercase()));
    }
    if let Some(p) = a.port {
        items.insert("port".into(), bytestring(&p.to_string()));
    }
    if let Some(u) = &a.user {
        items.insert("user".into(), bytestring(u));
    }
    if let Some(d) = &a.domain {
        items.insert("domain".into(), bytestring(d));
    }
    (b"/\0".to_vec(), items)
}

/// Make sure GVfs' FUSE bridge is up so mounts appear as folders. Desktops start it with
/// GVfs; a bare Hyprland session sometimes doesn't.
pub fn ensure_fuse() -> Result<PathBuf, Error> {
    let root = fuse_root();
    if is_mount_point(&root) {
        return Ok(root);
    }
    let helper = ["/usr/lib/gvfsd-fuse", "/usr/libexec/gvfsd-fuse", "/usr/lib/gvfs/gvfsd-fuse"].into_iter().map(Path::new).find(|p| p.exists());
    let Some(helper) = helper else {
        return Err(Error::Unavailable("GVfs' FUSE helper (gvfsd-fuse) isn't installed. Install the gvfs package.".into()));
    };
    let _ = std::fs::create_dir_all(&root);
    // It daemonizes itself and stays for the session, like the one GVfs starts.
    let status = std::process::Command::new(helper).arg(&root).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    if let Err(e) = status {
        return Err(Error::Unavailable(format!("couldn't start gvfsd-fuse: {e}")));
    }
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until {
        if is_mount_point(&root) {
            return Ok(root);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(Error::Unavailable("GVfs' FUSE bridge didn't start, so network folders can't open.".into()))
}

/// `$XDG_RUNTIME_DIR/gvfs` — where network places appear as folders.
pub fn fuse_root() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { getuid() }))).join("gvfs")
}

/// Inside a network place (so: slow, no Trash, no thumbnails).
pub fn is_network_path(p: &Path) -> bool {
    p.starts_with(fuse_root())
}

/// A readable name for a GVfs FUSE folder (`smb-share:server=nas,share=media` →
/// "media on nas"), for places that only have the path.
pub fn folder_name(p: &Path) -> Option<String> {
    if p.parent()? != fuse_root() {
        return None;
    }
    let name = p.file_name()?.to_string_lossy().into_owned();
    let (_kind, rest) = name.split_once(':')?;
    let get = |k: &str| rest.split(',').find_map(|kv| kv.strip_prefix(k).and_then(|v| v.strip_prefix('='))).map(str::to_string);
    let host = get("server").or_else(|| get("host"))?;
    Some(match (get("share"), get("user")) {
        (Some(share), _) => format!("{share} on {host}"),
        (None, Some(u)) => format!("{u} on {host}"),
        _ => host,
    })
}

fn is_mount_point(p: &Path) -> bool {
    let Ok(info) = std::fs::read_to_string("/proc/self/mountinfo") else { return false };
    let want = p.to_string_lossy();
    info.lines().any(|l| l.split(' ').nth(4).is_some_and(|mp| mp == want))
}

unsafe extern "C" {
    fn getuid() -> u32;
}

/// Ask GVfs to mount `a`'s spec, answering its questions through `on_ask`.
fn mount_location(a: &Address, op: u64, on_ask: impl Fn(Ask) + Send + Sync + 'static) -> Result<(), Error> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let (conn, path) = op_connection(op, Arc::new(on_ask), cancelled.clone())?;
    let me = conn.unique_name().map(|n| n.to_string()).unwrap_or_default();
    let object = zbus::zvariant::ObjectPath::try_from(path.as_str()).map_err(|e| Error::Failed(e.to_string()))?;
    let r = conn.call_method(Some(DAEMON), TRACKER_PATH, Some(TRACKER), "MountLocation", &(spec_for(a), (me.as_str(), object)));
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    match r.map_err(failed) {
        Err(Error::Failed(m)) if m.to_lowercase().contains("already mounted") => Ok(()),
        Err(Error::Failed(m)) if !resolves(a) => Err(Error::Failed(format!("Hostname not known ({m})"))),
        other => other.map(drop),
    }
}

/// The host name resolves (DNS, /etc/hosts or mDNS). SMB reports an unknown host as
/// "Invalid argument"; this tells the two apart.
fn resolves(a: &Address) -> bool {
    use std::net::ToSocketAddrs;
    (a.host.as_str(), a.port.unwrap_or(a.protocol.default_port())).to_socket_addrs().is_ok_and(|mut i| i.next().is_some())
}

/// Connect to `a`, asking questions through `on_ask`. Blocks until connected, failed or
/// cancelled; run it off the UI thread. `op` comes from [`next_op`]. SMB addresses need a
/// share; use [`shares`] for a bare server.
pub fn mount(a: &Address, op: u64, on_ask: impl Fn(Ask) + Send + Sync + 'static) -> Result<Mount, Error> {
    ensure_fuse()?;
    if let Some(m) = mounts()?.into_iter().find(|m| m.serves(a)) {
        return Ok(m);
    }
    mount_location(a, op, on_ask)?;
    // Found by address rather than spec: backends normalise names (case, default user).
    let found = mounts()?.into_iter().find(|m| m.serves(a));
    found.ok_or_else(|| Error::Failed("The server connected, but its folder didn't appear. Try again.".into()))
}

/// The shares an SMB server offers (hidden `$` shares left out), logging in first if it
/// wants that. GVfs keeps a browsed server off the FUSE bridge, so `gio list` reads them.
pub fn shares(a: &Address, op: u64, on_ask: impl Fn(Ask) + Send + Sync + 'static) -> Result<Vec<String>, Error> {
    let server = Address { share: None, path: String::new(), ..a.clone() };
    mount_location(&server, op, on_ask)?;
    let out = std::process::Command::new("gio")
        .args(["list", "--attributes=standard::display-name"])
        .arg(server.uri())
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| Error::Unavailable(format!("couldn't run gio to list the shares: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(Error::Failed(err.rsplit(": ").next().unwrap_or(&err).trim().to_string()));
    }
    let mut names: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            // "NAME\t0\t(mountable)\tstandard::display-name=Name"
            let display = l.split('\t').find_map(|f| f.strip_prefix("standard::display-name="));
            display.or_else(|| l.split('\t').next()).map(str::to_string)
        })
        .filter(|n| !n.is_empty() && !n.ends_with('$'))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    Ok(names)
}

/// Disconnect. `force` closes it even when files there are still open.
pub fn unmount(m: &Mount, force: bool) -> Result<(), Error> {
    let op = next_op();
    let (conn, path) = op_connection(op, Arc::new(|_| {}), Arc::default())?;
    let me = conn.unique_name().map(|n| n.to_string()).unwrap_or_default();
    let object = zbus::zvariant::ObjectPath::try_from(path.as_str()).map_err(|e| Error::Failed(e.to_string()))?;
    let flags: u32 = if force { 1 } else { 0 };
    conn.call_method(Some(m.dbus_id.as_str()), m.object.as_str(), Some("org.gtk.vfs.Mount"), "Unmount", &(me.as_str(), object, flags)).map_err(failed)?;
    Ok(())
}

/// Call `on_change` whenever any app connects or disconnects a place (Nautilus too).
/// Runs on its own thread for the life of the process.
pub fn watch(on_change: impl Fn() + Send + 'static) {
    let _ = std::thread::Builder::new().name("ef-net-watch".into()).spawn(move || {
        let Ok(conn) = Connection::session() else { return };
        let Ok(rule) = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).interface(TRACKER).map(|b| b.build()) else { return };
        let Ok(iter) = zbus::blocking::MessageIterator::for_match_rule(rule, &conn, Some(64)) else { return };
        for msg in iter.flatten() {
            let member = msg.header().member().map(|m| m.to_string());
            if matches!(member.as_deref(), Some("Mounted" | "Unmounted")) {
                on_change();
            }
        }
    });
}

/// Turn GVfs' error text into a sentence that says what to do.
pub fn explain(e: &Error, a: &Address) -> String {
    let raw = match e {
        Error::Cancelled => return "Connecting was cancelled.".into(),
        Error::Unavailable(m) => return m.clone(),
        Error::Failed(m) => m.as_str(),
    };
    let l = raw.to_lowercase();
    let where_ = a.host_port();
    if l.contains("connection refused") {
        format!("Nothing answered on {where_}. Check the address and port, and that the server is running.")
    } else if l.contains("hostname not known") || l.contains("name or service not known") || l.contains("could not resolve") || l.contains("failed to resolve") || l.contains("no address associated") {
        format!("Couldn't find a server called “{}”. Check the spelling, or use its IP address.", a.host)
    } else if l.contains("timed out") || l.contains("timeout") {
        format!("{where_} didn't answer in time. It may be off, asleep or behind a firewall.")
    } else if l.contains("no route") || l.contains("network is unreachable") {
        format!("Can't reach {where_} from this network.")
    } else if l.contains("permission denied") || l.contains("access denied") || l.contains("login") && l.contains("incorrect") || l.contains("authentication") {
        "The server didn't accept that user name and password.".into()
    } else if l.contains("share") && (l.contains("not found") || l.contains("doesn") || l.contains("bad network name")) || l.contains("no such file") {
        match &a.share {
            Some(s) => format!("{} has no share called “{s}”. Leave the share out to see the ones it has.", a.host),
            None => format!("{where_} doesn't have that folder."),
        }
    } else if l.contains("host key") || l.contains("identity") {
        format!("{where_}'s identity couldn't be checked, so EchoFiles didn't connect.")
    } else {
        let mut s = raw.trim().to_string();
        if !s.ends_with('.') {
            s.push('.');
        }
        s
    }
}

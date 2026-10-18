//! Your Android phone in EchoFiles (design system `PhoneItem`, `PhoneHub`, `PairPhone`,
//! `PhotosPage`, `MessagesPage`, `NotificationsPage`, `PhoneSettings`).
//!
//! `ef_phone` is the link to EchoConnect on the phone; this module keeps what the app
//! shows — the paired phones, battery, notifications, texts, transfers and the shared
//! clipboard — and shows the phone's files as a folder (`ef_phonefs`: our own SFTP
//! client, pinned to the key the phone named, mounted with FUSE), so once it's open every
//! other part of EchoFiles treats it like any folder. Views are in `phone_view.rs`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, Read};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ef_config::{self as config, PhoneNotifications};
use ef_phone::{Device, Event, Notification, Service, Sftp, Sms, Trusted};
use ef_phonefs::PhoneFs;
use iced::Task;
use iced::futures::channel::mpsc;

use crate::actions::FileMsg;
use crate::app::{App, Message, background};
use crate::overlay::Tone;
use crate::thumbs::Pixels;

pub const IP_ID: &str = "phone-ip";
pub const SMS_ID: &str = "phone-sms";
/// The scrolling body of the hub, Photos and Notifications pages.
pub const PAGE_SCROLL: &str = "phone-page";
const PHOTO_JOBS: usize = 4;
const PHOTOS_PAGE: usize = 240;

/// A page about the phone, shown in a pane like a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhonePage {
    Hub,
    Photos,
    Messages,
    Notifications,
}

impl PhonePage {
    pub fn title(self) -> &'static str {
        match self {
            PhonePage::Hub => "Phone",
            PhonePage::Photos => "Photos",
            PhonePage::Messages => "Messages",
            PhonePage::Notifications => "Notifications",
        }
    }
}

/// Where a photo came from (the Photos filter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    Camera,
    Screenshots,
    Chats,
    Other,
}

/// ((new photos, new screenshots, new others), total) for the Photos page, cached.
type NewCount = ((usize, usize, usize), usize);

#[derive(Debug, Clone)]
pub struct Photo {
    pub path: PathBuf,
    /// Seconds since the epoch.
    pub mtime: i64,
    pub size: u64,
    pub source: Source,
    pub video: bool,
}

/// A file going to or coming from the phone.
pub struct Moved {
    pub transfer: u64,
    pub name: String,
    pub size: u64,
    pub upload: bool,
    pub done: Arc<AtomicU64>,
    pub result: Option<Result<PathBuf, String>>,
    pub at: SystemTime,
}

/// The Connect phone dialog.
pub struct PairDialog {
    pub ip: String,
    pub opened: Instant,
    /// Looking since (for the "can't see it" help after 10 s).
    pub since: Instant,
    pub firewall: Option<Result<(), String>>,
    pub firewall_busy: bool,
    /// Paired while the dialog was open: it moves on to Allow files.
    pub paired: Option<String>,
}

/// What to do once the phone's files are mounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    Files,
    FilesInTab,
    Nothing,
}

#[derive(Default)]
pub struct PhoneState {
    /// `photos_new().len()` for the views, keyed by (photo list address, length, imported
    /// count) so a frame doesn't rebuild an import key per photo.
    pub new_count: std::cell::Cell<Option<NewCount>>,
    /// Photo id → its local path (the reverse of `remote_photos`), for arriving thumbnails.
    pub remote_ids: HashMap<u64, PathBuf>,
    pub active: DeviceState,
    pub active_id: String,
    pub states: HashMap<String, DeviceState>,
    pub import_pending: HashMap<PathBuf, String>,
    pub clip_sensitive: bool,

    pub last_image: Vec<u8>,
    pub started: bool,
    pub error: Option<String>,
    pub port: u16,
    /// Connected now (paired or not), by id.
    pub devices: HashMap<String, Device>,
    pub trusted: Vec<Trusted>,
    /// The phone the hub shows.
    pub current: Option<String>,
    /// id → (level, charging).
    pub battery: HashMap<String, (i32, bool)>,
    pub low_warned: HashSet<String>,
    pub seen: HashMap<String, i64>,
    pub dialog: Option<PairDialog>,
    /// (device, code, the phone asked).
    pub code: Option<(String, String, bool)>,
    /// Newest message per conversation.
    /// Free and total bytes on the phone, read in the background after mounting.
    /// The newest camera photo, drawn on the phone's screen.
    pub imported: HashSet<String>,
    /// Offers waiting for an answer (auto-accept off): transfer → (device, name, size).
    pub offers: Vec<(u64, String, String, u64)>,
    /// Shared clipboard history: (from the phone, text, when).
    pub clip: VecDeque<(bool, String, SystemTime)>,
    /// What we last put on, or sent from, the clipboard (no echo).
    pub clip_last: String,
    pub mute_draft: String,
    /// Notifications shown in full (by key); others show their first lines.
    pub expanded: HashSet<String>,
    /// The automatic-clipboard setup form on the hub is open.
    pub clipboard_setup: bool,
    pub ufw: Option<bool>,
}

#[derive(Default)]
pub struct DeviceState {
    pub payloads: HashMap<u64, String>,
    pub backup_requests: HashMap<u64, String>,
    pub index_scan: Option<crate::connect::PhoneIndexScan>,
    pub index_at: Option<Instant>,
    pub moved: Vec<Moved>,
    pub players: HashMap<String, ef_phone::Value>,
    pub signal: String,
    pub generation: u64,
    pub call: Option<ef_phone::Value>,
    pub extra_address: String,
    pub extra_code: String,
    pub extra_connect: String,
    pub extra_status: String,
    pub qr: Option<iced::widget::svg::Handle>,
    pub clipboard_qr: Option<iced::widget::svg::Handle>,
    pub captures: HashMap<String, PathBuf>,
    pub last_theme: String,
    pub dnd: bool,
    pub away_since: Option<Instant>,
    pub contacts: HashMap<String, String>,
    pub remote_photos: HashMap<PathBuf, u64>,
    pub remote_thumb: HashMap<u64, PathBuf>,
    pub thumb_wait: HashMap<u64, (PathBuf, Instant)>,
    pub photo_before: Option<(i64, u64)>,
    pub photo_more: bool,
    pub copy_photos: Option<(usize, Vec<PathBuf>)>,
    pub photo_requests: HashMap<String, String>,
    pub ringing: Option<Instant>,
    pub notifications: Vec<Notification>,
    pub reply: HashMap<String, String>,
    pub threads: HashMap<i64, Sms>,
    pub thread_msgs: HashMap<i64, Vec<Sms>>,
    pub open_thread: Option<i64>,
    pub compose: String,
    pub sftp: Option<Sftp>,
    pub sftp_error: Option<String>,
    pub mount: Option<(Arc<PhoneFs>, PathBuf)>,
    pub space: Option<(u64, u64)>,
    pub mounting: bool,
    pub want: Option<Want>,
    pub browse_after_mount: Option<(PathBuf, Option<std::ffi::OsString>)>,
    pub photos: Option<Result<Vec<Photo>, String>>,
    pub photos_loading: bool,
    pub photo_thumbs: HashMap<PathBuf, Option<iced::advanced::image::Handle>>,
    pub photo_queue: VecDeque<(PathBuf, i64)>,
    pub photo_jobs: usize,
    pub photo_sel: HashSet<PathBuf>,
    pub photo_filter: Option<Source>,
    pub photo_limit: usize,
    pub screen: Option<(PathBuf, String)>,
}

impl std::ops::Deref for PhoneState {
    type Target = DeviceState;
    fn deref(&self) -> &DeviceState {
        &self.active
    }
}
impl std::ops::DerefMut for PhoneState {
    fn deref_mut(&mut self) -> &mut DeviceState {
        &mut self.active
    }
}
// ------------------------------------------------------------------------------ service plumbing

#[derive(Debug, Clone)]
pub enum PhoneMsg {
    Scoped(u64, Box<PhoneMsg>),
    Extra(String),
    LocalDnd(bool),
    RemoteImage(Vec<u8>),
    ExtraField(u8, String),
    ExtraDone(Result<String, String>),
    NotificationAction(String, String),
    Attachment(ef_phone::Value),
    Event(Event),
    LocalClip(String),
    LocalImage(Vec<u8>),
    Open(PhonePage),
    Select(String),
    Dialog(bool),
    Pair(String),
    AcceptPair,
    RejectPair,
    Forget(String),
    IpDraft(String),
    IpSubmit,
    AllowFirewall,
    Firewall(Result<(), String>),
    Ufw(bool),
    Files(Want),
    SendFiles,
    SendPaths(Vec<PathBuf>),
    Picked(Result<Vec<PathBuf>, String>),
    Ring,
    Mounted(Result<(Arc<PhoneFs>, PathBuf), String>),
    ImportReady(Result<(Vec<PathBuf>, PathBuf), String>),
    SaveDirReady(Result<PathBuf, String>),
    Space(Option<(u64, u64)>),
    /// Ask for the photo list again after it failed.
    RetryPhotos,
    Screen(PathBuf, Option<String>),
    PhotoToggle(PathBuf),
    PhotoOpen(PathBuf),
    PhotosClear,
    PhotosFilter(Option<Source>),
    PhotosMore,
    PhotoQueue,
    PhotosCopy,
    PhotosSaveTo,
    SaveFolder(Result<Option<PathBuf>, String>),
    Import,
    Thread(i64),
    Compose(String),
    SendSms,
    RefreshMessages,
    Dismiss(String),
    /// Show a long notification in full, or clamp it again.
    ToggleNotification(String),
    /// Show or hide the automatic-clipboard setup form.
    ClipboardSetup(bool),
    DismissAll,
    ReplyDraft(String, String),
    Reply(String),
    ClipCopy(String),
    AcceptOffer(u64),
    DeclineOffer(u64),
    OpenFile(PathBuf),
    OpenSaveFolder,
    // settings
    SetFiles(bool),
    SetClipboard(bool),
    SetMessages(bool),
    SetNotifications(PhoneNotifications),
    SetBatteryWarning(bool),
    SetAutoAccept(bool),
    SetPicture(config::PhonePicture),
    ChangeSaveTo,
    SaveTo(PathBuf),
    MuteDraft(String),
    MuteAdd,
    MuteRemove(usize),
}

static SERVICE: OnceLock<Service> = OnceLock::new();

/// The mounted phone, for thumbnail jobs off the UI thread: its id, where its files are,
/// and whether its EchoConnect makes thumbnails by path.
static THUMB_TARGET: Mutex<Option<(String, PathBuf, bool)>> = Mutex::new(None);
/// Thumbnail requests waiting for the phone: request id → where to hand the picture file.
static THUMB_WAIT: OnceLock<Mutex<HashMap<String, std::sync::mpsc::SyncSender<Result<PathBuf, String>>>>> = OnceLock::new();
/// Incoming thumbnail transfers → their request ids.
static THUMB_TRANSFERS: OnceLock<Mutex<HashMap<u64, String>>> = OnceLock::new();
static THUMB_NEXT: AtomicU64 = AtomicU64::new(1);

fn thumb_wait() -> &'static Mutex<HashMap<String, std::sync::mpsc::SyncSender<Result<PathBuf, String>>>> {
    THUMB_WAIT.get_or_init(Default::default)
}

fn thumb_transfers() -> &'static Mutex<HashMap<u64, String>> {
    THUMB_TRANSFERS.get_or_init(Default::default)
}

pub(crate) fn set_thumb_target(target: Option<(String, PathBuf, bool)>) {
    *THUMB_TARGET.lock().unwrap() = target;
}

/// A thumbnail for `p` if it's on the mounted phone (`None` when it isn't): EchoConnect's
/// own small picture when it makes them, else the camera preview from the file's first
/// bytes — never the whole photo over Wi-Fi.
pub(crate) fn phone_thumb(p: &Path, cancel: &std::sync::atomic::AtomicBool) -> Option<Option<Pixels>> {
    let (id, root, by_path) = THUMB_TARGET.lock().unwrap().clone()?;
    let rel = p.strip_prefix(&root).ok()?.to_string_lossy().into_owned();
    if by_path && let Some(px) = ask_thumb(&id, &rel, cancel) {
        return Some(Some(px));
    }
    Some(crate::thumbs::head_preview(p))
}

/// Ask EchoConnect for `rel`'s thumbnail and wait (up to 15 s, or until cancelled).
fn ask_thumb(id: &str, rel: &str, cancel: &std::sync::atomic::AtomicBool) -> Option<Pixels> {
    let request = format!("thumb-{}", THUMB_NEXT.fetch_add(1, Ordering::Relaxed));
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    thumb_wait().lock().unwrap().insert(request.clone(), tx);
    let sent = service().is_some_and(|s| s.send_packet(id, "echofiles.files.thumb", ef_phone::json!({"path": rel, "requestId": request})));
    let deadline = Instant::now() + Duration::from_secs(15);
    let got = sent
        .then(|| {
            loop {
                if cancel.load(Ordering::Relaxed) {
                    break None;
                }
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(r) => break r.ok(),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                    Err(_) => break None,
                }
            }
        })
        .flatten();
    thumb_wait().lock().unwrap().remove(&request);
    let file = got?;
    let px = crate::thumbs::small_image(&file);
    let _ = std::fs::remove_file(&file);
    px
}

/// Whether a thumbnail request is still waiting (so its picture is worth taking).
pub(crate) fn thumb_wanted(request: &str) -> bool {
    thumb_wait().lock().unwrap().contains_key(request)
}

/// The picture (or the phone's error) for a thumbnail request.
pub(crate) fn thumb_answer(request: &str, result: Result<PathBuf, String>) {
    match thumb_wait().lock().unwrap().remove(request) {
        Some(tx) => {
            let _ = tx.try_send(result);
        }
        // Nobody waits any more (scrolled away): don't keep the file.
        None => {
            if let Ok(f) = result {
                let _ = std::fs::remove_file(f);
            }
        }
    }
}
static EVENTS: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<PhoneMsg>>>> = OnceLock::new();
static EVENT_TX: OnceLock<mpsc::UnboundedSender<PhoneMsg>> = OnceLock::new();

fn event_tx() -> mpsc::UnboundedSender<PhoneMsg> {
    EVENT_TX
        .get_or_init(|| {
            let (tx, rx) = mpsc::unbounded();
            let _ = EVENTS.set(Mutex::new(Some(rx)));
            tx
        })
        .clone()
}

pub fn service() -> Option<&'static Service> {
    SERVICE.get()
}

/// Start talking to phones (once), and watch the clipboard for the shared clipboard.
pub fn start() {
    static STARTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if service().is_some() || STARTING.swap(true, Ordering::AcqRel) {
        return;
    }
    let tx = event_tx();
    std::thread::spawn(move || {
        let t2 = tx.clone();
        // On failure `Event::Failed` has already said why.
        match Service::start(move |e| {
            let _ = t2.unbounded_send(PhoneMsg::Event(e));
        }) {
            Ok(s) => {
                crate::connect::bluetooth(s.clone());
                let _ = SERVICE.set(s);
            }
            Err(e) => {
                let _ = tx.unbounded_send(PhoneMsg::Event(Event::Failed(e.to_string())));
            }
        }
        STARTING.store(false, Ordering::Release);
    });
    static WATCHERS: std::sync::Once = std::sync::Once::new();
    WATCHERS.call_once(|| {
        let tx = event_tx();
        std::thread::spawn(move || watch_clipboard(tx));
        std::thread::spawn(watch_images);
        std::thread::spawn(|| {
            let mut previous = None;
            loop {
                if let Some(state) = crate::connect::dnd_state()
                    && previous != Some(state)
                {
                    previous = Some(state);
                    if event_tx().unbounded_send(PhoneMsg::LocalDnd(state)).is_err() {
                        break;
                    }
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });
    });
}

/// `wl-paste --watch` runs a command on every clipboard change; it prints the text as one
/// base64 line so newlines survive.
fn watch_clipboard(tx: mpsc::UnboundedSender<PhoneMsg>) {
    use std::os::unix::process::CommandExt;
    let mut cmd = std::process::Command::new("wl-paste");
    cmd.args(["--no-newline", "--type", "text", "--watch", "sh", "-c", "base64 -w0; echo"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    // The watcher ends with EchoFiles (it used to outlive it, one per launch).
    // SAFETY: prctl is async-signal-safe; nothing else runs between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
    let Ok(mut child) = cmd.spawn() else {
        return;
    };
    let Some(out) = child.stdout.take() else { return };
    for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
        if let Ok(bytes) = crate::codec::base64_decode(line.trim()).ok_or(())
            && let Ok(text) = String::from_utf8(bytes)
            && !text.is_empty()
            && text.len() < 1 << 20
        {
            let _ = tx.unbounded_send(PhoneMsg::LocalClip(text));
        }
    }
    let _ = child.wait();
}

fn watch_images() {
    use std::os::unix::process::CommandExt;
    let mut command = std::process::Command::new("wl-paste");
    command
        .args(["--type", "image/png", "--watch", "sh", "-c", "head -c 16777217 | base64 -w0; echo"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    unsafe {
        command.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
    if let Ok(mut child) = command.spawn() {
        if let Some(out) = child.stdout.take() {
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                if let Ok(bytes) = crate::codec::base64_decode(line.trim()).ok_or(())
                    && !bytes.is_empty()
                    && bytes.len() <= 16 * 1024 * 1024
                {
                    let _ = event_tx().unbounded_send(PhoneMsg::LocalImage(bytes));
                }
            }
        }
        let _ = child.wait();
    }
}

fn normal_image(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() > 16 * 1024 * 1024 {
        return None;
    }
    let image = decode_limited(bytes)?;
    let mut output = std::io::Cursor::new(Vec::new());
    image.write_to(&mut output, image::ImageFormat::Png).ok()?;
    let png = output.into_inner();
    (png.len() <= 16 * 1024 * 1024).then_some(png)
}

fn set_image(bytes: &[u8]) {
    use std::io::Write;
    if decode_limited(bytes).is_none() {
        return;
    }
    if let Ok(mut child) = std::process::Command::new("wl-copy").args(["--type", "image/png"]).stdin(std::process::Stdio::piped()).spawn() {
        if let Some(mut input) = child.stdin.take() {
            let _ = input.write_all(bytes);
        }
        let _ = child.wait();
    }
}

fn set_clipboard(text: &str) {
    use std::io::Write;
    if let Ok(mut c) = std::process::Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut i) = c.stdin.take() {
            let _ = i.write_all(text.as_bytes());
        }
        let _ = c.wait();
    }
}

/// Phone events and clipboard changes, as a subscription stream (taken once).
pub fn events() -> impl iced::futures::Stream<Item = PhoneMsg> {
    let _ = event_tx();
    let rx = EVENTS.get().and_then(|m| m.lock().unwrap().take());
    iced::futures::stream::unfold(rx, |rx| async move {
        use iced::futures::StreamExt;
        let mut rx = rx?;
        let item = rx.next().await?;
        Some((item, Some(rx)))
    })
}

/// Where a phone's files appear: `$XDG_RUNTIME_DIR/echofiles/<phone name>` — private to
/// the user and gone at logout. The name is the one the phone gives itself, so paths read
/// like the sidebar; an unusable name falls back to the device id.
fn mount_point(name: &str, id: &str) -> PathBuf {
    let clean: String = name.chars().map(|c| if c == '/' || c.is_control() { '-' } else { c }).collect();
    let clean = clean.trim().trim_start_matches('.');
    config::runtime_dir().join(if clean.is_empty() { id } else { clean })
}

fn seen_file() -> PathBuf {
    config::state_dir().join("phone-seen")
}

fn imported_file() -> PathBuf {
    config::state_dir().join("phone-imported")
}

fn now_s() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn load_state() -> PhoneState {
    let seen = std::fs::read_to_string(seen_file())
        .map(|t| t.lines().filter_map(|l| l.split_once('\t')).filter_map(|(id, s)| Some((id.to_string(), s.parse().ok()?))).collect())
        .unwrap_or_default();
    let imported = std::fs::read_to_string(imported_file()).map(|t| t.lines().map(str::to_string).collect()).unwrap_or_default();
    PhoneState {
        trusted: ef_phone::trusted(),
        seen,
        imported,
        active: DeviceState { photo_limit: PHOTOS_PAGE, ..Default::default() },
        ..Default::default()
    }
}

fn save_lines(file: PathBuf, text: String) {
    config::queue_state(file, text.into_bytes());
}

/// "2 h ago", "yesterday", "3 Oct".
pub fn ago(secs: i64) -> String {
    let d = (now_s() - secs).max(0);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86_399 => format!("{} h ago", d / 3600),
        86_400..=172_799 => "yesterday".into(),
        _ => format!("{} days ago", d / 86_400),
    }
}

mod events;
mod photos;
mod update;
use photos::*;

// ------------------------------------------------------------------------------ portal

/// The desktop's own file chooser (xdg-desktop-portal), for picking files to send or a
/// folder to save into.
fn portal_pick(title: &str, folder: bool) -> Result<Vec<PathBuf>, String> {
    use zbus::zvariant::{OwnedValue, Value};
    let conn = zbus::blocking::Connection::session().map_err(|e| e.to_string())?;
    let mut raw = [0u8; 6];
    let _ = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut raw));
    let token = format!("echofiles{}", raw.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let sender = conn.unique_name().map(|n| n.trim_start_matches(':').replace('.', "_")).unwrap_or_default();
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let request = zbus::blocking::Proxy::new(&conn, "org.freedesktop.portal.Desktop", path.as_str(), "org.freedesktop.portal.Request")
        .map_err(|e| e.to_string())?;
    let mut responses = request.receive_signal("Response").map_err(|e| e.to_string())?;
    let chooser = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.FileChooser",
    )
    .map_err(|e| e.to_string())?;
    let mut opts: HashMap<&str, Value<'_>> = HashMap::new();
    opts.insert("handle_token", Value::from(token.as_str()));
    opts.insert("multiple", Value::from(!folder));
    opts.insert("directory", Value::from(folder));
    let _: zbus::zvariant::OwnedObjectPath =
        chooser.call("OpenFile", &("", title, opts)).map_err(|e| format!("The file chooser isn't available: {e}"))?;
    let msg = responses.next().ok_or("The file chooser closed.")?;
    let (code, results): (u32, HashMap<String, OwnedValue>) = msg.body().deserialize().map_err(|e| e.to_string())?;
    if code != 0 {
        return Ok(Vec::new());
    }
    let uris: Vec<String> = results.get("uris").and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok()).unwrap_or_default();
    Ok(uris.iter().filter_map(|u| file_uri_path(u)).collect())
}

fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?, 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(out)))
}

fn ufw_active() -> bool {
    std::process::Command::new("systemctl").args(["is-active", "--quiet", "ufw"]).status().is_ok_and(|s| s.success())
}

fn allow_firewall() -> Result<(), String> {
    let out = std::process::Command::new("pkexec")
        .args([
            "sh",
            "-c",
            "ufw allow 1714:1764/udp comment 'EchoFiles phone link' && ufw allow 1714:1764/tcp comment 'EchoFiles phone link'",
        ])
        .output()
        .map_err(|e| format!("Couldn't ask for your password: {e}"))?;
    if out.status.success() {
        Ok(())
    } else if out.status.code() == Some(126) || out.status.code() == Some(127) {
        Err("Cancelled.".into())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn notify_desktop(phone: &str, n: &Notification) {
    let title = if n.title.is_empty() { n.app.clone() } else { format!("{} · {}", n.app, n.title) };
    let _ = crate::system::spawn_detached(
        std::process::Command::new("notify-send")
            .args(["--app-name", phone, "--icon", "phone", &title, &n.text])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null()),
    );
}

// ------------------------------------------------------------------------------ app side

impl App {
    /// The phone the hub is about: the chosen one, else a connected paired one, else the
    /// first paired one.
    pub(crate) fn phone_id(&self) -> Option<String> {
        let ph = &self.phone;
        if let Some(c) = &ph.current
            && ph.trusted.iter().any(|t| &t.id == c)
        {
            return Some(c.clone());
        }
        ph.trusted.iter().find(|t| ph.devices.get(&t.id).is_some_and(|d| d.paired)).or(ph.trusted.first()).map(|t| t.id.clone())
    }

    pub(crate) fn phone_name(&self, id: &str) -> String {
        self.phone
            .devices
            .get(id)
            .map(|d| d.name.clone())
            .or_else(|| self.phone.trusted.iter().find(|t| t.id == id).map(|t| t.name.clone()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Phone".into())
    }

    /// Paired and connected right now.
    pub(crate) fn phone_online(&self, id: &str) -> bool {
        self.phone.devices.get(id).is_some_and(|d| d.paired)
    }

    /// The phone's files are mounted at this root.
    pub(crate) fn phone_mount_root(&self) -> Option<&Path> {
        self.phone.mount.as_ref().map(|(m, _)| m.root())
    }

    /// The phone's storage folder (inside the mount).
    pub(crate) fn phone_storage(&self) -> Option<&Path> {
        self.phone.mount.as_ref().map(|(_, s)| s.as_path())
    }

    pub(crate) fn is_phone_path(&self, p: &Path) -> bool {
        self.phone_mount_root().is_some_and(|r| p.starts_with(r))
    }

    fn phone_seen(&mut self, id: &str) {
        self.phone.seen.insert(id.to_string(), now_s());
        let text: String = self.phone.seen.iter().map(|(k, v)| format!("{k}\t{v}\n")).collect();
        save_lines(seen_file(), text);
    }

    /// Show a phone page in the active pane.
    /// Open a phone page, at its top: the pages share one scrolling area, so without this
    /// Photos opened as far down as the hub had been scrolled.
    pub(crate) fn open_phone_page(&mut self, page: PhonePage) -> Task<Message> {
        let changed = self.pane().phone != Some(page);
        let task = self.open_phone_page_inner(page);
        if changed {
            Task::batch([iced::widget::operation::snap_to(PAGE_SCROLL, iced::widget::operation::RelativeOffset::START), task])
        } else {
            task
        }
    }

    fn open_phone_page_inner(&mut self, page: PhonePage) -> Task<Message> {
        self.mode = crate::app::Mode::Files;
        let pane = self.pane_mut();
        if pane.phone != Some(page) {
            if !pane.special() {
                let loc = pane.location.clone();
                pane.back.push(loc);
                pane.forward.clear();
            }
            pane.drives = false;
            pane.shares = None;
            pane.phone = Some(page);
            pane.clear_search();
        }
        let Some(id) = self.phone_id() else { return Task::none() };
        let online = self.phone_online(&id);
        let svc = service();
        match page {
            PhonePage::Photos if self.settings.phone.files => self.request_photos(),
            PhonePage::Messages if online && self.settings.phone.messages => {
                if let Some(s) = svc {
                    s.request_conversations(&id);
                    if let Some(t) = self.phone.open_thread {
                        s.request_thread(&id, t);
                    }
                }
            }
            PhonePage::Notifications if online => {
                if let Some(s) = svc {
                    s.request_notifications(&id);
                }
            }
            PhonePage::Hub if online => {
                if let Some(s) = svc {
                    s.request_battery(&id);
                }
            }
            _ => {}
        }
        Task::none()
    }

    /// Ask EchoConnect for its photo list (newest first), unless it's here or on its way.
    pub(crate) fn request_photos(&mut self) {
        if self.phone.photos.is_some() || self.phone.photos_loading || !self.settings.phone.files {
            return;
        }
        if let (Some(s), Some(id)) = (service(), self.phone_id())
            && s.send_packet(&id, "echofiles.photos.request", ef_phone::json!({}))
        {
            self.phone.photos_loading = true;
        }
    }

    /// The photo the drawn phone shows as its lock screen: the newest camera photo.
    fn screen_photo(&self) -> Option<(PathBuf, i64)> {
        let Some(Ok(list)) = &self.phone.photos else { return None };
        list.iter()
            .find(|p| p.source == Source::Camera && !p.video)
            .or_else(|| list.iter().find(|p| !p.video))
            .map(|p| (p.path.clone(), p.mtime))
    }

    /// Make the lock-screen picture from the newest photo's thumbnail, fetching that
    /// thumbnail first if it isn't here yet.
    pub(crate) fn want_screen(&mut self) -> Task<Message> {
        if self.settings.phone.picture != config::PhonePicture::Photo {
            return Task::none();
        }
        let Some((path, mtime)) = self.screen_photo() else { return Task::none() };
        if self.phone.screen.as_ref().is_some_and(|(p, _)| p == &path) {
            return Task::none();
        }
        match self.phone.photo_thumbs.get(&path) {
            Some(Some(iced::advanced::image::Handle::Path(_, file))) => self.screen_from(path, file.clone(), mtime),
            Some(_) => Task::none(),
            None => {
                if !self.phone.thumb_wait.values().any(|(p, _)| p == &path) && !self.phone.photo_queue.iter().any(|(p, _)| p == &path) {
                    self.phone.photo_queue.push_front((path, mtime));
                }
                self.pump_photo_thumbs()
            }
        }
    }

    /// Size the thumbnail `file` of `photo` for the drawn phone's screen.
    pub(crate) fn screen_from(&self, photo: PathBuf, file: PathBuf, mtime: i64) -> Task<Message> {
        let generation = self.phone.generation;
        background(move || screen_image(&file, mtime), move |b| scoped(generation, PhoneMsg::Screen(photo.clone(), b)))
    }

    /// The newest photo's thumbnail arrived: it becomes the lock screen.
    pub(crate) fn thumb_arrived(&self, photo: &Path, file: &Path) -> Task<Message> {
        match self.screen_photo() {
            Some((p, mtime)) if p == photo && self.settings.phone.picture == config::PhonePicture::Photo => {
                self.screen_from(p, file.to_path_buf(), mtime)
            }
            _ => Task::none(),
        }
    }

    /// Get the phone's files mounted (asking it to start its SFTP server first), then `want`.
    pub(crate) fn phone_files(&mut self, want: Want) -> Task<Message> {
        if !self.settings.phone.files {
            return Task::none();
        }
        if let Some((_, storage)) = self.phone.mount.clone() {
            return self.after_mount(want, &storage);
        }
        self.phone.want = Some(match (self.phone.want, want) {
            (Some(w), Want::Nothing) => w,
            _ => want,
        });
        let Some(id) = self.phone_id() else { return Task::none() };
        if self.phone.mounting {
            return Task::none();
        }
        if let Some(s) = self.phone.sftp.clone() {
            return self.mount_phone(s);
        }
        if let Some(svc) = service()
            && svc.request_sftp(&id)
        {
            self.phone.mounting = true;
            self.phone.sftp_error = None;
        }
        Task::none()
    }

    fn mount_phone(&mut self, s: Sftp) -> Task<Message> {
        let generation = self.phone.generation;
        self.phone.mounting = true;
        let remote = ef_phonefs::Remote {
            ip: s.ip,
            port: s.port,
            user: s.user,
            password: s.password,
            fingerprint: s.host_key_fingerprint,
            base: s.path,
        };
        let at = self.phone_id().map(|id| mount_point(&self.phone_name(&id), &id));
        background(
            move || {
                let at = at.ok_or("No phone is selected.")?;
                ef_phonefs::mount(&remote, &at).map(|fs| {
                    let root = fs.root().to_path_buf();
                    (Arc::new(fs), root)
                })
            },
            move |r| scoped(generation, PhoneMsg::Mounted(r)),
        )
    }

    pub(crate) fn open_phone_result(&mut self, uri: &Path, directory: bool, reveal: bool) -> Task<Message> {
        let text = uri.to_string_lossy();
        let Some((id, path)) = text.strip_prefix("phone://").and_then(|s| s.split_once('/')) else { return Task::none() };
        if !self.phone_online(id) {
            self.toast_error("Phone isn't nearby".into(), "Reconnect the phone to open this cached result.".into());
            return Task::none();
        }
        if Path::new(path).is_absolute() || Path::new(path).components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Task::none();
        }
        self.phone.current = Some(id.into());
        self.activate_phone(id);
        if directory || reveal {
            let p = Path::new(path);
            self.phone.browse_after_mount = Some(if reveal {
                (p.parent().unwrap_or(Path::new("")).into(), p.file_name().map(|n| n.to_os_string()))
            } else {
                (p.into(), None)
            });
            return self.phone_files(Want::Files);
        }
        if let Some(s) = service() {
            let request = format!("file-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos());
            self.phone.captures.insert(request.clone(), config::state_dir().join("phone-open"));
            self.phone.photo_requests.insert(request.clone(), "photo-open".into());
            s.send_packet(id, "echofiles.files.read", ef_phone::json!({"path":path,"requestId":request}));
        }
        Task::none()
    }

    fn after_mount(&mut self, want: Want, storage: &Path) -> Task<Message> {
        self.phone.want = None;
        let mut tasks = Vec::new();
        match want {
            Want::Files => {
                let (path, reveal) = self.phone.browse_after_mount.take().unwrap_or((PathBuf::new(), None));
                tasks.push(self.go(storage.join(path), true));
                if let Some(name) = reveal {
                    self.pane_mut().reveal = vec![name];
                }
            }
            Want::FilesInTab => tasks.push(self.update(Message::OpenTab(storage.to_path_buf()))),
            Want::Nothing => {}
        }
        Task::batch(tasks)
    }

    /// The phone went away or was forgotten: close its folders and its mount.
    fn drop_phone_mount(&mut self) -> Task<Message> {
        self.phone.generation = crate::pane::next_id();
        self.phone.want = None;
        self.phone.photos = None;
        self.phone.photos_loading = false;
        self.phone.screen = None;
        self.phone.photo_queue.clear();
        self.phone.photo_thumbs.clear();
        self.phone.photo_jobs = 0;
        self.phone.sftp = None;
        self.phone.mounting = false;
        self.phone.space = None;
        set_thumb_target(None);
        let Some((m, _)) = self.phone.mount.take() else { return Task::none() };
        let root = m.root().to_path_buf();
        let home = config::home();
        let hidden = self.show_hidden;
        let mut tasks = Vec::new();
        for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()) {
            pane.back.retain(|p| !p.starts_with(&root));
            pane.forward.retain(|p| !p.starts_with(&root));
            if !pane.special() && pane.location.starts_with(&root) {
                pane.location = home.clone();
                tasks.push(pane.load(home.clone(), hidden));
            }
        }
        std::thread::spawn(move || m.unmount());
        Task::batch(tasks)
    }

    /// Ask EchoConnect for queued thumbnails, a few at a time; they arrive as transfers.
    fn pump_photo_thumbs(&mut self) -> Task<Message> {
        while self.phone.photo_jobs < PHOTO_JOBS {
            let Some((p, _)) = self.phone.photo_queue.pop_front() else { break };
            if self.phone.photo_thumbs.contains_key(&p) {
                continue;
            }
            self.phone.photo_jobs += 1;
            if let Some(photo_id) = self.phone.remote_photos.get(&p).copied() {
                if let (Some(s), Some(id)) = (service(), self.phone_id()) {
                    if s.send_packet(&id, "echofiles.photos.thumb.request", ef_phone::json!({"id":photo_id})) {
                        self.phone.thumb_wait.insert(photo_id, (p.clone(), Instant::now()));
                    } else {
                        self.phone.photo_jobs = self.phone.photo_jobs.saturating_sub(1);
                        self.phone.photo_thumbs.insert(p.clone(), None);
                    }
                }
                continue;
            }
            // Not one of EchoConnect's photos: nothing to ask for.
            self.phone.photo_jobs = self.phone.photo_jobs.saturating_sub(1);
            self.phone.photo_thumbs.insert(p, None);
        }
        Task::none()
    }

    /// Queue thumbnails for what the Photos page shows.
    pub(crate) fn want_photo_thumbs(&mut self) -> Task<Message> {
        let Some(Ok(list)) = &self.phone.photos else { return Task::none() };
        let filter = self.phone.photo_filter;
        let items: Vec<(PathBuf, i64)> = list
            .iter()
            .filter(|p| filter.is_none_or(|f| p.source == f))
            .take(self.phone.photo_limit)
            .filter(|p| {
                !self.phone.photo_thumbs.contains_key(&p.path)
                    && !self.phone.thumb_wait.values().any(|(path, _)| path == &p.path)
                    && !self.phone.remote_thumb.values().any(|path| path == &p.path)
            })
            .map(|p| (p.path.clone(), p.mtime))
            .collect();
        self.phone.photo_queue = items.into();
        self.pump_photo_thumbs()
    }

    /// How many photos `photos_new` would return, cached between frames.
    pub(crate) fn photos_new_count(&self) -> usize {
        let Some(Ok(list)) = &self.phone.photos else { return 0 };
        let stamp = (list.as_ptr() as usize, list.len(), self.phone.imported.len());
        if let Some((s, n)) = self.phone.new_count.get()
            && s == stamp
        {
            return n;
        }
        let n = self.photos_new().len();
        self.phone.new_count.set(Some((stamp, n)));
        n
    }

    pub(crate) fn photos_new(&self) -> Vec<&Photo> {
        match &self.phone.photos {
            Some(Ok(list)) => list
                .iter()
                .filter(|p| {
                    p.source == Source::Camera
                        && !self.phone.imported.contains(&import_key(&self.phone_id().unwrap_or_default(), self.phone_storage(), p))
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn request_remote_photo(&mut self, photo: u64, destination: PathBuf, action: &str) {
        if let (Some(s), Some(id)) = (service(), self.phone_id()) {
            let request =
                format!("photo-{photo}-{}", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos());
            self.phone.captures.insert(request.clone(), destination);
            self.phone.photo_requests.insert(request.clone(), action.into());
            if !s.send_packet(&id, "echofiles.photos.read", ef_phone::json!({"id":photo,"requestId":request})) {
                self.phone.extra_status = "Phone isn't nearby. Reconnect to download photos.".into();
            }
        }
    }

    fn send_to_phone(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        let Some(id) = self.phone_id() else { return Task::none() };
        let files: Vec<PathBuf> = paths.into_iter().filter(|p| p.is_file()).collect();
        if files.is_empty() {
            self.toast_error("Nothing to send".into(), "Only files can be sent to the phone, not folders.".into());
            return Task::none();
        }
        if !self.phone_online(&id) {
            self.toast_error(format!("{} isn't nearby", self.phone_name(&id)), "Open EchoConnect on the phone, on the same Wi-Fi.".into());
            return Task::none();
        }
        if let Some(s) = service() {
            s.send_files(&id, files);
        }
        Task::none()
    }

    fn start_phone_index(&mut self, id: &str) {
        let mut scan = crate::connect::PhoneIndexScan::new(id.into());
        if let (Some(s), Some(body)) = (service(), scan.next()) {
            s.send_packet(id, "echofiles.files.list", body);
        }
        self.phone.index_scan = Some(scan);
        self.phone.index_at = Some(Instant::now());
        self.phone.extra_status = "Refreshing phone file names…".into();
    }

    fn send_phone_settings(&self) {
        if let (Some(s), Some(id)) = (service(), self.phone_id()) {
            let p = &self.settings.phone;
            s.send_packet(&id, "echofiles.settings", ef_phone::json!({"clipboard":p.clipboard,"files":p.files,"sms":p.messages,"notifications":p.notifications != PhoneNotifications::Off,"autolock":p.autolock,"backup":p.backup,"dnd":p.dnd_sync}));
        }
    }

    /// Expire the ringing state; refresh discovery while the Connect dialog is open.
    pub(crate) fn phone_tick(&mut self) {
        let stale: Vec<_> =
            self.phone.thumb_wait.iter().filter(|(_, (_, at))| at.elapsed() > Duration::from_secs(30)).map(|(id, _)| *id).collect();
        for id in &stale {
            if let Some((path, _)) = self.phone.thumb_wait.remove(id) {
                self.phone.photo_jobs = self.phone.photo_jobs.saturating_sub(1);
                self.phone.photo_thumbs.insert(path, None);
            }
        }
        if !stale.is_empty() {
            let _ = event_tx().unbounded_send(PhoneMsg::PhotoQueue);
        }
        if self.phone.index_scan.is_some() && self.phone.index_at.is_some_and(|t| t.elapsed() > Duration::from_secs(30)) {
            self.phone.index_scan = None;
            self.phone.extra_status = "Phone index refresh interrupted; previous snapshot kept. Toggle Index phone files to retry.".into();
        }
        if self.settings.phone.autolock
            && self.phone.away_since.is_some_and(|t| t.elapsed() > Duration::from_secs(30))
            && !self.phone.devices.values().any(|d| d.paired)
        {
            crate::connect::lock();
            self.phone.away_since = None;
        }
        if let Some(s) = service() {
            if self.phone.qr.is_none() {
                self.phone.qr = crate::connect::pairing_svg(s).map(iced::widget::svg::Handle::from_memory);
            }
            let theme = crate::connect::theme(&self.palette);
            let fingerprint = theme.to_string();
            if self.phone.last_theme != fingerprint {
                self.phone.last_theme = fingerprint;
                for d in self.phone.devices.values().filter(|d| d.paired) {
                    s.send_packet(&d.id, "echofiles.theme", theme.clone());
                }
            }
        }
        if self.phone.ringing.is_some_and(|t| t.elapsed() > Duration::from_secs(30)) {
            self.phone.ringing = None;
        }
        if let Some(d) = &self.phone.dialog
            && d.opened.elapsed().as_secs() % 5 == 0
            && let Some(s) = service()
        {
            s.announce();
        }
    }

    /// Anything phone-related moving on screen (transfer bars).
    pub(crate) fn phone_animating(&self) -> bool {
        self.phone.moved.iter().any(|m| m.result.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months() {
        assert_eq!(month_of(0), "1970-01");
        assert_eq!(month_of(1_759_400_000), "2025-10");
        assert_eq!(month_of(951_782_400), "2000-02"); // 29 Feb 2000
    }

    #[test]
    fn uris() {
        assert_eq!(file_uri_path("file:///home/a/My%20File.txt"), Some(PathBuf::from("/home/a/My File.txt")));
        assert_eq!(file_uri_path("file:///x%"), Some(PathBuf::from("/x%")));
        assert_eq!(file_uri_path("http://x"), None);
    }

    #[test]
    fn exif_parse() {
        // Minimal JPEG: SOI, APP1 Exif with IFD0 (orientation 6) → IFD1 pointing at a tiny "JPEG".
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II*\0");
        tiff.extend_from_slice(&8u32.to_le_bytes());
        // IFD0 at 8: one entry (orientation), next IFD at 26
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&6u16.to_le_bytes());
        tiff.extend_from_slice(&0u16.to_le_bytes());
        tiff.extend_from_slice(&26u32.to_le_bytes());
        // IFD1 at 26: two entries, offset 56, length 4
        tiff.extend_from_slice(&2u16.to_le_bytes());
        for (tag, val) in [(0x0201u16, 56u32), (0x0202, 4)] {
            tiff.extend_from_slice(&tag.to_le_bytes());
            tiff.extend_from_slice(&4u16.to_le_bytes());
            tiff.extend_from_slice(&1u32.to_le_bytes());
            tiff.extend_from_slice(&val.to_le_bytes());
        }
        tiff.extend_from_slice(&0u32.to_le_bytes());
        tiff.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xD9]);
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        jpeg.extend_from_slice(&((tiff.len() + 8) as u16).to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&tiff);
        let (thumb, o) = exif_thumbnail(&jpeg).expect("thumbnail");
        assert_eq!(thumb, &[0xFF, 0xD8, 0xFF, 0xD9]);
        assert_eq!(o, 6);
    }
}

fn scoped(generation: u64, msg: PhoneMsg) -> Message {
    Message::Phone(PhoneMsg::Scoped(generation, Box::new(msg)))
}

fn read_image_limited(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut data = Vec::new();
    std::fs::File::open(path).ok()?.take((64 << 20) + 1).read_to_end(&mut data).ok()?;
    (data.len() <= 64 << 20).then_some(data)
}
fn decode_limited(bytes: &[u8]) -> Option<image::DynamicImage> {
    if bytes.len() > 64 << 20 {
        return None;
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 << 20);
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    reader.limits(limits);
    reader.decode().ok()
}

impl PhoneState {
    fn activate(&mut self, id: &str) {
        if self.active_id == id {
            return;
        }
        let mut state = self.states.remove(id).unwrap_or_default();
        if state.generation == 0 {
            state.generation = crate::pane::next_id();
            state.photo_limit = PHOTOS_PAGE;
        }
        std::mem::swap(&mut state, &mut self.active);
        let old = std::mem::replace(&mut self.active_id, id.to_string());
        if !old.is_empty() {
            self.states.insert(old, state);
        }
    }
}

fn trim_phone_cache(dir: &Path) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let Ok(_guard) = LOCK.try_lock() else { return };
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file().then(|| (m.modified().ok(), m.len(), e.path()))
        })
        .collect();
    entries.sort_by_key(|e| e.0);
    let mut total: u64 = entries.iter().map(|e| e.1).sum();
    let excess = entries.len().saturating_sub(1500);
    for (n, (_, size, path)) in entries.into_iter().enumerate() {
        if n >= excess && total <= 256 << 20 {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn devices_keep_independent_messages_and_sessions() {
        let mut phones = PhoneState::default();
        phones.activate("a");
        let generation = phones.generation;
        phones.compose = "reply to A".into();
        phones.open_thread = Some(7);
        phones.photo_limit = 23;
        phones.activate("b");
        assert!(phones.compose.is_empty());
        assert!(phones.open_thread.is_none());
        assert_ne!(phones.generation, generation);
        phones.compose = "reply to B".into();
        phones.activate("a");
        assert_eq!(phones.compose, "reply to A");
        assert_eq!(phones.open_thread, Some(7));
        assert_eq!(phones.photo_limit, 23);
    }
    #[test]
    fn import_keys_include_device_and_relative_directory() {
        let photo = Photo { path: PathBuf::from("/mount/DCIM/a.jpg"), mtime: 1, size: 1, source: Source::Camera, video: false };
        let key = import_key("a", Some(Path::new("/mount")), &photo);
        assert_ne!(key, import_key("b", Some(Path::new("/mount")), &photo));
        let other = Photo { path: PathBuf::from("/mount/other/a.jpg"), ..photo.clone() };
        assert_ne!(key, import_key("a", Some(Path::new("/mount")), &other));
    }

    #[test]
    fn mount_points_are_one_safe_folder() {
        let base = config::runtime_dir();
        assert_eq!(mount_point("Aditya's S24+", "abc"), base.join("Aditya's S24+"));
        assert_eq!(mount_point("a/../b", "abc"), base.join("a-..-b"));
        assert_eq!(mount_point("..", "abc"), base.join("abc"));
        assert_eq!(mount_point("  ", "abc"), base.join("abc"));
    }
}

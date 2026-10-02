//! Your Android phone in EchoFiles (design system `PhoneItem`, `PhoneHub`, `PairPhone`,
//! `PhotosPage`, `MessagesPage`, `NotificationsPage`, `PhoneSettings`).
//!
//! `ef_phone` speaks KDE Connect; this module keeps what the app shows — the paired
//! phones, battery, notifications, texts, transfers and the shared clipboard — and opens
//! the phone's files through its SFTP server mounted with GVfs, so once it's open every
//! other part of EchoFiles treats it like any folder. Views are in `phone_view.rs`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, Read};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ef_config::{self as config, PhoneNotifications};
use ef_net::{gvfs, Address, Ask, Login, Mount, Protocol, Remember};
use ef_phone::{Device, Event, Notification, Service, Sftp, Sms, Trusted};
use iced::futures::channel::mpsc;
use iced::Task;

use crate::actions::FileMsg;
use crate::app::{background, App, Message};
use crate::overlay::Tone;
use crate::thumbs::Pixels;

pub const IP_ID: &str = "phone-ip";
pub const SMS_ID: &str = "phone-sms";
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
    Photos,
    Nothing,
}

#[derive(Default)]
pub struct PhoneState {
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
    pub ringing: Option<Instant>,
    pub notifications: Vec<Notification>,
    pub reply: HashMap<String, String>,
    /// Newest message per conversation.
    pub threads: HashMap<i64, Sms>,
    pub thread_msgs: HashMap<i64, Vec<Sms>>,
    pub open_thread: Option<i64>,
    pub compose: String,
    pub sftp: Option<Sftp>,
    pub sftp_error: Option<String>,
    pub mount: Option<(Mount, PathBuf)>,
    /// Free and total bytes on the phone, read in the background after mounting.
    pub space: Option<(u64, u64)>,
    pub mounting: bool,
    pub want: Option<Want>,
    pub photos: Option<Result<Vec<Photo>, String>>,
    pub photos_loading: bool,
    pub photo_thumbs: HashMap<PathBuf, Option<iced::advanced::image::Handle>>,
    pub photo_queue: VecDeque<(PathBuf, i64)>,
    pub photo_jobs: usize,
    pub photo_sel: HashSet<PathBuf>,
    pub photo_filter: Option<Source>,
    pub photo_limit: usize,
    /// The newest camera photo, drawn on the phone's screen.
    pub screen: Option<(PathBuf, String)>,
    pub imported: HashSet<String>,
    pub moved: Vec<Moved>,
    /// Offers waiting for an answer (auto-accept off): transfer → (device, name, size).
    pub offers: Vec<(u64, String, String, u64)>,
    /// Shared clipboard history: (from the phone, text, when).
    pub clip: VecDeque<(bool, String, SystemTime)>,
    /// What we last put on, or sent from, the clipboard (no echo).
    pub clip_last: String,
    pub mute_draft: String,
    pub ufw: Option<bool>,
}

// ------------------------------------------------------------------------------ service plumbing

#[derive(Debug, Clone)]
pub enum PhoneMsg {
    Event(Event),
    LocalClip(String),
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
    Mounted(Result<Mount, String>),
    Space(Option<(u64, u64)>),
    PhotosLoaded(Result<Vec<Photo>, String>),
    PhotoThumb(PathBuf, Option<Pixels>),
    Screen(PathBuf, Option<String>),
    PhotoToggle(PathBuf),
    PhotoOpen(PathBuf),
    PhotosClear,
    PhotosFilter(Option<Source>),
    PhotosMore,
    PhotosCopy,
    PhotosSaveTo,
    SaveFolder(Result<Option<PathBuf>, String>),
    Import,
    Thread(i64),
    Compose(String),
    SendSms,
    RefreshMessages,
    Dismiss(String),
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
    let tx = event_tx();
    std::thread::spawn(move || {
        let t2 = tx.clone();
        // On failure `Event::Failed` has already said why.
        if let Ok(s) = Service::start(move |e| {
            let _ = t2.unbounded_send(PhoneMsg::Event(e));
        }) {
            let _ = SERVICE.set(s);
        }
    });
    let tx = event_tx();
    std::thread::spawn(move || watch_clipboard(tx));
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
    let Ok(mut child) = cmd.spawn()
    else {
        return;
    };
    let Some(out) = child.stdout.take() else { return };
    use base64::Engine;
    for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(line.trim())
            && let Ok(text) = String::from_utf8(bytes)
            && !text.is_empty()
            && text.len() < 1 << 20
        {
            let _ = tx.unbounded_send(PhoneMsg::LocalClip(text));
        }
    }
}

fn set_clipboard(text: &str) {
    use std::io::Write;
    if let Ok(mut c) = std::process::Command::new("wl-copy").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn() {
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
    PhoneState { trusted: ef_phone::trusted(), seen, imported, photo_limit: PHOTOS_PAGE, ..Default::default() }
}

fn save_lines(file: PathBuf, text: String) {
    std::thread::spawn(move || {
        if let Some(d) = file.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(file, text);
    });
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

// ------------------------------------------------------------------------------ photos

fn photo_source(rel: &str) -> Source {
    let l = rel.to_lowercase();
    if l.contains("screenshot") {
        Source::Screenshots
    } else if l.contains("whatsapp") || l.contains("telegram") || l.contains("signal") {
        Source::Chats
    } else if l.starts_with("dcim") {
        Source::Camera
    } else {
        Source::Other
    }
}

const PHOTO_DIRS: &[&str] = &[
    "DCIM/Camera",
    "DCIM/Screenshots",
    "DCIM",
    "Pictures/Screenshots",
    "Pictures",
    "Movies",
    "Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Images",
    "Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Video",
    "WhatsApp/Media/WhatsApp Images",
    "Pictures/Telegram",
    "Telegram/Telegram Images",
];

fn is_media(name: &str) -> Option<bool> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "webp" | "heic" | "heif" | "gif" => Some(false),
        "mp4" | "mov" | "3gp" | "mkv" | "webm" => Some(true),
        _ => None,
    }
}

/// Every photo and video in the usual folders, newest first.
fn scan_photos(root: &Path) -> Result<Vec<Photo>, String> {
    use std::os::unix::fs::MetadataExt;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut any_dir = false;
    for rel in PHOTO_DIRS {
        let dir = root.join(rel);
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        any_dir = true;
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Some(video) = is_media(&name) else { continue };
            let path = e.path();
            if !seen.insert(path.clone()) {
                continue;
            }
            let Ok(m) = e.metadata() else { continue };
            if !m.is_file() || m.len() == 0 {
                continue;
            }
            out.push(Photo { path, mtime: m.mtime(), size: m.len(), source: photo_source(rel), video });
        }
    }
    if !any_dir {
        return Err("No photo folders on the phone (DCIM, Pictures). If it has photos, check KDE Connect has All files access.".into());
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.mtime));
    Ok(out)
}

fn thumb_cache(path: &Path, mtime: i64, tag: &str) -> PathBuf {
    let key = format!("{:x}", md5::compute(format!("{}\0{mtime}\0{tag}", path.display()).as_bytes()));
    std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| config::home().join(".cache")).join("echofiles/phone-thumbs").join(format!("{key}.png"))
}

/// The thumbnail JPEG inside a JPEG's EXIF block, and the photo's orientation — read from
/// the first bytes only, so a thumbnail costs ~64 KB over Wi-Fi instead of the whole photo.
fn exif_thumbnail(head: &[u8]) -> Option<(&[u8], u16)> {
    if head.len() < 4 || head[0] != 0xFF || head[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 4 <= head.len() {
        if head[i] != 0xFF {
            return None;
        }
        let marker = head[i + 1];
        let len = u16::from_be_bytes([head[i + 2], head[i + 3]]) as usize;
        if marker == 0xDA {
            return None;
        }
        if marker == 0xE1 && head.get(i + 4..i + 10) == Some(b"Exif\0\0") {
            let tiff = head.get(i + 10..(i + 2 + len).min(head.len()))?;
            return parse_tiff_thumb(tiff);
        }
        i += 2 + len;
    }
    None
}

fn parse_tiff_thumb(t: &[u8]) -> Option<(&[u8], u16)> {
    let le = match t.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |o: usize| -> Option<u16> {
        let b = t.get(o..o + 2)?;
        Some(if le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })
    };
    let u32_at = |o: usize| -> Option<u32> {
        let b = t.get(o..o + 4)?;
        Some(if le { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) } else { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) })
    };
    let ifd0 = u32_at(4)? as usize;
    let n0 = u16_at(ifd0)? as usize;
    let mut orientation = 1;
    for k in 0..n0 {
        let e = ifd0 + 2 + 12 * k;
        if u16_at(e)? == 0x0112 {
            orientation = u16_at(e + 8)?;
        }
    }
    let ifd1 = u32_at(ifd0 + 2 + 12 * n0)? as usize;
    if ifd1 == 0 {
        return None;
    }
    let n1 = u16_at(ifd1)? as usize;
    let (mut off, mut len) = (0usize, 0usize);
    for k in 0..n1 {
        let e = ifd1 + 2 + 12 * k;
        match u16_at(e)? {
            0x0201 => off = u32_at(e + 8)? as usize,
            0x0202 => len = u32_at(e + 8)? as usize,
            _ => {}
        }
    }
    let jpeg = t.get(off..off.checked_add(len)?)?;
    (len > 0 && jpeg.starts_with(&[0xFF, 0xD8])).then_some((jpeg, orientation))
}

fn orient(img: image::DynamicImage, o: u16) -> image::DynamicImage {
    match o {
        3 => img.rotate180(),
        6 => img.rotate90(),
        8 => img.rotate270(),
        _ => img,
    }
}

fn to_pixels(img: image::DynamicImage) -> Pixels {
    let rgba = img.to_rgba8();
    Pixels { width: rgba.width(), height: rgba.height(), rgba: Arc::new(rgba.into_raw()) }
}

/// A square thumbnail for the Photos grid: cache → EXIF thumbnail → whole file (small
/// images only). Videos get none.
fn photo_thumb(path: &Path, mtime: i64) -> Option<Pixels> {
    let cache = thumb_cache(path, mtime, "tile");
    if let Ok(img) = image::open(&cache) {
        return Some(to_pixels(img));
    }
    let name = path.file_name()?.to_string_lossy().into_owned();
    if is_media(&name)? {
        return None;
    }
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = vec![0u8; 128 * 1024];
    let n = f.read(&mut head).ok()?;
    head.truncate(n);
    let img = match exif_thumbnail(&head) {
        Some((jpeg, o)) => orient(image::load_from_memory_with_format(jpeg, image::ImageFormat::Jpeg).ok()?, o),
        None => {
            let size = f.metadata().ok()?.len();
            if size > 24 << 20 {
                return None;
            }
            f.read_to_end(&mut head).ok()?;
            image::load_from_memory(&head).ok()?
        }
    };
    let img = img.resize_to_fill(200, 200, image::imageops::FilterType::Triangle);
    if let Some(d) = cache.parent() {
        let _ = std::fs::create_dir_all(d);
        let _ = img.save(&cache);
    }
    Some(to_pixels(img))
}

/// The newest photo, sized for the phone's screen, as a base64 JPEG.
fn screen_image(path: &Path, mtime: i64) -> Option<String> {
    use base64::Engine;
    let cache = thumb_cache(path, mtime, "screen").with_extension("jpg");
    let bytes = match std::fs::read(&cache) {
        Ok(b) => b,
        Err(_) => {
            let data = std::fs::read(path).ok()?;
            let o = exif_thumbnail(&data).map_or(1, |(_, o)| o);
            let img = orient(image::load_from_memory(&data).ok()?, o).resize_to_fill(300, 620, image::imageops::FilterType::Triangle);
            let mut out = Vec::new();
            img.to_rgb8().write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Jpeg).ok()?;
            if let Some(d) = cache.parent() {
                let _ = std::fs::create_dir_all(d);
                let _ = std::fs::write(&cache, &out);
            }
            out
        }
    };
    Some(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn import_key(p: &Photo) -> String {
    format!("{}\t{}\t{}", p.path.file_name().unwrap_or_default().to_string_lossy(), p.size, p.mtime)
}

fn month_of(secs: i64) -> String {
    // Civil date from days since the epoch (Howard Hinnant's algorithm).
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}")
}

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
    let request = zbus::blocking::Proxy::new(&conn, "org.freedesktop.portal.Desktop", path.as_str(), "org.freedesktop.portal.Request").map_err(|e| e.to_string())?;
    let mut responses = request.receive_signal("Response").map_err(|e| e.to_string())?;
    let chooser = zbus::blocking::Proxy::new(&conn, "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop", "org.freedesktop.portal.FileChooser").map_err(|e| e.to_string())?;
    let mut opts: HashMap<&str, Value<'_>> = HashMap::new();
    opts.insert("handle_token", Value::from(token.as_str()));
    opts.insert("multiple", Value::from(!folder));
    opts.insert("directory", Value::from(folder));
    let _: zbus::zvariant::OwnedObjectPath = chooser.call("OpenFile", &("", title, opts)).map_err(|e| format!("The file chooser isn't available: {e}"))?;
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
        .args(["sh", "-c", "ufw allow 1714:1764/udp comment 'KDE Connect (EchoFiles)' && ufw allow 1714:1764/tcp comment 'KDE Connect (EchoFiles)'"])
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
    let _ = std::process::Command::new("notify-send")
        .args(["--app-name", phone, "--icon", "phone", &title, &n.text])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
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
        self.phone.devices.get(id).map(|d| d.name.clone()).or_else(|| self.phone.trusted.iter().find(|t| t.id == id).map(|t| t.name.clone())).filter(|n| !n.is_empty()).unwrap_or_else(|| "Phone".into())
    }

    /// Paired and connected right now.
    pub(crate) fn phone_online(&self, id: &str) -> bool {
        self.phone.devices.get(id).is_some_and(|d| d.paired)
    }

    /// The phone's files are mounted at this root.
    pub(crate) fn phone_mount_root(&self) -> Option<&Path> {
        self.phone.mount.as_ref().map(|(m, _)| m.root.as_path())
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
    pub(crate) fn open_phone_page(&mut self, page: PhonePage) -> Task<Message> {
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
            PhonePage::Photos if self.settings.phone.files => {
                if self.phone.mount.is_none() {
                    return self.phone_files(Want::Photos);
                }
                if self.phone.photos.is_none() && !self.phone.photos_loading {
                    return self.scan_phone_photos();
                }
            }
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

    fn scan_phone_photos(&mut self) -> Task<Message> {
        let Some(root) = self.phone_storage().map(Path::to_path_buf) else { return Task::none() };
        self.phone.photos_loading = true;
        background(move || scan_photos(&root), |r| Message::Phone(PhoneMsg::PhotosLoaded(r)))
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
        self.phone.mounting = true;
        let address = Address { protocol: Protocol::Sftp, host: s.ip.clone(), port: Some(s.port), user: Some(s.user.clone()), domain: None, share: None, path: String::new() };
        let (user, password) = (s.user.clone(), s.password.clone());
        let op = gvfs::next_op();
        background(
            move || {
                gvfs::mount(&address, op, move |ask| match ask {
                    Ask::Login(l) => l.answer(Some(Login { user: user.clone(), domain: String::new(), password: password.clone(), anonymous: false, remember: Remember::Never })),
                    // The phone's host key is new each time its server starts; the TLS link
                    // already proved who it is.
                    Ask::Question(q) => {
                        let pick = q.choices.iter().position(|c| !c.to_lowercase().contains("cancel")).unwrap_or(0);
                        q.answer(Some(pick));
                    }
                    Ask::Withdrawn(_) => {}
                })
                .map_err(|e| e.to_string())
            },
            |r| Message::Phone(PhoneMsg::Mounted(r)),
        )
    }

    fn after_mount(&mut self, want: Want, storage: &Path) -> Task<Message> {
        self.phone.want = None;
        let mut tasks = Vec::new();
        if self.phone.photos.is_none() && !self.phone.photos_loading {
            tasks.push(self.scan_phone_photos());
        }
        match want {
            Want::Files => tasks.push(self.go(storage.to_path_buf(), true)),
            Want::FilesInTab => tasks.push(self.update(Message::OpenTab(storage.to_path_buf()))),
            Want::Photos | Want::Nothing => {}
        }
        Task::batch(tasks)
    }

    /// The phone went away or was forgotten: close its folders and its mount.
    fn drop_phone_mount(&mut self) -> Task<Message> {
        self.phone.sftp = None;
        self.phone.mounting = false;
        self.phone.space = None;
        let Some((m, _)) = self.phone.mount.take() else { return Task::none() };
        let root = m.root.clone();
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
        std::thread::spawn(move || {
            let _ = gvfs::unmount(&m, true);
        });
        Task::batch(tasks)
    }

    fn pump_photo_thumbs(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        while self.phone.photo_jobs < PHOTO_JOBS {
            let Some((p, m)) = self.phone.photo_queue.pop_front() else { break };
            if self.phone.photo_thumbs.contains_key(&p) {
                continue;
            }
            self.phone.photo_jobs += 1;
            let q = p.clone();
            tasks.push(background(move || photo_thumb(&q, m), move |px| Message::Phone(PhoneMsg::PhotoThumb(p.clone(), px))));
        }
        Task::batch(tasks)
    }

    /// Queue thumbnails for what the Photos page shows.
    pub(crate) fn want_photo_thumbs(&mut self) -> Task<Message> {
        let Some(Ok(list)) = &self.phone.photos else { return Task::none() };
        let filter = self.phone.photo_filter;
        let items: Vec<(PathBuf, i64)> = list.iter().filter(|p| filter.is_none_or(|f| p.source == f)).take(self.phone.photo_limit).filter(|p| !self.phone.photo_thumbs.contains_key(&p.path)).map(|p| (p.path.clone(), p.mtime)).collect();
        self.phone.photo_queue = items.into();
        self.pump_photo_thumbs()
    }

    pub(crate) fn photos_new(&self) -> Vec<&Photo> {
        match &self.phone.photos {
            Some(Ok(list)) => list.iter().filter(|p| p.source == Source::Camera && !self.phone.imported.contains(&import_key(p))).collect(),
            _ => Vec::new(),
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
            self.toast_error(format!("{} isn't nearby", self.phone_name(&id)), "Open KDE Connect on the phone, on the same Wi-Fi.".into());
            return Task::none();
        }
        if let Some(s) = service() {
            s.send_files(&id, files);
        }
        Task::none()
    }

    pub(crate) fn phone_update(&mut self, msg: PhoneMsg) -> Task<Message> {
        match msg {
            PhoneMsg::Event(e) => self.phone_event(e),
            PhoneMsg::LocalClip(text) => {
                if !self.settings.phone.clipboard || text == self.phone.clip_last {
                    return Task::none();
                }
                self.phone.clip_last = text.clone();
                let ids: Vec<String> = self.phone.devices.values().filter(|d| d.paired).map(|d| d.id.clone()).collect();
                if let Some(s) = service()
                    && !ids.is_empty()
                {
                    for id in ids {
                        s.send_clipboard(&id, &text, false, false);
                    }
                    self.phone.clip.push_front((false, text, SystemTime::now()));
                    self.phone.clip.truncate(20);
                }
                Task::none()
            }
            PhoneMsg::Open(page) => self.open_phone_page(page),
            PhoneMsg::Select(id) => {
                self.phone.current = Some(id);
                self.phone.dialog = None;
                self.open_phone_page(PhonePage::Hub)
            }
            PhoneMsg::Dialog(open) => {
                if open {
                    self.menu = None;
                    self.command = None;
                    self.phone.dialog = Some(PairDialog { ip: String::new(), opened: Instant::now(), since: Instant::now(), firewall: None, firewall_busy: false, paired: None });
                    if let Some(s) = service() {
                        s.announce();
                    }
                    background(ufw_active, |on| Message::Phone(PhoneMsg::Ufw(on)))
                } else {
                    self.phone.dialog = None;
                    // Waiting on the phone's answer: the request simply lapses.
                    if self.phone.code.as_ref().is_some_and(|c| !c.2) {
                        self.phone.code = None;
                    }
                    Task::none()
                }
            }
            PhoneMsg::Pair(id) => {
                if let Some(s) = service() {
                    s.pair(&id);
                }
                Task::none()
            }
            PhoneMsg::AcceptPair => {
                if let (Some((id, _, true)), Some(s)) = (self.phone.code.clone(), service()) {
                    s.accept_pair(&id);
                }
                Task::none()
            }
            PhoneMsg::RejectPair => {
                // Codes don't match, or we stop waiting: either way the phone is told.
                if let Some((id, _, _)) = self.phone.code.take()
                    && let Some(s) = service()
                {
                    s.reject_pair(&id);
                }
                Task::none()
            }
            PhoneMsg::Forget(id) => {
                if let Some(s) = service() {
                    s.unpair(&id);
                }
                Task::none()
            }
            PhoneMsg::IpDraft(t) => {
                if let Some(d) = &mut self.phone.dialog {
                    d.ip = t;
                }
                Task::none()
            }
            PhoneMsg::IpSubmit => {
                let Some(d) = &self.phone.dialog else { return Task::none() };
                let raw = d.ip.trim();
                let addr: Option<SocketAddr> = raw.parse::<SocketAddr>().ok().or_else(|| raw.parse::<IpAddr>().ok().map(|ip| SocketAddr::new(ip, ef_phone::UDP_PORT)));
                match (addr, service()) {
                    (Some(a), Some(s)) => {
                        s.connect_to(a);
                        s.announce_to(SocketAddr::new(a.ip(), ef_phone::UDP_PORT));
                    }
                    (None, _) => self.toast_error("That isn't an IP address".into(), "Type the phone's address, like 192.168.1.20. KDE Connect shows it under its own device name.".into()),
                    _ => {}
                }
                Task::none()
            }
            PhoneMsg::AllowFirewall => {
                if let Some(d) = &mut self.phone.dialog {
                    d.firewall_busy = true;
                }
                background(allow_firewall, |r| Message::Phone(PhoneMsg::Firewall(r)))
            }
            PhoneMsg::Firewall(r) => {
                if let Some(d) = &mut self.phone.dialog {
                    d.firewall_busy = false;
                    d.firewall = Some(r);
                }
                if let Some(s) = service() {
                    s.announce();
                }
                Task::none()
            }
            PhoneMsg::Ufw(on) => {
                self.phone.ufw = Some(on);
                Task::none()
            }
            PhoneMsg::Files(want) => self.phone_files(want),
            PhoneMsg::SendFiles => {
                let name = self.phone_id().map(|i| self.phone_name(&i)).unwrap_or_else(|| "phone".into());
                background(move || portal_pick(&format!("Send to {name}"), false), |r| Message::Phone(PhoneMsg::Picked(r)))
            }
            PhoneMsg::SendPaths(paths) => self.send_to_phone(paths),
            PhoneMsg::Picked(Ok(paths)) => {
                if paths.is_empty() {
                    return Task::none();
                }
                self.send_to_phone(paths)
            }
            PhoneMsg::Picked(Err(e)) => {
                self.toast_error("Couldn't open the file chooser".into(), e);
                Task::none()
            }
            PhoneMsg::Ring => {
                let Some(id) = self.phone_id() else { return Task::none() };
                if let Some(s) = service()
                    && s.ring(&id)
                {
                    self.phone.ringing = if self.phone.ringing.is_some() { None } else { Some(Instant::now()) };
                }
                Task::none()
            }
            PhoneMsg::Mounted(Ok(m)) => {
                self.phone.mounting = false;
                let storage = match &self.phone.sftp {
                    Some(s) if !s.path.is_empty() => {
                        let rel = s.path.trim_start_matches('/');
                        let p = m.root.join(rel);
                        if p.is_dir() { p } else { m.root.clone() }
                    }
                    _ => m.root.clone(),
                };
                self.phone.mount = Some((m, storage.clone()));
                let want = self.phone.want.unwrap_or(Want::Nothing);
                let t = self.after_mount(want, &storage);
                let s2 = storage.clone();
                let space = background(move || crate::phone_view::fs_space(&s2), |r| Message::Phone(PhoneMsg::Space(r)));
                Task::batch([t, space, crate::network::refresh_mounts_task()])
            }
            PhoneMsg::Space(s) => {
                self.phone.space = s;
                Task::none()
            }
            PhoneMsg::Mounted(Err(e)) => {
                self.phone.mounting = false;
                self.phone.sftp = None;
                let wanted = self.phone.want.take().is_some_and(|w| w != Want::Nothing);
                self.phone.sftp_error = Some(e.clone());
                if wanted {
                    self.toast_error("Couldn't open the phone's files".into(), format!("{e} Check that Filesystem expose is on in KDE Connect."));
                }
                Task::none()
            }
            PhoneMsg::PhotosLoaded(r) => {
                self.phone.photos_loading = false;
                let newest = match &r {
                    Ok(list) => list.iter().find(|p| p.source == Source::Camera && !p.video).or_else(|| list.iter().find(|p| !p.video)).map(|p| (p.path.clone(), p.mtime)),
                    Err(_) => None,
                };
                self.phone.photos = Some(r);
                let mut tasks = vec![self.want_photo_thumbs()];
                if let Some((path, mtime)) = newest
                    && self.phone.screen.as_ref().is_none_or(|(p, _)| p != &path)
                {
                    let q = path.clone();
                    tasks.push(background(move || screen_image(&q, mtime), move |b| Message::Phone(PhoneMsg::Screen(path.clone(), b))));
                }
                Task::batch(tasks)
            }
            PhoneMsg::Screen(path, b64) => {
                if let Some(b) = b64 {
                    self.phone.screen = Some((path, b));
                }
                Task::none()
            }
            PhoneMsg::PhotoThumb(path, px) => {
                self.phone.photo_jobs = self.phone.photo_jobs.saturating_sub(1);
                let h = px.map(|px| iced::advanced::image::Handle::from_rgba(px.width, px.height, (*px.rgba).clone()));
                self.phone.photo_thumbs.insert(path, h);
                self.pump_photo_thumbs()
            }
            PhoneMsg::PhotoToggle(p) => {
                if !self.phone.photo_sel.remove(&p) {
                    self.phone.photo_sel.insert(p);
                }
                Task::none()
            }
            PhoneMsg::PhotoOpen(p) => self.open_path(p),
            PhoneMsg::PhotosClear => {
                self.phone.photo_sel.clear();
                Task::none()
            }
            PhoneMsg::PhotosFilter(f) => {
                self.phone.photo_filter = f;
                self.phone.photo_limit = PHOTOS_PAGE;
                self.want_photo_thumbs()
            }
            PhoneMsg::PhotosMore => {
                self.phone.photo_limit += PHOTOS_PAGE;
                self.want_photo_thumbs()
            }
            PhoneMsg::PhotosCopy => {
                let paths: Vec<PathBuf> = self.phone.photo_sel.iter().cloned().collect();
                if !paths.is_empty() {
                    let n = paths.len();
                    self.clip = Some(crate::actions::Clip { paths, cut: false });
                    self.push_toast(Tone::Accent, format!("{n} copied"), Some("Open any folder and paste with Ctrl+V.".into()), None);
                }
                Task::none()
            }
            PhoneMsg::PhotosSaveTo => background(|| portal_pick("Save photos to", true).map(|v| v.into_iter().next()), |r| Message::Phone(PhoneMsg::SaveFolder(r))),
            PhoneMsg::SaveFolder(Ok(Some(dest))) => {
                let sources: Vec<PathBuf> = self.phone.photo_sel.drain().collect();
                if sources.is_empty() {
                    return Task::none();
                }
                self.file_update(FileMsg::Start { kind: ef_core::ops::Kind::Copy, sources, dest })
            }
            PhoneMsg::SaveFolder(Ok(None)) => Task::none(),
            PhoneMsg::SaveFolder(Err(e)) => {
                self.toast_error("Couldn't open the folder chooser".into(), e);
                Task::none()
            }
            PhoneMsg::Import => {
                let new: Vec<Photo> = self.photos_new().into_iter().cloned().collect();
                if new.is_empty() {
                    return Task::none();
                }
                let base = config::home().join("Pictures/Phone");
                let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
                for p in &new {
                    groups.entry(month_of(p.mtime)).or_default().push(p.path.clone());
                    self.phone.imported.insert(import_key(p));
                }
                let text: String = self.phone.imported.iter().map(|k| format!("{k}\n")).collect();
                save_lines(imported_file(), text);
                let mut tasks = Vec::new();
                for (month, sources) in groups {
                    let dest = base.join(month);
                    let _ = std::fs::create_dir_all(&dest);
                    tasks.push(self.file_update(FileMsg::Start { kind: ef_core::ops::Kind::Copy, sources, dest }));
                }
                Task::batch(tasks)
            }
            PhoneMsg::Thread(t) => {
                self.phone.open_thread = Some(t);
                if let (Some(id), Some(s)) = (self.phone_id(), service()) {
                    s.request_thread(&id, t);
                }
                iced::widget::operation::focus(SMS_ID)
            }
            PhoneMsg::Compose(t) => {
                self.phone.compose = t;
                Task::none()
            }
            PhoneMsg::SendSms => {
                let text = self.phone.compose.trim().to_string();
                let (Some(t), Some(id)) = (self.phone.open_thread, self.phone_id()) else { return Task::none() };
                if text.is_empty() {
                    return Task::none();
                }
                let addresses: Vec<String> = self.phone.threads.get(&t).map(|m| m.address.split(", ").map(str::to_string).collect()).unwrap_or_default();
                if let Some(s) = service()
                    && s.send_sms(&id, &addresses, &text)
                {
                    let sent = Sms { thread: t, uid: -(now_s()), address: addresses.join(", "), body: text, date: now_s() * 1000, outgoing: true, read: true };
                    self.phone.thread_msgs.entry(t).or_default().push(sent.clone());
                    self.phone.threads.insert(t, sent);
                    self.phone.compose.clear();
                }
                Task::none()
            }
            PhoneMsg::RefreshMessages => {
                if let (Some(id), Some(s)) = (self.phone_id(), service()) {
                    s.request_conversations(&id);
                }
                Task::none()
            }
            PhoneMsg::Dismiss(key) => {
                if let (Some(id), Some(s)) = (self.phone_id(), service()) {
                    s.dismiss_notification(&id, &key);
                }
                self.phone.notifications.retain(|n| n.key != key);
                Task::none()
            }
            PhoneMsg::DismissAll => {
                if let (Some(id), Some(s)) = (self.phone_id(), service()) {
                    for n in self.phone.notifications.iter().filter(|n| n.dismissable) {
                        s.dismiss_notification(&id, &n.key);
                    }
                }
                self.phone.notifications.retain(|n| !n.dismissable);
                Task::none()
            }
            PhoneMsg::ReplyDraft(key, t) => {
                self.phone.reply.insert(key, t);
                Task::none()
            }
            PhoneMsg::Reply(key) => {
                let text = self.phone.reply.get(&key).cloned().unwrap_or_default();
                let reply_id = self.phone.notifications.iter().find(|n| n.key == key).and_then(|n| n.reply_id.clone());
                if let (Some(rid), Some(id), Some(s)) = (reply_id, self.phone_id(), service())
                    && !text.trim().is_empty()
                    && s.reply_notification(&id, &rid, text.trim())
                {
                    self.phone.reply.remove(&key);
                }
                Task::none()
            }
            PhoneMsg::ClipCopy(text) => {
                self.phone.clip_last = text.clone();
                std::thread::spawn(move || set_clipboard(&text));
                Task::none()
            }
            PhoneMsg::AcceptOffer(t) => {
                if let Some(s) = service() {
                    s.accept_file(t, config::expand(&self.settings.phone.save_to));
                }
                self.phone.offers.retain(|o| o.0 != t);
                self.toasts.retain(|x| !matches!(&x.action, Some((_, Message::Phone(PhoneMsg::AcceptOffer(id)))) if *id == t));
                Task::none()
            }
            PhoneMsg::DeclineOffer(t) => {
                if let Some(s) = service() {
                    s.reject_file(t);
                }
                self.phone.offers.retain(|o| o.0 != t);
                Task::none()
            }
            PhoneMsg::OpenFile(p) => self.open_path(p),
            PhoneMsg::OpenSaveFolder => {
                let dir = config::expand(&self.settings.phone.save_to);
                let _ = std::fs::create_dir_all(&dir);
                self.go(dir, true)
            }
            PhoneMsg::SetFiles(on) => {
                self.settings.phone.files = on;
                let t = if on { Task::none() } else { self.drop_phone_mount() };
                Task::batch([t, self.persist_settings()])
            }
            PhoneMsg::SetClipboard(on) => {
                self.settings.phone.clipboard = on;
                self.persist_settings()
            }
            PhoneMsg::SetMessages(on) => {
                self.settings.phone.messages = on;
                if !on {
                    self.phone.threads.clear();
                    self.phone.thread_msgs.clear();
                } else if let (Some(id), Some(s)) = (self.phone_id(), service()) {
                    s.request_conversations(&id);
                }
                self.persist_settings()
            }
            PhoneMsg::SetNotifications(n) => {
                self.settings.phone.notifications = n;
                if n == PhoneNotifications::Off {
                    self.phone.notifications.clear();
                }
                self.persist_settings()
            }
            PhoneMsg::SetBatteryWarning(on) => {
                self.settings.phone.battery_warning = on;
                self.persist_settings()
            }
            PhoneMsg::SetAutoAccept(on) => {
                self.settings.phone.auto_accept = on;
                self.persist_settings()
            }
            PhoneMsg::SetPicture(p) => {
                self.settings.phone.picture = p;
                self.persist_settings()
            }
            PhoneMsg::ChangeSaveTo => background(
                || portal_pick("Save files from the phone to", true).map(|v| v.into_iter().next()),
                |r| match r {
                    Ok(Some(p)) => Message::Phone(PhoneMsg::SaveTo(p)),
                    _ => Message::Tick,
                },
            ),
            PhoneMsg::SaveTo(p) => {
                self.settings.phone.save_to = config::tilde(&p);
                self.persist_settings()
            }
            PhoneMsg::MuteDraft(t) => {
                self.phone.mute_draft = t;
                Task::none()
            }
            PhoneMsg::MuteAdd => {
                let name = self.phone.mute_draft.trim().to_string();
                if !name.is_empty() && !self.settings.phone.muted_apps.iter().any(|a| a.eq_ignore_ascii_case(&name)) {
                    self.settings.phone.muted_apps.push(name);
                    self.phone.mute_draft.clear();
                    return self.persist_settings();
                }
                Task::none()
            }
            PhoneMsg::MuteRemove(i) => {
                if i < self.settings.phone.muted_apps.len() {
                    self.settings.phone.muted_apps.remove(i);
                }
                self.persist_settings()
            }
        }
    }

    fn phone_event(&mut self, e: Event) -> Task<Message> {
        match e {
            Event::Started { port } => {
                self.phone.started = true;
                self.phone.port = port;
                self.phone.error = None;
                Task::none()
            }
            Event::Failed(why) => {
                self.phone.error = Some(why);
                Task::none()
            }
            Event::Device(d) => {
                let id = d.id.clone();
                let paired = d.paired;
                // A phone whose details changed (renamed) keeps its trust record fresh.
                self.phone.devices.insert(id.clone(), d);
                if paired {
                    self.phone_seen(&id);
                    if self.settings.phone.clipboard && !self.phone.clip_last.is_empty()
                        && let Some(s) = service()
                    {
                        s.send_clipboard(&id, &self.phone.clip_last, true, false);
                    }
                    if self.settings.phone.files && self.phone.mount.is_none() && Some(&id) == self.phone_id().as_ref() {
                        return self.phone_files(Want::Nothing);
                    }
                }
                Task::none()
            }
            Event::Gone(id) => {
                if self.phone.devices.get(&id).is_some_and(|d| d.paired) {
                    self.phone_seen(&id);
                }
                self.phone.devices.remove(&id);
                if self.phone.code.as_ref().is_some_and(|c| c.0 == id) {
                    self.phone.code = None;
                }
                self.phone.ringing = None;
                if Some(&id) == self.phone_id().as_ref() {
                    self.phone.notifications.clear();
                    return self.drop_phone_mount();
                }
                Task::none()
            }
            Event::PairRequested { id, code } => {
                self.phone.code = Some((id, code, true));
                if self.phone.dialog.is_none() {
                    self.phone.dialog = Some(PairDialog { ip: String::new(), opened: Instant::now(), since: Instant::now(), firewall: None, firewall_busy: false, paired: None });
                }
                self.show_window()
            }
            Event::PairCode { id, code } => {
                self.phone.code = Some((id, code, false));
                Task::none()
            }
            Event::Paired(d) => {
                self.phone.code = None;
                self.phone.trusted = ef_phone::trusted();
                self.phone.current = Some(d.id.clone());
                self.phone.devices.insert(d.id.clone(), d.clone());
                self.phone_seen(&d.id);
                match &mut self.phone.dialog {
                    Some(dlg) => dlg.paired = Some(d.id.clone()),
                    None => self.push_toast(Tone::Accent, format!("Paired with {}", d.name), None, None),
                }
                if self.settings.phone.files { self.phone_files(Want::Nothing) } else { Task::none() }
            }
            Event::PairRejected(id) => {
                if self.phone.code.as_ref().is_some_and(|c| c.0 == id) {
                    self.phone.code = None;
                }
                self.toast_error(format!("{} didn't pair", self.phone_name(&id)), "It was declined on the phone. Try again, and tap Accept there.".into());
                Task::none()
            }
            Event::Unpaired(id) => {
                let name = self.phone_name(&id);
                self.phone.trusted = ef_phone::trusted();
                if let Some(d) = self.phone.devices.get_mut(&id) {
                    d.paired = false;
                }
                let t = if self.phone.current.as_deref() == Some(&id) || self.phone_id().is_none() {
                    self.phone.current = None;
                    self.phone.notifications.clear();
                    self.phone.threads.clear();
                    self.phone.thread_msgs.clear();
                    self.phone.photos = None;
                    self.phone.screen = None;
                    self.drop_phone_mount()
                } else {
                    Task::none()
                };
                // Pages about a phone that's gone close.
                if self.phone.trusted.is_empty() {
                    for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()) {
                        pane.phone = None;
                    }
                }
                self.push_toast(Tone::Accent, format!("Forgot {name}"), Some("Pair again from the Phone section.".into()), None);
                t
            }
            Event::Battery { id, level, charging } => {
                self.phone.battery.insert(id.clone(), (level, charging));
                if self.settings.phone.battery_warning && level <= 15 && !charging {
                    if self.phone.low_warned.insert(id.clone()) {
                        self.push_toast(Tone::Accent, format!("{} is at {level}%", self.phone_name(&id)), Some("Plug it in soon.".into()), None);
                    }
                } else if level > 20 || charging {
                    self.phone.low_warned.remove(&id);
                }
                Task::none()
            }
            Event::Clipboard { text, .. } => {
                if !self.settings.phone.clipboard || text == self.phone.clip_last {
                    return Task::none();
                }
                self.phone.clip_last = text.clone();
                self.phone.clip.push_front((true, text.clone(), SystemTime::now()));
                self.phone.clip.truncate(20);
                std::thread::spawn(move || set_clipboard(&text));
                Task::none()
            }
            Event::Notification { id, notification: n } => {
                let mode = self.settings.phone.notifications;
                if mode == PhoneNotifications::Off || self.settings.phone.muted_apps.iter().any(|a| a.eq_ignore_ascii_case(&n.app)) {
                    return Task::none();
                }
                let fresh = !self.phone.notifications.iter().any(|x| x.key == n.key);
                if mode == PhoneNotifications::Desktop && fresh && !n.silent {
                    notify_desktop(&self.phone_name(&id), &n);
                }
                self.phone.notifications.retain(|x| x.key != n.key);
                self.phone.notifications.push(n);
                self.phone.notifications.sort_by_key(|n| std::cmp::Reverse(n.time));
                Task::none()
            }
            Event::NotificationGone { key, .. } => {
                self.phone.notifications.retain(|n| n.key != key);
                Task::none()
            }
            Event::Sms { messages, .. } => {
                if !self.settings.phone.messages {
                    return Task::none();
                }
                for m in messages {
                    let t = m.thread;
                    let newer = self.phone.threads.get(&t).is_none_or(|old| m.date >= old.date);
                    if newer {
                        self.phone.threads.insert(t, m.clone());
                    }
                    let list = self.phone.thread_msgs.entry(t).or_default();
                    // Our own optimistic copy (negative uid) gives way to the phone's.
                    if m.outgoing {
                        list.retain(|x| !(x.uid < 0 && x.body == m.body));
                    }
                    if !list.iter().any(|x| (x.uid == m.uid && x.uid > 0) || (x.date == m.date && x.body == m.body && x.outgoing == m.outgoing)) {
                        list.push(m);
                    }
                    list.sort_by_key(|x| x.date);
                }
                Task::none()
            }
            Event::Sftp { result, .. } => match result {
                Ok(s) => {
                    self.phone.sftp = Some(s.clone());
                    self.mount_phone(s)
                }
                Err(e) => {
                    self.phone.mounting = false;
                    let wanted = self.phone.want.take().is_some_and(|w| w != Want::Nothing);
                    self.phone.sftp_error = Some(e.clone());
                    if wanted {
                        self.toast_error("The phone didn't share its files".into(), format!("{e} In KDE Connect, turn on Filesystem expose and allow All files access."));
                    }
                    Task::none()
                }
            },
            Event::Text { id, text } => {
                self.phone.clip_last = text.clone();
                let t2 = text.clone();
                std::thread::spawn(move || set_clipboard(&t2));
                self.push_toast(Tone::Accent, format!("Text from {}", self.phone_name(&id)), Some(format!("Copied to the clipboard: {}", text.chars().take(120).collect::<String>())), None);
                Task::none()
            }
            Event::Url { id, url } => {
                if url.starts_with("http://") || url.starts_with("https://") {
                    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
                }
                self.push_toast(Tone::Accent, format!("Link from {}", self.phone_name(&id)), Some(url), None);
                Task::none()
            }
            Event::Incoming { id, transfer, name, size, .. } => {
                if self.settings.phone.auto_accept {
                    if let Some(s) = service() {
                        s.accept_file(transfer, config::expand(&self.settings.phone.save_to));
                    }
                } else {
                    let mut sz = String::new();
                    ef_core::fmt::size(size, &mut sz);
                    let phone = self.phone_name(&id);
                    self.phone.offers.push((transfer, id, name.clone(), size));
                    self.push_toast(Tone::Accent, format!("{phone} wants to send “{name}”"), Some(format!("{sz} · saves to {}", self.settings.phone.save_to)), Some(("Accept".into(), Message::Phone(PhoneMsg::AcceptOffer(transfer)))));
                }
                Task::none()
            }
            Event::Other { .. } | Event::Ready(_) => Task::none(),
            Event::TransferStarted { transfer, name, size, upload, done, .. } => {
                self.phone.moved.insert(0, Moved { transfer, name, size, upload, done, result: None, at: SystemTime::now() });
                self.phone.moved.truncate(30);
                Task::none()
            }
            Event::TransferDone { id, transfer, upload, result } => {
                if let Some(m) = self.phone.moved.iter_mut().find(|m| m.transfer == transfer) {
                    if let (Ok(p), false) = (&result, upload) {
                        m.name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    }
                    m.result = Some(result.clone());
                    m.at = SystemTime::now();
                }
                match (&result, upload) {
                    (Err(e), true) => self.toast_error(format!("Couldn't send to {}", self.phone_name(&id)), e.clone()),
                    (Err(e), false) => self.toast_error(format!("Couldn't receive from {}", self.phone_name(&id)), e.clone()),
                    (Ok(p), false) => {
                        let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
                        self.push_toast(Tone::Accent, format!("Received “{name}”"), Some(format!("From {} · in {}", self.phone_name(&id), self.settings.phone.save_to)), Some(("Show".into(), Message::Phone(PhoneMsg::OpenSaveFolder))));
                        let dir = p.parent().map(Path::to_path_buf);
                        if let Some(d) = dir {
                            return self.refresh(&d);
                        }
                    }
                    (Ok(_), true) => {}
                }
                Task::none()
            }
        }
    }

    /// Expire the ringing state; refresh discovery while the Connect dialog is open.
    pub(crate) fn phone_tick(&mut self) {
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

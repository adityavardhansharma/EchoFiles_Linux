//! Discovery, links and packets.
//!
//! Connection set-up, as KDE Connect does it: whoever hears the other's UDP identity opens
//! the TCP connection, sends its identity in plain text and becomes the TLS *server*; the
//! side that accepted reads that identity and becomes the TLS *client*. With protocol 8
//! both then send their identity again inside TLS. Each link is one thread that writes
//! queued packets and reads lines (a short read timeout interleaves the two).
//!
//! File payloads go over their own connections: the sender listens on a port in
//! 1739–1764 and names it in the packet; the receiver connects and is the TLS client.
//!
//! A device can also be reached over Bluetooth ([`Service::adopt`]): the same identity
//! exchange and TLS, over an RFCOMM socket. Bluetooth carries only the clipboard, calls and
//! pings; everything else, and every payload, waits for Wi-Fi.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rustls::pki_types::ServerName;
use rustls::{ClientConnection, ServerConnection, StreamOwned};
use serde_json::{json, Value};

use crate::identity::{self, Identity, Trusted};
use crate::tls::Tls;
use crate::{Device, Event, Notification, Sftp, Sms, Via, PORT_MAX, PORT_MIN, UDP_PORT};

const PROTOCOL: u32 = 8;
const PAYLOAD_PORTS: std::ops::RangeInclusive<u16> = 1739..=PORT_MAX;
const MAX_LINE: usize = 8 << 20;

/// What a laptop sends, and so what a phone accepts (each side enables a feature when the
/// other lists its packets). `echofiles.*` are EchoConnect's own; stock KDE Connect ignores them.
const LAPTOP_SENDS: &[&str] = &[
    "kdeconnect.battery",
    "kdeconnect.battery.request",
    "kdeconnect.clipboard",
    "kdeconnect.clipboard.connect",
    "kdeconnect.contacts.request_all_uids_timestamps",
    "kdeconnect.contacts.request_vcards_by_uid",
    "kdeconnect.findmyphone.request",
    "kdeconnect.mpris.request",
    "kdeconnect.notification.action",
    "kdeconnect.notification.reply",
    "kdeconnect.notification.request",
    "kdeconnect.ping",
    "kdeconnect.share.request",
    "kdeconnect.sftp.request",
    "kdeconnect.sms.request",
    "kdeconnect.sms.request_attachment",
    "kdeconnect.sms.request_conversation",
    "kdeconnect.sms.request_conversations",
    "kdeconnect.telephony.request_mute",
    "echofiles.call.request",
    "echofiles.capture.request",
    "echofiles.dnd",
    "echofiles.photos.request",
    "echofiles.photos.thumb.request",
    "echofiles.theme",
];
/// What a phone sends, and so what a laptop accepts.
const PHONE_SENDS: &[&str] = &[
    "kdeconnect.battery",
    "kdeconnect.battery.request",
    "kdeconnect.clipboard",
    "kdeconnect.clipboard.connect",
    "kdeconnect.connectivity_report",
    "kdeconnect.contacts.response_uids_timestamps",
    "kdeconnect.contacts.response_vcards",
    "kdeconnect.mpris",
    "kdeconnect.notification",
    "kdeconnect.ping",
    "kdeconnect.share.request",
    "kdeconnect.sftp",
    "kdeconnect.sms.attachment_file",
    "kdeconnect.sms.messages",
    "kdeconnect.telephony",
    "echofiles.backup",
    "echofiles.call",
    "echofiles.capture",
    "echofiles.dnd",
    "echofiles.lock",
    "echofiles.photos",
    "echofiles.photos.thumb",
];

/// Which end of the pair this service is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Laptop,
    Phone,
}

impl Role {
    fn sends(self) -> &'static [&'static str] {
        match self {
            Role::Laptop => LAPTOP_SENDS,
            Role::Phone => PHONE_SENDS,
        }
    }
    fn accepts(self) -> &'static [&'static str] {
        match self {
            Role::Laptop => PHONE_SENDS,
            Role::Phone => LAPTOP_SENDS,
        }
    }
}

/// Packets allowed over Bluetooth: small, and the reason Bluetooth is there.
fn bluetooth_ok(kind: &str) -> bool {
    matches!(kind, "kdeconnect.identity" | "kdeconnect.pair" | "kdeconnect.ping" | "kdeconnect.clipboard" | "kdeconnect.clipboard.connect" | "kdeconnect.telephony" | "kdeconnect.telephony.request_mute")
        || kind.starts_with("echofiles.call")
}

type Tracer = Box<dyn Fn(&str) + Send + Sync>;
static TRACE: std::sync::OnceLock<Option<Tracer>> = std::sync::OnceLock::new();

/// Send a line per packet to `f` (first call wins). Without it, `EF_PHONE_LOG=1` prints to
/// stderr.
pub fn trace_to(f: impl Fn(&str) + Send + Sync + 'static) {
    let _ = TRACE.set(Some(Box::new(f)));
}

fn trace(what: &str) {
    let t = TRACE.get_or_init(|| std::env::var_os("EF_PHONE_LOG").map(|_| Box::new(|l: &str| eprintln!("phone: {l}")) as Tracer));
    if let Some(f) = t {
        f(what);
    }
}

fn short(id: &str) -> &str {
    &id[..id.len().min(8)]
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn packet(kind: &str, body: Value) -> Value {
    json!({ "id": now_ms(), "type": kind, "body": body })
}

fn line(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec(v).unwrap_or_default();
    b.push(b'\n');
    b
}

fn s(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::String(x) => x.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn i(v: &Value, k: &str) -> i64 {
    match &v[k] {
        Value::Number(n) => n.as_i64().unwrap_or(0),
        Value::String(x) => x.parse().unwrap_or(0),
        Value::Bool(b) => i64::from(*b),
        _ => 0,
    }
}

fn b(v: &Value, k: &str) -> bool {
    match &v[k] {
        Value::Bool(x) => *x,
        Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        Value::String(x) => x == "true",
        _ => false,
    }
}

/// Faster notice of a phone that left the Wi-Fi: keepalive after 10 s idle, 3 probes 5 s apart.
fn keepalive(sock: &TcpStream) {
    let fd = sock.as_raw_fd();
    let set = |level: i32, opt: i32, val: i32| {
        // SAFETY: plain setsockopt on a socket we own, with an int-sized value.
        unsafe { libc::setsockopt(fd, level, opt, (&val as *const i32).cast(), std::mem::size_of::<i32>() as u32) };
    };
    set(libc::SOL_SOCKET, libc::SO_KEEPALIVE, 1);
    set(libc::IPPROTO_TCP, libc::TCP_KEEPIDLE, 10);
    set(libc::IPPROTO_TCP, libc::TCP_KEEPINTVL, 5);
    set(libc::IPPROTO_TCP, libc::TCP_KEEPCNT, 3);
}

/// One plain-text line (the identity before TLS), byte by byte so nothing past it is read.
fn read_plain_line(sock: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut one = [0u8; 1];
    loop {
        if sock.read(&mut one)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if one[0] == b'\n' {
            return Ok(out);
        }
        out.push(one[0]);
        if out.len() > 64 * 1024 {
            return Err(io::Error::other("identity too long"));
        }
    }
}

/// A byte stream a link can run on: TCP over Wi-Fi, or a Bluetooth socket.
pub(crate) trait Sock: Read + Write + Send + 'static {
    fn timeout(&self, d: Option<Duration>) -> io::Result<()>;
}

impl Sock for TcpStream {
    fn timeout(&self, d: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(d)
    }
}

impl Sock for UnixStream {
    fn timeout(&self, d: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(d)
    }
}

/// A TLS link, whichever side of the handshake we were.
enum Tls2<S: Sock> {
    Client(StreamOwned<ClientConnection, S>),
    Server(StreamOwned<ServerConnection, S>),
}

impl<S: Sock> Tls2<S> {
    fn sock(&self) -> &S {
        match self {
            Tls2::Client(s) => &s.sock,
            Tls2::Server(s) => &s.sock,
        }
    }
}

impl<S: Sock> Read for Tls2<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Tls2::Client(s) => s.read(buf),
            Tls2::Server(s) => s.read(buf),
        }
    }
}

impl<S: Sock> Write for Tls2<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Tls2::Client(s) => s.write(buf),
            Tls2::Server(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Tls2::Client(s) => s.flush(),
            Tls2::Server(s) => s.flush(),
        }
    }
}

struct Link {
    tx: Sender<Vec<u8>>,
    device: Device,
    cert: Vec<u8>,
    generation: u64,
    via: Via,
}

/// A payload the other side offered and we haven't answered yet.
struct Offer {
    device: String,
    ip: IpAddr,
    port: u16,
    name: String,
    size: i64,
    modified: i64,
}

/// What a payload is read from.
pub enum Source {
    Path(PathBuf),
    /// An open file (an Android content URI's descriptor) and its size.
    File(std::fs::File, u64),
    Bytes(Vec<u8>),
}

struct Inner {
    dir: PathBuf,
    kind: String,
    role: Role,
    me: Identity,
    tls: Tls,
    port: Mutex<u16>,
    emit: Box<dyn Fn(Event) + Send + Sync>,
    /// At most one link per transport per device; Wi-Fi first.
    links: Mutex<HashMap<String, Vec<Link>>>,
    offers: Mutex<HashMap<u64, Offer>>,
    /// Pairing in progress: device → (timestamp, we asked).
    pairing: Mutex<HashMap<String, (u64, bool)>>,
    next: AtomicU64,
    udp: Mutex<Option<UdpSocket>>,
}

/// The running phone service. Cheap to clone.
#[derive(Clone)]
pub struct Service {
    inner: Arc<Inner>,
}

impl Inner {
    fn identity_body(&self) -> Value {
        json!({
            "deviceId": self.me.device_id,
            "deviceName": self.me.name,
            "deviceType": self.kind,
            "protocolVersion": PROTOCOL,
            "incomingCapabilities": self.role.accepts(),
            "outgoingCapabilities": self.role.sends(),
            "tcpPort": *self.port.lock().unwrap(),
        })
    }

    fn identity_line(&self) -> Vec<u8> {
        line(&packet("kdeconnect.identity", self.identity_body()))
    }

    fn send(&self, id: &str, kind: &str, body: Value) -> bool {
        self.send_raw(id, kind, line(&packet(kind, body)))
    }

    /// Over Wi-Fi when there's a link, otherwise Bluetooth for the packets it carries.
    fn send_raw(&self, id: &str, kind: &str, bytes: Vec<u8>) -> bool {
        let links = self.links.lock().unwrap();
        let Some(ls) = links.get(id) else { return false };
        let link = ls.iter().find(|l| l.via == Via::Lan).or_else(|| ls.iter().find(|l| l.via == Via::Bluetooth && bluetooth_ok(kind)));
        let ok = link.is_some_and(|l| l.tx.send(bytes).is_ok());
        if ok {
            trace(&format!("→ {} {kind}", short(id)));
        }
        ok
    }

    fn paired(&self, id: &str) -> bool {
        self.links.lock().unwrap().get(id).is_some_and(|ls| ls.iter().any(|l| l.device.paired))
    }

    /// The device as the app sees it: details from the newest link, reachable over every live one.
    fn device(&self, id: &str) -> Option<Device> {
        let links = self.links.lock().unwrap();
        let ls = links.get(id)?;
        let mut d = ls.iter().find(|l| l.via == Via::Lan).or(ls.first())?.device.clone();
        d.lan = ls.iter().any(|l| l.via == Via::Lan);
        d.bluetooth = ls.iter().any(|l| l.via == Via::Bluetooth);
        d.paired = ls.iter().any(|l| l.device.paired);
        Some(d)
    }

    fn cert(&self, id: &str) -> Option<Vec<u8>> {
        self.links.lock().unwrap().get(id).and_then(|ls| ls.first().map(|l| l.cert.clone()))
    }

    fn new_id(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
}

impl Service {
    /// Start listening. `emit` is called from service threads.
    pub fn start(emit: impl Fn(Event) + Send + Sync + 'static) -> io::Result<Service> {
        Service::start_in(identity::dir(), None, emit)
    }

    /// Start with the identity and pairings kept in `dir`, under `name` (default: the host
    /// name). Two of these can talk to each other on one machine (the tests do).
    pub fn start_in(dir: PathBuf, name: Option<String>, emit: impl Fn(Event) + Send + Sync + 'static) -> io::Result<Service> {
        Service::start_as(dir, name, "laptop", emit)
    }

    /// Like [`Service::start_in`], announcing itself as `kind` ("phone" for a stand-in phone
    /// in tests and demos).
    pub fn start_as(dir: PathBuf, name: Option<String>, kind: &str, emit: impl Fn(Event) + Send + Sync + 'static) -> io::Result<Service> {
        let mut me = identity::load_or_create(&dir)?;
        if let Some(n) = name {
            me.name = n;
        }
        let tls = Tls::new(&me).map_err(|e| io::Error::other(e.to_string()))?;
        let role = if matches!(kind, "phone" | "tablet") { Role::Phone } else { Role::Laptop };
        let inner = Arc::new(Inner {
            dir,
            kind: kind.to_string(),
            role,
            me,
            tls,
            port: Mutex::new(UDP_PORT),
            emit: Box::new(emit),
            links: Mutex::new(HashMap::new()),
            offers: Mutex::new(HashMap::new()),
            pairing: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            udp: Mutex::new(None),
        });
        // TCP: 1716 first (what phones try), then the rest of the range.
        let listener = std::iter::once(UDP_PORT).chain(PORT_MIN..=PORT_MAX).find_map(|p| TcpListener::bind((Ipv4Addr::UNSPECIFIED, p)).ok());
        let Some(listener) = listener else {
            let why = "Ports 1714–1764 are all in use. If KDE Connect is running on this computer, quit it — EchoFiles talks to your phone itself.".to_string();
            (inner.emit)(Event::Failed(why.clone()));
            return Err(io::Error::other(why));
        };
        let port = listener.local_addr()?.port();
        *inner.port.lock().unwrap() = port;
        (inner.emit)(Event::Started { port });

        let acc = inner.clone();
        std::thread::Builder::new().name("phone-accept".into()).spawn(move || {
            for conn in listener.incoming().flatten() {
                let i = acc.clone();
                std::thread::spawn(move || {
                    let _ = incoming(&i, conn);
                });
            }
        })?;

        // UDP: hear phones announce themselves. Port 1716 may be taken (another KDE
        // Connect); we still announce ourselves and accept their connections.
        match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, UDP_PORT)) {
            Ok(sock) => {
                let _ = sock.set_broadcast(true);
                *inner.udp.lock().unwrap() = sock.try_clone().ok();
                let i = inner.clone();
                std::thread::Builder::new().name("phone-udp".into()).spawn(move || listen_udp(&i, sock))?;
            }
            Err(_) => {
                if let Ok(sock) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
                    let _ = sock.set_broadcast(true);
                    *inner.udp.lock().unwrap() = Some(sock);
                }
            }
        }
        let svc = Service { inner };
        svc.announce();
        // Say we're here again every 15 s while a paired device isn't connected (a phone
        // that came back to the Wi-Fi, or a laptop that woke up).
        let again = svc.clone();
        std::thread::Builder::new().name("phone-announce".into()).spawn(move || loop {
            std::thread::sleep(Duration::from_secs(15));
            let trusted = identity::trusted(&again.inner.dir);
            let links = again.inner.links.lock().unwrap();
            let missing = trusted.iter().any(|t| !links.get(&t.id).is_some_and(|ls| ls.iter().any(|l| l.via == Via::Lan)));
            drop(links);
            if missing {
                again.announce();
            }
        })?;
        Ok(svc)
    }

    pub fn device_id(&self) -> &str {
        &self.inner.me.device_id
    }

    pub fn port(&self) -> u16 {
        *self.inner.port.lock().unwrap()
    }

    /// Say we're here to the whole network (phones with the app open connect back).
    pub fn announce(&self) {
        self.announce_to(SocketAddr::from((Ipv4Addr::BROADCAST, UDP_PORT)));
    }

    /// Say we're here to one address (for networks that drop broadcasts).
    pub fn announce_to(&self, to: SocketAddr) {
        let bytes = self.inner.identity_line();
        if let Some(sock) = self.inner.udp.lock().unwrap().as_ref() {
            let _ = sock.send_to(&bytes, to);
        }
    }

    /// Connect straight to a phone's KDE Connect port (1716 unless it says otherwise) —
    /// the way in when broadcasts don't get through.
    pub fn connect_to(&self, to: SocketAddr) {
        let inner = self.inner.clone();
        std::thread::spawn(move || {
            let _ = outgoing(&inner, to);
        });
    }

    pub fn role(&self) -> Role {
        self.inner.role
    }

    pub fn name(&self) -> &str {
        &self.inner.me.name
    }

    pub fn devices(&self) -> Vec<Device> {
        let ids: Vec<String> = self.inner.links.lock().unwrap().keys().cloned().collect();
        ids.iter().filter_map(|id| self.inner.device(id)).collect()
    }

    /// Run a link over a Bluetooth socket (RFCOMM). The side that opened the connection is
    /// the `initiator`, like the side that heard the UDP announcement over Wi-Fi.
    pub fn adopt(&self, fd: OwnedFd, initiator: bool) {
        let inner = self.inner.clone();
        std::thread::spawn(move || {
            let sock = UnixStream::from(fd);
            let ip = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
            let r = if initiator { outgoing_on(&inner, sock, ip, Via::Bluetooth) } else { incoming_on(&inner, sock, ip, Via::Bluetooth) };
            if let Err(e) = r {
                trace(&format!("bluetooth link: {e}"));
            }
        });
    }

    // ------------------------------------------------------------------ pairing

    /// Ask the phone to pair; its owner confirms the code there.
    pub fn pair(&self, id: &str) {
        let ts = (now_ms() / 1000) as u64;
        let Some(cert) = self.inner.cert(id) else { return };
        let code = identity::verification_key(&self.inner.me.cert, &cert, ts);
        self.inner.pairing.lock().unwrap().insert(id.to_string(), (ts, true));
        self.inner.send(id, "kdeconnect.pair", json!({ "pair": true, "timestamp": ts }));
        (self.inner.emit)(Event::PairCode { id: id.to_string(), code });
    }

    pub fn accept_pair(&self, id: &str) {
        if self.inner.pairing.lock().unwrap().remove(id).is_none() {
            return;
        }
        self.inner.send(id, "kdeconnect.pair", json!({ "pair": true }));
        complete_pair(&self.inner, id);
    }

    pub fn reject_pair(&self, id: &str) {
        self.inner.pairing.lock().unwrap().remove(id);
        self.inner.send(id, "kdeconnect.pair", json!({ "pair": false }));
    }

    /// Forget the phone on both sides.
    pub fn unpair(&self, id: &str) {
        self.inner.send(id, "kdeconnect.pair", json!({ "pair": false }));
        forget(&self.inner, id);
    }

    // ------------------------------------------------------------------ requests

    fn paired_send(&self, id: &str, kind: &str, body: Value) -> bool {
        self.inner.paired(id) && self.inner.send(id, kind, body)
    }

    /// Send any packet to a paired device (stand-in phones use this for battery,
    /// notifications and texts).
    pub fn send_packet(&self, id: &str, kind: &str, body: Value) -> bool {
        self.paired_send(id, kind, body)
    }

    /// Ring the phone at full volume (again to stop it).
    pub fn ring(&self, id: &str) -> bool {
        self.paired_send(id, "kdeconnect.findmyphone.request", json!({}))
    }

    pub fn request_battery(&self, id: &str) -> bool {
        self.paired_send(id, "kdeconnect.battery.request", json!({ "request": true }))
    }

    /// `sensitive` (a password manager's copy) asks the other side not to keep it in history.
    pub fn send_clipboard(&self, id: &str, text: &str, on_connect: bool, sensitive: bool) -> bool {
        let mut body = if on_connect { json!({ "content": text, "timestamp": now_ms() }) } else { json!({ "content": text }) };
        if sensitive {
            body["sensitive"] = json!(true);
        }
        self.paired_send(id, if on_connect { "kdeconnect.clipboard.connect" } else { "kdeconnect.clipboard" }, body)
    }

    pub fn request_notifications(&self, id: &str) -> bool {
        self.paired_send(id, "kdeconnect.notification.request", json!({ "request": true }))
    }

    pub fn dismiss_notification(&self, id: &str, key: &str) -> bool {
        self.paired_send(id, "kdeconnect.notification.request", json!({ "cancel": key }))
    }

    pub fn reply_notification(&self, id: &str, reply_id: &str, message: &str) -> bool {
        self.paired_send(id, "kdeconnect.notification.reply", json!({ "requestReplyId": reply_id, "message": message }))
    }

    /// The newest message of every conversation.
    pub fn request_conversations(&self, id: &str) -> bool {
        self.paired_send(id, "kdeconnect.sms.request_conversations", json!({}))
    }

    pub fn request_thread(&self, id: &str, thread: i64) -> bool {
        self.paired_send(id, "kdeconnect.sms.request_conversation", json!({ "threadID": thread, "numberToRequest": 100 }))
    }

    pub fn send_sms(&self, id: &str, addresses: &[String], text: &str) -> bool {
        let addrs: Vec<Value> = addresses.iter().map(|a| json!({ "address": a })).collect();
        let phone = addresses.first().cloned().unwrap_or_default();
        self.paired_send(id, "kdeconnect.sms.request", json!({ "version": 2, "sendSms": true, "addresses": addrs, "phoneNumber": phone, "messageBody": text }))
    }

    /// Start the phone's SFTP server; the answer arrives as [`Event::Sftp`].
    pub fn request_sftp(&self, id: &str) -> bool {
        self.paired_send(id, "kdeconnect.sftp.request", json!({ "startBrowsing": true }))
    }

    /// Send a link or some text to the phone.
    pub fn share_text(&self, id: &str, text: &str) -> bool {
        let is_url = text.starts_with("http://") || text.starts_with("https://");
        self.paired_send(id, "kdeconnect.share.request", if is_url { json!({ "url": text }) } else { json!({ "text": text }) })
    }

    // ------------------------------------------------------------------ files

    /// Save an offered file into `dir` (a free name is picked if one's taken).
    pub fn accept_file(&self, transfer: u64, dir: PathBuf) {
        let Some(o) = self.inner.offers.lock().unwrap().remove(&transfer) else { return };
        let inner = self.inner.clone();
        std::thread::spawn(move || download(&inner, transfer, o, &dir));
    }

    pub fn reject_file(&self, transfer: u64) {
        self.inner.offers.lock().unwrap().remove(&transfer);
    }

    /// Send files to the phone, one after another. Returns the transfer ids, in order.
    pub fn send_files(&self, id: &str, files: Vec<PathBuf>) -> Vec<u64> {
        if !self.inner.paired(id) {
            return Vec::new();
        }
        let ids: Vec<u64> = files.iter().map(|_| self.inner.new_id()).collect();
        let inner = self.inner.clone();
        let device = id.to_string();
        let work: Vec<(u64, PathBuf)> = ids.iter().copied().zip(files).collect();
        std::thread::spawn(move || {
            let total: u64 = work.iter().filter_map(|(_, p)| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
            let n = work.len();
            for (t, path) in work {
                let body = share_body(&path, n, total);
                let result = upload(&inner, &device, t, "kdeconnect.share.request", body, Source::Path(path.clone()));
                (inner.emit)(Event::TransferDone { id: device.clone(), transfer: t, upload: true, result: result.map(|_| path) });
            }
        });
        ids
    }

    /// Send one payload with any packet (`share.request`, `echofiles.photos.thumb`…).
    /// `body` should carry `filename` for the other side's progress. Returns the transfer id.
    pub fn send_payload(&self, id: &str, kind: &str, body: Value, source: Source) -> Option<u64> {
        if !self.inner.paired(id) {
            return None;
        }
        let t = self.inner.new_id();
        let inner = self.inner.clone();
        let device = id.to_string();
        let kind = kind.to_string();
        std::thread::spawn(move || {
            let label = match &source {
                Source::Path(p) => p.clone(),
                _ => PathBuf::from(body["filename"].as_str().unwrap_or_default()),
            };
            let result = upload(&inner, &device, t, &kind, body, source);
            (inner.emit)(Event::TransferDone { id: device.clone(), transfer: t, upload: true, result: result.map(|_| label) });
        });
        Some(t)
    }
}

fn share_body(path: &Path, count: usize, total: u64) -> Value {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let modified = std::fs::metadata(path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64);
    json!({ "filename": name, "lastModified": modified, "numberOfFiles": count, "totalPayloadSize": total })
}

// ------------------------------------------------------------------------------ discovery

fn listen_udp(inner: &Arc<Inner>, sock: UdpSocket) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let Ok((n, from)) = sock.recv_from(&mut buf) else { continue };
        let Ok(v) = serde_json::from_slice::<Value>(buf[..n].trim_ascii()) else { continue };
        if v["type"] != "kdeconnect.identity" {
            continue;
        }
        let body = &v["body"];
        let id = s(body, "deviceId");
        let has_lan = inner.links.lock().unwrap().get(&id).is_some_and(|ls| ls.iter().any(|l| l.via == Via::Lan));
        if id.is_empty() || id == inner.me.device_id || has_lan {
            continue;
        }
        let port = i(body, "tcpPort");
        if !(1..=65535).contains(&port) {
            continue;
        }
        let i2 = inner.clone();
        std::thread::spawn(move || {
            let _ = outgoing(&i2, SocketAddr::new(from.ip(), port as u16));
        });
    }
}

/// We heard the phone: connect, introduce ourselves, then TLS as the server.
fn outgoing(inner: &Arc<Inner>, to: SocketAddr) -> io::Result<()> {
    let sock = TcpStream::connect_timeout(&to, Duration::from_secs(5))?;
    sock.set_nodelay(true)?;
    keepalive(&sock);
    outgoing_on(inner, sock, to.ip(), Via::Lan)
}

fn outgoing_on<S: Sock>(inner: &Arc<Inner>, mut sock: S, ip: IpAddr, via: Via) -> io::Result<()> {
    sock.write_all(&inner.identity_line())?;
    let mut conn = ServerConnection::new(inner.tls.server.clone()).map_err(io::Error::other)?;
    sock.timeout(Some(Duration::from_secs(10)))?;
    while conn.is_handshaking() {
        conn.complete_io(&mut sock)?;
    }
    let cert = conn.peer_certificates().and_then(|c| c.first()).map(|c| c.to_vec()).unwrap_or_default();
    run_link(inner, Tls2::Server(StreamOwned::new(conn, sock)), ip, cert, None, via)
}

/// The phone connected to us: read its identity, then TLS as the client.
fn incoming(inner: &Arc<Inner>, sock: TcpStream) -> io::Result<()> {
    let ip = sock.peer_addr()?.ip();
    sock.set_nodelay(true)?;
    keepalive(&sock);
    incoming_on(inner, sock, ip, Via::Lan)
}

fn incoming_on<S: Sock>(inner: &Arc<Inner>, mut sock: S, ip: IpAddr, via: Via) -> io::Result<()> {
    sock.timeout(Some(Duration::from_secs(10)))?;
    let first = read_plain_line(&mut sock)?;
    let v: Value = serde_json::from_slice(&first).map_err(io::Error::other)?;
    if v["type"] != "kdeconnect.identity" || s(&v["body"], "deviceId") == inner.me.device_id {
        return Ok(());
    }
    let name = ServerName::IpAddress(ip.into());
    let mut conn = ClientConnection::new(inner.tls.client.clone(), name).map_err(io::Error::other)?;
    while conn.is_handshaking() {
        conn.complete_io(&mut sock)?;
    }
    let cert = conn.peer_certificates().and_then(|c| c.first()).map(|c| c.to_vec()).unwrap_or_default();
    run_link(inner, Tls2::Client(StreamOwned::new(conn, sock)), ip, cert, Some(v["body"].clone()), via)
}

fn device_from(body: &Value, ip: IpAddr, cert: &[u8], trusted: &[Trusted]) -> Device {
    let id = s(body, "deviceId");
    let paired = trusted.iter().any(|t| t.id == id && t.cert == cert);
    let list = |k: &str| match &body[k] {
        Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    };
    let kind = match s(body, "deviceType") {
        k if k.is_empty() => "phone".to_string(),
        k => k,
    };
    Device { id, name: s(body, "deviceName"), kind, ip, paired, accepts: list("incomingCapabilities"), sends: list("outgoingCapabilities"), lan: false, bluetooth: false }
}

// ------------------------------------------------------------------------------ links

fn run_link<S: Sock>(inner: &Arc<Inner>, mut stream: Tls2<S>, ip: IpAddr, cert: Vec<u8>, plain_identity: Option<Value>, via: Via) -> io::Result<()> {
    // Protocol 8: identities again, inside TLS.
    stream.write_all(&inner.identity_line())?;
    stream.flush()?;
    stream.sock().timeout(Some(Duration::from_millis(100)))?;

    let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();
    let generation = inner.new_id();
    let mut device: Option<Device> = None;
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = vec![0u8; 64 * 1024];
    let started = Instant::now();
    // Older peers don't repeat the identity inside TLS: fall back to the plain one.
    let mut pending_plain = plain_identity;

    let register = |d: &Device, tx: &Sender<Vec<u8>>| {
        let mut links = inner.links.lock().unwrap();
        let ls = links.entry(d.id.clone()).or_default();
        let fresh = !ls.iter().any(|l| l.device.paired);
        // A newer link replaces an older one on the same transport (its thread ends when
        // its sender drops).
        ls.retain(|l| l.via != via);
        ls.push(Link { tx: tx.clone(), device: d.clone(), cert: cert.clone(), generation, via });
        ls.sort_by_key(|l| l.via != Via::Lan);
        drop(links);
        trace(&format!("link {} {:?} {} paired={}", short(&d.id), via, d.name, d.paired));
        if let Some(full) = inner.device(&d.id) {
            (inner.emit)(Event::Device(full));
        }
        if d.paired && (fresh || via == Via::Lan) {
            on_paired_link(inner, &d.id);
        }
    };

    let result = loop {
        // Outgoing first.
        let mut closed = false;
        loop {
            match rx.try_recv() {
                Ok(bytes) => {
                    if stream.write_all(&bytes).is_err() {
                        closed = true;
                        break;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    closed = device.is_some();
                    break;
                }
            }
        }
        if closed {
            break Ok(());
        }
        let _ = stream.flush();
        if device.is_none() && started.elapsed() > Duration::from_secs(3)
            && let Some(body) = pending_plain.take() {
                let d = device_from(&body, ip, &cert, &identity::trusted(&inner.dir));
                register(&d, &tx);
                device = Some(d);
            }
        match stream.read(&mut tmp) {
            Ok(0) => break Ok(()),
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.len() > MAX_LINE {
                    break Err(io::Error::other("packet too large"));
                }
                while let Some(pos) = buf.iter().position(|&c| c == b'\n') {
                    let raw: Vec<u8> = buf.drain(..=pos).collect();
                    let Ok(v) = serde_json::from_slice::<Value>(raw.trim_ascii()) else { continue };
                    if v["type"] == "kdeconnect.identity" {
                        let d = device_from(&v["body"], ip, &cert, &identity::trusted(&inner.dir));
                        if d.id.is_empty() || d.id == inner.me.device_id {
                            return Ok(());
                        }
                        pending_plain = None;
                        register(&d, &tx);
                        device = Some(d);
                        continue;
                    }
                    if let Some(d) = &device {
                        trace(&format!("← {} {}", short(&d.id), v["type"].as_str().unwrap_or("?")));
                        handle(inner, &d.id, ip, &v);
                    }
                }
            }
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted) => {}
            Err(e) => break Err(e),
        }
    };
    if let Some(d) = device {
        let mut links = inner.links.lock().unwrap();
        let mut gone = false;
        if let Some(ls) = links.get_mut(&d.id) {
            let before = ls.len();
            ls.retain(|l| l.generation != generation);
            if ls.len() < before && ls.is_empty() {
                links.remove(&d.id);
                gone = true;
            }
        }
        drop(links);
        trace(&format!("link closed {} {:?}", short(&d.id), via));
        if gone {
            inner.pairing.lock().unwrap().remove(&d.id);
            (inner.emit)(Event::Gone(d.id));
        } else if let Some(full) = inner.device(&d.id) {
            (inner.emit)(Event::Device(full));
        }
    }
    result
}

/// A paired phone connected: ask for what the app shows.
fn on_paired_link(inner: &Arc<Inner>, id: &str) {
    inner.send(id, "kdeconnect.battery.request", json!({ "request": true }));
    if inner.role == Role::Laptop {
        inner.send(id, "kdeconnect.notification.request", json!({ "request": true }));
    }
    (inner.emit)(Event::Ready(id.to_string()));
}

fn complete_pair(inner: &Arc<Inner>, id: &str) {
    {
        let mut links = inner.links.lock().unwrap();
        let Some(ls) = links.get_mut(id) else { return };
        let Some(l) = ls.first() else { return };
        let t = Trusted { id: id.to_string(), name: l.device.name.clone(), kind: l.device.kind.clone(), cert: l.cert.clone() };
        if identity::trust(&inner.dir, &t).is_err() {
            return;
        }
        for l in ls.iter_mut() {
            l.device.paired = true;
        }
    }
    let Some(dev) = inner.device(id) else { return };
    (inner.emit)(Event::Paired(dev));
    on_paired_link(inner, id);
}

fn forget(inner: &Arc<Inner>, id: &str) {
    let _ = identity::forget(&inner.dir, id);
    inner.pairing.lock().unwrap().remove(id);
    if let Some(ls) = inner.links.lock().unwrap().get_mut(id) {
        for l in ls.iter_mut() {
            l.device.paired = false;
        }
    }
    (inner.emit)(Event::Unpaired(id.to_string()));
}

fn handle(inner: &Arc<Inner>, id: &str, ip: IpAddr, v: &Value) {
    let kind = v["type"].as_str().unwrap_or_default();
    let body = &v["body"];
    let emit = |e: Event| (inner.emit)(e);
    if kind == "kdeconnect.pair" {
        let wants = b(body, "pair");
        let asked = inner.pairing.lock().unwrap().get(id).copied();
        match (wants, asked) {
            (true, Some((_, true))) => {
                inner.pairing.lock().unwrap().remove(id);
                complete_pair(inner, id);
            }
            (true, _) => {
                if inner.paired(id) {
                    // It forgot us but we didn't: confirm.
                    inner.send(id, "kdeconnect.pair", json!({ "pair": true }));
                    return;
                }
                let ts = i(body, "timestamp").max(0) as u64;
                let Some(cert) = inner.cert(id) else { return };
                let code = identity::verification_key(&inner.me.cert, &cert, ts);
                inner.pairing.lock().unwrap().insert(id.to_string(), (ts, false));
                emit(Event::PairRequested { id: id.to_string(), code });
            }
            (false, Some(_)) => {
                inner.pairing.lock().unwrap().remove(id);
                emit(Event::PairRejected(id.to_string()));
            }
            (false, None) => {
                if inner.paired(id) || identity::trusted(&inner.dir).iter().any(|t| t.id == id) {
                    forget(inner, id);
                }
            }
        }
        return;
    }
    if !inner.paired(id) {
        return;
    }
    let id = id.to_string();
    match kind {
        "kdeconnect.battery" => emit(Event::Battery { id, level: i(body, "currentCharge") as i32, charging: b(body, "isCharging") }),
        "kdeconnect.clipboard" | "kdeconnect.clipboard.connect" => {
            let text = s(body, "content");
            if !text.is_empty() {
                emit(Event::Clipboard { id, text, sensitive: b(body, "sensitive") });
            }
        }
        "kdeconnect.notification" => {
            let key = s(body, "id");
            if b(body, "isCancel") {
                emit(Event::NotificationGone { id, key });
                return;
            }
            let reply = s(body, "requestReplyId");
            let n = Notification {
                key,
                app: s(body, "appName"),
                title: s(body, "title"),
                text: match s(body, "text") {
                    t if t.is_empty() => s(body, "ticker"),
                    t => t,
                },
                time: i(body, "time"),
                dismissable: body.get("isClearable").is_none_or(|_| b(body, "isClearable")),
                silent: b(body, "silent"),
                reply_id: (!reply.is_empty()).then_some(reply),
            };
            emit(Event::Notification { id, notification: n });
        }
        "kdeconnect.sms.messages" => {
            let list = match &body["messages"] {
                Value::Array(a) => a.clone(),
                _ => Vec::new(),
            };
            let messages = list
                .iter()
                .map(|m| {
                    let address = match &m["addresses"] {
                        Value::Array(a) => a.iter().map(|x| s(x, "address")).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", "),
                        _ => s(m, "address"),
                    };
                    Sms { thread: i(m, "thread_id"), uid: i(m, "_id"), address, body: s(m, "body"), date: i(m, "date"), outgoing: i(m, "type") == 2, read: i(m, "read") != 0 }
                })
                .collect();
            emit(Event::Sms { id, messages });
        }
        "kdeconnect.sftp" => {
            let err = s(body, "errorMessage");
            let result = if !err.is_empty() {
                Err(err)
            } else {
                let paths: Vec<String> = match &body["multiPaths"] {
                    Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                    _ => Vec::new(),
                };
                let names: Vec<String> = match &body["pathNames"] {
                    Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                    _ => Vec::new(),
                };
                let ip_s = match s(body, "ip") {
                    x if x.is_empty() => ip.to_string(),
                    x => x,
                };
                Ok(Sftp { ip: ip_s, port: i(body, "port") as u16, user: s(body, "user"), password: s(body, "password"), path: s(body, "path"), places: paths.into_iter().zip(names).collect() })
            };
            emit(Event::Sftp { id, result });
        }
        "kdeconnect.share.request" if v.get("payloadTransferInfo").is_none() => {
            if body.get("text").is_some() {
                emit(Event::Text { id, text: s(body, "text") });
            } else if body.get("url").is_some() {
                emit(Event::Url { id, url: s(body, "url") });
            }
        }
        _ => {
            // Anything carrying a payload (a shared file, a thumbnail, an MMS attachment, a
            // photo taken for the laptop) is offered; the app accepts it into a folder.
            let port = i(&v["payloadTransferInfo"], "port");
            if v.get("payloadTransferInfo").is_some() && (1..=65535).contains(&port) {
                let size = v["payloadSize"].as_i64().unwrap_or(-1);
                let name = match s(body, "filename") {
                    n if n.is_empty() => kind.to_string(),
                    n => n,
                };
                let transfer = inner.new_id();
                inner.offers.lock().unwrap().insert(transfer, Offer { device: id.clone(), ip, port: port as u16, name: name.clone(), size, modified: i(body, "lastModified") });
                emit(Event::Incoming { id, transfer, name, size: size.max(0) as u64, kind: kind.to_string(), body: body.clone() });
            } else {
                emit(Event::Other { id, kind: kind.to_string(), body: body.clone() });
            }
        }
    }
}

// ------------------------------------------------------------------------------ payloads

/// `name`, or `name (2).ext`… — never overwrite.
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let clean: String = Path::new(name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let clean = if clean.is_empty() || clean == "." || clean == ".." { "file".to_string() } else { clean };
    let first = dir.join(&clean);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match clean.rfind('.') {
        Some(p) if p > 0 => (&clean[..p], &clean[p..]),
        _ => (clean.as_str(), ""),
    };
    (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).unwrap_or(first)
}

fn download(inner: &Arc<Inner>, transfer: u64, o: Offer, dir: &Path) {
    let done = Arc::new(AtomicU64::new(0));
    (inner.emit)(Event::TransferStarted { id: o.device.clone(), transfer, name: o.name.clone(), size: o.size.max(0) as u64, upload: false, done: done.clone() });
    let result = (|| -> io::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let mut sock = TcpStream::connect_timeout(&SocketAddr::new(o.ip, o.port), Duration::from_secs(10))?;
        sock.set_read_timeout(Some(Duration::from_secs(30)))?;
        let mut conn = ClientConnection::new(inner.tls.client.clone(), ServerName::IpAddress(o.ip.into())).map_err(io::Error::other)?;
        while conn.is_handshaking() {
            conn.complete_io(&mut sock)?;
        }
        let peer = conn.peer_certificates().and_then(|c| c.first()).map(|c| c.to_vec()).unwrap_or_default();
        let expected = inner.cert(&o.device);
        if expected.as_deref() != Some(peer.as_slice()) {
            return Err(io::Error::other("The phone's certificate changed; not saving the file."));
        }
        let mut stream = StreamOwned::new(conn, sock);
        let target = free_name(dir, &o.name);
        let part = target.with_file_name(format!(".{}.part", target.file_name().unwrap_or_default().to_string_lossy()));
        let mut file = std::fs::File::create(&part)?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut got: u64 = 0;
        loop {
            if o.size >= 0 && got >= o.size as u64 {
                break;
            }
            let n = match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                // The phone closes without a TLS close_notify.
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => {
                    let _ = std::fs::remove_file(&part);
                    return Err(e);
                }
            };
            let n = if o.size >= 0 { n.min((o.size as u64 - got) as usize) } else { n };
            file.write_all(&buf[..n])?;
            got += n as u64;
            done.store(got, Ordering::Relaxed);
        }
        file.sync_all()?;
        if o.size >= 0 && got < o.size as u64 {
            let _ = std::fs::remove_file(&part);
            return Err(io::Error::other("The phone stopped sending before the file was complete."));
        }
        if o.modified > 0 {
            let t = SystemTime::UNIX_EPOCH + Duration::from_millis(o.modified as u64);
            let _ = file.set_modified(t);
        }
        drop(file);
        std::fs::rename(&part, &target)?;
        Ok(target)
    })();
    (inner.emit)(Event::TransferDone { id: o.device, transfer, upload: false, result: result.map_err(|e| e.to_string()) });
}

fn upload(inner: &Arc<Inner>, device: &str, transfer: u64, kind: &str, body: Value, source: Source) -> Result<(), String> {
    let (mut reader, size): (Box<dyn Read + Send>, u64) = match source {
        Source::Path(path) => {
            let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            if !meta.is_file() {
                return Err("Only files can be sent, not folders.".into());
            }
            (Box::new(std::fs::File::open(&path).map_err(|e| e.to_string())?), meta.len())
        }
        Source::File(f, size) => (Box::new(f), size),
        Source::Bytes(b) => {
            let n = b.len() as u64;
            (Box::new(io::Cursor::new(b)), n)
        }
    };
    let name = body["filename"].as_str().unwrap_or(kind).to_string();
    let listener = PAYLOAD_PORTS.clone().find_map(|p| TcpListener::bind((Ipv4Addr::UNSPECIFIED, p)).ok()).ok_or("No free port to send from (1739–1764).")?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let mut p = packet(kind, body);
    p["payloadSize"] = json!(size);
    p["payloadTransferInfo"] = json!({ "port": port });
    // Payloads need Wi-Fi: the receiver connects back over TCP.
    let has_lan = inner.links.lock().unwrap().get(device).is_some_and(|ls| ls.iter().any(|l| l.via == Via::Lan));
    if !has_lan || !inner.send_raw(device, kind, line(&p)) {
        return Err("Not connected over Wi-Fi.".into());
    }
    let done = Arc::new(AtomicU64::new(0));
    (inner.emit)(Event::TransferStarted { id: device.to_string(), transfer, name, size, upload: true, done: done.clone() });
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut sock = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err("The other device didn't pick up the file.".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    sock.set_nonblocking(false).map_err(|e| e.to_string())?;
    sock.set_read_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
    sock.set_write_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
    let mut conn = ServerConnection::new(inner.tls.server.clone()).map_err(|e| e.to_string())?;
    while conn.is_handshaking() {
        conn.complete_io(&mut sock).map_err(|e| e.to_string())?;
    }
    let mut stream = StreamOwned::new(conn, sock);
    let mut buf = vec![0u8; 256 * 1024];
    let mut sent = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        stream.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        sent += n as u64;
        done.store(sent, Ordering::Relaxed);
    }
    stream.flush().map_err(|e| e.to_string())?;
    stream.conn.send_close_notify();
    let _ = stream.flush();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_names() {
        let d = std::env::temp_dir().join(format!("ef-phone-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.txt"), b"x").unwrap();
        assert_eq!(free_name(&d, "a.txt"), d.join("a (2).txt"));
        assert_eq!(free_name(&d, "../../etc/passwd"), d.join("passwd"));
        assert_eq!(free_name(&d, ".."), d.join("file"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn field_readers() {
        let v = json!({ "a": "5", "b": 7, "c": true, "d": "true" });
        assert_eq!(i(&v, "a"), 5);
        assert_eq!(s(&v, "b"), "7");
        assert!(b(&v, "c") && b(&v, "d"));
        assert_eq!(i(&v, "missing"), 0);
    }
}

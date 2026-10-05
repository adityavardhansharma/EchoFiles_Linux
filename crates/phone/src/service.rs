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

use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
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
    "echofiles.clipboard.image.inline", "echofiles.clipboard.image", "echofiles.files.list", "echofiles.files.read", "echofiles.files.write", "echofiles.settings", "echofiles.call", "echofiles.backup", "echofiles.backup.ack", "echofiles.photos.read", "kdeconnect.mpris.request",
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
    "echofiles.clipboard.image.inline", "echofiles.clipboard.image", "echofiles.files.list", "echofiles.files.read", "echofiles.files.write", "echofiles.settings", "echofiles.call", "echofiles.backup", "echofiles.backup.ack", "echofiles.photos.read", "kdeconnect.mpris.request",
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
    "echofiles.backup", "echofiles.backup.ack",
    "echofiles.call",
    "echofiles.capture",
    "echofiles.dnd",
    "echofiles.lock",
    "echofiles.photos",
    "echofiles.photos.thumb",
    "echofiles.photos.file",
    "echofiles.photos.error",
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
        || kind.starts_with("echofiles.call") || kind == "echofiles.clipboard.image.inline"
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
    &id[..id.char_indices().nth(8).map_or(id.len(), |(i, _)| i)]
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
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut out = Vec::new();
    let mut one = [0u8; 1];
    loop {
        if Instant::now() > deadline { return Err(io::Error::new(io::ErrorKind::TimedOut, "identity deadline")); }
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
        self.set_write_timeout(d)?;
        self.set_read_timeout(d)
    }
}

impl Sock for UnixStream {
    fn timeout(&self, d: Option<Duration>) -> io::Result<()> {
        self.set_write_timeout(d)?;
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
    revoked: Arc<AtomicBool>,
    tx: SyncSender<Vec<u8>>,
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
    generation: u64,
    created: Instant,
    cert: Vec<u8>,
}

/// What a payload is read from.
pub enum Source {
    Path(PathBuf),
    /// An open file (an Android content URI's descriptor) and its size.
    File(std::fs::File, u64),
    Bytes(Vec<u8>),
}

#[derive(Clone)]
struct PendingPair {
    cert: Vec<u8>,
    generation: u64,
    created: Instant,
    asked: bool,
    timestamp: u64,
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
    transfers: Mutex<HashMap<u64, Arc<AtomicBool>>>,
    /// Pairing in progress: device → (timestamp, we asked).
    pairing: Mutex<HashMap<String, PendingPair>>,
    qr: Mutex<HashMap<String, (String, Instant)>>,
    clips: Mutex<HashMap<String, i64>>,
    next: AtomicU64,
    udp: Mutex<Option<UdpSocket>>,
    connections: Arc<AtomicUsize>,
    transfer_slots: Arc<AtomicUsize>,
    pending: Mutex<HashSet<SocketAddr>>,
    stopped: AtomicBool,
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
        let allowed = |l: &&Link| l.device.paired || kind == "kdeconnect.pair";
        let link = ls.iter().filter(allowed).find(|l| l.via == Via::Lan).or_else(|| ls.iter().filter(allowed).find(|l| l.via == Via::Bluetooth && bluetooth_ok(kind)));
        let ok = link.is_some_and(|l| l.tx.try_send(bytes).is_ok());
        if ok {
            trace(&format!("→ {} {kind}", short(id)));
        }
        ok
    }

    fn authorization(&self, id: &str) -> Option<(Vec<u8>, u64)> {
        self.links.lock().unwrap().get(id)?.iter().find(|l| l.device.paired && l.via == Via::Lan).map(|l| (l.cert.clone(), l.generation))
    }

    fn authorized(&self, id: &str, cert: &[u8], generation: u64) -> bool {
        if self.stopped.load(Ordering::Acquire) { return false; }
        self.links.lock().unwrap().get(id).is_some_and(|ls| ls.iter().any(|l| l.device.paired && l.cert == cert && l.generation == generation))
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
            transfers: Mutex::new(HashMap::new()),
            pairing: Mutex::new(HashMap::new()),
            qr: Mutex::new(HashMap::new()),
            clips: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            udp: Mutex::new(None),
            connections: Arc::new(AtomicUsize::new(0)),
            transfer_slots: Arc::new(AtomicUsize::new(0)),
            pending: Mutex::new(HashSet::new()),
            stopped: AtomicBool::new(false),
        });
        let mut startup = Startup { inner: inner.clone(), complete: false };
        // TCP: 1716 first (what phones try), then the rest of the range.
        let listener = std::iter::once(UDP_PORT).chain(PORT_MIN..=PORT_MAX).find_map(|p| TcpListener::bind((Ipv4Addr::UNSPECIFIED, p)).ok());
        let Some(listener) = listener else {
            let why = "Ports 1714–1764 are all in use. If KDE Connect is running on this computer, quit it — EchoFiles talks to your phone itself.".to_string();
            (inner.emit)(Event::Failed(why.clone()));
            return Err(io::Error::other(why));
        };
        let port = listener.local_addr()?.port();
        *inner.port.lock().unwrap() = port;
        listener.set_nonblocking(true)?;

        let acc = inner.clone();
        std::thread::Builder::new().name("phone-accept".into()).spawn(move || {
            while !acc.stopped.load(Ordering::Acquire) {
                let conn = match listener.accept() {
                    Ok((conn, _)) => conn,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => { std::thread::sleep(Duration::from_millis(50)); continue; },
                    Err(_) => break,
                };
                let Some(permit) = Permit::take(&acc.connections, 32) else { continue };
                let i = acc.clone();
                std::thread::spawn(move || {
                    let _permit = permit;
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
            for _ in 0..150 { if again.inner.stopped.load(Ordering::Acquire) { return; } std::thread::sleep(Duration::from_millis(100)); }
            let trusted = identity::trusted(&again.inner.dir);
            let links = again.inner.links.lock().unwrap();
            let missing = trusted.iter().any(|t| !links.get(&t.id).is_some_and(|ls| ls.iter().any(|l| l.via == Via::Lan)));
            drop(links);
            if missing {
                again.announce();
            }
        })?;
        startup.complete = true;
        (svc.inner.emit)(Event::Started { port });
        Ok(svc)
    }

    pub fn shutdown(&self) { self.inner.stopped.store(true, Ordering::Release); }

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
        if self.inner.stopped.load(Ordering::Acquire) { return; }
        let bytes = self.inner.identity_line();
        if let Some(sock) = self.inner.udp.lock().unwrap().as_ref() {
            let _ = sock.send_to(&bytes, to);
        }
    }

    /// Connect straight to a phone's KDE Connect port (1716 unless it says otherwise) —
    /// the way in when broadcasts don't get through.
    pub fn connect_to(&self, to: SocketAddr) {
        connect_bounded(&self.inner, to);
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
        if self.inner.stopped.load(Ordering::Acquire) { return; }
        let Some(permit) = Permit::take(&self.inner.connections, 32) else { return };
        let inner = self.inner.clone();
        std::thread::spawn(move || {
            let _permit = permit;
            let sock = UnixStream::from(fd);
            let ip = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
            let r = if initiator { outgoing_on(&inner, sock, ip, Via::Bluetooth) } else { incoming_on(&inner, sock, ip, Via::Bluetooth) };
            if let Err(e) = r {
                trace(&format!("bluetooth link: {e}"));
            }
        });
    }

    // ------------------------------------------------------------------ pairing

    /// A short-lived fingerprint from a QR code scanned by the owner.
    pub fn expect_qr(&self, id: &str, fingerprint: &str) {
        if id.len() <= 64 && self.inner.qr.lock().unwrap().len() < 128 && fingerprint.len() == 64 && fingerprint.bytes().all(|b| b.is_ascii_hexdigit()) {
            self.inner.qr.lock().unwrap().insert(id.into(), (fingerprint.to_ascii_lowercase(), Instant::now()));
        }
    }

    pub fn fingerprint(&self) -> String { fingerprint(&self.inner.me.cert) }

    /// Ask the phone to pair; its owner confirms the code there.
    pub fn pair(&self, id: &str) {
        let ts = (now_ms() / 1000) as u64;
        let Some(cert) = self.inner.cert(id) else { return };
        let code = identity::verification_key(&self.inner.me.cert, &cert, ts);
        let generation = self.inner.links.lock().unwrap().get(id).and_then(|ls| ls.first()).map(|l| l.generation);
        let Some(generation) = generation else { return };
        self.inner.pairing.lock().unwrap().insert(id.to_string(), PendingPair { cert, generation, created: Instant::now(), asked: true, timestamp: ts });
        self.inner.send(id, "kdeconnect.pair", json!({ "pair": true, "timestamp": ts }));
        (self.inner.emit)(Event::PairCode { id: id.to_string(), code });
    }

    /// Accept only the request whose code was actually displayed to the user.
    pub fn accept_pair_code(&self, id: &str, code: &str) {
        let pending = {
            let mut pairs = self.inner.pairing.lock().unwrap();
            let matches = pairs.get(id).is_some_and(|p| identity::verification_key(&self.inner.me.cert, &p.cert, p.timestamp) == code);
            if !matches { return; }
            pairs.remove(id)
        };
        if let Some(pending) = pending { if complete_pair(&self.inner, id, &pending) { self.inner.send(id, "kdeconnect.pair", json!({ "pair": true })); } }
    }

    pub fn accept_pair(&self, id: &str) {
        let pending = self.inner.pairing.lock().unwrap().remove(id);
        if let Some(pending) = pending {
            if complete_pair(&self.inner, id, &pending) {
                self.inner.send(id, "kdeconnect.pair", json!({ "pair": true }));
            }
        }
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
        body["clipId"] = json!(format!("{}-{}", self.device_id(), self.inner.new_id()));
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
        let Some(permit) = Permit::take(&self.inner.transfer_slots, 8) else { return };
        let Some(o) = self.inner.offers.lock().unwrap().remove(&transfer) else { return };
        if o.created.elapsed() > Duration::from_secs(300) || !self.inner.authorized(&o.device, &o.cert, o.generation) { return; }
        self.inner.transfers.lock().unwrap().insert(transfer, Arc::new(AtomicBool::new(false)));
        let inner = self.inner.clone();
        std::thread::spawn(move || { let _permit = permit; download(&inner, transfer, o, &dir); });
    }

    /// Cancellation is checked between chunks and while waiting for the receiver.
    pub fn cancel_transfer(&self, transfer: u64) {
        if let Some(flag) = self.inner.transfers.lock().unwrap().get(&transfer) { flag.store(true, Ordering::Relaxed); }
        self.reject_file(transfer);
    }

    pub fn reject_file(&self, transfer: u64) {
        self.inner.offers.lock().unwrap().remove(&transfer);
    }

    /// Send files to the phone, one after another. Returns the transfer ids, in order.
    pub fn send_files(&self, id: &str, files: Vec<PathBuf>) -> Vec<u64> {
        if !self.inner.paired(id) {
            return Vec::new();
        }
        let Some(permit) = Permit::take(&self.inner.transfer_slots, 8) else { return Vec::new() };
        let Some(auth) = self.inner.authorization(id) else { return Vec::new() };
        let files: Vec<_> = files.into_iter().take(1024).collect();
        let ids: Vec<u64> = files.iter().map(|_| self.inner.new_id()).collect();
        for id in &ids { self.inner.transfers.lock().unwrap().insert(*id, Arc::new(AtomicBool::new(false))); }
        let inner = self.inner.clone();
        let device = id.to_string();
        let work: Vec<(u64, PathBuf)> = ids.iter().copied().zip(files).collect();
        std::thread::spawn(move || {
            let _permit = permit;
            let total: u64 = work.iter().filter_map(|(_, p)| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
            let n = work.len();
            for (t, path) in work {
                let body = share_body(&path, n, total);
                let result = upload(&inner, &device, t, "kdeconnect.share.request", body, Source::Path(path.clone()), &auth);
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
        let permit = Permit::take(&self.inner.transfer_slots, 8)?;
        let auth = self.inner.authorization(id)?;
        let t = self.inner.new_id();
        self.inner.transfers.lock().unwrap().insert(t, Arc::new(AtomicBool::new(false)));
        let inner = self.inner.clone();
        let device = id.to_string();
        let kind = kind.to_string();
        std::thread::spawn(move || {
            let _permit = permit;
            let label = match &source {
                Source::Path(p) => p.clone(),
                _ => PathBuf::from(body["filename"].as_str().unwrap_or_default()),
            };
            let result = upload(&inner, &device, t, &kind, body, source, &auth);
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
    let _ = sock.set_read_timeout(Some(Duration::from_millis(250)));
    let mut buf = vec![0u8; 64 * 1024];
    while !inner.stopped.load(Ordering::Acquire) {
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
        connect_bounded(inner, SocketAddr::new(from.ip(), port as u16));
    }
}

/// We heard the phone: connect, introduce ourselves, then TLS as the server.
fn outgoing(inner: &Arc<Inner>, to: SocketAddr) -> io::Result<()> {
    let sock = TcpStream::connect_timeout(&to, Duration::from_secs(5))?;
    sock.set_nodelay(true)?;
    keepalive(&sock);
    outgoing_on(inner, sock, to.ip(), Via::Lan)
}

fn outgoing_on<S: Sock>(inner: &Arc<Inner>, sock: S, ip: IpAddr, via: Via) -> io::Result<()> {
    let mut sock = Deadline { inner: sock, until: Some(Instant::now() + Duration::from_secs(15)) };
    sock.write_all(&inner.identity_line())?;
    let mut conn = ServerConnection::new(inner.tls.server.clone()).map_err(io::Error::other)?;
    sock.timeout(Some(Duration::from_secs(10)))?;
    while conn.is_handshaking() {
        conn.complete_io(&mut sock)?;
    }
    let cert = conn.peer_certificates().and_then(|c| c.first()).map(|c| c.to_vec()).unwrap_or_default();
    sock.until = None;
    run_link(inner, Tls2::Server(StreamOwned::new(conn, sock)), ip, cert, None, via)
}

/// The phone connected to us: read its identity, then TLS as the client.
fn incoming(inner: &Arc<Inner>, sock: TcpStream) -> io::Result<()> {
    let ip = sock.peer_addr()?.ip();
    sock.set_nodelay(true)?;
    keepalive(&sock);
    incoming_on(inner, sock, ip, Via::Lan)
}

fn incoming_on<S: Sock>(inner: &Arc<Inner>, sock: S, ip: IpAddr, via: Via) -> io::Result<()> {
    let mut sock = Deadline { inner: sock, until: Some(Instant::now() + Duration::from_secs(15)) };
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
    sock.until = None;
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

    let (tx, rx): (SyncSender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::sync_channel(64);
    let generation = inner.new_id();
    let revoked = Arc::new(AtomicBool::new(false));
    let mut device: Option<Device> = None;
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = vec![0u8; 64 * 1024];
    let started = Instant::now();
    // Older peers don't repeat the identity inside TLS: fall back to the plain one.
    let mut pending_plain = plain_identity;

    let register = |d: &Device, tx: &SyncSender<Vec<u8>>| -> bool {
        if d.id.is_empty() || d.id.len() > 64 || !d.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') { return false; }
        if identity::trusted(&inner.dir).iter().any(|t| t.id == d.id && t.cert != cert) { return false; }
        let mut links = inner.links.lock().unwrap();
        let ls = links.entry(d.id.clone()).or_default();
        if ls.iter().any(|l| l.cert != cert) { if d.paired { ls.clear(); } else { return false; } }
        let fresh = !ls.iter().any(|l| l.device.paired);
        // A newer link replaces an older one on the same transport (its thread ends when
        // its sender drops).
        ls.retain(|l| l.via != via);
        ls.push(Link { revoked: revoked.clone(), tx: tx.clone(), device: d.clone(), cert: cert.clone(), generation, via });
        ls.sort_by_key(|l| l.via != Via::Lan);
        drop(links);
        trace(&format!("link {} {:?} {} paired={}", short(&d.id), via, d.name, d.paired));
        if let Some(full) = inner.device(&d.id) {
            (inner.emit)(Event::Device(full));
        }
        if d.paired && (fresh || via == Via::Lan) {
            on_paired_link(inner, &d.id);
        }
        let qr = inner.qr.lock().unwrap().remove(&d.id);
        if qr.is_some_and(|(fp, created)| created.elapsed() < Duration::from_secs(120) && fp == fingerprint(&cert)) {
            Service { inner: inner.clone() }.pair(&d.id);
        }
        true
    };

    let result = loop {
        if inner.stopped.load(Ordering::Acquire) { break Ok(()); }
        if started.elapsed() > Duration::from_secs(120) && device.as_ref().is_none_or(|d| !inner.authorized(&d.id, &cert, generation)) { break Err(io::Error::other("unpaired connection expired")); }
        if let Some(d) = &device {
            let current = inner.links.lock().unwrap().get(&d.id).is_some_and(|ls| ls.iter().any(|l| l.generation == generation));
            if !current {
                // Revocation ends the link immediately, but the peer must still learn
                // it was forgotten. Never drain queued feature data after revocation.
                if revoked.load(Ordering::Relaxed) {
                    let _ = stream.write_all(&line(&packet("kdeconnect.pair", json!({"pair":false}))));
                    let _ = stream.flush();
                }
                break Ok(());
            }
        }
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
                if !register(&d, &tx) { break Err(io::Error::other("identity certificate mismatch")); }
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
                        if device.is_some() { continue; }
                        if d.id.is_empty() || d.id == inner.me.device_id {
                            return Ok(());
                        }
                        pending_plain = None;
                        if !register(&d, &tx) { return Err(io::Error::other("identity certificate mismatch")); }
                        device = Some(d);
                        continue;
                    }
                    if let Some(d) = &device {
                        trace(&format!("← {} {}", short(&d.id), v["type"].as_str().unwrap_or("?")));
                        if via == Via::Bluetooth && (!bluetooth_ok(v["type"].as_str().unwrap_or_default()) || v.get("payloadTransferInfo").is_some()) { continue; }
                        handle(inner, &d.id, ip, generation, &cert, &v);
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

fn fingerprint(cert: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(cert).iter().map(|b| format!("{b:02x}")).collect()
}

fn complete_pair(inner: &Arc<Inner>, id: &str, pending: &PendingPair) -> bool {
    if pending.created.elapsed() > Duration::from_secs(120) { return false; }
    {
        let mut links = inner.links.lock().unwrap();
        let Some(ls) = links.get_mut(id) else { return false };
        let Some(l) = ls.iter().find(|l| l.generation == pending.generation && l.cert == pending.cert) else { return false };
        let t = Trusted { id: id.to_string(), name: l.device.name.clone(), kind: l.device.kind.clone(), cert: l.cert.clone() };
        if identity::trust(&inner.dir, &t).is_err() { return false; }
        for l in ls.iter_mut() { l.device.paired = l.cert == pending.cert; }
    }
    let Some(dev) = inner.device(id) else { return false };
    (inner.emit)(Event::Paired(dev));
    on_paired_link(inner, id);
    true
}

fn forget(inner: &Arc<Inner>, id: &str) {
    let _ = identity::forget(&inner.dir, id);
    inner.pairing.lock().unwrap().remove(id);
    inner.offers.lock().unwrap().retain(|_, o| o.device != id);
    if let Some(ls) = inner.links.lock().unwrap().remove(id) {
        for l in ls { l.revoked.store(true, Ordering::Relaxed); }
    }
    (inner.emit)(Event::Unpaired(id.to_string()));
    (inner.emit)(Event::Gone(id.to_string()));
}

fn handle(inner: &Arc<Inner>, id: &str, ip: IpAddr, generation: u64, cert: &[u8], v: &Value) {
    let authenticated = {
        let links = inner.links.lock().unwrap();
        let Some(link) = links.get(id).and_then(|ls| ls.iter().find(|l| l.generation == generation && l.cert == cert)) else { return };
        link.device.paired
    };
    let kind = v["type"].as_str().unwrap_or_default();
    let body = &v["body"];
    let emit = |e: Event| (inner.emit)(e);
    if kind == "kdeconnect.pair" {
        let wants = b(body, "pair");
        let asked = inner.pairing.lock().unwrap().get(id).filter(|p| p.generation == generation && p.cert == cert && p.created.elapsed() < Duration::from_secs(120)).cloned();
        match (wants, asked) {
            (true, Some(pending)) if pending.asked => {
                inner.pairing.lock().unwrap().remove(id);
                complete_pair(inner, id, &pending);
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
                inner.pairing.lock().unwrap().insert(id.to_string(), PendingPair { cert, generation, created: Instant::now(), asked: false, timestamp: ts });
                emit(Event::PairRequested { id: id.to_string(), code });
            }
            (false, Some(_)) => {
                inner.pairing.lock().unwrap().remove(id);
                emit(Event::PairRejected(id.to_string()));
            }
            (false, None) => {
                if authenticated {
                    forget(inner, id);
                }
            }
        }
        return;
    }
    if !authenticated {
        return;
    }
    let id = id.to_string();
    match kind {
        "kdeconnect.battery" => emit(Event::Battery { id, level: i(body, "currentCharge") as i32, charging: b(body, "isCharging") }),
        "kdeconnect.clipboard" | "kdeconnect.clipboard.connect" => {
            let clip = s(body, "clipId");
            if !clip.is_empty() {
                let mut seen = inner.clips.lock().unwrap();
                seen.retain(|_, at| now_ms() - *at < 60_000);
                if seen.len() >= 4096 { seen.clear(); }
                if seen.insert(format!("{id}:{clip}"), now_ms()).is_some() { return; }
            }
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
                actions: body["actions"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default(),
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
                    Sms { thread: i(m, "thread_id"), uid: i(m, "_id"), address, body: s(m, "body"), date: i(m, "date"), outgoing: i(m, "type") == 2, read: i(m, "read") != 0, attachments: m["attachments"].as_array().cloned().unwrap_or_default() }
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
                Ok(Sftp { ip: ip_s, port: i(body, "port") as u16, user: s(body, "user"), password: s(body, "password"), host_key_fingerprint: s(body, "hostKeyFingerprint"), path: s(body, "path"), places: paths.into_iter().zip(names).collect() })
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
                let mut offers = inner.offers.lock().unwrap();
                offers.retain(|_, o| o.created.elapsed() < Duration::from_secs(300));
                if offers.len() >= 128 { return; }
                offers.insert(transfer, Offer { device: id.clone(), ip, port: port as u16, name: name.clone(), size, modified: i(body, "lastModified"), cert: cert.to_vec(), generation, created: Instant::now() });
                drop(offers);
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
    if std::fs::symlink_metadata(&first).is_err() {
        return first;
    }
    let (stem, ext) = match clean.rfind('.') {
        Some(p) if p > 0 => (&clean[..p], &clean[p..]),
        _ => (clean.as_str(), ""),
    };
    (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| std::fs::symlink_metadata(p).is_err()).unwrap_or(first)
}

/// Atomic publication without replacing another file. Android SELinux disallows hard
/// links in app data, so use the Linux renameat2 operation on both supported platforms.
fn publish_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let from = std::ffi::CString::new(from.as_os_str().as_bytes()).map_err(io::Error::other)?;
    let to = std::ffi::CString::new(to.as_os_str().as_bytes()).map_err(io::Error::other)?;
    // SAFETY: both strings are NUL-terminated and valid throughout the syscall.
    let result = unsafe { libc::syscall(libc::SYS_renameat2, libc::AT_FDCWD, from.as_ptr(), libc::AT_FDCWD, to.as_ptr(), 1u32) };
    if result == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

struct TransferGuard<'a>(&'a Inner, u64);
impl Drop for TransferGuard<'_> { fn drop(&mut self) { self.0.transfers.lock().unwrap().remove(&self.1); } }
fn cancelled(inner: &Inner, transfer: u64) -> bool {
    inner.transfers.lock().unwrap().get(&transfer).is_some_and(|flag| flag.load(Ordering::Relaxed))
}

fn download(inner: &Arc<Inner>, transfer: u64, o: Offer, dir: &Path) {
    let _guard = TransferGuard(inner, transfer);
    let done = Arc::new(AtomicU64::new(0));
    (inner.emit)(Event::TransferStarted { id: o.device.clone(), transfer, name: o.name.clone(), size: o.size.max(0) as u64, upload: false, done: done.clone() });
    let result = (|| -> io::Result<PathBuf> {
        if cancelled(inner, transfer) { return Err(io::Error::other("Cancelled")); }
        if !inner.authorized(&o.device, &o.cert, o.generation) { return Err(io::Error::other("pairing revoked")); }
        std::fs::create_dir_all(dir)?;
        let mut sock = TcpStream::connect_timeout(&SocketAddr::new(o.ip, o.port), Duration::from_secs(10))?;
        sock.set_read_timeout(Some(Duration::from_secs(30)))?;
        let mut conn = ClientConnection::new(inner.tls.client.clone(), ServerName::IpAddress(o.ip.into())).map_err(io::Error::other)?;
        while conn.is_handshaking() {
            conn.complete_io(&mut sock)?;
        }
        let peer = conn.peer_certificates().and_then(|c| c.first()).map(|c| c.to_vec()).unwrap_or_default();
        if o.cert != peer {
            return Err(io::Error::other("The phone's certificate changed; not saving the file."));
        }
        let mut stream = StreamOwned::new(conn, sock);
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
        let part = dir.join(format!(".echo-{}.part", random.iter().map(|b| format!("{b:02x}")).collect::<String>()));
        struct Cleanup(PathBuf);
        impl Drop for Cleanup { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }
        let mut file = { use std::os::unix::fs::OpenOptionsExt; std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&part)? };
        let _cleanup = Cleanup(part.clone());
        let mut buf = vec![0u8; 256 * 1024];
        let mut got: u64 = 0;
        loop {
            if cancelled(inner, transfer) { return Err(io::Error::other("Cancelled")); }
            if !inner.authorized(&o.device, &o.cert, o.generation) { return Err(io::Error::other("pairing revoked")); }
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
        if cancelled(inner, transfer) { return Err(io::Error::other("Cancelled")); }
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
        loop {
            if !inner.authorized(&o.device, &o.cert, o.generation) { return Err(io::Error::other("pairing revoked")); }
            let target = free_name(dir, &o.name);
            match publish_no_replace(&part, &target) {
                Ok(()) => break Ok(target),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => break Err(e),
            }
        }
    })();
    (inner.emit)(Event::TransferDone { id: o.device, transfer, upload: false, result: result.map_err(|e| e.to_string()) });
}

fn upload(inner: &Arc<Inner>, device: &str, transfer: u64, kind: &str, body: Value, source: Source, auth: &(Vec<u8>, u64)) -> Result<(), String> {
    let _guard = TransferGuard(inner, transfer);
    if cancelled(inner, transfer) { return Err("Cancelled".into()); }
    let (expected, generation) = auth;
    if !inner.authorized(device, expected, *generation) { return Err("Pairing or connection revoked".into()); }
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
        if !inner.authorized(device, expected, *generation) { return Err("Pairing or connection revoked".into()); }
        if cancelled(inner, transfer) { return Err("Cancelled".into()); }
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
    let peer = conn.peer_certificates().and_then(|c| c.first()).map(|c| c.to_vec());
    if peer.as_deref() != Some(expected.as_slice()) { return Err("Payload certificate does not match the paired device".into()); }
    let mut stream = StreamOwned::new(conn, sock);
    let mut buf = vec![0u8; 256 * 1024];
    let mut sent = 0u64;
    while sent < size {
        if cancelled(inner, transfer) { return Err("Cancelled".into()); }
        let remaining = (size - sent).min(buf.len() as u64) as usize;
        let n = reader.read(&mut buf[..remaining]).map_err(|e| e.to_string())?;
        if n == 0 { return Err("The source file changed before the transfer finished.".into()); }
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

struct Permit(Arc<AtomicUsize>);
impl Permit {
    fn take(counter: &Arc<AtomicUsize>, max: usize) -> Option<Self> {
        counter.fetch_update(Ordering::AcqRel, Ordering::Relaxed, |n| (n < max).then_some(n + 1)).ok()?;
        Some(Self(counter.clone()))
    }
}
impl Drop for Permit { fn drop(&mut self) { self.0.fetch_sub(1, Ordering::AcqRel); } }

fn connect_bounded(inner: &Arc<Inner>, to: SocketAddr) {
    if inner.stopped.load(Ordering::Acquire) { return; }
    let Some(permit) = Permit::take(&inner.connections, 32) else { return };
    if !inner.pending.lock().unwrap().insert(to) { return; }
    let inner = inner.clone();
    std::thread::spawn(move || {
        let _permit = permit;
        let _ = outgoing(&inner, to);
        inner.pending.lock().unwrap().remove(&to);
    });
}

struct Deadline<S> { inner: S, until: Option<Instant> }
impl<S> Deadline<S> {
    fn check(&self) -> io::Result<()> {
        if self.until.is_some_and(|d| Instant::now() >= d) { Err(io::Error::new(io::ErrorKind::TimedOut, "complete handshake deadline")) } else { Ok(()) }
    }
}
impl<S: Read> Read for Deadline<S> { fn read(&mut self, b: &mut [u8]) -> io::Result<usize> { self.check()?; self.inner.read(b) } }
impl<S: Write> Write for Deadline<S> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> { self.check()?; self.inner.write(b) }
    fn flush(&mut self) -> io::Result<()> { self.check()?; self.inner.flush() }
}
impl<S: Sock> Sock for Deadline<S> { fn timeout(&self, d: Option<Duration>) -> io::Result<()> { self.inner.timeout(d) } }

struct Startup { inner: Arc<Inner>, complete: bool }
impl Drop for Startup { fn drop(&mut self) { if !self.complete { self.inner.stopped.store(true, Ordering::Release); } } }

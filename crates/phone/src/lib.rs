//! Android phones over Wi-Fi, speaking the KDE Connect protocol (version 8) ourselves —
//! nothing from KDE runs on the laptop; the phone runs the stock KDE Connect app.
//!
//! [`Service::start`] listens for phones (UDP 1716 and a TCP port in 1716–1764), keeps one
//! TLS link per phone, and turns their packets into [`Event`]s. Methods on [`Service`]
//! send requests: pair, ring, clipboard, notifications, texts, files and the phone's SFTP
//! server (which the app then opens through GVfs like any network place).

mod identity;
mod service;
mod tls;

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

pub use identity::{dir as config_dir, host_name, Trusted};
pub use serde_json::{json, Value};
pub use service::{trace_to, Role, Service, Source};

/// Lowest and highest TCP port KDE Connect uses.
pub const PORT_MIN: u16 = 1714;
pub const PORT_MAX: u16 = 1764;
/// The UDP discovery port.
pub const UDP_PORT: u16 = 1716;

/// A phone EchoFiles can talk to right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    /// "phone", "tablet", "desktop", "laptop", "tv".
    pub kind: String,
    pub ip: IpAddr,
    /// Paired, and presenting the certificate it paired with.
    pub paired: bool,
    /// What the phone accepts (`kdeconnect.sms.request`…): its enabled plugins.
    pub accepts: Vec<String>,
    /// What it sends.
    pub sends: Vec<String>,
    /// Reachable over Wi-Fi now.
    pub lan: bool,
    /// Reachable over Bluetooth now (clipboard and calls only).
    pub bluetooth: bool,
}

/// How a link reaches the other device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Via {
    Lan,
    Bluetooth,
}

impl Device {
    pub fn can(&self, packet_type: &str) -> bool {
        self.accepts.iter().any(|t| t == packet_type)
    }

    /// Runs EchoConnect (our own app), not stock KDE Connect.
    pub fn echoconnect(&self) -> bool {
        self.sends.iter().any(|t| t.starts_with("echofiles."))
    }
}

/// A notification shown on the phone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// The phone's key for it (used to dismiss).
    pub key: String,
    pub app: String,
    pub title: String,
    pub text: String,
    /// Milliseconds since the epoch.
    pub time: i64,
    pub dismissable: bool,
    pub silent: bool,
    /// Set when the app takes quick replies.
    pub reply_id: Option<String>,
}

/// One text message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sms {
    pub thread: i64,
    pub uid: i64,
    pub address: String,
    pub body: String,
    /// Milliseconds since the epoch.
    pub date: i64,
    /// Sent from the phone (otherwise received).
    pub outgoing: bool,
    pub read: bool,
}

/// Where the phone's SFTP server is listening (valid while the phone keeps it running).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sftp {
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    /// The storage root, e.g. `/storage/emulated/0`.
    pub path: String,
    /// Other storage (SD card), with the phone's names for them.
    pub places: Vec<(String, String)>,
}

/// What happened, for the app.
#[derive(Debug, Clone)]
pub enum Event {
    /// Listening on this TCP port.
    Started { port: u16 },
    /// Couldn't start (ports taken — is KDE Connect itself running?).
    Failed(String),
    /// A phone is connected (paired or not), or its details changed.
    Device(Device),
    /// A phone's link closed.
    Gone(String),
    /// The phone asks to pair; both screens show `code`. Answer with
    /// [`Service::accept_pair`] or [`Service::reject_pair`].
    PairRequested { id: String, code: String },
    /// We asked to pair; the phone shows `code` and waits for its owner.
    PairCode { id: String, code: String },
    Paired(Device),
    /// A paired device's link is up and the first requests went out.
    Ready(String),
    PairRejected(String),
    Unpaired(String),
    Battery { id: String, level: i32, charging: bool },
    Clipboard { id: String, text: String, sensitive: bool },
    Notification { id: String, notification: Notification },
    NotificationGone { id: String, key: String },
    Sms { id: String, messages: Vec<Sms> },
    Sftp { id: String, result: Result<Sftp, String> },
    /// Shared from the phone: some text, or a link.
    Text { id: String, text: String },
    Url { id: String, url: String },
    /// The other side offers a payload (a shared file, a thumbnail…) with packet `kind`;
    /// answer with [`Service::accept_file`] or [`Service::reject_file`].
    Incoming { id: String, transfer: u64, name: String, size: u64, kind: String, body: Value },
    TransferStarted { id: String, transfer: u64, name: String, size: u64, upload: bool, done: Arc<AtomicU64> },
    /// A download finished (the file's path) or an upload did (the sent file).
    TransferDone { id: String, transfer: u64, upload: bool, result: Result<PathBuf, String> },
    /// Any other packet from a paired device (requests a phone would answer).
    Other { id: String, kind: String, body: Value },
}

/// Phones paired with this computer.
pub fn trusted() -> Vec<Trusted> {
    identity::trusted(&identity::dir())
}

/// Devices paired with the service whose state lives in `dir`.
pub fn trusted_in(dir: &std::path::Path) -> Vec<Trusted> {
    identity::trusted(dir)
}

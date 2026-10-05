//! EchoConnect's core on Android: `ef_phone` running as the phone, exposed to Kotlin with
//! UniFFI.
//!
//! Kotlin owns everything Android (notifications, texts, contacts, calls, the clipboard,
//! the camera); Rust owns the connection — discovery, pairing, TLS, packets, payloads,
//! Bluetooth links and the SFTP server the laptop browses the phone through. Events cross
//! as `(kind, json)` pairs and packets as JSON bodies, so new features need no FFI changes.

use std::collections::HashMap;
use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ef_phone::{json, Device, Event, Service, Source, Value};

#[doc(hidden)]
pub mod sftp;

uniffi::setup_scaffolding!();

/// Receives everything the core reports. Called from core threads.
#[uniffi::export(with_foreign)]
pub trait Listener: Send + Sync {
    /// `kind` names the event ("device", "paired", "packet", "incoming"…); `json` is its data.
    fn on_event(&self, kind: String, json: String);
    /// One line of the packet log.
    fn on_log(&self, line: String);
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{0}")]
    Failed(String),
}

#[derive(uniffi::Object)]
pub struct Core {
    svc: Service,
    progress: Arc<Mutex<HashMap<u64, (u64, Arc<AtomicU64>)>>>,
    sftp: Mutex<Option<sftp::Server>>,
    dir: std::path::PathBuf,
}

fn device_json(d: &Device) -> Value {
    json!({
        "id": d.id, "name": d.name, "kind": d.kind, "ip": d.ip.to_string(), "paired": d.paired,
        "lan": d.lan, "bluetooth": d.bluetooth, "echoconnect": d.echoconnect(), "accepts": d.accepts,
    })
}

fn event_json(e: &Event, progress: &Mutex<HashMap<u64, (u64, Arc<AtomicU64>)>>) -> (&'static str, Value) {
    match e {
        Event::Started { port } => ("started", json!({ "port": port })),
        Event::Failed(why) => ("failed", json!({ "message": why })),
        Event::Device(d) => ("device", device_json(d)),
        Event::Gone(id) => ("gone", json!({ "id": id })),
        Event::PairRequested { id, code } => ("pair_requested", json!({ "id": id, "code": code })),
        Event::PairCode { id, code } => ("pair_code", json!({ "id": id, "code": code })),
        Event::Paired(d) => ("paired", device_json(d)),
        Event::Ready(id) => ("ready", json!({ "id": id })),
        Event::PairRejected(id) => ("pair_rejected", json!({ "id": id })),
        Event::Unpaired(id) => ("unpaired", json!({ "id": id })),
        Event::Battery { id, level, charging } => ("battery", json!({ "id": id, "level": level, "charging": charging })),
        Event::Clipboard { id, text, sensitive } => ("clipboard", json!({ "id": id, "text": text, "sensitive": sensitive })),
        Event::Text { id, text } => ("text", json!({ "id": id, "text": text })),
        Event::Url { id, url } => ("url", json!({ "id": id, "url": url })),
        Event::Incoming { id, transfer, name, size, kind, body } => ("incoming", json!({ "id": id, "transfer": transfer, "name": name, "size": size, "kind": kind, "body": body })),
        Event::TransferStarted { id, transfer, name, size, upload, done } => {
            progress.lock().unwrap().insert(*transfer, (*size, done.clone()));
            ("transfer_started", json!({ "id": id, "transfer": transfer, "name": name, "size": size, "upload": upload }))
        }
        Event::TransferDone { id, transfer, upload, result } => {
            progress.lock().unwrap().remove(transfer);
            match result {
                Ok(p) => ("transfer_done", json!({ "id": id, "transfer": transfer, "upload": upload, "path": p.to_string_lossy() })),
                Err(e) => ("transfer_done", json!({ "id": id, "transfer": transfer, "upload": upload, "error": e })),
            }
        }
        Event::Other { id, kind, body } => ("packet", json!({ "id": id, "type": kind, "body": body })),
        // Laptop-side events a phone never sees.
        Event::Notification { id, .. } | Event::NotificationGone { id, .. } | Event::Sms { id, .. } | Event::Sftp { id, .. } => ("ignored", json!({ "id": id })),
    }
}

fn parse(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|_| json!({}))
}

#[uniffi::export]
impl Core {
    /// Start as a phone named `name`, keeping identity and pairings in `dir`.
    #[uniffi::constructor]
    pub fn start(dir: String, name: String, listener: Arc<dyn Listener>) -> Result<Arc<Core>, CoreError> {
        let l2 = listener.clone();
        ef_phone::trace_to(move |line| l2.on_log(line.to_string()));
        let progress: Arc<Mutex<HashMap<u64, (u64, Arc<AtomicU64>)>>> = Arc::new(Mutex::new(HashMap::new()));
        let p2 = progress.clone();
        let dir = std::path::PathBuf::from(dir);
        let svc = Service::start_as(dir.clone(), Some(name), "phone", move |e| {
            let (kind, data) = event_json(&e, &p2);
            if kind != "ignored" {
                listener.on_event(kind.to_string(), data.to_string());
            }
        })
        .map_err(|e| CoreError::Failed(e.to_string()))?;
        Ok(Arc::new(Core { svc, progress, sftp: Mutex::new(None), dir }))
    }

    pub fn expect_qr(&self, id: String, fingerprint: String) { self.svc.expect_qr(&id, &fingerprint); }

    pub fn device_id(&self) -> String {
        self.svc.device_id().to_string()
    }

    pub fn port(&self) -> u16 {
        self.svc.port()
    }

    /// Say we're here (after joining a network, or every so often while unpaired).
    pub fn announce(&self) {
        self.svc.announce();
    }

    /// Connect straight to a laptop (from a QR code or a typed address).
    pub fn connect(&self, host: String, port: u16) -> bool {
        match format!("{host}:{port}").parse::<std::net::SocketAddr>() {
            Ok(a) => {
                self.svc.connect_to(a);
                self.svc.announce_to(std::net::SocketAddr::new(a.ip(), ef_phone::UDP_PORT));
                true
            }
            Err(_) => false,
        }
    }

    pub fn pair(&self, id: String) {
        self.svc.pair(&id);
    }

    pub fn accept_pair(&self, id: String) {
        self.svc.accept_pair(&id);
    }

    pub fn accept_pair_code(&self, id: String, code: String) {
        self.svc.accept_pair_code(&id, &code);
    }

    /// Revoke active work and release network services when Android stops its owner.
    pub fn shutdown(&self) {
        self.svc.shutdown();
        self.stop_sftp();
    }

    pub fn reject_pair(&self, id: String) {
        self.svc.reject_pair(&id);
    }

    pub fn unpair(&self, id: String) {
        self.svc.unpair(&id);
    }

    /// Send a packet to a paired laptop. False if it isn't connected (or the packet needs Wi-Fi).
    pub fn send(&self, id: String, kind: String, body: String) -> bool {
        self.svc.send_packet(&id, &kind, parse(&body))
    }

    pub fn send_clipboard(&self, id: String, text: String, sensitive: bool) -> bool {
        self.svc.send_clipboard(&id, &text, false, sensitive)
    }

    /// Send a payload read from `fd` (ownership passes to the core; it closes it).
    pub fn send_fd(&self, id: String, kind: String, body: String, fd: i32, size: u64) -> Option<u64> {
        if fd < 0 {
            return None;
        }
        // SAFETY: Kotlin detached this descriptor (ParcelFileDescriptor.detachFd) for us.
        let file = std::fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) });
        self.svc.send_payload(&id, &kind, parse(&body), Source::File(file, size))
    }

    pub fn send_bytes(&self, id: String, kind: String, body: String, bytes: Vec<u8>) -> Option<u64> {
        self.svc.send_payload(&id, &kind, parse(&body), Source::Bytes(bytes))
    }

    pub fn send_path(&self, id: String, kind: String, body: String, path: String) -> Option<u64> {
        self.svc.send_payload(&id, &kind, parse(&body), Source::Path(path.into()))
    }

    /// Save an offered payload into `dir`.
    pub fn accept_file(&self, transfer: u64, dir: String) {
        self.svc.accept_file(transfer, dir.into());
    }

    pub fn cancel_transfer(&self, transfer: u64) { self.svc.cancel_transfer(transfer); }

    pub fn reject_file(&self, transfer: u64) {
        self.svc.reject_file(transfer);
    }

    /// Bytes moved so far and the total, for a running transfer.
    pub fn progress(&self, transfer: u64) -> Vec<u64> {
        self.progress.lock().unwrap().get(&transfer).map(|(size, done)| vec![done.load(Ordering::Relaxed), *size]).unwrap_or_default()
    }

    /// Run a link over a Bluetooth socket (one end of a socket pair Kotlin bridges to RFCOMM).
    pub fn adopt_fd(&self, fd: i32, initiator: bool) {
        if fd >= 0 {
            // SAFETY: Kotlin detached this descriptor for us.
            self.svc.adopt(unsafe { OwnedFd::from_raw_fd(fd) }, initiator);
        }
    }

    pub fn devices(&self) -> String {
        Value::Array(self.svc.devices().iter().map(device_json).collect()).to_string()
    }

    /// Laptops paired with this phone (connected or not).
    pub fn trusted(&self) -> String {
        Value::Array(ef_phone::trusted_in(&self.dir).iter().map(|t| json!({ "id": t.id, "name": t.name, "kind": t.kind })).collect()).to_string()
    }

    /// Serve `roots` (name → folder) over SFTP for the laptop and tell it how to connect.
    /// Returns the port, or an error.
    pub fn start_sftp(&self, laptop: String, roots: Vec<String>, names: Vec<String>) -> Result<u16, CoreError> {
        let mut slot = self.sftp.lock().unwrap();
        let server = match slot.take() {
            Some(s) => s,
            None => sftp::Server::start(roots.clone()).map_err(CoreError::Failed)?,
        };
        let port = server.port();
        let body = json!({
            "port": port, "user": server.user(), "password": server.password(), "hostKeyFingerprint": server.fingerprint(),
            "path": roots.first().cloned().unwrap_or_default(), "multiPaths": roots, "pathNames": names,
        });
        *slot = Some(server);
        self.svc.send_packet(&laptop, "kdeconnect.sftp", body);
        Ok(port)
    }

    pub fn stop_sftp(&self) {
        self.sftp.lock().unwrap().take();
    }
}

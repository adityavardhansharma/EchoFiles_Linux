//! EchoConnect-only laptop integration. Protocol data stays on the authenticated link.
use std::{collections::HashMap, path::{Path, PathBuf}, time::Duration};
use ef_phone::{json, Service, Value};
use ef_theme::{Palette, Rgb};
use zbus::{blocking::{Connection, Proxy}, zvariant::{OwnedObjectPath, OwnedValue}};

pub const BT_UUID: &str = "d5f58a22-99f5-4c68-9e21-e1b168395832";
type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

struct BluetoothProfile(Service);
#[zbus::interface(name = "org.bluez.Profile1")]
impl BluetoothProfile {
    fn release(&self) {}
    fn new_connection(&self, _device: OwnedObjectPath, fd: zbus::zvariant::OwnedFd, _properties: HashMap<String, OwnedValue>) {
        self.0.adopt(fd.into(), false);
    }
    fn request_disconnection(&self, _device: OwnedObjectPath) {}
}

pub fn bluetooth(service: Service) {
    std::thread::spawn(move || {
        let run = || -> Result<(), Box<dyn std::error::Error>> {
            let c = Connection::system()?;
            let path = "/app/echofiles/EchoConnect";
            c.object_server().at(path, BluetoothProfile(service))?;
            let manager = Proxy::new(&c, "org.bluez", "/org/bluez", "org.bluez.ProfileManager1")?;
            let options: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::from([
                ("Name", "EchoConnect".into()), ("Role", "server".into()),
                ("RequireAuthentication", true.into()), ("RequireAuthorization", false.into()),
            ]);
            manager.call::<_, _, ()>("RegisterProfile", &(zbus::zvariant::ObjectPath::try_from(path)?, BT_UUID, options))?;
            loop { std::thread::park_timeout(Duration::from_secs(60)); }
        };
        if let Err(e) = run() { eprintln!("EchoConnect Bluetooth: {e}"); }
    });
}

pub fn bluetooth_address() -> Option<String> {
    let c = Connection::system().ok()?;
    let objects: Objects = Proxy::new(&c, "org.bluez", "/", "org.freedesktop.DBus.ObjectManager").ok()?.call("GetManagedObjects", &()).ok()?;
    objects.values().find_map(|interfaces| interfaces.get("org.bluez.Adapter1").and_then(|a| a.get("Address")).and_then(|v| <&str>::try_from(v).ok()).map(str::to_owned))
}

/// PipeWire exposes HFP calls, including Answer/Hangup, on the session bus.
pub fn call(action: &str, number: &str) -> Result<(), String> {
    let c = Connection::session().map_err(|e| e.to_string())?;
    let manager = Proxy::new(&c, "org.pipewire.Telephony", "/org/pipewire/Telephony", "org.freedesktop.DBus.ObjectManager").map_err(|e| e.to_string())?;
    let objects: Objects = manager.call("GetManagedObjects", &()).map_err(|e| e.to_string())?;
    let method = match action { "answer" => "Answer", "hangup" => "Hangup", _ => return Err("Unknown call action".into()) };
    let normalize = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
    let number = normalize(number);
    let calls: Vec<_> = objects.into_iter().filter(|(_, interfaces)| interfaces.contains_key("org.pipewire.Telephony.Call1")).collect();
    let matching: Vec<_> = calls.iter().filter(|(_, interfaces)| {
        let props = &interfaces["org.pipewire.Telephony.Call1"];
        let n = props.get("LineIdentification").and_then(|v| <&str>::try_from(v).ok()).unwrap_or("");
        !number.is_empty() && normalize(n) == number
    }).collect();
    let chosen = if matching.len() == 1 { matching.first().copied() } else if calls.len() == 1 { calls.first() } else { None };
    if let Some((path, _)) = chosen {
        return Proxy::new(&c, "org.pipewire.Telephony", path.clone(), "org.pipewire.Telephony.Call1").map_err(|e| e.to_string())?.call::<_, _, ()>(method, &()).map_err(|e| e.to_string());
    }
    Err("No Bluetooth call is available. Pair the phone in Bluetooth settings and connect its headset profile.".into())
}

const HFP_AG: &str = "0000111f-0000-1000-8000-00805f9b34fb";
static CALL_AUDIO: std::sync::Mutex<Option<(String, Vec<std::process::Child>)>> = std::sync::Mutex::new(None);

pub fn connect_headset(name: &str) -> Result<String, String> {
    let c = Connection::system().map_err(|e| e.to_string())?;
    let objects: Objects = Proxy::new(&c, "org.bluez", "/", "org.freedesktop.DBus.ObjectManager").map_err(|e| e.to_string())?.call("GetManagedObjects", &()).map_err(|e| e.to_string())?;
    let candidates: Vec<_> = objects.iter().filter_map(|(path, interfaces)| {
        let d = interfaces.get("org.bluez.Device1")?;
        if d.get("Paired").and_then(|v| bool::try_from(v).ok()) != Some(true) { return None; }
        let uuids = d.get("UUIDs").and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok()).unwrap_or_default();
        uuids.iter().any(|u| u == HFP_AG).then_some((path, d))
    }).collect();
    let matching: Vec<_> = candidates.iter().filter(|(_, d)| ["Name", "Alias"].iter().any(|key| d.get(*key).and_then(|v| <&str>::try_from(v).ok()) == Some(name))).collect();
    let chosen = if matching.len() == 1 { matching.first().copied() } else if candidates.len() == 1 { candidates.first() } else { None };
    let (path, device) = chosen.ok_or("Pair this phone in Bluetooth settings. If several phones are paired, give this phone the same name in Bluetooth and EchoConnect.")?;
    let address = device.get("Address").and_then(|v| <&str>::try_from(v).ok()).ok_or("Phone has no Bluetooth address")?.to_string();
    Proxy::new(&c, "org.bluez", (*path).clone(), "org.bluez.Device1").map_err(|e| e.to_string())?.call::<_, _, ()>("ConnectProfile", &(HFP_AG,)).map_err(|e| e.to_string())?;
    *CALL_AUDIO.lock().unwrap() = Some((path.to_string(), Vec::new()));
    Ok(address)
}

/// Route only this call's two streams. System default devices stay unchanged.
pub fn route_call_audio(address: &str) -> Result<(), String> {
    use std::process::{Command, Stdio};
    use std::os::unix::process::CommandExt;
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while std::time::Instant::now() < deadline {
        let output = Command::new("pw-dump").output().map_err(|e| e.to_string())?;
        let objects: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
        let empty = Vec::new(); let objects = objects.as_array().unwrap_or(&empty);
        let devices: Vec<_> = objects.iter().filter(|o| o["info"]["props"]["api.bluez5.address"].as_str().or_else(|| o["info"]["props"]["bluez5.address"].as_str()) == Some(address)).filter_map(|o| o["id"].as_u64()).collect();
        let node = |class: &str| objects.iter().find_map(|o| {
            let p = &o["info"]["props"];
            let dev = p["device.id"].as_u64().or_else(|| p["device.id"].as_str()?.parse().ok());
            if p["media.class"].as_str() == Some(class) && dev.is_some_and(|d| devices.contains(&d)) { p["node.name"].as_str().map(str::to_owned) } else { None }
        });
        if let (Some(source), Some(sink)) = (node("Audio/Source"), node("Audio/Sink")) {
            let mut state = CALL_AUDIO.lock().unwrap();
            let (_, children) = state.as_mut().ok_or("Call routing was cancelled")?;
            for (capture, playback) in [(source.as_str(), "@DEFAULT_AUDIO_SINK@"), ("@DEFAULT_AUDIO_SOURCE@", sink.as_str())] {
                let mut command = Command::new("pw-loopback");
                command.args(["-n", "EchoConnect call", "-C", capture, "-P", playback]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
                // SAFETY: only the async-signal-safe prctl syscall runs between fork and exec.
                unsafe { command.pre_exec(|| { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM); Ok(()) }); }
                children.push(command.spawn().map_err(|e| e.to_string())?);
            }
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err("Call answered, but PipeWire did not expose the phone's audio. Use phone to return audio, or enable the hands-free profile in Bluetooth settings.".into())
}

pub fn release_headset() -> Result<(), String> {
    if let Some((path, children)) = CALL_AUDIO.lock().unwrap().take() {
        for mut child in children { let _ = child.kill(); let _ = child.wait(); }
        let c = Connection::system().map_err(|e| e.to_string())?;
        Proxy::new(&c, "org.bluez", path, "org.bluez.Device1").map_err(|e| e.to_string())?.call::<_, _, ()>("DisconnectProfile", &(HFP_AG,)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn theme(p: &Palette) -> Value {
    let mut tokens = json!({});
    let rgb = |c: Rgb| -> u64 { 0xff000000 | (((c.r * 255.0).round() as u64) << 16) | (((c.g * 255.0).round() as u64) << 8) | (c.b * 255.0).round() as u64 };
    macro_rules! colors { ($($field:ident),*) => { $(tokens[stringify!($field).replace('_', "-")] = json!(rgb(p.$field));)* }; }
    colors!(bg,bg_sunken,bg_deep,bg_raised,line,line_strong,ink,ink_strong,ink_muted,ink_faint,accent,accent_ink,on_accent,accent_soft,selection,focus_ring,state_hover,state_active,state_press,scrim,world_linux,world_windows,world_phone,world_network);
    for (name, s) in [("success",p.success),("warning",p.warning),("danger",p.danger),("info",p.info)] {
        tokens[name] = json!(rgb(s.base)); tokens[format!("{name}-soft")] = json!(rgb(s.soft)); tokens[format!("{name}-ink")] = json!(rgb(s.ink)); tokens[format!("on-{name}")] = json!(rgb(s.on));
    }
    for (name, c) in &p.icon_slots { tokens[*name] = json!(rgb(*c)); }
    json!({ "tokens": tokens })
}

pub fn pairing_svg(s: &Service) -> Option<Vec<u8>> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:9").ok()?;
    let body = json!({"app":"echoconnect", "version":1, "id":s.device_id(), "host":sock.local_addr().ok()?.ip().to_string(), "port":s.port(), "fingerprint":s.fingerprint(), "bluetooth":bluetooth_address()});
    let qr = qrcode::QrCode::new(body.to_string()).ok()?;
    Some(qr.render::<qrcode::render::svg::Color>().min_dimensions(240,240).build().into_bytes())
}

pub fn grant_clipboard(address: &str, code: &str, connect: &str) -> Result<(), String> {
    if address.parse::<std::net::SocketAddr>().is_err() || connect.parse::<std::net::SocketAddr>().is_err() || code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Enter the wireless-debugging IP:port, six-digit pairing code and connection IP:port shown on your phone.".into());
    }
    let paired = adb(&["pair", address], Some(code))?;
    if !paired.contains("Successfully paired") { return Err("Wireless debugging pairing failed. Generate a new code.".into()); }
    adb(&["connect", connect], None)?;
    let result = adb(&["-s", connect, "shell", "pm", "grant", "app.echoconnect", "android.permission.READ_LOGS"], None);
    let _ = adb(&["disconnect", connect], None);
    result.map(|_| ())
}

pub fn lock() { let _ = std::process::Command::new("loginctl").arg("lock-session").spawn(); }

pub fn otp(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if !["otp", "verification", "one-time", "security code", "login code"].iter().any(|word| lower.contains(word)) { return None; }
    text.split(|c: char| !c.is_ascii_digit()).find(|s| (4..=8).contains(&s.len())).map(str::to_owned)
}

/// A complete index replaces the last snapshot only after every requested page arrives.
#[derive(Default)]
pub struct PhoneIndexScan {
    pub device: String,
    pub request: String,
    queue: std::collections::VecDeque<(String, i64)>,
    seen: std::collections::HashSet<String>,
    entries: Vec<Value>,
    counter: u64,
}
impl PhoneIndexScan {
    pub fn new(device: String) -> Self {
        Self { device, queue: [(String::new(), 0)].into(), ..Self::default() }
    }
    pub fn next(&mut self) -> Option<Value> {
        let (path, offset) = self.queue.pop_front()?;
        self.counter += 1;
        self.request = format!("index-{}-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos(), self.counter);
        Some(json!({"requestId":self.request,"path":path,"offset":offset}))
    }
    pub fn accept(&mut self, body: &Value) -> Result<(), String> {
        if body["error"].is_string() { return Err(body["error"].as_str().unwrap().into()); }
        let entries = body["entries"].as_array().ok_or("Invalid file listing")?;
        if self.entries.len() + entries.len() > 200_000 { return Err("Phone index exceeds 200,000 files; previous snapshot kept".into()); }
        for entry in entries {
            let Some(path) = entry["path"].as_str() else { continue };
            if path.starts_with('/') || Path::new(path).components().any(|c| matches!(c, std::path::Component::ParentDir)) { continue; }
            if entry["directory"].as_bool() == Some(true) && self.seen.insert(path.to_owned()) { self.queue.push_back((path.to_owned(), 0)); }
            self.entries.push(json!({"device":self.device,"path":path,"directory":entry["directory"],"size":entry["size"],"modified":entry["modified"]}));
        }
        if let Some(next) = body["next"].as_i64().filter(|n| *n >= 0) { self.queue.push_front((body["path"].as_str().unwrap_or("").into(), next)); }
        Ok(())
    }
    pub fn save(&self) -> std::io::Result<usize> {
        let dir = ef_config::state_dir().join("phone-index");
        let mut data = Vec::new();
        for entry in &self.entries { serde_json::to_writer(&mut data, entry)?; data.push(b'\n'); }
        ef_config::atomic_write(&dir.join(format!("{}.jsonl", self.device)), &data)?;
        Ok(self.entries.len())
    }
}

pub fn deduplicate_backup(path: &Path) -> std::io::Result<PathBuf> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?; let mut hash = Sha256::new(); let mut buf = [0u8;65536];
    loop { let n = file.read(&mut buf)?; if n == 0 { break; } hash.update(&buf[..n]); }
    let digest = format!("{:x}", hash.finalize());
    let dir = path.parent().unwrap_or(Path::new("."));
    let index = dir.join(".echo-hashes"); std::fs::create_dir_all(&index)?;
    // A marker is a hint. Verify the original still exists and has the same bytes
    // before deleting a retry; an interrupted write or user deletion must not lose data.
    static BACKUP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = BACKUP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let marker = index.join(&digest);
    if let Ok(name) = std::fs::read_to_string(&marker) {
        if Path::new(&name).file_name().is_some_and(|n| n == name.as_str()) {
            let original = dir.join(&name);
            if original != path {
                if let Ok(mut file) = std::fs::File::open(&original) {
                    let mut hash = Sha256::new();
                    loop { let n = file.read(&mut buf)?; if n == 0 { break; } hash.update(&buf[..n]); }
                    if format!("{:x}", hash.finalize()) == digest { std::fs::remove_file(path)?; return Ok(original); }
                }
            }
        }
    }
    std::fs::write(marker, path.file_name().unwrap_or_default().as_encoded_bytes())?;
    Ok(path.into())
}

#[cfg(test)] mod tests {
    #[test] fn backup_retries_and_stale_markers_preserve_bytes() {
        let dir = tempfile::tempdir().unwrap(); let first = dir.path().join("first.jpg"); let second = dir.path().join("second.jpg");
        std::fs::write(&first, b"photo bytes").unwrap(); std::fs::write(&second, b"photo bytes").unwrap();
        assert_eq!(super::deduplicate_backup(&first).unwrap(), first);
        assert_eq!(super::deduplicate_backup(&second).unwrap(), first); assert!(!second.exists());
        std::fs::remove_file(&first).unwrap(); std::fs::write(&second, b"photo bytes").unwrap();
        assert_eq!(super::deduplicate_backup(&second).unwrap(), second); assert!(second.exists());
        // Editing an original invalidates the marker; the new photo must survive.
        std::fs::write(&second, b"edited").unwrap(); std::fs::write(&first, b"photo bytes").unwrap();
        assert_eq!(super::deduplicate_backup(&first).unwrap(), first); assert_eq!(std::fs::read(&second).unwrap(), b"edited");
    }
    #[test] fn index_scan_pages_and_rejects_outside_paths() {
        let mut scan = super::PhoneIndexScan::new("test".into());
        assert_eq!(scan.next().unwrap()["path"], "");
        scan.accept(&super::json!({"path":"", "next":500, "entries":[{"path":"DCIM", "directory":true},{"path":"../private","directory":true}]})).unwrap();
        assert_eq!(scan.next().unwrap()["offset"], 500);
        scan.accept(&super::json!({"path":"", "next":-1, "entries":[{"path":"DCIM", "directory":true}]})).unwrap();
        assert_eq!(scan.next().unwrap()["path"], "DCIM");
        scan.accept(&super::json!({"path":"DCIM", "next":-1, "entries":[]})).unwrap();
        assert!(scan.next().is_none());
        assert!(scan.accept(&super::json!({"error":"permission denied"})).is_err());
    }
    #[test] fn codes_need_context() { assert_eq!(super::otp("Your verification code is 194285"), Some("194285".into())); assert_eq!(super::otp("Order 194285 shipped"), None); }
}

pub fn clipboard_qr() -> Result<(Vec<u8>, String, String), String> {
    let mut random = [0u8; 24]; getrandom::fill(&mut random).map_err(|e| e.to_string())?;
    let name = format!("studio-{}", random[..8].iter().map(|b| format!("{b:02x}")).collect::<String>());
    let secret = random[8..].iter().map(|b| format!("{b:02x}")).collect::<String>();
    let qr = qrcode::QrCode::new(format!("WIFI:T:ADB;S:{name};P:{secret};;")).map_err(|e| e.to_string())?;
    Ok((qr.render::<qrcode::render::svg::Color>().min_dimensions(240,240).build().into_bytes(), name, secret))
}

fn adb(args: &[&str], input: Option<&str>) -> Result<String, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("adb").args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|_| "Install android-tools to enable automatic clipboard.".to_string())?;
    if let Some(text) = input { if let Some(mut stdin) = child.stdin.take() { writeln!(stdin, "{text}").map_err(|e| e.to_string())?; } }
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(_) => { let out = child.wait_with_output().map_err(|e| e.to_string())?; if !out.status.success() { return Err(String::from_utf8_lossy(&out.stderr).into_owned()); } return Ok(String::from_utf8_lossy(&out.stdout).into_owned()); }
            None if std::time::Instant::now() > deadline => { let _ = child.kill(); let _ = child.wait(); return Err("Wireless debugging timed out. Open its Settings page and try again.".into()); }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// Only the service name and phone IP belonging to this displayed, one-time QR are used.
pub fn grant_clipboard_qr(name: &str, secret: &str, phone_ip: std::net::IpAddr) -> Result<String, String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    let mut paired = false;
    while std::time::Instant::now() < deadline {
        let services = adb(&["mdns", "services"], None)?;
        for line in services.lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 3 { continue; }
            let Some(address) = fields.last().and_then(|s| s.parse::<std::net::SocketAddr>().ok()) else { continue };
            if address.ip() != phone_ip { continue; }
            let endpoint = address.to_string();
            if !paired && fields[0] == name && line.contains("_adb-tls-pairing") {
                let response = adb(&["pair", &endpoint], Some(secret))?;
                if !response.contains("Successfully paired") { return Err("Pairing failed. Generate another code.".into()); }
                paired = true;
            }
            if paired && line.contains("_adb-tls-connect") {
                adb(&["connect", &endpoint], None)?;
                let result = adb(&["-s", &endpoint, "shell", "pm", "grant", "app.echoconnect", "android.permission.READ_LOGS"], None);
                let _ = adb(&["disconnect", &endpoint], None);
                result?;
                return Ok("Permission granted. Allow Display over other apps, then turn Developer options off on the phone.".into());
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err("The QR code expired. Open Wireless debugging → Pair device with QR code and try again.".into())
}

pub fn set_dnd(enabled: bool) {
    let _ = std::process::Command::new("makoctl").args(["mode", if enabled { "-a" } else { "-r" }, "do-not-disturb"]).output();
}

/// Mako is the notification daemon used by Omarchy. Unknown state is not "off".
pub fn dnd_state() -> Option<bool> {
    let out = std::process::Command::new("makoctl").arg("mode").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).lines().any(|line| line.trim() == "do-not-disturb"))
}

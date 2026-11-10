//! The bridge between EchoConnect and EchoScribe (the dictation app): EchoConnect's
//! `echofiles.scribe` packets ride the phone link this app already keeps — paired, TLS, Wi-Fi
//! or Bluetooth — and are passed to EchoScribe over its local socket; EchoScribe's replies go
//! back to the phone the same way. EchoScribe gets no network access to the phone and no second
//! pairing.
//!
//! The socket is `$XDG_RUNTIME_DIR/echoscribe/sock`. After saying `bridge` (answered `ok`) it
//! carries JSON lines: `{"t":"phones","list":[…]}` and `{"t":"packet","id","body"}` from here,
//! `{"t":"send","id","body"}` from EchoScribe. When EchoScribe isn't running, phones are told
//! `{"op":"state","available":false}` so EchoConnect can say so.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use ef_phone::{Event, Value, json};

const CONTROL: &str = "echofiles.scribe";
const AUDIO: &str = "echofiles.scribe.audio";

static OUT: Mutex<Option<UnixStream>> = Mutex::new(None);

fn sock() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("echoscribe").join("sock")
}

fn write(v: &Value) -> bool {
    let mut g = OUT.lock().unwrap_or_else(|e| e.into_inner());
    let ok = g.as_mut().is_some_and(|s| writeln!(s, "{v}").is_ok());
    if !ok {
        *g = None;
    }
    ok
}

fn connected() -> bool {
    OUT.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// Paired EchoConnect phones, and whether each is reachable now.
fn phones() -> Value {
    let Some(s) = crate::phone::service() else { return json!([]) };
    let list: Vec<Value> = s
        .devices()
        .iter()
        .filter(|d| d.paired && d.echoconnect())
        .map(|d| json!({ "id": d.id, "name": d.name, "online": d.lan || d.bluetooth, "lan": d.lan, "bluetooth": d.bluetooth }))
        .collect();
    Value::Array(list)
}

fn send_phones() {
    write(&json!({ "t": "phones", "list": phones() }));
}

fn to_phone(id: &str, body: Value) {
    if let Some(s) = crate::phone::service() {
        s.send_packet(id, CONTROL, body);
    }
}

/// Tell every online phone whether dictation is available right now.
fn announce(available: bool) {
    let Some(s) = crate::phone::service() else { return };
    for d in s.devices().iter().filter(|d| d.paired && d.echoconnect() && (d.lan || d.bluetooth)) {
        if available {
            // EchoScribe answers a hello with its full state.
            write(&json!({ "t": "packet", "id": d.id, "body": { "op": "hello" } }));
        } else {
            s.send_packet(&d.id, CONTROL, json!({ "op": "state", "available": false }));
        }
    }
}

/// Keeps a connection to EchoScribe while it runs, reconnecting every few seconds.
pub fn start() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::Builder::new()
            .name("echofiles-scribe".into())
            .spawn(|| loop {
                if let Ok(mut s) = UnixStream::connect(sock()) {
                    let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
                    let mut reply = String::new();
                    let ok = writeln!(s, "bridge").is_ok() && BufReader::new(&s).read_line(&mut reply).is_ok() && reply.trim() == "ok";
                    if ok && let Ok(reader) = s.try_clone() {
                        let _ = s.set_read_timeout(None);
                        let _ = reader.set_read_timeout(None);
                        *OUT.lock().unwrap_or_else(|e| e.into_inner()) = Some(s);
                        send_phones();
                        announce(true);
                        for line in BufReader::new(reader).lines() {
                            let Ok(line) = line else { break };
                            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                            if v["t"] == "send"
                                && let Some(id) = v["id"].as_str()
                            {
                                to_phone(id, v["body"].clone());
                            }
                        }
                        *OUT.lock().unwrap_or_else(|e| e.into_inner()) = None;
                        announce(false);
                    }
                }
                std::thread::sleep(Duration::from_secs(3));
            })
            .ok();
    });
}

/// Phones coming and going update EchoScribe's list. Never consumes the event.
pub fn observe(e: &Event) {
    match e {
        Event::Device(_) | Event::Gone(_) | Event::Paired(_) | Event::Unpaired(_) => {
            if connected() {
                send_phones();
            }
        }
        Event::Ready(id) => {
            if connected() {
                send_phones();
                write(&json!({ "t": "packet", "id": id, "body": { "op": "hello" } }));
            }
        }
        _ => {}
    }
}

/// Takes EchoConnect's dictation packets out of the event stream (the window never sees them).
pub fn claim(e: &Event) -> bool {
    let Event::Other { id, kind, body } = e else { return false };
    if kind != CONTROL && kind != AUDIO {
        return false;
    }
    let mut body = body.clone();
    if kind == AUDIO {
        body["op"] = json!("audio");
    }
    if !write(&json!({ "t": "packet", "id": id, "body": body })) && kind == CONTROL {
        to_phone(id, json!({ "op": "state", "available": false, "error": "EchoScribe isn't running on the laptop" }));
    }
    true
}

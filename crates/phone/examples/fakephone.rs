//! A stand-in Android phone for trying EchoFiles' phone features without one: it pairs
//! (accepting automatically), reports battery, sends notifications and texts, answers the
//! SFTP request with a local test server, shares a file and some text, and prints what the
//! laptop asks for.
//!
//! `cargo run -p echofiles-phone --example fakephone -- <state dir> <sftp port>`

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::mpsc;
use std::time::Duration;

use ef_phone::{json, Event, Service};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().unwrap_or_else(|| "/tmp/fakephone".into()));
    let sftp_port: u16 = args.next().and_then(|a| a.parse().ok()).unwrap_or(2222);
    let (tx, rx) = mpsc::channel();
    let svc = Service::start_as(dir.join("state"), Some("Pixel 9 (test)".into()), "phone", move |e| {
        let _ = tx.send(e);
    })
    .expect("start");
    eprintln!("fake phone on tcp {}", svc.port());
    let app = SocketAddr::from((Ipv4Addr::LOCALHOST, ef_phone::UDP_PORT));
    svc.announce_to(app);
    let mut laptop: Option<String> = None;
    let mut ticks = 0u32;
    let mut paired_at: Option<u32> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(e) => match e {
                Event::Device(d) => {
                    eprintln!("connected: {} ({}) paired={}", d.name, d.id, d.paired);
                    laptop = Some(d.id.clone());
                    if d.paired {
                        paired_at = Some(ticks);
                        hello(&svc, &d.id);
                    }
                }
                Event::PairRequested { id, code } => {
                    eprintln!("pair request, code {code} — accepting");
                    svc.accept_pair(&id);
                }
                Event::Paired(d) => {
                    eprintln!("paired with {}", d.name);
                    paired_at = Some(ticks);
                    hello(&svc, &d.id);
                }
                Event::Gone(id) => eprintln!("gone: {id}"),
                Event::Clipboard { text, .. } => eprintln!("clipboard from laptop: {text:?}"),
                Event::Incoming { transfer, name, size, .. } => {
                    eprintln!("laptop sends {name} ({size} bytes)");
                    svc.accept_file(transfer, dir.join("received"));
                }
                Event::TransferDone { upload, result, .. } => eprintln!("transfer done (upload={upload}): {result:?}"),
                Event::Other { id, kind, body } => {
                    eprintln!("asked: {kind} {body}");
                    match kind.as_str() {
                        "kdeconnect.sftp.request" => {
                            svc.send_packet(&id, "kdeconnect.sftp", json!({ "ip": "127.0.0.1", "port": sftp_port, "user": "kdeconnect", "password": "secret", "path": "/storage/emulated/0", "multiPaths": ["/storage/emulated/0"], "pathNames": ["Internal storage"] }));
                        }
                        "kdeconnect.notification.request" if body.get("request").is_some() => notifications(&svc, &id),
                        "kdeconnect.sms.request_conversations" => conversations(&svc, &id),
                        "kdeconnect.sms.request_conversation" => thread(&svc, &id, body["threadID"].as_i64().unwrap_or(1)),
                        "kdeconnect.battery.request" => battery(&svc, &id),
                        _ => {}
                    }
                }
                other => eprintln!("{other:?}"),
            },
            Err(_) => {
                ticks += 1;
                if laptop.is_none() && ticks.is_multiple_of(6) {
                    svc.announce_to(app);
                }
                // A few seconds after pairing: share a file, like a phone would.
                if let (Some(id), Some(at)) = (&laptop, paired_at)
                    && ticks == at + 10
                {
                    let f = dir.join("from-phone.txt");
                    let _ = std::fs::write(&f, "Hello from the test phone\n");
                    svc.send_files(id, vec![f]);
                }
            }
        }
    }
}

fn battery(svc: &Service, id: &str) {
    svc.send_packet(id, "kdeconnect.battery", json!({ "currentCharge": 64, "isCharging": true, "thresholdEvent": 0 }));
}

fn hello(svc: &Service, id: &str) {
    battery(svc, id);
    notifications(svc, id);
}

fn notifications(svc: &Service, id: &str) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    for (k, app, title, text, reply) in [
        ("0|com.whatsapp|1", "WhatsApp", "Priya", "See you at 7? I'll bring the charger", true),
        ("0|com.whatsapp|2", "WhatsApp", "Family", "Mom: sent a photo", true),
        ("0|com.google.android.gm|3", "Gmail", "Your boarding pass", "IndiGo · 6E 2134 · BLR → DEL", false),
        ("0|in.swiggy|4", "Swiggy", "Order on the way", "Arriving in 12 minutes", false),
    ] {
        let mut body = json!({ "id": k, "appName": app, "title": title, "text": text, "isClearable": true, "time": now - 60_000, "silent": false });
        if reply {
            body["requestReplyId"] = json!(format!("reply-{k}"));
        }
        svc.send_packet(id, "kdeconnect.notification", body);
    }
}

fn conversations(svc: &Service, id: &str) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let msgs = json!([
        { "thread_id": 1, "_id": 11, "addresses": [{ "address": "+91 98765 43210" }], "body": "See you at 7? I'll bring the charger", "date": now - 120_000, "type": 1, "read": 0 },
        { "thread_id": 2, "_id": 21, "addresses": [{ "address": "Mom" }], "body": "Call me when you're free", "date": now - 3_600_000, "type": 1, "read": 1 },
        { "thread_id": 3, "_id": 31, "addresses": [{ "address": "HDFC Bank" }], "body": "Your OTP is 482913. Do not share it.", "date": now - 7_200_000, "type": 1, "read": 1 },
    ]);
    svc.send_packet(id, "kdeconnect.sms.messages", json!({ "messages": msgs }));
}

fn thread(svc: &Service, id: &str, t: i64) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let msgs = json!([
        { "thread_id": t, "_id": t * 10 + 1, "addresses": [{ "address": "+91 98765 43210" }], "body": "Are we still on for dinner?", "date": now - 600_000, "type": 1, "read": 1 },
        { "thread_id": t, "_id": t * 10 + 2, "addresses": [{ "address": "+91 98765 43210" }], "body": "Yes! Booked the table for 7:30", "date": now - 400_000, "type": 2, "read": 1 },
        { "thread_id": t, "_id": t * 10 + 3, "addresses": [{ "address": "+91 98765 43210" }], "body": "See you at 7? I'll bring the charger", "date": now - 120_000, "type": 1, "read": 0 },
    ]);
    svc.send_packet(id, "kdeconnect.sms.messages", json!({ "messages": msgs }));
}

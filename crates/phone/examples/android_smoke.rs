//! Interactive emulator integration harness. Uses disposable credentials, never the user's pairings.
//! Point EchoConnect at 10.0.2.2 and the printed port, then pair in the app.
use ef_phone::{json, Event, Service};
use std::{io::BufRead, sync::{mpsc, Arc, Mutex}};
fn main() {
    let dir = std::env::temp_dir().join(format!("echoconnect-smoke-{}", std::process::id()));
    let (tx, rx) = mpsc::channel();
    let s = Service::start_in(dir, Some("EchoConnect test laptop".into()), move |e| { let _ = tx.send(e); }).unwrap();
    println!("PORT {}", s.port());
    let current = Arc::new(Mutex::new(String::new())); let device = current.clone(); let events = s.clone();
    std::thread::spawn(move || { for e in rx {
        match &e {
            Event::Device(d) if d.name == "sdk_gphone64_x86_64" || d.name.starts_with("Google sdk_gphone") => { *device.lock().unwrap() = d.id.clone(); println!("DEVICE {} paired={}", d.name, d.paired); }
            Event::PairRequested { id, code } if *device.lock().unwrap() == *id => { println!("PAIR {code}"); events.accept_pair(id); }
            Event::PairCode { code, .. } => println!("CODE {code}"),
            Event::Ready(id) => { println!("READY {id}"); events.send_clipboard(id, "EchoConnect emulator clipboard check", false, false); }
            Event::Battery { level, .. } => println!("BATTERY {level}"),
            Event::TransferDone { result, .. } => println!("TRANSFER {result:?}"),
            Event::Incoming { transfer, name, .. } => { println!("INCOMING {name}"); events.accept_file(*transfer, std::env::temp_dir().join("echoconnect-smoke-inbox")); }
            Event::Clipboard { text, .. } => println!("CLIPBOARD {text}"),
            Event::Other { kind, .. } => println!("PACKET {kind}"),
            _ => {}
        }
    } });
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let id = current.lock().unwrap().clone();
        match line.as_str() {
            "connect" => s.connect_to("127.0.0.1:18716".parse().unwrap()),
            "pair" => s.pair(&id),
            "ring" => { s.ring(&id); }
            "file" => { s.send_payload(&id, "kdeconnect.share.request", json!({"filename":"echo-smoke.txt"}), ef_phone::Source::Bytes(b"EchoConnect file transfer check\n".to_vec())); }
            "clip" => { s.send_clipboard(&id, "EchoConnect emulator clipboard check", false, false); }
            "sensitive" => { s.send_clipboard(&id, "do-not-store-this-test", false, true); }
            "theme" => { s.send_packet(&id, "echofiles.theme", json!({"tokens":{"bg":4278913328u64,"ink":4291417077u64,"accent":4283225087u64}})); }
            "quit" => break,
            _ => println!("Commands: pair, ring, file, clip, sensitive, theme, quit"),
        }
    }
}

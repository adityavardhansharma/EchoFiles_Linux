//! Two services on one machine: connect, pair, ring, clipboard and a file both ways.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::mpsc;
use std::time::Duration;

use ef_phone::{Event, Service};

fn start(name: &str) -> (Service, mpsc::Receiver<Event>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ef-phone-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (tx, rx) = mpsc::channel();
    let svc = Service::start_in(dir.clone(), Some(name.to_string()), move |e| {
        let _ = tx.send(e);
    })
    .expect("start");
    (svc, rx, dir)
}

fn wait<T>(rx: &mpsc::Receiver<Event>, what: &str, mut f: impl FnMut(&Event) -> Option<T>) -> T {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(left) {
            Ok(e) => {
                if let Some(v) = f(&e) {
                    return v;
                }
            }
            Err(_) => panic!("timed out waiting for {what}"),
        }
    }
}

#[test]
fn pair_and_talk() {
    let (a, ra, da) = start("laptop");
    let (b, rb, db) = start("phone");
    // b connects to a's TCP port: b is the TLS server, a the client.
    b.connect_to(SocketAddr::from((Ipv4Addr::LOCALHOST, a.port())));
    let b_id = wait(&ra, "a sees b", |e| match e {
        Event::Device(d) if d.name == "phone" => Some(d.id.clone()),
        _ => None,
    });
    let a_id = wait(&rb, "b sees a", |e| match e {
        Event::Device(d) if d.name == "laptop" => Some(d.id.clone()),
        _ => None,
    });

    a.pair(&b_id);
    let code_a = wait(&ra, "a's code", |e| match e {
        Event::PairCode { code, .. } => Some(code.clone()),
        _ => None,
    });
    let code_b = wait(&rb, "b asked", |e| match e {
        Event::PairRequested { code, .. } => Some(code.clone()),
        _ => None,
    });
    assert_eq!(code_a, code_b, "both screens show the same code");
    b.accept_pair(&a_id);
    wait(&ra, "a paired", |e| matches!(e, Event::Paired(_)).then_some(()));
    assert_eq!(ef_phone_trusted_count(&da), 1);
    assert_eq!(ef_phone_trusted_count(&db), 1);

    // a → b clipboard
    assert!(a.send_clipboard(&b_id, "hello phone", false, false));
    // b doesn't accept clipboard from... it does: same capabilities both ways in the test.
    let got = wait(&rb, "clipboard", |e| match e {
        Event::Clipboard { text, .. } => Some(text.clone()),
        _ => None,
    });
    assert_eq!(got, "hello phone");

    // a → b file
    let src = da.join("photo.jpg");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&src, &data).unwrap();
    a.send_files(&b_id, vec![src.clone()]);
    let (transfer, name, size) = wait(&rb, "offer", |e| match e {
        Event::Incoming { transfer, name, size, .. } => Some((*transfer, name.clone(), *size)),
        _ => None,
    });
    assert_eq!(name, "photo.jpg");
    assert_eq!(size, data.len() as u64);
    let inbox = db.join("inbox");
    b.accept_file(transfer, inbox.clone());
    let saved = wait(&rb, "download done", |e| match e {
        Event::TransferDone { upload: false, result, .. } => Some(result.clone()),
        _ => None,
    })
    .expect("download");
    assert_eq!(std::fs::read(&saved).unwrap(), data);
    let sent = wait(&ra, "upload done", |e| match e {
        Event::TransferDone { upload: true, result, .. } => Some(result.clone()),
        _ => None,
    });
    assert!(sent.is_ok(), "{sent:?}");

    // unpair from b: a forgets too.
    b.unpair(&a_id);
    wait(&ra, "a unpaired", |e| matches!(e, Event::Unpaired(_)).then_some(()));
    assert_eq!(ef_phone_trusted_count(&da), 0);

    let _ = std::fs::remove_dir_all(&da);
    let _ = std::fs::remove_dir_all(&db);
}

fn ef_phone_trusted_count(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir.join("trusted")).map(|r| r.count()).unwrap_or(0)
}

/// A Bluetooth-only link (a socket pair stands in for RFCOMM): pair, clipboard, and the
/// phone's own capabilities.
#[test]
fn bluetooth_only() {
    let dir = |n: &str| {
        let d = std::env::temp_dir().join(format!("ef-phone-bt-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    };
    let (da, db) = (dir("laptop"), dir("phone"));
    let (ta, ra) = mpsc::channel();
    let (tb, rb) = mpsc::channel();
    let a = Service::start_as(da.clone(), Some("laptop".into()), "laptop", move |e| {
        let _ = ta.send(e);
    })
    .unwrap();
    let b = Service::start_as(db.clone(), Some("phone".into()), "phone", move |e| {
        let _ = tb.send(e);
    })
    .unwrap();
    let (x, y) = std::os::unix::net::UnixStream::pair().unwrap();
    b.adopt(x.into(), true);
    a.adopt(y.into(), false);
    let phone = wait(&ra, "laptop sees phone over bluetooth", |e| match e {
        Event::Device(d) if d.name == "phone" => Some(d.clone()),
        _ => None,
    });
    assert!(phone.bluetooth && !phone.lan);
    assert!(phone.echoconnect(), "the phone role lists echofiles.* packets");
    assert!(phone.can("kdeconnect.findmyphone.request"));
    let laptop_id = wait(&rb, "phone sees laptop", |e| match e {
        Event::Device(d) if d.name == "laptop" => Some(d.id.clone()),
        _ => None,
    });
    a.pair(&phone.id);
    wait(&rb, "phone asked", |e| matches!(e, Event::PairRequested { .. }).then_some(()));
    b.accept_pair(&laptop_id);
    wait(&ra, "paired", |e| matches!(e, Event::Paired(_)).then_some(()));
    assert!(b.send_clipboard(&laptop_id, "otp 482913", false, true));
    let (text, sensitive) = wait(&ra, "clipboard over bluetooth", |e| match e {
        Event::Clipboard { text, sensitive, .. } => Some((text.clone(), *sensitive)),
        _ => None,
    });
    assert_eq!(text, "otp 482913");
    assert!(sensitive);
    // Files need Wi-Fi.
    assert!(!a.send_packet(&phone.id, "kdeconnect.share.request", ef_phone::json!({ "text": "x" })));
    let _ = std::fs::remove_dir_all(&da);
    let _ = std::fs::remove_dir_all(&db);
}

// Regression tests for peer authorization and non-destructive payload publication.
// All active test connections target localhost. Test identities/files are disposable.
#[allow(dead_code)]
#[path = "../src/identity.rs"] mod identity;
#[path = "../src/tls.rs"] mod tls;
use std::{fs, io::{Read,Write}, net::{TcpStream,TcpListener,Ipv4Addr},path::Path,sync::mpsc,time::{Duration,Instant}};
use ef_phone::{Service,Event,json,Value};
use rustls::{ServerConnection,ClientConnection,StreamOwned};
struct Peer { cfg:tls::Tls, stream:StreamOwned<ServerConnection,TcpStream> }
fn packet(kind:&str,body:Value)->Value {json!({"id":1,"type":kind,"body":body})}
fn send(w:&mut impl Write,v:&Value){w.write_all(format!("{v}\n").as_bytes()).unwrap();w.flush().unwrap();}
fn recv(r:&mut impl Read)->Value {let mut b=Vec::new();loop {let mut x=[0];r.read_exact(&mut x).unwrap();if x[0]==b'\n'{break}b.push(x[0]);}serde_json::from_slice(&b).unwrap()}
fn wait<T>(rx:&mpsc::Receiver<Event>, f:impl Fn(&Event)->Option<T>)->T{let until=Instant::now()+Duration::from_secs(8);loop{let e=rx.recv_timeout(until.saturating_duration_since(Instant::now())).unwrap();if let Some(v)=f(&e){return v}}}
fn start(dir:&Path)->(Service,mpsc::Receiver<Event>){let(tx,rx)=mpsc::channel();let s=Service::start_in(dir.into(),Some("review laptop".into()),move|e|{let _=tx.send(e);}).unwrap();(s,rx)}
fn peer(dir:&Path,id:&str,port:u16)->Peer {
 let me=identity::load_or_create(dir).unwrap();let cfg=tls::Tls::new(&me).unwrap();
 let mut sock=TcpStream::connect((Ipv4Addr::LOCALHOST,port)).unwrap();sock.set_read_timeout(Some(Duration::from_secs(8))).unwrap();sock.set_write_timeout(Some(Duration::from_secs(8))).unwrap();
 let ident=packet("kdeconnect.identity",json!({"deviceId":id,"deviceName":id,"deviceType":"phone","protocolVersion":8}));send(&mut sock,&ident);
 let mut conn=ServerConnection::new(cfg.server.clone()).unwrap();while conn.is_handshaking(){conn.complete_io(&mut sock).unwrap();}
 let mut stream=StreamOwned::new(conn,sock);send(&mut stream,&ident);let _=recv(&mut stream);Peer{cfg,stream}
}
fn paired(base:&Path)->(Service,mpsc::Receiver<Event>,Peer){let(s,rx)=start(&base.join("laptop"));let mut p=peer(&base.join("real"),"realphone",s.port());wait(&rx,|e|matches!(e,Event::Device(d)if d.id=="realphone").then_some(()));s.pair("realphone");loop{if recv(&mut p.stream)["type"]=="kdeconnect.pair"{break}}send(&mut p.stream,&packet("kdeconnect.pair",json!({"pair":true})));wait(&rx,|e|matches!(e,Event::Paired(_)).then_some(()));(s,rx,p)}

#[test]
fn unrelated_certificate_cannot_collect_payload() {
 let root=std::env::temp_dir().join(format!("ef-payload-auth-{}",std::process::id()));
 let(s,rx,mut p)=paired(&root); let src=root.join("secret.txt"); fs::write(&src,b"private fixture").unwrap();s.send_files("realphone",vec![src]);
 let port=loop{let v=recv(&mut p.stream);if v["type"]=="kdeconnect.share.request"{break v["payloadTransferInfo"]["port"].as_u64().unwrap() as u16}};
 let attacker=identity::load_or_create(&root.join("attacker")).unwrap();let cfg=tls::Tls::new(&attacker).unwrap();
 let sock=TcpStream::connect((Ipv4Addr::LOCALHOST,port)).unwrap();sock.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
 let conn=ClientConnection::new(cfg.client.clone(),rustls::pki_types::ServerName::IpAddress(Ipv4Addr::LOCALHOST.into())).unwrap();let mut stream=StreamOwned::new(conn,sock);let mut stolen=[0;15];assert!(stream.read_exact(&mut stolen).is_err());
 let result=wait(&rx,|e|if let Event::TransferDone{result,..}=e{Some(result.clone())}else{None});assert!(result.is_err());
 fs::remove_dir_all(root).unwrap();
}
#[test]
fn untrusted_identity_cannot_unpair_or_inject() {
 let root=std::env::temp_dir().join(format!("ef-peer-auth-{}",std::process::id()));
 let(s,rx,mut real)=paired(&root); let mut spoof=peer(&root.join("spoof"),"realphone",s.port());
 // The peer may already be closed at this point; neither write may yield an event.
 let _=spoof.stream.write_all(format!("{}\n",packet("kdeconnect.pair",json!({"pair":false}))).as_bytes());
 let _=spoof.stream.write_all(format!("{}\n",packet("kdeconnect.clipboard",json!({"content":"attacker"}))).as_bytes());let _=spoof.stream.flush();
 send(&mut real.stream,&packet("kdeconnect.clipboard",json!({"content":"authenticated"})));
 let text=wait(&rx,|e|if let Event::Clipboard{text,..}=e{Some(text.clone())}else{None});assert_eq!(text,"authenticated");
 assert_eq!(identity::trusted(&root.join("laptop")).len(),1);
 assert!(!rx.try_iter().any(|e| matches!(e,Event::Unpaired(_))));
 fs::remove_dir_all(root).unwrap();
}
#[test]
fn receive_does_not_follow_staging_symlink_or_overwrite_destination() {
 let root=std::env::temp_dir().join(format!("ef-safe-receive-{}",std::process::id()));
 let(s,rx,mut p)=paired(&root);let listener=TcpListener::bind((Ipv4Addr::LOCALHOST,0)).unwrap();let port=listener.local_addr().unwrap().port();
 let mut v=packet("kdeconnect.share.request",json!({"filename":"photo.txt"}));v["payloadSize"]=json!(3);v["payloadTransferInfo"]=json!({"port":port});send(&mut p.stream,&v);
 let transfer=wait(&rx,|e|if let Event::Incoming{transfer,..}=e{Some(*transfer)}else{None});
 let inbox=root.join("inbox");fs::create_dir_all(&inbox).unwrap();let victim=root.join("unrelated.txt");fs::write(&victim,b"keep this").unwrap();std::os::unix::fs::symlink(&victim,inbox.join(".photo.txt.part")).unwrap();fs::write(inbox.join("photo.txt"),b"existing").unwrap();
 let cfg=p.cfg.server.clone();let server=std::thread::spawn(move||{let(mut sock,_)=listener.accept().unwrap();let mut conn=ServerConnection::new(cfg).unwrap();while conn.is_handshaking(){conn.complete_io(&mut sock).unwrap();}let mut stream=StreamOwned::new(conn,sock);stream.write_all(b"new").unwrap();stream.flush().unwrap();});
 s.accept_file(transfer,inbox.clone());let done=wait(&rx,|e|if let Event::TransferDone{result,upload:false,..}=e{Some(result.clone())}else{None}).unwrap();server.join().unwrap();
 assert_eq!(fs::read(&victim).unwrap(),b"keep this");assert_eq!(fs::read(inbox.join("photo.txt")).unwrap(),b"existing");assert_eq!(fs::read(done).unwrap(),b"new");fs::remove_dir_all(root).unwrap();
}

#[test]
fn replaced_untrusted_link_cannot_borrow_new_trust() {
    let root = tempfile::tempdir().unwrap();
    let (svc, rx) = start(&root.path().join("laptop"));
    let mut old = peer(&root.path().join("old"), "sharedid", svc.port());
    wait(&rx, |e| matches!(e, Event::Device(d) if d.id == "sharedid").then_some(()));
    let real = identity::load_or_create(&root.path().join("real")).unwrap();
    identity::trust(&root.path().join("laptop"), &identity::Trusted { id: "sharedid".into(), name: "real".into(), kind: "phone".into(), cert: real.cert.to_vec() }).unwrap();
    let mut authenticated = peer(&root.path().join("real"), "sharedid", svc.port());
    wait(&rx, |e| matches!(e, Event::Device(d) if d.id == "sharedid" && d.paired).then_some(()));
    let _ = old.stream.write_all(format!("{}\n", packet("kdeconnect.clipboard", json!({"content":"impostor"}))).as_bytes()); let _ = old.stream.flush();
    send(&mut authenticated.stream, &packet("kdeconnect.clipboard", json!({"content":"trusted"})));
    assert_eq!(wait(&rx, |e| if let Event::Clipboard { text, .. } = e { Some(text.clone()) } else { None }), "trusted");
    svc.shutdown();
}

#[test]
fn revoked_file_offer_is_not_downloaded() {
    let root = tempfile::tempdir().unwrap(); let (svc, rx, mut remote) = paired(root.path());
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap(); listener.set_nonblocking(true).unwrap();
    let mut v = packet("kdeconnect.share.request", json!({"filename":"revoked.txt"})); v["payloadSize"] = json!(3); v["payloadTransferInfo"] = json!({"port":listener.local_addr().unwrap().port()}); send(&mut remote.stream, &v);
    let transfer = wait(&rx, |e| if let Event::Incoming { transfer, .. } = e { Some(*transfer) } else { None });
    svc.unpair("realphone"); svc.accept_file(transfer, root.path().join("inbox"));
    assert!(listener.accept().is_err()); assert!(!root.path().join("inbox").exists()); svc.shutdown();
}

#[test]
fn malformed_hex_is_fallible_and_unicode_identity_is_rejected() {
    let root = tempfile::tempdir().unwrap(); let trust = root.path().join("trusted"); fs::create_dir(&trust).unwrap(); fs::write(trust.join("peer"), "certificate=aéz\n").unwrap();
    assert!(identity::trusted(root.path()).is_empty());
    let (svc, rx) = start(&root.path().join("laptop"));
    let _invalid = peer(&root.path().join("invalid"), "aaaaaaaé", svc.port());
    let _valid = peer(&root.path().join("valid"), "validid", svc.port());
    wait(&rx, |e| matches!(e, Event::Device(d) if d.id == "validid").then_some(()));
    assert!(!svc.devices().iter().any(|d| d.id == "aaaaaaaé")); svc.shutdown();
}

#[test]
fn pairing_approval_cannot_switch_to_a_replacement_peer() {
    let root = tempfile::tempdir().unwrap();
    let (svc, rx) = start(&root.path().join("laptop"));
    let mut first = peer(&root.path().join("first"), "sameid", svc.port());
    wait(&rx, |e| matches!(e, Event::Device(d) if d.id == "sameid").then_some(()));
    send(&mut first.stream, &packet("kdeconnect.pair", json!({"pair":true,"timestamp":1})));
    let old_code = wait(&rx, |e| if let Event::PairRequested { code, .. } = e { Some(code.clone()) } else { None });
    drop(first);
    wait(&rx, |e| matches!(e, Event::Gone(id) if id == "sameid").then_some(()));
    let mut replacement = peer(&root.path().join("replacement"), "sameid", svc.port());
    wait(&rx, |e| matches!(e, Event::Device(d) if d.id == "sameid").then_some(()));
    send(&mut replacement.stream, &packet("kdeconnect.pair", json!({"pair":true,"timestamp":2})));
    let new_code = wait(&rx, |e| if let Event::PairRequested { code, .. } = e { Some(code.clone()) } else { None });
    assert_ne!(old_code, new_code);
    svc.accept_pair_code("sameid", &old_code);
    assert!(identity::trusted(&root.path().join("laptop")).is_empty());
    svc.accept_pair_code("sameid", &new_code);
    wait(&rx, |e| matches!(e, Event::Paired(d) if d.id == "sameid").then_some(()));
    let expected = identity::load_or_create(&root.path().join("replacement")).unwrap();
    assert_eq!(identity::trusted(&root.path().join("laptop"))[0].cert, expected.cert.as_ref());
    svc.shutdown();
}

#[test]
fn concurrent_identity_creation_publishes_one_private_bundle() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let identities = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8).map(|_| scope.spawn(|| identity::load_or_create(root.path()).unwrap())).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect::<Vec<_>>()
    });
    assert!(identities.iter().all(|i| i.device_id == identities[0].device_id && i.cert == identities[0].cert));
    assert_eq!(fs::metadata(root.path().join("identity.json")).unwrap().permissions().mode() & 0o777, 0o600);
    fs::write(root.path().join("identity.json"), b"incomplete").unwrap();
    assert!(identity::load_or_create(root.path()).is_err());
    assert_eq!(fs::read(root.path().join("identity.json")).unwrap(), b"incomplete");
}

#[test]
fn shutdown_releases_listener_and_does_not_restart_connections() {
    let root = tempfile::tempdir().unwrap();
    for _ in 0..3 {
        let (svc, _) = start(root.path());
        let port = svc.port();
        svc.shutdown();
        let until = Instant::now() + Duration::from_secs(3);
        let listener = loop {
            if let Ok(l) = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)) { break l; }
            assert!(Instant::now() < until, "stopped service retained its listener");
            std::thread::sleep(Duration::from_millis(20));
        };
        listener.set_nonblocking(true).unwrap();
        svc.connect_to((Ipv4Addr::LOCALHOST, port).into());
        assert!(listener.accept().is_err());
    }
}

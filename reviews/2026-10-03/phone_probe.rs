// Temporarily copy into crates/phone/tests/deep_review_probe.rs to run.
// All active test connections target localhost. Test identities/files are disposable.
#[path = "../src/identity.rs"] mod identity;
#[path = "../src/tls.rs"] mod tls;
use std::{fs, io::{Read,Write}, net::{TcpStream,TcpListener,SocketAddr,Ipv4Addr},path::{Path,PathBuf},sync::mpsc,time::{Duration,Instant}};
use ef_phone::{Service,Event,json,Value};
use rustls::{ServerConnection,ClientConnection,StreamOwned};
struct Peer { me:identity::Identity, cfg:tls::Tls, stream:StreamOwned<ServerConnection,TcpStream> }
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
 let mut stream=StreamOwned::new(conn,sock);send(&mut stream,&ident);let _=recv(&mut stream);Peer{me,cfg,stream}
}
fn paired(base:&Path)->(Service,mpsc::Receiver<Event>,Peer){let(s,rx)=start(&base.join("laptop"));let mut p=peer(&base.join("real"),"realphone",s.port());wait(&rx,|e|matches!(e,Event::Device(d)if d.id=="realphone").then_some(()));s.pair("realphone");loop{if recv(&mut p.stream)["type"]=="kdeconnect.pair"{break}}send(&mut p.stream,&packet("kdeconnect.pair",json!({"pair":true})));wait(&rx,|e|matches!(e,Event::Paired(_)).then_some(()));(s,rx,p)}
#[test]
fn isolated_security_reproductions(){
 let root=std::env::temp_dir().join(format!("ef-phone-deep-review-{}",std::process::id()));fs::create_dir_all(&root).unwrap();println!("fixtures: {}",root.display());
 // An unrelated certificate can collect an outgoing file.
 let(s,_rx,mut p)=paired(&root.join("upload"));let src=root.join("secret.txt");fs::write(&src,b"review-only-secret").unwrap();s.send_files("realphone",vec![src]);
 let port=loop{let v=recv(&mut p.stream);if v["type"]=="kdeconnect.share.request"{break v["payloadTransferInfo"]["port"].as_u64().unwrap() as u16}};
 let attacker=identity::load_or_create(&root.join("attacker")).unwrap();let cfg=tls::Tls::new(&attacker).unwrap();assert_ne!(attacker.cert,p.me.cert);
 let sock=TcpStream::connect((Ipv4Addr::LOCALHOST,port)).unwrap();sock.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
 let conn=ClientConnection::new(cfg.client.clone(),rustls::pki_types::ServerName::IpAddress(Ipv4Addr::LOCALHOST.into())).unwrap();let mut stream=StreamOwned::new(conn,sock);let mut stolen=[0;18];stream.read_exact(&mut stolen).unwrap();assert_eq!(&stolen,b"review-only-secret");println!("UPLOAD: unrelated certificate received full file");
 // A pre-existing .part symlink redirects a download to an unrelated file.
 let(s,rx,mut p)=paired(&root.join("download"));let listener=TcpListener::bind((Ipv4Addr::LOCALHOST,0)).unwrap();let port=listener.local_addr().unwrap().port();
 let mut v=packet("kdeconnect.share.request",json!({"filename":"photo.txt"}));v["payloadSize"]=json!(3);v["payloadTransferInfo"]=json!({"port":port});send(&mut p.stream,&v);
 let transfer=wait(&rx,|e|if let Event::Incoming{transfer,..}=e {Some(*transfer)}else{None});
 let inbox=root.join("inbox");fs::create_dir_all(&inbox).unwrap();let victim=root.join("unrelated.txt");fs::write(&victim,b"keep this").unwrap();std::os::unix::fs::symlink(&victim,inbox.join(".photo.txt.part")).unwrap();
 let cfg=p.cfg.server.clone();let server=std::thread::spawn(move||{let(mut sock,_)=listener.accept().unwrap();sock.set_read_timeout(Some(Duration::from_secs(8))).unwrap();let mut conn=ServerConnection::new(cfg).unwrap();while conn.is_handshaking(){conn.complete_io(&mut sock).unwrap();}let mut stream=StreamOwned::new(conn,sock);stream.write_all(b"new").unwrap();stream.flush().unwrap();});
 s.accept_file(transfer,inbox);let done=wait(&rx,|e|if let Event::TransferDone{result,upload:false,..}=e{Some(result.clone())}else{None});assert!(done.is_ok(),"{done:?}");server.join().unwrap();assert_eq!(fs::read(&victim).unwrap(),b"new");println!("DOWNLOAD: .part symlink overwrote unrelated file");
 // An unpaired impostor can erase an existing trust record.
 let(s,rx,_p)=paired(&root.join("forget"));let mut spoof=peer(&root.join("forget/spoof"),"realphone",s.port());wait(&rx,|e|matches!(e,Event::Device(d)if d.id=="realphone"&&!d.paired).then_some(()));send(&mut spoof.stream,&packet("kdeconnect.pair",json!({"pair":false})));wait(&rx,|e|matches!(e,Event::Unpaired(_)).then_some(()));assert!(identity::trusted(&root.join("forget/laptop")).is_empty());println!("UNPAIR: untrusted certificate erased real pairing");
 // The old untrusted connection is authorized by a newer trusted connection's state.
 let base=root.join("stale");let(s,rx)=start(&base.join("laptop"));let mut spoof=peer(&base.join("spoof"),"realphone",s.port());wait(&rx,|e|matches!(e,Event::Device(_)).then_some(()));
 let real=identity::load_or_create(&base.join("real")).unwrap();identity::trust(&base.join("laptop"),&identity::Trusted{id:"realphone".into(),name:"real".into(),kind:"phone".into(),cert:real.cert.to_vec()}).unwrap();let _real=peer(&base.join("real"),"realphone",s.port());wait(&rx,|e|matches!(e,Event::Device(d)if d.paired).then_some(()));
 send(&mut spoof.stream,&packet("kdeconnect.clipboard",json!({"content":"untrusted injected text"})));let text=wait(&rx,|e|if let Event::Clipboard{text,..}=e{Some(text.clone())}else{None});assert_eq!(text,"untrusted injected text");println!("SESSION: old untrusted connection delivered authenticated clipboard event");
}

#[test]
fn unicode_identity_panics_link_worker() {
 let root=std::env::temp_dir().join(format!("ef-phone-unicode-review-{}",std::process::id()));
 let (service,rx)=start(&root.join("laptop"));
 let mut remote=peer(&root.join("peer"),"aaaaaaaé",service.port());
 let mut byte=[0];
 let closed=match remote.stream.read(&mut byte) { Ok(0)=>true, Err(e)=>!matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut), _=>false };
 assert!(closed,"expected peer worker to terminate");
 assert!(!rx.try_iter().any(|e|matches!(e,Event::Device(_))));
 println!("UNICODE: unpaired UTF-8 identity terminated link worker before Device event; inspect panic above");
}

//! Stopping a service frees its port. A test binary of its own: the other tests' services
//! take the lowest free port in the range as they start, so in parallel one of them can
//! grab the port this test just freed and make it look retained.

use std::net::{Ipv4Addr, TcpListener};
use std::time::{Duration, Instant};

use ef_phone::Service;

#[test]
fn shutdown_releases_listener_and_does_not_restart_connections() {
    let root = tempfile::tempdir().unwrap();
    for _ in 0..3 {
        let svc = Service::start_in(root.path().into(), Some("review laptop".into()), |_| {}).unwrap();
        let port = svc.port();
        svc.shutdown();
        let until = Instant::now() + Duration::from_secs(3);
        let listener = loop {
            if let Ok(l) = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)) {
                break l;
            }
            assert!(Instant::now() < until, "stopped service retained its listener");
            std::thread::sleep(Duration::from_millis(20));
        };
        listener.set_nonblocking(true).unwrap();
        svc.connect_to((Ipv4Addr::LOCALHOST, port).into());
        assert!(listener.accept().is_err());
    }
}

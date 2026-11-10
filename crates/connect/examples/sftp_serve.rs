//! Serve a folder over EchoConnect's SFTP server: `cargo run -p echoconnect-core --example sftp_serve -- <dir>`.
//! Prints the port, user, password and host-key fingerprint EchoFiles needs to mount it.
fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let s = echoconnect::sftp::Server::start(vec![dir], [7; 32]).expect("start");
    println!("{} {} {} {}", s.port(), s.user(), s.password(), s.fingerprint());
    std::thread::sleep(std::time::Duration::from_secs(600));
}

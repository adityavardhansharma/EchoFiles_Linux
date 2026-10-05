//! Serve a folder over EchoConnect's SFTP server: `cargo run -p echoconnect-core --example sftp_serve -- <dir>`.
fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let s = echoconnect::sftp::Server::start(vec![dir]).expect("start");
    println!("{} {} {}", s.port(), s.user(), s.password());
    std::thread::sleep(std::time::Duration::from_secs(60));
}

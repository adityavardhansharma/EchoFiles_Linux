//! Mount an EchoConnect file server by hand:
//! `cargo run -p echofiles-phonefs --example mount -- <ip> <port> <user> <password> <fingerprint> <phone folder> <mount point>`
//! (`sftp_serve` in echoconnect-core prints the first five). Ctrl-C or 10 minutes unmounts.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    assert_eq!(a.len(), 7, "ip port user password fingerprint base mountpoint");
    let remote = ef_phonefs::Remote {
        ip: a[0].clone(),
        port: a[1].parse().expect("port"),
        user: a[2].clone(),
        password: a[3].clone(),
        fingerprint: a[4].clone(),
        base: a[5].clone(),
    };
    let fs = ef_phonefs::mount(&remote, std::path::Path::new(&a[6])).unwrap_or_else(|e| panic!("{e}"));
    println!("mounted at {}", fs.root().display());
    std::thread::sleep(std::time::Duration::from_secs(600));
}

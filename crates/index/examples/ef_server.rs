//! Serve `ef` without the app (for benchmarks and tests): `cargo run --release --example
//! ef_server` with the same `XDG_RUNTIME_DIR` as the `ef` you test with.
fn main() {
    if !ef_index::cli::server::spawn() {
        eprintln!("couldn't listen on {}", ef_index::cli::server::socket_path().display());
        std::process::exit(1);
    }
    eprintln!("serving ef on {}", ef_index::cli::server::socket_path().display());
    loop {
        std::thread::park();
    }
}

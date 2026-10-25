//! rclone's remote control: HTTP/1.1 POST with a JSON body over the private Unix socket.
//!
//! Hand-written on purpose (no TLS, no redirects, one request per connection): a request
//! line, `Content-Length`, the body; the reply is read to EOF (`Connection: close`) and may
//! be chunked. Connecting to a local socket costs microseconds, so each call opens its own
//! connection and calls never wait behind each other.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::error::{Error, Kind};

/// Budgets from the design: metadata 5 s, live search 10 s.
pub const METADATA: Duration = Duration::from_secs(5);
pub const SEARCH: Duration = Duration::from_secs(10);
/// Mounts and unmounts can take a while (first listing of a big Drive); still bounded.
pub const SLOW: Duration = Duration::from_secs(120);

/// A handle to the rcd. Cheap to clone; holds only the socket path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rc {
    socket: PathBuf,
}

impl Rc {
    pub fn new(socket: impl Into<PathBuf>) -> Rc {
        Rc { socket: socket.into() }
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Call `method` (`core/version`, `operations/list`, …) with `params`.
    pub fn call(&self, method: &str, params: &Value, timeout: Duration) -> Result<Value, Error> {
        let deadline = Instant::now() + timeout;
        let mut s = UnixStream::connect(&self.socket).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => Error::new(Kind::NotRunning, e.to_string()),
            _ => Error::new(Kind::Io, e.to_string()),
        })?;
        s.set_write_timeout(Some(timeout)).map_err(io)?;
        let body = serde_json::to_vec(params).map_err(|e| Error::new(Kind::Io, e.to_string()))?;
        let head = format!("POST /{method} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
        s.write_all(head.as_bytes()).map_err(io)?;
        s.write_all(&body).map_err(io)?;
        let mut reply = Vec::new();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Error::new(Kind::Timeout, format!("{method} took longer than {} s", timeout.as_secs())));
            }
            s.set_read_timeout(Some(left)).map_err(io)?;
            match s.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => reply.extend_from_slice(&buf[..n]),
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                    return Err(Error::new(Kind::Timeout, format!("{method} took longer than {} s", timeout.as_secs())));
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io(e)),
            }
        }
        parse_reply(&reply)
    }

    /// `core/version`: the rcd is up and answering.
    pub fn ping(&self) -> Result<(), Error> {
        self.call("core/version", &Value::Object(Default::default()), Duration::from_secs(2)).map(|_| ())
    }
}

fn io(e: std::io::Error) -> Error {
    Error::new(Kind::Io, e.to_string())
}

/// Status, headers and body of an HTTP/1.1 reply; rc errors become [`Error`]s.
pub fn parse_reply(raw: &[u8]) -> Result<Value, Error> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| Error::new(Kind::Io, "rclone sent an incomplete reply"))?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let mut lines = head.split("\r\n");
    let status: u16 = lines.next().and_then(|l| l.split_whitespace().nth(1)).and_then(|c| c.parse().ok()).ok_or_else(|| Error::new(Kind::Io, "rclone sent a reply without a status"))?;
    let chunked = lines.any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let rest = &raw[split + 4..];
    let body = if chunked { dechunk(rest)? } else { rest.to_vec() };
    let value: Value = if body.iter().all(|b| b.is_ascii_whitespace()) {
        Value::Object(Default::default())
    } else {
        serde_json::from_slice(&body).map_err(|e| Error::new(Kind::Io, format!("rclone sent bad JSON: {e}")))?
    };
    if status >= 400 {
        let msg = value.get("error").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("rclone answered {status}"));
        return Err(Error::from_rclone(&msg));
    }
    Ok(value)
}

fn dechunk(mut b: &[u8]) -> Result<Vec<u8>, Error> {
    let bad = || Error::new(Kind::Io, "rclone sent a bad chunked reply");
    let mut out = Vec::new();
    loop {
        let eol = b.windows(2).position(|w| w == b"\r\n").ok_or_else(bad)?;
        let size_text = String::from_utf8_lossy(&b[..eol]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16).map_err(|_| bad())?;
        b = &b[eol + 2..];
        if size == 0 {
            return Ok(out);
        }
        if b.len() < size + 2 {
            return Err(bad());
        }
        out.extend_from_slice(&b[..size]);
        b = &b[size + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::net::UnixListener;

    #[test]
    fn parses_plain_chunked_and_errors() {
        let ok = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 18\r\n\r\n{\"version\":\"1.68\"}";
        assert_eq!(parse_reply(ok).unwrap()["version"], "1.68");
        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n{\"a\":\r\n2\r\n1}\r\n0\r\n\r\n";
        assert_eq!(parse_reply(chunked).unwrap()["a"], 1);
        let err = b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 72\r\n\r\n{\"error\":\"googleapi: Error 403: Rate Limit Exceeded, rateLimitExceeded\"}";
        assert_eq!(parse_reply(err).unwrap_err().kind, Kind::RateLimited);
        assert_eq!(parse_reply(b"HTTP/1.1 200 OK\r\n\r\n").unwrap(), json!({}));
        assert!(parse_reply(b"garbage").is_err());
    }

    /// A fake rcd answering one canned reply per connection.
    fn fake(replies: Vec<&'static [u8]>) -> (tempfile::TempDir, Rc, std::thread::JoinHandle<Vec<String>>) {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("rc.sock");
        let l = UnixListener::bind(&sock).unwrap();
        let t = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for r in replies {
                let (mut c, _) = l.accept().unwrap();
                let mut buf = vec![0u8; 4096];
                let mut got = Vec::new();
                // Read the head and the declared body.
                loop {
                    let n = c.read(&mut buf).unwrap();
                    got.extend_from_slice(&buf[..n]);
                    if let Some(i) = got.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&got[..i]).to_string();
                        let len: usize = head.lines().find_map(|l| l.strip_prefix("Content-Length: ")).unwrap().parse().unwrap();
                        if got.len() >= i + 4 + len {
                            seen.push(String::from_utf8_lossy(&got).to_string());
                            break;
                        }
                    }
                    if n == 0 { break; }
                }
                if !r.is_empty() { c.write_all(r).unwrap(); }
            }
            seen
        });
        (dir, Rc::new(sock), t)
    }

    #[test]
    fn talks_http_over_the_socket() {
        let (_d, rc, t) = fake(vec![b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\n{\"ok\":true}"]);
        let v = rc.call("operations/list", &json!({"fs": "ef-a:", "remote": "Docs"}), METADATA).unwrap();
        assert_eq!(v["ok"], true);
        let seen = t.join().unwrap();
        assert!(seen[0].starts_with("POST /operations/list HTTP/1.1\r\n"));
        assert!(seen[0].ends_with("{\"fs\":\"ef-a:\",\"remote\":\"Docs\"}"));
    }

    #[test]
    fn not_running_and_timeouts() {
        let dir = tempfile::tempdir().unwrap();
        let rc = Rc::new(dir.path().join("missing.sock"));
        assert_eq!(rc.ping().unwrap_err().kind, Kind::NotRunning);
        // A server that accepts but never answers.
        let sock = dir.path().join("slow.sock");
        let l = UnixListener::bind(&sock).unwrap();
        let t = std::thread::spawn(move || { let (c, _) = l.accept().unwrap(); std::thread::sleep(Duration::from_millis(600)); drop(c); });
        let e = Rc::new(&sock).call("core/version", &json!({}), Duration::from_millis(200)).unwrap_err();
        assert_eq!(e.kind, Kind::Timeout);
        t.join().unwrap();
    }
}

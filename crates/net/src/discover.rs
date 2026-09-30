//! Servers announcing themselves on the local network (mDNS/DNS-SD through Avahi): NAS
//! boxes, Macs and Linux machines sharing files, printers-with-storage…

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::address::{Address, Protocol};

/// One server found nearby.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nearby {
    /// What it calls itself: "Living Room NAS".
    pub name: String,
    pub address: Address,
}

fn protocol_for(service: &str) -> Option<Protocol> {
    Some(match service {
        "_smb._tcp" => Protocol::Smb,
        "_sftp-ssh._tcp" | "_ssh._tcp" => Protocol::Sftp,
        "_ftp._tcp" => Protocol::Ftp,
        _ => return None,
    })
}

/// Avahi's parsable output escapes bytes as `\DDD` (decimal).
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 4 <= b.len() && b[i + 1..i + 4].iter().all(u8::is_ascii_digit)
            && let Ok(v) = std::str::from_utf8(&b[i + 1..i + 4]).unwrap_or("").parse::<u8>() {
                out.push(v);
                i += 4;
                continue;
            }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse `avahi-browse -p -r` output: resolved lines start with `=`.
/// `=;eth0;IPv4;Living\032Room;_smb._tcp;local;nas.local;192.168.1.5;445;"txt"`
pub fn parse(output: &str) -> Vec<Nearby> {
    let mut out: Vec<Nearby> = Vec::new();
    for line in output.lines() {
        let f: Vec<&str> = line.split(';').collect();
        if f.len() < 9 || f[0] != "=" {
            continue;
        }
        let Some(protocol) = protocol_for(f[4]) else { continue };
        // Prefer the .local name (survives DHCP changes); the IP when there's none.
        let host = if f[6].is_empty() { f[7].to_string() } else { f[6].to_string() };
        let port = f[8].parse::<u16>().ok().filter(|&p| p != protocol.default_port());
        let address = Address { protocol, host, port, user: None, domain: None, share: None, path: String::new() };
        // SFTP is announced as both _ssh and _sftp-ssh; IPv4 and IPv6 repeat everything.
        if out.iter().any(|n| n.address == address) {
            continue;
        }
        out.push(Nearby { name: unescape(f[3]), address });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.address.protocol.label().cmp(b.address.protocol.label())));
    out
}

/// Look for servers for up to `limit`. Empty when Avahi isn't installed or running.
pub fn scan(limit: Duration) -> Result<Vec<Nearby>, String> {
    let mut child = Command::new("avahi-browse")
        .args(["--all", "--resolve", "--parsable", "--terminate", "--no-db-lookup"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Avahi isn't installed, so EchoFiles can't look for servers nearby.".to_string())?;
    let until = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    let mut text = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut text);
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    if text.is_empty() && err.to_lowercase().contains("daemon not running") {
        return Err("The Avahi service isn't running, so EchoFiles can't look for servers nearby.".into());
    }
    Ok(parse(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_resolved_services() {
        let text = "+;eth0;IPv4;Living\\032Room;_smb._tcp;local\n\
=;eth0;IPv4;Living\\032Room;_smb._tcp;local;nas.local;192.168.1.5;445;\"\"\n\
=;eth0;IPv6;Living\\032Room;_smb._tcp;local;nas.local;fe80::1;445;\"\"\n\
=;eth0;IPv4;box;_ssh._tcp;local;box.local;192.168.1.9;22;\"\"\n\
=;eth0;IPv4;box;_sftp-ssh._tcp;local;box.local;192.168.1.9;22;\"\"\n\
=;eth0;IPv4;printer;_ipp._tcp;local;p.local;192.168.1.3;631;\"\"\n";
        let n = parse(text);
        assert_eq!(n.len(), 2);
        assert_eq!(n[0].name, "box");
        assert_eq!(n[0].address.protocol, Protocol::Sftp);
        assert_eq!(n[1].name, "Living Room");
        assert_eq!(n[1].address.uri(), "smb://nas.local/");
    }
}

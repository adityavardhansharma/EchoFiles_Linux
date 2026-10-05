//! This laptop's KDE Connect identity (device id, certificate, key) and the phones it trusts.
//!
//! Everything lives in `~/.config/echofiles/phone/`: an atomically published mode-600
//! `identity.json` bundle (legacy split identities remain readable), and `trusted/<device id>` — one file per paired phone
//! holding its name, type and certificate. A phone is trusted only while it presents the
//! very certificate it paired with.

use std::io;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sha2::{Digest, Sha256};

pub fn dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")).join("echofiles/phone")
}

/// Who we are on the network.
pub struct Identity {
    pub device_id: String,
    pub name: String,
    pub cert: CertificateDer<'static>,
    pub key: PrivatePkcs8KeyDer<'static>,
}

impl Identity {
    pub fn key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(self.key.clone_key())
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    s.len().is_multiple_of(2).then_some(())?;
    s.as_bytes().chunks_exact(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()).collect()
}

pub fn host_name() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: gethostname writes at most `len` bytes into `buf`.
    let ok = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } == 0;
    let name = if ok { String::from_utf8_lossy(&buf[..buf.iter().position(|&b| b == 0).unwrap_or(buf.len())]).into_owned() } else { String::new() };
    if name.trim().is_empty() { "EchoFiles".into() } else { name }
}

/// Load the identity, making one the first time (32 hex characters, the form current
/// KDE Connect expects; the certificate's common name is the device id).
pub fn load_or_create(d: &Path) -> io::Result<Identity> {
    std::fs::create_dir_all(d)?;
    let lock = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(d.join("identity.lock"))?;
    // SAFETY: the descriptor belongs to this scoped File; close releases the lock.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 { return Err(io::Error::last_os_error()); }
    let bundle = d.join("identity.json");
    match std::fs::read(&bundle) {
        Ok(bytes) => {
            let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
            let id = v["id"].as_str().filter(|id| safe_id(id)).ok_or_else(|| io::Error::other("invalid identity ID"))?;
            let cert = v["certificate"].as_str().and_then(unhex).ok_or_else(|| io::Error::other("invalid identity certificate"))?;
            let key = v["key"].as_str().and_then(unhex).ok_or_else(|| io::Error::other("invalid identity key"))?;
            let identity = Identity { device_id: id.into(), name: host_name(), cert: CertificateDer::from(cert), key: PrivatePkcs8KeyDer::from(key) };
            crate::tls::Tls::new(&identity).map_err(|e| io::Error::other(format!("Invalid stored phone identity: {e}")))?;
            return Ok(identity);
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => {},
        Err(e) => return Err(e),
    }
    let id_file = d.join("device-id");
    let cert_file = d.join("certificate.der");
    let key_file = d.join("private-key.der");
    if let (Ok(id), Ok(cert), Ok(key)) = (std::fs::read_to_string(&id_file), std::fs::read(&cert_file), std::fs::read(&key_file)) {
        let id = id.trim().to_string();
        if safe_id(&id) {
            let identity = Identity { device_id: id, name: host_name(), cert: CertificateDer::from(cert), key: PrivatePkcs8KeyDer::from(key) };
            crate::tls::Tls::new(&identity).map_err(|e| io::Error::other(format!("Invalid stored phone identity: {e}")))?;
            return Ok(identity);
        }
        return Err(io::Error::other("Invalid stored phone device ID"));
    }
    if [&id_file, &cert_file, &key_file].iter().any(|p| p.exists()) { return Err(io::Error::other("Incomplete phone identity; restore the identity files before retrying")); }
    let mut raw = [0u8; 16];
    getrandom::fill(&mut raw).map_err(|e| io::Error::other(e.to_string()))?;
    let id = hex(&raw);
    let key = rcgen::KeyPair::generate().map_err(|e| io::Error::other(e.to_string()))?;
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).map_err(|e| io::Error::other(e.to_string()))?;
    params.distinguished_name.push(rcgen::DnType::CommonName, id.clone());
    params.distinguished_name.push(rcgen::DnType::OrganizationName, "KDE");
    params.distinguished_name.push(rcgen::DnType::OrganizationalUnitName, "KDE Connect");
    params.not_before = rcgen::date_time_ymd(2024, 1, 1);
    params.not_after = rcgen::date_time_ymd(2034, 1, 1);
    let cert = params.self_signed(&key).map_err(|e| io::Error::other(e.to_string()))?;
    let key_der = key.serialize_der();
    let bytes = serde_json::to_vec(&serde_json::json!({"id":id, "certificate":hex(cert.der()), "key":hex(&key_der)})).map_err(io::Error::other)?;
    write_private(&bundle, &bytes)?;
    Ok(Identity { device_id: id, name: host_name(), cert: cert.der().clone(), key: PrivatePkcs8KeyDer::from(key_der) })
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().ok_or_else(|| io::Error::other("missing parent"))?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    std::fs::File::open(dir)?.sync_all()
}

/// The certificate's SubjectPublicKeyInfo, DER.
pub fn public_key(cert: &[u8]) -> Option<Vec<u8>> {
    let (_, c) = x509_parser::parse_x509_certificate(cert).ok()?;
    Some(c.tbs_certificate.subject_pki.raw.to_vec())
}

/// The code both screens show while pairing: SHA-256 over the two public keys (larger
/// first) and the pairing request's timestamp, first 8 hex digits, upper case.
pub fn verification_key(ours: &[u8], theirs: &[u8], timestamp: u64) -> String {
    let (Some(a), Some(b)) = (public_key(theirs), public_key(ours)) else { return "????????".into() };
    let (a, b) = if a < b { (b, a) } else { (a, b) };
    let mut h = Sha256::new();
    h.update(&a);
    h.update(&b);
    if timestamp != 0 {
        h.update(timestamp.to_string().as_bytes());
    }
    hex(&h.finalize())[..8].to_uppercase()
}

/// A phone we've paired with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trusted {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub cert: Vec<u8>,
}

fn trusted_dir(d: &Path) -> PathBuf {
    d.join("trusted")
}

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn trusted(d: &Path) -> Vec<Trusted> {
    let Ok(rd) = std::fs::read_dir(trusted_dir(d)) else { return Vec::new() };
    let mut out: Vec<Trusted> = rd
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(e.path()).ok()?;
            let mut t = Trusted { id, name: String::new(), kind: "phone".into(), cert: Vec::new() };
            for line in text.lines() {
                match line.split_once('=') {
                    Some(("name", v)) => t.name = v.to_string(),
                    Some(("type", v)) => t.kind = v.to_string(),
                    Some(("certificate", v)) => t.cert = unhex(v)?,
                    _ => {}
                }
            }
            (!t.cert.is_empty() && safe_id(&t.id)).then_some(t)
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn trust(d: &Path, t: &Trusted) -> io::Result<()> {
    if !safe_id(&t.id) {
        return Err(io::Error::other("bad device id"));
    }
    std::fs::create_dir_all(trusted_dir(d))?;
    let name = t.name.replace('\n', " ");
    write_private(&trusted_dir(d).join(&t.id), format!("name={name}\ntype={}\ncertificate={}\n", t.kind.replace(['\n', '\r'], " "), hex(&t.cert)).as_bytes())
}

pub fn forget(d: &Path, id: &str) -> io::Result<()> {
    if !safe_id(id) {
        return Ok(());
    }
    match std::fs::remove_file(trusted_dir(d).join(id)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!(unhex(&hex(&[0, 1, 254, 255])).unwrap(), vec![0, 1, 254, 255]);
        assert!(unhex("abc").is_none());
    }

    #[test]
    fn verification_key_is_symmetric() {
        let a = rcgen::generate_simple_self_signed(vec!["a".into()]).unwrap().cert.der().to_vec();
        let b = rcgen::generate_simple_self_signed(vec!["b".into()]).unwrap().cert.der().to_vec();
        let k1 = verification_key(&a, &b, 1_700_000_000);
        let k2 = verification_key(&b, &a, 1_700_000_000);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 8);
        assert_ne!(k1, verification_key(&a, &b, 1_700_000_001));
    }
}

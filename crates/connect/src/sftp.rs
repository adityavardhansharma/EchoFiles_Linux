//! The SFTP server EchoFiles browses the phone through: one random user and password per
//! start, an Ed25519 host key that stays the same across starts (EchoFiles checks it
//! against the fingerprint sent over the paired link), and only the shared folders
//! (internal storage, SD cards) are reachable. Errors carry `errno N` so the laptop's
//! folder reports the same error the phone hit (not empty, exists, no space…).

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use russh::keys::PrivateKey;
use russh::keys::ssh_key::private::Ed25519Keypair;
use russh::server::{Auth, ChannelOpenHandle, Msg, Server as _, Session};
use russh::{Channel, ChannelId};
use russh_sftp::extensions::{STATVFS, Statvfs, StatvfsExtension};
use russh_sftp::protocol::{
    Attrs, Data, ExtendedReply, File, FileAttributes, Handle, Name, OpenFlags, Packet, Status, StatusCode, Version,
};
use russh_sftp::server::StatusReply;

pub struct Server {
    rt: Option<tokio::runtime::Runtime>,
    port: u16,
    user: String,
    password: String,
    fingerprint: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl Server {
    /// Serve `roots` on a free port in 1739–1764, with the host key made from `seed`.
    pub fn start(roots: Vec<String>, seed: [u8; 32]) -> Result<Server, String> {
        let roots: Vec<PathBuf> = roots.into_iter().filter_map(|p| std::fs::canonicalize(p).ok()).filter(|p| p.is_dir()).collect();
        if roots.is_empty() {
            return Err("Nothing to share: allow All files access for EchoConnect.".into());
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("sftp")
            .build()
            .map_err(|e| e.to_string())?;
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&seed));
        let user = "echoconnect".to_string();
        let password = random_hex(16);
        let listener = rt.block_on(async {
            for p in 1739..=1764u16 {
                if let Ok(l) = tokio::net::TcpListener::bind(("0.0.0.0", p)).await {
                    return Some(l);
                }
            }
            None
        });
        let listener = listener.ok_or("No free port for file browsing (1739–1764).")?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let fingerprint = key.public_key().fingerprint(russh::keys::HashAlg::Sha256).to_string();
        let config = Arc::new(russh::server::Config {
            auth_rejection_time: Duration::from_secs(2),
            auth_rejection_time_initial: Some(Duration::from_secs(0)),
            inactivity_timeout: Some(Duration::from_secs(3600)),
            // Small status replies to writes otherwise wait on Nagle + delayed ACK (~40 ms each).
            nodelay: true,
            keys: vec![key],
            ..Default::default()
        });
        let mut app = App { roots: Arc::new(roots), user: user.clone(), password: password.clone() };
        rt.spawn(async move {
            let _ = app.run_on_socket(config, &listener).await;
        });
        Ok(Server { rt: Some(rt), port, user, password, fingerprint })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn user(&self) -> String {
        self.user.clone()
    }
    pub fn fingerprint(&self) -> String {
        self.fingerprint.clone()
    }
    pub fn password(&self) -> String {
        self.password.clone()
    }
}

#[derive(Clone)]
struct App {
    roots: Arc<Vec<PathBuf>>,
    user: String,
    password: String,
}

impl russh::server::Server for App {
    type Handler = Conn;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Conn {
        Conn { app: self.clone(), channels: HashMap::new() }
    }
}

struct Conn {
    app: App,
    channels: HashMap<ChannelId, Channel<Msg>>,
}

impl russh::server::Handler for Conn {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        if user == self.app.user && password == self.app.password { Ok(Auth::Accept) } else { Ok(Auth::reject()) }
    }

    async fn channel_open_session(&mut self, channel: Channel<Msg>, reply: ChannelOpenHandle, _: &mut Session) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn channel_eof(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), Self::Error> {
        session.close(channel)?;
        Ok(())
    }

    async fn subsystem_request(&mut self, id: ChannelId, name: &str, session: &mut Session) -> Result<(), Self::Error> {
        match (name, self.channels.remove(&id)) {
            ("sftp", Some(channel)) => {
                session.channel_success(id)?;
                let roots = self.app.roots.clone();
                tokio::spawn(russh_sftp::server::run(channel.into_stream(), Sftp { roots, open: HashMap::new(), next: 1 }));
            }
            _ => session.channel_failure(id)?,
        }
        Ok(())
    }
}

enum Open {
    Dir(std::collections::VecDeque<File>, bool),
    File(std::fs::File),
}

struct Sftp {
    roots: Arc<Vec<PathBuf>>,
    open: HashMap<String, Open>,
    next: u64,
}

/// Storage calls block; let the runtime move other sessions off this worker meanwhile.
fn blocking<T>(f: impl FnOnce() -> T) -> T {
    tokio::task::block_in_place(f)
}

fn ok(id: u32) -> Status {
    Status { id, status_code: StatusCode::Ok, error_message: "Ok".into(), language_tag: "en-US".into() }
}

fn code(e: std::io::Error) -> StatusReply {
    let status = match e.kind() {
        std::io::ErrorKind::NotFound => StatusCode::NoSuchFile,
        std::io::ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
        _ => StatusCode::Failure,
    };
    match e.raw_os_error() {
        Some(n) => status.with_message(format!("errno {n}")),
        None => status.into(),
    }
}

/// `path` made absolute against the first root, `.` and `..` folded, without touching the disk.
fn normal(base: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    let full = if p.is_absolute() { p.to_path_buf() } else { base.join(p) };
    let mut out = PathBuf::from("/");
    for c in full.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(x) => out.push(x),
            _ => {}
        }
    }
    out
}

impl Sftp {
    /// The path, if it's inside a shared folder (the bare `/` lists them).
    fn inside(&self, path: &str) -> Result<PathBuf, StatusReply> {
        let p = normal(&self.roots[0], path);
        let mut ancestor = p.as_path();
        while !ancestor.exists() {
            // A dangling symlink must not become a writable escape hatch.
            if std::fs::symlink_metadata(ancestor).is_ok() {
                return Err(StatusCode::PermissionDenied.into());
            }
            ancestor = ancestor.parent().ok_or(StatusReply::from(StatusCode::PermissionDenied))?;
        }
        let resolved = std::fs::canonicalize(ancestor).map_err(code)?;
        let rest = p.strip_prefix(ancestor).map_err(|_| StatusReply::from(StatusCode::PermissionDenied))?;
        // `join("")` would add a trailing slash, and "file/" fails with ENOTDIR.
        let target = if rest.as_os_str().is_empty() { resolved } else { resolved.join(rest) };
        if self.roots.iter().any(|r| target.starts_with(r)) { Ok(target) } else { Err(StatusCode::PermissionDenied.into()) }
    }

    fn handle(&mut self, o: Open) -> String {
        let h = self.next.to_string();
        self.next += 1;
        self.open.insert(h.clone(), o);
        h
    }
}

fn attrs(m: &std::fs::Metadata) -> FileAttributes {
    FileAttributes::from(m)
}

impl russh_sftp::server::Handler for Sftp {
    type Error = StatusReply;

    fn unimplemented(&self) -> StatusReply {
        StatusCode::OpUnsupported.into()
    }

    async fn init(&mut self, _: u32, _: HashMap<String, String>) -> Result<Version, StatusReply> {
        let mut v = Version::new();
        v.extensions.insert(STATVFS.to_string(), "2".to_string());
        Ok(v)
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, StatusReply> {
        let p = normal(&self.roots[0], if path.is_empty() { "." } else { &path });
        Ok(Name { id, files: vec![File::dummy(p.to_string_lossy())] })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, StatusReply> {
        let p = self.inside(&path)?;
        let m = std::fs::metadata(&p).map_err(code)?;
        Ok(Attrs { id, attrs: attrs(&m) })
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, StatusReply> {
        let p = self.inside(&path)?;
        let m = std::fs::symlink_metadata(&p).map_err(code)?;
        Ok(Attrs { id, attrs: attrs(&m) })
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, StatusReply> {
        match self.open.get(&handle) {
            Some(Open::File(f)) => Ok(Attrs { id, attrs: attrs(&f.metadata().map_err(code)?) }),
            _ => Err(StatusCode::Failure.into()),
        }
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, StatusReply> {
        let p = self.inside(&path)?;
        let files = blocking(|| -> Result<Vec<File>, StatusReply> {
            let mut files = Vec::new();
            for e in std::fs::read_dir(&p).map_err(code)?.flatten() {
                if let Ok(m) = e.metadata() {
                    files.push(File::new(e.file_name().to_string_lossy(), attrs(&m)));
                }
            }
            Ok(files)
        })?;
        Ok(Handle { id, handle: self.handle(Open::Dir(files.into(), false)) })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, StatusReply> {
        match self.open.get_mut(&handle) {
            Some(Open::Dir(files, done)) if !*done => {
                // Up to 256 per reply; Eof once all went out.
                let n = files.len().min(256);
                let chunk: Vec<File> = files.drain(..n).collect();
                if files.is_empty() {
                    *done = true;
                }
                if chunk.is_empty() {
                    return Err(StatusCode::Eof.into());
                }
                Ok(Name { id, files: chunk })
            }
            Some(Open::Dir(..)) => Err(StatusCode::Eof.into()),
            _ => Err(StatusCode::Failure.into()),
        }
    }

    async fn open(&mut self, id: u32, filename: String, flags: OpenFlags, _: FileAttributes) -> Result<Handle, StatusReply> {
        let p = self.inside(&filename)?;
        let mut o = std::fs::OpenOptions::new();
        o.read(flags.contains(OpenFlags::READ))
            .write(flags.contains(OpenFlags::WRITE) || flags.contains(OpenFlags::APPEND))
            .append(flags.contains(OpenFlags::APPEND))
            .truncate(flags.contains(OpenFlags::TRUNCATE));
        if flags.contains(OpenFlags::EXCLUDE) {
            o.create_new(true);
        } else if flags.contains(OpenFlags::CREATE) {
            o.create(true);
        }
        let f = blocking(|| o.open(&p)).map_err(code)?;
        Ok(Handle { id, handle: self.handle(Open::File(f)) })
    }

    async fn read(&mut self, id: u32, handle: String, offset: u64, len: u32) -> Result<Data, StatusReply> {
        let Some(Open::File(f)) = self.open.get_mut(&handle) else { return Err(StatusCode::Failure.into()) };
        let mut data = vec![0u8; len.min(256 * 1024) as usize];
        let n = blocking(|| {
            f.seek(SeekFrom::Start(offset))?;
            f.read(&mut data)
        })
        .map_err(code)?;
        if n == 0 {
            return Err(StatusCode::Eof.into());
        }
        data.truncate(n);
        Ok(Data { id, data })
    }

    async fn write(&mut self, id: u32, handle: String, offset: u64, data: Vec<u8>) -> Result<Status, StatusReply> {
        let Some(Open::File(f)) = self.open.get_mut(&handle) else { return Err(StatusCode::Failure.into()) };
        blocking(|| {
            f.seek(SeekFrom::Start(offset))?;
            f.write_all(&data)
        })
        .map_err(code)?;
        Ok(ok(id))
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, StatusReply> {
        if let Some(Open::File(f)) = self.open.remove(&handle) {
            let _ = blocking(|| f.sync_all());
        }
        Ok(ok(id))
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, StatusReply> {
        std::fs::remove_file(self.inside(&filename)?).map_err(code)?;
        Ok(ok(id))
    }

    async fn mkdir(&mut self, id: u32, path: String, _: FileAttributes) -> Result<Status, StatusReply> {
        std::fs::create_dir(self.inside(&path)?).map_err(code)?;
        Ok(ok(id))
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, StatusReply> {
        std::fs::remove_dir(self.inside(&path)?).map_err(code)?;
        Ok(ok(id))
    }

    async fn rename(&mut self, id: u32, oldpath: String, newpath: String) -> Result<Status, StatusReply> {
        std::fs::rename(self.inside(&oldpath)?, self.inside(&newpath)?).map_err(code)?;
        Ok(ok(id))
    }

    async fn setstat(&mut self, id: u32, path: String, a: FileAttributes) -> Result<Status, StatusReply> {
        let p = self.inside(&path)?;
        if a.size.is_some() || a.mtime.is_some() {
            let f = blocking(|| std::fs::File::options().write(true).open(&p)).map_err(code)?;
            set(&f, &a).map_err(code)?;
        }
        Ok(ok(id))
    }

    async fn fsetstat(&mut self, id: u32, handle: String, a: FileAttributes) -> Result<Status, StatusReply> {
        if let Some(Open::File(f)) = self.open.get(&handle) {
            set(f, &a).map_err(code)?;
        }
        Ok(ok(id))
    }

    async fn extended(&mut self, id: u32, request: String, data: Vec<u8>) -> Result<Packet, StatusReply> {
        if request != STATVFS {
            return Err(StatusCode::OpUnsupported.into());
        }
        let ext: StatvfsExtension = russh_sftp::de::from_bytes(&mut data.into()).map_err(|_| StatusReply::from(StatusCode::BadMessage))?;
        let p = self.inside(&ext.path)?;
        let v = rustix::fs::statvfs(&p).map_err(|e| code(e.into()))?;
        let s = Statvfs {
            block_size: v.f_bsize,
            fragment_size: v.f_frsize,
            blocks: v.f_blocks,
            blocks_free: v.f_bfree,
            blocks_avail: v.f_bavail,
            inodes: v.f_files,
            inodes_free: v.f_ffree,
            inodes_avail: v.f_favail,
            fs_id: v.f_fsid,
            flags: v.f_flag.bits(),
            name_max: v.f_namemax,
        };
        let data = russh_sftp::ser::to_bytes(&s).map_err(|_| StatusReply::from(StatusCode::Failure))?.to_vec();
        Ok(Packet::ExtendedReply(ExtendedReply { id, data }))
    }
}

/// Apply a size (truncate or extend) and a modification time from `a`.
fn set(f: &std::fs::File, a: &FileAttributes) -> std::io::Result<()> {
    if let Some(size) = a.size {
        f.set_len(size)?;
    }
    if let Some(mtime) = a.mtime {
        f.set_modified(std::time::UNIX_EPOCH + Duration::from_secs(mtime as u64))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::normal;
    use std::path::{Path, PathBuf};

    #[test]
    fn paths_fold() {
        let base = Path::new("/storage/emulated/0");
        assert_eq!(normal(base, "."), PathBuf::from("/storage/emulated/0"));
        assert_eq!(normal(base, "DCIM/../Download"), PathBuf::from("/storage/emulated/0/Download"));
        assert_eq!(normal(base, "/storage/emulated/0/../../../etc"), PathBuf::from("/etc"));
    }
    #[test]
    fn shared_roots_reject_symlink_escapes() {
        use super::*;
        let root = std::env::temp_dir().join(format!("echo-sftp-test-{}", std::process::id()));
        let shared = root.join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let outside = root.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, shared.join("escape")).unwrap();
        let shared = std::fs::canonicalize(&shared).unwrap();
        let server = Sftp { roots: Arc::new(vec![shared.clone()]), open: HashMap::new(), next: 0 };
        assert!(server.inside("escape/private.txt").is_err());
        assert!(server.inside("../outside/private.txt").is_err());
        assert_eq!(server.inside("new.txt").unwrap(), shared.join("new.txt"));
        std::fs::write(shared.join("note.txt"), "hi").unwrap();
        let note = server.inside("note.txt").unwrap();
        assert_eq!(note, std::fs::canonicalize(shared.join("note.txt")).unwrap());
        assert!(std::fs::metadata(&note).is_ok(), "existing files resolve without a trailing slash");
        std::fs::remove_dir_all(root).unwrap();
    }
}

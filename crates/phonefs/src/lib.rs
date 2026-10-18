//! The phone's files as a folder on this computer.
//!
//! EchoConnect serves the phone's storage over SFTP; this is the other half, all in
//! process: an SSH client that only accepts the host key the phone named over the paired
//! TLS link, and a FUSE filesystem on top of it. Nothing goes through `ssh`,
//! `~/.ssh/known_hosts` or GVfs, so a phone that restarts its server (new port, new
//! address) never trips a stale host-key entry.
//!
//! Requests run as tasks on a small Tokio runtime and answer FUSE when they finish, so
//! reads, thumbnails and listings overlap instead of queueing behind each other.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::future::Future;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fuser::{
    BackgroundSession, Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags, Generation, INodeNo, InitFlags, KernelConfig,
    MountOption, OpenFlags, RenameFlags, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyDirectoryPlus, ReplyEmpty, ReplyEntry,
    ReplyOpen, ReplyStatfs, ReplyWrite, Request, TimeOrNow, WriteFlags,
};
use russh::keys::{HashAlg, PublicKeyOrCertificate};
use russh_sftp::client::RawSftpSession;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::{FileAttributes, OpenFlags as Open, StatusCode};

/// How long the kernel may keep names and attributes before asking again.
const TTL: Duration = Duration::from_secs(1);
/// Attributes learned from a listing answer lookups this long without a round trip. Long
/// enough to outlast a big folder's listing, so the file manager's stat of each entry
/// that follows doesn't go back to the phone.
const FRESH: Duration = Duration::from_secs(10);
/// A finished listing is reused while its folder's modified time is unchanged, for at most
/// this long (an edit inside a file doesn't touch the folder's time).
const CACHE_FOR: Duration = Duration::from_secs(600);
/// Most entries handed over per readdir call (the kernel asks again for the rest).
const READDIR_MAX: usize = 512;
/// Largest read the phone's server answers in one reply.
const READ_PIECE: u32 = 256 * 1024;
/// Writes go out in pieces this big, all in flight at once.
const WRITE_PIECE: usize = 128 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Where and how to reach the phone's file server, from its reply to a files request.
#[derive(Clone, Debug)]
pub struct Remote {
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    /// `SHA256:…` of the server's host key, as the phone sent it over the paired link.
    pub fingerprint: String,
    /// The folder on the phone shown as the mount's root.
    pub base: String,
}

/// A mounted phone. Dropping it (or [`PhoneFs::unmount`]) takes the folder away.
pub struct PhoneFs {
    root: PathBuf,
    session: Mutex<Option<BackgroundSession>>,
    sftp: Arc<RawSftpSession>,
    _ssh: russh::client::Handle<Pinned>,
    rt: Option<tokio::runtime::Runtime>,
}

impl std::fmt::Debug for PhoneFs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhoneFs").field("root", &self.root).finish()
    }
}

impl PhoneFs {
    /// The folder the phone's files appear in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Take the folder away. Safe to call more than once; blocks until FUSE lets go.
    pub fn unmount(&self) {
        let session = self.session.lock().unwrap().take();
        if let Some(s) = session {
            let _ = s.umount_and_join();
        }
        let _ = self.sftp.close_session();
    }
}

impl Drop for PhoneFs {
    fn drop(&mut self) {
        self.unmount();
        // Tasks still waiting on the phone end with the runtime.
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

/// Connect to the phone's file server and show it at `at` (created if needed).
pub fn mount(remote: &Remote, at: &Path) -> Result<PhoneFs, String> {
    if remote.fingerprint.is_empty() {
        return Err("The phone didn't say which key its file server uses. Update EchoConnect on the phone.".into());
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("phonefs")
        .build()
        .map_err(|e| e.to_string())?;
    let (ssh, sftp) = rt.block_on(async {
        tokio::time::timeout(CONNECT_TIMEOUT, connect(remote)).await.map_err(|_| "The phone didn't answer in time.".to_string())?
    })?;
    let sftp = Arc::new(sftp);
    let base = remote.base.trim_end_matches('/').to_string();
    let top = rt.block_on(sftp.stat(if base.is_empty() { "/".to_string() } else { base.clone() }));
    match top {
        Ok(a) if a.attrs.is_dir() => {}
        Ok(_) => return Err("The phone's shared folder isn't a folder.".into()),
        Err(e) => return Err(format!("The phone's shared folder can't be opened ({e}).")),
    }
    prepare(at)?;
    let fs = Fs {
        sftp: sftp.clone(),
        rt: rt.handle().clone(),
        base,
        nodes: Arc::new(Mutex::new(Nodes::new())),
        open: Arc::new(Mutex::new(HashMap::new())),
        dirs: Arc::new(Mutex::new(HashMap::new())),
        next_fh: AtomicU64::new(1),
        cache: Arc::new(Mutex::new(HashMap::new())),
        uid: unsafe { libc::getuid() },
        gid: unsafe { libc::getgid() },
    };
    let mut config = fuser::Config::default();
    config.mount_options = vec![
        MountOption::FSName("echoconnect".into()),
        MountOption::Subtype("echoconnect".into()),
        MountOption::NoDev,
        MountOption::NoSuid,
        MountOption::NoAtime,
    ];
    let session = fuser::spawn_mount(fs, at, &config).map_err(|e| format!("Couldn't show the phone's files as a folder: {e}"))?;
    Ok(PhoneFs { root: at.to_path_buf(), session: Mutex::new(Some(session)), sftp, _ssh: ssh, rt: Some(rt) })
}

/// Make `at` an empty place to mount on, clearing a mount a crashed run left behind.
fn prepare(at: &Path) -> Result<(), String> {
    if let Some(parent) = at.parent() {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(parent).map_err(|e| e.to_string())?;
    }
    let stale = match std::fs::metadata(at) {
        // "Transport endpoint is not connected": a FUSE mount whose process is gone.
        Err(e) if e.raw_os_error() == Some(libc::ENOTCONN) => true,
        Err(_) => false,
        Ok(m) => at.parent().and_then(|p| std::fs::metadata(p).ok()).is_some_and(|p| p.dev() != m.dev()),
    };
    if stale {
        let _ = std::process::Command::new("fusermount3").arg("-u").arg("-z").arg(at).status();
    }
    match std::fs::create_dir(at) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(format!("Couldn't make the phone's folder: {e}")),
    }
}

struct Pinned(String);

impl russh::client::Handler for Pinned {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        Ok(match key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.fingerprint(HashAlg::Sha256).to_string() == self.0,
            PublicKeyOrCertificate::Certificate(_) => false,
        })
    }
}

async fn connect(r: &Remote) -> Result<(russh::client::Handle<Pinned>, RawSftpSession), String> {
    let config = Arc::new(russh::client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(15)),
        nodelay: true,
        ..Default::default()
    });
    let mut ssh = russh::client::connect(config, (r.ip.as_str(), r.port), Pinned(r.fingerprint.clone())).await.map_err(|e| match e {
        russh::Error::UnknownKey => "The phone's file server didn't prove it's your phone, so EchoFiles didn't connect.".to_string(),
        e => format!("Couldn't reach the phone's file server ({e})."),
    })?;
    let auth = ssh.authenticate_password(&r.user, &r.password).await.map_err(|e| format!("The file connection failed ({e})."))?;
    if !auth.success() {
        return Err("The phone turned the file connection down. Try again.".into());
    }
    let channel = ssh.channel_open_session().await.map_err(|e| format!("The file connection failed ({e})."))?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| format!("The file connection failed ({e})."))?;
    // A full 256 KiB read reply plus its header is over the client's default packet limit.
    let cfg = russh_sftp::client::Config { max_packet_len: 2 * READ_PIECE, ..Default::default() };
    let sftp = RawSftpSession::new_with_config(channel.into_stream(), cfg);
    sftp.init().await.map_err(|e| format!("The file connection failed ({e})."))?;
    sftp.set_timeout(30);
    Ok((ssh, sftp))
}

// ------------------------------------------------------------------------------ inodes

struct Node {
    /// Path below the mount root, `""` for the root itself.
    path: String,
    /// Times the kernel was handed this inode and hasn't forgotten it.
    lookups: u64,
}

struct Nodes {
    by_ino: HashMap<u64, Node>,
    by_path: HashMap<String, u64>,
    /// Attributes by path, from recent listings and stats.
    attrs: HashMap<String, (FileAttributes, Instant)>,
    next: u64,
}

impl Nodes {
    fn new() -> Nodes {
        let mut n = Nodes { by_ino: HashMap::new(), by_path: HashMap::new(), attrs: HashMap::new(), next: 2 };
        n.by_ino.insert(INodeNo::ROOT.0, Node { path: String::new(), lookups: 1 });
        n.by_path.insert(String::new(), INodeNo::ROOT.0);
        n
    }

    fn path(&self, ino: INodeNo) -> Option<String> {
        self.by_ino.get(&ino.0).map(|n| n.path.clone())
    }

    /// The inode for `path`, made if it's new; doesn't count as handing it out.
    fn ino(&mut self, path: &str) -> u64 {
        if let Some(&i) = self.by_path.get(path) {
            return i;
        }
        let i = self.next;
        self.next += 1;
        self.by_ino.insert(i, Node { path: path.to_string(), lookups: 0 });
        self.by_path.insert(path.to_string(), i);
        i
    }

    /// The inode for `path`, counted as handed to the kernel.
    fn entered(&mut self, path: &str) -> u64 {
        let i = self.ino(path);
        if let Some(n) = self.by_ino.get_mut(&i) {
            n.lookups += 1;
        }
        i
    }

    fn forget(&mut self, ino: u64, count: u64) {
        if ino == INodeNo::ROOT.0 {
            return;
        }
        let Some(n) = self.by_ino.get_mut(&ino) else { return };
        n.lookups = n.lookups.saturating_sub(count);
        if n.lookups == 0 {
            let path = std::mem::take(&mut n.path);
            self.by_ino.remove(&ino);
            if self.by_path.get(&path) == Some(&ino) {
                self.by_path.remove(&path);
            }
        }
    }

    fn fresh(&self, path: &str) -> Option<FileAttributes> {
        self.attrs.get(path).filter(|(_, at)| at.elapsed() < FRESH).map(|(a, _)| a.clone())
    }

    fn learn(&mut self, path: &str, a: &FileAttributes) {
        if self.attrs.len() > 50_000 {
            self.attrs.retain(|_, (_, at)| at.elapsed() < FRESH);
        }
        self.attrs.insert(path.to_string(), (a.clone(), Instant::now()));
    }

    /// `path` is gone: the next file made there gets a new inode.
    fn removed(&mut self, path: &str) {
        self.by_path.remove(path);
        self.attrs.remove(path);
    }

    /// `from` (and everything under it) now lives at `to`.
    fn moved(&mut self, from: &str, to: &str) {
        self.removed(to);
        self.attrs.clear();
        let prefix = format!("{from}/");
        let moved: Vec<(String, u64)> =
            self.by_path.iter().filter(|(p, _)| *p == from || p.starts_with(&prefix)).map(|(p, &i)| (p.clone(), i)).collect();
        for (old, i) in moved {
            let new = format!("{to}{}", &old[from.len()..]);
            self.by_path.remove(&old);
            self.by_path.insert(new.clone(), i);
            if let Some(n) = self.by_ino.get_mut(&i) {
                n.path = new;
            }
        }
    }
}

fn join(parent: &str, name: &OsStr) -> Option<String> {
    let name = name.to_str()?;
    if name.is_empty() || name.contains('/') || name == "." || name == ".." {
        return None;
    }
    Some(if parent.is_empty() { name.to_string() } else { format!("{parent}/{name}") })
}

fn parent_of(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

// ------------------------------------------------------------------------------ filesystem

type Entries = Vec<(String, FileAttributes)>;

/// An open directory's listing, filled as the phone's batches arrive so the first entries
/// can be shown before the last are statted.
struct Dir {
    path: String,
    state: Mutex<DirState>,
    changed: tokio::sync::Notify,
}

#[derive(Default)]
struct DirState {
    entries: Entries,
    done: bool,
    error: Option<Errno>,
}

impl Dir {
    fn new(path: String, entries: Entries, done: bool) -> Arc<Dir> {
        Arc::new(Dir { path, state: Mutex::new(DirState { entries, done, error: None }), changed: tokio::sync::Notify::new() })
    }

    fn push(&self, batch: Entries) {
        self.state.lock().unwrap().entries.extend(batch);
        self.changed.notify_waiters();
    }

    fn finish(&self, error: Option<Errno>) {
        {
            let mut s = self.state.lock().unwrap();
            s.done = true;
            s.error = error;
        }
        self.changed.notify_waiters();
    }

    /// Entries from `index` on, once there are some or the listing has ended.
    fn from(&self, index: usize) -> Option<(Entries, Option<Errno>)> {
        let s = self.state.lock().unwrap();
        if s.entries.len() > index || s.done {
            let start = index.min(s.entries.len());
            let end = (start + READDIR_MAX).min(s.entries.len());
            Some((s.entries[start..end].to_vec(), s.error))
        } else {
            None
        }
    }
}

/// A finished listing of one folder, kept for the next visit.
struct Cached {
    mtime: u32,
    entries: Arc<Entries>,
    at: Instant,
}

struct Fs {
    sftp: Arc<RawSftpSession>,
    rt: tokio::runtime::Handle,
    /// The phone-side folder the root shows, without a trailing slash (`""` for `/`).
    base: String,
    nodes: Arc<Mutex<Nodes>>,
    /// Open file handle → the server's handle.
    open: Arc<Mutex<HashMap<u64, String>>>,
    /// Open directory → its listing, filling in as the phone answers.
    dirs: Arc<Mutex<HashMap<u64, Arc<Dir>>>>,
    next_fh: AtomicU64,
    cache: Arc<Mutex<HashMap<String, Cached>>>,
    uid: u32,
    gid: u32,
}

/// What a request needs once it's off the FUSE thread.
#[derive(Clone)]
struct Ctx {
    sftp: Arc<RawSftpSession>,
    base: String,
    nodes: Arc<Mutex<Nodes>>,
    /// Finished listings by folder path.
    cache: Arc<Mutex<HashMap<String, Cached>>>,
    uid: u32,
    gid: u32,
}

impl Ctx {
    /// Something in `path`'s folder changed: its kept listing is out of date.
    fn changed(&self, path: &str) {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(parent_of(path));
        cache.remove(path);
    }

    fn remote(&self, path: &str) -> String {
        if path.is_empty() { if self.base.is_empty() { "/".into() } else { self.base.clone() } } else { format!("{}/{path}", self.base) }
    }

    fn attr(&self, ino: u64, a: &FileAttributes) -> FileAttr {
        let mode = a.permissions.unwrap_or(0o100_644);
        let kind = match mode & libc::S_IFMT {
            libc::S_IFDIR => FileType::Directory,
            libc::S_IFLNK => FileType::Symlink,
            _ => FileType::RegularFile,
        };
        let size = a.size.unwrap_or(0);
        let mtime = UNIX_EPOCH + Duration::from_secs(a.mtime.unwrap_or(0) as u64);
        let atime = a.atime.map_or(mtime, |t| UNIX_EPOCH + Duration::from_secs(t as u64));
        FileAttr {
            ino: INodeNo(ino),
            size,
            blocks: size.div_ceil(512),
            atime,
            mtime,
            ctime: mtime,
            crtime: mtime,
            kind,
            perm: (mode & 0o7777) as u16,
            nlink: if kind == FileType::Directory { 2 } else { 1 },
            // The phone's owners mean nothing here; the files are the user's.
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: WRITE_PIECE as u32,
            flags: 0,
        }
    }

    async fn stat(&self, path: &str) -> Result<FileAttributes, Errno> {
        let a = self.sftp.stat(self.remote(path)).await.map_err(|e| errno(&e))?.attrs;
        self.nodes.lock().unwrap().learn(path, &a);
        Ok(a)
    }

    /// Stat `path` and hand its inode to the kernel.
    async fn entry(&self, path: &str) -> Result<FileAttr, Errno> {
        let a = self.stat(path).await?;
        let ino = self.nodes.lock().unwrap().entered(path);
        Ok(self.attr(ino, &a))
    }
}

/// The phone's server puts `errno N` in its error text; older servers only give a code.
fn errno(e: &SftpError) -> Errno {
    match e {
        SftpError::Status(s) => {
            if let Some(n) = s.error_message.strip_prefix("errno ").and_then(|n| n.trim().parse::<i32>().ok()) {
                return Errno::from_i32(n);
            }
            match s.status_code {
                StatusCode::NoSuchFile => Errno::ENOENT,
                StatusCode::PermissionDenied => Errno::EACCES,
                StatusCode::OpUnsupported => Errno::ENOSYS,
                _ => Errno::EIO,
            }
        }
        SftpError::Timeout => Errno::from_i32(libc::ETIMEDOUT),
        SftpError::IO(_) | SftpError::UnexpectedBehavior(_) => Errno::from_i32(libc::ENOTCONN),
        _ => Errno::EIO,
    }
}

fn is_eof(e: &SftpError) -> bool {
    matches!(e, SftpError::Status(s) if s.status_code == StatusCode::Eof)
}

fn open_flags(flags: i32) -> Open {
    let mut o = match flags & libc::O_ACCMODE {
        libc::O_WRONLY => Open::WRITE,
        libc::O_RDWR => Open::READ | Open::WRITE,
        _ => Open::READ,
    };
    if flags & libc::O_APPEND != 0 {
        o |= Open::APPEND;
    }
    if flags & libc::O_TRUNC != 0 {
        o |= Open::TRUNCATE;
    }
    if flags & libc::O_CREAT != 0 {
        o |= Open::CREATE;
    }
    if flags & libc::O_EXCL != 0 {
        o |= Open::EXCLUDE;
    }
    o
}

fn secs(t: TimeOrNow) -> u32 {
    let t = match t {
        TimeOrNow::SpecificTime(t) => t,
        TimeOrNow::Now => SystemTime::now(),
    };
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32)
}

/// `len` bytes from `offset`, fewer only at the end of the file.
async fn read_exact(sftp: &RawSftpSession, handle: &str, offset: u64, len: u32) -> Result<Vec<u8>, Errno> {
    let mut out = Vec::with_capacity(len as usize);
    while (out.len() as u32) < len {
        let want = (len - out.len() as u32).min(READ_PIECE);
        match sftp.read(handle, offset + out.len() as u64, want).await {
            Ok(d) if d.data.is_empty() => break,
            Ok(d) => out.extend_from_slice(&d.data),
            Err(e) if is_eof(&e) => break,
            Err(e) => return Err(errno(&e)),
        }
    }
    Ok(out)
}

impl Fs {
    fn ctx(&self) -> Ctx {
        Ctx {
            sftp: self.sftp.clone(),
            base: self.base.clone(),
            nodes: self.nodes.clone(),
            cache: self.cache.clone(),
            uid: self.uid,
            gid: self.gid,
        }
    }

    fn spawn(&self, f: impl Future<Output = ()> + Send + 'static) {
        self.rt.spawn(f);
    }

    fn path(&self, ino: INodeNo) -> Option<String> {
        self.nodes.lock().unwrap().path(ino)
    }

    fn child(&self, parent: INodeNo, name: &OsStr) -> Result<String, Errno> {
        let p = self.path(parent).ok_or(Errno::ENOENT)?;
        join(&p, name).ok_or(Errno::EINVAL)
    }

    fn handle(&self, fh: FileHandle) -> Option<String> {
        self.open.lock().unwrap().get(&fh.0).cloned()
    }

    fn new_fh(&self) -> u64 {
        self.next_fh.fetch_add(1, Ordering::Relaxed)
    }

    fn dir(&self, fh: FileHandle) -> Option<Arc<Dir>> {
        self.dirs.lock().unwrap().get(&fh.0).cloned()
    }

    /// Run `then` with `dir`'s entries from `index` on as soon as there are some (or the
    /// listing has ended): at once if they're here, else when the next batch lands.
    fn when_ready(&self, dir: Arc<Dir>, index: usize, then: impl FnOnce(&Dir, Entries, Option<Errno>) + Send + 'static) {
        if let Some((entries, error)) = dir.from(index) {
            return then(&dir, entries, error);
        }
        self.spawn(async move {
            loop {
                let mut wait = std::pin::pin!(dir.changed.notified());
                wait.as_mut().enable();
                if let Some((entries, error)) = dir.from(index) {
                    return then(&dir, entries, error);
                }
                wait.await;
            }
        });
    }
}

impl Filesystem for Fs {
    fn init(&mut self, _req: &Request, config: &mut KernelConfig) -> std::io::Result<()> {
        for flag in [
            InitFlags::FUSE_DO_READDIRPLUS,
            InitFlags::FUSE_READDIRPLUS_AUTO,
            InitFlags::FUSE_ATOMIC_O_TRUNC,
            InitFlags::FUSE_AUTO_INVAL_DATA,
            InitFlags::FUSE_PARALLEL_DIROPS,
        ] {
            let _ = config.add_capabilities(flag);
        }
        let _ = config.set_max_write(1 << 20);
        let _ = config.set_max_readahead(1 << 20);
        let _ = config.set_max_background(64);
        Ok(())
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let path = match self.child(parent, name) {
            Ok(p) => p,
            Err(e) => return reply.error(e),
        };
        let ctx = self.ctx();
        let cached = {
            let mut n = ctx.nodes.lock().unwrap();
            n.fresh(&path).map(|a| (n.entered(&path), a))
        };
        if let Some((ino, a)) = cached {
            return reply.entry(&TTL, &ctx.attr(ino, &a), Generation(0));
        }
        self.spawn(async move {
            match ctx.entry(&path).await {
                Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
                Err(e) => reply.error(e),
            }
        });
    }

    fn forget(&self, _req: &Request, ino: INodeNo, nlookup: u64) {
        self.nodes.lock().unwrap().forget(ino.0, nlookup);
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        let Some(path) = self.path(ino) else { return reply.error(Errno::ENOENT) };
        let ctx = self.ctx();
        if let Some(a) = ctx.nodes.lock().unwrap().fresh(&path) {
            return reply.attr(&TTL, &ctx.attr(ino.0, &a));
        }
        self.spawn(async move {
            match ctx.stat(&path).await {
                Ok(a) => reply.attr(&TTL, &ctx.attr(ino.0, &a)),
                Err(e) => reply.error(e),
            }
        });
    }

    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        fh: Option<FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<fuser::BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        let Some(path) = self.path(ino) else { return reply.error(Errno::ENOENT) };
        let handle = fh.and_then(|f| self.handle(f));
        let ctx = self.ctx();
        // Owners and modes don't carry over to phone storage; sizes and times do.
        let mut a = FileAttributes::empty();
        a.size = size;
        if let Some(m) = mtime {
            a.mtime = Some(secs(m));
            a.atime = Some(atime.map_or(secs(m), secs));
        }
        self.spawn(async move {
            if a.size.is_some() || a.mtime.is_some() {
                let r = match &handle {
                    Some(h) => ctx.sftp.fsetstat(h.clone(), a).await,
                    None => ctx.sftp.setstat(ctx.remote(&path), a).await,
                };
                if let Err(e) = r {
                    return reply.error(errno(&e));
                }
                ctx.changed(&path);
            }
            match ctx.stat(&path).await {
                Ok(a) => reply.attr(&TTL, &ctx.attr(ino.0, &a)),
                Err(e) => reply.error(e),
            }
        });
    }

    fn mkdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, _mode: u32, _umask: u32, reply: ReplyEntry) {
        let path = match self.child(parent, name) {
            Ok(p) => p,
            Err(e) => return reply.error(e),
        };
        let ctx = self.ctx();
        self.spawn(async move {
            if let Err(e) = ctx.sftp.mkdir(ctx.remote(&path), FileAttributes::empty()).await {
                return reply.error(errno(&e));
            }
            ctx.changed(&path);
            match ctx.entry(&path).await {
                Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
                Err(e) => reply.error(e),
            }
        });
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let path = match self.child(parent, name) {
            Ok(p) => p,
            Err(e) => return reply.error(e),
        };
        let ctx = self.ctx();
        self.spawn(async move {
            match ctx.sftp.remove(ctx.remote(&path)).await {
                Ok(_) => {
                    ctx.nodes.lock().unwrap().removed(&path);
                    ctx.changed(&path);
                    reply.ok()
                }
                Err(e) => reply.error(errno(&e)),
            }
        });
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let path = match self.child(parent, name) {
            Ok(p) => p,
            Err(e) => return reply.error(e),
        };
        let ctx = self.ctx();
        self.spawn(async move {
            match ctx.sftp.rmdir(ctx.remote(&path)).await {
                Ok(_) => {
                    ctx.nodes.lock().unwrap().removed(&path);
                    ctx.changed(&path);
                    reply.ok()
                }
                Err(e) => reply.error(errno(&e)),
            }
        });
    }

    fn rename(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        newparent: INodeNo,
        newname: &OsStr,
        flags: RenameFlags,
        reply: ReplyEmpty,
    ) {
        if flags.intersects(RenameFlags::RENAME_EXCHANGE | RenameFlags::RENAME_WHITEOUT) {
            return reply.error(Errno::EINVAL);
        }
        let (from, to) = match (self.child(parent, name), self.child(newparent, newname)) {
            (Ok(a), Ok(b)) => (a, b),
            (Err(e), _) | (_, Err(e)) => return reply.error(e),
        };
        let ctx = self.ctx();
        self.spawn(async move {
            if flags.contains(RenameFlags::RENAME_NOREPLACE) && ctx.sftp.stat(ctx.remote(&to)).await.is_ok() {
                return reply.error(Errno::EEXIST);
            }
            match ctx.sftp.rename(ctx.remote(&from), ctx.remote(&to)).await {
                Ok(_) => {
                    ctx.nodes.lock().unwrap().moved(&from, &to);
                    ctx.changed(&from);
                    ctx.changed(&to);
                    reply.ok()
                }
                Err(e) => reply.error(errno(&e)),
            }
        });
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        let Some(path) = self.path(ino) else { return reply.error(Errno::ENOENT) };
        let ctx = self.ctx();
        let fh = self.new_fh();
        let open = self.open.clone();
        self.spawn(async move {
            match ctx.sftp.open(ctx.remote(&path), open_flags(flags.0), FileAttributes::empty()).await {
                Ok(h) => {
                    if flags.0 & libc::O_TRUNC != 0 {
                        ctx.nodes.lock().unwrap().attrs.remove(&path);
                    }
                    open.lock().unwrap().insert(fh, h.handle);
                    reply.opened(FileHandle(fh), FopenFlags::empty())
                }
                Err(e) => reply.error(errno(&e)),
            }
        });
    }

    fn create(&self, _req: &Request, parent: INodeNo, name: &OsStr, _mode: u32, _umask: u32, flags: i32, reply: ReplyCreate) {
        let path = match self.child(parent, name) {
            Ok(p) => p,
            Err(e) => return reply.error(e),
        };
        let ctx = self.ctx();
        let fh = self.new_fh();
        let open = self.open.clone();
        self.spawn(async move {
            let h = match ctx.sftp.open(ctx.remote(&path), open_flags(flags) | Open::CREATE, FileAttributes::empty()).await {
                Ok(h) => h.handle,
                Err(e) => return reply.error(errno(&e)),
            };
            open.lock().unwrap().insert(fh, h);
            ctx.changed(&path);
            match ctx.entry(&path).await {
                Ok(attr) => reply.created(&TTL, &attr, Generation(0), FileHandle(fh), FopenFlags::empty()),
                Err(e) => reply.error(e),
            }
        });
    }

    fn read(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyData,
    ) {
        let Some(handle) = self.handle(fh) else { return reply.error(Errno::EBADF) };
        let sftp = self.sftp.clone();
        self.spawn(async move {
            // Big reads go out as several requests at once; Wi-Fi latency, not
            // bandwidth, is what a phone read waits on.
            let mut pieces = tokio::task::JoinSet::new();
            let mut at = 0u32;
            while at < size {
                let len = (size - at).min(READ_PIECE);
                let (s, h) = (sftp.clone(), handle.clone());
                let off = offset + at as u64;
                pieces.spawn(async move { (off, len, read_exact(&s, &h, off, len).await) });
                at += len;
            }
            let mut got = Vec::new();
            while let Some(done) = pieces.join_next().await {
                match done {
                    Ok((off, len, Ok(data))) => got.push((off, len, data)),
                    Ok((_, _, Err(e))) => return reply.error(e),
                    Err(_) => return reply.error(Errno::EIO),
                }
            }
            got.sort_by_key(|(off, ..)| *off);
            let mut out = Vec::with_capacity(size as usize);
            for (_, len, data) in got {
                let short = (data.len() as u32) < len;
                out.extend_from_slice(&data);
                if short {
                    break;
                }
            }
            reply.data(&out);
        });
    }

    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: WriteFlags,
        _flags: OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyWrite,
    ) {
        let Some(handle) = self.handle(fh) else { return reply.error(Errno::EBADF) };
        let path = self.path(ino);
        let ctx = self.ctx();
        let data = data.to_vec();
        self.spawn(async move {
            let mut pieces = tokio::task::JoinSet::new();
            for (i, chunk) in data.chunks(WRITE_PIECE).enumerate() {
                let (s, h, chunk) = (ctx.sftp.clone(), handle.clone(), chunk.to_vec());
                let off = offset + (i * WRITE_PIECE) as u64;
                pieces.spawn(async move { s.write(h, off, chunk).await });
            }
            while let Some(done) = pieces.join_next().await {
                match done {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => return reply.error(errno(&e)),
                    Err(_) => return reply.error(Errno::EIO),
                }
            }
            if let Some(p) = path {
                ctx.nodes.lock().unwrap().attrs.remove(&p);
                ctx.changed(&p);
            }
            reply.written(data.len() as u32);
        });
    }

    fn flush(&self, _req: &Request, _ino: INodeNo, _fh: FileHandle, _lock_owner: fuser::LockOwner, reply: ReplyEmpty) {
        reply.ok();
    }

    fn fsync(&self, _req: &Request, _ino: INodeNo, _fh: FileHandle, _datasync: bool, reply: ReplyEmpty) {
        // The phone syncs a file when it's closed.
        reply.ok();
    }

    fn release(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        _flags: OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        let Some(handle) = self.open.lock().unwrap().remove(&fh.0) else { return reply.ok() };
        let path = self.path(ino);
        let ctx = self.ctx();
        self.spawn(async move {
            let _ = ctx.sftp.close(handle).await;
            if let Some(p) = path {
                ctx.nodes.lock().unwrap().attrs.remove(&p);
            }
            reply.ok();
        });
    }

    fn opendir(&self, _req: &Request, ino: INodeNo, _flags: OpenFlags, reply: ReplyOpen) {
        let Some(path) = self.path(ino) else { return reply.error(Errno::ENOENT) };
        let ctx = self.ctx();
        let fh = self.new_fh();
        let dirs = self.dirs.clone();
        self.spawn(async move {
            let remote = ctx.remote(&path);
            // The folder's own time says whether the listing kept from last time still holds.
            let top = match ctx.sftp.stat(remote.clone()).await {
                Ok(a) => a.attrs,
                Err(e) => return reply.error(errno(&e)),
            };
            let mtime = top.mtime.unwrap_or(0);
            let kept =
                ctx.cache.lock().unwrap().get(&path).filter(|c| c.mtime == mtime && c.at.elapsed() < CACHE_FOR).map(|c| c.entries.clone());
            if let Some(entries) = kept {
                {
                    let mut n = ctx.nodes.lock().unwrap();
                    n.learn(&path, &top);
                    for (name, a) in entries.iter() {
                        if let Some(p) = join(&path, OsStr::new(name)) {
                            n.learn(&p, a);
                        }
                    }
                }
                dirs.lock().unwrap().insert(fh, Dir::new(path, (*entries).clone(), true));
                return reply.opened(FileHandle(fh), FopenFlags::empty());
            }
            let h = match ctx.sftp.opendir(remote).await {
                Ok(h) => h.handle,
                Err(e) => return reply.error(errno(&e)),
            };
            ctx.nodes.lock().unwrap().learn(&path, &top);
            let dir = Dir::new(path.clone(), Vec::new(), false);
            dirs.lock().unwrap().insert(fh, dir.clone());
            // Open now; readdir hands entries over as the phone's batches arrive.
            reply.opened(FileHandle(fh), FopenFlags::empty());
            let error = loop {
                match ctx.sftp.readdir(h.clone()).await {
                    Ok(names) => {
                        let batch: Entries = names
                            .files
                            .into_iter()
                            .filter(|f| f.filename != "." && f.filename != "..")
                            .map(|f| (f.filename, f.attrs))
                            .collect();
                        {
                            let mut n = ctx.nodes.lock().unwrap();
                            for (name, a) in &batch {
                                if let Some(p) = join(&path, OsStr::new(name)) {
                                    n.learn(&p, a);
                                }
                            }
                        }
                        dir.push(batch);
                    }
                    Err(e) if is_eof(&e) => break None,
                    Err(e) => break Some(errno(&e)),
                }
            };
            let _ = ctx.sftp.close(h).await;
            dir.finish(error);
            if error.is_none() {
                let entries = Arc::new(dir.state.lock().unwrap().entries.clone());
                ctx.cache.lock().unwrap().insert(path, Cached { mtime, entries, at: Instant::now() });
            }
        });
    }

    fn readdir(&self, _req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, mut reply: ReplyDirectory) {
        let Some(dir) = self.dir(fh) else { return reply.error(Errno::EBADF) };
        let ctx = self.ctx();
        // Offsets 0 and 1 are `.` and `..`; entry k is offset k + 2.
        let start = (offset as usize).saturating_sub(2);
        self.when_ready(dir, start, move |dir, entries, error| {
            if let (true, Some(e)) = (entries.is_empty() && offset >= 2, error) {
                return reply.error(e);
            }
            let mut n = ctx.nodes.lock().unwrap();
            let up = n.by_path.get(parent_of(&dir.path)).copied().unwrap_or(ino.0);
            let dots = [(ino.0, "."), (up, "..")];
            for (i, (d, name)) in dots.iter().enumerate().skip(offset as usize) {
                if reply.add(INodeNo(*d), i as u64 + 1, FileType::Directory, name) {
                    return reply.ok();
                }
            }
            for (k, (name, a)) in entries.iter().enumerate() {
                let Some(p) = join(&dir.path, OsStr::new(name)) else { continue };
                let attr = ctx.attr(n.ino(&p), a);
                if reply.add(attr.ino, (start + k + 3) as u64, attr.kind, name) {
                    break;
                }
            }
            reply.ok();
        });
    }

    fn readdirplus(&self, _req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, mut reply: ReplyDirectoryPlus) {
        let Some(dir) = self.dir(fh) else { return reply.error(Errno::EBADF) };
        let ctx = self.ctx();
        let start = (offset as usize).saturating_sub(2);
        self.when_ready(dir, start, move |dir, entries, error| {
            if let (true, Some(e)) = (entries.is_empty() && offset >= 2, error) {
                return reply.error(e);
            }
            let mut n = ctx.nodes.lock().unwrap();
            let folder = FileAttributes { permissions: Some(libc::S_IFDIR | 0o755), ..FileAttributes::empty() };
            let here = n.fresh(&dir.path).map_or_else(|| ctx.attr(ino.0, &folder), |a| ctx.attr(ino.0, &a));
            let up = n.by_path.get(parent_of(&dir.path)).copied().unwrap_or(ino.0);
            let dots = [(ino.0, ".", here), (up, "..", ctx.attr(up, &folder))];
            for (i, (d, name, attr)) in dots.iter().enumerate().skip(offset as usize) {
                if reply.add(INodeNo(*d), i as u64 + 1, name, &TTL, attr, Generation(0)) {
                    return reply.ok();
                }
            }
            for (k, (name, a)) in entries.iter().enumerate() {
                let Some(p) = join(&dir.path, OsStr::new(name)) else { continue };
                let attr = ctx.attr(n.ino(&p), a);
                if reply.add(attr.ino, (start + k + 3) as u64, name, &TTL, &attr, Generation(0)) {
                    break;
                }
                // Each entry handed out with attributes counts as a lookup.
                n.entered(&p);
            }
            reply.ok();
        });
    }

    fn releasedir(&self, _req: &Request, _ino: INodeNo, fh: FileHandle, _flags: OpenFlags, reply: ReplyEmpty) {
        self.dirs.lock().unwrap().remove(&fh.0);
        reply.ok();
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        let ctx = self.ctx();
        self.spawn(async move {
            match ctx.sftp.statvfs(ctx.remote("")).await {
                Ok(s) => reply.statfs(
                    s.blocks,
                    s.blocks_free,
                    s.blocks_avail,
                    s.inodes,
                    s.inodes_free,
                    s.block_size as u32,
                    s.name_max as u32,
                    s.fragment_size as u32,
                ),
                // An older phone app: no space figures, but the folder still works.
                Err(_) => reply.statfs(0, 0, 0, 0, 0, 4096, 255, 4096),
            }
        });
    }

    fn access(&self, _req: &Request, _ino: INodeNo, _mask: fuser::AccessFlags, reply: ReplyEmpty) {
        // The phone decides what may be read or written, when it's tried.
        reply.ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_join_without_escaping() {
        assert_eq!(join("", OsStr::new("DCIM")).as_deref(), Some("DCIM"));
        assert_eq!(join("DCIM", OsStr::new("a.jpg")).as_deref(), Some("DCIM/a.jpg"));
        assert_eq!(join("DCIM", OsStr::new("..")), None);
        assert_eq!(join("DCIM", OsStr::new("a/b")), None);
        assert_eq!(parent_of("DCIM/Camera/a.jpg"), "DCIM/Camera");
        assert_eq!(parent_of("DCIM"), "");
    }

    #[test]
    fn moves_carry_children_and_forget_frees() {
        let mut n = Nodes::new();
        let dir = n.entered("DCIM");
        let file = n.entered("DCIM/a.jpg");
        n.moved("DCIM", "Photos");
        assert_eq!(n.path(INodeNo(dir)).as_deref(), Some("Photos"));
        assert_eq!(n.path(INodeNo(file)).as_deref(), Some("Photos/a.jpg"));
        assert!(!n.by_path.contains_key("DCIM/a.jpg"));
        n.forget(file, 1);
        assert!(n.path(INodeNo(file)).is_none());
        assert!(!n.by_path.contains_key("Photos/a.jpg"));
        n.forget(INodeNo::ROOT.0, 99);
        assert_eq!(n.path(INodeNo::ROOT).as_deref(), Some(""));
    }

    #[test]
    fn removed_paths_get_new_inodes() {
        let mut n = Nodes::new();
        let a = n.entered("x.txt");
        n.removed("x.txt");
        let b = n.entered("x.txt");
        assert_ne!(a, b);
        // The old inode stays valid until the kernel forgets it.
        assert_eq!(n.path(INodeNo(a)).as_deref(), Some("x.txt"));
        n.forget(a, 1);
        assert_eq!(n.by_path.get("x.txt"), Some(&b));
    }

    #[test]
    fn server_errnos_pass_through() {
        use russh_sftp::protocol::Status;
        let s =
            |code, msg: &str| SftpError::Status(Status { id: 0, status_code: code, error_message: msg.into(), language_tag: "en".into() });
        assert_eq!(errno(&s(StatusCode::Failure, "errno 39")), Errno::ENOTEMPTY);
        assert_eq!(errno(&s(StatusCode::Failure, "errno 17")), Errno::EEXIST);
        assert_eq!(errno(&s(StatusCode::NoSuchFile, "No such file")), Errno::ENOENT);
        assert_eq!(errno(&s(StatusCode::Failure, "Failure")), Errno::EIO);
    }
}

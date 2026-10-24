//! `ef` served by the running EchoFiles app.
//!
//! EchoFiles starts at login and keeps settings, open index maps, user and time-zone
//! tables and a warm thread pool. `ef` connects to a private socket, hands over its
//! stdout and stderr (as file descriptors, so output goes straight to its terminal or
//! pipe with no copying), its working directory, arguments and the few environment
//! variables that change output; the app runs the command and answers with the exit
//! code. If the app isn't running, `ef` runs the same code in its own process.
//!
//! Request: `u32` length, then NUL-separated fields — protocol version, settings file,
//! working folder, flags (`t` stdout is a terminal, `n` NO_COLOR set, `l` LS_COLORS set),
//! LS_COLORS, then the arguments — with the two file descriptors attached (SCM_RIGHTS).
//! Reply: one byte, the exit code; [`DECLINE`] means "run it yourself".

use std::io::{IoSlice, IoSliceMut, Read, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer, SendAncillaryMessage, SendFlags};

use super::Ctx;
use super::find::Indexes;
use crate::config::{self, Settings};

const VERSION: &str = "ef1";
/// Reply meaning "not served here; run it in your own process".
pub const DECLINE: u8 = 254;
const MAX_REQUEST: usize = 1 << 20;

pub fn socket_path() -> PathBuf {
    config::socket_path().with_file_name("echofiles-ef.sock")
}

// ---------------------------------------------------------------------------------------
// Client (the `ef` binary)
// ---------------------------------------------------------------------------------------

/// Ask the running app to run `argv`. `None` if it isn't running (or declines): run it
/// locally then.
pub fn client(argv: &[String], cwd: &std::path::Path, out_tty: bool) -> Option<u8> {
    let path = socket_path();
    // Only talk to our own socket: our terminal is handed to whoever is listening.
    let meta = std::fs::symlink_metadata(&path).ok()?;
    if meta.uid() != rustix::process::getuid().as_raw() || meta.permissions().mode() & 0o077 != 0 {
        return None;
    }
    let sock = UnixStream::connect(&path).ok()?;
    // The app only answers for the same configuration (tests and other setups that point
    // XDG_CONFIG_HOME elsewhere run locally).
    let cfg = Settings::path();
    let mut fields: Vec<&[u8]> = vec![VERSION.as_bytes(), cfg.as_os_str().as_bytes(), cwd.as_os_str().as_bytes()];
    let ls_set = std::env::var_os("LS_COLORS").is_some();
    let flags = format!(
        "{}{}{}",
        if out_tty { "t" } else { "" },
        if std::env::var_os("NO_COLOR").is_some() { "n" } else { "" },
        if ls_set { "l" } else { "" }
    );
    fields.push(flags.as_bytes());
    let ls = std::env::var_os("LS_COLORS").unwrap_or_default();
    fields.push(ls.as_bytes());
    for a in argv {
        fields.push(a.as_bytes());
    }
    let payload = fields.join(&0u8);
    let mut msg = (payload.len() as u32).to_le_bytes().to_vec();
    msg.extend_from_slice(&payload);
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let fds = [stdout.as_fd(), stderr.as_fd()];
    let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut anc = SendAncillaryBuffer::new(&mut space);
    anc.push(SendAncillaryMessage::ScmRights(&fds));
    let sent = rustix::net::sendmsg(&sock, &[IoSlice::new(&msg)], &mut anc, SendFlags::empty()).ok()?;
    if sent != msg.len() {
        return None;
    }
    let mut code = [0u8; 1];
    let mut s = &sock;
    match s.read_exact(&mut code) {
        Ok(()) if code[0] == DECLINE => None,
        Ok(()) => Some(code[0]),
        Err(_) => {
            eprintln!("ef: EchoFiles stopped while answering; run the command again");
            Some(1)
        }
    }
}

// ---------------------------------------------------------------------------------------
// Server (the app)
// ---------------------------------------------------------------------------------------

/// What stays loaded between requests.
struct Warm {
    settings: Mutex<(Option<SystemTime>, Arc<Settings>)>,
    indexes: Arc<Indexes>,
    pool: rayon::ThreadPool,
}

impl Warm {
    /// The settings, re-read only when the file changed.
    fn settings(&self) -> Arc<Settings> {
        let mtime = std::fs::metadata(Settings::path()).and_then(|m| m.modified()).ok();
        let mut s = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        if s.0 != mtime
            && let Ok(fresh) = Settings::load()
        {
            *s = (mtime, Arc::new(fresh));
        }
        s.1.clone()
    }
}

/// Start answering `ef` in this process (the app). Returns whether it is listening.
/// Call once; later calls do nothing.
pub fn spawn() -> bool {
    static STARTED: OnceLock<bool> = OnceLock::new();
    *STARTED.get_or_init(|| {
        let path = socket_path();
        let _ = std::fs::remove_file(&path);
        let Ok(listener) = UnixListener::bind(&path) else { return false };
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let Ok(pool) = rayon::ThreadPoolBuilder::new().num_threads(threads).thread_name(|i| format!("ef-query-{i}")).build() else {
            return false;
        };
        let warm = Arc::new(Warm {
            settings: Mutex::new((None, Arc::new(Settings::load().unwrap_or_default()))),
            indexes: super::shared_indexes(),
            pool,
        });
        // Load now, not on the first query: index maps, users, groups, time zone.
        let w = warm.clone();
        std::thread::spawn(move || {
            let s = w.settings();
            let _ = w.indexes.current(&s);
            let _ = crate::units::users();
            let _ = crate::units::groups();
            let _ = crate::units::tz();
        });
        std::thread::Builder::new()
            .name("ef-server".into())
            .spawn(move || {
                let me = rustix::process::getuid();
                for stream in listener.incoming().flatten() {
                    // Only our own user's processes.
                    match rustix::net::sockopt::socket_peercred(&stream) {
                        Ok(c) if c.uid == me => {}
                        _ => continue,
                    }
                    let warm = warm.clone();
                    let _ = std::thread::Builder::new().name("ef-request".into()).spawn(move || serve(stream, &warm));
                }
            })
            .is_ok()
    })
}

fn serve(stream: UnixStream, warm: &Warm) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut len = [0u8; 4];
    let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(4))];
    let mut anc = RecvAncillaryBuffer::new(&mut space);
    let Ok(got) = rustix::net::recvmsg(&stream, &mut [IoSliceMut::new(&mut len)], &mut anc, RecvFlags::CMSG_CLOEXEC) else { return };
    let mut fds: Vec<OwnedFd> = Vec::new();
    for m in anc.drain() {
        if let RecvAncillaryMessage::ScmRights(r) = m {
            fds.extend(r);
        }
    }
    let reply = |code: u8| {
        let mut s = &stream;
        let _ = s.write_all(&[code]);
    };
    if got.bytes != 4 || fds.len() != 2 {
        return reply(DECLINE);
    }
    let n = u32::from_le_bytes(len) as usize;
    if n > MAX_REQUEST {
        return reply(DECLINE);
    }
    let mut payload = vec![0u8; n];
    if (&stream).read_exact(&mut payload).is_err() {
        return;
    }
    let mut fields = payload.split(|&b| b == 0);
    if fields.next() != Some(VERSION.as_bytes()) || fields.next() != Some(Settings::path().as_os_str().as_bytes()) {
        return reply(DECLINE);
    }
    let cwd = PathBuf::from(std::ffi::OsStr::from_bytes(fields.next().unwrap_or_default()));
    let flags = fields.next().unwrap_or_default().to_vec();
    let ls = fields.next().unwrap_or_default();
    let argv: Vec<String> = match fields.map(|f| String::from_utf8(f.to_vec())).collect::<Result<_, _>>() {
        Ok(a) => a,
        // Arguments that aren't UTF-8: let the client handle them.
        Err(_) => return reply(DECLINE),
    };
    if super::runs_locally(&argv) || !cwd.is_absolute() {
        return reply(DECLINE);
    }
    let mut it = fds.into_iter();
    let (out, err) = (it.next().unwrap(), it.next().unwrap());
    let ctx = Ctx {
        cwd,
        out: super::stream(std::fs::File::from(out)),
        err: super::stream(std::fs::File::from(err)),
        out_tty: flags.contains(&b't'),
        ls_colors: flags.contains(&b'l').then(|| String::from_utf8_lossy(ls).into_owned()),
        no_color: flags.contains(&b'n'),
        settings: warm.settings(),
        indexes: warm.indexes.clone(),
        served: true,
        app_running: true,
    };
    // The release profile unwinds (see Cargo.toml), so a panic here ends this request with
    // an error instead of the whole app.
    let code =
        warm.pool.install(|| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| super::run(&argv, &ctx)))).unwrap_or_else(|_| {
            ctx.write_err(b"ef: internal error while answering; please report it\n");
            1
        });
    let _ = ctx.out.lock().map(|mut o| o.flush());
    // Close our copies of the caller's output before telling it we're done, so a pipe
    // reader sees end-of-file when the caller exits.
    drop(ctx);
    reply(code);
}

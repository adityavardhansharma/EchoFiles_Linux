//! Integration with the desktop: one running instance, start at login, and the EchoFiles
//! skill for AI agents. Every change here is something the user switched on in Settings,
//! and every one is undone when they switch it off.

use std::io::{Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{OpenOptionsExt, MetadataExt, PermissionsExt, FileTypeExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use ef_config as config;
use iced::futures::channel::mpsc;

// ---------------------------------------------------------------------------------------
// Single instance
// ---------------------------------------------------------------------------------------

/// What another `echofiles` launch asks the running one to do.
#[derive(Debug, Clone)]
pub enum Request {
    /// Show a window (at this folder, if given).
    Open(Option<PathBuf>),
    /// Connect to a network address (`smb://nas/Media`) and show it.
    Connect(String),
    Settings,
}

/// What a launch argument asks for: a network address, a `file://` URI (desktop entries
/// pass `%U`) or a path.
pub fn request_for(arg: &str) -> Request {
    let is_net = arg.starts_with("\\\\") || arg.split_once("://").is_some_and(|(s, _)| ef_net::Protocol::from_scheme(s).is_some());
    if is_net {
        return Request::Connect(arg.to_string());
    }
    let path = match arg.strip_prefix("file://") {
        // file:///home/me or file://localhost/home/me
        Some(rest) => decode_path(rest.strip_prefix("localhost").unwrap_or(rest)),
        None => ef_config::expand(arg),
    };
    Request::Open(Some(if path.is_absolute() { path } else { std::env::current_dir().unwrap_or_default().join(path) }))
}

/// If EchoFiles is already running, hand it this launch's request and return `true`.
pub fn forward_to_running(req: &Request) -> bool {
    let path = config::socket_path();
    if !private_runtime(&path) || !std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_socket() && m.uid() == unsafe { libc::getuid() }) { return false; }
    let Ok(mut s) = UnixStream::connect(path) else { return false };
    let (tag, data) = match req {
        Request::Open(Some(p)) => (1, p.as_os_str().as_bytes()),
        Request::Open(None) => (0, &b""[..]),
        Request::Connect(uri) => (2, uri.as_bytes()),
        Request::Settings => (3, &b""[..]),
    };
    if data.len() > 1024 * 1024 { return false; }
    let _ = s.set_write_timeout(Some(std::time::Duration::from_secs(2)));
    s.write_all(&[tag]).and_then(|_| s.write_all(&(data.len() as u32).to_le_bytes())).and_then(|_| s.write_all(data)).is_ok()
}

static OWN_SOCKET: OnceLock<(PathBuf, u64, u64)> = OnceLock::new();

static INBOX: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<Request>>>> = OnceLock::new();

/// Become the running instance: listen for requests from later launches. Returns whether
/// this process bound the socket (and so should remove it on exit).
pub fn listen() -> bool {
    let path = config::socket_path();
    if !private_runtime(&path) { return false; }
    static OWNER: OnceLock<std::fs::File> = OnceLock::new();
    let lock_path = path.with_extension("lock");
    let Ok(lock) = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(lock_path) else { return false };
    if rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).is_err() { return false; }
    // Only the process holding the lifetime lock may unlink a stale socket.
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else { return false };
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    if let Ok(m) = std::fs::symlink_metadata(&path) { let _ = OWN_SOCKET.set((path.clone(), m.dev(), m.ino())); }
    let _ = OWNER.set(lock);
    let (tx, rx) = mpsc::unbounded();
    INBOX.get_or_init(|| Mutex::new(Some(rx)));
    std::thread::Builder::new()
        .name("ef-socket".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                // A client that never sends a line must not block every later launch.
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                let mut stream = stream;
                let mut header = [0u8; 5];
                if stream.read_exact(&mut header).is_err() { continue; }
                let len = u32::from_le_bytes(header[1..].try_into().unwrap()) as usize;
                if len > 1024 * 1024 { continue; }
                let mut data = vec![0; len];
                if stream.read_exact(&mut data).is_err() { continue; }
                let req = match header[0] {
                    0 if len == 0 => Request::Open(None),
                    1 => Request::Open(Some(PathBuf::from(std::ffi::OsString::from_vec(data)))),
                    2 => match String::from_utf8(data) { Ok(u) => Request::Connect(u), Err(_) => continue },
                    3 if len == 0 => Request::Settings,
                    _ => continue,
                };
                if tx.unbounded_send(req).is_err() {
                    return;
                }
            }
        })
        .ok();
    true
}

/// Requests from other launches, as a subscription stream (taken once).
pub fn requests() -> impl iced::futures::Stream<Item = Request> {
    let rx = INBOX.get().and_then(|m| m.lock().unwrap().take());
    iced::futures::stream::unfold(rx, |rx| async move {
        use iced::futures::StreamExt;
        let mut rx = rx?;
        let item = rx.next().await?;
        Some((item, Some(rx)))
    })
}

// ---------------------------------------------------------------------------------------
// Start at login (XDG autostart; Omarchy's uwsm session runs these)
// ---------------------------------------------------------------------------------------

fn autostart_file() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home().join(".config"))
        .join("autostart/echofiles.desktop")
}

pub fn set_start_at_login(on: bool) -> Result<(), String> {
    let file = autostart_file();
    if !on {
        return match std::fs::remove_file(&file) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("Couldn't remove {}: {e}", file.display())),
        };
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let text = format!(
        "[Desktop Entry]\nType=Application\nName=EchoFiles\nComment=Keeps EchoFiles ready and its search index fresh\nExec={} --background\nIcon=echofiles\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\nX-EchoFiles-Owned=true\n",
        desktop_arg(exe.to_str().ok_or("Executable path cannot be represented in a desktop entry")?)
    );
    std::fs::create_dir_all(file.parent().unwrap())
        .and_then(|_| std::fs::write(&file, text))
        .map_err(|e| format!("Couldn't write {}: {e}", file.display()))
}

pub fn starts_at_login() -> bool {
    autostart_file().exists()
}

// ---------------------------------------------------------------------------------------
// AI agent skill (the Omarchy way: one SKILL.md linked into global agent skill folders)
// ---------------------------------------------------------------------------------------

const SKILL: &str = r#"---
name: echofiles
description: >
  Fast file search on this computer through EchoFiles. Use it to find files or folders by
  name in indexed locations or search any accessible folder live with ef find --in, including
  external drives and /tmp, before running find/fd/locate over large trees.
  Triggers: find a file, where is, search files, locate, which folder, all PDFs, recent files.
---

# EchoFiles — `ef`

`ef` searches EchoFiles' index of the user's files. With `--in` on a folder outside the
index, it searches that folder live without changing the indexed locations.

```sh
ef find report                      # names containing all words, best matches first
ef find "annual report"             # exact phrase
ef find '*' --ext pdf --limit 50    # every PDF, first 50
ef find invoice --in ~/Documents    # only inside a folder
ef find report --in /tmp             # live search outside the index
ef find src --dirs                  # folders only (--files for files only)
ef find config --hidden             # include dotfiles and hidden folders
ef find report --count              # just the number of matches
ef status                           # what's indexed and how fresh
```

- Matching ignores case and accents ("resume" finds "Résumé").
- Results are full paths, one per line. Always pass `--limit` for broad queries.
- `--in` accepts any accessible folder. Outside indexed locations it walks the folder live,
  so searches there can take longer.
- The index updates within seconds in Downloads, Desktop, Documents and the folder open in
  EchoFiles, and at least every minute elsewhere. For a file created moments ago in some
  other folder, confirm with `ls`.
- If `ef` says it's turned off, the user disabled it in EchoFiles → Settings → AI agents:
  fall back to `fd`/`find` and don't try to re-enable it.
"#;

fn skill_source_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home().join(".local/share"))
        .join("echofiles/agents/skills/echofiles")
}

fn skill_links() -> [PathBuf; 4] {
    let home = config::home();
    [
        home.join(".agents/skills/echofiles"),
        home.join(".claude/skills/echofiles"),
        home.join(".codex/skills/echofiles"),
        home.join(".pi/agent/skills/echofiles"),
    ]
}

fn is_our_link(link: &Path, source: &Path) -> bool {
    std::fs::read_link(link).is_ok_and(|target| target == source)
}

/// Whether all supported agents can discover the global skill.
pub fn skill_installed() -> bool {
    let source = skill_source_dir();
    source.join("SKILL.md").is_file() && skill_links().iter().all(|link| is_our_link(link, &source))
}

pub fn set_skill(on: bool) -> Result<(), String> {
    let source = skill_source_dir();
    let links = skill_links();
    if !on {
        // Never remove another agent skill with the same name.
        for link in &links {
            if is_our_link(link, &source) {
                std::fs::remove_file(link).map_err(|e| format!("Couldn't remove {}: {e}", link.display()))?;
            }
        }
        return Ok(());
    }

    // Check every destination before touching any of them, so a conflicting skill cannot
    // leave a half-installed set of links.
    for link in &links {
        if link.symlink_metadata().is_ok() && !is_our_link(link, &source) {
            return Err(format!("{} already exists and isn't EchoFiles' — leaving it alone.", config::tilde(link)));
        }
    }

    std::fs::create_dir_all(&source)
        .and_then(|_| std::fs::write(source.join("SKILL.md"), SKILL))
        .map_err(|e| format!("Couldn't write the skill to {}: {e}", source.display()))?;
    for link in &links {
        if is_our_link(link, &source) {
            continue;
        }
        std::fs::create_dir_all(link.parent().unwrap())
            .and_then(|_| std::os::unix::fs::symlink(&source, link))
            .map_err(|e| format!("Couldn't link the skill into {}: {e}", config::tilde(link.parent().unwrap())))?;
    }
    Ok(())
}

/// Apply the saved Settings switch during install and when the app starts.
pub fn sync_skill_from_settings() -> Result<(), String> {
    let settings = config::Settings::load().map_err(|e| format!("Couldn't read EchoFiles settings: {e}"))?;
    set_skill(settings.agents.skill)
}

/// Where `ef` is on `$PATH`, if anywhere.
pub fn ef_on_path() -> Option<PathBuf> {
    std::env::var_os("PATH")?.to_str()?.split(':').map(|d| Path::new(d).join("ef")).find(|p| p.is_file())
}

/// Last folder (for "Open new windows at: Last folder").
pub fn save_last_folder(p: &Path) {
    config::queue_state(config::state_dir().join("last-folder"), p.as_os_str().as_bytes().to_vec());
}

pub fn last_folder() -> Option<PathBuf> {
    let bytes = std::fs::read(config::state_dir().join("last-folder")).ok()?;
    let p = PathBuf::from(std::ffi::OsString::from_vec(bytes));
    p.is_dir().then_some(p)
}

fn decode_path(value: &str) -> PathBuf {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(n) = std::str::from_utf8(&bytes[i+1..i+3]).ok().and_then(|s| u8::from_str_radix(s, 16).ok()) { out.push(n); i += 3; continue; }
        }
        out.push(bytes[i]); i += 1;
    }
    PathBuf::from(std::ffi::OsString::from_vec(out))
}

pub fn request_for_os(arg: &std::ffi::OsStr) -> Request {
    match arg.to_str() { Some(s) => request_for(s), None => Request::Open(Some(PathBuf::from(arg))) }
}

fn desktop_arg(value: &str) -> String {
    let mut quoted = String::from("\"");
    for c in value.chars() {
        if matches!(c, '\\' | '\"' | '`' | '$') { quoted.push('\\'); }
        if c == '%' { quoted.push('%'); }
        quoted.push(c);
    }
    quoted.push('\"');
    quoted.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t")
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn raw_uri_paths_and_desktop_arguments() {
        let Request::Open(Some(p)) = request_for("file:///tmp/new%0Aline%FF") else { panic!() };
        assert_eq!(p.as_os_str().as_bytes(), b"/tmp/new\nline\xff");
        let arg = desktop_arg("/tmp/a b%\"$`\\/echofiles");
        assert!(arg.starts_with('"') && arg.ends_with('"')); assert!(arg.contains("%%")); assert!(arg.contains("\\\\\""));
    }
}

fn private_runtime(socket: &Path) -> bool {
    socket.parent().and_then(|p| std::fs::symlink_metadata(p).ok()).is_some_and(|m| m.is_dir() && m.uid() == unsafe { libc::getuid() } && m.mode() & 0o077 == 0)
}

pub fn remove_own_socket() {
    if let Some((path, dev, ino)) = OWN_SOCKET.get() {
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.dev() == *dev && m.ino() == *ino) { let _ = std::fs::remove_file(path); }
    }
}

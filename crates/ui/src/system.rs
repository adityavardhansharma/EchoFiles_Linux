//! Integration with the desktop: one running instance, start at login, and the EchoFiles
//! skill for AI agents. Every change here is something the user switched on in Settings,
//! and every one is undone when they switch it off.

use std::io::{BufRead, BufReader, Write};
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
    Settings,
}

/// If EchoFiles is already running, hand it this launch's request and return `true`.
pub fn forward_to_running(req: &Request) -> bool {
    let Ok(mut s) = UnixStream::connect(config::socket_path()) else { return false };
    let line = match req {
        Request::Open(Some(p)) => format!("open\t{}\n", p.display()),
        Request::Open(None) => "open\n".into(),
        Request::Settings => "settings\n".into(),
    };
    s.write_all(line.as_bytes()).is_ok()
}

static INBOX: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<Request>>>> = OnceLock::new();

/// Become the running instance: listen for requests from later launches.
pub fn listen() {
    let path = config::socket_path();
    let _ = std::fs::remove_file(&path); // stale socket from a crash; `forward_to_running` failed
    let Ok(listener) = UnixListener::bind(&path) else { return };
    let (tx, rx) = mpsc::unbounded();
    INBOX.get_or_init(|| Mutex::new(Some(rx)));
    std::thread::Builder::new()
        .name("ef-socket".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut line = String::new();
                if BufReader::new(stream).read_line(&mut line).is_err() {
                    continue;
                }
                let line = line.trim_end_matches('\n');
                let req = match line.split_once('\t') {
                    Some(("open", p)) if !p.is_empty() => Request::Open(Some(PathBuf::from(p))),
                    _ if line == "settings" => Request::Settings,
                    _ => Request::Open(None),
                };
                if tx.unbounded_send(req).is_err() {
                    return;
                }
            }
        })
        .ok();
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
        "[Desktop Entry]\nType=Application\nName=EchoFiles\nComment=Keeps EchoFiles ready and its search index fresh\nExec={} --background\nIcon=echofiles\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
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
    let dir = config::state_dir();
    let _ = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join("last-folder"), p.as_os_str().as_encoded_bytes()));
}

pub fn last_folder() -> Option<PathBuf> {
    let bytes = std::fs::read(config::state_dir().join("last-folder")).ok()?;
    let p = PathBuf::from(String::from_utf8(bytes).ok()?);
    p.is_dir().then_some(p)
}

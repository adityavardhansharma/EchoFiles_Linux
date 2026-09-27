//! A polkit authentication agent for EchoFiles' own requests.
//!
//! udisks needs an administrator password to mount internal drives. Desktops normally run
//! a session-wide agent that shows the password prompt; a bare Hyprland session may have
//! none, so mounting would just fail. EchoFiles registers an agent scoped to its own
//! process (the way `pkttyagent --process` does), so it only ever answers questions about
//! what EchoFiles itself asked for, and shows the prompt in its own dialog.
//!
//! The password never leaves this process except to polkit's helper
//! (`/run/polkit/agent-helper.socket`), which runs PAM and tells polkitd the result.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, OnceLock};

use futures_channel::oneshot;
use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Value};

const AGENT_PATH: &str = "/org/echofiles/PolkitAgent";
const HELPER_SOCKET: &str = "/run/polkit/agent-helper.socket";
const ATTEMPTS: usize = 3;

/// One password request for the UI. Answer with [`Prompt::answer`]; `None` cancels.
#[derive(Clone, Debug)]
pub struct Prompt {
    /// polkit's message, e.g. "Authentication is required to mount AVS".
    pub message: String,
    pub user: String,
    /// The previous attempt failed; show "That password didn't work".
    pub retry: bool,
    reply: Arc<Mutex<Option<oneshot::Sender<Option<String>>>>>,
}

impl Prompt {
    pub fn answer(&self, password: Option<String>) {
        if let Some(tx) = self.reply.lock().unwrap().take() {
            let _ = tx.send(password);
        }
    }
}

struct Agent {
    on_prompt: Box<dyn Fn(Prompt) + Send + Sync>,
    cancelled: Arc<Mutex<Vec<String>>>,
}

fn user_name(uid: u32) -> Option<String> {
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|l| {
        let mut f = l.split(':');
        let name = f.next()?;
        let _ = f.next();
        (f.next()?.parse::<u32>().ok()? == uid).then(|| name.to_string())
    })
}

fn identity_uid(id: &(String, HashMap<String, OwnedValue>)) -> Option<u32> {
    if id.0 != "unix-user" {
        return None;
    }
    match &**id.1.get("uid")? {
        Value::U32(u) => Some(*u),
        Value::I32(i) => Some(*i as u32),
        _ => None,
    }
}

/// Talk to polkit's helper: `user\ncookie\n`, then answer PAM's prompts with the password.
fn authenticate(user: &str, cookie: &str, password: &str) -> Result<(), String> {
    let mut s = UnixStream::connect(HELPER_SOCKET).map_err(|e| format!("couldn't reach polkit's helper: {e}"))?;
    write!(s, "{user}\n{cookie}\n").map_err(|e| e.to_string())?;
    let mut lines = BufReader::new(s.try_clone().map_err(|e| e.to_string())?);
    let mut line = String::new();
    let mut last_error = String::new();
    loop {
        line.clear();
        if lines.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err(if last_error.is_empty() { "polkit's helper stopped".into() } else { last_error });
        }
        let l = line.trim_end_matches('\n');
        if l.starts_with("PAM_PROMPT_ECHO_OFF") || l.starts_with("PAM_PROMPT_ECHO_ON") {
            writeln!(s, "{password}").map_err(|e| e.to_string())?;
        } else if let Some(m) = l.strip_prefix("PAM_ERROR_MSG ") {
            last_error = m.to_string();
        } else if l.starts_with("SUCCESS") {
            return Ok(());
        } else if l.starts_with("FAILURE") {
            return Err(if last_error.is_empty() { "wrong password".into() } else { last_error });
        }
    }
}

#[zbus::interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Agent {
    async fn begin_authentication(
        &self,
        _action_id: String,
        message: String,
        _icon_name: String,
        _details: HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, HashMap<String, OwnedValue>)>,
    ) -> zbus::fdo::Result<()> {
        // Prefer authenticating as ourselves (wheel members), else the first admin offered.
        let me = unsafe { libc_getuid() };
        let uid = identities.iter().filter_map(identity_uid).find(|&u| u == me).or_else(|| identities.iter().find_map(identity_uid));
        let user = uid.and_then(user_name).ok_or_else(|| zbus::fdo::Error::Failed("no user to authenticate as".into()))?;
        for attempt in 0..ATTEMPTS {
            let (tx, rx) = oneshot::channel();
            (self.on_prompt)(Prompt { message: message.clone(), user: user.clone(), retry: attempt > 0, reply: Arc::new(Mutex::new(Some(tx))) });
            let password = match rx.await {
                Ok(Some(p)) => p,
                _ => return Err(zbus::fdo::Error::Failed("cancelled".into())),
            };
            if self.cancelled.lock().unwrap().contains(&cookie) {
                return Err(zbus::fdo::Error::Failed("cancelled".into()));
            }
            let (u, c) = (user.clone(), cookie.clone());
            // The helper blocks on PAM (which may sleep after a wrong password): off-thread.
            let (done_tx, done_rx) = oneshot::channel();
            std::thread::spawn(move || {
                let _ = done_tx.send(authenticate(&u, &c, &password));
            });
            match done_rx.await {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(_)) if attempt + 1 < ATTEMPTS => continue,
                Ok(Err(e)) => return Err(zbus::fdo::Error::Failed(e)),
                Err(_) => return Err(zbus::fdo::Error::Failed("helper vanished".into())),
            }
        }
        Err(zbus::fdo::Error::Failed("too many attempts".into()))
    }

    async fn cancel_authentication(&self, cookie: String) -> zbus::fdo::Result<()> {
        self.cancelled.lock().unwrap().push(cookie);
        Ok(())
    }
}

unsafe extern "C" {
    #[link_name = "getuid"]
    fn libc_getuid() -> u32;
}

fn start_time() -> Option<u64> {
    // Field 22 of /proc/self/stat; the command name (field 2) may contain spaces, so count
    // from the closing parenthesis.
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let rest = &stat[stat.rfind(')')? + 2..];
    rest.split_whitespace().nth(19)?.parse().ok()
}

static AGENT: OnceLock<Result<Connection, String>> = OnceLock::new();

/// Register the agent once per process. Prompts arrive through `on_prompt`, from a D-Bus
/// thread; later calls ignore their callback and return the first result.
pub fn ensure(on_prompt: impl Fn(Prompt) + Send + Sync + 'static) -> Result<(), String> {
    AGENT
        .get_or_init(|| {
            let agent = Agent { on_prompt: Box::new(on_prompt), cancelled: Arc::default() };
            let conn = zbus::blocking::connection::Builder::system()
                .and_then(|b| b.serve_at(AGENT_PATH, agent))
                .and_then(|b| b.build())
                .map_err(|e| e.to_string())?;
            let mut subject: HashMap<&str, Value> = HashMap::new();
            subject.insert("pid", Value::U32(std::process::id()));
            subject.insert("start-time", Value::U64(start_time().unwrap_or(0)));
            let locale = std::env::var("LANG").unwrap_or_else(|_| "C".into());
            conn.call_method(
                Some("org.freedesktop.PolicyKit1"),
                "/org/freedesktop/PolicyKit1/Authority",
                Some("org.freedesktop.PolicyKit1.Authority"),
                "RegisterAuthenticationAgent",
                &(("unix-process", subject), locale.as_str(), AGENT_PATH),
            )
            .map_err(|e| e.to_string())?;
            Ok(conn)
        })
        .as_ref()
        .map(drop)
        .map_err(Clone::clone)
}

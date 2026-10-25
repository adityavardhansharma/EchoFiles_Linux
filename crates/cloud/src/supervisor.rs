//! The one `rclone rcd` EchoFiles owns: start it lazily, keep it answering, restart it with
//! backoff, stop it cleanly.
//!
//! An rcd left over from a crashed EchoFiles (it still answers on our socket) is told to
//! unmount and quit before a fresh one starts. That replaces a parent-death signal, which
//! Linux ties to the *thread* that spawned the child and would kill rclone whenever a
//! short-lived background thread finished.

use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::error::{Error, Kind};
use crate::paths;
use crate::rc::Rc;
use crate::rclone::Rclone;

/// Restart delays after unexpected exits, then a pause between tries.
const DELAYS: [u64; 5] = [1, 2, 5, 15, 30];
/// More than this many failures inside [`WINDOW`] and we stop and say so.
const GIVE_UP_AFTER: usize = 5;
const WINDOW: Duration = Duration::from_secs(120);
const LOG_MAX: u64 = 5 * 1024 * 1024;

/// When to try starting again after failures.
#[derive(Debug, Default)]
pub struct Backoff {
    failures: Vec<Instant>,
}

impl Backoff {
    /// Record a failure at `now`; `None` means give up (too many failures too fast).
    pub fn fail(&mut self, now: Instant) -> Option<Duration> {
        self.failures.retain(|t| now.duration_since(*t) < WINDOW);
        self.failures.push(now);
        if self.failures.len() >= GIVE_UP_AFTER {
            return None;
        }
        Some(Duration::from_secs(DELAYS[(self.failures.len() - 1).min(DELAYS.len() - 1)]))
    }

    pub fn reset(&mut self) {
        self.failures.clear();
    }
}

/// A running rcd.
pub struct Rcd {
    child: Child,
    rc: Rc,
}

impl Rcd {
    /// Start `rclone rcd` on our socket with our config, cache and log.
    pub fn spawn(rclone: &Rclone) -> Result<Rcd, Error> {
        let io = |e: std::io::Error| Error::new(Kind::Io, e.to_string());
        let sock = paths::socket();
        if let Some(dir) = sock.parent() {
            paths::private_dir(dir).map_err(io)?;
        }
        paths::private_dir(&ef_config::cloud_cache_dir()).map_err(io)?;
        paths::private_dir(&ef_config::cloud_mount_root()).map_err(io)?;
        paths::ensure_config().map_err(io)?;
        let log = paths::log_file();
        if let Some(dir) = log.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        rotate(&log);
        let rc = Rc::new(&sock);
        quit_orphan(&rc);
        let _ = std::fs::remove_file(&sock);
        let mut cmd = rclone.command();
        cmd.arg("rcd")
            .arg("--rc-addr").arg(format!("unix://{}", sock.display()))
            // Only this user can reach the socket (0700 folder), so rc needs no password.
            .arg("--rc-no-auth")
            .arg("--cache-dir").arg(ef_config::cloud_cache_dir())
            .arg("--log-file").arg(&log)
            .arg("--log-level").arg("NOTICE")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = cmd.spawn().map_err(|e| Error::new(Kind::NotRunning, format!("couldn't start rclone: {e}")))?;
        let mut rcd = Rcd { child, rc };
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if rcd.rc.ping().is_ok() {
                return Ok(rcd);
            }
            if let Ok(Some(status)) = rcd.child.try_wait() {
                return Err(Error::new(Kind::NotRunning, format!("rclone exited ({status}): {}", last_log_line(&log).unwrap_or_default())));
            }
            if Instant::now() > until {
                rcd.kill();
                return Err(Error::new(Kind::Timeout, "rclone didn't start answering within 5 s"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn rc(&self) -> &Rc {
        &self.rc
    }

    pub fn exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    /// Unmount everything, ask rclone to quit, wait up to 5 s, then kill.
    pub fn stop(mut self) {
        let _ = self.rc.call("mount/unmountall", &json!({}), Duration::from_secs(10));
        let _ = self.rc.call("core/quit", &json!({}), Duration::from_secs(2));
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until {
            if self.exited() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        self.kill();
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// An rcd from an EchoFiles that crashed still owns our socket: unmount and quit it.
fn quit_orphan(rc: &Rc) {
    if rc.ping().is_err() {
        return;
    }
    let _ = rc.call("mount/unmountall", &json!({}), Duration::from_secs(10));
    let _ = rc.call("core/quit", &json!({}), Duration::from_secs(2));
    let until = Instant::now() + Duration::from_secs(5);
    while Instant::now() < until && rc.ping().is_ok() {
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Keep the last two logs of at most 5 MB.
fn rotate(log: &Path) {
    if std::fs::metadata(log).is_ok_and(|m| m.len() > LOG_MAX) {
        let _ = std::fs::rename(log, log.with_extension("log.1"));
    }
}

fn last_log_line(log: &Path) -> Option<String> {
    let text = std::fs::read_to_string(log).ok()?;
    text.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.chars().take(300).collect())
}

/// What the supervisor is doing, for the sidebar and Settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Health {
    /// Not started (no account, or not needed yet).
    Idle,
    Running,
    /// Waiting before the next try.
    Restarting,
    /// Too many failures; "Try again" resets it.
    GaveUp(String),
}

/// Owns the rcd. All methods block; call them from background threads.
pub struct Supervisor {
    rclone: Rclone,
    inner: Mutex<Inner>,
}

struct Inner {
    rcd: Option<Rcd>,
    backoff: Backoff,
    next_try: Option<Instant>,
    gave_up: Option<String>,
    missed_pings: u8,
}

/// The rcd was (re)started by this call: mounts must be restored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Started {
    Fresh,
    Existing,
}

impl Supervisor {
    pub fn new(rclone: Rclone) -> Supervisor {
        Supervisor { rclone, inner: Mutex::new(Inner { rcd: None, backoff: Backoff::default(), next_try: None, gave_up: None, missed_pings: 0 }) }
    }

    pub fn rclone(&self) -> &Rclone {
        &self.rclone
    }

    /// A handle to a running rcd, starting one if needed (respecting backoff).
    pub fn ensure(&self) -> Result<(Rc, Started), Error> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(rcd) = g.rcd.as_mut() {
            if !rcd.exited() {
                return Ok((rcd.rc().clone(), Started::Existing));
            }
            g.rcd = None;
            let now = Instant::now();
            match g.backoff.fail(now) {
                Some(d) => g.next_try = Some(now + d),
                None => g.gave_up = Some("rclone keeps exiting — see the log.".into()),
            }
        }
        if let Some(why) = &g.gave_up {
            return Err(Error::new(Kind::NotRunning, why.clone()));
        }
        if let Some(t) = g.next_try
            && Instant::now() < t
        {
            return Err(Error::new(Kind::NotRunning, "rclone is restarting"));
        }
        match Rcd::spawn(&self.rclone) {
            Ok(rcd) => {
                let rc = rcd.rc().clone();
                g.rcd = Some(rcd);
                g.next_try = None;
                g.missed_pings = 0;
                Ok((rc, Started::Fresh))
            }
            Err(e) => {
                let now = Instant::now();
                match g.backoff.fail(now) {
                    Some(d) => g.next_try = Some(now + d),
                    None => g.gave_up = Some(format!("rclone keeps failing to start: {}", e.message)),
                }
                Err(e)
            }
        }
    }

    /// The handle if the rcd is running, without starting it.
    pub fn running(&self) -> Option<Rc> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let r = g.rcd.as_mut()?;
        if r.exited() { None } else { Some(r.rc().clone()) }
    }

    /// The 60 s health check: two missed pings in a row kill a hung rcd, so the next
    /// [`ensure`](Self::ensure) starts a fresh one. A healthy check resets the backoff.
    pub fn check(&self) -> Health {
        let rc = {
            let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(why) = &g.gave_up {
                return Health::GaveUp(why.clone());
            }
            let next_try = g.next_try.is_some();
            match g.rcd.as_mut() {
                None => return if next_try { Health::Restarting } else { Health::Idle },
                Some(r) => {
                    if r.exited() {
                        return Health::Restarting;
                    }
                    r.rc().clone()
                }
            }
        };
        let ok = rc.ping().is_ok();
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if ok {
            g.missed_pings = 0;
            g.backoff.reset();
            return Health::Running;
        }
        g.missed_pings += 1;
        if g.missed_pings >= 2 {
            if let Some(mut r) = g.rcd.take() {
                r.kill();
            }
            g.missed_pings = 0;
            let now = Instant::now();
            match g.backoff.fail(now) {
                Some(d) => g.next_try = Some(now + d),
                None => g.gave_up = Some("rclone stopped answering — see the log.".into()),
            }
            return Health::Restarting;
        }
        Health::Running
    }

    /// "Try again" after giving up.
    pub fn reset(&self) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.gave_up = None;
        g.next_try = None;
        g.backoff.reset();
    }

    /// Unmount, quit, and forget the rcd (quit, last account removed).
    pub fn stop(&self) {
        let rcd = self.inner.lock().unwrap_or_else(|e| e.into_inner()).rcd.take();
        if let Some(r) = rcd {
            r.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_then_gives_up() {
        let mut b = Backoff::default();
        let t0 = Instant::now();
        let steps: Vec<_> = (0..4).map(|i| b.fail(t0 + Duration::from_secs(i)).map(|d| d.as_secs())).collect();
        assert_eq!(steps, vec![Some(1), Some(2), Some(5), Some(15)]);
        assert_eq!(b.fail(t0 + Duration::from_secs(5)), None, "five failures in two minutes");
        // Failures older than two minutes don't count.
        let mut b = Backoff::default();
        for i in 0..4 {
            b.fail(t0 + Duration::from_secs(i * 60));
        }
        assert!(b.fail(t0 + Duration::from_secs(4 * 60)).is_some());
        b.reset();
        assert_eq!(b.fail(t0).map(|d| d.as_secs()), Some(1));
    }
}

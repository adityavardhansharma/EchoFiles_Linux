//! Browser sign-in: `rclone config reconnect <remote>: --auto-confirm` as a short-lived
//! child. rclone starts its own page on 127.0.0.1:53682 and opens Google's sign-in in the
//! browser; we only read its output for the link (for "Copy sign-in link") and the result.
//! The remote already exists (created over rc), so no secret is ever on a command line.

use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::error::{Error, Kind};
use crate::paths;
use crate::rclone::Rclone;

#[derive(Default)]
struct State {
    url: Option<String>,
    done: Option<Result<(), Error>>,
    output: String,
}

/// A sign-in in progress. Cheap to clone; every clone sees the same child.
#[derive(Clone)]
pub struct SignIn {
    child: Arc<Mutex<Option<Child>>>,
    state: Arc<(Mutex<State>, Condvar)>,
}

impl SignIn {
    pub fn start(rclone: &Rclone, account: &str) -> Result<SignIn, Error> {
        paths::ensure_config().map_err(|e| Error::new(Kind::Io, e.to_string()))?;
        let mut cmd = rclone.command();
        cmd.arg("config").arg("reconnect").arg(paths::fs(account)).arg("--auto-confirm").stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| Error::new(Kind::NotRunning, format!("couldn't start rclone: {e}")))?;
        let state: Arc<(Mutex<State>, Condvar)> = Arc::default();
        let streams: Vec<Box<dyn std::io::Read + Send>> = [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>)].into_iter().flatten().collect();
        let readers: Vec<_> = streams
            .into_iter()
            .map(|s| {
                let st = state.clone();
                std::thread::spawn(move || {
                    for line in BufReader::new(s).lines().map_while(Result::ok) {
                        let (m, cv) = &*st;
                        let mut g = m.lock().unwrap_or_else(|e| e.into_inner());
                        if g.url.is_none()
                            && let Some(u) = auth_url(&line)
                        {
                            g.url = Some(u);
                            cv.notify_all();
                        }
                        if g.output.len() < 16 * 1024 {
                            g.output.push_str(&line);
                            g.output.push('\n');
                        }
                    }
                })
            })
            .collect();
        let child = Arc::new(Mutex::new(Some(child)));
        let (c2, s2) = (child.clone(), state.clone());
        std::thread::spawn(move || {
            // Poll so `cancel` can take the child and kill it meanwhile.
            let status = loop {
                let mut g = c2.lock().unwrap_or_else(|e| e.into_inner());
                match g.as_mut().map(Child::try_wait) {
                    None => break None,
                    Some(Ok(Some(st))) => break Some(st),
                    Some(Ok(None)) => {}
                    Some(Err(_)) => break None,
                }
                drop(g);
                std::thread::sleep(Duration::from_millis(100));
            };
            for r in readers {
                let _ = r.join();
            }
            let (m, cv) = &*s2;
            let mut g = m.lock().unwrap_or_else(|e| e.into_inner());
            g.done = Some(match status {
                Some(st) if st.success() => Ok(()),
                Some(_) => Err(explain_failure(&g.output)),
                None => Err(Error::new(Kind::Other, "Sign-in cancelled.")),
            });
            cv.notify_all();
        });
        Ok(SignIn { child, state })
    }

    /// Wait (up to `limit`) for the link rclone prints; `None` once finished without one.
    pub fn wait_url(&self, limit: Duration) -> Option<String> {
        let (m, cv) = &*self.state;
        let g = m.lock().unwrap_or_else(|e| e.into_inner());
        let (g, _) = cv.wait_timeout_while(g, limit, |s| s.url.is_none() && s.done.is_none()).unwrap_or_else(|e| e.into_inner());
        g.url.clone()
    }

    /// Wait until rclone exits.
    pub fn wait(&self) -> Result<(), Error> {
        let (m, cv) = &*self.state;
        let g = m.lock().unwrap_or_else(|e| e.into_inner());
        let g = cv.wait_while(g, |s| s.done.is_none()).unwrap_or_else(|e| e.into_inner());
        g.done.clone().unwrap_or(Ok(()))
    }

    /// Stop waiting for the browser: kill rclone (frees port 53682).
    pub fn cancel(&self) {
        if let Some(mut c) = self.child.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// rclone's line naming the local sign-in page.
pub fn auth_url(line: &str) -> Option<String> {
    let start = line.find("http://127.0.0.1:53682/")?;
    let url: String = line[start..].chars().take_while(|c| !c.is_whitespace() && *c != '"').collect();
    Some(url)
}

/// What went wrong, in the dialog's words.
pub fn explain_failure(output: &str) -> Error {
    let o = output.to_ascii_lowercase();
    if o.contains("address already in use") || o.contains("bind:") {
        Error::new(Kind::Other, "Another sign-in is using port 53682. Finish or close it, then try again.")
    } else if o.contains("access_denied") {
        Error::new(Kind::Other, "Google didn't allow access. Try again and choose Allow.")
    } else if o.contains("no such host") || o.contains("dial tcp") || o.contains("network is unreachable") {
        Error::new(Kind::Offline, "Couldn't reach Google. Check the internet connection.")
    } else {
        let last = output.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("rclone stopped without saying why");
        Error::new(Kind::Other, format!("Sign-in didn't finish: {}", last.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_link_and_explains() {
        assert_eq!(auth_url("2026/10/25 NOTICE: If your browser doesn't open automatically go to the following link: http://127.0.0.1:53682/auth?state=Abc_12"), Some("http://127.0.0.1:53682/auth?state=Abc_12".into()));
        assert_eq!(auth_url("Waiting for code..."), None);
        assert!(explain_failure("listen tcp 127.0.0.1:53682: bind: address already in use").message.contains("port 53682"));
        assert!(explain_failure("error=access_denied").message.contains("Allow"));
        assert!(explain_failure("weird\n").message.contains("weird"));
    }

    #[test]
    fn cancel_ends_wait() {
        // A stand-in "rclone" that sleeps, so cancel has something to kill.
        let r = Rclone { path: "/bin/sleep".into(), version: crate::rclone::MIN_VERSION };
        let mut c = std::process::Command::new(&r.path);
        c.arg("30");
        let child = c.spawn().unwrap();
        let s = SignIn { child: Arc::new(Mutex::new(Some(child))), state: Arc::default() };
        let s2 = s.clone();
        let waiter = std::thread::spawn(move || {
            // Mark done the way `start`'s watcher does once the child is gone.
            loop {
                if s2.child.lock().unwrap().is_none() {
                    let (m, cv) = &*s2.state;
                    m.lock().unwrap().done = Some(Err(Error::new(Kind::Other, "Sign-in cancelled.")));
                    cv.notify_all();
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        s.cancel();
        assert!(s.wait().is_err());
        waiter.join().unwrap();
        assert_eq!(s.wait_url(Duration::from_millis(10)), None);
    }
}

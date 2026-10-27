//! The app's phone connection, lent to `ef phone` ([`ef_index::phone::Link`]).
//!
//! `ef` requests carry a `requestId` starting with `ef-` and a random part, and their
//! answers are taken out of the phone event stream before the window sees them
//! ([`claim`]): no gallery updates, toasts or transfer rows for an agent's fetches. Only a
//! file answering a request made here, from the phone it was asked of, is accepted.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::{Duration, Instant};

use ef_index::phone::{Link, LinkPhone};
use ef_phone::{Event, Value};

enum Waiter {
    Reply { device: String, tx: SyncSender<Value> },
    File { device: String, dir: PathBuf, tx: SyncSender<Result<PathBuf, String>> },
}

static WAITERS: Mutex<Option<HashMap<String, Waiter>>> = Mutex::new(None);
/// Transfers accepted for `ef`: transfer → (request, where the result goes).
static TRANSFERS: Mutex<Option<HashMap<u64, (String, SyncSender<Result<PathBuf, String>>)>>> = Mutex::new(None);

fn with_waiters<R>(f: impl FnOnce(&mut HashMap<String, Waiter>) -> R) -> R {
    f(WAITERS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new))
}

fn with_transfers<R>(f: impl FnOnce(&mut HashMap<u64, (String, SyncSender<Result<PathBuf, String>>)>) -> R) -> R {
    f(TRANSFERS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new))
}

fn new_request() -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    let mut random = [0u8; 8];
    let _ = getrandom::fill(&mut random);
    format!("ef-{}-{}", N.fetch_add(1, Ordering::Relaxed), random.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

fn request_of(body: &Value) -> Option<&str> {
    body["requestId"].as_str().filter(|r| r.starts_with("ef-"))
}

/// Take `e` if it answers an `ef` request; the window never sees it then.
pub fn claim(e: &Event) -> bool {
    match e {
        Event::Other { id, body, .. } => {
            let Some(request) = request_of(body) else { return false };
            with_waiters(|w| match w.remove(request) {
                Some(Waiter::Reply { device, tx }) if device == *id => {
                    let _ = tx.try_send(body.clone());
                }
                // A packet instead of a file: the phone saying why it can't send it.
                Some(Waiter::File { device, tx, .. }) if device == *id => {
                    let why = body["error"].as_str().or(body["message"].as_str()).unwrap_or("The phone couldn't send the file");
                    let _ = tx.try_send(Err(why.to_owned()));
                }
                Some(other) => {
                    w.insert(request.to_owned(), other);
                }
                None => {}
            });
            true
        }
        Event::Incoming { id, transfer, body, .. } => {
            let Some(request) = request_of(body) else { return false };
            let waiter = with_waiters(|w| match w.remove(request) {
                Some(Waiter::File { device, dir, tx }) if device == *id => Some((dir, tx)),
                Some(other) => {
                    w.insert(request.to_owned(), other);
                    None
                }
                None => None,
            });
            if let Some(s) = crate::phone::service() {
                match waiter {
                    Some((dir, tx)) => {
                        with_transfers(|t| t.insert(*transfer, (request.to_owned(), tx)));
                        s.accept_file(*transfer, dir);
                    }
                    None => s.reject_file(*transfer),
                }
            }
            true
        }
        Event::TransferStarted { transfer, upload: false, .. } => with_transfers(|t| t.contains_key(transfer)),
        Event::TransferDone { transfer, upload: false, result, .. } => match with_transfers(|t| t.remove(transfer)) {
            Some((_, tx)) => {
                let _ = tx.try_send(result.clone());
                true
            }
            None => false,
        },
        _ => false,
    }
}

pub struct Bridge;

impl Bridge {
    fn send(&self, phone: &str, kind: &str, mut body: Value, request: &str) -> Result<(), String> {
        let s = crate::phone::service().ok_or("The phone connection isn't running in EchoFiles")?;
        body["requestId"] = Value::from(request);
        if !s.send_packet(phone, kind, body) {
            return Err("The phone isn't connected".into());
        }
        Ok(())
    }
}

impl Link for Bridge {
    fn phones(&self) -> Vec<LinkPhone> {
        let connected: Vec<ef_phone::Device> = crate::phone::service().map(|s| s.devices()).unwrap_or_default();
        let mut out: Vec<LinkPhone> = connected
            .iter()
            .filter(|d| d.paired)
            .map(|d| LinkPhone { id: d.id.clone(), name: d.name.clone(), online: d.lan, echoconnect: d.echoconnect() })
            .collect();
        for t in ef_phone::trusted() {
            if !out.iter().any(|p| p.id == t.id) {
                out.push(LinkPhone { id: t.id, name: t.name, online: false, echoconnect: false });
            }
        }
        out
    }

    fn ask(&self, phone: &str, kind: &str, body: Value, wait: Duration) -> Result<Value, String> {
        let request = new_request();
        let (tx, rx) = sync_channel(1);
        with_waiters(|w| w.insert(request.clone(), Waiter::Reply { device: phone.to_owned(), tx }));
        if let Err(e) = self.send(phone, kind, body, &request) {
            with_waiters(|w| w.remove(&request));
            return Err(e);
        }
        let r = rx.recv_timeout(wait);
        with_waiters(|w| w.remove(&request));
        r.map_err(|_| "The phone didn't answer in time".into())
    }

    fn fetch(&self, phone: &str, kind: &str, body: Value, dir: &Path, wait: Duration) -> Result<PathBuf, String> {
        let request = new_request();
        let (tx, rx) = sync_channel(1);
        let started = Instant::now();
        with_waiters(|w| w.insert(request.clone(), Waiter::File { device: phone.to_owned(), dir: dir.to_owned(), tx }));
        if let Err(e) = self.send(phone, kind, body, &request) {
            with_waiters(|w| w.remove(&request));
            return Err(e);
        }
        let r = rx.recv_timeout(wait.saturating_sub(started.elapsed()));
        // Late: forget the request and stop a transfer that's still running.
        with_waiters(|w| w.remove(&request));
        if r.is_err() {
            let late: Vec<u64> = with_transfers(|t| {
                let ids: Vec<u64> = t.iter().filter(|(_, (q, _))| *q == request).map(|(id, _)| *id).collect();
                for id in &ids {
                    t.remove(id);
                }
                ids
            });
            if let Some(s) = crate::phone::service() {
                for id in late {
                    s.cancel_transfer(id);
                }
            }
        }
        r.unwrap_or_else(|_| Err("The phone didn't send the file in time".into()))
    }
}

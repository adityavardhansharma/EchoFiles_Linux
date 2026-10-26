//! Search, step 2 (design §10): after saved results paint, the same search goes to Google
//! Drive. New matches join the list (and the Drive index, if it's on); saved Drive results
//! Drive didn't confirm are checked one by one, and gone ones leave.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ef_cloud::catalog;
use ef_cloud::drive::GoogleKind;
use ef_cloud::live::{self, Found, Skip};
use ef_cloud::paths;
use iced::Task;

use crate::app::{background, App, Message};
use crate::cloud::CloudMsg;
use crate::search;

/// Quiet time after the last keystroke before Drive is asked.
const DEBOUNCE: Duration = Duration::from_millis(350);
/// Saved Drive results checked when Drive didn't confirm them.
const VERIFY: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveStatus {
    Searching,
    Done { fresh: usize, at: Instant },
    Timeout,
    Offline,
    /// Regex, glob or size/date filters: Drive can't be asked.
    Pattern,
}

impl LiveStatus {
    /// The status bar's words, or nothing once "checked" has been shown for 3 s.
    pub fn text(&self) -> Option<String> {
        Some(match self {
            LiveStatus::Searching => "Searching Google Drive…".into(),
            LiveStatus::Done { fresh, at } if at.elapsed() < Duration::from_secs(3) => {
                if *fresh > 0 { format!("Google Drive checked · {fresh} new") } else { "Google Drive checked".into() }
            }
            LiveStatus::Done { .. } => return None,
            LiveStatus::Timeout => "Google Drive didn't answer — showing saved results".into(),
            LiveStatus::Offline => "Google Drive unreachable — showing saved results".into(),
            LiveStatus::Pattern => "Google Drive can't search by pattern — showing saved results".into(),
        })
    }
}

/// One account's answer: what Drive found, and My Drive's own ID if it was looked up.
#[derive(Debug, Clone)]
pub struct Answer {
    pub account: String,
    pub root: Option<String>,
    pub found: Vec<Found>,
}

/// Where a live match goes in the list.
#[derive(Debug, Clone)]
struct Candidate {
    path: PathBuf,
    dir: bool,
    size: i64,
    modified: i64,
    google: Option<GoogleKind>,
    rank: u64,
    put: Option<(String, catalog::Entry)>,
}

#[derive(Default)]
pub struct LiveState {
    inflight: bool,
    pending: Option<(u64, u64)>,
    roots: HashMap<String, String>,
    folders_asked: HashMap<String, Instant>,
}

impl App {
    fn mounted_accounts(&self) -> Vec<String> {
        self.settings.cloud.accounts.iter().filter(|a| self.cloud.status.get(&a.id).is_some_and(|s| s.mounted && !s.signed_out)).map(|a| a.id.clone()).collect()
    }

    /// Saved results painted: ask Drive once typing has paused.
    pub(crate) fn cloud_live_schedule(&mut self, pane: u64, generation: u64) -> Task<Message> {
        if !self.settings.cloud.live_search || self.mounted_accounts().is_empty() {
            return Task::none();
        }
        background(move || std::thread::sleep(DEBOUNCE), move |_| Message::Cloud(CloudMsg::LiveDue { pane, generation }))
    }

    pub(crate) fn cloud_live_due(&mut self, pane_id: u64, generation: u64) -> Task<Message> {
        let hidden = self.show_hidden;
        let Some(pane) = self.pane_by_id(pane_id).filter(|p| p.search_generation == generation && p.everywhere()) else { return Task::none() };
        let text = pane.query.trim().to_string();
        let plan = search::live_plan(&text, hidden);
        let Some(pane) = self.pane_by_id_mut(pane_id) else { return Task::none() };
        let (ask, _) = match plan {
            Ok(p) => p,
            Err(Skip::Pattern) => {
                pane.live = Some(LiveStatus::Pattern);
                return Task::none();
            }
            Err(Skip::TooShort) => {
                pane.live = None;
                return Task::none();
            }
        };
        if self.cloud.live.inflight {
            // Latest wins: one query at a time, the newest text next.
            self.cloud.live.pending = Some((pane_id, generation));
            return Task::none();
        }
        let Ok(q) = live::translate(&ask) else { return Task::none() };
        let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
        if let Some(p) = self.pane_by_id_mut(pane_id) {
            p.live = Some(LiveStatus::Searching);
        }
        self.cloud.live.inflight = true;
        let accounts: Vec<(String, Option<String>)> = self.mounted_accounts().into_iter().map(|a| {
            let r = self.cloud.live.roots.get(&a).cloned();
            (a, r)
        }).collect();
        background(
            move || {
                let rc = sup.running().ok_or_else(|| ef_cloud::Error::new(ef_cloud::Kind::NotRunning, "rclone isn't running"))?;
                let mut out = Vec::new();
                let mut last_err = None;
                for (account, root) in accounts {
                    let root = match root {
                        Some(r) => Some(r),
                        None => live::root_id(&rc, &account).ok().flatten(),
                    };
                    match live::query(&rc, &account, &q) {
                        Ok(found) => out.push(Answer { account, root, found }),
                        Err(e) => last_err = Some(e),
                    }
                }
                match (out.is_empty(), last_err) {
                    (true, Some(e)) => Err(e),
                    _ => Ok(out),
                }
            },
            move |r| Message::Cloud(CloudMsg::LiveResults { pane: pane_id, generation, result: r }),
        )
    }

    pub(crate) fn cloud_live_results(&mut self, pane_id: u64, generation: u64, result: Result<Vec<Answer>, ef_cloud::Error>) -> Task<Message> {
        self.cloud.live.inflight = false;
        let next = self.cloud.live.pending.take().map(|(p, g)| Task::done(Message::Cloud(CloudMsg::LiveDue { pane: p, generation: g }))).unwrap_or_else(Task::none);
        let hidden = self.show_hidden;
        let current = self.pane_by_id(pane_id).is_some_and(|p| p.search_generation == generation);
        if !current {
            return next;
        }
        let answers = match result {
            Ok(a) => a,
            Err(e) => {
                if let Some(p) = self.pane_by_id_mut(pane_id) {
                    p.live = Some(if e.kind == ef_cloud::Kind::Timeout { LiveStatus::Timeout } else { LiveStatus::Offline });
                }
                return next;
            }
        };
        let text = self.pane_by_id(pane_id).map(|p| p.query.trim().to_string()).unwrap_or_default();
        let Ok((_, filter)) = search::live_plan(&text, hidden) else { return next };
        let ranker = ef_index::Ranker::new(&text);
        let mut candidates: Vec<Candidate> = Vec::new();
        let mut unresolved: HashSet<String> = HashSet::new();
        for ans in &answers {
            if let Some(r) = &ans.root {
                self.cloud.live.roots.insert(ans.account.clone(), r.clone());
            }
            let mut folders = self.cloud.index.folders.get(&ans.account).cloned().unwrap_or_default();
            if let Some(r) = &ans.root {
                folders.insert(r.clone(), String::new());
            }
            for f in &ans.found {
                let name = f.mount_name();
                let Some(rank) = filter.rank(&name, f.is_dir()) else { continue };
                let google = GoogleKind::from_mime(&f.mime);
                match f.path_in(&folders) {
                    Some(path) => {
                        let put = catalog::Entry { id: f.id.clone(), parent: f.parents.first().cloned().unwrap_or_default(), path: path.clone(), dir: f.is_dir(), size: f.size, modified: f.modified, mime: f.mime.clone(), md5: String::new() };
                        let shown = name.strip_suffix(ef_cloud::drive::LINK_SUFFIX).unwrap_or(&name);
                        candidates.push(Candidate { path: paths::mount_point(&ans.account).join(&path), dir: f.is_dir(), size: f.size, modified: f.modified, google, rank: ranker.rank(shown.as_bytes()).min(rank), put: Some((ans.account.clone(), put)) });
                    }
                    // A folder made since the folder map was built: open it in the browser for now.
                    None if !f.web.is_empty() => {
                        unresolved.insert(ans.account.clone());
                        candidates.push(Candidate { path: PathBuf::from(&f.web), dir: f.is_dir(), size: f.size, modified: f.modified, google, rank, put: None });
                    }
                    None => {}
                }
            }
        }
        candidates.sort_by_key(|c| c.rank);
        let settings = self.settings.clone();
        let mut tasks = vec![next];
        // The index learns what live search found (§9.4 source 3).
        if settings.cloud.index {
            let puts: Vec<(String, serde_json::Value)> = candidates.iter().filter_map(|c| c.put.as_ref()).map(|(a, e)| (a.clone(), catalog::put_change(e))).collect();
            std::thread::spawn(move || {
                for (a, c) in puts {
                    let _ = catalog::append(&a, &c);
                }
            });
        }
        // Folders Drive named that we don't know yet: refresh the map (at most once a minute).
        for a in unresolved {
            if self.cloud.live.folders_asked.get(&a).is_none_or(|t| t.elapsed() > Duration::from_secs(60)) {
                self.cloud.live.folders_asked.insert(a.clone(), Instant::now());
                if !settings.cloud.index {
                    tasks.push(self.cloud_crawl(&a, true));
                }
            }
        }
        let Some(pane) = self.pane_by_id_mut(pane_id) else { return Task::batch(tasks) };
        let Some(res) = pane.results.as_mut() else {
            pane.live = Some(LiveStatus::Done { fresh: 0, at: Instant::now() });
            return Task::batch(tasks);
        };
        let have: HashSet<PathBuf> = res.hits.iter().map(|h| h.path.clone()).collect();
        let confirmed: HashSet<PathBuf> = candidates.iter().map(|c| c.path.clone()).filter(|p| have.contains(p)).collect();
        let fresh: Vec<search::Hit> = candidates
            .into_iter()
            .filter(|c| !have.contains(&c.path))
            .map(|c| {
                let mut h = search::drive_hit(&settings, c.path.clone(), c.dir, c.size, c.modified, c.google);
                if c.put.is_none() {
                    h.location = "Google Drive · folder not indexed yet (opens in the browser)".into();
                }
                h
            })
            .collect();
        let n = fresh.len();
        let untouched = pane.result_cursor.is_none_or(|c| c == 0);
        if untouched {
            // Nothing under the hands yet: one list in rank order (no animation).
            res.hits.extend(fresh);
            let r = ef_index::Ranker::new(&text);
            res.hits.sort_by_cached_key(|h| r.rank(h.name.as_bytes()));
        } else {
            // Never move what's under the cursor: new matches go below it.
            let at = pane.result_cursor.map_or(0, |c| c + 1).min(res.hits.len());
            res.hits.splice(at..at, fresh);
        }
        res.total += n;
        if untouched && !res.hits.is_empty() {
            pane.result_cursor = Some(0);
        }
        pane.live = Some(LiveStatus::Done { fresh: n, at: Instant::now() });
        // Saved Drive results Drive didn't confirm: check they still exist (§10.6).
        let check: Vec<PathBuf> = res.hits.iter().filter(|h| ef_config::is_cloud_path(&h.path) && !confirmed.contains(&h.path) && !h.gone).take(VERIFY).map(|h| h.path.clone()).collect();
        if !check.is_empty() {
            tasks.push(background(
                move || {
                    use rayon::prelude::*;
                    check.into_par_iter().filter(|p| matches!(std::fs::symlink_metadata(p), Err(e) if e.kind() == std::io::ErrorKind::NotFound)).collect::<Vec<_>>()
                },
                move |gone| Message::Cloud(CloudMsg::LiveChecked { pane: pane_id, generation, gone }),
            ));
        }
        Task::batch(tasks)
    }

    pub(crate) fn cloud_live_checked(&mut self, pane_id: u64, generation: u64, gone: Vec<PathBuf>) -> Task<Message> {
        if gone.is_empty() {
            return Task::none();
        }
        self.cloud_index_removed(&gone);
        let Some(pane) = self.pane_by_id_mut(pane_id).filter(|p| p.search_generation == generation) else { return Task::none() };
        let Some(res) = pane.results.as_mut() else { return Task::none() };
        let selected = pane.result_cursor.and_then(|c| res.hits.get(c)).map(|h| h.path.clone());
        let gone: HashSet<PathBuf> = gone.into_iter().collect();
        let before = res.hits.len();
        res.hits.retain_mut(|h| {
            if !gone.contains(&h.path) {
                return true;
            }
            if Some(&h.path) == selected.as_ref() {
                // Keep the row under the cursor, marked, so nothing jumps.
                h.gone = true;
                return true;
            }
            false
        });
        res.total = res.total.saturating_sub(before - res.hits.len());
        if let Some(sel) = selected {
            pane.result_cursor = res.hits.iter().position(|h| h.path == sel);
        }
        Task::none()
    }

    /// Open a live result that only has a browser link.
    pub(crate) fn open_web_result(&mut self, p: &Path) -> Task<Message> {
        self.cloud_update(CloudMsg::OpenUrl(p.to_string_lossy().into_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_words() {
        assert_eq!(LiveStatus::Searching.text().unwrap(), "Searching Google Drive…");
        assert_eq!(LiveStatus::Done { fresh: 3, at: Instant::now() }.text().unwrap(), "Google Drive checked · 3 new");
        assert!(LiveStatus::Done { fresh: 0, at: Instant::now() - Duration::from_secs(5) }.text().is_none());
        assert!(LiveStatus::Pattern.text().unwrap().contains("pattern"));
    }
}

//! The Google Drive index (opt-in: Settings → Google Drive → Include Google Drive in search
//! index, its own switch). Full crawls write each account's catalog; folder listings, our
//! own operations and live search keep it fresh in between (design §9.4). With the switch
//! off, only an in-memory folder map exists (§9.6), never written to disk.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime};

use ef_cloud::catalog;
use ef_cloud::crawl::{self, Progress};
use ef_cloud::{paths, Availability};
use iced::widget::{container, row, text};
use iced::{Alignment, Element, Length, Task};

use crate::app::{background, App, Message};
use crate::cloud::CloudMsg;
use crate::style::{self, color};
use crate::widgets::{self as w, Variant};

/// A full crawl older than this runs again at start (live search covers what's new).
const STALE: Duration = Duration::from_secs(24 * 3600);

#[derive(Clone, Default)]
pub enum IndexRun {
    #[default]
    Idle,
    Crawling(Progress),
    Done { items: usize, at: SystemTime },
    Failed(String),
}


/// Per-account index state and the folder maps (folder ID → path).
#[derive(Default)]
pub struct CloudIndex {
    pub runs: HashMap<String, IndexRun>,
    pub folders: HashMap<String, HashMap<String, String>>,
}

fn crawl_options(app: &App, dirs_only: bool, own_client: bool) -> crawl::Options {
    crawl::Options { dirs_only, tps: if own_client { 20 } else { 8 }, exclude_names: if dirs_only { Vec::new() } else { app.settings.search.exclude_names.clone() } }
}

fn age(account: &str) -> Option<Duration> {
    std::fs::metadata(paths::catalog(account)).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok())
}

impl App {
    /// The switch changed. On: crawl every account. Off: delete every Drive catalog now.
    pub(crate) fn cloud_index_switched(&mut self, on: bool) -> Task<Message> {
        if !on {
            for run in self.cloud.index.runs.values() {
                if let IndexRun::Crawling(p) = run {
                    p.cancel.store(true, Ordering::Relaxed);
                }
            }
            self.cloud.index.runs.clear();
            let _ = std::fs::remove_dir_all(ef_config::cloud_index_dir());
            let ids: Vec<String> = self.settings.cloud.accounts.iter().map(|a| a.id.clone()).collect();
            return Task::batch(ids.iter().map(|id| self.cloud_crawl(id, true)).collect::<Vec<_>>());
        }
        let ids: Vec<String> = self.settings.cloud.accounts.iter().filter(|a| a.in_index()).map(|a| a.id.clone()).collect();
        Task::batch(ids.iter().map(|id| self.cloud_crawl(id, false)).collect::<Vec<_>>())
    }

    /// An account mounted: crawl if its catalog is missing or a day old (index on), or build
    /// the in-memory folder map (index off).
    pub(crate) fn cloud_index_mounted(&mut self, id: &str) -> Task<Message> {
        let in_index = self.cloud_account(id).is_some_and(|a| a.in_index());
        if !self.settings.cloud.index || !in_index {
            return if self.cloud.index.folders.contains_key(id) { Task::none() } else { self.cloud_crawl(id, true) };
        }
        if age(id).is_some_and(|a| a < STALE) {
            if !self.cloud.index.folders.contains_key(id) {
                let i2 = id.to_string();
                return background(move || catalog::load(&i2).folder_paths(), {
                    let id = id.to_string();
                    move |f| Message::Cloud(CloudMsg::Folders(id.clone(), f))
                });
            }
            return Task::none();
        }
        self.cloud_crawl(id, false)
    }

    /// Start a full crawl (or a folders-only one for the folder map).
    pub(crate) fn cloud_crawl(&mut self, id: &str, dirs_only: bool) -> Task<Message> {
        let Some(Availability::Ready(rclone)) = self.cloud.avail.clone() else { return Task::none() };
        if matches!(self.cloud.index.runs.get(id), Some(IndexRun::Crawling(_))) {
            return Task::none();
        }
        let own = self.cloud.form.clients.iter().any(|(a, c)| a == id && c.is_some());
        let o = crawl_options(self, dirs_only, own);
        let progress = Progress::default();
        if !dirs_only {
            self.cloud.index.runs.insert(id.to_string(), IndexRun::Crawling(progress.clone()));
        }
        let i2 = id.to_string();
        let id = id.to_string();
        background(
            move || {
                let c = crawl::crawl(&rclone, &i2, &o, &progress).map_err(|e| e.explain())?;
                if !dirs_only {
                    catalog::write_base(&i2, &c).map_err(|e| e.to_string())?;
                }
                Ok((c.len(), c.folder_paths()))
            },
            move |r: Result<(usize, HashMap<String, String>), String>| Message::Cloud(CloudMsg::Crawled { account: id.clone(), dirs_only, result: r }),
        )
    }

    pub(crate) fn cloud_crawled(&mut self, account: String, dirs_only: bool, result: Result<(usize, HashMap<String, String>), String>) -> Task<Message> {
        match result {
            Ok((n, folders)) => {
                self.cloud.index.folders.insert(account.clone(), folders);
                if !dirs_only {
                    self.cloud.index.runs.insert(account, IndexRun::Done { items: n, at: SystemTime::now() });
                }
            }
            Err(e) => {
                if !dirs_only {
                    self.cloud.index.runs.insert(account, IndexRun::Failed(e));
                }
            }
        }
        Task::none()
    }

    /// A Drive folder was listed through the API: record exactly what's in it (§9.4 source 2)
    /// and keep the folder map current.
    pub(crate) fn cloud_folder_listed(&mut self, account: &str, dir: &str, entries: &[ef_cloud::drive::Entry]) {
        let cat: Vec<catalog::Entry> = entries.iter().map(|e| catalog::Entry::from_drive(e, String::new())).collect();
        if let Some(map) = self.cloud.index.folders.get_mut(account) {
            for e in cat.iter().filter(|e| e.dir && !e.id.is_empty()) {
                map.insert(e.id.clone(), e.path.clone());
            }
        }
        if self.settings.cloud.index {
            let change = catalog::folder_change(dir, &cat);
            let account = account.to_string();
            std::thread::spawn(move || {
                let _ = catalog::append(&account, &change);
            });
        }
    }

    /// Changes made in EchoFiles (§9.4 source 1): deletes leave the catalog at once.
    pub(crate) fn cloud_index_removed(&self, paths: &[std::path::PathBuf]) {
        if !self.settings.cloud.index {
            return;
        }
        let changes: Vec<(String, serde_json::Value)> = paths.iter().filter_map(|p| ef_cloud::DriveUri::from_mount_path(p)).map(|u| (u.account.clone(), catalog::del_change("", &u.path))).collect();
        std::thread::spawn(move || {
            for (a, c) in changes {
                let _ = catalog::append(&a, &c);
            }
        });
    }

    /// Google's change feed (own client ID): index changes almost as they happen.
    pub(crate) fn cloud_apply_changes(&mut self, account: &str, changes: Vec<ef_cloud::changes::Change>) {
        if changes.is_empty() || !self.settings.cloud.index {
            return;
        }
        let folders = self.cloud.index.folders.entry(account.to_string()).or_default();
        let root = self.cloud.live.root(account);
        let mut lines = Vec::new();
        for c in changes {
            if c.gone {
                folders.remove(&c.id);
                lines.push(catalog::del_change(&c.id, ""));
                continue;
            }
            let found = ef_cloud::live::Found { id: c.id.clone(), name: c.name.clone(), parents: c.parents.clone(), mime: c.mime.clone(), size: c.size, modified: c.modified, web: String::new() };
            let mut known = folders.clone();
            if let Some(r) = &root {
                known.insert(r.clone(), String::new());
            }
            let Some(path) = found.path_in(&known) else { continue };
            if found.is_dir() {
                folders.insert(c.id.clone(), path.clone());
            }
            lines.push(catalog::put_change(&catalog::Entry { id: c.id, parent: c.parents.first().cloned().unwrap_or_default(), path, dir: found.is_dir(), size: c.size, modified: c.modified, mime: c.mime, md5: String::new() }));
        }
        let account = account.to_string();
        std::thread::spawn(move || {
            for l in lines {
                let _ = catalog::append(&account, &l);
            }
        });
    }

    /// Hourly: catalogs a day old crawl again.
    pub(crate) fn cloud_index_tick(&mut self) -> Task<Message> {
        if !self.settings.cloud.index {
            return Task::none();
        }
        let stale: Vec<String> = self.settings.cloud.accounts.iter().filter(|a| a.in_index() && self.cloud.status.get(&a.id).is_some_and(|s| s.mounted)).filter(|a| age(&a.id).is_none_or(|t| t >= STALE)).map(|a| a.id.clone()).collect();
        Task::batch(stale.iter().map(|id| self.cloud_crawl(id, false)).collect::<Vec<_>>())
    }

    pub(crate) fn cloud_index_status(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mut items = 0usize;
        let mut crawling: Option<u64> = None;
        let mut failed: Option<&str> = None;
        let mut newest: Option<SystemTime> = None;
        for a in &self.settings.cloud.accounts {
            match self.cloud.index.runs.get(&a.id) {
                Some(IndexRun::Crawling(pr)) => *crawling.get_or_insert(0) += pr.items.load(Ordering::Relaxed),
                Some(IndexRun::Done { items: n, at }) => {
                    items += n;
                    newest = newest.max(Some(*at));
                }
                Some(IndexRun::Failed(e)) => failed = Some(e),
                _ => {
                    if let Ok(m) = std::fs::metadata(paths::catalog(&a.id)) {
                        newest = newest.max(m.modified().ok());
                        items += std::fs::read(paths::catalog(&a.id)).map(|b| b.iter().filter(|&&c| c == b'\n').count()).unwrap_or(0);
                    }
                }
            }
        }
        let (pill, detail) = if let Some(n) = crawling {
            (w::pill(p, "Indexing", Some(p.info)), format!("Indexing Google Drive · {} items", crate::settings::fmt_count(n as usize)))
        } else if let Some(e) = failed {
            (w::pill(p, "Problem", Some(p.warning)), e.to_string())
        } else if newest.is_some() {
            (w::pill(p, "Ready", Some(p.success)), format!("Google Drive indexed · {} items · {}", crate::settings::fmt_count(items), crate::indexer::ago(newest)))
        } else if self.settings.cloud.accounts.is_empty() {
            (w::pill(p, "Off", None), "Connect Google Drive first.".to_string())
        } else {
            (w::pill(p, "Waiting", None), "Indexes once Google Drive is connected.".to_string())
        };
        let refresh = w::text_button(
            p,
            &self.icons,
            if crawling.is_some() { "Indexing…" } else { "Refresh now" },
            Some("refresh"),
            None,
            Variant::Secondary,
            (crawling.is_none() && !self.settings.cloud.accounts.is_empty()).then_some(Message::Cloud(CloudMsg::RefreshIndex)),
        );
        container(row![pill, text(detail).size(style::META).font(style::FONT).color(color(p.ink_muted)).width(Length::Fill), refresh].spacing(style::SPACE_4).align_y(Alignment::Center)).padding([10, 16]).width(Length::Fill).into()
    }
}

impl std::fmt::Debug for IndexRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexRun::Idle => f.write_str("Idle"),
            IndexRun::Crawling(p) => write!(f, "Crawling({})", p.items.load(Ordering::Relaxed)),
            IndexRun::Done { items, .. } => write!(f, "Done({items})"),
            IndexRun::Failed(e) => write!(f, "Failed({e})"),
        }
    }
}

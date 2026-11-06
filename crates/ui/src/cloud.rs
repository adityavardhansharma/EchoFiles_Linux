//! Google Drive in EchoFiles (design/docs/google-drive.md; design system `CloudItem`,
//! `CloudConnectDialog`, `CloudPage`).
//!
//! A connected account is mounted by rclone under `$XDG_RUNTIME_DIR/echofiles/cloud/<id>`,
//! so once it's open every other part of EchoFiles treats it like any folder. This module
//! owns what's special: the Cloud section, signing in through the browser, mounting and
//! restoring mounts, quota and pending uploads, Google Docs in listings, and Drive's own
//! trash.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ef_cloud::drive::{self, MountOptions, Uploads};
use ef_cloud::signin::SignIn;
use ef_cloud::supervisor::{Health, Supervisor};
use ef_cloud::{paths, Availability, DriveUri};
use ef_config::{self as config, CloudAccess, CloudAccount};
use iced::widget::{column, container, row, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Point, Task};

use crate::app::{background, App, Message};
use crate::overlay::Tone;
use crate::style::{self, color};
use crate::widgets::{self as w, Variant};

pub const RENAME_ID: &str = "cloud-rename";
const SIGNIN_LIMIT: Duration = Duration::from_secs(5 * 60);
const TRASH_URL: &str = "https://drive.google.com/drive/trash";
const DRIVE_URL: &str = "https://drive.google.com/";
const PERMISSIONS_URL: &str = "https://myaccount.google.com/permissions";

// ------------------------------------------------------------------------------ state

/// One account's live state (not saved).
#[derive(Debug, Default, Clone)]
pub struct Status {
    pub mounted: bool,
    pub mounting: bool,
    /// Two calls in a row failed with network errors.
    pub offline: bool,
    net_failures: u8,
    /// Google revoked or expired the sign-in.
    pub signed_out: bool,
    /// (used, total); total 0 = unlimited.
    pub quota: Option<(u64, u64)>,
    quota_at: Option<Instant>,
    pub uploads: Uploads,
    /// Last mount error, for the tooltip.
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Step {
    /// Choose access, then sign in.
    Start { access: CloudAccess },
    /// The browser is open; rclone waits for Google.
    Waiting { account: String, url: Option<String>, again: bool, started: Instant },
    /// rclone is missing or too old.
    Missing,
    Rename { account: String, text: String },
    SignOut { account: String },
    /// Files from the Drive are open in other apps.
    Busy { account: String },
    /// Quitting while uploads are pending.
    Quit,
    /// Pick a shared drive to add (`None` while asking).
    Drives { account: String, drives: Option<Vec<(String, String)>> },
}

#[derive(Debug, Clone)]
pub struct CloudDialog {
    pub step: Step,
    pub error: Option<String>,
    pub opened: Instant,
}

#[derive(Default)]
pub struct CloudState {
    /// rclone as found; `None` until checked.
    pub avail: Option<Availability>,
    pub checking: bool,
    pub fuse: bool,
    pub sup: Option<Arc<Supervisor>>,
    pub status: HashMap<String, Status>,
    pub dialog: Option<CloudDialog>,
    signin: Option<(String, SignIn)>,
    /// Accounts to open in the active pane once mounted.
    open_after: HashSet<String>,
    /// A Drive folder from last time, reopened if its mount is back quickly.
    pub reopen: Option<(PathBuf, Instant)>,
    polling: bool,
    health_at: Option<Instant>,
    /// Folders whose Google types were asked for (no repeat on every reload).
    kinds_asked: HashMap<PathBuf, Instant>,
    /// Access chosen for the sign-in in progress.
    pending_access: Option<CloudAccess>,
    /// When the last pending upload finished ("Uploaded to Google Drive" for a moment).
    pub uploaded_at: Option<Instant>,
    /// "Stopped responding" was already said.
    gave_up_shown: bool,
    /// Settings → Google Drive drafts and facts.
    pub form: crate::cloud_settings::CloudForm,
    /// A change (client ID, access) waiting for its sign-in.
    pub again: Option<crate::cloud_settings::Again>,
    /// Drive index runs and folder maps.
    pub index: crate::cloud_index::CloudIndex,
    index_at: Option<Instant>,
    pub live: crate::cloud_live::LiveState,
    changes_at: Option<Instant>,
    changes_busy: bool,
}

impl std::fmt::Debug for CloudState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CloudState")
    }
}

#[derive(Debug, Clone)]
pub enum CloudMsg {
    Checked(Availability, bool),
    Recheck,
    OpenConnect,
    Close,
    SetAccess(CloudAccess),
    StartSignIn,
    Started { account: String, again: bool, result: Result<Handle, String> },
    SignInUrl(String, Option<String>),
    SignInDone { account: String, again: bool, result: Result<(), String> },
    Verified { account: String, again: bool, result: Result<(u64, u64), String> },
    CopyLink,
    Mounted(String, Result<(), ef_cloud::Error>),
    Click(String),
    OpenTab(String),
    Menu(String, Point),
    OpenUrl(String),
    Refresh(String),
    Disconnect(String),
    Disconnected(String, Result<(), ef_cloud::Error>),
    DisconnectAnyway(String),
    AskSignOut(String),
    SignOut(String),
    SignedOut,
    AskRename(String),
    RenameDraft(String),
    RenameSubmit,
    Quota(String, Result<(u64, u64), ef_cloud::Error>),
    Polled(Vec<(String, Result<Uploads, ef_cloud::Error>)>),
    Health(Health, Vec<String>),
    TryAgain,
    Trashed(Vec<PathBuf>, Vec<String>),
    Purged(Vec<PathBuf>, Vec<String>),
    QuitWait,
    QuitAnyway,
    DownloadAs(PathBuf, &'static str),
    Downloaded(String, Result<PathBuf, String>),
    CopiedWithin(Vec<PathBuf>, Vec<String>),
    CopyText(String),
    // Settings → Google Drive
    ClientDraft(String),
    SecretDraft(String),
    ToggleSecret,
    ToggleAdvanced,
    SaveClient,
    UseRcloneApp,
    ClientsRead(Vec<(String, Option<String>)>),
    ClientChanged { account: String, result: Result<Option<drive::Client>, String> },
    ChangeAccess(String),
    ScopeSet(String, Result<(), String>),
    CacheDraft(String),
    CacheSubmit,
    CacheSize(u64),
    ClearCache,
    SetIndex(bool),
    SetLive(bool),
    OpenLog,
    // index
    RefreshIndex,
    Crawled { account: String, dirs_only: bool, result: Result<(usize, HashMap<String, String>), String> },
    Folders(String, HashMap<String, String>),
    Listed { dir: PathBuf, account: String, path: String, entries: Vec<drive::Entry> },
    // live search
    LiveDue { pane: u64, generation: u64 },
    LiveResults { pane: u64, generation: u64, result: Result<Vec<crate::cloud_live::Answer>, ef_cloud::Error> },
    LiveChecked { pane: u64, generation: u64, gone: Vec<PathBuf> },
    // phase 7
    Changes(String, Result<Option<Vec<ef_cloud::changes::Change>>, ef_cloud::Error>),
    Untrash(Vec<PathBuf>),
    Untrashed(Vec<PathBuf>, Result<(u64, u64), String>),
    // phase 8
    PlaceRemoved(String),
    ListDrives(String),
    DrivesListed(String, Result<Vec<(String, String)>, String>),
    AddPlace(String, PlaceChoice),
    PlaceAdded(CloudAccount, Result<(), String>),
    PlaceIndexed(String, bool),
    SetFulltext(bool),
}

/// What to add under an account.
#[derive(Debug, Clone)]
pub enum PlaceChoice {
    Shared,
    /// A shared drive: (Drive's ID, name).
    Team(String, String),
}

/// A sign-in handle that can travel in a message.
#[derive(Clone)]
pub struct Handle(pub SignIn);

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SignIn")
    }
}

fn cm(m: CloudMsg) -> Message {
    Message::Cloud(m)
}

/// "4.2 GB of 15 GB" / "4.2 GB used".
pub fn usage_text(used: u64, total: u64) -> String {
    let mut s = String::new();
    ef_core::fmt::size(used, &mut s);
    if total > 0 {
        s.push_str(" of ");
        ef_core::fmt::size(total, &mut s);
    } else {
        s.push_str(" used");
    }
    s
}

/// The name rclone gives a Google Doc in the mount, without its `.link.html`.
pub fn display_stem(name: &str) -> Option<&str> {
    name.strip_suffix(drive::LINK_SUFFIX)
}

/// First boot work: find rclone if any account is connected.
pub fn boot_tasks(settings: &config::Settings) -> Task<Message> {
    if settings.cloud.accounts.is_empty() {
        return Task::none();
    }
    check_task()
}

fn check_task() -> Task<Message> {
    background(|| (ef_cloud::rclone::check(), ef_cloud::rclone::fuse_available()), |(a, f)| cm(CloudMsg::Checked(a, f)))
}

impl App {
    // ------------------------------------------------------------------ lookups

    pub(crate) fn cloud_account(&self, id: &str) -> Option<&CloudAccount> {
        self.settings.cloud.accounts.iter().find(|a| a.id == id)
    }

    /// The account holding `path`, its name and mount root.
    pub(crate) fn cloud_place_at(&self, path: &Path) -> Option<(&CloudAccount, PathBuf)> {
        let uri = DriveUri::from_mount_path(path)?;
        let a = self.cloud_account(&uri.account)?;
        Some((a, paths::mount_point(&a.id)))
    }

    pub(crate) fn cloud_mount_options(&self, a: &CloudAccount) -> MountOptions {
        let shared = matches!(a.place, Some(config::CloudPlace::Shared));
        MountOptions { cache_max_gb: self.settings.cloud.cache_max_gb, read_only: a.access == CloudAccess::Read || shared }
    }

    fn cloud_rclone(&self) -> Option<&ef_cloud::Rclone> {
        match &self.cloud.avail {
            Some(Availability::Ready(r)) => Some(r),
            _ => None,
        }
    }

    pub(crate) fn cloud_uploads(&self) -> u64 {
        self.cloud.status.values().map(|s| s.uploads.pending()).sum()
    }

    fn cloud_active(&self, id: &str) -> bool {
        let pane = self.pane();
        !pane.special() && pane.location.starts_with(paths::mount_point(id))
    }

    // ------------------------------------------------------------------ tasks

    fn cloud_mount_task(&mut self, id: &str) -> Task<Message> {
        let (Some(sup), Some(a)) = (self.cloud.sup.clone(), self.cloud_account(id).cloned()) else { return Task::none() };
        let o = self.cloud_mount_options(&a);
        let st = self.cloud.status.entry(id.to_string()).or_default();
        if st.mounting {
            return Task::none();
        }
        st.mounting = true;
        let id = id.to_string();
        background(
            move || {
                let (rc, _) = sup.ensure()?;
                drive::mount(&rc, &a.id, o)
            },
            move |r| cm(CloudMsg::Mounted(id.clone(), r)),
        )
    }

    fn cloud_quota_task(&self, id: &str) -> Task<Message> {
        let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
        let id = id.to_string();
        background(
            {
                let id = id.clone();
                move || {
                    let rc = sup.running().ok_or_else(|| ef_cloud::Error::new(ef_cloud::Kind::NotRunning, "rclone isn't running"))?;
                    drive::about(&rc, &id)
                }
            },
            move |r| cm(CloudMsg::Quota(id.clone(), r)),
        )
    }

    /// Every second while anything is mounted: pending uploads (and the 60 s health check).
    pub(crate) fn cloud_tick(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        // Sign-in that never came back from the browser.
        if let Some(CloudDialog { step: Step::Waiting { started, .. }, .. }) = &self.cloud.dialog
            && started.elapsed() > SIGNIN_LIMIT
        {
            self.cloud_cancel_signin();
            if let Some(d) = &mut self.cloud.dialog {
                d.step = Step::Start { access: CloudAccess::Full };
                d.error = Some("Sign-in timed out.".into());
            }
        }
        if let Some((_, at)) = &self.cloud.reopen
            && at.elapsed() > Duration::from_secs(2)
        {
            self.cloud.reopen = None;
        }
        let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
        let mounted: Vec<String> = self.cloud.status.iter().filter(|(_, s)| s.mounted).map(|(id, _)| id.clone()).collect();
        if !mounted.is_empty() && !self.cloud.polling {
            self.cloud.polling = true;
            let s2 = sup.clone();
            tasks.push(background(
                move || {
                    let rc = s2.running();
                    mounted.into_iter().map(|id| {
                        let r = match &rc {
                            Some(rc) => drive::uploads(rc, &id),
                            None => Err(ef_cloud::Error::new(ef_cloud::Kind::NotRunning, "rclone isn't running")),
                        };
                        (id, r)
                    }).collect()
                },
                |v| cm(CloudMsg::Polled(v)),
            ));
        }
        if self.cloud.health_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(60)) && !self.settings.cloud.accounts.is_empty() {
            self.cloud.health_at = Some(Instant::now());
            let ids: Vec<String> = self.settings.cloud.accounts.iter().map(|a| a.id.clone()).collect();
            let s2 = sup.clone();
            tasks.push(background(
                move || {
                    let h = s2.check();
                    // Mounts that should be there but aren't (rclone restarted, unmounted elsewhere).
                    let lost = ids.into_iter().filter(|id| !drive::is_mount_point(&paths::mount_point(id))).collect();
                    (h, lost)
                },
                |(h, lost)| cm(CloudMsg::Health(h, lost)),
            ));
        }
        // Own client ID + index on: Google's change feed every minute (§12.4).
        if self.settings.cloud.index && !self.cloud.changes_busy && self.cloud.changes_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(60)) {
            self.cloud.changes_at = Some(Instant::now());
            let own: Vec<String> = self.cloud.form.clients.iter().filter(|(id, c)| c.is_some() && self.cloud.status.get(id).is_some_and(|s| s.mounted)).map(|(id, _)| id.clone()).collect();
            for id in own {
                self.cloud.changes_busy = true;
                let s2 = sup.clone();
                let i2 = id.clone();
                tasks.push(background(move || s2.running().ok_or_else(|| ef_cloud::Error::new(ef_cloud::Kind::NotRunning, "")).and_then(|rc| ef_cloud::changes::poll(&rc, &i2)), move |r| cm(CloudMsg::Changes(id.clone(), r))));
            }
        }
        if self.cloud.index_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(3600)) {
            self.cloud.index_at = Some(Instant::now());
            tasks.push(self.cloud_index_tick());
        }
        let stale: Vec<String> = self.cloud.status.iter().filter(|(_, s)| s.mounted && s.quota_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(600))).map(|(id, _)| id.clone()).collect();
        for id in stale {
            if let Some(s) = self.cloud.status.get_mut(&id) {
                s.quota_at = Some(Instant::now());
            }
            tasks.push(self.cloud_quota_task(&id));
        }
        Task::batch(tasks)
    }

    /// A listing of a Drive folder finished: ask which entries are Google Docs (once a minute
    /// per folder at most), so they show as "Google Doc" with the right icon.
    pub(crate) fn cloud_listed(&mut self, dir: &Path) -> Task<Message> {
        let Some(uri) = DriveUri::from_mount_path(dir) else { return Task::none() };
        if self.cloud.kinds_asked.get(dir).is_some_and(|t| t.elapsed() < Duration::from_secs(5)) {
            return Task::none();
        }
        let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
        self.cloud.kinds_asked.insert(dir.to_path_buf(), Instant::now());
        let dir = dir.to_path_buf();
        background(
            {
                let uri = uri.clone();
                move || sup.running().and_then(|rc| drive::list(&rc, &uri.account, &uri.path).ok()).unwrap_or_default()
            },
            move |entries| cm(CloudMsg::Listed { dir: dir.clone(), account: uri.account.clone(), path: uri.path.clone(), entries }),
        )
    }

    fn cloud_cancel_signin(&mut self) {
        if let Some((account, s)) = self.cloud.signin.take() {
            s.cancel();
            // A half-made account leaves nothing behind.
            if self.cloud_account(&account).is_none()
                && let Some(sup) = self.cloud.sup.clone()
            {
                std::thread::spawn(move || {
                    if let Some(rc) = sup.running() {
                        let _ = drive::delete_remote(&rc, &account);
                    }
                });
            }
        }
    }

    /// Panes inside the account go home, so nothing holds the mount open.
    fn cloud_leave(&mut self, id: &str) -> Task<Message> {
        let root = paths::mount_point(id);
        let home = config::home();
        let show_hidden = self.show_hidden;
        let mut tasks = Vec::new();
        for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()) {
            pane.back.retain(|p| !p.starts_with(&root));
            pane.forward.retain(|p| !p.starts_with(&root));
            if pane.location.starts_with(&root) && !pane.special() {
                pane.location = home.clone();
                tasks.push(pane.load(home.clone(), show_hidden));
            }
        }
        Task::batch(tasks)
    }

    /// Delete key inside Drive: rclone moves the files to Google Drive's own trash.
    pub(crate) fn cloud_trash(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        let p2 = paths.clone();
        background(move || ef_core::ops::delete(&p2, &ef_core::ops::Progress::default()), move |errs| cm(CloudMsg::Trashed(paths.clone(), errs)))
    }

    /// "Delete permanently" inside Drive: skip Google's trash (rc, not FUSE).
    pub(crate) fn cloud_purge(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
        let p2 = paths.clone();
        background(
            move || {
                let Some(rc) = sup.running() else { return vec!["Google Drive isn't running.".to_string()] };
                p2.iter()
                    .filter_map(|p| {
                        let u = DriveUri::from_mount_path(p)?;
                        let dir = p.is_dir();
                        drive::delete_permanently(&rc, &u.account, &u.path, dir).err().map(|e| format!("{}: {}", u.name(), e.explain()))
                    })
                    .collect()
            },
            move |errs| cm(CloudMsg::Purged(paths.clone(), errs)),
        )
    }

    /// Copy inside one Drive: server-side, nothing downloaded or uploaded.
    pub(crate) fn cloud_copy_within(&mut self, sources: Vec<PathBuf>, dest: PathBuf) -> Task<Message> {
        let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
        let s2 = sources.clone();
        background(
            move || {
                let Some(rc) = sup.running() else { return vec!["Google Drive isn't running.".to_string()] };
                let Some(to) = DriveUri::from_mount_path(&dest) else { return vec!["That folder isn't on Google Drive.".to_string()] };
                s2.iter()
                    .filter_map(|p| {
                        let from = DriveUri::from_mount_path(p)?;
                        let mut name = from.name().to_string();
                        // "Copy of" like Drive itself when the copy lands next to the original.
                        if from.parent().as_ref() == Some(&to) {
                            name = format!("Copy of {name}");
                        }
                        drive::copy_within(&rc, &from.account, &from.path, &to.join(&name).path).err().map(|e| format!("{}: {}", from.name(), e.explain()))
                    })
                    .collect()
            },
            move |errs| cm(CloudMsg::CopiedWithin(sources.clone(), errs)),
        )
    }

    /// A saved Drive result whose account isn't mounted: offer to connect it.
    pub(crate) fn open_drive_uri(&mut self, p: &Path) -> Task<Message> {
        let Some(u) = DriveUri::parse(&p.to_string_lossy()) else { return Task::none() };
        let name = self.cloud_account(&u.account).map(|a| a.name.clone()).unwrap_or_else(|| "Google Drive".into());
        if self.cloud_account(&u.account).is_none() {
            self.toast_error(format!("“{}” is on a Google Drive that isn't connected", u.name()), "Connect it again from the Cloud section.".into());
            return Task::none();
        }
        self.push_toast(Tone::Accent, format!("{name} isn't connected"), Some(format!("“{}” opens once it's connected.", display_stem(u.name()).unwrap_or(u.name()))), Some(("Connect".into(), cm(CloudMsg::Click(u.account.clone())))));
        Task::none()
    }

    /// Both ends are on the same Drive account (a copy can stay on Google's side).
    pub(crate) fn same_cloud(sources: &[PathBuf], dest: &Path) -> bool {
        let Some(d) = DriveUri::from_mount_path(dest) else { return false };
        !sources.is_empty() && sources.iter().all(|s| DriveUri::from_mount_path(s).is_some_and(|u| u.account == d.account && !s.is_dir()))
    }

    /// Before quitting: pending uploads ask first; then mounts and rclone stop.
    pub(crate) fn cloud_before_exit(&mut self) -> bool {
        if self.cloud_uploads() > 0 && !matches!(self.cloud.dialog, Some(CloudDialog { step: Step::Quit, .. })) {
            self.cloud.dialog = Some(CloudDialog { step: Step::Quit, error: None, opened: Instant::now() });
            return false;
        }
        self.cloud_shutdown();
        true
    }

    pub(crate) fn cloud_shutdown(&mut self) {
        self.cloud_cancel_signin();
        if let Some(sup) = self.cloud.sup.take() {
            sup.stop();
        }
    }

    fn cloud_next_name(&self) -> String {
        let n = self.settings.cloud.accounts.len();
        if n == 0 { "Google Drive".into() } else { format!("Google Drive {}", n + 1) }
    }

    // ------------------------------------------------------------------ update

    pub(crate) fn cloud_update(&mut self, msg: CloudMsg) -> Task<Message> {
        match msg {
            CloudMsg::ClientDraft(_) | CloudMsg::SecretDraft(_) | CloudMsg::ToggleSecret | CloudMsg::ToggleAdvanced | CloudMsg::SaveClient | CloudMsg::UseRcloneApp | CloudMsg::ClientsRead(_) | CloudMsg::ClientChanged { .. } | CloudMsg::ChangeAccess(_) | CloudMsg::ScopeSet(..) | CloudMsg::CacheDraft(_) | CloudMsg::CacheSubmit | CloudMsg::CacheSize(_) | CloudMsg::ClearCache | CloudMsg::SetIndex(_) | CloudMsg::SetLive(_) | CloudMsg::OpenLog => self.cloud_settings_update(msg),
            CloudMsg::Checked(avail, fuse) => {
                self.cloud.checking = false;
                self.cloud.fuse = fuse;
                if let Availability::Ready(r) = &avail
                    && self.cloud.sup.as_ref().is_none_or(|s| s.rclone() != r)
                {
                    self.cloud.sup = Some(Arc::new(Supervisor::new(r.clone())));
                }
                let ready = matches!(avail, Availability::Ready(_));
                self.cloud.avail = Some(avail);
                if let Some(d) = &mut self.cloud.dialog {
                    match d.step {
                        Step::Missing if ready => d.step = Step::Start { access: CloudAccess::Full },
                        Step::Start { .. } if !ready => d.step = Step::Missing,
                        _ => {}
                    }
                }
                if !ready || !fuse {
                    return Task::none();
                }
                let ids: Vec<String> = self.settings.cloud.accounts.iter().map(|a| a.id.clone()).collect();
                Task::batch(ids.iter().map(|id| self.cloud_mount_task(id)).collect::<Vec<_>>())
            }
            CloudMsg::Recheck => {
                self.cloud.checking = true;
                check_task()
            }
            CloudMsg::OpenConnect => {
                self.menu = None;
                self.command = None;
                let step = match &self.cloud.avail {
                    Some(Availability::Ready(_)) => Step::Start { access: CloudAccess::Full },
                    Some(_) => Step::Missing,
                    None => {
                        self.cloud.checking = true;
                        self.cloud.dialog = Some(CloudDialog { step: Step::Start { access: CloudAccess::Full }, error: None, opened: Instant::now() });
                        return check_task();
                    }
                };
                self.cloud.dialog = Some(CloudDialog { step, error: None, opened: Instant::now() });
                Task::none()
            }
            CloudMsg::Close => {
                if let Some(d) = self.cloud.dialog.take()
                    && let Step::Waiting { again, .. } = d.step
                {
                    self.cloud_cancel_signin();
                    if again {
                        return self.cloud_restore_again();
                    }
                }
                Task::none()
            }
            CloudMsg::SetAccess(a) => {
                if let Some(CloudDialog { step: Step::Start { access }, .. }) = &mut self.cloud.dialog {
                    *access = a;
                }
                Task::none()
            }
            CloudMsg::StartSignIn => {
                let Some(CloudDialog { step: Step::Start { access }, .. }) = self.cloud.dialog.clone() else { return Task::none() };
                if !self.cloud.fuse {
                    if let Some(d) = &mut self.cloud.dialog {
                        d.error = Some("Google Drive can't show as a folder: FUSE isn't installed. Install the fuse3 package, then try again.".into());
                    }
                    return Task::none();
                }
                let (Some(sup), Some(rclone)) = (self.cloud.sup.clone(), self.cloud_rclone().cloned()) else {
                    if let Some(d) = &mut self.cloud.dialog {
                        d.step = Step::Missing;
                    }
                    return Task::none();
                };
                let account = paths::new_account_id();
                self.start_signin(sup, rclone, account, access, false)
            }
            CloudMsg::Started { account, again, result } => match result {
                Ok(Handle(s)) => {
                    if !matches!(&self.cloud.dialog, Some(CloudDialog { step: Step::Waiting { account: a, .. }, .. }) if a == &account) {
                        // Cancelled while rclone was starting.
                        s.cancel();
                        return Task::none();
                    }
                    self.cloud.signin = Some((account.clone(), s.clone()));
                    let s2 = s.clone();
                    let a2 = account.clone();
                    Task::batch([
                        background(move || s2.wait_url(SIGNIN_LIMIT), move |u| cm(CloudMsg::SignInUrl(a2.clone(), u))),
                        background(move || s.wait().map_err(|e| e.message), move |r| cm(CloudMsg::SignInDone { account: account.clone(), again, result: r })),
                    ])
                }
                Err(e) => {
                    if let Some(d) = &mut self.cloud.dialog {
                        d.step = Step::Start { access: CloudAccess::Full };
                        d.error = Some(e);
                    }
                    Task::none()
                }
            },
            CloudMsg::SignInUrl(account, u) => {
                if let Some(CloudDialog { step: Step::Waiting { account: a, url, .. }, .. }) = &mut self.cloud.dialog
                    && *a == account
                {
                    *url = u;
                }
                Task::none()
            }
            CloudMsg::CopyLink => match &self.cloud.dialog {
                Some(CloudDialog { step: Step::Waiting { url: Some(u), .. }, .. }) => iced::clipboard::write(u.clone()),
                _ => Task::none(),
            },
            CloudMsg::SignInDone { account, again, result } => {
                if self.cloud.signin.as_ref().is_some_and(|(a, _)| a == &account) {
                    self.cloud.signin = None;
                }
                let waiting = matches!(&self.cloud.dialog, Some(CloudDialog { step: Step::Waiting { account: a, .. }, .. }) if a == &account);
                if !waiting {
                    return Task::none();
                }
                match result {
                    Ok(()) => {
                        let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
                        let a2 = account.clone();
                        background(
                            move || {
                                let (rc, _) = sup.ensure().map_err(|e| e.explain())?;
                                match drive::about(&rc, &a2) {
                                    Ok(q) => Ok(q),
                                    // rclone read its config before the token was written: restart it.
                                    Err(_) => {
                                        sup.stop();
                                        let (rc, _) = sup.ensure().map_err(|e| e.explain())?;
                                        drive::about(&rc, &a2).map_err(|e| e.explain())
                                    }
                                }
                            },
                            move |r| cm(CloudMsg::Verified { account: account.clone(), again, result: r }),
                        )
                    }
                    Err(e) => {
                        let restore = if again { self.cloud_restore_again() } else {
                            self.cloud_drop_remote(&account);
                            Task::none()
                        };
                        if let Some(d) = &mut self.cloud.dialog {
                            // Signing in again can't start over from "choose access"; just say why.
                            if again {
                                d.step = Step::Missing;
                                self.cloud.dialog = None;
                                self.toast_error("Couldn't sign in to Google Drive".into(), e);
                            } else {
                                d.step = Step::Start { access: CloudAccess::Full };
                                d.error = Some(e);
                            }
                        }
                        restore
                    }
                }
            }
            CloudMsg::Verified { account, again, result } => match result {
                Ok(q) => {
                    self.cloud.dialog = None;
                    let st = self.cloud.status.entry(account.clone()).or_default();
                    st.quota = Some(q);
                    st.quota_at = Some(Instant::now());
                    st.signed_out = false;
                    if again {
                        self.push_toast(Tone::Accent, "Signed in to Google Drive again".into(), None, None);
                        return self.cloud_again_done(&account);
                    }
                    let access = self.cloud.pending_access.take().unwrap_or_default();
                    let name = self.cloud_next_name();
                    self.settings.cloud.accounts.push(CloudAccount::new(account.clone(), name.clone(), access));
                    self.cloud.open_after.insert(account.clone());
                    self.push_toast(Tone::Accent, format!("{name} connected"), None, None);
                    let m = self.cloud_mount_task(&account);
                    Task::batch([self.persist_settings(), m])
                }
                Err(e) => {
                    if again {
                        self.cloud.dialog = None;
                        self.toast_error("Couldn't sign in to Google Drive".into(), e);
                        return self.cloud_restore_again();
                    }
                    self.cloud_drop_remote(&account);
                    self.cloud.dialog = Some(CloudDialog { step: Step::Start { access: CloudAccess::Full }, error: Some(e), opened: Instant::now() });
                    Task::none()
                }
            },
            CloudMsg::Mounted(id, r) => {
                let st = self.cloud.status.entry(id.clone()).or_default();
                st.mounting = false;
                match r {
                    Ok(()) => {
                        st.mounted = true;
                        st.error = None;
                        st.offline = false;
                        st.net_failures = 0;
                        let mut tasks = vec![self.cloud_quota_task(&id), self.cloud_index_mounted(&id)];
                        if let Some(s) = self.cloud.status.get_mut(&id) {
                            s.quota_at = Some(Instant::now());
                        }
                        if self.cloud.open_after.remove(&id) {
                            tasks.push(self.go(paths::mount_point(&id), true));
                        } else if let Some((p, _)) = self.cloud.reopen.take() {
                            let pane = self.pane();
                            if DriveUri::from_mount_path(&p).is_some_and(|u| u.account == id) && pane.location == config::home() && pane.back.is_empty() {
                                tasks.push(self.go(p, true));
                            } else {
                                self.cloud.reopen = Some((p, Instant::now() - Duration::from_secs(1)));
                            }
                        }
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        let name = self.cloud_account(&id).map(|a| a.name.clone()).unwrap_or_else(|| "Google Drive".into());
                        let opening = self.cloud.open_after.remove(&id);
                        if let Some(st) = self.cloud.status.get_mut(&id) {
                            st.mounted = false;
                            st.error = Some(e.explain());
                            match e.kind {
                                ef_cloud::Kind::SignedOut => st.signed_out = true,
                                ef_cloud::Kind::Offline => st.offline = true,
                                _ => {}
                            }
                        }
                        if opening || e.kind != ef_cloud::Kind::Offline {
                            self.toast_error(format!("Couldn't open {name}"), e.explain());
                        }
                        Task::none()
                    }
                }
            }
            CloudMsg::Click(_) if matches!(self.cloud.avail, Some(Availability::Missing | Availability::TooOld { .. })) => {
                self.cloud.dialog = Some(CloudDialog { step: Step::Missing, error: None, opened: Instant::now() });
                Task::none()
            }
            CloudMsg::Click(id) => {
                if self.cloud.status.get(&id).is_some_and(|s| s.signed_out) {
                    return self.cloud_signin_again(&id);
                }
                if self.cloud.status.get(&id).is_some_and(|s| s.mounted) && drive::is_mount_point(&paths::mount_point(&id)) {
                    return self.go(paths::mount_point(&id), true);
                }
                self.cloud.open_after.insert(id.clone());
                if self.cloud.sup.is_none() {
                    self.cloud.checking = true;
                    return check_task();
                }
                self.cloud_mount_task(&id)
            }
            CloudMsg::OpenTab(id) => Task::done(Message::OpenTab(paths::mount_point(&id))),
            CloudMsg::Menu(id, at) => {
                self.open_menu(crate::overlay::MenuFor::Cloud(id), at);
                Task::none()
            }
            CloudMsg::OpenUrl(u) => {
                let _ = std::process::Command::new("xdg-open").arg(&u).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                Task::none()
            }
            CloudMsg::Refresh(id) => {
                let root = paths::mount_point(&id);
                self.cloud.kinds_asked.retain(|p, _| !p.starts_with(&root));
                let dirs: Vec<PathBuf> = self.tabs.iter().flat_map(|t| t.panes.iter()).filter(|p| !p.special() && p.location.starts_with(&root)).map(|p| p.location.clone()).collect();
                let mut tasks: Vec<Task<Message>> = dirs.iter().map(|d| self.refresh(d)).collect();
                if let Some(sup) = self.cloud.sup.clone() {
                    let fs = paths::fs(&id);
                    // Forget rclone's cached folder listings so the refresh asks Google.
                    tasks.push(background(move || { if let Some(rc) = sup.running() { let _ = rc.call("vfs/refresh", &serde_json::json!({ "fs": fs, "recursive": false }), Duration::from_secs(30)); } }, |_| Message::Tick));
                }
                tasks.push(self.cloud_quota_task(&id));
                Task::batch(tasks)
            }
            CloudMsg::Disconnect(id) => {
                let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
                let leave = self.cloud_leave(&id);
                let i2 = id.clone();
                let un = background(move || match sup.running() { Some(rc) => drive::unmount(&rc, &i2), None => Ok(()) }, move |r| cm(CloudMsg::Disconnected(id.clone(), r)));
                Task::batch([leave, un])
            }
            CloudMsg::Disconnected(id, r) => {
                match r {
                    Ok(()) => {
                        if let Some(st) = self.cloud.status.get_mut(&id) {
                            st.mounted = false;
                            st.uploads = Uploads::default();
                        }
                    }
                    Err(_) if drive::is_mount_point(&paths::mount_point(&id)) => {
                        self.cloud.dialog = Some(CloudDialog { step: Step::Busy { account: id }, error: None, opened: Instant::now() });
                    }
                    Err(_) => {
                        if let Some(st) = self.cloud.status.get_mut(&id) {
                            st.mounted = false;
                        }
                    }
                }
                Task::none()
            }
            CloudMsg::DisconnectAnyway(id) => {
                self.cloud.dialog = None;
                let i2 = id.clone();
                background(move || drive::unmount_lazy(&i2), move |r| cm(CloudMsg::Disconnected(id.clone(), r)))
            }
            CloudMsg::AskSignOut(id) => {
                self.menu = None;
                self.cloud.dialog = Some(CloudDialog { step: Step::SignOut { account: id }, error: None, opened: Instant::now() });
                Task::none()
            }
            CloudMsg::SignOut(id) => {
                self.cloud.dialog = None;
                let place = self.cloud_account(&id).is_some_and(|a| a.under.is_some());
                let name = self.cloud_account(&id).map(|a| a.name.clone()).unwrap_or_default();
                // An account takes its shared places with it.
                let ids: Vec<String> = std::iter::once(id.clone()).chain(self.settings.cloud.accounts.iter().filter(|a| a.under.as_deref() == Some(&id)).map(|a| a.id.clone())).collect();
                let mut tasks = Vec::new();
                for i in &ids {
                    tasks.push(self.cloud_leave(i));
                    self.cloud.status.remove(i);
                    self.cloud.index.folders.remove(i);
                    self.cloud.index.runs.remove(i);
                }
                self.settings.cloud.accounts.retain(|a| !ids.contains(&a.id));
                tasks.push(self.persist_settings());
                let sup = self.cloud.sup.clone();
                let last = self.settings.cloud.accounts.is_empty();
                tasks.push(background(
                    move || {
                        for i2 in &ids {
                            if let Some(rc) = sup.as_ref().and_then(|s| s.running()) {
                                let _ = drive::unmount(&rc, i2);
                                let _ = drive::delete_remote(&rc, i2);
                            }
                            let _ = drive::unmount_lazy(i2);
                            let _ = std::fs::remove_dir(paths::mount_point(i2));
                            ef_cloud::catalog::remove(i2);
                            let _ = std::fs::remove_file(ef_cloud::changes::page_file(i2));
                            // Cached file contents for this account (rclone keeps them per remote).
                            for sub in ["vfs", "vfsMeta"] {
                                let _ = std::fs::remove_dir_all(config::cloud_cache_dir().join(sub).join(paths::remote(i2)));
                            }
                        }
                        if last && let Some(s) = &sup {
                            s.stop();
                        }
                    },
                    move |_| if place { cm(CloudMsg::PlaceRemoved(name.clone())) } else { cm(CloudMsg::SignedOut) },
                ));
                if last {
                    self.cloud.sup = None;
                }
                Task::batch(tasks)
            }
            CloudMsg::PlaceRemoved(name) => {
                self.push_toast(Tone::Accent, format!("Removed {name} from the sidebar"), None, None);
                Task::none()
            }
            CloudMsg::ListDrives(id) => {
                self.menu = None;
                let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
                self.cloud.dialog = Some(CloudDialog { step: Step::Drives { account: id.clone(), drives: None }, error: None, opened: Instant::now() });
                let i2 = id.clone();
                background(move || sup.running().ok_or_else(|| "Google Drive isn't running.".to_string()).and_then(|rc| drive::shared_drives(&rc, &i2).map_err(|e| e.explain())), move |r| cm(CloudMsg::DrivesListed(id.clone(), r)))
            }
            CloudMsg::DrivesListed(id, r) => {
                if let Some(CloudDialog { step: Step::Drives { account, drives }, error, .. }) = &mut self.cloud.dialog
                    && *account == id
                {
                    match r {
                        Ok(d) => *drives = Some(d),
                        Err(e) => *error = Some(e),
                    }
                }
                Task::none()
            }
            CloudMsg::AddPlace(parent, place) => {
                self.menu = None;
                self.cloud.dialog = None;
                let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
                let Some(p) = self.cloud_account(&parent).cloned() else { return Task::none() };
                let (cfg_place, name, dplace) = match &place {
                    PlaceChoice::Shared => (config::CloudPlace::Shared, format!("Shared with me · {}", p.name), drive::Place::SharedWithMe),
                    PlaceChoice::Team(id, n) => (config::CloudPlace::Team { drive: id.clone() }, n.clone(), drive::Place::SharedDrive(id.clone())),
                };
                if self.settings.cloud.accounts.iter().any(|a| a.under.as_deref() == Some(&parent) && a.place.as_ref() == Some(&cfg_place)) {
                    return Task::none();
                }
                let id = paths::new_account_id();
                let access = if matches!(cfg_place, config::CloudPlace::Shared) { CloudAccess::Read } else { p.access };
                let account = CloudAccount { under: Some(parent.clone()), place: Some(cfg_place), ..CloudAccount::new(id.clone(), name, access) };
                let i2 = id.clone();
                background(
                    move || {
                        let (rc, _) = sup.ensure().map_err(|e| e.explain())?;
                        drive::create_place(&rc, &parent, &i2, &dplace).map_err(|e| e.explain())
                    },
                    move |r| cm(CloudMsg::PlaceAdded(account.clone(), r)),
                )
            }
            CloudMsg::PlaceAdded(account, r) => match r {
                Ok(()) => {
                    let id = account.id.clone();
                    // Keep shared places right under their account in the sidebar.
                    let at = self.settings.cloud.accounts.iter().rposition(|a| a.id == account.under.clone().unwrap_or_default() || a.under == account.under).map_or(self.settings.cloud.accounts.len(), |i| i + 1);
                    self.settings.cloud.accounts.insert(at, account);
                    self.cloud.open_after.insert(id.clone());
                    let m = self.cloud_mount_task(&id);
                    Task::batch([self.persist_settings(), m])
                }
                Err(e) => {
                    self.toast_error("Couldn't add it to the sidebar".into(), e);
                    Task::none()
                }
            },
            CloudMsg::PlaceIndexed(id, on) => {
                if let Some(a) = self.settings.cloud.accounts.iter_mut().find(|a| a.id == id) {
                    a.indexed = on;
                }
                let save = self.persist_settings();
                if !on {
                    ef_cloud::catalog::remove(&id);
                    self.cloud.index.runs.remove(&id);
                    return save;
                }
                let crawl = if self.settings.cloud.index { self.cloud_crawl(&id, false) } else { Task::none() };
                Task::batch([save, crawl])
            }
            CloudMsg::SetFulltext(on) => {
                self.settings.cloud.fulltext = on;
                self.persist_settings()
            }
            CloudMsg::SignedOut => {
                self.push_toast(Tone::Accent, "Signed out of Google Drive".into(), Some("rclone still has access until you remove it in your Google account.".into()), Some(("Open Google's account permissions".into(), cm(CloudMsg::OpenUrl(PERMISSIONS_URL.into())))));
                Task::none()
            }
            CloudMsg::AskRename(id) => {
                self.menu = None;
                let text = self.cloud_account(&id).map(|a| a.name.clone()).unwrap_or_default();
                self.cloud.dialog = Some(CloudDialog { step: Step::Rename { account: id, text }, error: None, opened: Instant::now() });
                Task::batch([iced::widget::operation::focus(RENAME_ID), iced::widget::operation::select_all(RENAME_ID)])
            }
            CloudMsg::RenameDraft(t) => {
                if let Some(CloudDialog { step: Step::Rename { text, .. }, error, .. }) = &mut self.cloud.dialog {
                    *text = t;
                    *error = None;
                }
                Task::none()
            }
            CloudMsg::RenameSubmit => {
                let Some(CloudDialog { step: Step::Rename { account, text }, .. }) = self.cloud.dialog.clone() else { return Task::none() };
                let name = text.trim().to_string();
                if name.is_empty() {
                    if let Some(d) = &mut self.cloud.dialog {
                        d.error = Some("Type a name.".into());
                    }
                    return Task::none();
                }
                self.cloud.dialog = None;
                if let Some(a) = self.settings.cloud.accounts.iter_mut().find(|a| a.id == account) {
                    a.name = name;
                }
                self.persist_settings()
            }
            CloudMsg::Quota(id, r) => {
                if let Some(st) = self.cloud.status.get_mut(&id) {
                    match r {
                        Ok(q) => {
                            st.quota = Some(q);
                            st.offline = false;
                            st.net_failures = 0;
                            st.signed_out = false;
                        }
                        Err(e) => self.cloud_failed(&id, &e),
                    }
                }
                Task::none()
            }
            CloudMsg::Polled(v) => {
                self.cloud.polling = false;
                let mut finished = false;
                for (id, r) in v {
                    let Some(st) = self.cloud.status.get_mut(&id) else { continue };
                    match r {
                        Ok(u) => {
                            if st.uploads.pending() > 0 && u.pending() == 0 {
                                finished = true;
                            }
                            st.uploads = u;
                        }
                        Err(e) if e.kind == ef_cloud::Kind::NotRunning => st.mounted = false,
                        Err(_) => {}
                    }
                }
                if finished && self.cloud_uploads() == 0 {
                    self.cloud.uploaded_at = Some(Instant::now());
                    if matches!(self.cloud.dialog, Some(CloudDialog { step: Step::Quit, error: Some(_), .. })) {
                        // "Wait for uploads" was chosen: quit now that they're done.
                        self.cloud.dialog = None;
                        return self.request_exit();
                    }
                }
                Task::none()
            }
            CloudMsg::Health(h, lost) => {
                let mut tasks = Vec::new();
                match &h {
                    Health::GaveUp(why) if !self.cloud.gave_up_shown => {
                        self.cloud.gave_up_shown = true;
                        self.push_toast(Tone::Danger, "Google Drive stopped responding".into(), Some(why.clone()), Some(("Try again".into(), cm(CloudMsg::TryAgain))));
                    }
                    Health::Running => self.cloud.gave_up_shown = false,
                    _ => {}
                }
                for id in lost {
                    if self.cloud.status.get(&id).is_some_and(|s| s.mounted && !s.mounting) {
                        // Was mounted; rclone restarted or the mount went away: put it back.
                        if let Some(s) = self.cloud.status.get_mut(&id) {
                            s.mounted = false;
                        }
                        tasks.push(self.cloud_mount_task(&id));
                    }
                }
                Task::batch(tasks)
            }
            CloudMsg::TryAgain => {
                if let Some(s) = &self.cloud.sup {
                    s.reset();
                }
                self.cloud.gave_up_shown = false;
                let ids: Vec<String> = self.settings.cloud.accounts.iter().map(|a| a.id.clone()).collect();
                Task::batch(ids.iter().map(|id| self.cloud_mount_task(id)).collect::<Vec<_>>())
            }
            CloudMsg::Listed { dir, account, path, entries } => {
                if !entries.is_empty() {
                    crate::kinds::set_google(dir, drive::google_kinds(&entries));
                    self.cloud_folder_listed(&account, &path, &entries);
                }
                Task::none()
            }
            CloudMsg::RefreshIndex => {
                let ids: Vec<String> = self.settings.cloud.accounts.iter().filter(|a| a.in_index()).map(|a| a.id.clone()).collect();
                Task::batch(ids.iter().map(|id| self.cloud_crawl(id, false)).collect::<Vec<_>>())
            }
            CloudMsg::Crawled { account, dirs_only, result } => self.cloud_crawled(account, dirs_only, result),
            CloudMsg::LiveDue { pane, generation } => self.cloud_live_due(pane, generation),
            CloudMsg::LiveResults { pane, generation, result } => self.cloud_live_results(pane, generation, result),
            CloudMsg::LiveChecked { pane, generation, gone } => self.cloud_live_checked(pane, generation, gone),
            CloudMsg::Changes(account, r) => {
                self.cloud.changes_busy = false;
                if let Ok(Some(changes)) = r {
                    self.cloud_apply_changes(&account, changes);
                }
                Task::none()
            }
            CloudMsg::Untrash(paths) => {
                let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
                let p2 = paths.clone();
                background(
                    move || {
                        let rc = sup.running().ok_or("Google Drive isn't running.")?;
                        let mut dirs: Vec<DriveUri> = p2.iter().filter_map(|p| DriveUri::from_mount_path(p)?.parent()).collect();
                        dirs.dedup();
                        let mut total = (0, 0);
                        for d in dirs {
                            let (n, e) = drive::untrash(&rc, &d.account, &d.path).map_err(|e| e.explain())?;
                            total.0 += n;
                            total.1 += e;
                        }
                        Ok(total)
                    },
                    move |r| cm(CloudMsg::Untrashed(paths.clone(), r)),
                )
            }
            CloudMsg::Untrashed(paths, r) => {
                match r {
                    Ok((n, errs)) => {
                        let body = (n as usize > paths.len()).then(|| "Google Drive puts back everything trashed in that folder, so earlier deletions came back too.".to_string());
                        self.push_toast(Tone::Accent, format!("Restored {n} {} from Google Drive's trash", if n == 1 { "item" } else { "items" }), body, None);
                        if errs > 0 {
                            self.toast_error("Some items couldn't be restored".into(), format!("{errs} stayed in Google Drive's trash. Restore them from drive.google.com."));
                        }
                    }
                    Err(e) => self.toast_error("Couldn't restore from Google Drive's trash".into(), e),
                }
                self.cloud_refresh_parents(&paths)
            }
            CloudMsg::Folders(account, f) => {
                self.cloud.index.folders.insert(account, f);
                Task::none()
            }
            CloudMsg::Trashed(paths, errs) => {
                if errs.is_empty() {
                    self.cloud_index_removed(&paths);
                }
                if errs.is_empty() {
                    let n = paths.len();
                    // Undo puts back from Drive's trash (everything trashed in that folder).
                    self.push_toast(Tone::Accent, if n == 1 { "Moved to Google Drive's trash".into() } else { format!("Moved {n} items to Google Drive's trash") }, Some("It's in Google Drive's trash on drive.google.com.".into()), Some(("Undo".into(), cm(CloudMsg::Untrash(paths.clone())))));
                    let _ = TRASH_URL;
                } else {
                    self.toast_error("Couldn't move to Google Drive's trash".into(), errs.join("\n"));
                }
                self.cloud_refresh_parents(&paths)
            }
            CloudMsg::Purged(paths, errs) => {
                if errs.is_empty() {
                    self.cloud_index_removed(&paths);
                }
                if !errs.is_empty() {
                    self.toast_error("Couldn't delete from Google Drive".into(), errs.join("\n"));
                }
                self.cloud_refresh_parents(&paths)
            }
            CloudMsg::CopiedWithin(paths, errs) => {
                if !errs.is_empty() {
                    self.toast_error("Couldn't copy on Google Drive".into(), errs.join("\n"));
                }
                let dirs: Vec<PathBuf> = self.tabs.iter().flat_map(|t| t.panes.iter()).filter(|p| !p.special() && ef_config::is_cloud_path(&p.location)).map(|p| p.location.clone()).collect();
                let _ = paths;
                Task::batch(dirs.iter().map(|d| self.refresh(d)).collect::<Vec<_>>())
            }
            CloudMsg::QuitWait => {
                // Stay open; the dialog closes and quitting happens when uploads finish.
                if let Some(d) = &mut self.cloud.dialog {
                    d.error = Some("Waiting for uploads… EchoFiles quits when they're done.".into());
                }
                Task::none()
            }
            CloudMsg::QuitAnyway => {
                self.cloud.dialog = None;
                // rclone keeps pending uploads in its cache and sends them next time.
                for st in self.cloud.status.values_mut() {
                    st.uploads = Uploads::default();
                }
                self.request_exit()
            }
            CloudMsg::CopyText(t) => iced::clipboard::write(t),
            CloudMsg::DownloadAs(path, format) => {
                let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
                let Some(u) = DriveUri::from_mount_path(&path) else { return Task::none() };
                let stem = display_stem(u.name()).unwrap_or(u.name()).to_string();
                let to_dir = config::home().join("Downloads");
                let to_name = format!("{stem}.{format}");
                let label = to_name.clone();
                background(
                    move || {
                        let _ = std::fs::create_dir_all(&to_dir);
                        let rc = sup.running().ok_or("Google Drive isn't running.")?;
                        drive::export(&rc, &u.account, &u.path, format, &to_dir, &to_name).map_err(|e| e.explain())?;
                        Ok(to_dir.join(&to_name))
                    },
                    move |r| cm(CloudMsg::Downloaded(label.clone(), r)),
                )
            }
            CloudMsg::Downloaded(name, r) => {
                match r {
                    Ok(p) => {
                        let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
                        self.push_toast(Tone::Accent, format!("Downloaded “{name}”"), Some("Saved in Downloads.".into()), Some(("Show in folder".into(), Message::Navigate(dir))));
                    }
                    Err(e) => self.toast_error(format!("Couldn't download “{name}”"), e),
                }
                Task::none()
            }
        }
    }

    fn cloud_drop_remote(&self, account: &str) {
        if let Some(sup) = self.cloud.sup.clone() {
            let account = account.to_string();
            std::thread::spawn(move || {
                if let Some(rc) = sup.running() {
                    let _ = drive::delete_remote(&rc, &account);
                }
            });
        }
    }

    fn cloud_refresh_parents(&mut self, paths: &[PathBuf]) -> Task<Message> {
        let mut dirs: Vec<PathBuf> = paths.iter().filter_map(|p| p.parent().map(Path::to_path_buf)).collect();
        dirs.dedup();
        Task::batch(dirs.iter().map(|d| self.refresh(d)).collect::<Vec<_>>())
    }

    /// Network errors twice in a row mark the account offline; a revoked token asks to sign in.
    fn cloud_failed(&mut self, id: &str, e: &ef_cloud::Error) {
        let Some(st) = self.cloud.status.get_mut(id) else { return };
        match e.kind {
            ef_cloud::Kind::Offline | ef_cloud::Kind::Timeout => {
                st.net_failures = st.net_failures.saturating_add(1);
                if st.net_failures >= 2 {
                    st.offline = true;
                }
            }
            ef_cloud::Kind::SignedOut => st.signed_out = true,
            _ => {}
        }
    }

    /// Create the remote (unless signing in again) and start rclone's browser sign-in.
    pub(crate) fn start_signin(&mut self, sup: Arc<Supervisor>, rclone: ef_cloud::Rclone, account: String, access: CloudAccess, again: bool) -> Task<Message> {
        self.cloud.pending_access = Some(access);
        self.cloud.dialog = Some(CloudDialog { step: Step::Waiting { account: account.clone(), url: None, again, started: Instant::now() }, error: None, opened: Instant::now() });
        let a2 = account.clone();
        background(
            move || {
                if !again {
                    let (rc, _) = sup.ensure().map_err(|e| e.explain())?;
                    drive::create(&rc, &a2, access.into(), None).map_err(|e| e.explain())?;
                }
                SignIn::start(&rclone, &a2).map(Handle).map_err(|e| e.explain())
            },
            move |r| cm(CloudMsg::Started { account: account.clone(), again, result: r }),
        )
    }

    /// "Sign in again" for an existing account (revoked token, new client ID, new access).
    pub(crate) fn cloud_signin_again(&mut self, id: &str) -> Task<Message> {
        // A shared place uses its account's sign-in: sign that in again instead.
        if let Some(parent) = self.cloud_account(id).and_then(|a| a.under.clone()) {
            return self.cloud_signin_again(&parent);
        }
        let (Some(sup), Some(rclone), Some(a)) = (self.cloud.sup.clone(), self.cloud_rclone().cloned(), self.cloud_account(id).cloned()) else {
            self.cloud.dialog = Some(CloudDialog { step: Step::Missing, error: None, opened: Instant::now() });
            return Task::none();
        };
        self.start_signin(sup, rclone, a.id, a.access, true)
    }

    // ------------------------------------------------------------------ sidebar

    /// The Cloud section (design system `Sidebar` › Cloud, `CloudItem`).
    pub(crate) fn cloud_section(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let open = self.settings.sidebar.cloud_open;
        let add = self.tip(w::icon_button(p, &self.icons, "plus", Some(cm(CloudMsg::OpenConnect)), false), "Connect Google Drive", None);
        let mut col = column![self.fold_head("Cloud", color(p.world_cloud), None, open, crate::view::Fold::Cloud, Some(add))].spacing(1);
        if self.settings.cloud.accounts.is_empty() {
            if open {
                col = col.push(self.side_button("plus", "Add Google Drive", cm(CloudMsg::OpenConnect), self.cloud.dialog.is_some()));
            }
            return col.into();
        }
        for a in self.settings.cloud.accounts.iter().filter(|a| a.under.is_none()) {
            if open || self.cloud_active(&a.id) {
                col = col.push(self.cloud_item(a));
            }
            for c in self.settings.cloud.accounts.iter().filter(|c| c.under.as_deref() == Some(&a.id)) {
                if open || self.cloud_active(&c.id) {
                    col = col.push(container(self.cloud_item(c)).padding(iced::Padding { left: 16.0, ..Default::default() }));
                }
            }
        }
        col.into()
    }

    fn cloud_item<'a>(&'a self, a: &'a CloudAccount) -> Element<'a, Message> {
        let p = &self.palette;
        let st = self.cloud.status.get(&a.id).cloned().unwrap_or_default();
        let active = self.cloud_active(&a.id);
        let on = st.mounted && !st.offline && !st.signed_out;
        let ink = if active { color(p.ink_strong) } else if on { color(p.ink) } else { color(p.ink_muted) };
        let head = text(a.name.clone()).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(ink).wrapping(text::Wrapping::None);
        let mut lines = column![head].spacing(3).width(Length::Fill).clip(true);
        if on && let Some((used, total)) = st.quota.filter(|q| q.1 > 0) {
            lines = lines.push(w::usage_bar(p, used as f32 / total as f32, p.world_cloud));
        }
        let no_rclone = matches!(self.cloud.avail, Some(Availability::Missing | Availability::TooOld { .. }));
        let pill: Option<Element<'_, Message>> = if no_rclone {
            Some(w::pill(p, "Needs rclone", Some(p.warning)))
        } else if st.mounting {
            Some(w::pill(p, "Connecting…", Some(p.info)))
        } else if st.signed_out {
            Some(w::pill(p, "Sign in again", Some(p.warning)))
        } else if st.offline {
            Some(w::pill(p, "Offline", None))
        } else if st.uploads.pending() > 0 {
            Some(w::pill(p, &format!("Uploading {}", st.uploads.pending()), Some(p.info)))
        } else if !self.cloud.fuse && self.cloud.avail.is_some() {
            Some(w::pill(p, "Not mounted", None))
        } else {
            None
        };
        let meta: Element<'_, Message> = match pill {
            Some(pl) => pl,
            None => {
                let t = match st.quota {
                    Some((u, t)) if on => usage_text(u, t),
                    _ if a.access == CloudAccess::Read => "Read-only · click to open".into(),
                    _ => "Click to open".into(),
                };
                text(t).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None).into()
            }
        };
        lines = lines.push(meta);
        let tint = if active { color(p.accent_ink) } else if on { color(p.world_cloud) } else { color(p.ink_muted) };
        let glyph = match a.place {
            Some(config::CloudPlace::Shared) => "share",
            Some(config::CloudPlace::Team { .. }) => "folder",
            None => "cloud",
        };
        let r = row![container(w::glyph(&self.icons, glyph, 16.0, tint)).padding([2, 0]), lines].spacing(style::SPACE_4).align_y(Alignment::Start);
        let body = w::row_button(p, container(r).padding(iced::Padding { top: 6.0, bottom: 6.0, left: 0.0, right: 0.0 }).into(), active, Some(cm(CloudMsg::Click(a.id.clone()))));
        let access = if a.access == CloudAccess::Read { "Read-only" } else { "Full access" };
        let tip = match (&st.quota, &st.error) {
            (_, Some(e)) if !st.mounted => format!("{} · {access} · {e}", a.name),
            (Some((u, t)), _) => format!("{} · {access} · {}", a.name, usage_text(*u, *t)),
            _ => format!("{} · {access}", a.name),
        };
        let item = self.tip(body, &tip, None);
        let id = a.id.clone();
        w::context_area(item, move |at| cm(CloudMsg::Menu(id.clone(), at)))
    }

    /// Right-click menu for a Drive account.
    pub(crate) fn cloud_menu(&self, id: &str) -> Vec<crate::overlay::Entry> {
        use crate::overlay::{item, Entry};
        let Some(a) = self.cloud_account(id) else { return Vec::new() };
        let mounted = self.cloud.status.get(id).is_some_and(|s| s.mounted);
        let mut e = vec![
            item("external", "Open", None, cm(CloudMsg::Click(a.id.clone()))),
            item("plus", "Open in new tab", None, cm(CloudMsg::OpenTab(a.id.clone()))),
            item("cloud", "Open in browser", None, cm(CloudMsg::OpenUrl(DRIVE_URL.into()))),
        ];
        if mounted {
            e.push(item("refresh", "Refresh", None, cm(CloudMsg::Refresh(a.id.clone()))));
        }
        e.push(Entry::Sep);
        e.push(item("rename", "Rename…", None, cm(CloudMsg::AskRename(a.id.clone()))));
        if mounted {
            e.push(item("arrow-up", "Disconnect", None, cm(CloudMsg::Disconnect(a.id.clone()))));
        }
        if a.under.is_some() {
            e.push(item("close", "Remove from sidebar", None, cm(CloudMsg::SignOut(a.id.clone()))));
        } else {
            let has_shared = self.settings.cloud.accounts.iter().any(|c| c.under.as_deref() == Some(&a.id) && matches!(c.place, Some(config::CloudPlace::Shared)));
            e.push(Entry::Sep);
            if !has_shared {
                e.push(item("share", "Show Shared with me", None, cm(CloudMsg::AddPlace(a.id.clone(), PlaceChoice::Shared))));
            }
            e.push(item("folder", "Add a shared drive…", None, cm(CloudMsg::ListDrives(a.id.clone()))));
            e.push(Entry::Sep);
            e.push(item("close", "Sign out…", None, cm(CloudMsg::AskSignOut(a.id.clone()))));
        }
        e.push(Entry::Sep);
        e.push(item("settings", "Google Drive settings", None, Message::Settings(crate::settings::SettingsMsg::OpenPage(crate::settings::Page::Cloud))));
        e
    }

    // ------------------------------------------------------------------ dialogs

    /// Connect Google Drive and the other Drive dialogs (design system `CloudConnectDialog`).
    pub(crate) fn cloud_layer(&self) -> Option<Element<'_, Message>> {
        let d = self.cloud.dialog.as_ref()?;
        let p = &self.palette;
        let note = |s: &str| -> Element<'_, Message> { text(s.to_string()).size(style::BODY).font(style::FONT).color(color(p.ink_muted)).into() };
        let error_box = |e: &str| -> Element<'_, Message> {
            let pal = p.clone();
            container(row![w::glyph(&self.icons, "error", 14.0, color(p.danger.ink)), text(e.to_string()).size(style::META).font(style::FONT).color(color(p.ink))].spacing(6).align_y(Alignment::Center))
                .padding([8, 10])
                .width(Length::Fill)
                .style(move |_| container::Style { background: Some(Background::Color(color(pal.danger.soft))), border: Border { color: color(pal.danger.base), width: 1.0, radius: 2.0.into() }, ..Default::default() })
                .into()
        };
        let cancel = |label: &str| w::text_button(p, &self.icons, label, None, Some("Esc"), Variant::Ghost, Some(cm(CloudMsg::Close)));
        let (icon, title, body, foot, danger): (&'static str, String, Element<'_, Message>, Vec<Element<'_, Message>>, bool) = match &d.step {
            Step::Start { access } => {
                let opts = [("Full", cm(CloudMsg::SetAccess(CloudAccess::Full))), ("Read-only", cm(CloudMsg::SetAccess(CloudAccess::Read)))];
                let what = if *access == CloudAccess::Full { "Open, change, add and delete files" } else { "Open and copy files; nothing changes on Drive" };
                let mut b = column![
                    note("You sign in on Google's page in your browser, through rclone. EchoFiles never sees your Google password."),
                    row![w::segmented(p, &opts, usize::from(*access == CloudAccess::Read)), text(what).size(style::META).font(style::FONT).color(color(p.ink_muted))].spacing(style::SPACE_4).align_y(Alignment::Center),
                ]
                .spacing(style::SPACE_4);
                if self.cloud.checking {
                    b = b.push(note("Looking for rclone…"));
                }
                if let Some(e) = &d.error {
                    b = b.push(error_box(e));
                }
                let can = !self.cloud.checking;
                ("cloud", "Connect Google Drive".into(), b.into(), vec![cancel("Cancel"), w::text_button(p, &self.icons, "Sign in with browser", Some("external"), Some("Enter"), Variant::Primary, can.then_some(cm(CloudMsg::StartSignIn)))], false)
            }
            Step::Waiting { url, again, .. } => {
                let mut b = column![note("Google's sign-in page opened in your browser. Sign in there and allow rclone to use Google Drive. If no browser opened, copy the link and open it yourself.")].spacing(style::SPACE_4);
                if let Some(e) = &d.error {
                    b = b.push(error_box(e));
                }
                let copy = w::text_button(p, &self.icons, "Copy sign-in link", Some("copy"), None, Variant::Secondary, url.is_some().then_some(cm(CloudMsg::CopyLink)));
                ("cloud", if *again { "Sign in again for your app".into() } else { "Finish signing in in your browser".into() }, b.into(), vec![copy, cancel("Cancel")], false)
            }
            Step::Missing => {
                let (msg, cmd) = match &self.cloud.avail {
                    Some(Availability::TooOld { found, .. }) => (format!("EchoFiles needs rclone {} or newer. This computer has {found}.", ef_cloud::rclone::MIN_VERSION), ef_cloud::rclone::install_command()),
                    _ => ("EchoFiles reaches Google Drive through rclone, which isn't installed. Install it, then check again.".to_string(), ef_cloud::rclone::install_command()),
                };
                let pal = p.clone();
                let well = container(row![text(cmd.clone()).size(style::BODY).font(style::FONT).color(color(p.ink)).width(Length::Fill), self.tip(w::icon_button(p, &self.icons, "copy", Some(cm(CloudMsg::CopyText(cmd.clone()))), false), "Copy command", None)].align_y(Alignment::Center))
                    .padding([8, 12])
                    .width(Length::Fill)
                    .style(move |_| container::Style { background: Some(Background::Color(color(pal.bg_deep))), border: Border { radius: 2.0.into(), ..Default::default() }, ..Default::default() });
                let b = column![note(&msg), well].spacing(style::SPACE_4);
                ("cloud", "Google Drive needs rclone".into(), b.into(), vec![cancel("Close"), w::text_button(p, &self.icons, if self.cloud.checking { "Checking…" } else { "Check again" }, Some("refresh"), None, Variant::Primary, (!self.cloud.checking).then_some(cm(CloudMsg::Recheck)))], false)
            }
            Step::Rename { text: t, .. } => {
                let input = text_input("Google Drive", t).id(RENAME_ID).on_input(|v| cm(CloudMsg::RenameDraft(v))).on_submit(cm(CloudMsg::RenameSubmit)).size(style::BODY).font(style::FONT).padding([6, 10]).style(w::field_style(p, d.error.is_some()));
                let mut b = column![note("The name shown in the sidebar, tabs and search results."), input].spacing(style::SPACE_4);
                if let Some(e) = &d.error {
                    b = b.push(error_box(e));
                }
                ("cloud", "Rename Google Drive".into(), b.into(), vec![cancel("Cancel"), w::text_button(p, &self.icons, "Rename", Some("rename"), Some("Enter"), Variant::Primary, Some(cm(CloudMsg::RenameSubmit)))], false)
            }
            Step::SignOut { account } => {
                let up = self.cloud.status.get(account).map_or(0, |s| s.uploads.pending());
                let mut b = column![note("EchoFiles will stop using this account and delete its saved index and cached files from this computer. Files on Google Drive aren't touched.")].spacing(style::SPACE_4);
                if up > 0 {
                    b = b.push(error_box(&format!("{up} {} still uploading. Signing out now loses those changes.", if up == 1 { "file is" } else { "files are" })));
                }
                let name = self.cloud_account(account).map(|a| a.name.clone()).unwrap_or_else(|| "Google Drive".into());
                ("cloud", format!("Sign out of {name}?"), b.into(), vec![cancel("Cancel"), w::text_button(p, &self.icons, if up > 0 { "Sign out anyway" } else { "Sign out" }, None, None, Variant::Danger, Some(cm(CloudMsg::SignOut(account.clone()))))], true)
            }
            Step::Busy { account } => {
                let name = self.cloud_account(account).map(|a| a.name.clone()).unwrap_or_else(|| "Google Drive".into());
                let b = column![note(&format!("Files from {name} are open in other apps. Close them, or disconnect anyway."))].spacing(style::SPACE_4);
                ("cloud", format!("Couldn't disconnect {name}"), b.into(), vec![cancel("Cancel"), w::text_button(p, &self.icons, "Disconnect anyway", None, None, Variant::Danger, Some(cm(CloudMsg::DisconnectAnyway(account.clone()))))], false)
            }
            Step::Drives { account, drives } => {
                let mut b = column![note("A shared drive shows under its account in the sidebar, with the same sign-in.")].spacing(style::SPACE_4);
                match drives {
                    None if d.error.is_none() => b = b.push(note("Asking Google Drive…")),
                    Some(v) if v.is_empty() => b = b.push(note("This account has no shared drives.")),
                    Some(v) => {
                        let mut list = column![].spacing(2);
                        for (id, name) in v {
                            let added = self.settings.cloud.accounts.iter().any(|a| a.under.as_deref() == Some(account) && a.place == Some(config::CloudPlace::Team { drive: id.clone() }));
                            let r = row![w::glyph(&self.icons, "folder", 16.0, color(p.world_cloud)), text(name.clone()).size(style::BODY).font(style::FONT).color(color(p.ink)).width(Length::Fill), text(if added { "In the sidebar" } else { "" }).size(style::LABEL).font(style::FONT).color(color(p.ink_muted))].spacing(style::SPACE_4).align_y(Alignment::Center);
                            list = list.push(w::row_button(p, container(r).padding([6, 4]).into(), false, (!added).then(|| cm(CloudMsg::AddPlace(account.clone(), PlaceChoice::Team(id.clone(), name.clone()))))));
                        }
                        b = b.push(list);
                    }
                    None => {}
                }
                if let Some(e) = &d.error {
                    b = b.push(error_box(e));
                }
                ("cloud", "Add a shared drive".into(), b.into(), vec![cancel("Close")], false)
            }
            Step::Quit => {
                let n = self.cloud_uploads();
                let mut b = column![note("Quitting now is safe: rclone keeps them and uploads them the next time EchoFiles starts.")].spacing(style::SPACE_4);
                if let Some(e) = &d.error {
                    b = b.push(note(e));
                }
                (
                    "cloud",
                    format!("{n} {} still uploading to Google Drive. Quit anyway?", if n == 1 { "file is" } else { "files are" }),
                    b.into(),
                    vec![w::text_button(p, &self.icons, "Quit and upload later", None, None, Variant::Ghost, Some(cm(CloudMsg::QuitAnyway))), w::text_button(p, &self.icons, "Wait for uploads", None, Some("Enter"), Variant::Primary, Some(cm(CloudMsg::QuitWait)))],
                    false,
                )
            }
        };
        Some(self.dialog_frame_with(icon, title, body, foot, danger, false, d.opened))
    }

    /// Enter in a Drive dialog.
    pub(crate) fn cloud_enter(&mut self) -> Task<Message> {
        match self.cloud.dialog.as_ref().map(|d| &d.step) {
            Some(Step::Start { .. }) if !self.cloud.checking => self.cloud_update(CloudMsg::StartSignIn),
            Some(Step::Rename { .. }) => self.cloud_update(CloudMsg::RenameSubmit),
            Some(Step::Quit) => self.cloud_update(CloudMsg::QuitWait),
            // Sign out, disconnect anyway: destructive, never on Enter.
            _ => Task::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words() {
        assert_eq!(display_stem("Budget 2026.link.html"), Some("Budget 2026"));
        assert_eq!(display_stem("report.pdf"), None);
        assert!(usage_text(4_509_715_660, 16_106_127_360).contains(" of "));
        assert!(usage_text(10, 0).ends_with(" used"));
    }
}

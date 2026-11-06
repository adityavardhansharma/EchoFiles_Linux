//! Settings → Google Drive (design system `SettingsPage` › Google Drive): accounts, the
//! Drive index and live search switches (separate from every other index switch), the
//! cache, and "Use my own client ID".

use std::path::PathBuf;

use ef_cloud::drive::{self, Client};
use ef_cloud::{paths, Availability};
use ef_config::{self as config, CloudAccess};
use iced::widget::{column, row, text, text_input};
use iced::{Alignment, Element, Length, Task};

use crate::app::{background, App, Message};
use crate::cloud::{usage_text, CloudMsg};
use crate::style::{self, color};
use crate::widgets::{self as w, Variant};

pub const CLIENT_ID: &str = "cloud-client-id";
pub const CLIENT_SECRET: &str = "cloud-client-secret";
pub const CACHE_ID: &str = "cloud-cache";
const CONSOLE_URL: &str = "https://console.cloud.google.com/apis/credentials";

/// Drafts and facts for the page.
#[derive(Debug, Default)]
pub struct CloudForm {
    pub client_id: String,
    pub client_secret: String,
    pub show_secret: bool,
    pub client_error: Option<String>,
    pub cache: Option<String>,
    pub cache_error: Option<String>,
    /// Bytes in rclone's cache folder (measured when the page opens).
    pub cached: Option<u64>,
    pub clearing: bool,
    /// Client ID in use per account (`None` = rclone's own app).
    pub clients: Vec<(String, Option<String>)>,
    pub advanced: bool,
}

/// A change waiting for its sign-in: put the old values back if it fails.
#[derive(Debug, Clone)]
pub struct Again {
    pub account: String,
    pub prev_client: Option<Option<Client>>,
    pub prev_access: Option<CloudAccess>,
    pub new_access: Option<CloudAccess>,
}

fn dir_size(dir: &std::path::Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => total += e.metadata().map_or(0, |m| m.len()),
                _ => {}
            }
        }
    }
    total
}

/// The end of a client ID, enough to recognise it: "…bf2a.apps.googleusercontent.com".
pub fn short_client(id: &str) -> String {
    let stem = id.strip_suffix(".apps.googleusercontent.com").unwrap_or(id);
    let tail: String = stem.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("…{tail}.apps.googleusercontent.com")
}

impl App {
    /// The page opened: measure the cache and read which client each account uses.
    pub(crate) fn cloud_settings_opened(&mut self) -> Task<Message> {
        let mut tasks = vec![background(|| dir_size(&config::cloud_cache_dir()), |n| Message::Cloud(CloudMsg::CacheSize(n)))];
        if !matches!(self.cloud.avail, Some(Availability::Ready(_))) && !self.cloud.checking {
            tasks.push(self.cloud_update(CloudMsg::Recheck));
        }
        if let Some(sup) = self.cloud.sup.clone() {
            let ids: Vec<String> = self.settings.cloud.accounts.iter().map(|a| a.id.clone()).collect();
            tasks.push(background(
                move || {
                    let Some(rc) = sup.running() else { return Vec::new() };
                    ids.into_iter().map(|id| {
                        let c = drive::client_id(&rc, &id).ok().flatten();
                        (id, c)
                    }).collect()
                },
                |v| Message::Cloud(CloudMsg::ClientsRead(v)),
            ));
        }
        Task::batch(tasks)
    }

    pub(crate) fn cloud_settings_update(&mut self, msg: CloudMsg) -> Task<Message> {
        match msg {
            CloudMsg::ClientDraft(t) => {
                self.cloud.form.client_id = t;
                self.cloud.form.client_error = None;
                Task::none()
            }
            CloudMsg::SecretDraft(t) => {
                self.cloud.form.client_secret = t;
                self.cloud.form.client_error = None;
                Task::none()
            }
            CloudMsg::ToggleSecret => {
                self.cloud.form.show_secret = !self.cloud.form.show_secret;
                Task::none()
            }
            CloudMsg::ToggleAdvanced => {
                self.cloud.form.advanced = !self.cloud.form.advanced;
                Task::none()
            }
            CloudMsg::ClientsRead(v) => {
                self.cloud.form.clients = v;
                Task::none()
            }
            CloudMsg::SaveClient | CloudMsg::UseRcloneApp => {
                let client = if matches!(msg, CloudMsg::SaveClient) {
                    let (id, secret) = (self.cloud.form.client_id.trim().to_string(), self.cloud.form.client_secret.trim().to_string());
                    if !drive::valid_client_id(&id) {
                        self.cloud.form.client_error = Some("A client ID ends with .apps.googleusercontent.com. Copy it from Google Cloud Console → Credentials.".into());
                        return iced::widget::operation::focus(CLIENT_ID);
                    }
                    if secret.is_empty() {
                        self.cloud.form.client_error = Some("Paste the client secret too. It's next to the client ID in Google Cloud Console.".into());
                        return iced::widget::operation::focus(CLIENT_SECRET);
                    }
                    Some(Client { id, secret })
                } else {
                    None
                };
                let Some(first) = self.settings.cloud.accounts.first().map(|a| a.id.clone()) else {
                    self.cloud.form.client_error = Some("Connect Google Drive first; the client ID is saved with the account.".into());
                    return Task::none();
                };
                let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
                let ids: Vec<String> = self.settings.cloud.accounts.iter().map(|a| a.id.clone()).collect();
                let f2 = first.clone();
                background(
                    move || {
                        let (rc, _) = sup.ensure().map_err(|e| e.explain())?;
                        // Remember the first account's old client so a failed sign-in can put it back.
                        let prev = drive::client_params(&rc, &f2).map_err(|e| e.explain())?;
                        for id in &ids {
                            drive::set_client(&rc, id, client.as_ref()).map_err(|e| e.explain())?;
                        }
                        Ok(prev)
                    },
                    move |r| Message::Cloud(CloudMsg::ClientChanged { account: first.clone(), result: r }),
                )
            }
            CloudMsg::ClientChanged { account, result } => match result {
                Ok(prev) => {
                    self.cloud.form.client_secret.clear();
                    // Every account signs in again with the new app; this one right away.
                    for a in &self.settings.cloud.accounts {
                        if a.id != account {
                            self.cloud.status.entry(a.id.clone()).or_default().signed_out = true;
                        }
                    }
                    self.cloud.again = Some(crate::cloud_settings::Again { account: account.clone(), prev_client: Some(prev), prev_access: None, new_access: None });
                    let t = self.cloud_signin_again(&account);
                    Task::batch([t, self.cloud_settings_opened()])
                }
                Err(e) => {
                    self.cloud.form.client_error = Some(e);
                    Task::none()
                }
            },
            CloudMsg::ChangeAccess(id) => {
                let Some(a) = self.cloud_account(&id).cloned() else { return Task::none() };
                let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
                let new = if a.access == CloudAccess::Full { CloudAccess::Read } else { CloudAccess::Full };
                self.cloud.again = Some(Again { account: id.clone(), prev_client: None, prev_access: Some(a.access), new_access: Some(new) });
                let i2 = id.clone();
                background(move || sup.ensure().map_err(|e| e.explain()).and_then(|(rc, _)| drive::set_scope(&rc, &i2, new.into()).map_err(|e| e.explain())), move |r| Message::Cloud(CloudMsg::ScopeSet(id.clone(), r)))
            }
            CloudMsg::ScopeSet(id, r) => match r {
                Ok(()) => self.cloud_signin_again(&id),
                Err(e) => {
                    self.cloud.again = None;
                    self.toast_error("Couldn't change access".into(), e);
                    Task::none()
                }
            },
            CloudMsg::CacheDraft(t) => {
                self.cloud.form.cache = Some(t);
                self.cloud.form.cache_error = None;
                Task::none()
            }
            CloudMsg::CacheSubmit => {
                let Some(t) = self.cloud.form.cache.clone() else { return Task::none() };
                match t.trim().trim_end_matches(['G', 'g', 'B', 'b', ' ']).parse::<u32>() {
                    Ok(n) if (1..=500).contains(&n) => {
                        self.cloud.form.cache = None;
                        if self.settings.cloud.cache_max_gb == n {
                            return Task::none();
                        }
                        self.settings.cloud.cache_max_gb = n;
                        let save = self.persist_settings();
                        // Takes effect when each Drive mounts again; do that now unless files are uploading.
                        let remount = if self.cloud_uploads() == 0 { self.cloud_remount_all() } else { Task::none() };
                        Task::batch([save, remount])
                    }
                    _ => {
                        self.cloud.form.cache_error = Some("Between 1 and 500 GB.".into());
                        Task::none()
                    }
                }
            }
            CloudMsg::CacheSize(n) => {
                self.cloud.form.cached = Some(n);
                self.cloud.form.clearing = false;
                Task::none()
            }
            CloudMsg::ClearCache => {
                if self.cloud_uploads() > 0 {
                    return Task::none();
                }
                self.cloud.form.clearing = true;
                let sup = self.cloud.sup.clone();
                let mounted: Vec<String> = self.cloud.status.iter().filter(|(_, s)| s.mounted).map(|(id, _)| id.clone()).collect();
                let opts: Vec<(String, drive::MountOptions)> = mounted.iter().filter_map(|id| self.cloud_account(id).map(|a| (id.clone(), self.cloud_mount_options(a)))).collect();
                background(
                    move || {
                        // Unmount, empty the cache, mount again: rclone must not be using it meanwhile.
                        let rc = sup.as_ref().and_then(|s| s.running());
                        if let Some(rc) = &rc {
                            for (id, _) in &opts {
                                let _ = drive::unmount(rc, id);
                            }
                        }
                        let dir = config::cloud_cache_dir();
                        for sub in ["vfs", "vfsMeta"] {
                            let _ = std::fs::remove_dir_all(dir.join(sub));
                        }
                        if let Some(rc) = &rc {
                            for (id, o) in &opts {
                                let _ = drive::mount(rc, id, *o);
                            }
                        }
                        dir_size(&dir)
                    },
                    |n| Message::Cloud(CloudMsg::CacheSize(n)),
                )
            }
            CloudMsg::SetIndex(on) => {
                self.settings.cloud.index = on;
                let save = self.persist_settings();
                let t = self.cloud_index_switched(on);
                Task::batch([save, t])
            }
            CloudMsg::SetLive(on) => {
                self.settings.cloud.live_search = on;
                self.persist_settings()
            }
            CloudMsg::OpenLog => {
                let log = paths::log_file();
                let target = if log.exists() { log } else { log.parent().map(PathBuf::from).unwrap_or(log) };
                self.cloud_update(CloudMsg::OpenUrl(target.to_string_lossy().into_owned()))
            }
            _ => Task::none(),
        }
    }

    /// Mount every connected account again (new cache limit, new credentials).
    pub(crate) fn cloud_remount_all(&mut self) -> Task<Message> {
        let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
        let accounts: Vec<(String, drive::MountOptions)> = self.settings.cloud.accounts.iter().filter(|a| self.cloud.status.get(&a.id).is_some_and(|s| s.mounted)).map(|a| (a.id.clone(), self.cloud_mount_options(a))).collect();
        Task::batch(accounts.into_iter().map(|(id, o)| {
            let s2 = sup.clone();
            let i2 = id.clone();
            background(move || s2.ensure().and_then(|(rc, _)| drive::remount(&rc, &i2, o)), move |r| Message::Cloud(CloudMsg::Mounted(id.clone(), r)))
        }).collect::<Vec<_>>())
    }

    // ------------------------------------------------------------------ view

    pub(crate) fn page_cloud(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let c = &self.settings.cloud;
        let f = &self.cloud.form;
        let cmsg = Message::Cloud;
        let mut groups = Vec::new();

        // Account
        let mut rows = Vec::new();
        let rclone_missing = matches!(self.cloud.avail, Some(Availability::Missing | Availability::TooOld { .. }));
        if rclone_missing {
            let why = match &self.cloud.avail {
                Some(Availability::TooOld { found, .. }) => format!("rclone {found} is too old; EchoFiles needs {} or newer. Update it with: {}", ef_cloud::rclone::MIN_VERSION, ef_cloud::rclone::install_command()),
                _ => format!("rclone isn't installed. Install it with: {}", ef_cloud::rclone::install_command()),
            };
            rows.push(self.block(self.note(p.warning, "alert", why, Some(("Check again", crate::settings::SettingsMsg::Page(crate::settings::Page::Cloud))))));
        }
        for a in &c.accounts {
            let st = self.cloud.status.get(&a.id).cloned().unwrap_or_default();
            let access = if a.access == CloudAccess::Read { "Read-only" } else { "Full access" };
            let mut desc = access.to_string();
            if let Some((u, t)) = st.quota {
                desc.push_str(" · ");
                desc.push_str(&usage_text(u, t));
                if t > 0 {
                    desc.push_str(" used");
                }
            }
            if st.signed_out {
                desc.push_str(" · needs you to sign in again");
            } else if !st.mounted {
                desc.push_str(" · not connected");
            }
            let controls = row![
                w::text_button(p, &self.icons, "Rename…", None, None, Variant::Ghost, Some(cmsg(CloudMsg::AskRename(a.id.clone())))),
                w::text_button(p, &self.icons, if a.access == CloudAccess::Full { "Make read-only…" } else { "Allow changes…" }, None, None, Variant::Ghost, (!rclone_missing).then(|| cmsg(CloudMsg::ChangeAccess(a.id.clone())))),
                if st.signed_out {
                    w::text_button(p, &self.icons, "Sign in again", Some("key"), None, Variant::Primary, (!rclone_missing).then(|| cmsg(CloudMsg::Click(a.id.clone()))))
                } else {
                    w::text_button(p, &self.icons, "Sign out…", None, None, Variant::Danger, Some(cmsg(CloudMsg::AskSignOut(a.id.clone()))))
                },
            ]
            .spacing(style::SPACE_3)
            .align_y(Alignment::Center);
            rows.push(self.setting(&a.name, &desc, controls.into()));
        }
        rows.push(self.setting(
            if c.accounts.is_empty() { "Connect Google Drive" } else { "Add another account" },
            if c.accounts.is_empty() { "Your Google Drive in the sidebar. You sign in on Google's page in your browser, through rclone." } else { "Each account gets its own place in the sidebar." },
            w::text_button(p, &self.icons, "Connect Google Drive", Some("plus"), None, Variant::Secondary, (!rclone_missing).then(|| cmsg(CloudMsg::OpenConnect))),
        ));
        groups.push(self.group("Account", rows));

        // Search
        let mut rows = vec![self.setting(
            "Include Google Drive in search index",
            "Saves file names, folders, sizes and dates from Google Drive on this computer, so search is instant. File contents are never saved. Off: nothing about your Drive is saved here. This switch is separate from the main search index.",
            w::switch(p, c.index, Some(Box::new(|b| Message::Cloud(CloudMsg::SetIndex(b))))),
        )];
        if c.index {
            rows.push(self.cloud_index_status());
        }
        rows.push(self.setting(
            "Search Google Drive live",
            "After showing saved results, also asks Google Drive. Google Drive only matches the start of words.",
            w::switch(p, c.live_search, Some(Box::new(|b| Message::Cloud(CloudMsg::SetLive(b))))),
        ));
        groups.push(self.group("Search", rows));

        // Storage
        let cache_text = f.cache.clone().unwrap_or_else(|| c.cache_max_gb.to_string());
        let cache_field = row![
            text_input("10", &cache_text).id(CACHE_ID).on_input(|t| Message::Cloud(CloudMsg::CacheDraft(t))).on_submit(Message::Cloud(CloudMsg::CacheSubmit)).size(style::BODY).font(style::FONT).padding([5, 10]).width(64).style(w::field_style(p, f.cache_error.is_some())),
            text("GB").size(style::META).font(style::FONT).color(color(p.ink_muted)),
        ]
        .spacing(style::SPACE_3)
        .align_y(Alignment::Center);
        let cache_desc = f.cache_error.clone().unwrap_or_else(|| "Files you open are kept here so they open instantly next time. Between 1 and 500 GB; press Enter to apply.".into());
        let mut cached = String::from("Cached now: ");
        match f.cached {
            Some(n) => ef_core::fmt::size(n, &mut cached),
            None => cached.push('…'),
        }
        let uploading = self.cloud_uploads() > 0;
        groups.push(self.group(
            "Storage",
            vec![
                self.setting("Cache size limit", &cache_desc, cache_field.into()),
                self.setting(
                    &cached,
                    &if uploading { format!("{} · can't clear while files are uploading", config::tilde(&config::cloud_cache_dir())) } else { config::tilde(&config::cloud_cache_dir()) },
                    w::text_button(p, &self.icons, if f.clearing { "Clearing…" } else { "Clear cache" }, None, None, Variant::Ghost, (!uploading && !f.clearing).then_some(cmsg(CloudMsg::ClearCache))),
                ),
            ],
        ));

        // Advanced: own client ID
        let own = f.clients.iter().find_map(|(_, c)| c.clone());
        let mut rows = vec![self.setting(
            "Use my own client ID",
            &match &own {
                Some(id) => format!("Using your app ({}). It has its own speed limits, and the index updates almost instantly.", short_client(id)),
                None => "Using rclone's shared app. Your own app has its own speed limits instead of sharing rclone's, and lets the index update almost instantly. rclone is retiring its shared Drive app during 2026.".into(),
            },
            row![
                w::text_button(p, &self.icons, if f.advanced { "Hide" } else { "Set up…" }, None, None, Variant::Ghost, Some(cmsg(CloudMsg::ToggleAdvanced))),
                w::text_button(p, &self.icons, "Use rclone's app", None, None, Variant::Ghost, own.is_some().then_some(cmsg(CloudMsg::UseRcloneApp))),
            ]
            .spacing(style::SPACE_3)
            .into(),
        )];
        if f.advanced {
            let field = |id: &'static str, ph: &'static str, v: &str, secure: bool, on: fn(String) -> CloudMsg| -> Element<'_, Message> {
                text_input(ph, v).id(id).secure(secure).on_input(move |t| Message::Cloud(on(t))).on_submit(Message::Cloud(CloudMsg::SaveClient)).size(style::BODY).font(style::FONT).padding([6, 10]).style(w::field_style(p, f.client_error.is_some())).into()
            };
            let muted = color(p.ink_muted);
            fn labeled<'a>(label: &str, muted: iced::Color, e: Element<'a, Message>) -> Element<'a, Message> {
                column![text(label.to_string()).size(style::LABEL).font(style::FONT_BOLD).color(muted), e].spacing(4).width(Length::Fill).into()
            }
            let secret = row![field(CLIENT_SECRET, "Client secret", &f.client_secret, !f.show_secret, CloudMsg::SecretDraft), self.tip(w::icon_button(p, &self.icons, if f.show_secret { "eye-off" } else { "eye" }, Some(cmsg(CloudMsg::ToggleSecret)), false), if f.show_secret { "Hide secret" } else { "Show secret" }, None)].spacing(style::SPACE_3).align_y(Alignment::Center);
            let mut form = column![
                row![labeled("Client ID", muted, field(CLIENT_ID, "…apps.googleusercontent.com", &f.client_id, false, CloudMsg::ClientDraft)), labeled("Client secret", muted, secret.into())].spacing(style::SPACE_4),
                row![
                    text(if c.accounts.len() > 1 { "Every account signs in again with your app." } else { "Google Drive signs in again with your app." }).size(style::META).font(style::FONT).color(color(p.ink_muted)).width(Length::Fill),
                    w::text_button(p, &self.icons, "Save and sign in again", Some("key"), None, Variant::Primary, (!rclone_missing).then_some(cmsg(CloudMsg::SaveClient))),
                ]
                .align_y(Alignment::Center),
            ]
            .spacing(style::SPACE_4);
            if let Some(e) = &f.client_error {
                form = form.push(row![w::glyph(&self.icons, "error", 14.0, color(p.danger.ink)), text(e.clone()).size(style::META).font(style::FONT).color(color(p.danger.ink))].spacing(6).align_y(Alignment::Center));
            }
            rows.push(self.block(form));
            rows.push(self.block(self.mono_block(&[
                ("1", "Create a project in Google Cloud Console"),
                ("2", "Enable the Google Drive API"),
                ("3", "OAuth consent screen: External, add yourself as a test user"),
                ("4", "Publish the app (In production), or Google signs you out every 7 days"),
                ("5", "Credentials → OAuth client ID → Desktop app; paste the ID and secret here"),
            ])));
            rows.push(self.block(row![
                text("Google shows an \"unverified app\" warning for your own app during sign-in. That's expected; choose to continue.").size(style::META).font(style::FONT).color(color(p.ink_muted)).width(Length::Fill),
                w::text_button(p, &self.icons, "Open Google Cloud Console", Some("external"), None, Variant::Ghost, Some(cmsg(CloudMsg::OpenUrl(CONSOLE_URL.into())))),
            ].align_y(Alignment::Center)));
        }
        groups.push(self.group("Advanced", rows));

        // rclone
        let (label, desc, ok) = match &self.cloud.avail {
            Some(Availability::Ready(r)) => (format!("rclone {}", r.version), format!("{} · log in {}", r.path.display(), config::tilde(&paths::log_file())), true),
            Some(Availability::TooOld { found, path }) => (format!("rclone {found} (too old)"), path.display().to_string(), false),
            Some(Availability::Missing) => ("rclone not found".into(), "Looked on PATH, /usr/bin and /usr/local/bin.".into(), false),
            None => ("rclone".into(), "Looking…".into(), false),
        };
        let mut rows = vec![self.setting(&label, &desc, w::text_button(p, &self.icons, "Open log", Some("file-text"), None, Variant::Ghost, ok.then_some(cmsg(CloudMsg::OpenLog))))];
        if ok && !self.cloud.fuse {
            rows.push(self.block(self.note(p.warning, "alert", "FUSE isn't installed, so Google Drive can't show as a folder. Install the fuse3 package.".into(), None)));
        }
        groups.push(self.group("rclone", rows));

        self.page(self.page_head("Google Drive", "Your Google Drive in the sidebar, through rclone. Sign-in happens in your browser."), groups)
    }

    /// Put back what changed before a sign-in that didn't finish.
    pub(crate) fn cloud_restore_again(&mut self) -> Task<Message> {
        let Some(again) = self.cloud.again.take() else { return Task::none() };
        let Some(sup) = self.cloud.sup.clone() else { return Task::none() };
        let ids: Vec<String> = if again.prev_client.is_some() { self.settings.cloud.accounts.iter().map(|a| a.id.clone()).collect() } else { vec![again.account.clone()] };
        for id in &ids {
            if let Some(s) = self.cloud.status.get_mut(id) {
                s.signed_out = false;
            }
        }
        background(
            move || {
                let Some(rc) = sup.running() else { return };
                if let Some(prev) = &again.prev_client {
                    for id in &ids {
                        let _ = drive::set_client(&rc, id, prev.as_ref());
                    }
                }
                if let Some(a) = again.prev_access {
                    let _ = drive::set_scope(&rc, &again.account, a.into());
                }
            },
            |_| Message::Tick,
        )
    }

    /// A sign-in again finished: apply what it was for (new access) and mount fresh.
    pub(crate) fn cloud_again_done(&mut self, account: &str) -> Task<Message> {
        let again = self.cloud.again.take().filter(|a| a.account == account);
        if let Some(new) = again.as_ref().and_then(|a| a.new_access)
            && let Some(a) = self.settings.cloud.accounts.iter_mut().find(|a| a.id == account)
        {
            a.access = new;
        }
        let save = self.persist_settings();
        let Some(sup) = self.cloud.sup.clone() else { return save };
        let Some(o) = self.cloud_account(account).map(|a| self.cloud_mount_options(a)) else { return save };
        let id = account.to_string();
        let i2 = id.clone();
        let remount = background(move || sup.ensure().and_then(|(rc, _)| drive::remount(&rc, &i2, o)), move |r| Message::Cloud(CloudMsg::Mounted(id.clone(), r)));
        if let Some(st) = self.cloud.status.get_mut(account) {
            st.mounting = true;
        }
        Task::batch([save, remount, self.cloud_settings_opened()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_ids() {
        assert_eq!(short_client("1234-abcdbf2a.apps.googleusercontent.com"), "…bf2a.apps.googleusercontent.com");
    }
}

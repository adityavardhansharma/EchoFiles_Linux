//! What the phone service reports (devices, pairing, battery, notifications, texts,
//! transfers, EchoConnect's own packets) turned into app state.

use super::*;

impl App {
    pub(super) fn phone_event(&mut self, e: Event) -> Task<Message> {
        let selected = self.phone_id();
        let selected_current = self.phone.current.clone();
        let device = match &e {
            Event::Notification { id, .. }
            | Event::NotificationGone { id, .. }
            | Event::Sms { id, .. }
            | Event::Sftp { id, .. }
            | Event::Other { id, .. }
            | Event::Incoming { id, .. }
            | Event::TransferStarted { id, .. }
            | Event::TransferDone { id, .. } => Some(id.clone()),
            _ => selected.clone(),
        };
        let routed = matches!(
            &e,
            Event::Notification { .. }
                | Event::NotificationGone { .. }
                | Event::Sms { .. }
                | Event::Sftp { .. }
                | Event::Other { .. }
                | Event::Incoming { .. }
                | Event::TransferStarted { .. }
                | Event::TransferDone { .. }
        );
        if let Some(id) = &device {
            self.activate_phone(id);
            if routed {
                self.phone.current = Some(id.clone());
            }
        }
        let task = self.phone_event_inner(e);
        if routed {
            self.phone.current = selected_current;
            if let Some(id) = selected {
                self.activate_phone(&id);
            }
        }
        task
    }

    pub(super) fn activate_phone(&mut self, id: &str) {
        self.phone.activate(id);
    }

    pub(crate) fn imported_copies(&mut self, keys: Vec<String>) {
        if keys.is_empty() {
            return;
        }
        self.phone.imported.extend(keys);
        save_lines(imported_file(), self.phone.imported.iter().map(|k| format!("{k}\n")).collect());
    }

    pub(super) fn phone_event_inner(&mut self, e: Event) -> Task<Message> {
        match e {
            Event::Started { port } => {
                self.phone.started = true;
                self.phone.port = port;
                self.phone.error = None;
                Task::none()
            }
            Event::Failed(why) => {
                self.phone.started = false;
                self.phone.error = Some(why);
                Task::none()
            }
            Event::Device(d) => {
                let id = d.id.clone();
                let paired = d.paired;
                // A phone whose details changed (renamed) keeps its trust record fresh.
                self.phone.devices.insert(id.clone(), d);
                if paired {
                    self.phone_seen(&id);
                    if self.settings.phone.clipboard
                        && !self.phone.clip_sensitive
                        && !self.phone.clip_last.is_empty()
                        && let Some(s) = service()
                    {
                        s.send_clipboard(&id, &self.phone.clip_last, true, false);
                    }
                    if self.settings.phone.files && Some(&id) == self.phone_id().as_ref() {
                        self.request_photos();
                        if self.phone.mount.is_none() {
                            return self.phone_files(Want::Nothing);
                        }
                    }
                }
                Task::none()
            }
            Event::Gone(id) => {
                if let Some(state) = self.phone.states.get_mut(&id) {
                    state.generation = crate::pane::next_id();
                    state.photos = None;
                    state.photos_loading = false;
                    state.mounting = false;
                    state.want = None;
                    if let Some((m, _)) = state.mount.take() {
                        std::thread::spawn(move || m.unmount());
                    }
                }
                if self.phone.active_id == id {
                    self.phone.away_since = Some(Instant::now());
                    self.phone.ringing = None;
                }
                if self.phone.devices.get(&id).is_some_and(|d| d.paired) {
                    self.phone_seen(&id);
                }
                self.phone.devices.remove(&id);
                if self.phone.code.as_ref().is_some_and(|c| c.0 == id) {
                    self.phone.code = None;
                }
                if self.phone.active_id == id {
                    self.phone.notifications.clear();
                    return self.drop_phone_mount();
                }
                Task::none()
            }
            Event::PairRequested { id, code } => {
                self.phone.code = Some((id, code, true));
                if self.phone.dialog.is_none() {
                    self.phone.dialog = Some(PairDialog {
                        ip: String::new(),
                        opened: Instant::now(),
                        since: Instant::now(),
                        firewall: None,
                        firewall_busy: false,
                        paired: None,
                    });
                }
                self.show_window()
            }
            Event::PairCode { id, code } => {
                self.phone.code = Some((id, code, false));
                Task::none()
            }
            Event::Paired(d) => {
                self.phone.code = None;
                self.phone.trusted = ef_phone::trusted();
                self.activate_phone(&d.id);
                self.phone.current = Some(d.id.clone());
                self.phone.devices.insert(d.id.clone(), d.clone());
                self.phone_seen(&d.id);
                match &mut self.phone.dialog {
                    Some(dlg) => dlg.paired = Some(d.id.clone()),
                    None => self.push_toast(Tone::Accent, format!("Paired with {}", d.name), None, None),
                }
                if self.settings.phone.files { self.phone_files(Want::Nothing) } else { Task::none() }
            }
            Event::PairRejected(id) => {
                if self.phone.code.as_ref().is_some_and(|c| c.0 == id) {
                    self.phone.code = None;
                }
                self.toast_error(
                    format!("{} didn't pair", self.phone_name(&id)),
                    "It was declined on the phone. Try again, and tap Accept there.".into(),
                );
                Task::none()
            }
            Event::Unpaired(id) => {
                if let Some(state) = self.phone.states.remove(&id)
                    && let Some((m, _)) = state.mount
                {
                    std::thread::spawn(move || m.unmount());
                }
                let index = config::state_dir().join("phone-index").join(format!("{id}.jsonl"));
                std::thread::spawn(move || {
                    let _ = std::fs::remove_file(index);
                });
                let name = self.phone_name(&id);
                self.phone.trusted = ef_phone::trusted();
                if let Some(d) = self.phone.devices.get_mut(&id) {
                    d.paired = false;
                }
                let t = if self.phone.current.as_deref() == Some(&id) || self.phone_id().is_none() {
                    self.phone.current = None;
                    self.phone.notifications.clear();
                    self.phone.threads.clear();
                    self.phone.thread_msgs.clear();
                    self.phone.photos = None;
                    self.phone.screen = None;
                    self.drop_phone_mount()
                } else {
                    Task::none()
                };
                // Pages about a phone that's gone close.
                if self.phone.trusted.is_empty() {
                    for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()) {
                        pane.phone = None;
                    }
                }
                self.push_toast(Tone::Accent, format!("Forgot {name}"), Some("Pair again from the Phone section.".into()), None);
                t
            }
            Event::Battery { id, level, charging } => {
                self.phone.battery.insert(id.clone(), (level, charging));
                if self.settings.phone.battery_warning && level <= 15 && !charging {
                    if self.phone.low_warned.insert(id.clone()) {
                        self.push_toast(
                            Tone::Accent,
                            format!("{} is at {level}%", self.phone_name(&id)),
                            Some("Plug it in soon.".into()),
                            None,
                        );
                    }
                } else if level > 20 || charging {
                    self.phone.low_warned.remove(&id);
                }
                Task::none()
            }
            Event::Clipboard { text, sensitive, .. } => {
                if sensitive {
                    self.phone.clip.retain(|(_, old, _)| old != &text);
                    if self.phone.clip_last == text {
                        self.phone.clip_sensitive = true;
                    }
                }
                if !self.settings.phone.clipboard || text == self.phone.clip_last {
                    return Task::none();
                }
                self.phone.clip_sensitive = sensitive;
                self.phone.clip_last = text.clone();
                if !sensitive {
                    self.phone.clip.push_front((true, text.clone(), SystemTime::now()));
                }
                self.phone.clip.truncate(20);
                std::thread::spawn(move || set_clipboard(&text));
                Task::none()
            }
            Event::Notification { id, notification: n } => {
                if self.settings.phone.otp
                    && let Some(code) = crate::connect::otp(&format!("{} {}", n.title, n.text))
                {
                    std::thread::spawn(move || set_clipboard(&code));
                    self.push_toast(Tone::Accent, "One-time code copied".into(), None, None);
                }

                let mode = self.settings.phone.notifications;
                if mode == PhoneNotifications::Off || self.settings.phone.muted_apps.iter().any(|a| a.eq_ignore_ascii_case(&n.app)) {
                    return Task::none();
                }
                let fresh = !self.phone.notifications.iter().any(|x| x.key == n.key);
                if mode == PhoneNotifications::Desktop && fresh && !n.silent && !self.phone.dnd {
                    notify_desktop(&self.phone_name(&id), &n);
                }
                self.phone.notifications.retain(|x| x.key != n.key);
                self.phone.notifications.push(n);
                self.phone.notifications.sort_by_key(|n| std::cmp::Reverse(n.time));
                Task::none()
            }
            Event::NotificationGone { key, .. } => {
                self.phone.notifications.retain(|n| n.key != key);
                Task::none()
            }
            Event::Sms { messages, .. } => {
                if self.settings.phone.otp {
                    for message in &messages {
                        if !message.read
                            && !message.outgoing
                            && now_s() * 1000 - message.date < 120000
                            && let Some(code) = crate::connect::otp(&message.body)
                        {
                            std::thread::spawn(move || set_clipboard(&code));
                            self.push_toast(Tone::Accent, "One-time code copied".into(), None, None);
                            break;
                        }
                    }
                }

                if !self.settings.phone.messages {
                    return Task::none();
                }
                // A batch is merged per thread with sets and sorted once (per-message scans
                // and sorts made a long conversation O(n²)).
                let mut batches: HashMap<i64, Vec<_>> = HashMap::new();
                for m in messages {
                    let t = m.thread;
                    if self.phone.threads.get(&t).is_none_or(|old| m.date >= old.date) {
                        self.phone.threads.insert(t, m.clone());
                    }
                    batches.entry(t).or_default().push(m);
                }
                for (t, batch) in batches {
                    let list = self.phone.thread_msgs.entry(t).or_default();
                    // Our own optimistic copy (negative uid) gives way to the phone's.
                    let confirmed: HashSet<&str> = batch.iter().filter(|m| m.outgoing).map(|m| m.body.as_str()).collect();
                    if !confirmed.is_empty() {
                        list.retain(|x| !(x.uid < 0 && confirmed.contains(x.body.as_str())));
                    }
                    let mut uids: HashSet<i64> = list.iter().filter(|x| x.uid > 0).map(|x| x.uid).collect();
                    let mut same: HashSet<(i64, String, bool)> = list.iter().map(|x| (x.date, x.body.clone(), x.outgoing)).collect();
                    for m in batch {
                        if (m.uid > 0 && uids.contains(&m.uid)) || same.contains(&(m.date, m.body.clone(), m.outgoing)) {
                            continue;
                        }
                        if m.uid > 0 {
                            uids.insert(m.uid);
                        }
                        same.insert((m.date, m.body.clone(), m.outgoing));
                        list.push(m);
                    }
                    list.sort_by_key(|x| x.date);
                }
                Task::none()
            }
            Event::Sftp { id, result } => {
                if !self.settings.phone.files || self.phone.active_id != id || !self.phone.mounting || self.phone.want.is_none() {
                    return Task::none();
                }
                match result {
                    Ok(s) => {
                        self.phone.sftp = Some(s.clone());
                        self.mount_phone(s)
                    }
                    Err(e) => {
                        self.phone.mounting = false;
                        let wanted = self.phone.want.take().is_some_and(|w| w != Want::Nothing);
                        self.phone.sftp_error = Some(e.clone());
                        if wanted {
                            self.toast_error("The phone didn't share its files".into(), e.clone());
                        }
                        Task::none()
                    }
                }
            }
            Event::Text { id, text } => {
                self.phone.clip_last = text.clone();
                let t2 = text.clone();
                std::thread::spawn(move || set_clipboard(&t2));
                self.push_toast(
                    Tone::Accent,
                    format!("Text from {}", self.phone_name(&id)),
                    Some(format!("Copied to the clipboard: {}", text.chars().take(120).collect::<String>())),
                    None,
                );
                Task::none()
            }
            Event::Url { id, url } => {
                if url.starts_with("http://") || url.starts_with("https://") {
                    let _ = crate::system::spawn_detached(std::process::Command::new("xdg-open").arg(&url));
                }
                self.push_toast(Tone::Accent, format!("Link from {}", self.phone_name(&id)), Some(url), None);
                Task::none()
            }
            Event::Incoming { id, transfer, name, size, kind, body } => {
                if !self.settings.phone.files && kind != "echofiles.clipboard.image" {
                    if let Some(s) = service() {
                        s.reject_file(transfer);
                    }
                    return Task::none();
                }
                if kind == "echofiles.files.thumb" {
                    let request = body["requestId"].as_str().unwrap_or("").to_string();
                    if let Some(s) = service() {
                        if thumb_wanted(&request) {
                            thumb_transfers().lock().unwrap().insert(transfer, request);
                            s.accept_file(transfer, config::state_dir().join("phone-file-thumbs"));
                        } else {
                            s.reject_file(transfer);
                        }
                    }
                    return Task::none();
                }
                if kind == "echofiles.photos.thumb" {
                    if let Some(path) = self.phone.remote_ids.get(&body["id"].as_u64().unwrap_or(0)).cloned() {
                        self.phone.thumb_wait.remove(&body["id"].as_u64().unwrap_or(0));
                        self.phone.remote_thumb.insert(transfer, path);
                        if let Some(s) = service() {
                            s.accept_file(transfer, config::state_dir().join("phone-thumbnails"));
                        }
                    }
                    return Task::none();
                }
                if kind == "echofiles.photos.file" || kind == "echofiles.files.read" {
                    let request = body["requestId"].as_str().unwrap_or("");
                    if let (Some(dest), Some(s)) = (self.phone.captures.remove(request), service()) {
                        let action = self.phone.photo_requests.remove(request).unwrap_or_else(|| "photo-save".into());
                        self.phone.payloads.insert(transfer, action);
                        s.accept_file(transfer, dest);
                    } else if let Some(s) = service() {
                        s.reject_file(transfer);
                    }
                    return Task::none();
                }
                if kind == "echofiles.capture" {
                    let destination = self.phone.captures.remove(body["requestId"].as_str().unwrap_or(""));
                    if let Some(s) = service() {
                        if let Some(dir) = destination {
                            s.accept_file(transfer, dir);
                        } else {
                            s.reject_file(transfer);
                        }
                    }
                    return Task::none();
                }
                if kind == "echofiles.backup" {
                    if let Some(s) = service() {
                        if self.settings.phone.backup {
                            self.phone.backup_requests.insert(transfer, body["requestId"].as_str().unwrap_or("").into());
                            self.phone.payloads.insert(transfer, kind);
                            s.accept_file(transfer, config::expand("~/Pictures/Phone/Backup"));
                        } else {
                            s.reject_file(transfer);
                        }
                    }
                    return Task::none();
                }
                if kind == "echofiles.clipboard.image" {
                    if let Some(s) = service() {
                        if self.settings.phone.clipboard && size <= 16 * 1024 * 1024 {
                            self.phone.payloads.insert(transfer, kind);
                            s.accept_file(transfer, config::state_dir().join("phone-clipboard"));
                        } else {
                            s.reject_file(transfer);
                        }
                    }
                    return Task::none();
                }
                if self.settings.phone.auto_accept {
                    if let Some(s) = service() {
                        s.accept_file(transfer, config::expand(&self.settings.phone.save_to));
                    }
                } else {
                    let mut sz = String::new();
                    ef_core::fmt::size(size, &mut sz);
                    let phone = self.phone_name(&id);
                    self.phone.offers.push((transfer, id, name.clone(), size));
                    self.push_toast(
                        Tone::Accent,
                        format!("{phone} wants to send “{name}”"),
                        Some(format!("{sz} · saves to {}", self.settings.phone.save_to)),
                        Some(("Accept".into(), Message::Phone(PhoneMsg::AcceptOffer(transfer)))),
                    );
                }
                Task::none()
            }
            Event::Ready(id) => {
                if let Some(s) = service() {
                    s.send_packet(&id, "echofiles.theme", crate::connect::theme(&self.palette));
                    s.send_packet(&id, "kdeconnect.contacts.request_all_uids_timestamps", ef_phone::json!({}));
                    s.send_packet(&id, "kdeconnect.mpris.request", ef_phone::json!({"requestPlayerList":true}));
                    if self.settings.phone.phone_index {
                        self.start_phone_index(&id);
                    }
                }
                self.phone.away_since = None;
                Task::none()
            }
            Event::Other { id, kind, body } => {
                match kind.as_str() {
                    // A thumbnail the phone couldn't make (no payload, just why).
                    "echofiles.files.thumb" => {
                        if let Some(request) = body["requestId"].as_str() {
                            thumb_answer(request, Err(body["error"].as_str().unwrap_or("No thumbnail").to_string()));
                        }
                    }
                    "echofiles.clipboard.image.inline" if self.settings.phone.clipboard => {
                        let data = body["data"].as_str().unwrap_or("");
                        if data.len() <= 65536
                            && let Ok(bytes) = crate::codec::base64_decode(data).ok_or(())
                        {
                            std::thread::spawn(move || {
                                if let Some(png) = normal_image(&bytes) {
                                    let _ = event_tx().unbounded_send(PhoneMsg::RemoteImage(png));
                                }
                            });
                        }
                    }
                    "echofiles.dnd" if self.settings.phone.dnd_sync => {
                        let enabled = body["enabled"].as_bool().unwrap_or(false);
                        if self.phone.dnd != enabled {
                            self.phone.dnd = enabled;
                            std::thread::spawn(move || {
                                crate::connect::set_dnd(enabled);
                            });
                        }
                    }
                    "echofiles.lock" => crate::connect::lock(),
                    "echofiles.call" if body["action"].as_str() == Some("phone") => {
                        return self.phone_update(PhoneMsg::Extra("phone".into()));
                    }
                    "kdeconnect.telephony" => {
                        self.phone.call = if body["isCancel"].as_bool() == Some(true) {
                            std::thread::spawn(|| {
                                let _ = crate::connect::release_headset();
                            });
                            None
                        } else {
                            Some(body)
                        };
                    }
                    "kdeconnect.mpris" => {
                        if let Some(players) = body["playerList"].as_array() {
                            self.phone.players.retain(|p, _| players.iter().any(|v| v.as_str() == Some(p)));
                            for p in players.iter().filter_map(|v| v.as_str()) {
                                if let Some(s) = service() {
                                    s.send_packet(&id, "kdeconnect.mpris.request", ef_phone::json!({"player":p,"requestNowPlaying":true}));
                                }
                            }
                        } else if let Some(player) = body["player"].as_str() {
                            self.phone.players.insert(player.into(), body);
                        }
                    }
                    "kdeconnect.connectivity_report" => {
                        self.phone.signal = body["signalStrengths"]
                            .as_object()
                            .map(|signals| {
                                signals
                                    .values()
                                    .map(|v| {
                                        format!(
                                            "{} · {}/4",
                                            v["networkType"].as_str().unwrap_or("Mobile"),
                                            v["signalStrength"].as_i64().unwrap_or(0)
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" · ")
                            })
                            .unwrap_or_default();
                    }
                    "echofiles.settings" => {
                        // The phone may only switch features off. Turning one on (sharing the
                        // clipboard, accepting backups, locking this session) is a decision
                        // made here, in Settings, never by a remote device.
                        let off = |k: &str| body[k].as_bool() == Some(false);
                        if off("backup") {
                            self.settings.phone.backup = false;
                        }
                        if off("dnd") {
                            self.settings.phone.dnd_sync = false;
                        }
                        if off("notifications") {
                            self.settings.phone.notifications = PhoneNotifications::Off;
                            self.phone.notifications.clear();
                        }
                        if off("autolock") {
                            self.settings.phone.autolock = false;
                        }
                        if off("clipboard") {
                            self.settings.phone.clipboard = false;
                        }
                        if off("files") {
                            self.settings.phone.files = false;
                        }
                        if off("sms") {
                            self.settings.phone.messages = false;
                        }
                        return self.persist_settings();
                    }
                    "kdeconnect.contacts.response_uids_timestamps" => {
                        if let (Some(s), Some(o)) = (service(), body.as_object()) {
                            for chunk in o.keys().collect::<Vec<_>>().chunks(100) {
                                s.send_packet(&id, "kdeconnect.contacts.request_vcards_by_uid", ef_phone::json!({"uids":chunk}));
                            }
                        }
                    }
                    "kdeconnect.contacts.response_vcards" => {
                        if let Some(o) = body.as_object() {
                            for (_, card) in o {
                                if let Some(card) = card.as_str() {
                                    let name = card.lines().find_map(|l| l.strip_prefix("FN:")).unwrap_or("").trim();
                                    for line in card.lines().filter(|l| l.starts_with("TEL")) {
                                        if let Some((_, number)) = line.split_once(':') {
                                            self.phone.contacts.insert(number.trim().into(), name.into());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "echofiles.photos.error" => {
                        self.phone.extra_status = body["message"].as_str().unwrap_or("Photo request failed").into();
                        self.phone.photos_loading = false;
                        if let Some((path, _)) = body["id"].as_u64().and_then(|id| self.phone.thumb_wait.remove(&id)) {
                            self.phone.photo_jobs = self.phone.photo_jobs.saturating_sub(1);
                            self.phone.photo_thumbs.insert(path, None);
                            return self.pump_photo_thumbs();
                        }
                        // The list itself failed: the Photos page says so, with Try again.
                        if self.phone.photos.is_none() {
                            self.phone.photos = Some(Err(self.phone.extra_status.clone()));
                        }
                    }
                    "echofiles.photos" => {
                        let mut photos = match self.phone.photos.take() {
                            Some(Ok(p)) => p,
                            _ => Vec::new(),
                        };
                        self.phone.photos_loading = false;
                        self.phone.photo_more = body["more"].as_bool().unwrap_or(false);
                        if let Some(entries) = body["photos"].as_array() {
                            for entry in entries {
                                let Some(photo_id) = entry["id"].as_u64() else { continue };
                                let name = Path::new(entry["name"].as_str().unwrap_or("photo")).file_name().unwrap_or_default();
                                let path = config::state_dir().join("phone-photos").join(photo_id.to_string()).join(name);
                                let folder = entry["folder"].as_str().unwrap_or("");
                                let source = if folder.contains("Screenshots") {
                                    Source::Screenshots
                                } else if folder.starts_with("DCIM") {
                                    Source::Camera
                                } else if folder.contains("WhatsApp") {
                                    Source::Chats
                                } else {
                                    Source::Other
                                };
                                let date = entry["date"].as_i64().unwrap_or(0);
                                self.phone.photo_before = Some((date, photo_id));
                                self.phone.remote_ids.insert(photo_id, path.clone());
                                if self.phone.remote_photos.insert(path.clone(), photo_id).is_none() {
                                    photos.push(Photo {
                                        path: path.clone(),
                                        mtime: date,
                                        size: entry["size"].as_u64().unwrap_or(0),
                                        source,
                                        video: entry["video"].as_bool().unwrap_or(false),
                                    });
                                }
                            }
                        }
                        photos.sort_by_key(|p| std::cmp::Reverse(p.mtime));
                        self.phone.photos = Some(Ok(photos));
                        let thumbs = self.want_photo_thumbs();
                        return Task::batch([thumbs, self.want_screen()]);
                    }
                    "echofiles.files.list" if self.settings.phone.phone_index => {
                        if let Some(mut scan) = self.phone.index_scan.take() {
                            if scan.device != id || body["requestId"].as_str() != Some(&scan.request) {
                                self.phone.index_scan = Some(scan);
                                return Task::none();
                            }
                            if let Err(error) = scan.accept(&body) {
                                self.phone.extra_status = format!("Phone index: {error}");
                            } else if let Some(next) = scan.next() {
                                if let Some(s) = service() {
                                    s.send_packet(&id, "echofiles.files.list", next);
                                }
                                self.phone.index_at = Some(Instant::now());
                                self.phone.index_scan = Some(scan);
                            } else {
                                self.phone.extra_status = match scan.save() {
                                    Ok(n) => format!("{n} phone names available offline"),
                                    Err(e) => format!("Could not save phone index: {e}"),
                                };
                            }
                        }
                    }
                    _ => {}
                }
                Task::none()
            }
            Event::TransferStarted { transfer, name, size, upload, done, .. } => {
                // Photo thumbnails and clipboard images are the app's own traffic, not files
                // you sent or received; they stay out of "Received from phone".
                let internal = self.phone.remote_thumb.contains_key(&transfer)
                    || thumb_transfers().lock().unwrap().contains_key(&transfer)
                    || self.phone.payloads.get(&transfer).is_some_and(|k| k == "echofiles.clipboard.image");
                if !internal {
                    self.phone.moved.insert(0, Moved { transfer, name, size, upload, done, result: None, at: SystemTime::now() });
                    self.phone.moved.truncate(30);
                }
                Task::none()
            }
            Event::TransferDone { id, transfer, upload, result } => {
                let thumb = thumb_transfers().lock().unwrap().remove(&transfer);
                if let Some(request) = thumb {
                    thumb_answer(&request, result);
                    return Task::none();
                }
                // Mark the row finished first: some branches below return early, which left
                // finished transfers showing a full progress bar forever.
                if let Some(m) = self.phone.moved.iter_mut().find(|m| m.transfer == transfer) {
                    if let (Ok(p), false) = (&result, upload) {
                        m.name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    }
                    m.result = Some(result.clone());
                    m.at = SystemTime::now();
                }
                if let Some(request) = self.phone.backup_requests.remove(&transfer) {
                    let received = result.clone();
                    let device = id.clone();
                    std::thread::spawn(move || {
                        let saved = received.and_then(|path| crate::connect::deduplicate_backup(&path).map_err(|e| e.to_string()));
                        if let Some(s) = service() {
                            s.send_packet(&device, "echofiles.backup.ack", ef_phone::json!({"requestId":request,"saved":saved.is_ok()}));
                        }
                    });
                }
                if let Some(remote) = self.phone.remote_thumb.remove(&transfer) {
                    self.phone.photo_jobs = self.phone.photo_jobs.saturating_sub(1);
                    self.phone.photo_thumbs.insert(remote.clone(), result.as_ref().ok().map(iced::advanced::image::Handle::from_path));
                    let screen = result.as_ref().map_or_else(|_| Task::none(), |file| self.thumb_arrived(&remote, file));
                    return Task::batch([self.pump_photo_thumbs(), screen]);
                }
                if let Some(kind) = self.phone.payloads.remove(&transfer) {
                    if result.is_err() && kind == "photo-copy" {
                        self.phone.copy_photos = None;
                    }
                    if let Ok(path) = &result {
                        if kind == "photo-open" {
                            return self.open_path(path.clone());
                        }
                        if let Some(key) = kind.strip_prefix("photo-import:") {
                            self.phone.imported.insert(key.into());
                            save_lines(imported_file(), self.phone.imported.iter().map(|k| format!("{k}\n")).collect());
                        }
                        if kind == "photo-copy"
                            && let Some((remaining, paths)) = &mut self.phone.copy_photos
                        {
                            paths.push(path.clone());
                            *remaining = remaining.saturating_sub(1);
                            if *remaining == 0 {
                                self.clip = Some(crate::actions::Clip { paths: paths.clone(), cut: false });
                                self.phone.copy_photos = None;
                            }
                        }

                        let path = path.clone();
                        std::thread::spawn(move || {
                            if kind == "echofiles.clipboard.image" {
                                if let Some(png) = std::fs::read(&path).ok().and_then(|bytes| normal_image(&bytes)) {
                                    let _ = event_tx().unbounded_send(PhoneMsg::RemoteImage(png));
                                }
                                let _ = std::fs::remove_file(path);
                            }
                        });
                    }
                }
                match (&result, upload) {
                    (Err(e), true) => self.toast_error(format!("Couldn't send to {}", self.phone_name(&id)), e.clone()),
                    (Err(e), false) => self.toast_error(format!("Couldn't receive from {}", self.phone_name(&id)), e.clone()),
                    (Ok(p), false) => {
                        let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
                        self.push_toast(
                            Tone::Accent,
                            format!("Received “{name}”"),
                            Some(format!("From {} · in {}", self.phone_name(&id), self.settings.phone.save_to)),
                            Some(("Show".into(), Message::Phone(PhoneMsg::OpenSaveFolder))),
                        );
                        let dir = p.parent().map(Path::to_path_buf);
                        if let Some(d) = dir {
                            return self.refresh(&d);
                        }
                    }
                    (Ok(_), true) => {}
                }
                Task::none()
            }
        }
    }
}

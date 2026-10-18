//! What the user does on the phone pages (PhoneMsg): pairing, files, photos, texts,
//! notifications, clipboard and Settings → Phone.

use super::*;

impl App {
    pub(crate) fn phone_update(&mut self, msg: PhoneMsg) -> Task<Message> {
        if let PhoneMsg::Scoped(generation, inner) = msg {
            let selected = self.phone.active_id.clone();
            let selected_current = self.phone.current.clone();
            let id = if self.phone.generation == generation {
                Some(selected.clone())
            } else {
                self.phone.states.iter().find(|(_, s)| s.generation == generation).map(|(id, _)| id.clone())
            };
            if let Some(id) = id {
                if id != selected && matches!(&*inner, PhoneMsg::Mounted(_)) {
                    if let Some(state) = self.phone.states.get_mut(&id) {
                        state.mounting = false;
                        state.want = None;
                    }
                    if let PhoneMsg::Mounted(Ok((m, _))) = *inner {
                        std::thread::spawn(move || m.unmount());
                    }
                    return Task::none();
                }
                self.activate_phone(&id);
                self.phone.current = Some(id);
                let task = self.phone_update(*inner);
                self.activate_phone(&selected);
                self.phone.current = selected_current;
                return task;
            }
            if let PhoneMsg::Mounted(Ok((m, _))) = *inner {
                std::thread::spawn(move || m.unmount());
            }
            return Task::none();
        }
        match msg {
            PhoneMsg::Scoped(..) => unreachable!(),
            PhoneMsg::RemoteImage(bytes) => {
                if self.settings.phone.clipboard {
                    self.phone.last_image = bytes.clone();
                    std::thread::spawn(move || set_image(&bytes));
                }
                Task::none()
            }
            PhoneMsg::LocalDnd(enabled) => {
                if self.phone.dnd != enabled {
                    self.phone.dnd = enabled;
                    if self.settings.phone.dnd_sync
                        && let (Some(s), Some(id)) = (service(), self.phone_id())
                    {
                        s.send_packet(&id, "echofiles.dnd", ef_phone::json!({"enabled":enabled}));
                    }
                }
                Task::none()
            }
            PhoneMsg::ExtraField(field, value) => {
                match field {
                    0 => self.phone.extra_address = value,
                    1 => self.phone.extra_code = value,
                    _ => self.phone.extra_connect = value,
                }
                Task::none()
            }
            PhoneMsg::ExtraDone(result) => {
                self.phone.extra_status = result.unwrap_or_else(|e| e);
                self.phone.extra_code.clear();
                self.phone.clipboard_qr = None;
                Task::none()
            }
            PhoneMsg::Attachment(body) => {
                if let (Some(s), Some(id)) = (service(), self.phone_id()) {
                    s.send_packet(&id, "kdeconnect.sms.request_attachment", body);
                }
                Task::none()
            }
            PhoneMsg::NotificationAction(key, action) => {
                if let (Some(s), Some(id)) = (service(), self.phone_id()) {
                    s.send_packet(&id, "kdeconnect.notification.action", ef_phone::json!({"key":key,"action":action}));
                }
                Task::none()
            }
            PhoneMsg::Extra(action) => {
                if action == "clipboard-qr" {
                    let ip = self.phone_id().and_then(|id| self.phone.devices.get(&id)).map(|d| d.ip);
                    if let Some(ip) = ip {
                        match crate::connect::clipboard_qr() {
                            Ok((svg, name, secret)) => {
                                self.phone.clipboard_qr = Some(iced::widget::svg::Handle::from_memory(svg));
                                self.phone.extra_status = "On the phone: Wireless debugging → Pair device with QR code. Scan this code; it expires in two minutes.".into();
                                return background(
                                    move || crate::connect::grant_clipboard_qr(&name, &secret, ip),
                                    |r| Message::Phone(PhoneMsg::ExtraDone(r)),
                                );
                            }
                            Err(e) => self.phone.extra_status = e,
                        }
                    }
                    return Task::none();
                }
                if action == "clipboard-setup" {
                    let (address, code, connect) =
                        (self.phone.extra_address.clone(), self.phone.extra_code.clone(), self.phone.extra_connect.clone());
                    self.phone.extra_status = "Pairing wireless debugging…".into();
                    return background(
                        move || {
                            crate::connect::grant_clipboard(&address, &code, &connect).map(|_| {
                                "Permission granted. On the phone, allow Display over other apps and turn Developer options off.".into()
                            })
                        },
                        |r| Message::Phone(PhoneMsg::ExtraDone(r)),
                    );
                }
                if matches!(action.as_str(), "answer" | "hangup" | "phone") {
                    let name = self.phone_id().map(|id| self.phone_name(&id)).unwrap_or_default();
                    let number = self.phone.call.as_ref().and_then(|c| c["phoneNumber"].as_str()).unwrap_or("").to_string();
                    let device = self.phone_id();
                    return background(
                        move || {
                            if action == "phone" {
                                crate::connect::release_headset()?;
                            } else if action == "answer" {
                                let address = crate::connect::connect_headset(&name)?;
                                std::thread::sleep(Duration::from_secs(2));
                                if let Err(e) =
                                    crate::connect::call(&action, &number).and_then(|_| crate::connect::route_call_audio(&address))
                                {
                                    let _ = crate::connect::release_headset();
                                    return Err(e);
                                }
                            } else {
                                crate::connect::call(&action, &number)?;
                                crate::connect::release_headset()?;
                            }
                            if let (Some(s), Some(id)) = (service(), device) {
                                s.send_packet(&id, "echofiles.call", ef_phone::json!({"onLaptop":action == "answer"}));
                            }
                            Ok(if action == "answer" { "Call audio connected to laptop" } else { "Phone audio released" }.into())
                        },
                        |r| Message::Phone(PhoneMsg::ExtraDone(r)),
                    );
                }
                if action == "dnd-sync" {
                    self.settings.phone.dnd_sync = !self.settings.phone.dnd_sync;
                    self.send_phone_settings();
                    return self.persist_settings();
                }
                if action == "otp" {
                    self.settings.phone.otp = !self.settings.phone.otp;
                    return self.persist_settings();
                }
                if action == "autolock" {
                    self.settings.phone.autolock = !self.settings.phone.autolock;
                    self.send_phone_settings();
                    return self.persist_settings();
                }
                if action == "backup" {
                    self.settings.phone.backup = !self.settings.phone.backup;
                    self.send_phone_settings();
                    return self.persist_settings();
                }
                if action == "index" {
                    self.settings.phone.phone_index = !self.settings.phone.phone_index;
                }
                if let (Some(s), Some(id)) = (service(), self.phone_id()) {
                    if action == "media-refresh" {
                        s.send_packet(&id, "kdeconnect.mpris.request", ef_phone::json!({"requestPlayerList":true}));
                    }
                    if let Some((command, player)) = action.strip_prefix("media:").and_then(|v| v.split_once(':')) {
                        s.send_packet(
                            &id,
                            "kdeconnect.mpris.request",
                            ef_phone::json!({"player":player,"action":command,"requestNowPlaying":true}),
                        );
                    }
                    match action.as_str() {
                        "photo" | "scan" => {
                            let request =
                                format!("{}", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis());
                            let destination = self.pane().location.clone();
                            self.phone.captures.insert(request.clone(), destination);
                            s.send_packet(&id, "echofiles.capture.request", ef_phone::json!({"requestId":request,"scan":action == "scan"}));
                            self.phone.extra_status = "Tap the camera notification on your phone.".into();
                        }
                        "index" if self.settings.phone.phone_index => self.start_phone_index(&id),
                        "index" => {
                            self.phone.index_scan = None;
                            let _ = std::fs::remove_file(config::state_dir().join("phone-index").join(format!("{id}.jsonl")));
                        }
                        "dnd" => {
                            s.send_packet(&id, "echofiles.dnd", ef_phone::json!({"enabled":true}));
                            self.phone.dnd = true;
                        }
                        "dnd-off" => {
                            s.send_packet(&id, "echofiles.dnd", ef_phone::json!({"enabled":false}));
                            self.phone.dnd = false;
                        }
                        _ => {}
                    }
                }
                self.persist_settings()
            }
            PhoneMsg::Event(e) => self.phone_event(e),
            PhoneMsg::LocalImage(bytes) => {
                if !self.settings.phone.clipboard || bytes == self.phone.last_image {
                    return Task::none();
                }
                self.phone.last_image = bytes.clone();
                if let Some(s) = service() {
                    for d in self.phone.devices.values().filter(|d| d.paired) {
                        if bytes.len() <= 48 * 1024 {
                            s.send_packet(
                                &d.id,
                                "echofiles.clipboard.image.inline",
                                ef_phone::json!({"data":crate::codec::base64_encode(&bytes),"mime":"image/png"}),
                            );
                        } else {
                            s.send_payload(
                                &d.id,
                                "echofiles.clipboard.image",
                                ef_phone::json!({"filename":"clipboard.png","mime":"image/png"}),
                                ef_phone::Source::Bytes(bytes.clone()),
                            );
                        }
                    }
                }
                Task::none()
            }
            PhoneMsg::LocalClip(text) => {
                if !self.settings.phone.clipboard || text == self.phone.clip_last {
                    return Task::none();
                }
                self.phone.clip_sensitive = false;
                self.phone.clip_last = text.clone();
                let ids: Vec<String> = self.phone.devices.values().filter(|d| d.paired).map(|d| d.id.clone()).collect();
                if let Some(s) = service()
                    && !ids.is_empty()
                {
                    for id in ids {
                        s.send_clipboard(&id, &text, false, false);
                    }
                    self.phone.clip.push_front((false, text, SystemTime::now()));
                    self.phone.clip.truncate(20);
                }
                Task::none()
            }
            PhoneMsg::Open(page) => self.open_phone_page(page),
            PhoneMsg::Select(id) => {
                self.activate_phone(&id);
                self.phone.current = Some(id);
                self.phone.dialog = None;
                self.open_phone_page(PhonePage::Hub)
            }
            PhoneMsg::Dialog(open) => {
                if open && service().is_none() {
                    start();
                }
                if open {
                    self.menu = None;
                    self.command = None;
                    self.phone.dialog = Some(PairDialog {
                        ip: String::new(),
                        opened: Instant::now(),
                        since: Instant::now(),
                        firewall: None,
                        firewall_busy: false,
                        paired: None,
                    });
                    if let Some(s) = service() {
                        s.announce();
                    }
                    background(ufw_active, |on| Message::Phone(PhoneMsg::Ufw(on)))
                } else {
                    self.phone.dialog = None;
                    // Waiting on the phone's answer: the request simply lapses.
                    if self.phone.code.as_ref().is_some_and(|c| !c.2) {
                        self.phone.code = None;
                    }
                    Task::none()
                }
            }
            PhoneMsg::Pair(id) => {
                if let Some(s) = service() {
                    s.pair(&id);
                }
                Task::none()
            }
            PhoneMsg::AcceptPair => {
                if let (Some((id, code, true)), Some(s)) = (self.phone.code.clone(), service()) {
                    s.accept_pair_code(&id, &code);
                }
                Task::none()
            }
            PhoneMsg::RejectPair => {
                // Codes don't match, or we stop waiting: either way the phone is told.
                if let Some((id, _, _)) = self.phone.code.take()
                    && let Some(s) = service()
                {
                    s.reject_pair(&id);
                }
                Task::none()
            }
            PhoneMsg::Forget(id) => {
                if let Some(s) = service() {
                    s.unpair(&id);
                }
                Task::none()
            }
            PhoneMsg::IpDraft(t) => {
                if let Some(d) = &mut self.phone.dialog {
                    d.ip = t;
                }
                Task::none()
            }
            PhoneMsg::IpSubmit => {
                let Some(d) = &self.phone.dialog else { return Task::none() };
                let raw = d.ip.trim();
                let addr: Option<SocketAddr> =
                    raw.parse::<SocketAddr>().ok().or_else(|| raw.parse::<IpAddr>().ok().map(|ip| SocketAddr::new(ip, ef_phone::UDP_PORT)));
                match (addr, service()) {
                    (Some(a), Some(s)) => {
                        s.connect_to(a);
                        s.announce_to(SocketAddr::new(a.ip(), ef_phone::UDP_PORT));
                    }
                    (None, _) => self.toast_error(
                        "That isn't an IP address".into(),
                        "Type the phone's address, like 192.168.1.20 — it's in the phone's Wi-Fi settings, under the network's details."
                            .into(),
                    ),
                    _ => {}
                }
                Task::none()
            }
            PhoneMsg::AllowFirewall => {
                if let Some(d) = &mut self.phone.dialog {
                    d.firewall_busy = true;
                }
                background(allow_firewall, |r| Message::Phone(PhoneMsg::Firewall(r)))
            }
            PhoneMsg::Firewall(r) => {
                if let Some(d) = &mut self.phone.dialog {
                    d.firewall_busy = false;
                    d.firewall = Some(r);
                }
                if let Some(s) = service() {
                    s.announce();
                }
                Task::none()
            }
            PhoneMsg::Ufw(on) => {
                self.phone.ufw = Some(on);
                Task::none()
            }
            PhoneMsg::Files(want) => self.phone_files(want),
            PhoneMsg::SendFiles => {
                let name = self.phone_id().map(|i| self.phone_name(&i)).unwrap_or_else(|| "phone".into());
                let generation = self.phone.generation;
                background(move || portal_pick(&format!("Send to {name}"), false), move |r| scoped(generation, PhoneMsg::Picked(r)))
            }
            PhoneMsg::SendPaths(paths) => self.send_to_phone(paths),
            PhoneMsg::Picked(Ok(paths)) => {
                if paths.is_empty() {
                    return Task::none();
                }
                self.send_to_phone(paths)
            }
            PhoneMsg::Picked(Err(e)) => {
                self.toast_error("Couldn't open the file chooser".into(), e);
                Task::none()
            }
            PhoneMsg::Ring => {
                let Some(id) = self.phone_id() else { return Task::none() };
                if let Some(s) = service()
                    && s.ring(&id)
                {
                    self.phone.ringing = if self.phone.ringing.is_some() { None } else { Some(Instant::now()) };
                }
                Task::none()
            }
            PhoneMsg::Mounted(Ok((m, storage))) => {
                let generation = self.phone.generation;
                self.phone.mounting = false;
                self.phone.mount = Some((m, storage.clone()));
                let want = self.phone.want.unwrap_or(Want::Nothing);
                let t = self.after_mount(want, &storage);
                let s2 = storage.clone();
                let space = background(move || crate::phone_view::fs_space(&s2), move |r| scoped(generation, PhoneMsg::Space(r)));
                Task::batch([t, space])
            }
            PhoneMsg::Space(s) => {
                self.phone.space = s;
                Task::none()
            }
            PhoneMsg::Mounted(Err(e)) => {
                self.phone.mounting = false;
                self.phone.sftp = None;
                let wanted = self.phone.want.take().is_some_and(|w| w != Want::Nothing);
                self.phone.sftp_error = Some(e.clone());
                if wanted {
                    self.toast_error("Couldn't open the phone's files".into(), e.clone());
                }
                Task::none()
            }
            PhoneMsg::RetryPhotos => {
                self.phone.photos = None;
                self.phone.photos_loading = false;
                self.request_photos();
                Task::none()
            }
            PhoneMsg::Screen(path, b64) => {
                if let Some(b) = b64 {
                    self.phone.screen = Some((path, b));
                }
                Task::none()
            }
            PhoneMsg::PhotoToggle(p) => {
                if !self.phone.photo_sel.remove(&p) {
                    self.phone.photo_sel.insert(p);
                }
                Task::none()
            }
            PhoneMsg::PhotoOpen(p) => {
                if let Some(id) = self.phone.remote_photos.get(&p).copied() {
                    self.request_remote_photo(id, config::state_dir().join("phone-photos"), "photo-open");
                    Task::none()
                } else {
                    self.open_path(p)
                }
            }
            PhoneMsg::PhotosClear => {
                self.phone.photo_sel.clear();
                Task::none()
            }
            PhoneMsg::PhotosFilter(f) => {
                self.phone.photo_filter = f;
                self.phone.photo_limit = PHOTOS_PAGE;
                self.want_photo_thumbs()
            }
            PhoneMsg::PhotoQueue => self.pump_photo_thumbs(),
            PhoneMsg::PhotosMore => {
                if self.phone.photo_more
                    && let (Some(s), Some(id), Some((before, before_id))) = (service(), self.phone_id(), self.phone.photo_before)
                {
                    s.send_packet(&id, "echofiles.photos.request", ef_phone::json!({"before":before,"beforeId":before_id}));
                }
                self.phone.photo_limit += PHOTOS_PAGE;
                self.want_photo_thumbs()
            }
            PhoneMsg::PhotosCopy => {
                let paths: Vec<PathBuf> = self.phone.photo_sel.iter().cloned().collect();
                let remote: Vec<u64> = paths.iter().filter_map(|p| self.phone.remote_photos.get(p).copied()).collect();
                if !remote.is_empty() {
                    self.phone.copy_photos = Some((remote.len(), Vec::new()));
                    for id in remote {
                        self.request_remote_photo(id, config::state_dir().join("phone-photos"), "photo-copy");
                    }
                    self.phone.extra_status = "Downloading photos before copying…".into();
                    return Task::none();
                }
                if !paths.is_empty() {
                    let n = paths.len();
                    self.clip = Some(crate::actions::Clip { paths, cut: false });
                    self.push_toast(Tone::Accent, format!("{n} copied"), Some("Open any folder and paste with Ctrl+V.".into()), None);
                }
                Task::none()
            }
            PhoneMsg::PhotosSaveTo => {
                let generation = self.phone.generation;
                background(
                    || portal_pick("Save photos to", true).map(|v| v.into_iter().next()),
                    move |r| scoped(generation, PhoneMsg::SaveFolder(r)),
                )
            }
            PhoneMsg::SaveFolder(Ok(Some(dest))) => {
                let sources: Vec<PathBuf> = self.phone.photo_sel.drain().collect();
                let remote: Vec<u64> = sources.iter().filter_map(|p| self.phone.remote_photos.get(p).copied()).collect();
                if !remote.is_empty() {
                    for id in remote {
                        self.request_remote_photo(id, dest.clone(), "photo-save");
                    }
                    return Task::none();
                }
                if sources.is_empty() {
                    return Task::none();
                }
                self.file_update(FileMsg::Start { kind: ef_core::ops::Kind::Copy, sources, dest })
            }
            PhoneMsg::SaveFolder(Ok(None)) => Task::none(),
            PhoneMsg::SaveFolder(Err(e)) => {
                self.toast_error("Couldn't open the folder chooser".into(), e);
                Task::none()
            }
            PhoneMsg::Import => {
                let new: Vec<Photo> = self.photos_new().into_iter().cloned().collect();
                if new.is_empty() {
                    return Task::none();
                }
                let base = config::home().join("Pictures/Phone");
                let remote: Vec<(u64, String, String)> =
                    new.iter()
                        .filter_map(|p| {
                            self.phone.remote_photos.get(&p.path).map(|id| {
                                (*id, month_of(p.mtime), import_key(&self.phone_id().unwrap_or_default(), self.phone_storage(), p))
                            })
                        })
                        .collect();
                if !remote.is_empty() {
                    for (id, month, key) in remote {
                        self.request_remote_photo(id, base.join(month), &format!("photo-import:{key}"));
                    }
                    return Task::none();
                }
                let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
                for p in &new {
                    groups.entry(month_of(p.mtime)).or_default().push(p.path.clone());
                    let key = import_key(&self.phone_id().unwrap_or_default(), self.phone_storage(), p);
                    self.phone.import_pending.insert(p.path.clone(), key);
                }
                let mut tasks = Vec::new();
                for (month, sources) in groups {
                    let dest = base.join(month);
                    let generation = self.phone.generation;
                    tasks.push(background(
                        move || std::fs::create_dir_all(&dest).map(|_| (sources, dest)).map_err(|e| e.to_string()),
                        move |r| scoped(generation, PhoneMsg::ImportReady(r)),
                    ));
                }
                Task::batch(tasks)
            }
            PhoneMsg::ImportReady(Ok((sources, dest))) => {
                self.file_update(FileMsg::Start { kind: ef_core::ops::Kind::Copy, sources, dest })
            }
            PhoneMsg::ImportReady(Err(e)) => {
                self.toast_error("Couldn't prepare the photo import".into(), e);
                Task::none()
            }
            PhoneMsg::Thread(t) => {
                self.phone.open_thread = Some(t);
                if let (Some(id), Some(s)) = (self.phone_id(), service()) {
                    s.request_thread(&id, t);
                }
                iced::widget::operation::focus(SMS_ID)
            }
            PhoneMsg::Compose(t) => {
                self.phone.compose = t;
                Task::none()
            }
            PhoneMsg::SendSms => {
                let text = self.phone.compose.trim().to_string();
                let (Some(t), Some(id)) = (self.phone.open_thread, self.phone_id()) else { return Task::none() };
                if text.is_empty() {
                    return Task::none();
                }
                let addresses: Vec<String> =
                    self.phone.threads.get(&t).map(|m| m.address.split(", ").map(str::to_string).collect()).unwrap_or_default();
                if let Some(s) = service()
                    && s.send_sms(&id, &addresses, &text)
                {
                    let sent = Sms {
                        thread: t,
                        uid: -(now_s()),
                        address: addresses.join(", "),
                        body: text,
                        date: now_s() * 1000,
                        outgoing: true,
                        read: true,
                        attachments: Vec::new(),
                    };
                    self.phone.thread_msgs.entry(t).or_default().push(sent.clone());
                    self.phone.threads.insert(t, sent);
                    self.phone.compose.clear();
                }
                Task::none()
            }
            PhoneMsg::RefreshMessages => {
                if let (Some(id), Some(s)) = (self.phone_id(), service()) {
                    s.request_conversations(&id);
                }
                Task::none()
            }
            PhoneMsg::ClipboardSetup(open) => {
                self.phone.clipboard_setup = open;
                Task::none()
            }
            PhoneMsg::ToggleNotification(key) => {
                if !self.phone.expanded.remove(&key) {
                    self.phone.expanded.insert(key);
                }
                Task::none()
            }
            PhoneMsg::Dismiss(key) => {
                if let (Some(id), Some(s)) = (self.phone_id(), service()) {
                    s.dismiss_notification(&id, &key);
                }
                self.phone.notifications.retain(|n| n.key != key);
                Task::none()
            }
            PhoneMsg::DismissAll => {
                if let (Some(id), Some(s)) = (self.phone_id(), service()) {
                    for n in self.phone.notifications.iter().filter(|n| n.dismissable) {
                        s.dismiss_notification(&id, &n.key);
                    }
                }
                self.phone.notifications.retain(|n| !n.dismissable);
                Task::none()
            }
            PhoneMsg::ReplyDraft(key, t) => {
                self.phone.reply.insert(key, t);
                Task::none()
            }
            PhoneMsg::Reply(key) => {
                let text = self.phone.reply.get(&key).cloned().unwrap_or_default();
                let reply_id = self.phone.notifications.iter().find(|n| n.key == key).and_then(|n| n.reply_id.clone());
                if let (Some(rid), Some(id), Some(s)) = (reply_id, self.phone_id(), service())
                    && !text.trim().is_empty()
                    && s.reply_notification(&id, &rid, text.trim())
                {
                    self.phone.reply.remove(&key);
                }
                Task::none()
            }
            PhoneMsg::ClipCopy(text) => {
                self.phone.clip_last = text.clone();
                std::thread::spawn(move || set_clipboard(&text));
                Task::none()
            }
            PhoneMsg::AcceptOffer(t) => {
                if let Some(s) = service() {
                    s.accept_file(t, config::expand(&self.settings.phone.save_to));
                }
                self.phone.offers.retain(|o| o.0 != t);
                self.toasts.retain(|x| !matches!(&x.action, Some((_, Message::Phone(PhoneMsg::AcceptOffer(id)))) if *id == t));
                Task::none()
            }
            PhoneMsg::DeclineOffer(t) => {
                if let Some(s) = service() {
                    s.reject_file(t);
                }
                self.phone.offers.retain(|o| o.0 != t);
                Task::none()
            }
            PhoneMsg::OpenFile(p) => self.open_path(p),
            PhoneMsg::OpenSaveFolder => {
                let dir = config::expand(&self.settings.phone.save_to);
                background(
                    move || std::fs::create_dir_all(&dir).map(|_| dir).map_err(|e| e.to_string()),
                    |r| Message::Phone(PhoneMsg::SaveDirReady(r)),
                )
            }
            PhoneMsg::SaveDirReady(Ok(dir)) => self.go(dir, true),
            PhoneMsg::SaveDirReady(Err(e)) => {
                self.toast_error("Couldn't open the receive folder".into(), e);
                Task::none()
            }
            PhoneMsg::SetFiles(on) => {
                self.settings.phone.files = on;
                if !on {
                    for state in self.phone.states.values_mut() {
                        state.generation = crate::pane::next_id();
                        state.want = None;
                        state.mounting = false;
                        state.photos = None;
                        state.photos_loading = false;
                        if let Some((m, _)) = state.mount.take() {
                            std::thread::spawn(move || m.unmount());
                        }
                    }
                }
                self.send_phone_settings();
                let t = if on { Task::none() } else { self.drop_phone_mount() };
                Task::batch([t, self.persist_settings()])
            }
            PhoneMsg::SetClipboard(on) => {
                self.settings.phone.clipboard = on;
                self.send_phone_settings();
                self.persist_settings()
            }
            PhoneMsg::SetMessages(on) => {
                self.settings.phone.messages = on;
                self.send_phone_settings();
                if !on {
                    self.phone.threads.clear();
                    self.phone.thread_msgs.clear();
                } else if let (Some(id), Some(s)) = (self.phone_id(), service()) {
                    s.request_conversations(&id);
                }
                self.persist_settings()
            }
            PhoneMsg::SetNotifications(n) => {
                self.settings.phone.notifications = n;
                self.send_phone_settings();
                if n == PhoneNotifications::Off {
                    self.phone.notifications.clear();
                }
                self.persist_settings()
            }
            PhoneMsg::SetBatteryWarning(on) => {
                self.settings.phone.battery_warning = on;
                self.persist_settings()
            }
            PhoneMsg::SetAutoAccept(on) => {
                self.settings.phone.auto_accept = on;
                self.persist_settings()
            }
            PhoneMsg::SetPicture(p) => {
                self.settings.phone.picture = p;
                self.persist_settings()
            }
            PhoneMsg::ChangeSaveTo => background(
                || portal_pick("Save files from the phone to", true).map(|v| v.into_iter().next()),
                |r| match r {
                    Ok(Some(p)) => Message::Phone(PhoneMsg::SaveTo(p)),
                    _ => Message::Tick,
                },
            ),
            PhoneMsg::SaveTo(p) => {
                self.settings.phone.save_to = config::tilde(&p);
                self.persist_settings()
            }
            PhoneMsg::MuteDraft(t) => {
                self.phone.mute_draft = t;
                Task::none()
            }
            PhoneMsg::MuteAdd => {
                let name = self.phone.mute_draft.trim().to_string();
                if !name.is_empty() && !self.settings.phone.muted_apps.iter().any(|a| a.eq_ignore_ascii_case(&name)) {
                    self.settings.phone.muted_apps.push(name);
                    self.phone.mute_draft.clear();
                    return self.persist_settings();
                }
                Task::none()
            }
            PhoneMsg::MuteRemove(i) => {
                if i < self.settings.phone.muted_apps.len() {
                    self.settings.phone.muted_apps.remove(i);
                }
                self.persist_settings()
            }
        }
    }
}

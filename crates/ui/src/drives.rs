//! Windows drives in EchoFiles (build plan §3; design system `DriveItem`, `DriveCard`,
//! `Banner`, `StatePill`): mounting with ntfs3 through udisks, the password prompt,
//! read-only states, and the Drives overview (Ctrl+Shift+D).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use ef_config as config;
use ef_core::fmt;
use ef_core::volume::FsInfo;
use ef_disks::polkit::Prompt;
use ef_disks::{MountError, Mounted, Volume};
use iced::futures::channel::mpsc;
use iced::widget::{column, container, row, scrollable, text, Space};
use iced::{Alignment, Background, Border, Element, Length, Point, Task};

use crate::app::{background, fs_of, App, Message};
use crate::overlay::{Dialog, MenuFor, Tone};
use crate::style::{self, color};
use crate::widgets as w;

/// What EchoFiles knows about a drive beyond udisks' facts. Keyed by device path.
#[derive(Debug, Clone, Default)]
pub struct DriveState {
    pub mounting: bool,
    pub read_only: bool,
    /// Why it's read-only when nobody asked (Fast Startup, hibernation).
    pub reason: Option<String>,
    /// Writing to the Windows system drive was allowed for this mount.
    pub write_allowed: bool,
}

#[derive(Debug, Clone)]
pub enum DriveMsg {
    Volumes(Result<Vec<Volume>, String>),
    Animations(bool),
    Click(usize),
    Mounted { device: String, result: Result<Mounted, MountError>, open: bool },
    Unmount(usize),
    Unmounted { device: String, result: Result<(), MountError> },
    AskWrite(usize),
    AllowWrite(usize),
    Menu(usize, Point),
    Prompt(Prompt),
}

/// "Windows (C:)", "AVS (D:)", "Local Disk (E:)"; the size tells two same-named drives
/// apart until their letters are known.
pub fn display_name(v: &Volume, all: &[Volume]) -> String {
    let base = if !v.label.is_empty() {
        v.label.clone()
    } else if v.system {
        "Windows".to_string()
    } else {
        "Local Disk".to_string()
    };
    match v.letter {
        Some(l) => format!("{base} ({l}:)"),
        None if all.iter().filter(|o| o.label == v.label && o.letter.is_none()).count() > 1 => {
            let mut s = format!("{base} · ");
            fmt::size(v.size, &mut s);
            s
        }
        None => base,
    }
}

static PROMPTS: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<Prompt>>>> = OnceLock::new();
static PROMPT_TX: OnceLock<mpsc::UnboundedSender<Prompt>> = OnceLock::new();

fn prompt_tx() -> mpsc::UnboundedSender<Prompt> {
    PROMPT_TX
        .get_or_init(|| {
            let (tx, rx) = mpsc::unbounded();
            let _ = PROMPTS.set(Mutex::new(Some(rx)));
            tx
        })
        .clone()
}

/// Password prompts from the polkit agent, as a subscription stream (taken once).
pub fn prompts() -> impl iced::futures::Stream<Item = Prompt> {
    let _ = prompt_tx();
    let rx = PROMPTS.get().and_then(|m| m.lock().unwrap().take());
    iced::futures::stream::unfold(rx, |rx| async move {
        use iced::futures::StreamExt;
        let mut rx = rx?;
        let item = rx.next().await?;
        Some((item, Some(rx)))
    })
}

/// Make sure EchoFiles can answer password questions about its own mounts.
fn ensure_agent() {
    let tx = prompt_tx();
    if let Err(e) = ef_disks::polkit::ensure(move |p| {
        let _ = tx.unbounded_send(p);
    }) {
        eprintln!("polkit agent: {e}");
    }
}

impl App {
    fn volumes_task(&self) -> Task<Message> {
        background(|| ef_disks::windows_volumes().map_err(|e| e.to_string()), |r| Message::Drive(DriveMsg::Volumes(r)))
    }

    fn mount_task(&mut self, i: usize, open: bool) -> Task<Message> {
        let Some(v) = self.volumes.get(i).cloned() else { return Task::none() };
        self.drive_state.entry(v.device.clone()).or_default().mounting = true;
        let device = v.device.clone();
        background(
            move || {
                ensure_agent();
                ef_disks::mount(&v, false)
            },
            move |result| Message::Drive(DriveMsg::Mounted { device: device.clone(), result, open }),
        )
    }

    pub(crate) fn drive_update(&mut self, msg: DriveMsg) -> Task<Message> {
        match msg {
            DriveMsg::Volumes(Ok(v)) => {
                self.volume_fs = v.iter().map(|v| v.mount_points.first().and_then(|m| fs_of(Path::new(m)))).collect();
                for (vol, fs) in v.iter().zip(&self.volume_fs) {
                    let st = self.drive_state.entry(vol.device.clone()).or_default();
                    st.read_only = fs.as_ref().is_some_and(|f| f.read_only);
                    if !vol.is_mounted() {
                        st.reason = None;
                        st.write_allowed = false;
                    }
                }
                self.volumes = v;
                Task::none()
            }
            DriveMsg::Volumes(Err(e)) => {
                self.notice = Some(e);
                Task::none()
            }
            DriveMsg::Animations(on) => {
                self.animations = on;
                Task::none()
            }
            DriveMsg::Click(i) => {
                let Some(v) = self.volumes.get(i) else { return Task::none() };
                if v.locked {
                    self.push_toast(Tone::Accent, format!("{} is locked with BitLocker", display_name(v, &self.volumes)), Some("Unlock it in Windows (or turn BitLocker off there) to open it here. Unlocking from Linux is coming.".into()), None);
                    return Task::none();
                }
                match v.mount_points.first() {
                    Some(mp) => {
                        let mp = PathBuf::from(mp);
                        self.go(mp, true)
                    }
                    None if self.drive_state.get(&v.device).is_some_and(|s| s.mounting) => Task::none(),
                    None => self.mount_task(i, true),
                }
            }
            DriveMsg::Mounted { device, result, open } => {
                let name = self.volumes.iter().find(|v| v.device == device).map(|v| display_name(v, &self.volumes)).unwrap_or_else(|| device.clone());
                let st = self.drive_state.entry(device.clone()).or_default();
                st.mounting = false;
                match result {
                    Ok(m) => {
                        st.read_only = m.read_only;
                        st.reason = m.forced_read_only.clone();
                        if let Some(r) = &m.forced_read_only {
                            self.push_toast(Tone::Accent, format!("{name} opened read-only"), Some(r.clone()), None);
                        }
                        let t = self.volumes_task();
                        if open {
                            let go = self.go(PathBuf::from(&m.mount_point), true);
                            return Task::batch([t, go]);
                        }
                        t
                    }
                    Err(MountError::NotAuthorized) => {
                        self.push_toast(Tone::Accent, format!("{name} wasn't mounted"), Some("Mounting a drive inside this computer needs your password.".into()), None);
                        Task::none()
                    }
                    Err(MountError::Failed(e)) => {
                        self.toast_error(format!("Couldn't mount {name}"), e);
                        Task::none()
                    }
                }
            }
            DriveMsg::Unmount(i) => {
                let Some(v) = self.volumes.get(i).cloned() else { return Task::none() };
                // Panes showing the drive move out first so nothing holds it open.
                let mps: Vec<PathBuf> = v.mount_points.iter().map(PathBuf::from).collect();
                let home = config::home();
                let mut tasks = Vec::new();
                let show_hidden = self.show_hidden;
                for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()) {
                    if mps.iter().any(|m| pane.location.starts_with(m)) {
                        pane.location = home.clone();
                        pane.back.clear();
                        pane.forward.clear();
                        tasks.push(pane.load(home.clone(), show_hidden));
                    }
                }
                let device = v.device.clone();
                tasks.push(background(
                    move || {
                        ensure_agent();
                        ef_disks::unmount(&v)
                    },
                    move |result| Message::Drive(DriveMsg::Unmounted { device: device.clone(), result }),
                ));
                Task::batch(tasks)
            }
            DriveMsg::Unmounted { device, result } => {
                let name = self.volumes.iter().find(|v| v.device == device).map(|v| display_name(v, &self.volumes)).unwrap_or_else(|| device.clone());
                match result {
                    Ok(()) => {
                        self.drive_state.remove(&device);
                    }
                    Err(e) => self.toast_error(format!("Couldn't unmount {name}"), format!("{e}. Close files and terminals that are using it, then try again.")),
                }
                self.volumes_task()
            }
            DriveMsg::AskWrite(i) => {
                self.dialog = Some(Dialog::WriteSystem { drive: i, opened: Instant::now() });
                Task::none()
            }
            DriveMsg::AllowWrite(i) => {
                let Some(v) = self.volumes.get(i).cloned() else { return Task::none() };
                let device = v.device.clone();
                let st = self.drive_state.entry(device.clone()).or_default();
                st.mounting = true;
                st.write_allowed = true;
                background(
                    move || {
                        ensure_agent();
                        ef_disks::remount(&v, false)
                    },
                    move |result| Message::Drive(DriveMsg::Mounted { device: device.clone(), result, open: true }),
                )
            }
            DriveMsg::Menu(i, at) => {
                self.open_menu(MenuFor::Drive(i), at);
                Task::none()
            }
            DriveMsg::Prompt(p) => {
                self.menu = None;
                self.dialog = Some(Dialog::Password { prompt: p, value: String::new(), opened: Instant::now() });
                iced::widget::operation::focus(crate::overlay::PASSWORD_ID)
            }
        }
    }

    /// The Windows volume holding `path`, with its index.
    pub(crate) fn volume_at(&self, path: &Path) -> Option<(usize, &Volume)> {
        self.volumes.iter().enumerate().find(|(_, v)| v.mount_points.iter().any(|m| path.starts_with(m)))
    }

    // ------------------------------------------------------------------ views

    /// A fact about the whole drive the open folder is on (design system `Banner`).
    pub(crate) fn drive_banner(&self) -> Option<Element<'_, Message>> {
        let pane = self.pane();
        if pane.drives {
            return None;
        }
        let (i, v) = self.volume_at(&pane.location)?;
        let st = self.drive_state.get(&v.device).cloned().unwrap_or_default();
        let name = display_name(v, &self.volumes);
        let p = &self.palette;
        let (tone, icon, msg, action): (ef_theme::Semantic, &str, String, Option<Element<'_, Message>>) = if let Some(r) = &st.reason {
            (p.warning, "alert", format!("{name} is read-only. {r} Restart Windows (not Shut down) or turn off Fast Startup to write here."), None)
        } else if st.write_allowed && v.system {
            (p.warning, "unlock", format!("Writing to {name} is on until the drive is unmounted."), None)
        } else if v.system && pane.fs.as_ref().is_some_and(|f| f.read_only) {
            (p.info, "shield", format!("{name} is read-only to protect Windows."), Some(w::text_button(p, &self.icons, "Allow writing…", Some("unlock"), None, w::Variant::Secondary, Some(Message::Drive(DriveMsg::AskWrite(i))))))
        } else {
            return None;
        };
        let mut r = row![w::glyph(&self.icons, icon, 16.0, color(tone.ink)), text(msg).size(style::META).font(style::FONT).color(color(p.ink)).width(Length::Fill)].spacing(style::SPACE_4).align_y(Alignment::Center);
        if let Some(a) = action {
            r = r.push(a);
        }
        let bg = color(tone.soft);
        Some(container(r).width(Length::Fill).padding([6, 16]).style(move |_| container::Style { background: Some(Background::Color(bg)), ..Default::default() }).into())
    }

    /// Sidebar row: name, then the state pill before the size on the second line.
    pub(crate) fn drive_item(&self, i: usize, v: &Volume) -> Element<'_, Message> {
        let p = &self.palette;
        let pane = self.pane();
        let active = !pane.drives && v.mount_points.iter().any(|m| pane.location.starts_with(m.as_str()));
        let fs = self.volume_fs.get(i).cloned().flatten();
        let st = self.drive_state.get(&v.device).cloned().unwrap_or_default();
        let ink = if active { color(p.ink_strong) } else if v.is_mounted() { color(p.ink) } else { color(p.ink_muted) };
        let head = text(display_name(v, &self.volumes)).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(ink).wrapping(text::Wrapping::None);
        let mut meta = String::new();
        // With a pill on the line there's only room for the free space; the total is in
        // the tooltip and on the Drives page.
        let pill = st.mounting || v.locked || !v.is_mounted() || st.reason.is_some() || fs.as_ref().is_some_and(|f| f.read_only);
        match &fs {
            Some(f) => {
                fmt::size(f.free, &mut meta);
                meta.push_str(" free");
                if !pill {
                    meta.push_str(" of ");
                    fmt::size(f.total, &mut meta);
                }
            }
            None => fmt::size(v.size, &mut meta),
        }
        let mut lines = column![head].spacing(3);
        if let Some(f) = &fs {
            lines = lines.push(w::usage_bar(p, f.used_fraction(), p.world_windows));
        }
        let mut meta_row = row![].spacing(style::SPACE_3).align_y(Alignment::Center);
        if st.mounting {
            meta_row = meta_row.push(w::pill(p, "Mounting…", Some(p.info)));
        } else if v.locked {
            meta_row = meta_row.push(w::pill(p, "Locked", Some(p.danger)));
        } else if !v.is_mounted() {
            meta_row = meta_row.push(w::pill(p, "Not mounted", None));
        } else if st.reason.is_some() {
            meta_row = meta_row.push(w::pill(p, "Needs check", Some(p.warning)));
        } else if fs.as_ref().is_some_and(|f| f.read_only) {
            meta_row = meta_row.push(w::pill(p, "Read-only", Some(p.warning)));
        }
        meta_row = meta_row.push(text(meta).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None));
        lines = lines.push(meta_row);
        let tint = if active { color(p.accent_ink) } else { color(p.ink_muted) };
        let r = row![container(w::glyph(&self.icons, if v.locked { "lock" } else { "drive" }, 16.0, tint)).padding([2, 0]), lines].spacing(style::SPACE_4).align_y(Alignment::Start);
        let item = w::row_button(p, container(r).padding([6, 0]).into(), active, Some(Message::Drive(DriveMsg::Click(i))));
        let tip = match v.mount_points.first() {
            Some(mp) => {
                let mut t = format!("{mp} · {} · ", fs.as_ref().map_or("ntfs3", |f| f.fs_type));
                if let Some(f) = &fs {
                    fmt::size(f.free, &mut t);
                    t.push_str(" free of ");
                    fmt::size(f.total, &mut t);
                }
                t
            }
            None => format!("{} · {}", v.device, if v.locked { "BitLocker" } else { "NTFS" }),
        };
        let item = self.tip(item, &tip, None);
        w::context_area(item, move |at| Message::Drive(DriveMsg::Menu(i, at)))
    }

    #[allow(clippy::too_many_arguments)]
    fn drive_card<'a>(&'a self, name: String, icon: &'static str, facts: String, fs: Option<FsInfo>, size: u64, world: ef_theme::Rgb, pill: Option<Element<'a, Message>>, msg: Message, menu: Option<usize>) -> Element<'a, Message> {
        let p = &self.palette;
        let mut free = String::new();
        match &fs {
            Some(f) => {
                fmt::size(f.free, &mut free);
                free.push_str(" free of ");
                fmt::size(f.total, &mut free);
            }
            None => {
                fmt::size(size, &mut free);
                free.push_str(" · not mounted");
            }
        }
        let mut head = row![text(name).size(15).font(style::FONT_BOLD).color(color(p.ink_strong)).width(Length::Fill).wrapping(text::Wrapping::None)].align_y(Alignment::Center);
        if let Some(pl) = pill {
            head = head.push(pl);
        }
        let used = fs.as_ref().map_or(0.0, |f| f.used_fraction());
        let body = column![
            iced::widget::svg(self.icons.color(icon)).width(48).height(48),
            head,
            text(facts).size(style::META).font(style::FONT).color(color(p.ink_muted)),
            w::usage_bar_large(p, used, world),
            text(free).size(style::META).font(style::FONT).color(color(p.ink_muted)),
        ]
        .spacing(style::SPACE_3);
        let pal = p.clone();
        let card = iced::widget::button(container(body).padding(16).width(280))
            .padding(0)
            .on_press(msg)
            .style(move |_, status| iced::widget::button::Style {
                background: Some(Background::Color(color(pal.bg_raised))),
                border: Border { color: if matches!(status, iced::widget::button::Status::Hovered) { color(pal.line_strong) } else { color(pal.line) }, width: 1.0, radius: 2.0.into() },
                text_color: color(pal.ink),
                ..Default::default()
            });
        match menu {
            Some(i) => w::context_area(card, move |at| Message::Drive(DriveMsg::Menu(i, at))),
            None => card.into(),
        }
    }

    /// Every drive as a card: Linux first, then Windows by letter (design system `DriveCard`).
    pub(crate) fn drives_view(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let root = fs_of(Path::new("/"));
        let mut cards: Vec<Element<'_, Message>> = vec![self.drive_card(
            "Linux (/)".into(),
            "drive",
            format!("{} · system", root.as_ref().map_or("unknown", |f| f.fs_type)),
            root.clone(),
            root.as_ref().map_or(0, |f| f.total),
            p.world_linux,
            None,
            Message::Navigate(PathBuf::from("/")),
            None,
        )];
        for (i, v) in self.volumes.iter().enumerate() {
            let fs = self.volume_fs.get(i).cloned().flatten();
            let st = self.drive_state.get(&v.device).cloned().unwrap_or_default();
            let pill = if v.locked {
                Some(w::pill(p, "Locked", Some(p.danger)))
            } else if st.mounting {
                Some(w::pill(p, "Mounting…", Some(p.info)))
            } else if !v.is_mounted() {
                Some(w::pill(p, "Not mounted", None))
            } else if st.reason.is_some() {
                Some(w::pill(p, "Needs check", Some(p.warning)))
            } else if fs.as_ref().is_some_and(|f| f.read_only) {
                Some(w::pill(p, "Read-only", Some(p.warning)))
            } else {
                None
            };
            let facts = format!("{} · {} · {}", if v.locked { "BitLocker" } else { "NTFS" }, fs.as_ref().map_or("ntfs3", |f| f.fs_type), if v.system { "Windows system" } else { "data" });
            cards.push(self.drive_card(display_name(v, &self.volumes), if v.locked { "lock" } else { "drive" }, facts, fs, v.size, p.world_windows, pill, Message::Drive(DriveMsg::Click(i)), Some(i)));
        }
        let mut grid = column![].spacing(style::SPACE_4);
        let mut it = cards.into_iter().peekable();
        while it.peek().is_some() {
            let mut r = row![].spacing(style::SPACE_4);
            for _ in 0..3 {
                if let Some(c) = it.next() {
                    r = r.push(c);
                }
            }
            grid = grid.push(r);
        }
        let head = column![
            text("Drives").size(22).font(style::FONT_BOLD).color(color(p.ink_strong)),
            text("Click a drive to open it — Windows drives mount on the first click. Right-click for more.").size(style::META).font(style::FONT).color(color(p.ink_muted)),
        ]
        .spacing(6);
        w::fill(scrollable(container(column![head, grid, Space::new().height(24)].spacing(style::SPACE_5)).padding([24, 28])).height(Length::Fill), p.bg).width(Length::Fill).height(Length::Fill).into()
    }
}

//! What the phone looks like in EchoFiles: the sidebar's Phone section (`PhoneItem`), the
//! hub (`PhoneHub`, with the drawn `PhoneDevice`), Photos, Messages, Notifications and the
//! Connect phone dialog (`PairPhone`). Flat like the rest of the app: raised boxes with
//! hairlines, theme colours, existing controls. State and actions are in `phone.rs`.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use ef_config::PhonePicture;
use ef_theme::{Palette, Rgb};
use iced::widget::{button, column, container, image, mouse_area, pin, row, scrollable, stack, svg, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Font, Length, Padding};

use crate::app::{App, Message};
use crate::phone::{self, PhoneMsg, PhonePage, Source, Want};
use crate::style::{self, color};
use crate::widgets::{self as w, Variant};

const SANS: Font = Font::with_name("Noto Sans");
const SANS_BOLD: Font = Font { weight: iced::font::Weight::Bold, ..SANS };

/// The sidebar row for the phone takes dropped files under this made-up path.
pub fn drop_target() -> PathBuf {
    PathBuf::from("\u{0}phone")
}

fn pm(m: PhoneMsg) -> Message {
    Message::Phone(m)
}

fn hex(c: Rgb) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
}

// ------------------------------------------------------------------------------ local time

fn local(secs: i64) -> libc::tm {
    let t = secs as libc::time_t;
    // SAFETY: localtime_r fills the zeroed struct we own.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    tm
}

const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

fn now_secs() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// "Today", "Yesterday", "Monday, 28 September", "28 September 2024".
fn day_label(secs: i64) -> String {
    let (t, n) = (local(secs), local(now_secs()));
    let day = |tm: &libc::tm| tm.tm_year as i64 * 400 + tm.tm_yday as i64;
    match day(&n) - day(&t) {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        2..=6 => format!("{}, {} {}", DAYS[t.tm_wday as usize % 7], t.tm_mday, MONTHS[t.tm_mon as usize % 12]),
        _ if t.tm_year == n.tm_year => format!("{} {}", t.tm_mday, MONTHS[t.tm_mon as usize % 12]),
        _ => format!("{} {} {}", t.tm_mday, MONTHS[t.tm_mon as usize % 12], t.tm_year + 1900),
    }
}

/// "10:31", or the day for older times.
fn short_time(ms: i64) -> String {
    let secs = ms / 1000;
    let t = local(secs);
    if now_secs() - secs < 20 * 3600 {
        format!("{:02}:{:02}", t.tm_hour, t.tm_min)
    } else {
        format!("{} {}", t.tm_mday, &MONTHS[t.tm_mon as usize % 12][..3])
    }
}

// ------------------------------------------------------------------------------ drawing

/// The phone, flat like the colour icons: frame in the slate slots, keys on the right,
/// thin bezel, pin-hole camera; the lock screen's wallpaper is the newest photo or three
/// flat theme-coloured circles. Text is laid over it by `phone_device`.
fn device_svg(p: &Palette, state: DeviceState, wallpaper: Option<&str>, battery: i32) -> String {
    let slot = |name: &str, fallback: Rgb| p.icon_slots.iter().find(|(n, _)| *n == name).map_or(fallback, |(_, c)| *c);
    let frame = hex(slot("slate-deep", p.ink_faint));
    let key = hex(slot("slate", p.ink_muted));
    let (x, y, wd, ht, r) = (14, 14, 276, 592, 30);
    let mut s = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 304 620"><defs><clipPath id="c"><rect x="{x}" y="{y}" width="{wd}" height="{ht}" rx="{r}"/></clipPath></defs>"#);
    s += &format!(r#"<rect x="300" y="128" width="4" height="70" rx="1.5" fill="{key}"/><rect x="300" y="214" width="4" height="38" rx="1.5" fill="{key}"/>"#);
    s += &format!(r#"<rect x="2" y="2" width="300" height="616" rx="40" fill="{frame}"/>"#);
    s += &format!(r#"<g fill="{key}"><rect x="2" y="70" width="4" height="3"/><rect x="298" y="70" width="4" height="3"/><rect x="2" y="548" width="4" height="3"/><rect x="298" y="548" width="4" height="3"/></g>"#);
    s += r##"<rect x="8" y="8" width="288" height="604" rx="35" fill="#020203"/><g clip-path="url(#c)">"##;
    if state == DeviceState::Away {
        s += &format!(r##"<rect x="{x}" y="{y}" width="{wd}" height="{ht}" fill="#040506"/>"##);
    } else {
        s += &format!(r##"<rect x="{x}" y="{y}" width="{wd}" height="{ht}" fill="#0b0d14"/>"##);
        match wallpaper {
            Some(b64) => s += &format!(r##"<image x="{x}" y="{y}" width="{wd}" height="{ht}" preserveAspectRatio="xMidYMid slice" xlink:href="data:image/jpeg;base64,{b64}"/><rect x="{x}" y="{y}" width="{wd}" height="200" fill="#000" fill-opacity="0.28"/><rect x="{x}" y="430" width="{wd}" height="176" fill="#000" fill-opacity="0.32"/>"##),
            None => {
                s += &format!(r#"<circle cx="300" cy="300" r="110" fill="{}" fill-opacity="0.5"/><circle cx="10" cy="520" r="120" fill="{}" fill-opacity="0.35"/><circle cx="260" cy="620" r="130" fill="{}" fill-opacity="0.45"/>"#, hex(p.world_network), hex(p.accent), hex(p.world_linux));
            }
        }
        // status bar: notification dots, signal, wifi, battery
        s += r##"<g fill="#fff"><circle cx="80" cy="36" r="3.2" fill-opacity="0.85"/><rect x="88" y="32.5" width="7" height="7" rx="1.5" fill-opacity="0.85"/><path d="M196 40h2v2h-2zM199.5 37h2v5h-2zM203 34h2v8h-2zM206.5 31h2v11h-2z"/><path d="M212 35.2a9 9 0 0 1 12 0l-6 7z"/></g>"##;
        let lv = (battery.clamp(0, 100) as f32) / 100.0;
        s += &format!(r##"<rect x="264" y="31" width="8" height="12" rx="2" fill="none" stroke="#fff" stroke-width="1.2"/><rect x="266" y="{:.1}" width="4" height="{:.1}" rx="0.8" fill="#fff"/>"##, 33.0 + 8.0 * (1.0 - lv), 8.0 * lv);
        match state {
            DeviceState::Ringing => {
                s += &format!(r##"<rect x="26" y="380" width="252" height="92" rx="26" fill="#0b0f24" fill-opacity="0.66"/><circle cx="66" cy="426" r="22" fill="{}"/><path d="M56 432h20l-3-4v-6a7 7 0 0 0-14 0v6zM63 435.5a3 3 0 0 0 6 0" fill="none" stroke="#fff" stroke-width="2.2" stroke-linejoin="round" stroke-linecap="round"/>"##, hex(p.accent));
            }
            _ => {
                s += &format!(r##"<rect x="26" y="392" width="252" height="78" rx="26" fill="#0b0f24" fill-opacity="0.55"/><rect x="42" y="408" width="18" height="18" rx="9" fill="{}"/><path d="M46.5 415l4.5-1.4 4.5 1.4v5.2l-4.5 1.4-4.5-1.4z" fill="none" stroke="#fff" stroke-width="1.4" stroke-linejoin="round"/>"##, hex(p.accent));
            }
        }
        s += r##"<g fill="none" stroke="#fff" stroke-opacity="0.8" stroke-width="1.5" stroke-linecap="round"><path d="M145 530a7 7 0 0 1 14 0v6M141 528a11 11 0 0 1 22 0v8M152 530v10M148 534v5M156 534v4"/></g>"##;
        s += r##"<g fill="#fff" fill-opacity="0.12"><circle cx="52" cy="568" r="19"/><circle cx="252" cy="568" r="19"/></g><g fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"><path d="M46 561c0 7 4 12 12 13l2-3.5-3.5-2.5-2 1.6c-2-1-3.6-2.6-4.6-4.6l1.6-2-2.5-3.5z"/><rect x="243" y="562" width="18" height="12" rx="2.5"/><circle cx="252" cy="568" r="3"/></g><rect x="128" y="596" width="48" height="3" rx="1.5" fill="#fff" fill-opacity="0.7"/>"##;
    }
    s += r##"</g><circle cx="152" cy="31" r="6" fill="#000"/><circle cx="152" cy="31" r="3" fill="#0e1328"/></svg>"##;
    s
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceState {
    On,
    Ringing,
    Away,
}

/// A ring gauge: track and arc, as SVG.
fn ring_svg(p: &Palette, pct: f32, fill: Rgb) -> svg::Handle {
    let c = 2.0 * std::f32::consts::PI * 22.0;
    let off = c * (1.0 - pct.clamp(0.0, 1.0));
    svg::Handle::from_memory(
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 56 56"><circle cx="28" cy="28" r="22" fill="none" stroke="{}" stroke-width="5"/><circle cx="28" cy="28" r="22" fill="none" stroke="{}" stroke-width="5" stroke-linecap="round" stroke-dasharray="{c:.2}" stroke-dashoffset="{off:.2}" transform="rotate(-90 28 28)"/></svg>"#,
            hex(p.line),
            hex(fill)
        )
        .into_bytes(),
    )
}

fn bordered(p: &Palette, bg: Rgb, edge: Rgb) -> impl Fn(&iced::Theme) -> container::Style + use<> {
    let (bg, edge) = (color(bg), color(edge));
    let _ = p;
    move |_| container::Style { background: Some(Background::Color(bg)), border: Border { color: edge, width: 1.0, radius: 4.0.into() }, ..Default::default() }
}

impl App {
    fn pt<'a>(&self, s: impl Into<String>, size: f32, ink: Rgb) -> iced::widget::Text<'a> {
        text(s.into()).size(size).font(style::FONT).color(color(ink))
    }

    fn ptb<'a>(&self, s: impl Into<String>, size: f32, ink: Rgb) -> iced::widget::Text<'a> {
        text(s.into()).size(size).font(style::FONT_BOLD).color(color(ink))
    }

    /// A 20×10 battery with its fill (design system `BatteryMeter`).
    pub(crate) fn battery_meter<'a>(&self, level: i32, charging: bool) -> Element<'a, Message> {
        let p = &self.palette;
        let fill = if charging {
            p.success.base
        } else if level <= 10 {
            p.danger.base
        } else if level <= 20 {
            p.warning.base
        } else {
            p.ink_muted
        };
        let lv = level.clamp(0, 100) as u16;
        let (f, edge) = (color(fill), color(p.ink_muted));
        let inner = row![
            container(Space::new()).width(Length::FillPortion(lv.max(1))).height(Length::Fill).style(move |_| container::Style { background: Some(Background::Color(f)), border: Border { radius: 1.0.into(), ..Border::default() }, ..Default::default() }),
            Space::new().width(Length::FillPortion((100 - lv).max(1))),
        ];
        let body = container(inner).width(20).height(10).padding(1).style(move |_| container::Style { border: Border { color: edge, width: 1.0, radius: 3.0.into() }, ..Default::default() });
        let tip = container(Space::new()).width(2).height(4).style(move |_| container::Style { background: Some(Background::Color(edge)), ..Default::default() });
        let mut r = row![body, tip].spacing(0).align_y(Alignment::Center);
        if charging {
            r = r.push(Space::new().width(2));
            r = r.push(w::glyph(&self.icons, "bolt", 11.0, color(p.success.ink)));
        }
        r.into()
    }

    // ------------------------------------------------------------------ sidebar

    /// The Phone section: the paired phone(s) with Files and Photos under the one that's
    /// connected, or Connect phone.
    pub(crate) fn phone_section(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let open = self.settings.sidebar.phone_open;
        let mut col = column![self.fold_head("Phone", color(p.world_phone), None, open, crate::view::Fold::Phone, None)].spacing(1);
        let pane = self.pane();
        let current = self.phone_id();
        if self.phone.trusted.is_empty() {
            if open {
                col = col.push(self.side_button("plus", "Connect phone", pm(PhoneMsg::Dialog(true)), self.phone.dialog.is_some()));
            }
            return col.into();
        }
        for t in &self.phone.trusted {
            let here = current.as_deref() == Some(t.id.as_str()) && (pane.phone.is_some() || (!pane.special() && self.is_phone_path(&pane.location)));
            if !open && !here {
                continue;
            }
            col = col.push(self.phone_item(&t.id, pane.phone == Some(PhonePage::Hub) && current.as_deref() == Some(t.id.as_str())));
            if open && current.as_deref() == Some(t.id.as_str()) && self.phone_online(&t.id) && self.settings.phone.files {
                let storage = self.phone_storage().map(Path::to_path_buf);
                let files_active = !pane.special() && storage.as_ref().is_some_and(|s| pane.location.starts_with(s));
                col = col.push(self.phone_sub("folder", "Files", pm(PhoneMsg::Files(Want::Files)), files_active, None));
                let new = self.photos_new().len();
                col = col.push(self.phone_sub("image", "Photos", pm(PhoneMsg::Open(PhonePage::Photos)), pane.phone == Some(PhonePage::Photos), (new > 0).then(|| format!("{new} new"))));
            }
        }
        if open {
            col = col.push(self.side_button("plus", "Connect another phone", pm(PhoneMsg::Dialog(true)), false));
        }
        col.into()
    }

    fn phone_sub<'a>(&'a self, icon: &str, label: &str, msg: Message, active: bool, trail: Option<String>) -> Element<'a, Message> {
        let p = &self.palette;
        let tint = if active { color(p.accent_ink) } else { color(p.ink_muted) };
        let mut r = row![
            w::glyph(&self.icons, icon, 16.0, tint),
            text(label.to_string()).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(color(if active { p.ink_strong } else { p.ink })).width(Length::Fill),
        ]
        .spacing(style::SPACE_4)
        .align_y(Alignment::Center);
        if let Some(t) = trail {
            r = r.push(self.pt(t, style::LABEL, p.ink_muted));
        }
        w::row_button(p, container(r).height(style::ROW).align_y(Alignment::Center).padding(Padding { left: 16.0, ..Padding::ZERO }).into(), active, Some(msg))
    }

    fn phone_item<'a>(&'a self, id: &str, active: bool) -> Element<'a, Message> {
        let p = &self.palette;
        let name = self.phone_name(id);
        let online = self.phone_online(id);
        let battery = self.phone.battery.get(id).copied();
        let ink = if active { color(p.ink_strong) } else if online { color(p.ink) } else { color(p.ink_muted) };
        let head = text(name.clone()).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(ink).wrapping(text::Wrapping::None).width(Length::Fill);
        let mut top = row![head].spacing(style::SPACE_3).align_y(Alignment::Center);
        if let (true, Some((lv, ch))) = (online, battery) {
            top = top.push(self.battery_meter(lv, ch));
        }
        let meta: Element<'a, Message> = if self.phone.code.as_ref().is_some_and(|c| c.0 == id) {
            w::pill(p, "Pairing…", Some(p.info))
        } else if online {
            let ip = self.phone.devices.get(id).map(|d| d.ip.to_string()).unwrap_or_default();
            let mut r = row![self.pt(format!("Wi-Fi · {ip}"), style::LABEL, p.ink_muted).wrapping(text::Wrapping::None).width(Length::Fill)];
            if let Some((lv, _)) = battery {
                r = r.push(self.pt(format!("{lv}%"), style::LABEL, p.ink));
            }
            r.into()
        } else {
            let seen = self.phone.seen.get(id).map(|s| format!("Not nearby · {}", phone::ago(*s))).unwrap_or_else(|| "Not nearby".into());
            self.pt(seen, style::LABEL, p.ink_muted).wrapping(text::Wrapping::None).into()
        };
        let tint = if active { color(p.accent_ink) } else if online { color(p.world_phone) } else { color(p.ink_muted) };
        let r = row![container(w::glyph(&self.icons, "phone", 16.0, tint)).padding([2, 0]), column![top, meta].spacing(3).width(Length::Fill).clip(true)].spacing(style::SPACE_4).align_y(Alignment::Start);
        let drop = self.drag.is_some() && self.drop_place.as_ref() == Some(&drop_target());
        let body = w::row_button(p, container(r).padding(Padding { top: 6.0, bottom: 6.0, left: 0.0, right: 0.0 }).into(), active, Some(pm(PhoneMsg::Select(id.to_string()))));
        let body: Element<'a, Message> = if drop {
            let pal = p.clone();
            container(body).style(move |_| container::Style { background: Some(Background::Color(color(pal.accent_soft))), border: Border { color: color(pal.accent), width: 1.0, radius: 2.0.into() }, ..Default::default() }).into()
        } else {
            body
        };
        let tip = if online { format!("{name} · drop files here to send them") } else { format!("{name} · open KDE Connect on the phone to connect") };
        let item = self.tip(body, &tip, None);
        mouse_area(item).on_enter(Message::Ui(crate::overlay::UiMsg::DropHover(Some(drop_target())))).on_exit(Message::Ui(crate::overlay::UiMsg::DropLeave(drop_target()))).into()
    }

    // ------------------------------------------------------------------ toolbar / status

    pub(crate) fn phone_crumbs(&self, page: PhonePage) -> Vec<(Option<&'static str>, String, PathBuf)> {
        let name = self.phone_id().map(|i| self.phone_name(&i)).unwrap_or_else(|| "Phone".into());
        let mut v = vec![(Some("phone"), name, PathBuf::new())];
        if page != PhonePage::Hub {
            v.push((None, page.title().to_string(), PathBuf::new()));
        }
        v
    }

    pub(crate) fn phone_status(&self, page: PhonePage) -> String {
        let Some(id) = self.phone_id() else { return "No phone paired".into() };
        let name = self.phone_name(&id);
        match page {
            PhonePage::Photos => match &self.phone.photos {
                Some(Ok(v)) => format!("{} photos and videos{}", v.len(), if self.phone.photo_sel.is_empty() { String::new() } else { format!("  ·  {} selected", self.phone.photo_sel.len()) }),
                _ => name,
            },
            PhonePage::Messages => format!("{} conversations", self.phone.threads.len()),
            PhonePage::Notifications => format!("{} notifications", self.phone.notifications.len()),
            PhonePage::Hub => match (self.phone_online(&id), self.phone.battery.get(&id)) {
                (true, Some((lv, ch))) => format!("{name} · {lv}%{}", if *ch { " · charging" } else { "" }),
                (true, None) => format!("{name} · connected"),
                (false, _) => format!("{name} · not nearby"),
            },
        }
    }

    // ------------------------------------------------------------------ pages

    pub(crate) fn phone_page_view(&self, page: PhonePage) -> Element<'_, Message> {
        let p = &self.palette;
        let content: Element<'_, Message> = match self.phone_id() {
            None => self.phone_empty(),
            Some(id) => match page {
                PhonePage::Hub => self.phone_hub(&id),
                PhonePage::Photos => self.photos_page(&id),
                PhonePage::Messages => self.messages_page(&id),
                PhonePage::Notifications => self.notifications_page(&id),
            },
        };
        w::fill(content, p.bg).width(Length::Fill).height(Length::Fill).into()
    }

    fn phone_empty(&self) -> Element<'_, Message> {
        let p = &self.palette;
        container(
            column![
                iced::widget::svg(self.icons.color("phone")).width(64).height(64),
                self.ptb("Connect your phone", 22.0, p.ink_strong),
                self.pt("Browse its files and photos, send files both ways, share the clipboard and see its notifications — over Wi-Fi, with the free KDE Connect app on the phone.", style::META, p.ink_muted).width(420).align_x(Alignment::Center),
                w::text_button(p, &self.icons, "Connect phone", Some("phone"), None, Variant::Primary, Some(pm(PhoneMsg::Dialog(true)))),
            ]
            .spacing(style::SPACE_4)
            .align_x(Alignment::Center),
        )
        .center(Length::Fill)
        .into()
    }

    /// The drawn phone with its live lock screen (design system `PhoneDevice`).
    pub(crate) fn phone_device<'a>(&'a self, id: &str, width: f32) -> Element<'a, Message> {
        let k = width / 304.0;
        let h = 620.0 * k;
        let online = self.phone_online(id);
        let state = if !online {
            DeviceState::Away
        } else if self.phone.ringing.is_some() {
            DeviceState::Ringing
        } else {
            DeviceState::On
        };
        let battery = self.phone.battery.get(id).map_or(0, |b| b.0);
        let wall = match self.settings.phone.picture {
            PhonePicture::Photo => self.phone.screen.as_ref().map(|(_, b)| b.as_str()),
            PhonePicture::Echofiles => None,
        };
        let handle = svg::Handle::from_memory(device_svg(&self.palette, state, wall, battery).into_bytes());
        let mut layers: Vec<Element<'a, Message>> = vec![svg(handle).width(width).height(h).into()];
        let white = Color::WHITE;
        let soft = Color { a: 0.78, ..Color::WHITE };
        let at = |e: Element<'a, Message>, x: f32, y: f32| -> Element<'a, Message> { pin(e).x(x * k).y(y * k).into() };
        let sz = |s: f32| (s * k).max(7.0);
        if width < 100.0 {
            // A thumbnail: the drawing alone.
        } else if state == DeviceState::Away {
            layers.push(container(text("Not nearby").size(sz(15.0)).font(SANS).color(Color::from_rgb8(0x5b, 0x60, 0x70))).center_x(width).padding(Padding { top: 290.0 * k, ..Padding::ZERO }).into());
        } else {
            let tm = local(now_secs());
            let (hh, mm) = (format!("{:02}", tm.tm_hour), format!("{:02}", tm.tm_min));
            layers.push(at(text(format!("{hh}:{mm}")).size(sz(12.5)).font(SANS_BOLD).color(white).into(), 32.0, 30.0));
            layers.push(at(text(format!("{battery}%")).size(sz(11.0)).font(SANS_BOLD).color(white).into(), 230.0, 30.0));
            let date = format!("{} {} {}", &DAYS[tm.tm_wday as usize % 7][..3], tm.tm_mday, MONTHS[tm.tm_mon as usize % 12]);
            layers.push(at(text(date).size(sz(14.0)).font(SANS).color(white).into(), 36.0, 80.0));
            layers.push(at(text(hh).size(sz(96.0)).font(SANS_BOLD).color(white).line_height(1.0).into(), 28.0, 98.0));
            layers.push(at(text(mm).size(sz(96.0)).font(SANS_BOLD).color(white).line_height(1.0).into(), 28.0, 190.0));
            if state == DeviceState::Ringing {
                layers.push(at(text("Ringing from EchoFiles").size(sz(14.0)).font(SANS_BOLD).color(white).into(), 100.0, 406.0));
                layers.push(at(text("Swipe to stop").size(sz(12.0)).font(SANS).color(soft).into(), 100.0, 428.0));
            } else {
                layers.push(at(text("EchoFiles  ·  now").size(sz(11.0)).font(SANS).color(soft).into(), 68.0, 410.0));
                layers.push(at(text("Connected to your laptop").size(sz(13.0)).font(SANS_BOLD).color(white).into(), 42.0, 432.0));
                layers.push(at(text("Files, photos and clipboard ready").size(sz(11.5)).font(SANS).color(soft).into(), 42.0, 450.0));
            }
        }
        container(stack(layers).width(width).height(h)).width(width).height(h).into()
    }

    #[allow(clippy::too_many_arguments)]
    fn gauge<'a>(&'a self, icon: &str, label: &str, value: String, unit: &str, pct: f32, fill: Rgb, sub: String, msg: Option<Message>) -> Element<'a, Message> {
        let p = &self.palette;
        let ring: Element<'a, Message> = if icon == "photos" {
            let mut thumbs = row![].spacing(2);
            if let Some(Ok(list)) = &self.phone.photos {
                for ph in list.iter().filter(|x| !x.video).take(3) {
                    let tile: Element<'a, Message> = match self.phone.photo_thumbs.get(&ph.path).cloned().flatten() {
                        Some(h) => image(h).width(22).height(48).content_fit(ContentFit::Cover).into(),
                        None => container(Space::new()).width(22).height(48).style(bordered(p, p.bg_deep, p.line)).into(),
                    };
                    thumbs = thumbs.push(tile);
                }
            }
            container(thumbs).width(70).into()
        } else {
            let glyph = w::glyph(&self.icons, icon, 16.0, if icon == "bolt" { color(p.success.ink) } else { color(p.ink_muted) });
            stack![svg(ring_svg(p, pct, fill)).width(48).height(48), container(glyph).center(48)].into()
        };
        let mut value_row = row![self.ptb(value, 22.0, p.ink_strong)].spacing(3).align_y(Alignment::End);
        if !unit.is_empty() {
            value_row = value_row.push(container(self.pt(unit.to_string(), style::META, p.ink_muted)).padding(Padding { bottom: 3.0, ..Padding::ZERO }));
        }
        let sub_ink = if msg.is_some() { p.accent_ink } else { p.ink_muted };
        let text_col = column![self.ptb(label.to_uppercase(), style::LABEL, p.ink_muted), value_row, self.pt(sub, style::LABEL, sub_ink).wrapping(text::Wrapping::None)].spacing(2);
        let body = container(row![ring, text_col].spacing(style::SPACE_4).align_y(Alignment::Center)).padding(style::SPACE_4).width(Length::Fill).clip(true);
        let pal = p.clone();
        match msg {
            Some(m) => button(body)
                .padding(0)
                .width(Length::Fill)
                .on_press(m)
                .style(move |_, st| button::Style {
                    background: Some(Background::Color(if matches!(st, button::Status::Hovered) { color(pal.state_hover) } else { color(pal.bg) })),
                    border: Border { color: if matches!(st, button::Status::Hovered) { color(pal.line_strong) } else { color(pal.line) }, width: 1.0, radius: 4.0.into() },
                    text_color: color(pal.ink),
                    ..Default::default()
                })
                .into(),
            None => body.style(bordered(p, p.bg, p.line)).into(),
        }
    }

    fn feature_tile<'a>(&'a self, icon: &str, label: &str, sub: String, badge: Option<String>, off: bool, msg: Message) -> Element<'a, Message> {
        let p = &self.palette;
        let tint = if off { p.ink_muted } else { p.world_phone };
        let (wp, bgd) = (color(tint), color(p.bg_deep));
        let chip = container(w::glyph(&self.icons, icon, 18.0, wp)).center(36).style(move |_| container::Style { background: Some(Background::Color(bgd)), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() });
        let mut head = row![self.ptb(label.to_string(), style::BODY, if off { p.ink_muted } else { p.ink_strong }).width(Length::Fill)].align_y(Alignment::Center);
        if let Some(b) = badge.filter(|_| !off) {
            let (a, on) = (color(p.accent), color(p.on_accent));
            head = head.push(container(text(b).size(style::LABEL).font(style::FONT_BOLD).color(on)).padding([0, 6]).style(move |_| container::Style { background: Some(Background::Color(a)), border: Border { radius: 9.0.into(), ..Border::default() }, ..Default::default() }));
        }
        let sub = if off { "Off · turn on in Settings → Phone".to_string() } else { sub };
        let body = row![chip, column![head, self.pt(sub, style::LABEL, p.ink_muted).wrapping(text::Wrapping::None)].spacing(3).width(Length::Fill)].spacing(style::SPACE_4).align_y(Alignment::Center);
        let pal = p.clone();
        button(container(body).padding(style::SPACE_4).width(Length::Fill).clip(true))
            .padding(0)
            .width(Length::Fill)
            .on_press(msg)
            .style(move |_, st| button::Style {
                background: Some(Background::Color(if matches!(st, button::Status::Hovered) { color(pal.state_hover) } else { color(pal.bg_raised) })),
                border: Border { color: if matches!(st, button::Status::Hovered) { color(pal.world_phone) } else { color(pal.line) }, width: 1.0, radius: 4.0.into() },
                text_color: color(pal.ink),
                ..Default::default()
            })
            .into()
    }

    fn phone_hub<'a>(&'a self, id: &str) -> Element<'a, Message> {
        let p = &self.palette;
        let online = self.phone_online(id);
        let name = self.phone_name(id);
        let narrow = self.window_size.width - if self.settings.sidebar.hidden { 0.0 } else { self.settings.sidebar.width as f32 } < 980.0;

        // ---- hero
        let pill = if online { w::pill(p, "Connected", Some(p.success)) } else { w::pill(p, &self.phone.seen.get(id).map(|s| format!("Not nearby · last seen {}", phone::ago(*s))).unwrap_or_else(|| "Not nearby".into()), None) };
        let top = row![pill, Space::new().width(Length::Fill), self.tip(w::icon_button(p, &self.icons, "settings", Some(Message::Settings(crate::settings::SettingsMsg::OpenPage(crate::settings::Page::Phone))), false), "Phone settings", None)].align_y(Alignment::Center);
        let ip = self.phone.devices.get(id).map(|d| d.ip.to_string());
        let mut meta = row![].spacing(style::SPACE_5).align_y(Alignment::Center);
        let fact = |icon: &str, s: String| row![w::glyph(&self.icons, icon, 13.0, color(p.ink_muted)), self.pt(s, style::META, p.ink_muted)].spacing(6).align_y(Alignment::Center);
        if let Some(ip) = ip {
            meta = meta.push(fact("wifi", ip));
        }
        meta = meta.push(fact("link", "KDE Connect".into()));
        meta = meta.push(fact("shield", "Paired".into()));

        let (lv, ch) = self.phone.battery.get(id).copied().unwrap_or((0, false));
        let has_battery = self.phone.battery.contains_key(id);
        let battery = self.gauge(
            if ch && online { "bolt" } else { "battery" },
            "Battery",
            if has_battery { lv.to_string() } else { "—".into() },
            if has_battery { "%" } else { "" },
            lv as f32 / 100.0,
            if lv <= 20 { p.warning.base } else { p.success.base },
            if !online { "Last known".into() } else if ch { "Charging".into() } else { "On battery".into() },
            None,
        );
        let storage = self.phone.space;
        let mut gauges = row![battery].spacing(style::SPACE_3);
        if let Some((free, total)) = storage {
            let mut f = String::new();
            ef_core::fmt::size(free, &mut f);
            let mut t = String::new();
            ef_core::fmt::size(total, &mut t);
            let (num, unit) = f.split_once(' ').map(|(a, b)| (a.to_string(), format!("{b} free"))).unwrap_or((f.clone(), String::new()));
            gauges = gauges.push(self.gauge("sd-card", "Storage", num, &unit, 1.0 - free as f32 / total.max(1) as f32, p.accent, format!("of {t}"), None));
        }
        if self.settings.phone.files {
            let new = self.photos_new().len();
            let (value, sub) = match &self.phone.photos {
                Some(Ok(_)) if new > 0 => (new.to_string(), "Import →".to_string()),
                Some(Ok(v)) => (v.len().to_string(), "All imported · open →".to_string()),
                Some(Err(_)) => ("—".into(), "Couldn't read photos".into()),
                None if self.phone.mounting || self.phone.photos_loading => ("…".into(), "Looking…".into()),
                None => ("—".into(), if online { "Open →".into() } else { "Not nearby".into() }),
            };
            let label = if matches!(&self.phone.photos, Some(Ok(_))) && new == 0 { "Photos" } else { "New photos" };
            gauges = gauges.push(self.gauge("photos", label, value, "", 0.0, p.accent, sub, Some(pm(PhoneMsg::Open(PhonePage::Photos)))));
        }

        let ringing = self.phone.ringing.is_some();
        let mut actions = row![
            w::text_button(p, &self.icons, if ringing { "Stop ringing" } else { "Ring phone" }, Some("phone-ring"), None, if ringing { Variant::Primary } else { Variant::Secondary }, online.then(|| pm(PhoneMsg::Ring))),
            w::text_button(p, &self.icons, "Send files", Some("upload"), None, Variant::Secondary, online.then(|| pm(PhoneMsg::SendFiles))),
        ]
        .spacing(style::SPACE_3);
        if self.settings.phone.files {
            actions = actions.push(w::text_button(p, &self.icons, "Get files", Some("download"), None, Variant::Secondary, online.then(|| pm(PhoneMsg::Files(Want::FilesInTab)))));
        }
        let mut facts = column![top, self.ptb(name, 28.0, p.ink_strong), meta, Space::new().height(style::SPACE_3), gauges, Space::new().height(style::SPACE_1), actions].spacing(style::SPACE_3).width(Length::Fill);
        if ringing {
            facts = facts.push(self.pt("Ringing at full volume, even on silent. It stops when you tap the phone or press Stop ringing.", style::LABEL, p.ink_muted));
        }
        if let Some(e) = self.phone.sftp_error.as_ref().filter(|_| online && self.settings.phone.files) {
            facts = facts.push(row![w::glyph(&self.icons, "alert", 14.0, color(p.warning.ink)), self.pt(format!("Files aren't shared: {e} Turn on Filesystem expose in KDE Connect and allow All files access."), style::META, p.warning.ink)].spacing(6).align_y(Alignment::Center));
        }
        let device = self.phone_device(id, 188.0);
        let hero_inner: Element<'a, Message> = if narrow {
            column![container(device).center_x(Length::Fill), facts].spacing(style::SPACE_5).into()
        } else {
            row![device, facts].spacing(40).align_y(Alignment::Center).into()
        };
        let hero = container(hero_inner).padding([24, 32]).width(Length::Fill).style(bordered(p, p.bg_raised, p.line));

        // ---- features
        let s = &self.settings.phone;
        let unread: usize = self.phone.threads.values().filter(|m| !m.read && !m.outgoing).count();
        let latest = self.phone.threads.values().max_by_key(|m| m.date).map(|m| format!("{}: {}", m.address, m.body.lines().next().unwrap_or_default())).unwrap_or_else(|| "Your texts, from the laptop".into());
        let apps: Vec<String> = {
            let mut a: Vec<String> = Vec::new();
            for n in &self.phone.notifications {
                if !a.contains(&n.app) {
                    a.push(n.app.clone());
                }
            }
            a
        };
        let notif_sub = if apps.is_empty() { "Nothing new".to_string() } else { apps.iter().take(3).cloned().collect::<Vec<_>>().join(", ") };
        let photo_count = match &self.phone.photos {
            Some(Ok(v)) => format!("{} photos and videos", v.len()),
            _ => "Your phone's photos, by day".into(),
        };
        let new = self.photos_new().len();
        let tiles = column![
            row![
                self.feature_tile("folder", "Files", "Internal storage".into(), None, !s.files, if s.files { pm(PhoneMsg::Files(Want::Files)) } else { Message::Settings(crate::settings::SettingsMsg::OpenPage(crate::settings::Page::Phone)) }),
                self.feature_tile("image", "Photos", photo_count, (new > 0).then(|| format!("{new} new")), !s.files, if s.files { pm(PhoneMsg::Open(PhonePage::Photos)) } else { Message::Settings(crate::settings::SettingsMsg::OpenPage(crate::settings::Page::Phone)) }),
            ]
            .spacing(style::SPACE_3),
            row![
                self.feature_tile("message", "Messages", latest, (unread > 0).then(|| unread.to_string()), !s.messages, if s.messages { pm(PhoneMsg::Open(PhonePage::Messages)) } else { Message::Settings(crate::settings::SettingsMsg::OpenPage(crate::settings::Page::Phone)) }),
                self.feature_tile("bell", "Notifications", notif_sub, (!self.phone.notifications.is_empty()).then(|| self.phone.notifications.len().to_string()), s.notifications == ef_config::PhoneNotifications::Off, if s.notifications == ef_config::PhoneNotifications::Off { Message::Settings(crate::settings::SettingsMsg::OpenPage(crate::settings::Page::Phone)) } else { pm(PhoneMsg::Open(PhonePage::Notifications)) }),
            ]
            .spacing(style::SPACE_3),
        ]
        .spacing(style::SPACE_3);
        let left = column![tiles, self.received_list()].spacing(style::SPACE_5).width(Length::FillPortion(3));
        let right = column![self.clipboard_card()].width(Length::FillPortion(2));
        let lower: Element<'a, Message> = if narrow { column![left, right].spacing(style::SPACE_5).into() } else { row![left, right].spacing(style::SPACE_5).into() };

        scrollable(container(column![hero, lower, Space::new().height(24)].spacing(style::SPACE_5).max_width(1080)).padding([24, 28])).height(Length::Fill).into()
    }

    fn received_list(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let head = row![
            self.ptb("RECEIVED FROM PHONE", style::LABEL, p.ink_muted).width(Length::Fill),
            w::text_button(p, &self.icons, "Open folder", Some("folder"), None, Variant::Ghost, Some(pm(PhoneMsg::OpenSaveFolder))),
        ]
        .align_y(Alignment::Center);
        let mut col = column![head].spacing(0);
        if self.phone.moved.is_empty() {
            col = col.push(container(self.pt(format!("Share a file to this laptop from any app on the phone. It lands in {}.", self.settings.phone.save_to), style::META, p.ink_muted)).padding([6, 0]));
            return col.into();
        }
        for m in self.phone.moved.iter().take(6) {
            col = col.push(w::hline(p.line));
            let icon = crate::kinds::icon(m.name.as_bytes(), ef_core::listing::Kind::File);
            let mut size = String::new();
            ef_core::fmt::size(m.size, &mut size);
            let (line2, trailing): (Element<'_, Message>, Element<'_, Message>) = match &m.result {
                None => {
                    let done = m.done.load(Ordering::Relaxed);
                    let mut d = String::new();
                    ef_core::fmt::size(done, &mut d);
                    (w::progress(p, done as f32 / m.size.max(1) as f32, p.accent, false), self.pt(format!("{d} of {size}"), style::LABEL, p.ink_muted).into())
                }
                Some(Ok(path)) => {
                    let verb = if m.upload { "Sent" } else { "Received" };
                    let when = m.at.duration_since(SystemTime::UNIX_EPOCH).map(|d| phone::ago(d.as_secs() as i64)).unwrap_or_default();
                    let action: Element<'_, Message> = if m.upload { Space::new().into() } else { w::text_button(p, &self.icons, "Open", None, None, Variant::Ghost, Some(pm(PhoneMsg::OpenFile(path.clone())))) };
                    (self.pt(format!("{verb} · {size} · {when}"), style::LABEL, p.ink_muted).into(), action)
                }
                Some(Err(e)) => (self.pt(e.clone(), style::LABEL, p.danger.ink).into(), Space::new().into()),
            };
            let arrow = if m.upload { "arrow-up" } else { "arrow-down" };
            let r = row![
                iced::widget::svg(self.icons.color(icon)).width(24).height(24),
                column![row![w::glyph(&self.icons, arrow, 12.0, color(p.ink_muted)), self.pt(m.name.clone(), style::BODY, p.ink).wrapping(text::Wrapping::None)].spacing(4).align_y(Alignment::Center), line2].spacing(4).width(Length::Fill).clip(true),
                trailing,
            ]
            .spacing(style::SPACE_4)
            .align_y(Alignment::Center);
            col = col.push(container(r).padding([8, 0]));
        }
        col.into()
    }

    fn clipboard_card(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let on = self.settings.phone.clipboard;
        let head = row![
            w::glyph(&self.icons, "clipboard", 16.0, color(p.world_phone)),
            self.ptb("Shared clipboard", style::BODY, p.ink_strong).width(Length::Fill),
            w::switch(p, on, Some(Box::new(|b| pm(PhoneMsg::SetClipboard(b))))),
        ]
        .spacing(style::SPACE_3)
        .align_y(Alignment::Center);
        let mut col = column![head].spacing(0);
        if on && self.phone.clip.is_empty() {
            col = col.push(container(self.pt("Nothing shared yet.", style::META, p.ink_muted)).padding([8, 0]));
        }
        if on {
            for (from_phone, t, at) in self.phone.clip.iter().take(4) {
                let when = at.duration_since(SystemTime::UNIX_EPOCH).map(|d| phone::ago(d.as_secs() as i64)).unwrap_or_default();
                let one_line: String = t.lines().next().unwrap_or_default().chars().take(80).collect();
                col = col.push(w::hline(p.line));
                col = col.push(
                    container(
                        row![
                            w::glyph(&self.icons, if *from_phone { "arrow-down" } else { "arrow-up" }, 14.0, color(p.ink_muted)),
                            self.pt(one_line, style::META, p.ink).wrapping(text::Wrapping::None).width(Length::Fill),
                            self.pt(when, style::LABEL, p.ink_muted),
                            self.tip(w::icon_button(p, &self.icons, "copy", Some(pm(PhoneMsg::ClipCopy(t.clone()))), false), "Copy again", None),
                        ]
                        .spacing(style::SPACE_3)
                        .align_y(Alignment::Center),
                    )
                    .padding([2, 0])
                    .clip(true),
                );
            }
        }
        col = col.push(container(self.pt(if on { "Copy on one, paste on the other. Works while EchoFiles is open." } else { "Off: nothing you copy crosses over." }, style::LABEL, p.ink_muted)).padding(Padding { top: 8.0, ..Padding::ZERO }));
        container(col).padding(style::SPACE_4).width(Length::Fill).style(bordered(p, p.bg_raised, p.line)).into()
    }

    fn page_header<'a>(&'a self, title: &str, sub: String, trailing: Vec<Element<'a, Message>>) -> Element<'a, Message> {
        let p = &self.palette;
        let mut r = row![column![self.ptb(title.to_string(), 22.0, p.ink_strong), self.pt(sub, style::META, p.ink_muted)].spacing(4).width(Length::Fill)].spacing(style::SPACE_3).align_y(Alignment::Center);
        for t in trailing {
            r = r.push(t);
        }
        r.into()
    }

    fn not_nearby<'a>(&'a self, id: &str, what: &str) -> Element<'a, Message> {
        let p = &self.palette;
        container(
            column![
                w::glyph(&self.icons, "phone", 40.0, color(p.ink_faint)),
                self.ptb(format!("{} isn't nearby", self.phone_name(id)), 18.0, p.ink_strong),
                self.pt(format!("Open KDE Connect on the phone, on the same Wi-Fi, to see its {what}."), style::META, p.ink_muted),
            ]
            .spacing(style::SPACE_4)
            .align_x(Alignment::Center),
        )
        .center(Length::Fill)
        .into()
    }

    fn photos_page<'a>(&'a self, id: &str) -> Element<'a, Message> {
        let p = &self.palette;
        if !self.settings.phone.files {
            return self.feature_off("Photos are off", "Turn on Files and photos to see the phone's photos here.", pm(PhoneMsg::SetFiles(true)));
        }
        let list = match &self.phone.photos {
            Some(Ok(v)) => v,
            Some(Err(e)) => {
                return container(column![w::glyph(&self.icons, "alert", 32.0, color(p.warning.ink)), self.pt(e.clone(), style::BODY, p.ink), w::text_button(p, &self.icons, "Try again", Some("refresh"), None, Variant::Secondary, Some(pm(PhoneMsg::Files(Want::Photos))))].spacing(style::SPACE_4).align_x(Alignment::Center)).center(Length::Fill).into();
            }
            None if !self.phone_online(id) && self.phone.mount.is_none() => return self.not_nearby(id, "photos"),
            None => {
                let msg = if let Some(e) = &self.phone.sftp_error { format!("The phone didn't share its files: {e}") } else { "Opening the phone's photos…".to_string() };
                return container(column![w::glyph(&self.icons, "sync", 24.0, color(p.info.ink)), self.pt(msg, style::BODY, p.ink_muted)].spacing(style::SPACE_4).align_x(Alignment::Center)).center(Length::Fill).into();
            }
        };
        let new = self.photos_new().len();
        let f = self.phone.photo_filter;
        let filters = [("All", None), ("Camera", Some(Source::Camera)), ("Screenshots", Some(Source::Screenshots)), ("Chats", Some(Source::Chats))];
        let opts: Vec<(&str, Message)> = filters.iter().map(|(l, s)| (*l, pm(PhoneMsg::PhotosFilter(*s)))).collect();
        let sel = filters.iter().position(|(_, s)| *s == f).unwrap_or(0);
        let mut trailing = vec![w::segmented(p, &opts, sel)];
        trailing.push(w::text_button(p, &self.icons, &if new > 0 { format!("Import {new} new") } else { "All imported".into() }, Some("download"), None, Variant::Primary, (new > 0).then(|| pm(PhoneMsg::Import))));
        let shown: Vec<&phone::Photo> = list.iter().filter(|x| f.is_none_or(|s| x.source == s)).collect();
        let mut col = column![self.page_header("Photos", format!("{} photos and videos from {} · imports go to ~/Pictures/Phone", shown.len(), self.phone_name(id)), trailing)].spacing(style::SPACE_5);
        if !self.phone.photo_sel.is_empty() {
            let n = self.phone.photo_sel.len();
            let bytes: u64 = list.iter().filter(|x| self.phone.photo_sel.contains(&x.path)).map(|x| x.size).sum();
            let mut sz = String::new();
            ef_core::fmt::size(bytes, &mut sz);
            let bar = row![
                self.ptb(format!("{n} selected"), style::BODY, p.ink_strong),
                self.pt(format!("· {sz}"), style::META, p.ink_muted).width(Length::Fill),
                w::text_button(p, &self.icons, "Save to…", Some("download"), None, Variant::Secondary, Some(pm(PhoneMsg::PhotosSaveTo))),
                w::text_button(p, &self.icons, "Copy", Some("copy"), Some("Ctrl+V to paste"), Variant::Secondary, Some(pm(PhoneMsg::PhotosCopy))),
                w::text_button(p, &self.icons, "Clear", None, None, Variant::Ghost, Some(pm(PhoneMsg::PhotosClear))),
            ]
            .spacing(style::SPACE_3)
            .align_y(Alignment::Center);
            let (bg, edge) = (p.accent_soft, p.accent);
            col = col.push(container(bar).padding([6, 12]).width(Length::Fill).style(bordered(p, bg, edge)));
        }
        let avail = self.window_size.width - if self.settings.sidebar.hidden { 0.0 } else { self.settings.sidebar.width as f32 } - 56.0;
        let per_row = ((avail + 4.0) / 122.0).floor().max(2.0) as usize;
        let mut day = String::new();
        let mut group: Vec<&phone::Photo> = Vec::new();
        let limit = self.phone.photo_limit;
        let flush = |col: iced::widget::Column<'a, Message>, day: &str, group: &[&'a phone::Photo]| -> iced::widget::Column<'a, Message> {
            if group.is_empty() {
                return col;
            }
            let mut grid = column![].spacing(4);
            for chunk in group.chunks(per_row) {
                let mut r = row![].spacing(4);
                for ph in chunk {
                    r = r.push(self.photo_tile(ph));
                }
                grid = grid.push(r);
            }
            col.push(column![row![self.ptb(day.to_string(), 15.0, p.ink_strong), self.pt(group.len().to_string(), style::LABEL, p.ink_muted)].spacing(style::SPACE_3).align_y(Alignment::End), grid].spacing(style::SPACE_3))
        };
        for ph in shown.iter().take(limit) {
            let d = day_label(ph.mtime);
            if d != day {
                col = flush(col, &day, &group);
                group.clear();
                day = d;
            }
            group.push(ph);
        }
        col = flush(col, &day, &group);
        if shown.len() > limit {
            col = col.push(container(w::text_button(p, &self.icons, &format!("Show more ({} left)", shown.len() - limit), Some("arrow-down"), None, Variant::Secondary, Some(pm(PhoneMsg::PhotosMore)))).center_x(Length::Fill));
        }
        if shown.is_empty() {
            col = col.push(self.pt("No photos here.", style::BODY, p.ink_muted));
        }
        scrollable(container(col.push(Space::new().height(24))).padding([24, 28])).height(Length::Fill).into()
    }

    fn photo_tile<'a>(&'a self, ph: &'a phone::Photo) -> Element<'a, Message> {
        let p = &self.palette;
        let sel = self.phone.photo_sel.contains(&ph.path);
        let inner: Element<'a, Message> = match self.phone.photo_thumbs.get(&ph.path).cloned().flatten() {
            Some(h) => image(h).width(Length::Fill).height(Length::Fill).content_fit(ContentFit::Cover).into(),
            None => container(w::glyph(&self.icons, if ph.video { "play" } else { "image" }, 20.0, color(p.ink_faint))).center(Length::Fill).into(),
        };
        let mut layers: Vec<Element<'a, Message>> = vec![container(inner).padding(if sel { 8 } else { 0 }).width(118).height(118).into()];
        if ph.video {
            layers.push(pin(container(w::glyph(&self.icons, "play", 10.0, Color::WHITE)).padding([1, 5]).style(|_| container::Style { background: Some(Background::Color(Color { a: 0.55, ..Color::BLACK })), border: Border { radius: 2.0.into(), ..Border::default() }, ..Default::default() })).x(90.0).y(96.0).into());
        }
        if sel {
            let (a, on) = (color(p.accent), color(p.on_accent));
            layers.push(pin(container(w::glyph(&self.icons, "check", 12.0, on)).center(20).style(move |_| container::Style { background: Some(Background::Color(a)), border: Border { radius: 10.0.into(), ..Border::default() }, ..Default::default() })).x(6.0).y(6.0).into());
        }
        let pal = p.clone();
        let tile = button(stack(layers).width(118).height(118)).padding(0).on_press(pm(PhoneMsg::PhotoToggle(ph.path.clone()))).style(move |_, st| button::Style {
            background: Some(Background::Color(color(pal.bg_deep))),
            border: Border { color: if sel { color(pal.accent) } else if matches!(st, button::Status::Hovered) { color(pal.line_strong) } else { Color::TRANSPARENT }, width: if sel { 3.0 } else { 1.0 }, radius: 2.0.into() },
            ..Default::default()
        });
        let name = ph.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        self.tip(mouse_area(tile).on_double_click(pm(PhoneMsg::PhotoOpen(ph.path.clone()))), &name, None)
    }

    fn feature_off<'a>(&'a self, title: &str, body: &str, turn_on: Message) -> Element<'a, Message> {
        let p = &self.palette;
        container(
            column![
                self.ptb(title.to_string(), 18.0, p.ink_strong),
                self.pt(body.to_string(), style::META, p.ink_muted),
                w::text_button(p, &self.icons, "Turn on", None, None, Variant::Primary, Some(turn_on)),
            ]
            .spacing(style::SPACE_4)
            .align_x(Alignment::Center),
        )
        .center(Length::Fill)
        .into()
    }

    fn messages_page<'a>(&'a self, id: &str) -> Element<'a, Message> {
        let p = &self.palette;
        if !self.settings.phone.messages {
            return self.feature_off("Messages are off", "Turn them on to read and send texts from the laptop. The phone asks for SMS permission the first time.", pm(PhoneMsg::SetMessages(true)));
        }
        if !self.phone_online(id) && self.phone.threads.is_empty() {
            return self.not_nearby(id, "messages");
        }
        let mut threads: Vec<&ef_phone::Sms> = self.phone.threads.values().collect();
        threads.sort_by_key(|m| std::cmp::Reverse(m.date));
        let palette_slots = ["purple", "orange", "blue", "green", "red", "yellow"];
        let avatar = |who: &str| -> Element<'a, Message> {
            let initial = who.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "#".into());
            let slot = palette_slots[who.bytes().map(usize::from).sum::<usize>() % palette_slots.len()];
            let c = p.icon_slots.iter().find(|(n, _)| *n == slot).map_or(p.accent, |(_, c)| *c);
            let (bg, fg) = (color(c), Color::WHITE);
            container(text(initial).size(style::BODY).font(style::FONT_BOLD).color(fg)).center(32).style(move |_| container::Style { background: Some(Background::Color(bg)), border: Border { radius: 16.0.into(), ..Border::default() }, ..Default::default() }).into()
        };
        let mut list = column![row![self.ptb("Conversations", style::BODY, p.ink_strong).width(Length::Fill), self.tip(w::icon_button(p, &self.icons, "refresh", Some(pm(PhoneMsg::RefreshMessages)), false), "Refresh", None)].align_y(Alignment::Center).padding([0, 4])].spacing(1);
        if threads.is_empty() {
            list = list.push(container(self.pt("Loading conversations…", style::META, p.ink_muted)).padding(12));
        }
        for m in threads.iter().take(200) {
            let active = self.phone.open_thread == Some(m.thread);
            let unread = !m.read && !m.outgoing;
            let r = row![
                avatar(&m.address),
                column![
                    self.pt(m.address.clone(), style::BODY, if unread || active { p.ink_strong } else { p.ink }).font(if unread { style::FONT_BOLD } else { style::FONT }).wrapping(text::Wrapping::None),
                    self.pt(format!("{}{}", if m.outgoing { "You: " } else { "" }, m.body.lines().next().unwrap_or_default()), style::META, p.ink_muted).wrapping(text::Wrapping::None),
                ]
                .spacing(2)
                .width(Length::Fill)
                .clip(true),
                self.pt(short_time(m.date), style::LABEL, p.ink_muted),
            ]
            .spacing(style::SPACE_3)
            .align_y(Alignment::Center);
            list = list.push(w::row_button(p, container(r).padding([8, 0]).into(), active, Some(pm(PhoneMsg::Thread(m.thread)))));
        }
        let left = w::fill(scrollable(list.padding([12, 8])).height(Length::Fill), p.bg_sunken).width(300).height(Length::Fill);

        let right: Element<'a, Message> = match self.phone.open_thread {
            None => container(self.pt("Pick a conversation.", style::BODY, p.ink_muted)).center(Length::Fill).into(),
            Some(t) => {
                let who = self.phone.threads.get(&t).map(|m| m.address.clone()).unwrap_or_default();
                let head = container(row![avatar(&who), self.ptb(who.clone(), style::BODY, p.ink_strong)].spacing(style::SPACE_3).align_y(Alignment::Center)).padding([10, 20]);
                let mut msgs = column![].spacing(style::SPACE_3);
                let mut last_day = String::new();
                for m in self.phone.thread_msgs.get(&t).map(Vec::as_slice).unwrap_or(&[]) {
                    let d = day_label(m.date / 1000);
                    if d != last_day {
                        msgs = msgs.push(container(self.pt(d.clone(), style::LABEL, p.ink_muted)).center_x(Length::Fill));
                        last_day = d;
                    }
                    let (bg, fg, edge) = if m.outgoing { (p.accent, p.on_accent, p.accent) } else { (p.bg_raised, p.ink_strong, p.line) };
                    let tm = local(m.date / 1000);
                    let bubble = container(column![text(m.body.clone()).size(style::BODY).font(style::FONT).color(color(fg)), text(format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)).size(10).font(style::FONT).color(Color { a: 0.7, ..color(fg) })].spacing(2))
                        .padding([8, 12])
                        .max_width(460)
                        .style(move |_| container::Style { background: Some(Background::Color(color(bg))), border: Border { color: color(edge), width: 1.0, radius: 12.0.into() }, ..Default::default() });
                    msgs = msgs.push(if m.outgoing { container(bubble).align_right(Length::Fill) } else { container(bubble).align_left(Length::Fill) });
                }
                let input = text_input("Text message", &self.phone.compose).id(crate::phone::SMS_ID).on_input(|t| pm(PhoneMsg::Compose(t))).on_submit(pm(PhoneMsg::SendSms)).size(style::BODY).font(style::FONT).padding([6, 10]).style(w::field_style(p, false));
                let can = self.phone_online(id) && !self.phone.compose.trim().is_empty();
                let compose = row![input, w::text_button(p, &self.icons, "Send", Some("send"), Some("Enter"), Variant::Primary, can.then(|| pm(PhoneMsg::SendSms)))].spacing(style::SPACE_3).align_y(Alignment::Center);
                column![
                    head,
                    w::hline(p.line),
                    scrollable(container(msgs).padding([16, 20])).height(Length::Fill).anchor_bottom(),
                    container(column![compose, self.pt(format!("Sends from {} as a normal text. Carrier rates apply.", self.phone_name(id)), style::LABEL, p.ink_muted)].spacing(6)).padding([10, 20]),
                ]
                .width(Length::Fill)
                .into()
            }
        };
        row![left, w::vline(p.line), right].height(Length::Fill).into()
    }

    fn notifications_page<'a>(&'a self, id: &str) -> Element<'a, Message> {
        let p = &self.palette;
        let mode = self.settings.phone.notifications;
        if mode == ef_config::PhoneNotifications::Off {
            return self.feature_off("Notifications are off", "Turn them on to see the phone's notifications here.", pm(PhoneMsg::SetNotifications(ef_config::PhoneNotifications::App)));
        }
        if !self.phone_online(id) && self.phone.notifications.is_empty() {
            return self.not_nearby(id, "notifications");
        }
        let sub = if mode == ef_config::PhoneNotifications::Desktop { "Shown here and as desktop notifications." } else { "Shown here only — no desktop pop-ups. Change it in Settings → Phone." };
        let any = self.phone.notifications.iter().any(|n| n.dismissable);
        let mut col = column![self.page_header("Notifications", sub.into(), vec![w::text_button(p, &self.icons, "Dismiss all", Some("close"), None, Variant::Ghost, any.then(|| pm(PhoneMsg::DismissAll)))])].spacing(style::SPACE_4);
        if self.phone.notifications.is_empty() {
            col = col.push(self.pt("Nothing new on the phone.", style::BODY, p.ink_muted));
        }
        let mut apps: Vec<String> = Vec::new();
        for n in &self.phone.notifications {
            if !apps.contains(&n.app) {
                apps.push(n.app.clone());
            }
        }
        for app in apps {
            let items: Vec<&ef_phone::Notification> = self.phone.notifications.iter().filter(|n| n.app == app).collect();
            let mut g = column![container(row![iced::widget::svg(self.icons.color("phone")).width(18).height(18), self.ptb(app.clone(), style::BODY, p.ink_strong), self.pt(format!("· {}", items.len()), style::META, p.ink_muted)].spacing(style::SPACE_3).align_y(Alignment::Center)).padding([10, 16])];
            for n in items {
                g = g.push(w::hline(p.line));
                let mut line = row![
                    column![self.ptb(if n.title.is_empty() { n.app.clone() } else { n.title.clone() }, style::BODY, p.ink_strong).wrapping(text::Wrapping::None), self.pt(n.text.clone(), style::META, p.ink_muted)].spacing(2).width(Length::Fill).clip(true),
                    self.pt(short_time(n.time), style::LABEL, p.ink_muted),
                ]
                .spacing(style::SPACE_4)
                .align_y(Alignment::Center);
                if n.dismissable {
                    line = line.push(self.tip(w::icon_button(p, &self.icons, "close", Some(pm(PhoneMsg::Dismiss(n.key.clone()))), false), "Dismiss on the phone too", None));
                }
                let mut body = column![line].spacing(style::SPACE_3);
                if n.reply_id.is_some() {
                    let draft = self.phone.reply.get(&n.key).cloned().unwrap_or_default();
                    let key = n.key.clone();
                    let k2 = n.key.clone();
                    let input = text_input("Reply", &draft).on_input(move |t| pm(PhoneMsg::ReplyDraft(key.clone(), t))).on_submit(pm(PhoneMsg::Reply(k2.clone()))).size(style::BODY).font(style::FONT).padding([5, 10]).style(w::field_style(p, false));
                    body = body.push(row![input, w::text_button(p, &self.icons, "Reply", Some("send"), None, Variant::Secondary, (!draft.trim().is_empty()).then(|| pm(PhoneMsg::Reply(n.key.clone()))))].spacing(style::SPACE_3).align_y(Alignment::Center));
                }
                g = g.push(container(body).padding([10, 16]));
            }
            col = col.push(container(g).max_width(760).width(Length::Fill).style(bordered(p, p.bg_raised, p.line)));
        }
        scrollable(container(col.push(Space::new().height(24))).padding([24, 28])).height(Length::Fill).into()
    }

    // ------------------------------------------------------------------ Connect phone

    /// Connect your phone (design system `PairPhone`).
    pub(crate) fn phone_layer(&self) -> Option<Element<'_, Message>> {
        let d = self.phone.dialog.as_ref()?;
        let p = &self.palette;
        let paired_now = d.paired.clone().filter(|id| self.phone.trusted.iter().any(|t| &t.id == id));
        let unpaired: Vec<&ef_phone::Device> = self.phone.devices.values().filter(|x| !x.paired).collect();
        let step = if self.phone.code.is_some() {
            2
        } else if paired_now.is_some() {
            3
        } else if unpaired.is_empty() {
            0
        } else {
            1
        };
        let steps = ["Get the app", "Choose your phone", "Check the code", "Allow files"];
        let mut stepper = row![].spacing(4);
        for (i, s) in steps.iter().enumerate() {
            let (edge, ink) = if i < step { (p.success.base, p.ink_muted) } else if i == step { (p.accent, p.ink_strong) } else { (p.line, p.ink_faint) };
            let e = color(edge);
            stepper = stepper.push(column![container(Space::new()).width(Length::Fill).height(2).style(move |_| container::Style { background: Some(Background::Color(e)), ..Default::default() }), text(format!("{}  {s}", i + 1)).size(style::LABEL).font(if i == step { style::FONT_BOLD } else { style::FONT }).color(color(ink))].spacing(6).width(Length::Fill));
        }
        let mut body = column![stepper].spacing(style::SPACE_4);
        if let Some(e) = &self.phone.error {
            body = body.push(row![w::glyph(&self.icons, "alert", 14.0, color(p.warning.ink)), self.pt(e.clone(), style::META, p.warning.ink)].spacing(6).align_y(Alignment::Center));
        }
        let mut foot: Vec<Element<'_, Message>> = vec![w::text_button(p, &self.icons, "Close", None, Some("Esc"), Variant::Ghost, Some(pm(PhoneMsg::Dialog(false))))];
        match (&self.phone.code, paired_now) {
            (Some((id, code, asked)), _) => {
                let name = self.phone_name(id);
                body = body.push(self.pt(if *asked { format!("{name} wants to pair. Check both screens show this code, then accept here.") } else { format!("Check {name} shows this code, then tap Accept on the phone.") }, style::BODY, p.ink));
                let mut tiles = row![].spacing(style::SPACE_3);
                for pair in code.as_bytes().chunks(2) {
                    let (bg, edge, ac) = (color(p.bg_deep), color(p.line_strong), color(p.accent));
                    tiles = tiles.push(container(text(String::from_utf8_lossy(pair).into_owned()).size(26).font(style::FONT_BOLD).color(color(p.ink_strong))).padding([8, 14]).style(move |_| container::Style {
                        background: Some(Background::Color(bg)),
                        border: Border { color: edge, width: 1.0, radius: 4.0.into() },
                        shadow: iced::Shadow { color: ac, offset: iced::Vector::new(0.0, 2.0), blur_radius: 0.0 },
                        ..Default::default()
                    }));
                }
                body = body.push(container(tiles).center_x(Length::Fill));
                if *asked {
                    foot = vec![
                        w::text_button(p, &self.icons, "Codes don't match", None, None, Variant::Ghost, Some(pm(PhoneMsg::RejectPair))),
                        w::text_button(p, &self.icons, "Pair", Some("check"), Some("Enter"), Variant::Primary, Some(pm(PhoneMsg::AcceptPair))),
                    ];
                } else {
                    body = body.push(row![w::glyph(&self.icons, "sync", 14.0, color(p.info.ink)), self.pt(format!("Waiting for {name}…"), style::META, p.ink_muted)].spacing(6).align_y(Alignment::Center));
                    foot = vec![w::text_button(p, &self.icons, "Cancel", None, Some("Esc"), Variant::Ghost, Some(pm(PhoneMsg::RejectPair)))];
                }
            }
            (None, Some(id)) => {
                let name = self.phone_name(&id);
                if self.phone.mount.is_some() {
                    body = body.push(self.pt(format!("Paired with {name}. Everything's ready: files, photos, battery and notifications."), style::BODY, p.ink));
                } else {
                    body = body.push(self.pt(format!("Paired with {name}. One switch left so EchoFiles can see the phone's files:"), style::BODY, p.ink));
                    body = body.push(self.pt("1. In KDE Connect, open the device's Plugin settings.\n2. Turn on Filesystem expose, and allow All files access when Android asks.", style::META, p.ink_muted));
                }
                let status: Element<'_, Message> = if self.phone.mount.is_some() {
                    w::pill(p, "Files available", Some(p.success))
                } else if self.phone.mounting {
                    w::pill(p, "Checking…", Some(p.info))
                } else {
                    row![w::pill(p, "Files not shared yet", Some(p.warning)), w::text_button(p, &self.icons, "Check again", Some("refresh"), None, Variant::Secondary, Some(pm(PhoneMsg::Files(Want::Nothing))))].spacing(style::SPACE_3).align_y(Alignment::Center).into()
                };
                body = body.push(status);
                foot.push(w::text_button(p, &self.icons, &format!("Open {name}"), Some("phone"), None, Variant::Primary, Some(pm(PhoneMsg::Select(id.clone())))));
            }
            (None, None) => {
                body = body.push(self.pt("EchoFiles talks to the free KDE Connect app on your Android phone — nothing else to install on this laptop. Get KDE Connect from Google Play or F-Droid, open it, and keep the phone on the same Wi-Fi.", style::META, p.ink_muted));
                let unpaired_none = unpaired.is_empty();
                let mut list = column![self.ptb("PHONES ON THIS NETWORK", style::LABEL, p.ink_muted)].spacing(style::SPACE_1 + 2.0);
                if unpaired.is_empty() {
                    list = list.push(row![w::glyph(&self.icons, "sync", 14.0, color(p.info.ink)), self.pt("Looking for phones with KDE Connect open…", style::META, p.ink_muted)].spacing(6).align_y(Alignment::Center));
                }
                for dev in unpaired {
                    let r = row![
                        w::glyph(&self.icons, "phone", 18.0, color(p.world_phone)),
                        column![self.pt(dev.name.clone(), style::BODY, p.ink), self.pt(format!("{} · {}", if dev.kind == "tablet" { "Tablet" } else { "Phone" }, dev.ip), style::LABEL, p.ink_muted)].spacing(2).width(Length::Fill),
                        self.ptb("Pair", style::BODY, p.accent_ink),
                        w::glyph(&self.icons, "chevron-right", 12.0, color(p.accent_ink)),
                    ]
                    .spacing(style::SPACE_4)
                    .align_y(Alignment::Center);
                    list = list.push(container(w::row_button(p, container(r).padding([8, 0]).into(), false, Some(pm(PhoneMsg::Pair(dev.id.clone()))))).style(bordered(p, p.bg_deep, p.line)));
                }
                body = body.push(list);
                if d.since.elapsed().as_secs() >= 10 && unpaired_none {
                    let mut help = column![self.ptb("Can't see your phone?", style::BODY, p.ink_strong), self.pt("Check both are on the same Wi-Fi and the KDE Connect app is open on the phone.", style::META, p.ink_muted)].spacing(style::SPACE_3);
                    if self.phone.ufw == Some(true) {
                        let fw: Element<'_, Message> = match &d.firewall {
                            Some(Ok(())) => w::pill(p, "Firewall allows KDE Connect", Some(p.success)),
                            Some(Err(e)) => self.pt(format!("Couldn't change the firewall: {e}"), style::META, p.danger.ink).into(),
                            None => row![
                                w::text_button(p, &self.icons, if d.firewall_busy { "Asking…" } else { "Allow KDE Connect…" }, Some("shield"), None, Variant::Secondary, (!d.firewall_busy).then(|| pm(PhoneMsg::AllowFirewall))),
                                self.pt("The firewall may be blocking it. This asks for your password once and opens ports 1714–1764.", style::LABEL, p.ink_muted).width(Length::Fill),
                            ]
                            .spacing(style::SPACE_3)
                            .align_y(Alignment::Center)
                            .into(),
                        };
                        help = help.push(fw);
                    }
                    let ip = text_input("192.168.1.20", &d.ip).id(crate::phone::IP_ID).on_input(|t| pm(PhoneMsg::IpDraft(t))).on_submit(pm(PhoneMsg::IpSubmit)).size(style::BODY).font(style::FONT).padding([5, 10]).width(200).style(w::field_style(p, false));
                    help = help.push(row![self.pt("Or type the phone's IP:", style::META, p.ink_muted), ip, w::text_button(p, &self.icons, "Connect", None, None, Variant::Secondary, (!d.ip.trim().is_empty()).then(|| pm(PhoneMsg::IpSubmit)))].spacing(style::SPACE_3).align_y(Alignment::Center));
                    body = body.push(container(help).padding(style::SPACE_4).width(Length::Fill).style(bordered(p, p.bg_deep, p.line)));
                }
            }
        }
        Some(self.dialog_frame_with("phone", "Connect your phone".into(), body.into(), foot, false, false, d.opened))
    }
}

/// Free and total bytes of the phone's storage, when the phone reports them.
pub fn fs_space(p: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs fills the zeroed struct we own.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 || s.f_blocks == 0 {
        return None;
    }
    let bs = s.f_frsize.max(1);
    Some((s.f_bavail * bs, s.f_blocks * bs))
}

//! The files screen (design system `AppWindow`): tab strip, toolbar, sidebar, one or two
//! panes, the preview pane and the status bar — with menus, dialogs, toasts and the
//! command palette stacked on top.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use ef_config::{self as config, Scope};
use ef_core::fmt;
use ef_core::trash;
use iced::widget::{button, column, container, mouse_area, pin, row, scrollable, stack, text, text_input, tooltip, Space};
use iced::{mouse, Alignment, Background, Border, Color, Element, Length, Padding};

use crate::actions::FileMsg;
use crate::app::{App, Message, PATH_ID, RENAME_ID, SEARCH_ID};
use crate::drives::display_name;
use crate::file_list::{self, FileList};
use crate::indexer::{self, IndexState};
use crate::overlay::{MenuFor, UiMsg};
use crate::pane::Pane;
use crate::settings::SettingsMsg;
use crate::style::{self, color};
use crate::widgets::{self as w, Variant};

const TABS_H: f32 = 32.0;

/// A sidebar section that folds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fold {
    Windows,
    Network,
    Phone,
}

impl App {
    fn glyph<'a>(&self, name: &str, size: f32, tint: Color) -> Element<'a, Message> {
        w::glyph(&self.icons, name, size, tint)
    }

    pub(crate) fn tip<'a>(&self, content: impl Into<Element<'a, Message>>, label: &str, key: Option<&str>) -> Element<'a, Message> {
        let p = self.palette.clone();
        let mut r = row![text(label.to_string()).size(style::LABEL).font(style::FONT).color(color(p.ink))].spacing(style::SPACE_3).align_y(Alignment::Center);
        if let Some(k) = key {
            r = r.push(w::kbd_chip(&p, k));
        }
        let bubble = container(r).padding([3, 8]).style(move |_| container::Style {
            background: Some(Background::Color(color(p.bg_raised))),
            border: Border { color: color(p.line_strong), width: 1.0, radius: 2.0.into() },
            ..Default::default()
        });
        tooltip(content, bubble, tooltip::Position::Bottom).gap(6).delay(std::time::Duration::from_millis(500)).into()
    }

    fn tool<'a>(&self, icon: &str, label: &str, key: Option<&str>, msg: Option<Message>, active: bool) -> Element<'a, Message> {
        self.tip(w::icon_button(&self.palette, &self.icons, icon, msg, active), label, key)
    }

    /// Breadcrumb segments: Home or the drive's name first, never raw mount paths.
    pub(crate) fn crumbs_for(&self, location: &Path) -> Vec<(Option<&'static str>, String, PathBuf)> {
        let home = config::home();
        let phone = self.phone_storage().filter(|s| location.starts_with(s)).map(Path::to_path_buf);
        let (base, mut out) = if let Some(storage) = phone {
            // The phone's storage: its name, never kdeconnect@192.168.….
            let rest = location.strip_prefix(&storage).unwrap_or(Path::new("")).to_path_buf();
            let name = self.phone_id().map(|i| self.phone_name(&i)).unwrap_or_else(|| "Phone".into());
            (rest, vec![(Some("phone"), name, storage)])
        } else if let Ok(rest) = location.strip_prefix(&home) {
            (rest.to_path_buf(), vec![(Some("home"), "Home".to_string(), home.clone())])
        } else if let Some((m, name)) = self.net_place_at(location) {
            // A network place: its name, never GVfs' `smb-share:server=…` folder.
            let rest = location.strip_prefix(&m.root).unwrap_or(Path::new("")).to_path_buf();
            (rest, vec![(Some("network"), name, m.root.clone())])
        } else if let Some((v, mp)) = self.volumes.iter().find_map(|v| v.mount_points.iter().find(|m| location.starts_with(m.as_str())).map(|m| (v, m.clone()))) {
            let rest = location.strip_prefix(&mp).unwrap_or(Path::new("")).to_path_buf();
            (rest, vec![(Some("drive"), display_name(v, &self.volumes), PathBuf::from(mp))])
        } else {
            (location.strip_prefix("/").unwrap_or(location).to_path_buf(), vec![(Some("drive"), "Computer".to_string(), PathBuf::from("/"))])
        };
        let mut acc = out[0].2.clone();
        for c in base.components() {
            acc.push(c);
            out.push((None, c.as_os_str().to_string_lossy().into_owned(), acc.clone()));
        }
        out
    }

    pub(crate) fn files_view(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mut center = column![].width(Length::Fill).height(Length::Fill);
        if let Some(n) = &self.notice {
            center = center.push(self.banner(n));
        }
        if let Some(b) = self.drive_banner() {
            center = center.push(b);
        }
        let tab = &self.tabs[self.tab];
        let panes: Element<'_, Message> = if tab.panes.len() == 2 {
            row![self.pane_view(&tab.panes[0], tab.active == 0, true), w::vline(p.line), self.pane_view(&tab.panes[1], tab.active == 1, true)].height(Length::Fill).into()
        } else {
            self.pane_view(&tab.panes[0], true, false)
        };
        center = center.push(panes);
        let mut body = row![].height(Length::Fill);
        if !self.settings.sidebar.hidden {
            body = body.push(self.sidebar());
            body = body.push(self.resize_handle());
        }
        body = body.push(center);
        if self.settings.appearance.preview && !self.pane().special() {
            body = body.push(w::vline(p.line));
            body = body.push(self.preview_view());
        }
        let root = column![self.tab_strip(), self.toolbar(), w::hline(p.line), body, w::hline(p.line), self.status_bar()];
        let base: Element<'_, Message> = w::fill(root, p.bg).width(Length::Fill).height(Length::Fill).into();
        let mut layers = vec![base];
        if let Some(g) = self.drag_ghost() {
            layers.push(g);
        }
        if let Some(t) = self.toast_layer() {
            layers.push(t);
        }
        if let Some(m) = self.menu_layer() {
            layers.push(m);
        }
        if let Some(c) = self.command_layer() {
            layers.push(c);
        }
        if let Some(c) = self.connect_layer() {
            layers.push(c);
        }
        if let Some(c) = self.phone_layer() {
            layers.push(c);
        }
        if let Some(d) = self.dialog_layer() {
            layers.push(d);
        }
        stack(layers).width(Length::Fill).height(Length::Fill).into()
    }

    fn resize_handle(&self) -> Element<'_, Message> {
        let line = color(if self.sidebar_drag.is_some() { self.palette.accent } else { self.palette.line });
        mouse_area(container(container(Space::new()).width(1).height(Length::Fill).style(move |_| container::Style { background: Some(Background::Color(line)), ..Default::default() })).width(5).height(Length::Fill).align_x(Alignment::Start))
            .on_press(Message::SidebarResize(Some(f32::NAN)))
            .interaction(mouse::Interaction::ResizingHorizontally)
            .into()
    }

    fn drag_ghost(&self) -> Option<Element<'_, Message>> {
        let d = self.drag.as_ref()?;
        let p = &self.palette;
        let label = if d.paths.len() == 1 { d.paths[0].file_name().unwrap_or_default().to_string_lossy().into_owned() } else { format!("{} items", d.paths.len()) };
        let verb = if self.modifiers.control() { "Copy" } else if self.modifiers.shift() { "Move" } else { "" };
        let mut r = row![self.glyph(if d.paths.len() == 1 && d.paths[0].is_dir() { "folder" } else { "file" }, 14.0, color(p.accent_ink)), text(label).size(style::META).font(style::FONT).color(color(p.ink_strong))].spacing(6).align_y(Alignment::Center);
        if !verb.is_empty() {
            r = r.push(w::kbd_chip(p, verb));
        }
        let pal = p.clone();
        let chip = container(r).padding([4, 8]).style(move |_| container::Style { background: Some(Background::Color(color(pal.bg_raised))), border: Border { color: color(pal.accent), width: 1.0, radius: 2.0.into() }, ..Default::default() });
        Some(pin(chip).x(d.at.x + 14.0).y(d.at.y + 10.0).into())
    }

    // ------------------------------------------------------------------ tabs

    fn tab_strip(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mut r = row![].spacing(0).align_y(Alignment::End).height(TABS_H);
        for (i, t) in self.tabs.iter().enumerate() {
            let active = i == self.tab;
            let pane = &t.panes[t.active];
            let icon = if pane.drives {
                "drive"
            } else if pane.phone.is_some() || self.is_phone_path(&pane.location) {
                "phone"
            } else if pane.shares.is_some() || ef_net::gvfs::is_network_path(&pane.location) {
                "network"
            } else if pane.location == config::home() {
                "home"
            } else {
                "folder"
            };
            let mut content = row![self.glyph(icon, 14.0, color(if active { p.accent_ink } else { p.ink_muted })), text(self.pane_title(pane)).size(style::META).font(if active { style::FONT_BOLD } else { style::FONT }).color(color(if active { p.ink_strong } else { p.ink_muted })).wrapping(text::Wrapping::None)]
                .spacing(6)
                .align_y(Alignment::Center);
            if t.panes.len() == 2 {
                content = content.push(self.glyph("columns", 12.0, color(p.ink_faint)));
            }
            if self.tabs.len() > 1 {
                content = content.push(w::icon_button(p, &self.icons, "close", Some(Message::CloseTab(i)), false));
            }
            let pal = p.clone();
            let tabb = button(container(content).height(TABS_H - 2.0).align_y(Alignment::Center).max_width(220).clip(true))
                .padding([0, 10])
                .on_press(Message::SelectTab(i))
                .style(move |_, status| button::Style {
                    background: Some(Background::Color(if active { color(pal.bg) } else if matches!(status, button::Status::Hovered) { color(pal.state_hover) } else { Color::TRANSPARENT })),
                    text_color: color(pal.ink),
                    border: Border::default(),
                    ..Default::default()
                });
            let accent = color(p.accent);
            let top: Element<'_, Message> = container(Space::new()).width(Length::Fill).height(2).style(move |_| container::Style { background: active.then_some(Background::Color(accent)), ..Default::default() }).into();
            let whole = column![top, tabb].width(Length::Shrink);
            let with_menu = w::context_area(mouse_area(whole).on_middle_press(Message::CloseTab(i)), move |at| Message::Ui(UiMsg::OpenMenu(MenuFor::Tab(i), at)));
            r = r.push(with_menu);
        }
        r = r.push(container(self.tool("plus", "New tab", Some("Ctrl+T"), Some(Message::NewTab), false)).height(TABS_H).align_y(Alignment::Center).padding([0, 4]));
        w::fill(r, p.bg_deep).width(Length::Fill).height(TABS_H).padding(Padding { left: 4.0, ..Padding::ZERO }).into()
    }

    // ------------------------------------------------------------------ toolbar

    fn path_bar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        if let Some(draft) = &self.path_edit {
            return text_input("Type a path — ~/Documents, /mnt/data or D:\\Work", draft)
                .id(PATH_ID)
                .on_input(Message::PathDraft)
                .on_submit(Message::PathSubmit)
                .size(style::BODY)
                .font(style::FONT)
                .padding([4, 8])
                .width(Length::Fill)
                .style(w::field_style(p, false))
                .into();
        }
        let pane = self.pane();
        let mut crumbs = if pane.drives {
            vec![(Some("drive"), "Drives".to_string(), PathBuf::new())]
        } else if let Some(page) = pane.phone {
            self.phone_crumbs(page)
        } else if let Some(page) = &pane.shares {
            vec![(Some("network"), page.address.host_port(), PathBuf::new())]
        } else {
            self.crumbs_for(&pane.location)
        };
        // Overflow: middle segments fold into "…" (a menu) when the path is long.
        let budget = (self.window_size.width - if self.settings.sidebar.hidden { 0.0 } else { self.settings.sidebar.width as f32 } - 720.0).max(160.0);
        let width = |c: &[(Option<&'static str>, String, PathBuf)]| c.iter().map(|(i, l, _)| l.chars().count() as f32 * 7.8 + 12.0 + if i.is_some() { 20.0 } else { 0.0 } + 16.0).sum::<f32>();
        let mut hidden: Vec<(String, PathBuf)> = Vec::new();
        while crumbs.len() > 3 && width(&crumbs) > budget {
            let (_, l, t) = crumbs.remove(1);
            hidden.push((l, t));
        }
        let n = crumbs.len();
        let everywhere = self.everywhere();
        let mut trail = row![].spacing(2).align_y(Alignment::Center);
        for (idx, (icon, label, target)) in crumbs.into_iter().enumerate() {
            if idx > 0 {
                trail = trail.push(self.glyph("chevron-right", 12.0, color(p.ink_faint)));
            }
            if idx == 1 && !hidden.is_empty() {
                let items = hidden.clone();
                let more = w::context_area(w::icon_button(p, &self.icons, "more-horizontal", Some(Message::Ui(UiMsg::OpenMenu(MenuFor::Crumbs(items.clone()), iced::Point::new(self.settings.sidebar.width as f32 + 180.0, 72.0)))), false), move |at| Message::Ui(UiMsg::OpenMenu(MenuFor::Crumbs(items.clone()), at)));
                trail = trail.push(more);
                trail = trail.push(self.glyph("chevron-right", 12.0, color(p.ink_faint)));
            }
            let last = idx == n - 1 && !everywhere;
            let ink = if last { color(p.ink_strong) } else { color(p.ink_muted) };
            let mut content = row![].spacing(style::SPACE_1 + 2.0).align_y(Alignment::Center);
            if let Some(icon) = icon {
                content = content.push(self.glyph(icon, 14.0, ink));
            }
            content = content.push(text(label).size(style::BODY).font(if last { style::FONT_BOLD } else { style::FONT }).color(ink).wrapping(text::Wrapping::None));
            let pal = p.clone();
            let hover_ink = color(p.ink_strong);
            let target_ok = !target.as_os_str().is_empty();
            let seg = button(container(content).height(24).align_y(Alignment::Center))
                .padding([0, 6])
                .on_press_maybe((!last && target_ok).then(|| Message::Navigate(target.clone())))
                .style(move |_, status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(Background::Color(color(pal.state_hover))),
                        button::Status::Pressed => Some(Background::Color(color(pal.state_press))),
                        _ => None,
                    },
                    text_color: if status == button::Status::Hovered { hover_ink } else { ink },
                    border: Border { radius: 2.0.into(), ..Border::default() },
                    ..Default::default()
                });
            // Drop files on a segment to move them up there.
            let seg: Element<'_, Message> = if self.drag.is_some() && target_ok {
                mouse_area(seg).on_enter(Message::Ui(UiMsg::DropHover(Some(target.clone())))).on_exit(Message::Ui(UiMsg::DropLeave(target.clone()))).into()
            } else {
                seg.into()
            };
            trail = trail.push(seg);
        }
        if everywhere {
            trail = trail.push(self.glyph("chevron-right", 12.0, color(p.ink_faint)));
            trail = trail.push(container(text("Search results").size(style::BODY).font(style::FONT_BOLD).color(color(p.ink_strong)).wrapping(text::Wrapping::None)).padding([0, 6]));
        }
        // Double-click the empty part of the bar to type a path (Ctrl+L).
        let filler = mouse_area(container(Space::new()).width(Length::Fill).height(24)).on_double_click(Message::EditPath(true)).on_press(Message::Ui(UiMsg::CloseMenu));
        container(row![trail, filler].align_y(Alignment::Center)).width(Length::Fill).clip(true).into()
    }

    fn toolbar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let pane = self.pane();
        let placeholder = match pane.scope {
            Scope::Folder => "Search this folder",
            Scope::Everywhere => "Search everywhere",
        };
        let search = text_input(placeholder, &pane.query)
            .id(SEARCH_ID)
            .on_input(Message::Search)
            .on_submit(Message::SearchSubmit)
            .size(style::BODY)
            .font(style::FONT)
            .padding([5, 10])
            .width(220)
            .style(w::field_style(p, false));
        let scope = w::segmented(p, &[("Folder", Message::SetScope(Scope::Folder)), ("Everywhere", Message::SetScope(Scope::Everywhere))], if pane.scope == Scope::Folder { 0 } else { 1 });
        let sep = || {
            let c = color(p.line);
            container(Space::new()).width(1).height(18).style(move |_| container::Style { background: Some(Background::Color(c)), ..Default::default() })
        };
        let bar = row![
            self.tool("arrow-left", "Back", Some("Alt+←"), (!pane.back.is_empty()).then_some(Message::Back), false),
            self.tool("arrow-right", "Forward", Some("Alt+→"), (!pane.forward.is_empty()).then_some(Message::Forward), false),
            self.tool("arrow-up", "Parent folder", Some("Alt+↑"), pane.location.parent().map(|_| Message::Up), false),
            self.tool("refresh", "Reload", Some("Ctrl+R"), Some(Message::Reload), false),
            sep(),
            self.path_bar(),
            self.tip(scope, "Where to search", Some("Ctrl+E")),
            search,
            sep(),
            self.view_menu_button(),
            self.tool("command", "Command palette", Some("Ctrl+K"), Some(Message::Ui(UiMsg::OpenCommand)), false),
            self.tool("settings", "Settings", Some("Ctrl+,"), Some(Message::Settings(SettingsMsg::Open)), false),
        ]
        .spacing(style::SPACE_1)
        .align_y(Alignment::Center);
        container(bar).height(style::TOOLBAR).padding([0, 12]).align_y(Alignment::Center).width(Length::Fill).into()
    }

    /// View dropdown: the current layout's glyph and a chevron; the menu holds the layout
    /// choice and the pane/hidden-file toggles with a check on what's on.
    fn view_menu_button(&self) -> Element<'_, Message> {
        let p = &self.palette;
        // Anchor under the button, right-aligned: the toolbar ends with this, Command, Settings.
        let right = self.window_size.width - 12.0 - 2.0 * (28.0 + style::SPACE_1);
        let at = iced::Point::new(right - 272.0, TABS_H + style::TOOLBAR - 2.0);
        let open = Message::Ui(UiMsg::OpenMenu(MenuFor::View, at));
        let ink = color(p.ink_muted);
        let content = row![self.glyph(if self.pane().grid { "grid" } else { "list" }, style::GLYPH, ink), self.glyph("chevron-down", 12.0, ink)].spacing(2).align_y(Alignment::Center);
        let pal = p.clone();
        let b = button(container(content).height(28).padding([0, 6]).align_y(Alignment::Center)).padding(0).on_press(open).style(move |_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(color(pal.state_hover))),
                button::Status::Pressed => Some(Background::Color(color(pal.state_press))),
                _ => None,
            },
            text_color: color(pal.ink),
            border: Border { radius: 4.0.into(), ..Border::default() },
            ..Default::default()
        });
        self.tip(b, "View", None)
    }

    // ------------------------------------------------------------------ sidebar

    fn section<'a>(&self, title: &str, mark: Option<Color>, count: Option<usize>) -> Element<'a, Message> {
        let p = &self.palette;
        let mut r = row![].spacing(style::SPACE_3).align_y(Alignment::Center);
        if let Some(c) = mark {
            r = r.push(container(Space::new()).width(6).height(6).style(move |_| container::Style { background: Some(Background::Color(c)), ..Default::default() }));
        }
        r = r.push(text(title.to_uppercase()).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted)).width(Length::Fill));
        if let Some(n) = count {
            r = r.push(text(n.to_string()).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)));
        }
        container(r).padding([4, 12]).into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let home = config::home();
        let places = [
            ("home", "Home", home.clone()),
            ("file", "Documents", home.join("Documents")),
            ("download", "Downloads", home.join("Downloads")),
            ("image", "Pictures", home.join("Pictures")),
            ("music", "Music", home.join("Music")),
            ("video", "Videos", home.join("Videos")),
            ("trash", "Trash", trash::home_trash().join("files")),
        ];
        let mut col = column![self.section("Linux", Some(color(p.world_linux)), None)].spacing(1);
        for (icon, label, path) in places {
            if path.is_dir() || label == "Trash" {
                col = col.push(self.side_item(icon, label.to_string(), path, false, 0.0));
            }
        }
        if !self.settings.sidebar.pinned.is_empty() {
            col = col.push(Space::new().height(style::SPACE_5));
            col = col.push(self.section("Pinned", None, None));
            for s in &self.settings.sidebar.pinned {
                let path = config::expand(s);
                let label = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| s.clone());
                col = col.push(self.side_item("pin", label, path, true, 0.0));
            }
        }
        if self.settings.sidebar.windows {
            col = col.push(Space::new().height(style::SPACE_5));
            col = col.push(self.windows_section());
        }
        if self.settings.sidebar.phone {
            col = col.push(Space::new().height(style::SPACE_5));
            col = col.push(self.phone_section());
        }
        if self.settings.sidebar.network {
            col = col.push(Space::new().height(style::SPACE_5));
            col = col.push(self.network_section());
        }
        // A thin scrollbar beside the rows, never over them.
        let bar = scrollable::Scrollbar::new().width(4).scroller_width(4).spacing(4);
        w::fill(scrollable(col).direction(scrollable::Direction::Vertical(bar)).height(Length::Fill), p.bg_sunken).width(self.settings.sidebar.width as f32).height(Length::Fill).padding([12, 8]).into()
    }

    /// A section head that folds its section: chevron, world mark, title, count, and an
    /// optional action at the end (design system `SidebarSection` with `collapsible`).
    pub(crate) fn fold_head<'a>(&'a self, title: &str, mark: Color, count: Option<usize>, open: bool, section: Fold, action: Option<Element<'a, Message>>) -> Element<'a, Message> {
        let p = &self.palette;
        let muted = color(p.ink_muted);
        let mut r = row![
            self.glyph(if open { "chevron-down" } else { "chevron-right" }, 12.0, muted),
            container(Space::new()).width(6).height(6).style(move |_| container::Style { background: Some(Background::Color(mark)), ..Default::default() }),
            text(title.to_uppercase()).size(style::LABEL).font(style::FONT_BOLD).color(muted).width(Length::Fill),
        ]
        .spacing(style::SPACE_3)
        .align_y(Alignment::Center);
        if let Some(n) = count {
            r = r.push(text(n.to_string()).size(style::LABEL).font(style::FONT).color(muted));
        }
        if let Some(a) = action {
            r = r.push(a);
        }
        let head = w::row_button(p, container(r).height(24).align_y(Alignment::Center).into(), false, Some(Message::Fold(section, !open)));
        self.tip(head, if open { "Collapse" } else { "Expand" }, None)
    }

    /// Windows drives. Collapsed, only the drive you're in (or All drives, when that's
    /// open) stays, so the sidebar always shows where you are.
    fn windows_section(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let open = self.settings.sidebar.windows_open;
        let pane = self.pane();
        let mut col = column![self.fold_head("Windows", color(p.world_windows), (!self.volumes.is_empty()).then_some(self.volumes.len()), open, Fold::Windows, None)].spacing(1);
        if open && self.volumes.is_empty() {
            col = col.push(container(text("No Windows drives found").size(style::META).font(style::FONT).color(color(p.ink_muted))).padding([4, 12]));
        }
        let here = if pane.special() { None } else { self.volume_at(&pane.location).map(|(i, _)| i) };
        for (i, v) in self.volumes.iter().enumerate() {
            if open || here == Some(i) {
                col = col.push(self.drive_item(i, v));
            }
        }
        if open || pane.drives {
            if open {
                col = col.push(Space::new().height(style::SPACE_4));
            }
            col = col.push(self.side_button("sliders", "All drives", Message::ShowDrives, pane.drives));
        }
        col.into()
    }

    pub(crate) fn side_button<'a>(&'a self, icon: &str, label: &str, msg: Message, active: bool) -> Element<'a, Message> {
        let p = &self.palette;
        let tint = if active { color(p.accent_ink) } else { color(p.ink_muted) };
        let r = row![self.glyph(icon, 16.0, tint), text(label.to_string()).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(color(if active { p.ink_strong } else { p.ink })).width(Length::Fill)]
            .spacing(style::SPACE_4)
            .align_y(Alignment::Center);
        w::row_button(p, container(r).height(style::ROW).align_y(Alignment::Center).into(), active, Some(msg))
    }

    /// A place in the sidebar: navigates on click, takes dropped files, has a menu.
    fn side_item<'a>(&'a self, icon: &str, label: String, path: PathBuf, pinned: bool, indent: f32) -> Element<'a, Message> {
        let p = &self.palette;
        let pane = self.pane();
        let active = pane.location == path && !self.everywhere() && !pane.special();
        let drop = self.drag.is_some() && self.drop_place.as_ref() == Some(&path);
        let tint = if active || drop { color(p.accent_ink) } else { color(p.ink_muted) };
        let ink = if active { color(p.ink_strong) } else { color(p.ink) };
        let r = row![
            self.glyph(icon, 16.0, tint),
            text(label).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(ink).width(Length::Fill).wrapping(text::Wrapping::None)
        ]
        .spacing(style::SPACE_4)
        .align_y(Alignment::Center);
        let inner = container(r).height(style::ROW).align_y(Alignment::Center).padding(Padding { left: indent, ..Padding::ZERO });
        let item: Element<'a, Message> = if drop {
            let pal = p.clone();
            container(w::row_button(p, inner.into(), false, Some(Message::Navigate(path.clone()))))
                .style(move |_| container::Style { background: Some(Background::Color(color(pal.accent_soft))), border: Border { color: color(pal.accent), width: 1.0, radius: 2.0.into() }, ..Default::default() })
                .into()
        } else {
            w::row_button(p, inner.into(), active, Some(Message::Navigate(path.clone())))
        };
        let (enter, target) = (path.clone(), path.clone());
        let item = mouse_area(item).on_middle_press(Message::OpenTab(path.clone())).on_enter(Message::Ui(UiMsg::DropHover(Some(enter)))).on_exit(Message::Ui(UiMsg::DropLeave(path.clone())));
        w::context_area(item, move |at| Message::Ui(UiMsg::OpenMenu(MenuFor::Place { path: target.clone(), pinned }, at)))
    }

    fn banner<'a>(&'a self, message: &'a str) -> Element<'a, Message> {
        let p = &self.palette;
        let tone = p.warning;
        container(
            row![
                self.glyph("alert", 16.0, color(tone.ink)),
                text(message).size(style::META).font(style::FONT).color(color(p.ink)).width(Length::Fill),
                self.tool("close", "Dismiss", None, Some(Message::DismissNotice), false),
            ]
            .spacing(style::SPACE_4)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(iced::Padding { left: 16.0, ..iced::Padding::from([6, 8]) })
        .style(move |_| container::Style { background: Some(Background::Color(color(tone.soft))), ..Default::default() })
        .into()
    }

    // ------------------------------------------------------------------ panes

    fn pane_view<'a>(&'a self, pane: &'a Pane, active: bool, dual: bool) -> Element<'a, Message> {
        let p = &self.palette;
        let content: Element<'a, Message> = if pane.drives {
            self.drives_view()
        } else if let Some(page) = &pane.shares {
            self.shares_view(page)
        } else if let Some(page) = pane.phone {
            self.phone_page_view(page)
        } else if pane.everywhere() {
            self.results_view(pane)
        } else {
            self.list(pane, active)
        };
        let mut col = column![];
        if dual {
            // Each pane says where it is and which world it's in; the active one carries
            // the accent edge.
            let world = if pane.shares.is_some() || ef_net::gvfs::is_network_path(&pane.location) {
                p.world_network
            } else if self.volume_at(&pane.location).is_some() {
                p.world_windows
            } else {
                p.world_linux
            };
            let edge = if active { color(p.accent) } else { color(p.line) };
            col = col.push(container(Space::new()).width(Length::Fill).height(2).style(move |_| container::Style { background: Some(Background::Color(edge)), ..Default::default() }));
            let wc = color(world);
            let head = row![
                container(Space::new()).width(6).height(6).style(move |_| container::Style { background: Some(Background::Color(wc)), ..Default::default() }),
                text(if pane.special() { pane.title() } else { self.net_uri_for(&pane.location).unwrap_or_else(|| config::tilde(&pane.location)) }).size(style::META).font(if active { style::FONT_BOLD } else { style::FONT }).color(color(if active { p.ink_strong } else { p.ink_muted })).wrapping(text::Wrapping::None),
            ]
            .spacing(style::SPACE_3)
            .align_y(Alignment::Center);
            let bg = if active { p.bg } else { p.bg_sunken };
            col = col.push(w::fill(container(head).height(24).align_y(Alignment::Center).padding([0, 16]).clip(true), bg).width(Length::Fill));
            col = col.push(w::hline(p.line));
        }
        col = col.push(content);
        if let Some(r) = self.rename.as_ref().filter(|r| r.pane == pane.id && r.error.is_some()) {
            let tone = p.danger;
            col = col.push(
                container(row![self.glyph("error", 14.0, color(tone.ink)), text(r.error.clone().unwrap_or_default()).size(style::META).font(style::FONT).color(color(tone.ink))].spacing(6).align_y(Alignment::Center))
                    .width(Length::Fill)
                    .padding([4, 16])
                    .style(move |_| container::Style { background: Some(Background::Color(color(tone.soft))), ..Default::default() }),
            );
        }
        let id = pane.id;
        // Clicks anywhere in a pane make it the active one.
        mouse_area(container(col).width(Length::Fill).height(Length::Fill)).on_press(Message::List(id, file_list::Action::Focus)).into()
    }

    fn list<'a>(&'a self, pane: &'a Pane, active: bool) -> Element<'a, Message> {
        let Some(loaded) = &pane.loaded else { return container(Space::new()).width(Length::Fill).height(Length::Fill).into() };
        let empty = (!pane.skeleton && pane.order.is_empty()).then(|| {
            if !pane.query.is_empty() {
                (format!("No matches for “{}”", pane.query), "Nothing in this folder matches. Esc clears the search · Ctrl+E searches everywhere.".to_string())
            } else if trash::is_trash_files(&pane.location) {
                ("The Trash is empty".to_string(), "Things you delete wait here until you empty the Trash.".to_string())
            } else if loaded.listing.is_empty() {
                ("This folder is empty".to_string(), "Right-click for New folder, or paste with Ctrl+V.".to_string())
            } else {
                ("Only hidden files here".to_string(), "Press Ctrl+H to show them.".to_string())
            }
        });
        let renaming = self.rename.as_ref().filter(|r| r.pane == pane.id).and_then(|r| r.pos);
        let editor: Option<Element<'a, Message>> = self.rename.as_ref().filter(|_| renaming.is_some()).map(|r| {
            let pal = self.palette.clone();
            text_input("", &r.value)
                .id(RENAME_ID)
                .on_input(|v| Message::File(FileMsg::RenameDraft(v)))
                .on_submit(Message::File(FileMsg::RenameCommit))
                .size(style::BODY)
                .font(style::FONT)
                .padding([3, 6])
                .style(move |_, _| text_input::Style {
                    background: Background::Color(Color::TRANSPARENT),
                    border: Border::default(),
                    icon: color(pal.ink_muted),
                    placeholder: color(pal.ink_muted),
                    value: color(pal.ink_strong),
                    selection: color(pal.selection),
                })
                .into()
        });
        let l = &loaded.listing;
        let thumbs = &self.thumbs;
        let grid = pane.grid;
        let thumb: Box<dyn Fn(usize) -> Option<iced::advanced::image::Handle> + 'a> = Box::new(move |i: usize| if grid { thumbs.get(&l.path(i)) } else { None });
        let id = pane.id;
        FileList::new(
            file_list::Model {
                listing: l,
                order: &pane.order,
                selected: &pane.selected,
                cut: &pane.cut,
                cursor: pane.cursor,
                generation: pane.generation,
                sort: pane.sort,
                skeleton: pane.skeleton,
                empty,
                row_h: self.settings.appearance.density.row_height(),
                grid,
                keys: active && !self.modal() && self.rename.is_none() && self.path_edit.is_none(),
                active,
                dragging: self.drag.is_some(),
                renaming,
                thumb,
            },
            &self.palette,
            &self.icons,
            &self.dates,
            move |a| Message::List(id, a),
        )
        .editor(editor)
        .into()
    }

    // ------------------------------------------------------------------ search results

    fn results_view<'a>(&'a self, pane: &'a Pane) -> Element<'a, Message> {
        let p = &self.palette;
        let Some(res) = &pane.results else {
            let msg = if pane.searching { "Searching…" } else { "Type to search" };
            return container(text(msg).size(style::META).font(style::FONT).color(color(p.ink_muted))).center(Length::Fill).into();
        };
        if res.hits.is_empty() {
            let hint = if res.from_index {
                format!("Nothing in the index matches. It was updated {}; new files in other folders can take up to a minute.", indexer::ago(res.index_updated))
            } else {
                "Nothing in your indexed folders matches.".to_string()
            };
            return container(
                column![
                    self.glyph("search", 40.0, color(p.ink_faint)),
                    text(format!("No matches for “{}”", res.query)).size(18).font(style::FONT_BOLD).color(color(p.ink_strong)),
                    text(hint).size(style::META).font(style::FONT).color(color(p.ink_muted)),
                ]
                .spacing(style::SPACE_4)
                .align_x(Alignment::Center),
            )
            .center(Length::Fill)
            .into();
        }
        let header = {
            let label = |s: &str| text(s.to_string()).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted));
            container(
                row![
                    Space::new().width(style::ICON_ROW),
                    label("NAME").width(Length::FillPortion(3)),
                    label("LOCATION").width(Length::FillPortion(4)),
                    container(label("SIZE")).width(style::COL_SIZE).align_x(Alignment::End),
                    label("MODIFIED").width(style::COL_DATE),
                ]
                .spacing(style::SPACE_4)
                .align_y(Alignment::Center),
            )
            .height(style::HEADER)
            .padding([0, 16])
            .align_y(Alignment::Center)
        };
        let row_h = self.settings.appearance.density.row_height();
        let mut rows = column![].spacing(0).padding([0, 4]);
        let mut buf = String::new();
        for (i, h) in res.hits.iter().enumerate() {
            let active = pane.result_cursor == Some(i);
            let icon = if h.is_dir { "folder" } else { crate::kinds::icon(h.name.as_bytes(), ef_core::listing::Kind::File) };
            buf.clear();
            if !h.is_dir {
                fmt::size(h.size, &mut buf);
            } else {
                buf.push('—');
            }
            let size = buf.clone();
            buf.clear();
            self.dates.format(h.mtime, &mut buf);
            let date = buf.clone();
            let meta = |s: String| text(s).size(style::META).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None);
            let content = row![
                iced::widget::svg(self.icons.color(icon)).width(style::ICON_ROW).height(style::ICON_ROW),
                container(text(h.name.clone()).size(style::BODY).font(style::FONT).color(color(if active { p.ink_strong } else { p.ink })).wrapping(text::Wrapping::None)).width(Length::FillPortion(3)).clip(true),
                container(meta(h.location.clone())).width(Length::FillPortion(4)).clip(true),
                container(meta(size)).width(style::COL_SIZE).align_x(Alignment::End),
                meta(date).width(style::COL_DATE),
            ]
            .spacing(style::SPACE_4)
            .align_y(Alignment::Center);
            let item = w::row_button(p, container(content).height(row_h).align_y(Alignment::Center).into(), active, Some(Message::ResultClick(i)));
            rows = rows.push(mouse_area(item).on_double_click(Message::ResultOpen(i)));
        }
        let footer = {
            let mut r = row![].spacing(style::SPACE_4).align_y(Alignment::Center);
            r = r.push(w::kbd_chip(p, "Enter"));
            r = r.push(text("open").size(style::META).font(style::FONT).color(color(p.ink_muted)));
            r = r.push(w::kbd_chip(p, "Alt+Enter"));
            r = r.push(text("show in folder").size(style::META).font(style::FONT).color(color(p.ink_muted)));
            r = r.push(Space::new().width(Length::Fill));
            if let Some(i) = pane.result_cursor {
                r = r.push(w::text_button(p, &self.icons, "Show in folder", Some("folder"), None, Variant::Secondary, Some(Message::ResultReveal(i))));
            }
            container(r).padding([6, 16]).width(Length::Fill)
        };
        column![header, w::hline(p.line), scrollable(rows).height(Length::Fill), w::hline(p.line), footer].height(Length::Fill).into()
    }

    // ------------------------------------------------------------------ status bar

    fn status_bar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let pane = self.pane();
        let mut left = String::new();
        if pane.drives {
            left.push_str(&format!("{} Windows {}", self.volumes.len(), if self.volumes.len() == 1 { "drive" } else { "drives" }));
        } else if let Some(page) = pane.phone {
            left.push_str(&self.phone_status(page));
        } else if let Some(page) = &pane.shares {
            match &page.shares {
                Some(Ok(v)) => left.push_str(&format!("{} {}", v.len(), if v.len() == 1 { "share" } else { "shares" })),
                Some(Err(_)) => left.push_str("Couldn't read the shares"),
                None => left.push_str("Reading shares…"),
            }
        } else if pane.everywhere() {
            match &pane.results {
                Some(r) => {
                    fmt::count(r.total, &mut left);
                    left.push_str(if r.total == 1 { " match" } else { " matches" });
                    if r.total > r.hits.len() {
                        left.push_str(" · showing first ");
                        fmt::count(r.hits.len(), &mut left);
                    }
                    left.push_str(if r.from_index { "  ·  from the index" } else { "  ·  live search (index off)" });
                }
                None if pane.searching => left.push_str("Searching…"),
                None => {}
            }
        } else {
            let total = pane.loaded.as_ref().map_or(0, |l| l.listing.len() - if self.show_hidden { 0 } else { l.hidden });
            if !pane.query.is_empty() {
                fmt::count(pane.order.len(), &mut left);
                left.push_str(" of ");
                fmt::count(total, &mut left);
                left.push_str(" match");
            } else {
                fmt::count(pane.order.len(), &mut left);
                left.push_str(if pane.order.len() == 1 { " item" } else { " items" });
            }
            let (files, dirs, bytes) = pane.selection_stats();
            if files + dirs > 0 {
                left.push_str("  ·  ");
                fmt::count(files + dirs, &mut left);
                left.push_str(" selected · ");
                // Folder sizes are counted in the background: "12.4 MB+" until done.
                let (extra, done) = match &self.sel_size {
                    Some(s) if dirs > 0 => (s.1.bytes.load(Ordering::Relaxed), s.1.done.load(Ordering::Relaxed)),
                    _ => (0, dirs == 0),
                };
                fmt::size(bytes + extra, &mut left);
                if !done {
                    left.push('+');
                }
            }
            if let Some(l) = &pane.loaded
                && !self.show_hidden && l.hidden > 0 && pane.query.is_empty() {
                    left.push_str("  ·  ");
                    fmt::count(l.hidden, &mut left);
                    left.push_str(" hidden");
                }
            if let Some(c) = self.clip.as_ref() {
                left.push_str(&format!("  ·  {} {} on the clipboard", c.paths.len(), if c.cut { "cut" } else { "copied" }));
            }
        }
        let mut right = String::new();
        if pane.loaded.as_ref().is_some_and(|l| !l.metadata_ready) {
            right.push_str("Reading details…  ·  ");
        }
        if std::env::var_os("ECHOFILES_TIMING").is_some() && let Some(l) = pane.loaded.as_ref().filter(|l| l.metadata_ready) {
            right.push_str(&format!("names {:.1} · sort {:.1} · details {:.1} · paint {:.1} ms  ·  ", l.names_ms, l.sort_ms, l.meta_ms, pane.first_paint_ms));
        }
        let running = self.transfers.iter().filter(|t| t.running()).count();
        if running > 0 {
            right.push_str(&format!("{running} {} running  ·  ", if running == 1 { "transfer" } else { "transfers" }));
        }
        if self.index_state == IndexState::Building {
            right.push_str("Indexing…  ·  ");
        }
        // The volume: its name, driver and state repeat here so read-only is never a surprise.
        let mut pill: Option<Element<'_, Message>> = None;
        let net = if pane.special() { None } else { self.net_place_at(&pane.location) };
        if !pane.special() && self.is_phone_path(&pane.location) {
            right.push_str(&self.phone_id().map(|i| self.phone_name(&i)).unwrap_or_default());
            right.push_str(" · phone · SFTP");
        } else if let Some((m, name)) = &net {
            // Network: which server and how, instead of FUSE's made-up free space.
            right.push_str(name);
            right.push_str(" · ");
            right.push_str(m.protocol().map_or(m.kind.as_str(), |p| p.label()));
            if pane.fs.as_ref().is_some_and(|f| f.read_only) {
                pill = Some(w::pill(p, "Read-only", Some(p.warning)));
            }
        } else if let Some(f) = pane.fs.as_ref().filter(|_| !pane.special()) {
            match self.volume_at(&pane.location) {
                Some((_, v)) => {
                    right.push_str(&display_name(v, &self.volumes));
                    right.push_str(" · ");
                }
                None => right.push_str("Linux · "),
            }
            right.push_str(f.fs_type);
            right.push_str(" · ");
            fmt::size(f.free, &mut right);
            right.push_str(" free");
            if f.read_only {
                pill = Some(w::pill(p, "Read-only", Some(p.warning)));
            }
        }
        let mut bar = row![
            text(left).size(style::META).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None),
            Space::new().width(Length::Fill),
            text(right).size(style::META).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None),
        ]
        .spacing(style::SPACE_3)
        .align_y(Alignment::Center);
        if let Some(pl) = pill {
            bar = bar.push(pl);
        }
        w::fill(bar, p.bg_deep).height(style::STATUSBAR).padding([0, 16]).align_y(Alignment::Center).width(Length::Fill).into()
    }
}

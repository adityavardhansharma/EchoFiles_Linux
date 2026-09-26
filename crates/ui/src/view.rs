//! The files view: toolbar, sidebar, file list or "Everywhere" results, and status bar.

use std::path::{Path, PathBuf};

use ef_config::{self as config, Scope};
use ef_core::fmt;
use ef_disks::Volume;
use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input, tooltip, Space};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::app::{App, Message, SEARCH_ID};
use crate::file_list::{self, FileList};
use crate::indexer::{self, IndexState};
use crate::settings::SettingsMsg;
use crate::style::{self, color};
use crate::widgets::{self as w, Variant};

/// Windows' own name for a volume: its label, or "Local Disk" when it has none. Duplicate
/// labels get their size so two partitions with the same label can be told apart.
pub fn drive_name(v: &Volume, all: &[Volume]) -> String {
    let base = if v.label.is_empty() { "Local Disk".to_string() } else { v.label.clone() };
    if all.iter().filter(|o| o.label == v.label).count() > 1 {
        let mut s = format!("{base} · ");
        fmt::size(v.size, &mut s);
        s
    } else {
        base
    }
}

impl App {
    fn glyph<'a>(&self, name: &str, size: f32, tint: Color) -> Element<'a, Message> {
        w::glyph(&self.icons, name, size, tint)
    }

    fn tip<'a>(&self, content: Element<'a, Message>, label: &str, key: Option<&str>) -> Element<'a, Message> {
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
        tooltip(content, bubble, tooltip::Position::Bottom).gap(6).into()
    }

    fn tool<'a>(&self, icon: &str, label: &str, key: Option<&str>, msg: Option<Message>, active: bool) -> Element<'a, Message> {
        self.tip(w::icon_button(&self.palette, &self.icons, icon, msg, active), label, key)
    }

    /// Breadcrumb segments: Home or the drive's name first, never raw mount paths.
    pub(crate) fn crumbs(&self) -> Vec<(Option<&'static str>, String, PathBuf)> {
        let home = config::home();
        let (base, mut out) = if let Ok(rest) = self.location.strip_prefix(&home) {
            (rest.to_path_buf(), vec![(Some("home"), "Home".to_string(), home.clone())])
        } else if let Some((v, mp)) = self.volumes.iter().find_map(|v| v.mount_points.iter().find(|m| self.location.starts_with(m.as_str())).map(|m| (v, m.clone()))) {
            let rest = self.location.strip_prefix(&mp).unwrap_or(Path::new("")).to_path_buf();
            (rest, vec![(Some("drive"), drive_name(v, &self.volumes), PathBuf::from(mp))])
        } else {
            (self.location.strip_prefix("/").unwrap_or(&self.location).to_path_buf(), vec![(Some("drive"), "Computer".to_string(), PathBuf::from("/"))])
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
        center = center.push(if self.everywhere() { self.results_view() } else { self.list() });
        let body = row![self.sidebar(), w::vline(p.line), center].height(Length::Fill);
        let root = column![self.toolbar(), w::hline(p.line), body, w::hline(p.line), self.status_bar()];
        w::fill(root, p.bg).width(Length::Fill).height(Length::Fill).into()
    }

    fn list(&self) -> Element<'_, Message> {
        let Some(loaded) = &self.loaded else { return container(Space::new()).width(Length::Fill).height(Length::Fill).into() };
        let empty = (!self.skeleton && self.order.is_empty()).then(|| {
            if !self.query.is_empty() {
                (format!("No matches for “{}”", self.query), "Nothing in this folder matches. Esc clears the search · Ctrl+E searches everywhere.".to_string())
            } else if loaded.listing.is_empty() {
                ("This folder is empty".to_string(), "New files show up here as soon as they're created.".to_string())
            } else {
                ("Only hidden files here".to_string(), "Press Ctrl+H to show them.".to_string())
            }
        });
        FileList::new(
            file_list::Model {
                listing: &loaded.listing,
                order: &self.order,
                selected: &self.selected,
                cursor: self.cursor,
                generation: self.generation,
                metadata_ready: loaded.metadata_ready,
                sort: self.sort,
                skeleton: self.skeleton,
                empty,
                row_h: self.settings.appearance.density.row_height(),
            },
            &self.palette,
            &self.icons,
            &self.dates,
            Message::List,
        )
        .into()
    }

    fn toolbar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let crumbs = self.crumbs();
        let n = crumbs.len();
        let mut trail = row![].spacing(2).align_y(Alignment::Center);
        for (idx, (icon, label, target)) in crumbs.into_iter().enumerate() {
            if idx > 0 {
                trail = trail.push(self.glyph("chevron-right", 12.0, color(p.ink_faint)));
            }
            let last = idx == n - 1 && !self.everywhere();
            let ink = if last { color(p.ink_strong) } else { color(p.ink_muted) };
            let mut content = row![].spacing(style::SPACE_1 + 2.0).align_y(Alignment::Center);
            if let Some(icon) = icon {
                content = content.push(self.glyph(icon, 14.0, ink));
            }
            content = content.push(text(label).size(style::BODY).font(if last { style::FONT_BOLD } else { style::FONT }).color(ink).wrapping(text::Wrapping::None));
            let pal = p.clone();
            let hover_ink = color(p.ink_strong);
            trail = trail.push(
                button(container(content).height(24).align_y(Alignment::Center))
                    .padding([0, 6])
                    .on_press_maybe((!last).then_some(Message::Navigate(target)))
                    .style(move |_, status| button::Style {
                        background: match status {
                            button::Status::Hovered => Some(Background::Color(color(pal.state_hover))),
                            button::Status::Pressed => Some(Background::Color(color(pal.state_press))),
                            _ => None,
                        },
                        text_color: if status == button::Status::Hovered { hover_ink } else { ink },
                        border: Border { radius: 2.0.into(), ..Border::default() },
                        ..Default::default()
                    }),
            );
        }
        if self.everywhere() {
            trail = trail.push(self.glyph("chevron-right", 12.0, color(p.ink_faint)));
            trail = trail.push(container(text("Search results").size(style::BODY).font(style::FONT_BOLD).color(color(p.ink_strong)).wrapping(text::Wrapping::None)).padding([0, 6]));
        }
        let crumb_box = container(trail).width(Length::Fill).clip(true);

        let placeholder = match self.scope {
            Scope::Folder => "Search this folder",
            Scope::Everywhere => "Search everywhere",
        };
        let search = text_input(placeholder, &self.query)
            .id(SEARCH_ID)
            .on_input(Message::Search)
            .on_submit(Message::SearchSubmit)
            .size(style::BODY)
            .font(style::FONT)
            .padding([5, 10])
            .width(240)
            .style(w::field_style(p, false));
        let scope = w::segmented(
            p,
            &[("Folder", Message::SetScope(Scope::Folder)), ("Everywhere", Message::SetScope(Scope::Everywhere))],
            if self.scope == Scope::Folder { 0 } else { 1 },
        );

        let bar = row![
            self.tool("arrow-left", "Back", Some("Alt+←"), (!self.back.is_empty()).then_some(Message::Back), false),
            self.tool("arrow-right", "Forward", Some("Alt+→"), (!self.forward.is_empty()).then_some(Message::Forward), false),
            self.tool("arrow-up", "Parent folder", Some("Alt+↑"), self.location.parent().map(|_| Message::Up), false),
            self.tool("refresh", "Reload", Some("F5"), Some(Message::Reload), false),
            container(Space::new()).width(1).height(18).style({
                let c = color(p.line);
                move |_| container::Style { background: Some(Background::Color(c)), ..Default::default() }
            }),
            crumb_box,
            self.tip(scope.into(), "Where to search", Some("Ctrl+E")),
            search,
            self.tool(
                if self.show_hidden { "eye" } else { "eye-off" },
                if self.show_hidden { "Hide hidden files" } else { "Show hidden files" },
                Some("Ctrl+H"),
                Some(Message::ToggleHidden),
                self.show_hidden,
            ),
            self.tool("settings", "Settings", Some("Ctrl+,"), Some(Message::Settings(SettingsMsg::Open)), false),
        ]
        .spacing(style::SPACE_1)
        .align_y(Alignment::Center);
        container(bar).height(style::TOOLBAR).padding([0, 12]).align_y(Alignment::Center).width(Length::Fill).into()
    }

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
            ("trash", "Trash", home.join(".local/share/Trash/files")),
        ];
        let mut col = column![self.section("Linux", Some(color(p.world_linux)), None)].spacing(1);
        for (icon, label, path) in places {
            if path.is_dir() {
                let active = self.location == path && !self.everywhere();
                col = col.push(self.side_item(icon, label.to_string(), active, Message::Navigate(path)));
            }
        }
        col = col.push(Space::new().height(style::SPACE_5));
        col = col.push(self.section("Windows", Some(color(p.world_windows)), (!self.volumes.is_empty()).then_some(self.volumes.len())));
        if self.volumes.is_empty() {
            col = col.push(container(text("No Windows drives found").size(style::META).font(style::FONT).color(color(p.ink_muted))).padding([4, 12]));
        }
        for (i, v) in self.volumes.iter().enumerate() {
            col = col.push(self.drive_item(i, v));
        }
        w::fill(scrollable(col).height(Length::Fill), p.bg_sunken).width(style::SIDEBAR).height(Length::Fill).padding([12, 8]).into()
    }

    fn side_item<'a>(&'a self, icon: &str, label: String, active: bool, msg: Message) -> Element<'a, Message> {
        let p = &self.palette;
        let tint = if active { color(p.accent_ink) } else { color(p.ink_muted) };
        let ink = if active { color(p.ink_strong) } else { color(p.ink) };
        let r = row![
            self.glyph(icon, 16.0, tint),
            text(label).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(ink).width(Length::Fill).wrapping(text::Wrapping::None)
        ]
        .spacing(style::SPACE_4)
        .align_y(Alignment::Center);
        w::row_button(p, container(r).height(style::ROW).align_y(Alignment::Center).into(), active, Some(msg))
    }

    fn drive_item<'a>(&'a self, i: usize, v: &Volume) -> Element<'a, Message> {
        let p = &self.palette;
        let active = v.mount_points.iter().any(|m| self.location.starts_with(m.as_str()));
        let fs = self.volume_fs.get(i).cloned().flatten();
        let ink = if active { color(p.ink_strong) } else if v.is_mounted() { color(p.ink) } else { color(p.ink_muted) };
        let head = text(drive_name(v, &self.volumes)).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(ink).wrapping(text::Wrapping::None);
        let dup = self.volumes.iter().filter(|o| o.label == v.label).count() > 1;
        let mut meta = String::new();
        match &fs {
            Some(f) => {
                fmt::size(f.free, &mut meta);
                meta.push_str(" free of ");
                fmt::size(f.total, &mut meta);
            }
            // Unmounted: just the size (the name carries it when two drives share a label),
            // so the line fits beside the Not mounted pill.
            None if dup => meta.push_str("NTFS"),
            None => fmt::size(v.size, &mut meta),
        }
        let mut lines = column![head].spacing(3);
        if let Some(f) = &fs {
            lines = lines.push(w::usage_bar(p, f.used_fraction(), p.world_windows));
        }
        let mut meta_row = row![].spacing(style::SPACE_3).align_y(Alignment::Center);
        if !v.is_mounted() {
            meta_row = meta_row.push(w::pill(p, "Not mounted", None));
        }
        meta_row = meta_row.push(text(meta).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None));
        lines = lines.push(meta_row);
        let tint = if active { color(p.accent_ink) } else { color(p.ink_muted) };
        let r = row![container(self.glyph("drive", 16.0, tint)).padding([2, 0]), lines].spacing(style::SPACE_4).align_y(Alignment::Start);
        w::row_button(p, container(r).padding([6, 0]).into(), active, Some(Message::DriveClicked(i)))
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
        .padding([6, 8])
        .padding(iced::Padding { left: 16.0, ..iced::Padding::from([6, 8]) })
        .style(move |_| container::Style { background: Some(Background::Color(color(tone.soft))), ..Default::default() })
        .into()
    }

    // ------------------------------------------------------------------ search results

    fn results_view(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let Some(res) = &self.results else {
            let msg = if self.searching { "Searching…" } else { "Type to search" };
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
            let active = self.result_cursor == Some(i);
            let icon = if h.is_dir { "folder" } else { crate::kinds::icon(h.name.as_bytes(), if h.is_dir { ef_core::listing::Kind::Dir } else { ef_core::listing::Kind::File }) };
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
            if let Some(i) = self.result_cursor {
                r = r.push(w::text_button(p, &self.icons, "Show in folder", Some("folder"), None, Variant::Secondary, Some(Message::ResultReveal(i))));
            }
            container(r).padding([6, 16]).width(Length::Fill)
        };
        column![header, w::hline(p.line), scrollable(rows).height(Length::Fill), w::hline(p.line), footer].height(Length::Fill).into()
    }

    // ------------------------------------------------------------------ status bar

    fn status_bar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mut left = String::new();
        if self.everywhere() {
            match &self.results {
                Some(r) => {
                    fmt::count(r.total, &mut left);
                    left.push_str(if r.total == 1 { " match" } else { " matches" });
                    if r.total > r.hits.len() {
                        left.push_str(" · showing first ");
                        fmt::count(r.hits.len(), &mut left);
                    }
                    left.push_str(if r.from_index { "  ·  from the index" } else { "  ·  live search (index off)" });
                }
                None if self.searching => left.push_str("Searching…"),
                None => {}
            }
        } else {
            let total = self.loaded.as_ref().map_or(0, |l| l.listing.len() - if self.show_hidden { 0 } else { l.hidden });
            if !self.query.is_empty() {
                fmt::count(self.order.len(), &mut left);
                left.push_str(" of ");
                fmt::count(total, &mut left);
                left.push_str(" match");
            } else {
                fmt::count(self.order.len(), &mut left);
                left.push_str(if self.order.len() == 1 { " item" } else { " items" });
            }
            let (files, dirs, bytes) = self.selection_stats();
            if files + dirs > 0 {
                left.push_str("  ·  ");
                fmt::count(files + dirs, &mut left);
                left.push_str(" selected");
                if files > 0 {
                    left.push_str(" · ");
                    fmt::size(bytes, &mut left);
                    if dirs > 0 {
                        left.push_str(" + ");
                        fmt::count(dirs, &mut left);
                        left.push_str(if dirs == 1 { " folder" } else { " folders" });
                    }
                }
            }
            if let Some(l) = &self.loaded {
                if !self.show_hidden && l.hidden > 0 && self.query.is_empty() {
                    left.push_str("  ·  ");
                    fmt::count(l.hidden, &mut left);
                    left.push_str(" hidden");
                }
            }
        }
        let mut right = String::new();
        if self.loaded.as_ref().is_some_and(|l| !l.metadata_ready) {
            right.push_str("Reading details…  ·  ");
        }
        if std::env::var_os("ECHOFILES_TIMING").is_some() {
            if let Some(l) = self.loaded.as_ref().filter(|l| l.metadata_ready) {
                right.push_str(&format!("names {:.1} · sort {:.1} · details {:.1} · paint {:.1} ms  ·  ", l.names_ms, l.sort_ms, l.meta_ms, self.first_paint_ms));
            }
        }
        match self.index_state {
            IndexState::Building => right.push_str("Indexing…  ·  "),
            IndexState::Off => {}
            _ => {}
        }
        if let Some(f) = &self.fs {
            right.push_str(f.fs_type);
            right.push_str(" · ");
            fmt::size(f.free, &mut right);
            right.push_str(" free");
        }
        let bar = row![
            text(left).size(style::META).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None),
            Space::new().width(Length::Fill),
            text(right).size(style::META).font(style::FONT).color(color(p.ink_muted)).wrapping(text::Wrapping::None),
        ]
        .align_y(Alignment::Center);
        w::fill(bar, p.bg_deep).height(style::STATUSBAR).padding([0, 16]).align_y(Alignment::Center).width(Length::Fill).into()
    }
}

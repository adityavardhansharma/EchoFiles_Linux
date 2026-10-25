//! The keyboard beyond the basics: the Yazi-style 10xer mode (single letters drive the
//! list: `j`/`k`, `h`/`l`, `gg`/`G`, Space marks, `y`/`x`/`p`…), the F1 sheet of every
//! shortcut, and the footer that shows the keys that matter right now and what's on the
//! clipboard.

use std::time::{Duration, Instant};

use ef_config::KeyMode;
use iced::widget::{button, column, container, mouse_area, row, scrollable, stack, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Task, Vector};

use crate::actions::FileMsg;
use crate::app::{App, Message};
use crate::style::{self, color};
use crate::widgets::{self as w};

/// A `g…` or `c…` chord waits this long for its second key.
const CHORD: Duration = Duration::from_millis(1000);
/// Ctrl+D / Ctrl+U move this many items.
const HALF_PAGE: isize = 12;

/// Every shortcut, grouped for the F1 sheet: (group, [(keys, what they do)]).
pub const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    ("Move around", &[
        ("Enter", "open"),
        ("Backspace  Alt+↑", "parent folder"),
        ("Alt+←  Alt+→", "back · forward"),
        ("→  ←", "column view: into the folder · back up"),
        ("Ctrl+L", "type a path"),
        ("Ctrl+K", "command palette"),
        ("letters", "jump to a name"),
    ]),
    ("Select", &[
        ("↑ ↓  Home End", "move"),
        ("Shift+↑ ↓", "extend the selection"),
        ("Ctrl+click", "add or remove one"),
        ("Ctrl+A", "select all"),
        ("Esc", "clear the selection, filter or search"),
    ]),
    ("Files", &[
        ("Ctrl+C  Ctrl+X  Ctrl+V", "copy · cut · paste"),
        ("F2", "rename"),
        ("Del", "move to the Trash"),
        ("Shift+Del", "delete permanently"),
        ("Ctrl+Shift+N", "new folder"),
        ("Ctrl+Z", "undo"),
        ("Ctrl+Shift+Z  Ctrl+Y", "redo"),
        ("Ctrl+Shift+C", "copy path"),
        ("Alt+Enter", "properties"),
        ("F5  F6", "dual pane: copy · move to the other side"),
    ]),
    ("Find", &[
        ("Ctrl+F", "filter this folder"),
        ("Ctrl+Shift+F", "filter with subfolders"),
        ("/", "search box"),
        ("Ctrl+E", "search this folder ↔ everywhere"),
    ]),
    ("View", &[
        ("Ctrl+1  Ctrl+2  Ctrl+3", "list · grid · columns"),
        ("Ctrl+=  Ctrl+-  Ctrl+0", "bigger · smaller · default icons"),
        ("Ctrl+wheel", "icon size"),
        ("Ctrl+H", "hidden files"),
        ("Space", "preview pane"),
        ("Ctrl+B", "sidebar"),
        ("F3  Tab", "dual pane · switch side"),
    ]),
    ("Tabs and app", &[
        ("Ctrl+T  Ctrl+W", "new tab · close tab"),
        ("Ctrl+Tab", "next tab"),
        ("Ctrl+Shift+Y", "10xer keys on or off"),
        ("F1", "this sheet"),
        ("Ctrl+,", "settings"),
        ("Ctrl+Q", "quit"),
    ]),
];

/// The 10xer keys (Yazi's, where EchoFiles has the same thing).
pub const TENXER: &[(&str, &str)] = &[
    ("j  k", "down · up"),
    ("h  l", "parent · open"),
    ("gg  G", "first · last"),
    ("Ctrl+D  Ctrl+U", "half a page down · up"),
    ("Space", "mark and move on"),
    ("v", "select from here (Esc ends)"),
    ("y  x  p", "copy · cut · paste"),
    ("d  D", "Trash · delete permanently"),
    ("r", "rename"),
    ("a  A", "new file · new folder"),
    ("u  Ctrl+R", "undo · redo"),
    ("f  /", "filter · search"),
    (".", "hidden files"),
    ("H  L", "back · forward"),
    ("t  [  ]", "new tab · previous · next tab"),
    ("gh  gd  gc", "home · Downloads · ~/.config"),
    ("cc", "copy path"),
    ("o", "open"),
];

impl App {
    pub(crate) fn tenxer(&self) -> bool {
        self.settings.general.keys == KeyMode::TenXer
    }

    /// Move the cursor: the selection follows, unless items were marked (Space) or `v` is
    /// extending from an anchor.
    fn tx_move(&mut self, to: impl FnOnce(usize, usize) -> usize) {
        if self.everywhere() {
            let n = self.pane().results.as_ref().map_or(0, |r| r.hits.len());
            let pane = self.pane_mut();
            if n > 0 {
                pane.result_cursor = Some(to(pane.result_cursor.unwrap_or(0), n).min(n - 1));
            }
            return;
        }
        let pane = self.pane_mut();
        let n = pane.order.len();
        if n == 0 {
            return;
        }
        let pos = if pane.cursor.is_none() { 0 } else { to(pane.cursor.unwrap_or(0), n).min(n - 1) };
        if let Some(anchor) = pane.visual {
            pane.clear_selection();
            for p in anchor.min(pos)..=anchor.max(pos) {
                if let Some(i) = pane.entry_at(p) {
                    pane.set_bit(i, true);
                }
            }
            pane.cursor = Some(pos);
        } else if pane.marking {
            pane.cursor = Some(pos);
        } else {
            pane.select_only(pos);
        }
    }

    /// A key in 10xer mode. `None`: not one of its keys, so the usual handling applies.
    pub(crate) fn tenxer_key(&mut self, ch: &str, ctrl: bool) -> Option<Task<Message>> {
        if self.pane().special() {
            return None;
        }
        let f = |m: FileMsg| Message::File(m);
        if ctrl {
            return match ch {
                "d" => {
                    self.tx_move(|c, _| c + HALF_PAGE as usize);
                    Some(Task::none())
                }
                "u" => {
                    self.tx_move(|c, _| c.saturating_sub(HALF_PAGE as usize));
                    Some(Task::none())
                }
                "r" => Some(self.update(f(FileMsg::Redo))),
                _ => None,
            };
        }
        let chord = self.chord.take().filter(|(_, at)| at.elapsed() < CHORD).map(|(c, _)| c);
        let go = |p: std::path::PathBuf| Message::Navigate(p);
        let msg = match (chord, ch) {
            (Some('g'), "g") => {
                self.tx_move(|_, _| 0);
                return Some(Task::none());
            }
            (Some('g'), "h" | "~") => go(ef_config::home()),
            (Some('g'), "d") => go(ef_config::home().join("Downloads")),
            (Some('g'), "c") => go(ef_config::home().join(".config")),
            (Some('c'), "c") => f(FileMsg::CopyPath),
            (Some(_), _) => return Some(Task::none()),
            (None, "g" | "c") => {
                self.chord = Some((ch.chars().next().unwrap_or('g'), Instant::now()));
                return Some(Task::none());
            }
            (None, "j") => {
                self.tx_move(|c, _| c + 1);
                return Some(Task::none());
            }
            (None, "k") => {
                self.tx_move(|c, _| c.saturating_sub(1));
                return Some(Task::none());
            }
            (None, "G") => {
                self.tx_move(|_, n| n - 1);
                return Some(Task::none());
            }
            (None, " ") => {
                let pane = self.pane_mut();
                if let Some(i) = pane.cursor.and_then(|c| pane.entry_at(c)) {
                    let on = !crate::file_list::is_selected(&pane.selected, i);
                    pane.set_bit(i, on);
                }
                pane.marking = true;
                self.tx_move(|c, _| c + 1);
                return Some(Task::none());
            }
            (None, "v") => {
                let pane = self.pane_mut();
                if pane.visual.take().is_none() {
                    let at = pane.cursor.unwrap_or(0);
                    pane.visual = Some(at);
                    pane.marking = true;
                    self.tx_move(|c, _| c);
                }
                return Some(Task::none());
            }
            (None, "h") => Message::Up,
            (None, "l" | "o") => {
                if self.everywhere() {
                    return self.pane().result_cursor.map(|i| self.update(Message::ResultOpen(i)));
                }
                f(FileMsg::Open)
            }
            (None, "H") => Message::Back,
            (None, "L") => Message::Forward,
            (None, "y") => f(FileMsg::Copy),
            (None, "x") => f(FileMsg::Cut),
            (None, "p") => f(FileMsg::Paste),
            (None, "d") => f(FileMsg::Trash),
            (None, "D") => f(FileMsg::AskDelete),
            (None, "r") => f(FileMsg::StartRename),
            (None, "a") => f(FileMsg::NewFile),
            (None, "A") => f(FileMsg::NewFolder),
            (None, "u") => f(FileMsg::Undo),
            (None, "f") => Message::OpenFilter(false),
            (None, "/") => Message::FocusSearch,
            (None, ".") => Message::ToggleHidden,
            (None, "t") => Message::NewTab,
            (None, "[") => Message::CycleTab(true),
            (None, "]") => Message::CycleTab(false),
            // Other letters do nothing here (no jump-to-name: they're commands).
            (None, c) if c.chars().all(|c| c.is_alphanumeric()) => return Some(Task::none()),
            _ => return None,
        };
        Some(self.update(msg))
    }

    /// F1: every shortcut on one sheet.
    pub(crate) fn help_layer(&self) -> Option<Element<'_, Message>> {
        if !self.help {
            return None;
        }
        let p = &self.palette;
        let group = |title: &str, keys: &[(&str, &str)]| -> Element<'_, Message> {
            let mut col = column![text(title.to_uppercase()).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted))].spacing(5);
            for (k, what) in keys {
                col = col.push(
                    row![
                        container(text(k.to_string()).size(style::META).font(style::FONT_BOLD).color(color(p.accent_ink)).wrapping(text::Wrapping::None)).width(178),
                        text(what.to_string()).size(style::META).font(style::FONT).color(color(p.ink)),
                    ]
                    .spacing(style::SPACE_3),
                );
            }
            col.width(Length::Fill).into()
        };
        let mut groups: Vec<Element<'_, Message>> = SHORTCUTS.iter().map(|(t, k)| group(t, k)).collect();
        groups.push(group(if self.tenxer() { "10xer keys (on)" } else { "10xer keys (Ctrl+Shift+Y turns them on)" }, TENXER));
        // Two columns, filled in order.
        let mut left = column![].spacing(style::SPACE_5).width(Length::Fill);
        let mut right = column![].spacing(style::SPACE_5).width(Length::Fill);
        let half = groups.len().div_ceil(2);
        for (i, g) in groups.into_iter().enumerate() {
            if i < half { left = left.push(g) } else { right = right.push(g) }
        }
        let head = row![
            w::glyph(&self.icons, "keyboard", 18.0, color(p.accent_ink)),
            text("Keyboard shortcuts").size(16).font(style::FONT_BOLD).color(color(p.ink_strong)).width(Length::Fill),
            w::kbd_chip(p, "F1"),
            w::icon_button(p, &self.icons, "close", Some(Message::ToggleHelp), false),
        ]
        .spacing(style::SPACE_4)
        .align_y(Alignment::Center);
        let pal = p.clone();
        let panel = container(column![head, scrollable(row![left, right].spacing(style::SPACE_5 * 2.0).padding(Padding { right: 16.0, ..Padding::ZERO })).height(Length::Shrink)].spacing(style::SPACE_5))
            .width(820)
            .max_height(self.window_size.height - 80.0)
            .padding(20)
            .style(move |_| container::Style {
                background: Some(Background::Color(color(pal.bg_raised))),
                border: Border { color: color(pal.accent), width: 2.0, radius: 0.0.into() },
                shadow: Shadow { color: Color { a: 0.45, ..Color::BLACK }, offset: Vector::new(0.0, 12.0), blur_radius: 32.0 },
                ..Default::default()
            });
        let scrim = color(p.scrim);
        let back = mouse_area(container(Space::new()).width(Length::Fill).height(Length::Fill).style(move |_| container::Style { background: Some(Background::Color(scrim)), ..Default::default() })).on_press(Message::ToggleHelp);
        Some(stack![back, container(iced::widget::opaque(panel)).center(Length::Fill)].into())
    }

    /// Keys that matter right now, for the footer.
    fn hints(&self) -> Vec<(&'static str, &'static str)> {
        let pane = self.pane();
        if self.rename.is_some() {
            return vec![("Enter", "rename"), ("Esc", "cancel")];
        }
        if pane.filter_bar {
            return vec![("Enter", "open"), ("Ctrl+Shift+F", if pane.deep { "this folder only" } else { "include subfolders" }), ("Esc", "close filter")];
        }
        if self.everywhere() {
            return vec![("Enter", "open"), ("Alt+Enter", "show in folder"), ("Esc", "clear")];
        }
        if pane.special() {
            return vec![("Esc", "back to files"), ("F1", "all keys")];
        }
        if self.tenxer() {
            if pane.visual.is_some() {
                return vec![("j k", "extend"), ("y x", "copy · cut"), ("d", "Trash"), ("Esc", "end")];
            }
            return vec![("j k", "move"), ("h l", "up · open"), ("Space", "mark"), ("y x p", "copy · cut · paste"), ("d", "Trash"), ("f", "filter"), ("F1", "all keys")];
        }
        let mut v = vec![("Enter", "open"), ("F2", "rename"), ("Del", "Trash"), ("Ctrl+C X V", "copy · cut · paste"), ("Ctrl+F", "filter")];
        if pane.layout == crate::pane::Layout::Columns {
            v.insert(1, ("→ ←", "in · out"));
        }
        v.push(("F1", "all keys"));
        v
    }

    /// Shortcut hints on the left, the clipboard on the right.
    pub(crate) fn footer(&self) -> Option<Element<'_, Message>> {
        if !self.settings.appearance.footer {
            return None;
        }
        let p = &self.palette;
        let mut left = row![].spacing(style::SPACE_4).align_y(Alignment::Center);
        if self.tenxer() {
            let mode = if self.pane().visual.is_some() { "VISUAL" } else { "10XER" };
            let pal = p.clone();
            left = left.push(container(text(mode).size(style::LABEL).font(style::FONT_BOLD).color(color(p.on_accent))).padding([1, 6]).style(move |_| container::Style { background: Some(Background::Color(color(pal.accent))), border: Border { radius: 2.0.into(), ..Border::default() }, ..Default::default() }));
        }
        for (k, what) in self.hints() {
            left = left.push(row![text(k).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink)), text(what).size(style::LABEL).font(style::FONT).color(color(p.ink_muted))].spacing(5).align_y(Alignment::Center));
        }
        let mut bar = row![container(left).width(Length::Fill).clip(true)].spacing(style::SPACE_4).align_y(Alignment::Center);
        match &self.clip {
            Some(c) => {
                let first = c.paths.first().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let what = match c.paths.len() {
                    1 => format!("“{first}”"),
                    n => format!("“{first}” and {} more", n - 1),
                };
                bar = bar.push(w::glyph(&self.icons, if c.cut { "cut" } else { "copy" }, 13.0, color(p.accent_ink)));
                bar = bar.push(text(format!("{} {what}", if c.cut { "Cut" } else { "Copied" })).size(style::LABEL).font(style::FONT).color(color(p.ink)).wrapping(text::Wrapping::None));
                let pal = p.clone();
                bar = bar.push(
                    button(text("Clear").size(style::LABEL).font(style::FONT).color(color(p.ink_muted)))
                        .padding([0, 4])
                        .on_press(Message::ClearClip)
                        .style(move |_, s| button::Style { background: matches!(s, button::Status::Hovered).then(|| Background::Color(color(pal.state_hover))), text_color: color(pal.ink), border: Border { radius: 2.0.into(), ..Border::default() }, ..Default::default() }),
                );
            }
            None => bar = bar.push(text("Clipboard empty").size(style::LABEL).font(style::FONT).color(color(p.ink_faint))),
        }
        Some(w::fill(bar, p.bg_sunken).height(22).padding(Padding { left: 16.0, right: 12.0, ..Padding::ZERO }).align_y(Alignment::Center).width(Length::Fill).into())
    }
}

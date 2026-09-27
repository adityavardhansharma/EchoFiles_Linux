//! Settings (design system `SettingsShell`, `SettingsNav`, `SettingsGroup`, `SettingRow`,
//! `PathListEditor`, `NameChips`, `IndexStatus`). A full screen of its own: a top bar back to
//! Files, a page list on the left, and one page of grouped rows on the right, each row with
//! what it does on the left and its control on the right. Every change saves immediately to
//! `~/.config/echofiles/settings.toml`, which `ef` reads too.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use ef_config::{self as config, Density, OpenTo, Scope, SearchConfig};
use iced::widget::{column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Background, Border, Element, Length, Task};

use crate::app::{App, Message, Mode};
use crate::indexer::{self, IndexState};
use crate::style::{self, color};
use crate::system;
use crate::widgets::{self as w, Variant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    General,
    Search,
    Agents,
    Appearance,
    About,
}

const PAGES: [(Page, &str, &str); 5] = [
    (Page::General, "sliders", "General"),
    (Page::Search, "search", "Search & index"),
    (Page::Agents, "terminal", "AI agents"),
    (Page::Appearance, "image", "Appearance"),
    (Page::About, "info", "About"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Roots,
    Exclude,
    Names,
}

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    Open,
    Close,
    Page(Page),
    Background(bool),
    StartAtLogin(bool),
    OpenTo(OpenTo),
    ShowHidden(bool),
    Index(bool),
    Rebuild,
    DefaultScope(Scope),
    Draft(Field, String),
    Add(Field),
    AddCurrent(Field),
    Remove(Field, usize),
    SkipCache(bool),
    ResetExclusions,
    Cli(bool),
    Skill(bool),
    Density(Density),
    Reveal(PathBuf),
    Saved(Result<(), String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveState {
    Saved,
    Saving,
    Failed(String),
}

pub struct SettingsUi {
    pub page: Page,
    drafts: [String; 3],
    /// Validation message under one field.
    error: Option<(Field, String)>,
    save: SaveState,
    /// A problem applying a setting to the system (autostart file, skill link).
    problem: Option<String>,
    skill_installed: bool,
    starts_at_login: bool,
}

impl SettingsUi {
    pub fn new(load_error: Option<String>) -> Self {
        SettingsUi {
            page: Page::General,
            drafts: Default::default(),
            error: None,
            save: load_error.map_or(SaveState::Saved, SaveState::Failed),
            problem: None,
            skill_installed: system::skill_installed(),
            starts_at_login: system::starts_at_login(),
        }
    }
}

fn slot(f: Field) -> usize {
    match f {
        Field::Roots => 0,
        Field::Exclude => 1,
        Field::Names => 2,
    }
}

fn mb(bytes: u64) -> String {
    let mut s = String::new();
    ef_core::fmt::size(bytes, &mut s);
    s
}

/// Check a new list entry; `Err` is the message shown under the field.
fn validate(cfg: &SearchConfig, field: Field, raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(if field == Field::Names { "Type a folder name first." } else { "Type a path first." }.into());
    }
    match field {
        Field::Names => {
            if raw.contains('/') {
                return Err(format!("“{raw}” is a path. Names can't contain “/” — add paths under Never show in search."));
            }
            if cfg.exclude_names.iter().any(|n| n == raw) {
                return Err(format!("“{raw}” is already skipped."));
            }
            Ok(raw.to_string())
        }
        Field::Roots | Field::Exclude => {
            let path = config::expand(raw);
            if !path.is_absolute() {
                return Err(format!("“{raw}” isn't a full path. Start it with “/” or “~/”."));
            }
            if !path.is_dir() {
                return Err(format!("“{raw}” isn't a folder on this computer. Check the spelling, or open the folder and use Add current folder."));
            }
            let list = if field == Field::Roots { &cfg.roots } else { &cfg.exclude_paths };
            if list.iter().any(|p| config::expand(p) == path) {
                return Err(format!("{} is already in the list.", config::tilde(&path)));
            }
            Ok(config::tilde(&path))
        }
    }
}

impl App {
    fn save_settings(&mut self) -> Task<Message> {
        self.settings_ui.save = SaveState::Saving;
        let s = self.settings.clone();
        // Saves run on their own threads and share one tmp file: take turns, and never let an
        // older snapshot land after a newer one.
        static NEXT: AtomicU64 = AtomicU64::new(1);
        static WRITTEN: Mutex<u64> = Mutex::new(0);
        let generation = NEXT.fetch_add(1, Ordering::Relaxed);
        Task::perform(
            async move {
                let (tx, rx) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let mut written = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
                    if *written > generation {
                        let _ = tx.send(Ok(()));
                        return;
                    }
                    let r = s.save().map_err(|e| format!("Couldn't save settings to {}: {e}", config::tilde(&config::Settings::path())));
                    if r.is_ok() {
                        *written = generation;
                    }
                    let _ = tx.send(r);
                });
                rx.await.unwrap_or(Ok(()))
            },
            |r| Message::Settings(SettingsMsg::Saved(r)),
        )
    }

    fn list_mut(&mut self, f: Field) -> &mut Vec<String> {
        let s = &mut self.settings.search;
        match f {
            Field::Roots => &mut s.roots,
            Field::Exclude => &mut s.exclude_paths,
            Field::Names => &mut s.exclude_names,
        }
    }

    fn add_entry(&mut self, f: Field, raw: &str) -> Task<Message> {
        match validate(&self.settings.search, f, raw) {
            Ok(v) => {
                self.list_mut(f).push(v);
                self.settings_ui.drafts[slot(f)].clear();
                self.settings_ui.error = None;
                self.index_settings_changed();
                self.save_settings()
            }
            Err(e) => {
                self.settings_ui.error = Some((f, e));
                Task::none()
            }
        }
    }

    pub(crate) fn settings_update(&mut self, msg: SettingsMsg) -> Task<Message> {
        match msg {
            SettingsMsg::Open => {
                self.mode = Mode::Settings;
                self.settings_ui.skill_installed = system::skill_installed();
                self.settings_ui.starts_at_login = system::starts_at_login();
                Task::none()
            }
            SettingsMsg::Close => {
                self.mode = Mode::Files;
                Task::none()
            }
            SettingsMsg::Page(p) => {
                self.settings_ui.page = p;
                self.settings_ui.error = None;
                Task::none()
            }
            SettingsMsg::Background(on) => {
                self.settings.general.background = on;
                if !on && self.settings.general.start_at_login {
                    // Starting hidden at login only makes sense if it keeps running.
                    self.settings.general.start_at_login = false;
                    self.apply_start_at_login(false);
                }
                self.save_settings()
            }
            SettingsMsg::StartAtLogin(on) => {
                self.settings.general.start_at_login = on;
                self.apply_start_at_login(on);
                self.save_settings()
            }
            SettingsMsg::OpenTo(o) => {
                self.settings.general.open_to = o;
                self.save_settings()
            }
            SettingsMsg::ShowHidden(on) => {
                self.settings.general.show_hidden = on;
                let t = if self.show_hidden != on { self.update(Message::ToggleHidden) } else { Task::none() };
                Task::batch([t, self.save_settings()])
            }
            SettingsMsg::Index(on) => {
                let t = self.set_index_enabled(on);
                Task::batch([t, self.save_settings()])
            }
            SettingsMsg::Rebuild => self.build_index(),
            SettingsMsg::DefaultScope(s) => {
                self.settings.search.default_scope = s;
                self.scope = s;
                self.save_settings()
            }
            SettingsMsg::Draft(f, v) => {
                self.settings_ui.drafts[slot(f)] = v;
                if self.settings_ui.error.as_ref().is_some_and(|(ef, _)| *ef == f) {
                    self.settings_ui.error = None;
                }
                Task::none()
            }
            SettingsMsg::Add(f) => {
                let raw = self.settings_ui.drafts[slot(f)].clone();
                self.add_entry(f, &raw)
            }
            SettingsMsg::AddCurrent(f) => {
                let raw = self.location.to_string_lossy().into_owned();
                self.add_entry(f, &raw)
            }
            SettingsMsg::Remove(f, i) => {
                if f == Field::Roots && self.settings.search.roots.len() == 1 {
                    self.settings_ui.error = Some((f, "Keep at least one folder — or turn the search index off instead.".into()));
                    return Task::none();
                }
                let list = self.list_mut(f);
                if i < list.len() {
                    list.remove(i);
                }
                self.settings_ui.error = None;
                self.index_settings_changed();
                self.save_settings()
            }
            SettingsMsg::SkipCache(on) => {
                self.settings.search.skip_cache_folders = on;
                self.index_settings_changed();
                self.save_settings()
            }
            SettingsMsg::ResetExclusions => {
                let d = SearchConfig::default();
                self.settings.search.exclude_names = d.exclude_names;
                self.settings.search.skip_cache_folders = d.skip_cache_folders;
                self.settings_ui.error = None;
                self.index_settings_changed();
                self.save_settings()
            }
            SettingsMsg::Cli(on) => {
                self.settings.agents.cli = on;
                self.save_settings()
            }
            SettingsMsg::Skill(on) => {
                self.settings.agents.skill = on;
                self.settings_ui.problem = system::set_skill(on).err();
                self.settings_ui.skill_installed = system::skill_installed();
                self.settings.agents.skill = self.settings_ui.skill_installed;
                self.save_settings()
            }
            SettingsMsg::Density(d) => {
                self.settings.appearance.density = d;
                self.save_settings()
            }
            SettingsMsg::Reveal(p) => {
                let target = if p.is_dir() { p } else { p.parent().map(PathBuf::from).unwrap_or(p) };
                self.mode = Mode::Files;
                self.update(Message::Navigate(target))
            }
            SettingsMsg::Saved(r) => {
                self.settings_ui.save = match r {
                    Ok(()) => SaveState::Saved,
                    Err(e) => SaveState::Failed(e),
                };
                Task::none()
            }
        }
    }

    fn apply_start_at_login(&mut self, on: bool) {
        self.settings_ui.problem = system::set_start_at_login(on).err();
        self.settings_ui.starts_at_login = system::starts_at_login();
        self.settings.general.start_at_login = self.settings_ui.starts_at_login;
    }

    // ------------------------------------------------------------------ view

    pub(crate) fn settings_view(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let page = match self.settings_ui.page {
            Page::General => self.page_general(),
            Page::Search => self.page_search(),
            Page::Agents => self.page_agents(),
            Page::Appearance => self.page_appearance(),
            Page::About => self.page_about(),
        };
        let content = scrollable(container(container(page).max_width(720)).width(Length::Fill).padding([28, 40]).align_x(Alignment::Center)).height(Length::Fill);
        let body = row![self.settings_nav(), w::vline(p.line), content].height(Length::Fill);
        w::fill(column![self.settings_bar(), w::hline(p.line), body], p.bg).width(Length::Fill).height(Length::Fill).into()
    }

    fn settings_bar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let status: Element<'_, Message> = match &self.settings_ui.save {
            SaveState::Saved => row![w::glyph(&self.icons, "check", 14.0, color(p.success.ink)), text("Saved").size(style::META).font(style::FONT).color(color(p.ink_muted))]
                .spacing(style::SPACE_1 + 2.0)
                .align_y(Alignment::Center)
                .into(),
            SaveState::Saving => text("Saving…").size(style::META).font(style::FONT).color(color(p.ink_muted)).into(),
            SaveState::Failed(e) => row![w::glyph(&self.icons, "error", 14.0, color(p.danger.ink)), text(e.clone()).size(style::META).font(style::FONT).color(color(p.danger.ink))]
                .spacing(style::SPACE_1 + 2.0)
                .align_y(Alignment::Center)
                .into(),
        };
        let bar = row![
            w::text_button(p, &self.icons, "Files", Some("arrow-left"), Some("Esc"), Variant::Ghost, Some(Message::Settings(SettingsMsg::Close))),
            container(Space::new()).width(1).height(18).style({
                let c = color(p.line);
                move |_| container::Style { background: Some(Background::Color(c)), ..Default::default() }
            }),
            text("Settings").size(style::BODY).font(style::FONT_BOLD).color(color(p.ink_strong)),
            Space::new().width(Length::Fill),
            status,
        ]
        .spacing(style::SPACE_4)
        .align_y(Alignment::Center);
        container(bar).height(style::TOOLBAR).padding([0, 12]).align_y(Alignment::Center).width(Length::Fill).into()
    }

    fn settings_nav(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mut col = column![].spacing(1);
        for (page, icon, label) in PAGES {
            let active = self.settings_ui.page == page;
            let r = row![
                w::glyph(&self.icons, icon, 16.0, color(if active { p.accent_ink } else { p.ink_muted })),
                text(label).size(style::BODY).font(if active { style::FONT_BOLD } else { style::FONT }).color(color(if active { p.ink_strong } else { p.ink })).wrapping(text::Wrapping::None),
            ]
            .spacing(style::SPACE_4)
            .align_y(Alignment::Center);
            col = col.push(w::row_button(p, container(r).height(32).align_y(Alignment::Center).into(), active, Some(Message::Settings(SettingsMsg::Page(page)))));
        }
        w::fill(col, p.bg_sunken).width(style::SIDEBAR).height(Length::Fill).padding([12, 8]).into()
    }

    // ------------------------------------------------------------------ building blocks

    fn page_head<'a>(&self, title: &str, desc: &str) -> Element<'a, Message> {
        let p = &self.palette;
        column![
            text(title.to_string()).size(22).font(style::FONT_BOLD).color(color(p.ink_strong)),
            text(desc.to_string()).size(style::META).font(style::FONT).color(color(p.ink_muted)),
        ]
        .spacing(style::SPACE_1 + 2.0)
        .into()
    }

    /// A titled, bordered box of rows separated by hairlines.
    fn group<'a>(&self, title: &str, rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
        let p = &self.palette;
        let mut inner = column![];
        let n = rows.len();
        for (i, r) in rows.into_iter().enumerate() {
            inner = inner.push(r);
            if i + 1 < n {
                inner = inner.push(w::hline(p.line));
            }
        }
        let (bg, edge) = (color(p.bg_raised), color(p.line));
        let boxed = container(inner).width(Length::Fill).style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border { color: edge, width: 1.0, radius: 4.0.into() },
            ..Default::default()
        });
        column![text(title.to_uppercase()).size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted)), boxed].spacing(style::SPACE_3).into()
    }

    /// Label + description on the left, control on the right.
    fn setting<'a>(&self, label: &str, desc: &str, control: Element<'a, Message>) -> Element<'a, Message> {
        self.setting_with(label, desc, false, control)
    }

    fn setting_with<'a>(&self, label: &str, desc: &str, disabled: bool, control: Element<'a, Message>) -> Element<'a, Message> {
        let p = &self.palette;
        let a = if disabled { w::DISABLED } else { 1.0 };
        let ink = iced::Color { a, ..color(p.ink_strong) };
        let muted = iced::Color { a, ..color(p.ink_muted) };
        let mut left = column![text(label.to_string()).size(style::BODY).font(style::FONT).color(ink)].spacing(3).width(Length::Fill);
        if !desc.is_empty() {
            left = left.push(text(desc.to_string()).size(style::META).font(style::FONT).color(muted));
        }
        container(row![left, control].spacing(style::SPACE_5 + 8.0).align_y(Alignment::Center)).padding([12, 16]).width(Length::Fill).into()
    }

    /// A row that's just content (lists, notes) with the group's padding.
    fn block<'a>(&self, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        container(content).padding([12, 16]).width(Length::Fill).into()
    }

    fn toggle<'a>(&self, on: bool, f: impl Fn(bool) -> SettingsMsg + 'a) -> Element<'a, Message> {
        w::switch(&self.palette, on, Some(Box::new(move |b| Message::Settings(f(b)))))
    }

    fn note<'a>(&self, tone: ef_theme::Semantic, icon: &str, msg: String, action: Option<(&str, SettingsMsg)>) -> Element<'a, Message> {
        let p = &self.palette;
        let mut r = row![w::glyph(&self.icons, icon, 16.0, color(tone.ink)), text(msg).size(style::META).font(style::FONT).color(color(p.ink)).width(Length::Fill)]
            .spacing(style::SPACE_4)
            .align_y(Alignment::Center);
        if let Some((label, m)) = action {
            r = r.push(w::text_button(p, &self.icons, label, None, None, Variant::Secondary, Some(Message::Settings(m))));
        }
        let (bg, edge) = (color(tone.soft), color(tone.base));
        container(r)
            .padding([10, 14])
            .width(Length::Fill)
            .style(move |_| container::Style { background: Some(Background::Color(bg)), border: Border { color: iced::Color { a: 0.5, ..edge }, width: 1.0, radius: 4.0.into() }, ..Default::default() })
            .into()
    }

    fn field_error<'a>(&self, f: Field) -> Option<Element<'a, Message>> {
        let p = &self.palette;
        let (ef, e) = self.settings_ui.error.as_ref()?;
        (*ef == f).then(|| {
            row![w::glyph(&self.icons, "error", 14.0, color(p.danger.ink)), text(e.clone()).size(style::META).font(style::FONT).color(color(p.danger.ink))]
                .spacing(style::SPACE_1 + 2.0)
                .align_y(Alignment::Center)
                .into()
        })
    }

    /// Design system `PathListEditor`: one row per folder with a remove button, then an
    /// input with Add and Add current folder.
    fn path_list<'a>(&self, f: Field, items: &[String], empty: &str, placeholder: &str) -> Vec<Element<'a, Message>> {
        let p = &self.palette;
        let mut rows: Vec<Element<'a, Message>> = Vec::new();
        if items.is_empty() {
            rows.push(self.block(text(empty.to_string()).size(style::META).font(style::FONT).color(color(p.ink_muted))));
        }
        for (i, item) in items.iter().enumerate() {
            let exists = config::expand(item).is_dir();
            let mut r = row![
                w::glyph(&self.icons, "folder", 16.0, color(p.ink_muted)),
                text(item.clone()).size(style::BODY).font(style::FONT).color(color(p.ink)).width(Length::Fill).wrapping(text::Wrapping::None),
            ]
            .spacing(style::SPACE_4)
            .align_y(Alignment::Center);
            if !exists {
                r = r.push(w::pill(p, "Missing", Some(p.warning)));
            }
            r = r.push(w::icon_button(p, &self.icons, "close", Some(Message::Settings(SettingsMsg::Remove(f, i))), false));
            rows.push(container(r).padding([4, 8]).padding(iced::Padding { left: 16.0, top: 4.0, bottom: 4.0, right: 8.0 }).width(Length::Fill).into());
        }
        let draft = &self.settings_ui.drafts[slot(f)];
        let invalid = self.settings_ui.error.as_ref().is_some_and(|(ef, _)| *ef == f);
        let input = text_input(placeholder, draft)
            .on_input(move |v| Message::Settings(SettingsMsg::Draft(f, v)))
            .on_submit(Message::Settings(SettingsMsg::Add(f)))
            .size(style::BODY)
            .font(style::FONT)
            .padding([5, 10])
            .style(w::field_style(p, invalid));
        let mut add = column![row![
            input,
            w::text_button(p, &self.icons, "Add", Some("plus"), None, Variant::Secondary, Some(Message::Settings(SettingsMsg::Add(f)))),
            w::text_button(p, &self.icons, "Add current folder", Some("folder-plus"), None, Variant::Ghost, Some(Message::Settings(SettingsMsg::AddCurrent(f)))),
        ]
        .spacing(style::SPACE_3)
        .align_y(Alignment::Center)]
        .spacing(style::SPACE_3);
        if let Some(e) = self.field_error(f) {
            add = add.push(e);
        }
        rows.push(self.block(add));
        rows
    }

    /// Design system `NameChips`.
    fn name_chips<'a>(&self) -> Vec<Element<'a, Message>> {
        let p = &self.palette;
        let mut chips = row![].spacing(6);
        let mut lines = column![].spacing(6);
        let mut width = 0usize;
        for (i, name) in self.settings.search.exclude_names.iter().enumerate() {
            // Wrap by character count: the font is monospaced.
            let w_chars = name.chars().count() + 5;
            if width + w_chars > 76 && width > 0 {
                lines = lines.push(chips);
                chips = row![].spacing(6);
                width = 0;
            }
            width += w_chars;
            let (bg, edge) = (color(p.bg_deep), color(p.line_strong));
            let chip = container(
                row![
                    text(name.clone()).size(style::META).font(style::FONT).color(color(p.ink)).wrapping(text::Wrapping::None),
                    w::icon_button(p, &self.icons, "close", Some(Message::Settings(SettingsMsg::Remove(Field::Names, i))), false),
                ]
                .spacing(0)
                .align_y(Alignment::Center),
            )
            .padding(iced::Padding { left: 8.0, right: 0.0, top: 0.0, bottom: 0.0 })
            .height(24)
            .align_y(Alignment::Center)
            .style(move |_| container::Style { background: Some(Background::Color(bg)), border: Border { color: edge, width: 1.0, radius: 2.0.into() }, ..Default::default() });
            chips = chips.push(chip);
        }
        lines = lines.push(chips);
        let invalid = self.settings_ui.error.as_ref().is_some_and(|(ef, _)| *ef == Field::Names);
        let input = text_input("Folder name, like node_modules", &self.settings_ui.drafts[slot(Field::Names)])
            .on_input(|v| Message::Settings(SettingsMsg::Draft(Field::Names, v)))
            .on_submit(Message::Settings(SettingsMsg::Add(Field::Names)))
            .size(style::BODY)
            .font(style::FONT)
            .padding([5, 10])
            .style(w::field_style(p, invalid));
        let mut add = column![row![
            input,
            w::text_button(p, &self.icons, "Add", Some("plus"), None, Variant::Secondary, Some(Message::Settings(SettingsMsg::Add(Field::Names)))),
            w::text_button(p, &self.icons, "Reset to recommended", Some("undo"), None, Variant::Ghost, Some(Message::Settings(SettingsMsg::ResetExclusions))),
        ]
        .spacing(style::SPACE_3)
        .align_y(Alignment::Center)]
        .spacing(style::SPACE_3);
        if let Some(e) = self.field_error(Field::Names) {
            add = add.push(e);
        }
        vec![self.block(lines), self.block(add)]
    }

    fn mono_block<'a>(&self, lines: &[(&str, &str)]) -> Element<'a, Message> {
        let p = &self.palette;
        let mut col = column![].spacing(style::SPACE_1 + 2.0);
        for (cmd, what) in lines {
            col = col.push(
                row![
                    text(cmd.to_string()).size(style::META).font(style::FONT).color(color(p.ink_strong)).width(Length::FillPortion(5)).wrapping(text::Wrapping::None),
                    text(what.to_string()).size(style::META).font(style::FONT).color(color(p.ink_muted)).width(Length::FillPortion(4)),
                ]
                .spacing(style::SPACE_4),
            );
        }
        let (bg, edge) = (color(p.bg_deep), color(p.line));
        container(col)
            .padding([12, 14])
            .width(Length::Fill)
            .style(move |_| container::Style { background: Some(Background::Color(bg)), border: Border { color: edge, width: 1.0, radius: 4.0.into() }, ..Default::default() })
            .into()
    }

    fn page<'a>(&self, head: Element<'a, Message>, groups: Vec<Element<'a, Message>>) -> Element<'a, Message> {
        let mut col = column![head].spacing(28);
        if let Some(prob) = &self.settings_ui.problem {
            col = col.push(self.note(self.palette.danger, "error", prob.clone(), None));
        }
        for g in groups {
            col = col.push(g);
        }
        col.push(Space::new().height(24)).into()
    }

    // ------------------------------------------------------------------ pages

    fn page_general(&self) -> Element<'_, Message> {
        let g = &self.settings.general;
        let p = &self.palette;
        let login_on = g.start_at_login && g.background;
        let window = self.group(
            "Running",
            vec![
                self.setting(
                    "Keep running in the background",
                    "Closing the window hides EchoFiles instead of quitting. The next window opens instantly and search stays up to date.",
                    self.toggle(g.background, SettingsMsg::Background),
                ),
                self.setting_with(
                    "Start at login",
                    if g.background { "Starts hidden when you log in, so even the first window is instant." } else { "Needs Keep running in the background." },
                    !g.background,
                    w::switch(p, login_on, g.background.then(|| Box::new(|b| Message::Settings(SettingsMsg::StartAtLogin(b))) as Box<dyn Fn(bool) -> Message>)),
                ),
            ],
        );
        let windows = self.group(
            "Windows",
            vec![
                self.setting(
                    "New windows open at",
                    "Where EchoFiles starts when you open it without a folder.",
                    w::segmented(
                        p,
                        &[("Home", Message::Settings(SettingsMsg::OpenTo(OpenTo::Home))), ("Last folder", Message::Settings(SettingsMsg::OpenTo(OpenTo::Last)))],
                        if g.open_to == OpenTo::Home { 0 } else { 1 },
                    ),
                ),
                self.setting("Show hidden files", "Dotfiles and dot-folders. Ctrl+H switches it for the open window.", self.toggle(g.show_hidden, SettingsMsg::ShowHidden)),
            ],
        );
        self.page(self.page_head("General", "How EchoFiles starts, runs and opens."), vec![window, windows])
    }

    fn index_status(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let entries: usize = self.roots.iter().map(|r| r.entries()).sum();
        let bytes: u64 = self.roots.iter().map(|r| r.bytes()).sum();
        let updated = self.roots.iter().filter_map(|r| r.updated).min();
        let (pill, detail) = match self.index_state {
            IndexState::Off => (w::pill(p, "Off", None), "Everywhere search walks your folders live instead — slower on big folders.".to_string()),
            IndexState::Opening => (w::pill(p, "Opening", Some(p.info)), "Loading the saved index…".to_string()),
            IndexState::Building if entries == 0 => (w::pill(p, "Indexing", Some(p.info)), "Building the index for the first time. Search works live meanwhile.".to_string()),
            IndexState::Building => (w::pill(p, "Updating", Some(p.info)), format!("{} items · {} · updating now", fmt_count(entries), mb(bytes))),
            IndexState::Ready if self.index_error.is_some() => (w::pill(p, "Problem", Some(p.warning)), self.index_error.clone().unwrap_or_default()),
            IndexState::Ready => (w::pill(p, "Ready", Some(p.success)), format!("{} items · {} on disk · updated {}", fmt_count(entries), mb(bytes), indexer::ago(updated))),
        };
        let building = self.index_state == IndexState::Building;
        let rebuild = w::text_button(
            p,
            &self.icons,
            if building { "Indexing…" } else { "Rebuild now" },
            Some("refresh"),
            None,
            Variant::Secondary,
            (!building && self.settings.search.index).then_some(Message::Settings(SettingsMsg::Rebuild)),
        );
        container(
            row![
                pill,
                text(detail).size(style::META).font(style::FONT).color(color(p.ink_muted)).width(Length::Fill),
                rebuild,
            ]
            .spacing(style::SPACE_4)
            .align_y(Alignment::Center),
        )
        .padding([10, 16])
        .width(Length::Fill)
        .into()
    }

    fn page_search(&self) -> Element<'_, Message> {
        let s = &self.settings.search;
        let p = &self.palette;
        let mut index_rows = vec![self.setting(
            "Search index",
            "A list of file names kept on disk so Everywhere search answers instantly and AI agents can use ef. Updates within seconds in Downloads, Desktop and Documents and every minute elsewhere. Turning it off deletes the index.",
            self.toggle(s.index, SettingsMsg::Index),
        )];
        index_rows.push(self.index_status());
        let index = self.group("Index", index_rows);

        let search = self.group(
            "Search box",
            vec![self.setting(
                "Search looks in",
                "What the search box searches when a window opens. Ctrl+E switches it any time.",
                w::segmented(
                    p,
                    &[("This folder", Message::Settings(SettingsMsg::DefaultScope(Scope::Folder))), ("Everywhere", Message::Settings(SettingsMsg::DefaultScope(Scope::Everywhere)))],
                    if s.default_scope == Scope::Folder { 0 } else { 1 },
                ),
            )],
        );

        let mut roots = vec![self.block(
            text("Everywhere search and ef cover these folders and everything inside them.").size(style::META).font(style::FONT).color(color(p.ink_muted)),
        )];
        roots.extend(self.path_list(Field::Roots, &s.roots, "", "Folder path, like ~/Projects"));
        let roots = self.group("Indexed folders", roots);

        let mut never = vec![self.block(
            text("Folders that never appear in search results — private or noisy places. Their contents aren't indexed at all.").size(style::META).font(style::FONT).color(color(p.ink_muted)),
        )];
        never.extend(self.path_list(Field::Exclude, &s.exclude_paths, "Nothing excluded.", "Folder path, like ~/Private"));
        let never = self.group("Never show in search", never);

        let mut skip = vec![self.setting(
            "Skip cache folders",
            "Folders marked with CACHEDIR.TAG (browser, build and package caches). They're still listed by name.",
            self.toggle(s.skip_cache_folders, SettingsMsg::SkipCache),
        )];
        skip.push(self.block(
            text("Folders with these names are listed but their contents aren't indexed — package stores, build output, version control internals.")
                .size(style::META)
                .font(style::FONT)
                .color(color(p.ink_muted)),
        ));
        skip.extend(self.name_chips());
        let skip = self.group("Skip contents of", skip);

        self.page(self.page_head("Search & index", "What search covers, and the index that makes it instant."), vec![index, search, roots, never, skip])
    }

    fn page_agents(&self) -> Element<'_, Message> {
        let a = &self.settings.agents;
        let p = &self.palette;
        let ef_path = system::ef_on_path();
        let location: Element<'_, Message> = match &ef_path {
            Some(path) => text(config::tilde(path)).size(style::META).font(style::FONT).color(color(p.ink_muted)).into(),
            None => w::pill(p, "Not installed", Some(p.warning)),
        };
        let mut groups = Vec::new();
        if !self.settings.search.index && a.cli {
            groups.push(self.note(p.warning, "alert", "ef answers from the search index, which is off. Agents will be told to turn it on.".into(), Some(("Go to Search & index", SettingsMsg::Page(Page::Search)))));
        }
        groups.push(self.group(
            "Command line",
            vec![
                self.setting(
                    "Allow EchoFiles commands",
                    "Lets AI agents and scripts search indexed files quickly and search other folders live with ef find --in. When off, ef refuses and says it's turned off.",
                    self.toggle(a.cli, SettingsMsg::Cli),
                ),
                self.setting(
                    "ef command",
                    if ef_path.is_some() { "Installed on your PATH." } else { "Run scripts/install.sh from the EchoFiles folder to put ef on your PATH." },
                    location,
                ),
            ],
        ));
        groups.push(self.group(
            "Skill",
            vec![self.setting_with(
                "Teach AI agents about ef",
                "Makes the EchoFiles skill available to Claude, Codex, Gemini, OpenCode and Pi in every project. Turning it off removes EchoFiles' skill links.",
                !a.cli,
                w::switch(p, self.settings_ui.skill_installed, a.cli.then(|| Box::new(|b| Message::Settings(SettingsMsg::Skill(b))) as Box<dyn Fn(bool) -> Message>)),
            )],
        ));
        groups.push(self.group(
            "Commands",
            vec![self.block(self.mono_block(&[
                ("ef find report", "names containing “report”"),
                ("ef find '*' --ext pdf --limit 50", "every PDF, first 50"),
                ("ef find invoice --in ~/Documents", "only inside a folder"),
                ("ef find src --dirs", "folders only"),
                ("ef status", "what's indexed, how fresh"),
                ("ef index", "update the index now"),
            ]))],
        ));
        self.page(self.page_head("AI agents", "Let AI agents on this computer use EchoFiles' index. Nothing leaves your computer."), groups)
    }

    fn page_appearance(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mut swatches = row![].spacing(6);
        for c in [p.bg, p.bg_raised, p.ink, p.accent, p.world_linux, p.world_windows, p.success.base, p.warning.base, p.danger.base] {
            let edge = color(p.line_strong);
            swatches = swatches.push(container(Space::new()).width(18).height(18).style(move |_| container::Style {
                background: Some(Background::Color(color(c))),
                border: Border { color: edge, width: 1.0, radius: 2.0.into() },
                ..Default::default()
            }));
        }
        let theme = self.group(
            "Theme",
            vec![
                self.setting(
                    &format!("Omarchy theme · {}", if p.name.is_empty() { "default" } else { &p.name }),
                    "EchoFiles follows your Omarchy theme and font and switches with them. Change the theme from the Omarchy menu.",
                    swatches.into(),
                ),
            ],
        );
        let d = self.settings.appearance.density;
        let layout = self.group(
            "Layout",
            vec![self.setting(
                "Row height",
                "How tightly the file list is packed.",
                w::segmented(
                    p,
                    &[
                        ("Compact", Message::Settings(SettingsMsg::Density(Density::Compact))),
                        ("Default", Message::Settings(SettingsMsg::Density(Density::Default))),
                        ("Comfortable", Message::Settings(SettingsMsg::Density(Density::Comfortable))),
                    ],
                    match d {
                        Density::Compact => 0,
                        Density::Default => 1,
                        Density::Comfortable => 2,
                    },
                ),
            )],
        );
        self.page(self.page_head("Appearance", "EchoFiles looks like the rest of your desktop."), vec![theme, layout])
    }

    fn page_about(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let open = |path: PathBuf| w::text_button(p, &self.icons, "Show", Some("folder"), None, Variant::Ghost, Some(Message::Settings(SettingsMsg::Reveal(path))));
        let info = self.group(
            "EchoFiles",
            vec![
                self.setting("Version", concat!("EchoFiles ", env!("CARGO_PKG_VERSION"), " · Linux"), Space::new().into()),
                self.setting("Settings file", &config::tilde(&config::Settings::path()), open(config::config_dir())),
                self.setting("Index files", &config::tilde(&config::index_dir()), open(config::index_dir())),
            ],
        );
        let keys = self.group(
            "Keyboard",
            vec![self.block(self.mono_block(&[
                ("Ctrl+F  /", "search"),
                ("Ctrl+E", "search this folder ↔ everywhere"),
                ("Esc", "clear search · leave Settings"),
                ("Alt+← Alt+→ Alt+↑", "back · forward · parent"),
                ("Backspace", "parent folder"),
                ("Ctrl+H", "show hidden files"),
                ("F5", "reload"),
                ("Ctrl+,", "settings"),
                ("Ctrl+Q", "quit"),
            ]))],
        );
        self.page(self.page_head("About", "Where EchoFiles keeps things, and how to drive it from the keyboard."), vec![info, keys])
    }
}

fn fmt_count(n: usize) -> String {
    let mut s = String::new();
    ef_core::fmt::count(n, &mut s);
    s
}

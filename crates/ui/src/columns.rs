//! Column view (Miller columns, the macOS browser): the folders above the open one stand
//! in columns on the left with the way down highlighted, the open folder is the full file
//! list in the middle, and what's inside the item under the cursor fills the column on the
//! right — so each folder opens in a new column. → enters the folder under the cursor,
//! ← goes back up; clicking in a column opens that folder with the item selected.
//!
//! Also here: the small listings those columns read, which folder peeks (hovering a
//! folder in a list) read too.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ef_core::fmt;
use ef_core::sort::{self, NameKeys, SortSpec};
use ef_core::Listing;
use iced::widget::{column, container, pin, row, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Point, Rectangle, Shadow, Task, Vector};

use crate::app::{background, App, Message};
use crate::file_list::{self, Action, FileList};
use crate::pane::{Layout, Pane};
use crate::style::{self, color};
use crate::widgets as w;

/// A column of a folder above the open one.
const COL_W: f32 = 220.0;
/// The column showing what's in the item under the cursor.
const RIGHT_W: f32 = 260.0;
/// The open folder's list never gets narrower than this.
const CENTER_MIN: f32 = 320.0;
/// Listings kept for columns and peeks beyond the ones on screen.
const KEEP: usize = 64;
/// Items a peek lists.
const PEEK_ROWS: usize = 12;
const PEEK_W: f32 = 300.0;

/// A folder read for a column or a peek, sorted by name with folders first.
pub struct ColData {
    pub listing: Listing,
    pub order: Vec<u32>,
}

impl std::fmt::Debug for ColData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ColData({} entries)", self.listing.len())
    }
}

pub enum Slot {
    Loading,
    Ready(Arc<ColData>),
    Failed(String),
}

/// Listings by folder, for the hidden-files setting they were read with.
#[derive(Default)]
pub struct ColCache {
    map: HashMap<PathBuf, Slot>,
    /// Changed on disk: read again when next wanted, showing the old listing meanwhile.
    stale: HashSet<PathBuf>,
    /// Bumped whenever a listing arrives, so columns rebuild.
    pub rev: u64,
    /// The newest read asked for each folder: an older one finishing later is ignored.
    asked: HashMap<PathBuf, u64>,
    seq: u64,
    /// The folders columns and the peek show, which are watched for changes.
    pub shown: Vec<PathBuf>,
}

impl ColCache {
    pub fn ready(&self, dir: &Path) -> Option<&Arc<ColData>> {
        match self.map.get(dir) {
            Some(Slot::Ready(d)) => Some(d),
            _ => None,
        }
    }

    /// Something changed in `dir` (or everywhere, for `None`).
    pub fn forget(&mut self, dir: Option<&Path>) {
        match dir {
            Some(d) if self.map.contains_key(d) => {
                self.stale.insert(d.to_path_buf());
            }
            Some(_) => {}
            None => self.stale.extend(self.map.keys().cloned()),
        }
        self.rev += 1;
    }

    /// Hidden files were switched on or off: every listing is wrong now.
    pub fn clear(&mut self) {
        self.map.clear();
        self.stale.clear();
        self.asked.clear();
        self.rev += 1;
    }
}

/// One column on the left: a folder above the open one.
pub struct Col {
    pub dir: PathBuf,
    pub data: Option<Arc<ColData>>,
    /// The entry on the way down to the open folder (a selection bitset).
    pub mark: Vec<u64>,
    pub pos: Option<usize>,
}

#[derive(Default)]
pub enum Right {
    #[default]
    Nothing,
    Folder(Col),
    Loading,
    Failed(String),
    /// A file: its entry in the pane's listing.
    File(usize),
}

#[derive(PartialEq, Eq, Clone)]
struct Key {
    location: PathBuf,
    cursor: Option<OsString>,
    rev: u64,
    loaded: usize,
}

/// What a pane in column view shows around its list.
#[derive(Default)]
pub struct Trail {
    pub left: Vec<Col>,
    pub right: Right,
    key: Option<Key>,
}

/// What a folder peek is showing.
pub struct Peek {
    pub pane: u64,
    pub dir: PathBuf,
    pub at: Rectangle,
}

fn read(dir: &Path, hidden: bool) -> Result<Arc<ColData>, String> {
    let listing = ef_core::listing::list(dir).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => "You don't have permission to look inside.".to_string(),
        _ => format!("Couldn't read it: {e}"),
    })?;
    let keys = NameKeys::build(&listing);
    let order = sort::order(&listing, &keys, SortSpec::default(), hidden);
    Ok(Arc::new(ColData { listing, order }))
}

/// Mark `name` in `data`: (bitset, display position).
fn mark(data: &ColData, name: Option<&std::ffi::OsStr>) -> (Vec<u64>, Option<usize>) {
    let mut bits = vec![0; data.listing.len().div_ceil(64)];
    let pos = name.and_then(|n| data.order.iter().position(|&i| data.listing.name(i as usize) == n));
    if let Some(p) = pos {
        file_list::set_bit(&mut bits, data.order[p] as usize, true);
    }
    (bits, pos)
}

impl App {
    /// Folders above `pane`'s, outermost first, starting where its breadcrumbs start.
    fn ancestors(&self, pane: &Pane) -> Vec<PathBuf> {
        self.crumbs_for(&pane.location).into_iter().map(|c| c.2).filter(|p| p != &pane.location && pane.location.starts_with(p)).collect()
    }

    /// Bring the columns of the panes on screen up to date and read the folders they (and a
    /// peek) need. Runs after every message, so it returns early when nothing moved.
    pub(crate) fn sync_columns(&mut self) -> Task<Message> {
        let hidden = self.show_hidden;
        let mut wants: Vec<PathBuf> = Vec::new();
        let mut thumbs: Vec<(PathBuf, i64)> = Vec::new();
        let ids: Vec<u64> = self.tabs[self.tab].panes.iter().filter(|p| p.layout == Layout::Columns && !p.special() && !p.everywhere()).map(|p| p.id).collect();
        for id in ids {
            let Some(pane) = self.pane_by_id(id) else { continue };
            let ancestors = self.ancestors(pane);
            let cursor = pane.cursor.and_then(|c| pane.entry_at(c)).zip(pane.loaded.clone()).map(|(i, l)| (i, l.listing.path(i), file_list::is_folder(&l.listing, i)));
            wants.extend(ancestors.iter().cloned());
            if let Some((_, path, true)) = &cursor {
                wants.push(path.clone());
            }
            let key = Key {
                location: pane.location.clone(),
                cursor: cursor.as_ref().and_then(|c| c.1.file_name().map(|n| n.to_os_string())),
                rev: self.cols.rev,
                loaded: pane.loaded.as_ref().map_or(0, |l| Arc::as_ptr(l) as usize),
            };
            if pane.miller.key.as_ref() == Some(&key) {
                continue;
            }
            let left = ancestors
                .into_iter()
                .map(|dir| {
                    let data = self.cols.ready(&dir).cloned();
                    let child = pane.location.strip_prefix(&dir).ok().and_then(|r| r.components().next()).map(|c| c.as_os_str().to_os_string());
                    let (mark, pos) = data.as_deref().map(|d| mark(d, child.as_deref())).unwrap_or_default();
                    Col { dir, data, mark, pos }
                })
                .collect();
            let right = match cursor {
                None => Right::Nothing,
                Some((i, path, false)) => {
                    if crate::thumbs::thumbnailable(&path) && self.thumbs.get(&path).is_none() {
                        thumbs.push((path, pane.loaded.as_ref().map_or(0, |l| l.listing.mtime[i])));
                    }
                    Right::File(i)
                }
                Some((_, dir, true)) => match self.cols.map.get(&dir) {
                    Some(Slot::Ready(d)) => {
                        let (mark, _) = mark(d, None);
                        Right::Folder(Col { dir, data: Some(d.clone()), mark, pos: None })
                    }
                    Some(Slot::Failed(e)) => Right::Failed(e.clone()),
                    _ => Right::Loading,
                },
            };
            if let Some(p) = self.pane_by_id_mut(id) {
                p.miller = Trail { left, right, key: Some(key) };
            }
        }
        if self.settings.appearance.peek
            && let Some(pk) = &self.peek
        {
            wants.push(pk.dir.clone());
        }
        let mut tasks = Vec::new();
        if !thumbs.is_empty() {
            let jobs = self.thumbs.want(thumbs);
            tasks.push(self.start_thumbs(jobs));
        }
        for dir in &wants {
            let stale = self.cols.stale.remove(dir);
            if self.cols.map.contains_key(dir) && !stale {
                continue;
            }
            if !self.cols.map.contains_key(dir) {
                self.cols.map.insert(dir.clone(), Slot::Loading);
            }
            self.cols.seq += 1;
            let seq = self.cols.seq;
            self.cols.asked.insert(dir.clone(), seq);
            let d = dir.clone();
            tasks.push(background(move || read(&d, hidden), {
                let dir = dir.clone();
                move |r| Message::ColLoaded(dir.clone(), hidden, seq, r)
            }));
        }
        wants.sort();
        wants.dedup();
        if wants != self.cols.shown {
            self.cols.shown = wants.clone();
            self.watch_visible();
        }
        if self.cols.map.len() > KEEP {
            let keep: HashSet<&PathBuf> = wants.iter().collect();
            self.cols.map.retain(|k, v| matches!(v, Slot::Loading) || keep.contains(k));
            let map = &self.cols.map;
            self.cols.stale.retain(|k| map.contains_key(k));
            self.cols.asked.retain(|k, _| map.contains_key(k));
        }
        Task::batch(tasks)
    }

    pub(crate) fn col_loaded(&mut self, dir: PathBuf, hidden: bool, seq: u64, r: Result<Arc<ColData>, String>) {
        if hidden != self.show_hidden || self.cols.asked.get(&dir) != Some(&seq) {
            return;
        }
        self.cols.asked.remove(&dir);
        self.cols.map.insert(dir, match r {
            Ok(d) => Slot::Ready(d),
            Err(e) => Slot::Failed(e),
        });
        self.cols.rev += 1;
    }

    /// → in column view: open the folder under the cursor with its first item selected.
    pub(crate) fn enter_column(&mut self) -> Task<Message> {
        let pane = self.pane();
        let Some(l) = pane.loaded.clone() else { return Task::none() };
        let Some(i) = pane.cursor.and_then(|c| pane.entry_at(c)) else { return Task::none() };
        if !file_list::is_folder(&l.listing, i) {
            return Task::none();
        }
        let t = self.go(l.listing.path(i), true);
        self.pane_mut().select_first = true;
        t
    }

    /// Something happened in one of a pane's side columns (`dir` is the folder it shows).
    pub(crate) fn column_action(&mut self, pane: u64, dir: PathBuf, action: Action) -> Task<Message> {
        let data = self.pane_by_id(pane).and_then(|p| match &p.miller.right {
            Right::Folder(c) if c.dir == dir => c.data.clone(),
            _ => p.miller.left.iter().find(|c| c.dir == dir).and_then(|c| c.data.clone()),
        });
        let Some(data) = data else { return Task::none() };
        let entry = |pos: usize| data.order.get(pos).map(|&i| i as usize);
        match action {
            Action::Cursor { pos, .. } | Action::Open(pos) => {
                let Some(i) = entry(pos) else { return Task::none() };
                let t = self.list_action(pane, Action::Focus);
                let path = data.listing.path(i);
                if matches!(action, Action::Open(_)) {
                    // Double-click: into a folder, or open the file.
                    if file_list::is_folder(&data.listing, i) {
                        return Task::batch([t, self.go(path, true)]);
                    }
                    let go = self.go(dir, true);
                    self.pane_mut().reveal = vec![data.listing.name(i).to_os_string()];
                    return Task::batch([t, go, self.open_path(path)]);
                }
                let go = self.go(dir, true);
                self.pane_mut().reveal = vec![data.listing.name(i).to_os_string()];
                Task::batch([t, go])
            }
            Action::Middle(pos) => match entry(pos) {
                Some(i) if file_list::is_folder(&data.listing, i) => self.update(Message::OpenTab(data.listing.path(i))),
                _ => Task::none(),
            },
            Action::Focus => self.list_action(pane, Action::Focus),
            _ => Task::none(),
        }
    }

    /// The width one pane gets in the window (for fitting columns).
    fn pane_width(&self) -> f32 {
        let mut w = self.window_size.width;
        if !self.settings.sidebar.hidden {
            w -= self.settings.sidebar.width as f32 + 5.0;
        }
        if self.settings.appearance.preview && !self.pane().special() {
            w -= crate::preview::WIDTH + 1.0;
        }
        if self.dual() {
            w = (w - 1.0) / 2.0;
        }
        w
    }

    /// A folder's own icon in a list, when it was given a colour or an icon.
    pub(crate) fn decor_fn<'a>(&'a self, l: &'a Listing) -> Box<dyn Fn(usize) -> Option<iced::advanced::svg::Handle> + 'a> {
        if !self.folders.has_decor() {
            return Box::new(|_| None);
        }
        Box::new(move |i| {
            if !l.is_dir(i) {
                return None;
            }
            let (icon, tint) = self.folders.decor(&l.path(i))?;
            Some(self.icons.folder(icon, tint))
        })
    }

    fn mini_list<'a>(&'a self, pane: u64, col: &'a Col) -> Element<'a, Message> {
        let Some(data) = col.data.as_deref() else {
            return w::fill(Space::new(), self.palette.bg).width(Length::Fill).height(Length::Fill).into();
        };
        let dir = col.dir.clone();
        FileList::new(
            file_list::Model {
                listing: &data.listing,
                order: &data.order,
                selected: &col.mark,
                cut: &[],
                cursor: col.pos,
                generation: data as *const ColData as u64,
                sort: SortSpec::default(),
                skeleton: false,
                empty: data.order.is_empty().then(|| ("Empty".to_string(), String::new())),
                row_h: self.settings.appearance.density.row_height(),
                grid: false,
                keys: false,
                active: false,
                dragging: false,
                renaming: None,
                thumb: Box::new(|_| None),
                zoom: crate::folders::DEFAULT_ZOOM,
                compact: true,
                decor: self.decor_fn(&data.listing),
                peek: false,
            },
            &self.palette,
            &self.icons,
            &self.dates,
            move |a| Message::Column(pane, dir.clone(), a),
        )
        .into()
    }

    fn col_note<'a>(&self, s: String) -> Element<'a, Message> {
        container(text(s).size(style::META).font(style::FONT).color(color(self.palette.ink_muted))).padding(16).width(Length::Fill).into()
    }

    /// The right column for a file: what it is, at a glance.
    fn file_card<'a>(&'a self, pane: &'a Pane, i: usize) -> Element<'a, Message> {
        let p = &self.palette;
        let Some(l) = pane.loaded.as_deref().map(|l| &l.listing) else { return Space::new().into() };
        let name = l.name_bytes(i);
        let path = l.path(i);
        let picture: Element<'a, Message> = match self.thumbs.get(&path) {
            Some(h) => iced::widget::image(h).width(160).height(160).content_fit(iced::ContentFit::Contain).into(),
            None => iced::widget::svg(self.icons.color(crate::kinds::icon(name, l.kind[i]))).width(72).height(72).into(),
        };
        let mut kind = String::new();
        crate::kinds::label(name, l.kind[i], &mut kind);
        let mut size = String::new();
        fmt::size(l.size[i], &mut size);
        let mut date = String::new();
        self.dates.format(l.mtime[i], &mut date);
        let fact = |label: &str, value: String| row![text(label.to_string()).size(style::META).font(style::FONT).color(color(p.ink_muted)).width(72), text(value).size(style::META).font(style::FONT).color(color(p.ink))].spacing(style::SPACE_3);
        column![
            container(picture).width(Length::Fill).align_x(Alignment::Center).padding([16, 0]),
            text(String::from_utf8_lossy(name).into_owned()).size(style::BODY).font(style::FONT_BOLD).color(color(p.ink_strong)),
            fact("Kind", kind),
            fact("Size", size),
            fact("Modified", date),
        ]
        .spacing(style::SPACE_3)
        .padding(16)
        .width(Length::Fill)
        .into()
    }

    /// A pane in column view.
    pub(crate) fn columns_view<'a>(&'a self, pane: &'a Pane, active: bool) -> Element<'a, Message> {
        let p = &self.palette;
        let trail = &pane.miller;
        let fit = ((self.pane_width() - CENTER_MIN - RIGHT_W) / (COL_W + 1.0)).floor().max(0.0) as usize;
        let skip = trail.left.len().saturating_sub(fit);
        let mut r = row![].height(Length::Fill);
        for col in &trail.left[skip..] {
            r = r.push(container(self.mini_list(pane.id, col)).width(COL_W).height(Length::Fill));
            r = r.push(w::vline(p.line));
        }
        r = r.push(container(self.list(pane, active)).width(Length::Fill).height(Length::Fill));
        r = r.push(w::vline(p.line));
        let right: Element<'a, Message> = match &trail.right {
            Right::Folder(col) => self.mini_list(pane.id, col),
            Right::File(i) => self.file_card(pane, *i),
            Right::Loading => self.col_note("Reading…".into()),
            Right::Failed(e) => self.col_note(e.clone()),
            Right::Nothing => Space::new().into(),
        };
        r = r.push(w::fill(right, p.bg_sunken).width(RIGHT_W).height(Length::Fill));
        r.into()
    }

    /// Hovering a folder: a popover with what's inside.
    pub(crate) fn peek_layer(&self) -> Option<Element<'_, Message>> {
        if !self.settings.appearance.peek || self.modal() || self.drag.is_some() {
            return None;
        }
        let pk = self.peek.as_ref().filter(|pk| self.tabs[self.tab].panes.iter().any(|p| p.id == pk.pane))?;
        let p = &self.palette;
        let name = pk.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| pk.dir.display().to_string());
        let head = |count: String| {
            row![
                iced::widget::svg(self.folders.decor(&pk.dir).map_or_else(|| self.icons.color("folder"), |(i, t)| self.icons.folder(i, t))).width(18).height(18),
                text(name.clone()).size(style::BODY).font(style::FONT_BOLD).color(color(p.ink_strong)).width(Length::Fill).wrapping(text::Wrapping::None),
                text(count).size(style::META).font(style::FONT).color(color(p.ink_muted)),
            ]
            .spacing(style::SPACE_3)
            .align_y(Alignment::Center)
        };
        let mut col = column![].spacing(2);
        let mut rows = 1.0;
        match self.cols.map.get(&pk.dir) {
            Some(Slot::Ready(d)) => {
                let n = d.order.len();
                col = col.push(head(format!("{n} {}", if n == 1 { "item" } else { "items" })));
                col = col.push(container(w::hline::<Message>(p.line)).padding([4, 0]));
                if n == 0 {
                    col = col.push(text("Empty folder").size(style::META).font(style::FONT).color(color(p.ink_muted)));
                    rows += 1.0;
                }
                let mut buf = String::new();
                for &i in d.order.iter().take(PEEK_ROWS) {
                    let i = i as usize;
                    let l = &d.listing;
                    let icon = if l.is_dir(i) { self.folders.decor(&l.path(i)).map_or_else(|| self.icons.color("folder"), |(ic, t)| self.icons.folder(ic, t)) } else { self.icons.color(crate::kinds::icon(l.name_bytes(i), l.kind[i])) };
                    buf.clear();
                    if !l.is_dir(i) {
                        fmt::size(l.size[i], &mut buf);
                    }
                    col = col.push(
                        row![
                            iced::widget::svg(icon).width(16).height(16),
                            text(String::from_utf8_lossy(l.name_bytes(i)).into_owned()).size(style::META).font(style::FONT).color(color(p.ink)).width(Length::Fill).wrapping(text::Wrapping::None),
                            text(buf.clone()).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)),
                        ]
                        .spacing(style::SPACE_3)
                        .height(22)
                        .align_y(Alignment::Center),
                    );
                    rows += 1.0;
                }
                if n > PEEK_ROWS {
                    col = col.push(text(format!("and {} more", n - PEEK_ROWS)).size(style::META).font(style::FONT).color(color(p.ink_muted)));
                    rows += 1.0;
                }
            }
            Some(Slot::Failed(e)) => {
                col = col.push(head(String::new()));
                col = col.push(text(e.clone()).size(style::META).font(style::FONT).color(color(p.ink_muted)));
                rows += 1.0;
            }
            _ => {
                col = col.push(head("Reading…".into()));
            }
        }
        let h = rows * 24.0 + 32.0;
        let at = self.fit(Point::new(pk.at.x + 28.0, pk.at.y + pk.at.height + 2.0), PEEK_W, h);
        // Above the folder when there is no room below it.
        let at = if at.y < pk.at.y + pk.at.height { Point::new(at.x, (pk.at.y - h - 2.0).max(4.0)) } else { at };
        let pal = p.clone();
        let panel = container(col.width(Length::Fill)).width(PEEK_W).padding([8, 10]).clip(true).style(move |_| container::Style {
            background: Some(Background::Color(color(pal.bg_raised))),
            border: Border { color: color(pal.line_strong), width: 1.0, radius: 2.0.into() },
            shadow: Shadow { color: Color { a: 0.3, ..Color::BLACK }, offset: Vector::new(0.0, 6.0), blur_radius: 16.0 },
            ..Default::default()
        });
        Some(pin(panel).x(at.x).y(at.y).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_folders_first_and_marks_the_way_down() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("b-folder")).unwrap();
        std::fs::write(dir.path().join("a-file"), b"x").unwrap();
        std::fs::write(dir.path().join(".hidden"), b"x").unwrap();
        let d = read(dir.path(), false).unwrap();
        let names: Vec<_> = d.order.iter().map(|&i| d.listing.name(i as usize).to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["b-folder", "a-file"]);
        assert_eq!(read(dir.path(), true).unwrap().order.len(), 3);
        let (bits, pos) = mark(&d, Some(std::ffi::OsStr::new("a-file")));
        assert_eq!(pos, Some(1));
        assert!(file_list::is_selected(&bits, d.order[1] as usize));
        assert!(!file_list::is_selected(&bits, d.order[0] as usize));
        assert_eq!(mark(&d, None).1, None);
        assert!(read(&dir.path().join("missing"), false).is_err());
    }

    #[test]
    fn forgetting_keeps_the_old_listing_until_the_new_one_arrives() {
        let mut c = ColCache::default();
        let d = Arc::new(ColData { listing: Listing::default(), order: Vec::new() });
        c.map.insert(PathBuf::from("/a"), Slot::Ready(d));
        c.forget(Some(Path::new("/a")));
        c.forget(Some(Path::new("/not-read")));
        assert!(c.ready(Path::new("/a")).is_some());
        assert!(c.stale.contains(Path::new("/a")) && c.stale.len() == 1);
        c.clear();
        assert!(c.ready(Path::new("/a")).is_none());
    }
}

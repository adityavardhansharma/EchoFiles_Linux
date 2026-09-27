//! The folder view: ONE widget that draws only the visible rows or tiles (build plan §2.2,
//! design system `FileList`, `FileRow`, `FileGrid`). Items have a fixed size, so scrolling
//! is pure arithmetic and a 100k folder costs the same per frame as a 100-entry one.
//!
//! Besides drawing, the widget turns pointer and keys into [`Action`]s: selection, open,
//! sort, right-click menus, middle-click tabs, drag and drop with spring-loaded folders,
//! and hosts the inline rename field as its one child.

use std::time::{Duration, Instant};

use ef_core::fmt::{self, DateFormatter};
use ef_core::listing::flags;
use ef_core::{Kind, Listing, SortBy, SortSpec};
use ef_theme::Palette;
use iced::advanced::image::{self as aimage, Image};
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse::click::{self, Click};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::svg::Svg;
use iced::advanced::text::{self, Text};
use iced::advanced::widget::{tree, Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::alignment::Vertical;
use iced::keyboard::{self, key::Named, Key};
use iced::mouse;
use iced::{Background, Border, Color, Element, Event, Length, Pixels, Point, Rectangle, Size, Vector};

use crate::kinds;
use crate::style::{self, color, Icons};

/// Rows drawn above and below the viewport so fast flicks never show a gap.
const OVERSCAN: usize = 3;
const SCROLLBAR: f32 = 6.0;
/// Grid cell (design system `FileGrid`): 104px wide; 88px picture + two lines of name.
const TILE_W: f32 = 104.0;
const TILE_H: f32 = 132.0;
const THUMB: f32 = 88.0;
/// Pointer travel before a press becomes a drag.
const DRAG_START: f32 = 6.0;
/// Hovering a folder this long during a drag opens it (spring-loading).
const SPRING: Duration = Duration::from_millis(700);
/// Hover this long over a cut-off name to see all of it.
const TIP_DELAY: Duration = Duration::from_millis(500);

/// What the list reports to the app. Positions index `order` (display order).
#[derive(Debug, Clone)]
pub enum Action {
    Cursor { pos: usize, extend: bool, toggle: bool },
    Open(usize),
    SelectAll,
    Sort(SortBy),
    /// Right-click: on an item (already selected by a `Cursor` before this) or empty space.
    Context { pos: Option<usize>, at: Point },
    /// Middle-click: open a folder in a new tab.
    Middle(usize),
    /// Click on empty space.
    ClearSelection,
    /// A press moved far enough to be a drag of the selection.
    DragStart,
    /// The drag ended over this list: on a folder item, or on the folder itself.
    Drop { pos: Option<usize> },
    /// A dragged-over folder has waited long enough: open it.
    Spring(usize),
    /// Clicked outside the rename field.
    EditorBlur,
    /// Visible items changed (grid thumbnails load for these).
    Visible { first: usize, last: usize },
    /// Any press inside: this pane becomes the active one.
    Focus,
}

/// Everything the list needs to draw one frame; owned by the app.
pub struct Model<'a> {
    pub listing: &'a Listing,
    pub order: &'a [u32],
    pub selected: &'a [u64],
    /// Entries shown dimmed because they were cut (bitset like `selected`).
    pub cut: &'a [u64],
    pub cursor: Option<usize>,
    /// Bumped on every navigation so the widget resets its scroll position.
    pub generation: u64,
    pub sort: SortSpec,
    /// Show placeholder rows instead of entries (a slow folder is still loading).
    pub skeleton: bool,
    /// Title and hint drawn when there is nothing to show.
    pub empty: Option<(String, String)>,
    /// Row height from Settings → Appearance → Density.
    pub row_h: f32,
    pub grid: bool,
    /// This list takes keyboard input (the active pane, nothing modal open).
    pub keys: bool,
    /// The active pane of two: drawn with the accent top edge by the caller.
    pub active: bool,
    /// Something is being dragged somewhere in the window.
    pub dragging: bool,
    /// Position being renamed; the editor child sits over its name.
    pub renaming: Option<usize>,
    /// Decoded thumbnail for an entry, if ready.
    pub thumb: Box<dyn Fn(usize) -> Option<aimage::Handle> + 'a>,
}

/// JetBrains Mono advances every glyph by 0.6 em.
const ADVANCE: f32 = style::BODY * 0.6;

/// Terminal-style column width: East Asian wide characters and emoji take two cells.
fn cols_of(c: char) -> usize {
    let u = c as u32;
    let wide = matches!(u, 0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x1F300..=0x1FAFF | 0x20000..=0x3FFFD);
    if wide { 2 } else { 1 }
}

/// Split a name into (stem, extension) that fit in `max` cells: the extension always stays
/// visible and the stem ends in `…` when cut, e.g. `Quarterly report for…​.xlsx`.
fn fit_name(name: &str, is_dir: bool, max: usize) -> (String, String, bool) {
    let (stem, ext) = match name.rfind('.') {
        Some(p) if p > 0 && !is_dir && name.len() - p <= 8 => (&name[..p], &name[p..]),
        _ => (name, ""),
    };
    let width = |s: &str| s.chars().map(cols_of).sum::<usize>();
    if width(stem) + width(ext) <= max {
        return (stem.to_string(), ext.to_string(), false);
    }
    let budget = max.saturating_sub(width(ext) + 1);
    let mut out = String::new();
    let mut used = 0;
    for c in stem.chars() {
        let w = cols_of(c);
        if used + w > budget {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    (out, ext.to_string(), true)
}

/// Break a name into at most two lines of `max` cells for a tile; the second ends in `…`.
fn two_lines(name: &str, max: usize) -> (String, String, bool) {
    let mut a = String::new();
    let mut b = String::new();
    let mut used = 0;
    let mut chars = name.chars().peekable();
    while let Some(&c) = chars.peek() {
        if used + cols_of(c) > max {
            break;
        }
        a.push(c);
        used += cols_of(c);
        chars.next();
    }
    used = 0;
    let rest: String = chars.collect();
    let rest_w: usize = rest.chars().map(cols_of).sum();
    if rest_w <= max {
        return (a, rest, false);
    }
    // Keep the end of the name (the extension) visible: "…d report.pdf".
    let tail: Vec<char> = rest.chars().rev().take_while(|c| {
        used += cols_of(*c);
        used < max
    }).collect();
    b.push('…');
    b.extend(tail.into_iter().rev());
    (a, b, true)
}

pub fn is_selected(bits: &[u64], entry: usize) -> bool {
    bits.get(entry / 64).is_some_and(|w| w & (1 << (entry % 64)) != 0)
}

pub fn set_bit(bits: &mut [u64], i: usize, on: bool) {
    if let Some(w) = bits.get_mut(i / 64) {
        if on { *w |= 1 << (i % 64) } else { *w &= !(1 << (i % 64)) }
    }
}

/// A folder, or a link to one: something you can drop into and open in place.
fn is_folder(l: &Listing, i: usize) -> bool {
    l.is_dir(i) || (l.kind[i] == Kind::Symlink && l.flags[i] & flags::LINK_DIR != 0)
}

pub struct FileList<'a, Message> {
    model: Model<'a>,
    palette: &'a Palette,
    icons: &'a Icons,
    dates: &'a DateFormatter,
    on_action: Box<dyn Fn(Action) -> Message + 'a>,
    editor: Option<Element<'a, Message>>,
}

#[derive(Default)]
struct State {
    offset: f32,
    generation: u64,
    last_cursor: Option<usize>,
    last_click: Option<Click>,
    modifiers: keyboard::Modifiers,
    viewport_rows: usize,
    columns: usize,
    dragging_thumb: Option<f32>,
    /// Left button down on an item: where, and whether selecting waits for the release
    /// (pressing an already-selected item keeps the selection so it can be dragged).
    press: Option<(Point, usize, bool)>,
    drag_started: bool,
    /// Folder under the pointer during a drag, and since when.
    drop_hover: Option<(usize, Instant)>,
    sprung: Option<usize>,
    /// Item under the pointer, and since when (for the full-name tooltip).
    hover: Option<(usize, Instant)>,
    visible: (usize, usize),
}

impl<'a, Message> FileList<'a, Message> {
    pub fn new(model: Model<'a>, palette: &'a Palette, icons: &'a Icons, dates: &'a DateFormatter, on_action: impl Fn(Action) -> Message + 'a) -> Self {
        Self { model, palette, icons, dates, on_action: Box::new(on_action), editor: None }
    }

    /// The inline rename field, drawn over the renamed item's name.
    pub fn editor(mut self, e: Option<Element<'a, Message>>) -> Self {
        self.editor = e;
        self
    }

    fn cols(&self, body: Rectangle) -> usize {
        if self.model.grid { (((body.width - style::SPACE_4 - SCROLLBAR) / TILE_W).floor() as usize).max(1) } else { 1 }
    }

    fn item_h(&self) -> f32 {
        if self.model.grid { TILE_H } else { self.model.row_h }
    }

    fn lines(&self, cols: usize) -> usize {
        self.model.order.len().div_ceil(cols)
    }

    fn content_height(&self, cols: usize) -> f32 {
        self.lines(cols) as f32 * self.item_h() + if self.model.grid { style::SPACE_4 } else { 0.0 }
    }

    fn max_offset(&self, body: Rectangle) -> f32 {
        (self.content_height(self.cols(body)) - body.height).max(0.0)
    }

    /// Where item `pos` is drawn, in window coordinates.
    fn item_rect(&self, state: &State, body: Rectangle, pos: usize) -> Rectangle {
        let cols = self.cols(body);
        let h = self.item_h();
        if self.model.grid {
            let (r, c) = (pos / cols, pos % cols);
            Rectangle { x: body.x + style::SPACE_4 / 2.0 + c as f32 * TILE_W, y: body.y + style::SPACE_4 / 2.0 + r as f32 * h - state.offset, width: TILE_W - 4.0, height: h - 4.0 }
        } else {
            Rectangle { x: body.x + style::SPACE_1, y: body.y + pos as f32 * h - state.offset, width: body.width - style::SPACE_1 * 2.0 - SCROLLBAR, height: h }
        }
    }

    fn item_at(&self, state: &State, body: Rectangle, p: Point) -> Option<usize> {
        if !body.contains(p) {
            return None;
        }
        let cols = self.cols(body);
        let h = self.item_h();
        if self.model.grid {
            let y = p.y - body.y - style::SPACE_4 / 2.0 + state.offset;
            let x = p.x - body.x - style::SPACE_4 / 2.0;
            if x < 0.0 || y < 0.0 {
                return None;
            }
            let c = (x / TILE_W) as usize;
            if c >= cols {
                return None;
            }
            let pos = (y / h) as usize * cols + c;
            (pos < self.model.order.len() && self.item_rect(state, body, pos).contains(p)).then_some(pos)
        } else {
            let pos = ((p.y - body.y + state.offset) / h) as usize;
            (pos < self.model.order.len()).then_some(pos)
        }
    }

    /// The rename field's box for item `pos`.
    fn editor_rect(&self, state: &State, body: Rectangle, bounds: Rectangle, pos: usize) -> Rectangle {
        let r = self.item_rect(state, body, pos);
        if self.model.grid {
            Rectangle { x: r.x - 8.0, y: r.y + THUMB + 6.0, width: r.width + 16.0, height: 26.0 }
        } else {
            let cols = columns(bounds);
            Rectangle { x: cols.name - 6.0, y: r.y + (r.height - 24.0) / 2.0, width: cols.name_w + 12.0, height: 24.0 }
        }
    }
}

/// Split the widget into header and body. The grid has no header.
fn areas(bounds: Rectangle, grid: bool) -> (Rectangle, Rectangle) {
    let hh = if grid { 0.0 } else { style::HEADER };
    let header = Rectangle { height: hh, ..bounds };
    let body = Rectangle { y: bounds.y + hh, height: (bounds.height - hh).max(0.0), ..bounds };
    (header, body)
}

struct Columns {
    icon: f32,
    name: f32,
    name_w: f32,
    size_right: f32,
    kind: f32,
    date: f32,
    /// Narrow panes drop Kind first, then Modified, so names keep room.
    show_kind: bool,
    show_date: bool,
}

fn columns(b: Rectangle) -> Columns {
    let pad = style::SPACE_5;
    let show_date = b.width >= 400.0;
    let show_kind = b.width >= 600.0;
    let right = b.x + b.width - pad - SCROLLBAR;
    let date = if show_date { right - style::COL_DATE } else { right + style::SPACE_4 };
    let kind = if show_kind { date - style::SPACE_4 - style::COL_KIND } else { date };
    let size_right = kind - style::SPACE_4;
    let icon = b.x + pad;
    let name = icon + style::ICON_ROW + style::SPACE_3;
    let name_w = (size_right - style::COL_SIZE - style::SPACE_4 - name).max(40.0);
    Columns { icon, name, name_w, size_right, kind, date, show_kind, show_date }
}

/// Which column header is under `x`.
fn header_column(c: &Columns, x: f32) -> SortBy {
    if c.show_date && x >= c.date {
        SortBy::Modified
    } else if c.show_kind && x >= c.kind {
        SortBy::Kind
    } else if x >= c.size_right - style::COL_SIZE {
        SortBy::Size
    } else {
        SortBy::Name
    }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for FileList<'a, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.editor.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        match &self.editor {
            Some(e) => tree.diff_children(std::slice::from_ref(e)),
            None => tree.children.clear(),
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let size = limits.max();
        let bounds = Rectangle::new(Point::ORIGIN, size);
        let (_, body) = areas(bounds, self.model.grid);
        let cols = self.cols(body);
        let h = self.item_h();
        let max = self.max_offset(body);
        let state = tree.state.downcast_mut::<State>();
        state.viewport_rows = (body.height / h).floor().max(1.0) as usize;
        state.columns = cols;
        if state.generation != self.model.generation {
            state.generation = self.model.generation;
            state.offset = 0.0;
            state.last_cursor = None;
        }
        // Keep the keyboard cursor in view whenever it moves.
        if self.model.cursor != state.last_cursor {
            if let Some(c) = self.model.cursor {
                let top = (c / cols) as f32 * h;
                if top < state.offset {
                    state.offset = top;
                } else if top + h > state.offset + body.height {
                    state.offset = top + h - body.height + if self.model.grid { style::SPACE_4 } else { 0.0 };
                }
            }
            state.last_cursor = self.model.cursor;
        }
        state.offset = state.offset.clamp(0.0, max);
        let mut children = Vec::new();
        let rect = self.model.renaming.map(|pos| self.editor_rect_in(tree.state.downcast_ref::<State>(), bounds, pos));
        if let (Some(editor), Some(r)) = (self.editor.as_mut(), rect) {
            let node = editor.as_widget_mut().layout(&mut tree.children[0], renderer, &layout::Limits::new(Size::ZERO, r.size()));
            children.push(node.move_to(r.position()));
        }
        layout::Node::with_children(size, children)
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operation: &mut dyn Operation) {
        if let (Some(editor), Some(child)) = (self.editor.as_mut(), layout.children().next()) {
            operation.traverse(&mut |operation| editor.as_widget_mut().operate(&mut tree.children[0], child, renderer, operation));
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // The rename field gets first say over everything inside its box.
        if let (Some(editor), Some(child)) = (self.editor.as_mut(), layout.children().next()) {
            editor.as_widget_mut().update(&mut tree.children[0], event, child, cursor, renderer, clipboard, shell, viewport);
            if shell.is_event_captured() {
                return;
            }
            if let Event::Mouse(mouse::Event::ButtonPressed(_)) = event
                && cursor.is_over(layout.bounds()) && !cursor.is_over(child.bounds()) {
                    shell.publish((self.on_action)(Action::EditorBlur));
                }
        }
        let bounds = layout.bounds();
        let (header, body) = areas(bounds, self.model.grid);
        let n = self.model.order.len();
        let max = self.max_offset(body);
        let state = tree.state.downcast_mut::<State>();
        let publish = |shell: &mut Shell<'_, Message>, a: Action| shell.publish((self.on_action)(a));
        let l = self.model.listing;

        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => state.modifiers = *m,
            Event::Window(iced::window::Event::RedrawRequested(now)) => {
                // Report which items are on screen (thumbnails load for exactly these).
                let cols = state.columns.max(1);
                let h = self.item_h();
                let first = (state.offset / h) as usize * cols;
                let last = (((state.offset + body.height) / h).ceil() as usize * cols).min(n);
                if (first, last) != state.visible {
                    state.visible = (first, last);
                    publish(shell, Action::Visible { first, last });
                }
                // Spring-loading and the name tooltip wake up on their own.
                if let Some((pos, since)) = state.drop_hover
                    && self.model.dragging && state.sprung != Some(pos) {
                        if *now >= since + SPRING {
                            state.sprung = Some(pos);
                            publish(shell, Action::Spring(pos));
                        } else {
                            shell.request_redraw();
                        }
                    }
                if let Some((_, since)) = state.hover
                    && *now < since + TIP_DELAY {
                        shell.request_redraw_at(since + TIP_DELAY);
                    }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(body) => {
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * self.item_h() * if self.model.grid { 1.0 } else { 3.0 },
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };
                let next = (state.offset + dy).clamp(0.0, max);
                if next != state.offset {
                    state.offset = next;
                    state.hover = None;
                    shell.request_redraw();
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                let Some(p) = cursor.position_over(bounds) else { return };
                publish(shell, Action::Focus);
                if *button == mouse::Button::Left && header.contains(p) {
                    let by = header_column(&columns(bounds), p.x);
                    publish(shell, Action::Sort(by));
                    shell.capture_event();
                    return;
                }
                // Scrollbar thumb drag.
                if *button == mouse::Button::Left && p.x >= body.x + body.width - SCROLLBAR * 2.0 && body.contains(p) && max > 0.0 {
                    state.dragging_thumb = Some(p.y);
                    shell.capture_event();
                    return;
                }
                let hit = self.item_at(state, body, p);
                match (button, hit) {
                    (mouse::Button::Left, Some(pos)) => {
                        let click = Click::new(p, mouse::Button::Left, state.last_click);
                        state.last_click = Some(click);
                        let m = state.modifiers;
                        let i = self.model.order[pos] as usize;
                        if click.kind() == click::Kind::Double {
                            publish(shell, Action::Open(pos));
                            state.press = None;
                        } else if is_selected(self.model.selected, i) && !m.shift() && !m.control() {
                            // Keep the selection for a drag; a plain click selects on release.
                            state.press = Some((p, pos, true));
                        } else {
                            publish(shell, Action::Cursor { pos, extend: m.shift(), toggle: m.control() });
                            state.press = Some((p, pos, false));
                        }
                        state.drag_started = false;
                        shell.capture_event();
                    }
                    (mouse::Button::Left, None) if body.contains(p) => {
                        publish(shell, Action::ClearSelection);
                        shell.capture_event();
                    }
                    (mouse::Button::Right, hit) if body.contains(p) => {
                        if let Some(pos) = hit {
                            let i = self.model.order[pos] as usize;
                            if !is_selected(self.model.selected, i) {
                                publish(shell, Action::Cursor { pos, extend: false, toggle: false });
                            }
                        } else {
                            publish(shell, Action::ClearSelection);
                        }
                        publish(shell, Action::Context { pos: hit, at: p });
                        shell.capture_event();
                    }
                    (mouse::Button::Middle, Some(pos)) => {
                        publish(shell, Action::Middle(pos));
                        shell.capture_event();
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(from) = state.dragging_thumb {
                    let content = self.content_height(self.cols(body)).max(1.0);
                    let dy = (position.y - from) * content / body.height.max(1.0);
                    state.offset = (state.offset + dy).clamp(0.0, max);
                    state.dragging_thumb = Some(position.y);
                    shell.request_redraw();
                    return;
                }
                if let Some((start, _, _)) = state.press
                    && !state.drag_started && start.distance(*position) > DRAG_START {
                        state.drag_started = true;
                        publish(shell, Action::DragStart);
                    }
                let over = cursor.is_over(bounds);
                let hit = if over { self.item_at(state, body, *position) } else { None };
                if self.model.dragging {
                    let folder = hit.filter(|&pos| {
                        let i = self.model.order[pos] as usize;
                        is_folder(l, i) && !is_selected(self.model.selected, i)
                    });
                    if folder != state.drop_hover.map(|d| d.0) {
                        state.drop_hover = folder.map(|pos| (pos, Instant::now()));
                        state.sprung = None;
                    }
                    shell.request_redraw();
                } else if state.drop_hover.is_some() {
                    state.drop_hover = None;
                }
                if hit != state.hover.map(|h| h.0) {
                    state.hover = hit.map(|pos| (pos, Instant::now()));
                    shell.request_redraw();
                } else if over {
                    // Header hover follows the pointer too.
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging_thumb = None;
                if let Some((_, pos, deferred)) = state.press.take()
                    && deferred && !state.drag_started {
                        publish(shell, Action::Cursor { pos, extend: false, toggle: false });
                    }
                if self.model.dragging && let Some(p) = cursor.position_over(body) {
                    let target = self.item_at(state, body, p).filter(|&pos| {
                        let i = self.model.order[pos] as usize;
                        is_folder(l, i) && !is_selected(self.model.selected, i)
                    });
                    publish(shell, Action::Drop { pos: target });
                }
                state.drag_started = false;
                state.drop_hover = None;
                state.sprung = None;
            }
            // Alt+arrows are history and parent-folder keys for the app, not list movement.
            Event::Keyboard(keyboard::Event::KeyPressed { key: Key::Named(named), modifiers, .. }) if n > 0 && self.model.keys && !modifiers.alt() => {
                let cur = self.model.cursor.unwrap_or(0);
                let cols = state.columns.max(1);
                let page = state.viewport_rows.saturating_sub(1).max(1) * cols;
                let fresh = self.model.cursor.is_none();
                let target = match named {
                    Named::ArrowDown => Some(if fresh { 0 } else { (cur + cols).min(n - 1) }),
                    Named::ArrowUp => Some(if fresh { 0 } else { cur.saturating_sub(cols) }),
                    Named::ArrowRight if self.model.grid => Some(if fresh { 0 } else { (cur + 1).min(n - 1) }),
                    Named::ArrowLeft if self.model.grid => Some(cur.saturating_sub(1)),
                    Named::PageDown => Some((cur + page).min(n - 1)),
                    Named::PageUp => Some(cur.saturating_sub(page)),
                    Named::Home => Some(0),
                    Named::End => Some(n - 1),
                    Named::Enter if self.model.cursor.is_some() && !modifiers.control() => {
                        publish(shell, Action::Open(cur));
                        shell.capture_event();
                        None
                    }
                    _ => None,
                };
                if let Some(pos) = target {
                    publish(shell, Action::Cursor { pos, extend: modifiers.shift(), toggle: false });
                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key: Key::Character(c), modifiers, .. }) if self.model.keys && modifiers.control() && c.as_str() == "a" => {
                publish(shell, Action::SelectAll);
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn mouse_interaction(&self, tree: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, viewport: &Rectangle, renderer: &iced::Renderer) -> mouse::Interaction {
        if let (Some(editor), Some(child)) = (self.editor.as_ref(), layout.children().next())
            && cursor.is_over(child.bounds()) {
                return editor.as_widget().mouse_interaction(&tree.children[0], child, cursor, viewport, renderer);
            }
        let (header, _) = areas(layout.bounds(), self.model.grid);
        if cursor.is_over(header) {
            mouse::Interaction::Pointer
        } else if self.model.dragging && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(&self, tree: &Tree, renderer: &mut iced::Renderer, theme: &iced::Theme, style: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor, viewport: &Rectangle) {
        use iced::advanced::Renderer as _;

        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let (header, body) = areas(bounds, self.model.grid);
        let p = self.palette;

        renderer.fill_quad(Quad { bounds, ..Quad::default() }, color(p.bg));
        if !self.model.grid {
            self.draw_header(renderer, bounds, header, cursor);
        }

        if self.model.skeleton {
            self.draw_skeleton(renderer, bounds, body);
            return;
        }
        if let Some((title, hint)) = &self.model.empty {
            self.draw_empty(renderer, body, title, hint);
            return;
        }

        let n = self.model.order.len();
        let cols = self.cols(body);
        let h = self.item_h();
        let first = ((state.offset / h) as usize).saturating_sub(OVERSCAN) * cols;
        let last = ((((state.offset + body.height) / h).ceil() as usize + OVERSCAN) * cols).min(n);
        let hover = cursor.position().and_then(|pt| self.item_at(state, body, pt));

        renderer.with_layer(body, |renderer| {
            for pos in first..last {
                if self.model.grid {
                    self.draw_tile(renderer, state, body, pos, hover);
                } else {
                    self.draw_row(renderer, state, bounds, body, pos, hover);
                }
            }
        });

        // Scrollbar.
        let content = self.content_height(cols);
        if content > body.height && body.height > 0.0 {
            let th = (body.height * body.height / content).max(24.0);
            let y = body.y + (body.height - th) * (state.offset / self.max_offset(body).max(1.0));
            let thumb = Rectangle { x: body.x + body.width - SCROLLBAR - 2.0, y, width: SCROLLBAR - 2.0, height: th };
            renderer.fill_quad(Quad { bounds: thumb, border: Border { radius: 3.0.into(), ..Border::default() }, ..Quad::default() }, color(p.line_strong));
        }

        if let (Some(editor), Some(child)) = (self.editor.as_ref(), layout.children().next()) {
            renderer.with_layer(body, |renderer| {
                let r = child.bounds().expand(1.0);
                renderer.fill_quad(Quad { bounds: r, border: Border { color: color(p.focus_ring), width: 1.0, radius: 4.0.into() }, ..Quad::default() }, color(p.bg_deep));
                editor.as_widget().draw(&tree.children[0], renderer, theme, style, child, cursor, viewport);
            });
        } else {
            self.draw_tooltip(renderer, state, bounds, body);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let (Some(editor), Some(child)) = (self.editor.as_mut(), layout.children().next()) else { return None };
        editor.as_widget_mut().overlay(&mut tree.children[0], child, renderer, viewport, translation)
    }
}

// ------------------------------------------------------------------------------ drawing

impl<'a, Message> FileList<'a, Message> {
    fn editor_rect_in(&self, state: &State, bounds: Rectangle, pos: usize) -> Rectangle {
        let (_, body) = areas(bounds, self.model.grid);
        self.editor_rect(state, body, bounds, pos)
    }

    #[allow(clippy::too_many_arguments)]
    fn text(&self, renderer: &mut iced::Renderer, content: String, at: Point, size: f32, font: iced::Font, c: Color, align: text::Alignment, clip: Rectangle) {
        use iced::advanced::text::Renderer as _;
        let shaping = if content.is_ascii() { text::Shaping::Basic } else { text::Shaping::Advanced };
        renderer.fill_text(
            Text {
                content,
                bounds: Size::new(f32::INFINITY, 40.0),
                size: Pixels(size),
                line_height: text::LineHeight::Absolute(Pixels(size + 5.0)),
                font,
                align_x: align,
                align_y: Vertical::Center,
                shaping,
                wrapping: text::Wrapping::None,
            },
            at,
            c,
            clip,
        );
    }

    fn draw_header(&self, renderer: &mut iced::Renderer, bounds: Rectangle, header: Rectangle, cursor: mouse::Cursor) {
        use iced::advanced::Renderer as _;
        let p = self.palette;
        let cols = columns(bounds);
        let arrow = if self.model.sort.descending { " ↓" } else { " ↑" };
        let hovered_col = cursor.position().filter(|pt| header.contains(*pt)).map(|pt| header_column(&cols, pt.x));
        if let Some(by) = hovered_col {
            // Same hover layer as every other clickable thing (design system States).
            let (x0, x1) = match by {
                SortBy::Name => (cols.name - style::SPACE_3, cols.size_right - style::COL_SIZE),
                SortBy::Size => (cols.size_right - style::COL_SIZE, cols.kind),
                SortBy::Kind => (cols.kind - style::SPACE_1, cols.date),
                _ => (cols.date - style::SPACE_1, bounds.x + bounds.width - SCROLLBAR),
            };
            renderer.fill_quad(
                Quad { bounds: Rectangle { x: x0, y: header.y + 2.0, width: x1 - x0 - style::SPACE_1, height: header.height - 4.0 }, border: Border { radius: 2.0.into(), ..Border::default() }, ..Quad::default() },
                color(p.state_hover),
            );
        }
        for (by, title, x, right) in [
            (SortBy::Name, "NAME", cols.name, false),
            (SortBy::Size, "SIZE", cols.size_right, true),
            (SortBy::Kind, "KIND", cols.kind, false),
            (SortBy::Modified, "MODIFIED", cols.date, false),
        ] {
            if (by == SortBy::Kind && !cols.show_kind) || (by == SortBy::Modified && !cols.show_date) {
                continue;
            }
            let active = self.model.sort.by == by;
            let t = if active { format!("{title}{arrow}") } else { title.to_string() };
            let c = if active || hovered_col == Some(by) { color(p.ink_strong) } else { color(p.ink_muted) };
            self.text(renderer, t, Point::new(x, header.center_y()), style::LABEL, style::FONT_BOLD, c, if right { text::Alignment::Right } else { text::Alignment::Left }, header);
        }
        renderer.fill_quad(Quad { bounds: Rectangle { y: header.y + header.height - 1.0, height: 1.0, ..header }, ..Quad::default() }, color(p.line));
    }

    fn draw_skeleton(&self, renderer: &mut iced::Renderer, bounds: Rectangle, body: Rectangle) {
        use iced::advanced::Renderer as _;
        let p = self.palette;
        let cols = columns(bounds);
        let rows = (body.height / self.model.row_h) as usize + 1;
        for r in 0..rows {
            let y = body.y + r as f32 * self.model.row_h + (self.model.row_h - 10.0) / 2.0;
            let w = 0.25 + ((r * 37) % 45) as f32 / 100.0;
            for (x, width, h) in [
                (cols.icon, style::ICON_ROW, 14.0),
                (cols.name, cols.name_w * w, 10.0),
                (cols.size_right - 56.0, 56.0, 10.0),
                (cols.kind, style::COL_KIND * 0.6, 10.0),
                (cols.date, style::COL_DATE * 0.7, 10.0),
            ] {
                let bar = Rectangle { x, y: y + (10.0 - h) / 2.0, width, height: h };
                renderer.fill_quad(Quad { bounds: bar, border: Border { radius: 2.0.into(), ..Border::default() }, ..Quad::default() }, color(p.line));
            }
        }
    }

    fn draw_empty(&self, renderer: &mut iced::Renderer, body: Rectangle, title: &str, hint: &str) {
        use iced::advanced::svg::Renderer as _;
        let p = self.palette;
        let icon = Rectangle { x: body.center_x() - 32.0, y: body.y + body.height * 0.3 - 32.0, width: 64.0, height: 64.0 };
        renderer.draw_svg(Svg::new(self.icons.color("folder-open")), icon, body);
        self.text(renderer, title.to_string(), Point::new(body.center_x(), icon.y + 64.0 + 28.0), 22.0, style::FONT_BOLD, color(p.ink_strong), text::Alignment::Center, body);
        self.text(renderer, hint.to_string(), Point::new(body.center_x(), icon.y + 64.0 + 58.0), style::META, style::FONT, color(p.ink_muted), text::Alignment::Center, body);
    }

    /// Selection, hover, cursor and drop-target ground for an item.
    fn draw_ground(&self, renderer: &mut iced::Renderer, state: &State, r: Rectangle, pos: usize, i: usize, hover: Option<usize>) {
        use iced::advanced::Renderer as _;
        let p = self.palette;
        let selected = is_selected(self.model.selected, i);
        let is_cursor = self.model.cursor == Some(pos) && self.model.active;
        let drop = state.drop_hover.is_some_and(|d| d.0 == pos) && self.model.dragging;
        if drop {
            renderer.fill_quad(Quad { bounds: r, border: Border { color: color(p.accent), width: 1.0, radius: 2.0.into() }, ..Quad::default() }, color(p.accent_soft));
            // Spring-loading: a 2px bar fills over 700ms, then the folder opens.
            if let Some((_, since)) = state.drop_hover {
                let t = (since.elapsed().as_secs_f32() / SPRING.as_secs_f32()).min(1.0);
                renderer.fill_quad(Quad { bounds: Rectangle { y: r.y + r.height - 2.0, height: 2.0, width: r.width * t, ..r }, ..Quad::default() }, color(p.accent));
            }
            return;
        }
        if selected || hover == Some(pos) || is_cursor {
            let bg = if selected { color(p.selection) } else if hover == Some(pos) { color(p.state_hover) } else { Color::TRANSPARENT };
            let border = if is_cursor { Border { color: color(p.focus_ring), width: 1.0, radius: 2.0.into() } } else { Border { radius: 2.0.into(), ..Border::default() } };
            renderer.fill_quad(Quad { bounds: r, border, ..Quad::default() }, Background::Color(bg));
        }
    }

    /// One badge per icon, bottom-right; priority broken > lock > cloud > link.
    fn draw_badge(&self, renderer: &mut iced::Renderer, icon: Rectangle, i: usize, clip: Rectangle) {
        use iced::advanced::svg::Renderer as _;
        use iced::advanced::Renderer as _;
        let l = self.model.listing;
        let f = l.flags[i];
        let (glyph, tint) = if f & flags::BROKEN != 0 {
            ("error", self.palette.danger.ink)
        } else if f & flags::LOCKED != 0 {
            ("lock", self.palette.warning.ink)
        } else if f & flags::CLOUD != 0 {
            ("cloud", self.palette.info.ink)
        } else if l.kind[i] == Kind::Symlink {
            ("link", self.palette.ink_strong)
        } else {
            return;
        };
        let s = (icon.width * 0.55).max(9.0);
        let b = Rectangle { x: icon.x + icon.width - s + 2.0, y: icon.y + icon.height - s + 2.0, width: s, height: s };
        renderer.fill_quad(Quad { bounds: b.expand(1.0), border: Border { radius: (s / 2.0 + 1.0).into(), ..Border::default() }, ..Quad::default() }, color(self.palette.bg_raised));
        renderer.draw_svg(Svg::new(self.icons.glyph(glyph)).color(color(tint)), b.shrink(1.5), clip);
    }

    fn alpha(&self, i: usize) -> f32 {
        let l = self.model.listing;
        if is_selected(self.model.cut, i) {
            0.5 // opacity-cut
        } else if l.flags[i] & flags::HIDDEN != 0 {
            0.6 // opacity-hidden-file
        } else {
            1.0
        }
    }

    fn draw_row(&self, renderer: &mut iced::Renderer, state: &State, bounds: Rectangle, body: Rectangle, pos: usize, hover: Option<usize>) {
        use iced::advanced::svg::Renderer as _;
        let p = self.palette;
        let cols = columns(bounds);
        let l = self.model.listing;
        let i = self.model.order[pos] as usize;
        let row = self.item_rect(state, body, pos);
        let (y, rh) = (row.y, row.height);
        self.draw_ground(renderer, state, row, pos, i, hover);
        let selected = is_selected(self.model.selected, i);
        let name = l.name_bytes(i);
        let alpha = self.alpha(i);
        let icon_box = Rectangle { x: cols.icon, y: y + (rh - style::ICON_ROW) / 2.0, width: style::ICON_ROW, height: style::ICON_ROW };
        renderer.draw_svg(Svg { opacity: alpha, ..Svg::new(self.icons.color(kinds::icon(name, l.kind[i]))) }, icon_box, body);
        self.draw_badge(renderer, icon_box, i, body);
        let cy = y + rh / 2.0;
        if self.model.renaming != Some(pos) {
            let full = String::from_utf8_lossy(name);
            let max_cells = (cols.name_w / ADVANCE).floor() as usize;
            let (mut stem, mut ext, _) = fit_name(&full, l.is_dir(i), max_cells);
            if !name.is_ascii() {
                // Shaped text doesn't advance exactly 0.6 em per cell (CJK, emoji), so a
                // separately placed extension would float; draw the name as one run.
                stem.push_str(&ext);
                ext.clear();
            }
            let mut ink = if selected { color(p.ink_strong) } else { color(p.ink) };
            ink.a *= alpha;
            let mut ink_ext = color(p.ink_muted);
            ink_ext.a *= alpha;
            let name_clip = Rectangle { x: cols.name, y, width: cols.name_w, height: rh };
            let clip = name_clip.intersection(&body).unwrap_or(name_clip);
            let stem_cells: usize = stem.chars().map(cols_of).sum();
            for (content, x, c) in [(stem, cols.name, ink), (ext, cols.name + stem_cells as f32 * ADVANCE, ink_ext)] {
                if !content.is_empty() {
                    self.text(renderer, content, Point::new(x, cy), style::BODY, style::FONT, c, text::Alignment::Left, clip);
                }
            }
        }
        let mut buf = String::with_capacity(32);
        let meta = |renderer: &mut iced::Renderer, s: String, x: f32, right: bool, w: f32| {
            let clip = Rectangle { x: if right { x - w } else { x }, y, width: w, height: rh };
            self.text(renderer, s, Point::new(x, cy), style::META, style::FONT, color(p.ink_muted), if right { text::Alignment::Right } else { text::Alignment::Left }, clip.intersection(&body).unwrap_or(clip));
        };
        let stated = l.flags[i] & flags::STATED != 0;
        if l.is_dir(i) {
            buf.push('—');
        } else if stated {
            fmt::size(l.size[i], &mut buf);
        }
        meta(renderer, buf.clone(), cols.size_right, true, style::COL_SIZE);
        buf.clear();
        kinds::label(name, l.kind[i], &mut buf);
        if cols.show_kind {
            meta(renderer, buf.clone(), cols.kind, false, style::COL_KIND);
        }
        if stated && cols.show_date {
            buf.clear();
            self.dates.format(l.mtime[i], &mut buf);
            meta(renderer, buf, cols.date, false, style::COL_DATE);
        }
    }

    fn draw_tile(&self, renderer: &mut iced::Renderer, state: &State, body: Rectangle, pos: usize, hover: Option<usize>) {
        use iced::advanced::image::Renderer as _;
        use iced::advanced::svg::Renderer as _;
        let p = self.palette;
        let l = self.model.listing;
        let i = self.model.order[pos] as usize;
        let r = self.item_rect(state, body, pos);
        self.draw_ground(renderer, state, r, pos, i, hover);
        let alpha = self.alpha(i);
        let name = l.name_bytes(i);
        let pic = Rectangle { x: r.center_x() - THUMB / 2.0, y: r.y + 4.0, width: THUMB, height: THUMB };
        match (self.model.thumb)(i) {
            Some(handle) => {
                // Fit the thumbnail inside the 88px square, keeping its shape.
                let (w, h) = match renderer.measure_image(&handle) {
                    Some(sz) if sz.width > 0 && sz.height > 0 => {
                        let s = (THUMB / sz.width as f32).min(THUMB / sz.height as f32);
                        (sz.width as f32 * s, sz.height as f32 * s)
                    }
                    _ => (THUMB, THUMB),
                };
                let fit = Rectangle { x: pic.center_x() - w / 2.0, y: pic.y + THUMB - h, width: w, height: h };
                renderer.draw_image(Image { opacity: alpha, ..Image::new(handle) }, fit, body);
            }
            None => {
                let icon = Rectangle { x: pic.center_x() - 24.0, y: pic.center_y() - 24.0, width: 48.0, height: 48.0 };
                renderer.draw_svg(Svg { opacity: alpha, ..Svg::new(self.icons.color(kinds::icon(name, l.kind[i]))) }, icon, body);
                self.draw_badge(renderer, Rectangle { x: icon.x + 24.0, y: icon.y + 24.0, width: 24.0, height: 24.0 }, i, body);
            }
        }
        if self.model.renaming == Some(pos) {
            return;
        }
        let full = String::from_utf8_lossy(name);
        let max = ((r.width - 8.0) / (style::META * 0.6)).floor() as usize;
        let (a, b, _) = two_lines(&full, max);
        let mut ink = if is_selected(self.model.selected, i) { color(p.ink_strong) } else { color(p.ink) };
        ink.a *= alpha;
        let clip = r.intersection(&body).unwrap_or(r);
        self.text(renderer, a, Point::new(r.center_x(), pic.y + THUMB + 14.0), style::META, style::FONT, ink, text::Alignment::Center, clip);
        if !b.is_empty() {
            self.text(renderer, b, Point::new(r.center_x(), pic.y + THUMB + 30.0), style::META, style::FONT, ink, text::Alignment::Center, clip);
        }
    }

    /// Full name of a cut-off item after a short hover (design system `Tooltip`).
    fn draw_tooltip(&self, renderer: &mut iced::Renderer, state: &State, bounds: Rectangle, body: Rectangle) {
        use iced::advanced::Renderer as _;
        let Some((pos, since)) = state.hover else { return };
        if since.elapsed() < TIP_DELAY || pos >= self.model.order.len() || self.model.dragging {
            return;
        }
        let l = self.model.listing;
        let i = self.model.order[pos] as usize;
        let full = String::from_utf8_lossy(l.name_bytes(i)).into_owned();
        let cut = if self.model.grid {
            let r = self.item_rect(state, body, pos);
            two_lines(&full, ((r.width - 8.0) / (style::META * 0.6)).floor() as usize).2
        } else {
            fit_name(&full, l.is_dir(i), (columns(bounds).name_w / ADVANCE).floor() as usize).2
        };
        if !cut {
            return;
        }
        let p = self.palette;
        let r = self.item_rect(state, body, pos);
        let w: f32 = full.chars().map(cols_of).sum::<usize>() as f32 * (style::LABEL * 0.6) + 16.0;
        let w = w.min(bounds.width - 16.0);
        let x = if self.model.grid { r.center_x() - w / 2.0 } else { columns(bounds).name - 8.0 };
        let x = x.clamp(bounds.x + 4.0, bounds.x + bounds.width - w - 4.0);
        let y = r.y + r.height + 2.0;
        let y = if y + 22.0 > bounds.y + bounds.height { r.y - 24.0 } else { y };
        let bubble = Rectangle { x, y, width: w, height: 22.0 };
        renderer.with_layer(bounds, |renderer| {
            renderer.fill_quad(Quad { bounds: bubble, border: Border { color: color(p.line_strong), width: 1.0, radius: 2.0.into() }, ..Quad::default() }, color(p.bg_raised));
            self.text(renderer, full, Point::new(bubble.x + 8.0, bubble.center_y()), style::LABEL, style::FONT, color(p.ink), text::Alignment::Left, bubble);
        });
    }
}

impl<'a, Message: 'a> From<FileList<'a, Message>> for Element<'a, Message> {
    fn from(list: FileList<'a, Message>) -> Self {
        Element::new(list)
    }
}

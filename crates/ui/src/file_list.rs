//! The details view: ONE widget that draws only the visible rows (build plan §2.2, design
//! system `FileList`). Rows have a fixed height, so scrolling is pure arithmetic and a 100k
//! folder costs the same per frame as a 100-entry one.

use ef_core::fmt::{self, DateFormatter};
use ef_core::listing::flags;
use ef_core::{Listing, SortBy, SortSpec};
use ef_theme::Palette;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::svg::Svg;
use iced::advanced::text::{self, Text};
use iced::advanced::widget::{tree, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::alignment::Vertical;
use iced::keyboard::{self, key::Named, Key};
use iced::advanced::mouse::click::{self, Click};
use iced::mouse;
use iced::{Background, Border, Color, Element, Event, Length, Pixels, Point, Rectangle, Size};

use crate::kinds;
use crate::style::{self, color, Icons};

/// Rows drawn above and below the viewport so fast flicks never show a gap.
const OVERSCAN: usize = 3;
const SCROLLBAR: f32 = 6.0;

/// What the list reports to the app. Indices are display positions (into `order`).
#[derive(Debug, Clone)]
pub enum Action {
    Cursor { pos: usize, extend: bool, toggle: bool },
    Open(usize),
    SelectAll,
    Sort(SortBy),
}

/// Everything the list needs to draw one frame; owned by the app.
pub struct Model<'a> {
    pub listing: &'a Listing,
    pub order: &'a [u32],
    pub selected: &'a [u64],
    pub cursor: Option<usize>,
    /// Bumped on every navigation so the widget resets its scroll position.
    pub generation: u64,
    /// Phase B finished; until then sizes and dates are blank.
    #[allow(dead_code)]
    pub metadata_ready: bool,
    pub sort: SortSpec,
    /// Show placeholder rows instead of entries (a slow folder is still loading).
    pub skeleton: bool,
    /// Title and hint drawn when there is nothing to show.
    pub empty: Option<(String, String)>,
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
fn fit_name(name: &str, is_dir: bool, max: usize) -> (String, String) {
    let (stem, ext) = match name.rfind('.') {
        Some(p) if p > 0 && !is_dir && name.len() - p <= 8 => (&name[..p], &name[p..]),
        _ => (name, ""),
    };
    let width = |s: &str| s.chars().map(cols_of).sum::<usize>();
    if width(stem) + width(ext) <= max {
        return (stem.to_string(), ext.to_string());
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
    (out, ext.to_string())
}

pub fn is_selected(bits: &[u64], entry: usize) -> bool {
    bits.get(entry / 64).is_some_and(|w| w & (1 << (entry % 64)) != 0)
}

pub struct FileList<'a, Message> {
    model: Model<'a>,
    palette: &'a Palette,
    icons: &'a Icons,
    dates: &'a DateFormatter,
    on_action: Box<dyn Fn(Action) -> Message + 'a>,
}

#[derive(Default)]
struct State {
    offset: f32,
    generation: u64,
    last_cursor: Option<usize>,
    last_click: Option<Click>,
    modifiers: keyboard::Modifiers,
    viewport_rows: usize,
    dragging_thumb: Option<f32>,
}

impl<'a, Message> FileList<'a, Message> {
    pub fn new(
        model: Model<'a>,
        palette: &'a Palette,
        icons: &'a Icons,
        dates: &'a DateFormatter,
        on_action: impl Fn(Action) -> Message + 'a,
    ) -> Self {
        Self { model, palette, icons, dates, on_action: Box::new(on_action) }
    }

    fn content_height(&self) -> f32 {
        self.model.order.len() as f32 * style::ROW
    }

    fn max_offset(&self, viewport: f32) -> f32 {
        (self.content_height() - viewport).max(0.0)
    }

    fn row_at(&self, state: &State, body: Rectangle, p: Point) -> Option<usize> {
        if !body.contains(p) {
            return None;
        }
        let pos = ((p.y - body.y + state.offset) / style::ROW) as usize;
        (pos < self.model.order.len()).then_some(pos)
    }
}

/// Split the widget into header and body.
fn areas(bounds: Rectangle) -> (Rectangle, Rectangle) {
    let header = Rectangle { height: style::HEADER, ..bounds };
    let body = Rectangle { y: bounds.y + style::HEADER, height: (bounds.height - style::HEADER).max(0.0), ..bounds };
    (header, body)
}

struct Columns {
    icon: f32,
    name: f32,
    name_w: f32,
    size_right: f32,
    kind: f32,
    date: f32,
}

fn columns(b: Rectangle) -> Columns {
    let pad = style::SPACE_5;
    let right = b.x + b.width - pad - SCROLLBAR;
    let date = right - style::COL_DATE;
    let kind = date - style::SPACE_4 - style::COL_KIND;
    let size_right = kind - style::SPACE_4;
    let icon = b.x + pad;
    let name = icon + style::ICON_ROW + style::SPACE_3;
    let name_w = (size_right - style::COL_SIZE - style::SPACE_4 - name).max(40.0);
    Columns { icon, name, name_w, size_right, kind, date }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for FileList<'a, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let size = limits.max();
        let state = tree.state.downcast_mut::<State>();
        let body_h = (size.height - style::HEADER).max(0.0);
        state.viewport_rows = (body_h / style::ROW).floor().max(1.0) as usize;
        if state.generation != self.model.generation {
            state.generation = self.model.generation;
            state.offset = 0.0;
            state.last_cursor = None;
        }
        // Keep the keyboard cursor in view whenever it moves.
        if self.model.cursor != state.last_cursor {
            if let Some(c) = self.model.cursor {
                let top = c as f32 * style::ROW;
                if top < state.offset {
                    state.offset = top;
                } else if top + style::ROW > state.offset + body_h {
                    state.offset = top + style::ROW - body_h;
                }
            }
            state.last_cursor = self.model.cursor;
        }
        state.offset = state.offset.clamp(0.0, self.max_offset(body_h));
        layout::Node::new(size)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let (_, body) = areas(layout.bounds());
        let n = self.model.order.len();
        let max = self.max_offset(body.height);
        let publish = |shell: &mut Shell<'_, Message>, a: Action| shell.publish((self.on_action)(a));

        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => state.modifiers = *m,
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(body) => {
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * style::ROW * 3.0,
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };
                let next = (state.offset + dy).clamp(0.0, max);
                if next != state.offset {
                    state.offset = next;
                    shell.request_redraw();
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(p) = cursor.position() else { return };
                let (header, _) = areas(layout.bounds());
                if header.contains(p) {
                    let c = columns(layout.bounds());
                    let by = if p.x >= c.date {
                        SortBy::Modified
                    } else if p.x >= c.kind {
                        SortBy::Kind
                    } else if p.x >= c.size_right - style::COL_SIZE {
                        SortBy::Size
                    } else {
                        SortBy::Name
                    };
                    publish(shell, Action::Sort(by));
                    shell.capture_event();
                    return;
                }
                // Scrollbar thumb drag.
                if p.x >= body.x + body.width - SCROLLBAR * 2.0 && body.contains(p) && max > 0.0 {
                    state.dragging_thumb = Some(p.y);
                    shell.capture_event();
                    return;
                }
                if let Some(pos) = self.row_at(state, body, p) {
                    let click = Click::new(p, mouse::Button::Left, state.last_click);
                    state.last_click = Some(click);
                    if click.kind() == click::Kind::Double {
                        publish(shell, Action::Open(pos));
                    } else {
                        let m = state.modifiers;
                        publish(shell, Action::Cursor { pos, extend: m.shift(), toggle: m.control() });
                    }
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(from) = state.dragging_thumb {
                    let content = self.content_height().max(1.0);
                    let dy = (position.y - from) * content / body.height.max(1.0);
                    state.offset = (state.offset + dy).clamp(0.0, max);
                    state.dragging_thumb = Some(position.y);
                    shell.request_redraw();
                } else if cursor.is_over(body) {
                    // Hover highlight follows the pointer.
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => state.dragging_thumb = None,
            Event::Keyboard(keyboard::Event::KeyPressed { key: Key::Named(named), modifiers, .. }) if n > 0 => {
                let cur = self.model.cursor.unwrap_or(0);
                let page = state.viewport_rows.saturating_sub(1).max(1);
                let target = match named {
                    Named::ArrowDown => Some(if self.model.cursor.is_none() { 0 } else { (cur + 1).min(n - 1) }),
                    Named::ArrowUp => Some(cur.saturating_sub(1)),
                    Named::PageDown => Some((cur + page).min(n - 1)),
                    Named::PageUp => Some(cur.saturating_sub(page)),
                    Named::Home => Some(0),
                    Named::End => Some(n - 1),
                    Named::Enter if self.model.cursor.is_some() => {
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
            Event::Keyboard(keyboard::Event::KeyPressed { key: Key::Character(c), modifiers, .. })
                if modifiers.control() && c.as_str() == "a" =>
            {
                publish(shell, Action::SelectAll);
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::None
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use iced::advanced::svg::Renderer as _;
        use iced::advanced::text::Renderer as _;
        use iced::advanced::Renderer as _;

        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let (header, body) = areas(bounds);
        let p = self.palette;
        let cols = columns(bounds);

        renderer.fill_quad(Quad { bounds, ..Quad::default() }, color(p.bg));

        // ---- header
        let label = |renderer: &mut iced::Renderer, s: &str, x: f32, right: bool, c: Color| {
            renderer.fill_text(
                Text {
                    content: s.to_string(),
                    bounds: Size::new(200.0, style::HEADER),
                    size: Pixels(style::LABEL),
                    line_height: text::LineHeight::Absolute(Pixels(16.0)),
                    font: style::FONT_BOLD,
                    align_x: if right { text::Alignment::Right } else { text::Alignment::Left },
                    align_y: Vertical::Center,
                    shaping: text::Shaping::Basic,
                    wrapping: text::Wrapping::None,
                },
                Point::new(x, header.center_y()),
                c,
                header,
            );
        };
        let arrow = if self.model.sort.descending { " ↓" } else { " ↑" };
        for (by, title, x, right) in [
            (SortBy::Name, "NAME", cols.name, false),
            (SortBy::Size, "SIZE", cols.size_right, true),
            (SortBy::Kind, "KIND", cols.kind, false),
            (SortBy::Modified, "MODIFIED", cols.date, false),
        ] {
            let active = self.model.sort.by == by;
            let t = if active { format!("{title}{arrow}") } else { title.to_string() };
            let c = if active { color(p.ink_strong) } else { color(p.ink_muted) };
            label(renderer, &t, x, right, c);
        }
        renderer.fill_quad(
            Quad { bounds: Rectangle { y: header.y + header.height - 1.0, height: 1.0, ..header }, ..Quad::default() },
            color(p.line),
        );

        if self.model.skeleton {
            let rows = (body.height / style::ROW) as usize + 1;
            for r in 0..rows {
                let y = body.y + r as f32 * style::ROW + (style::ROW - 10.0) / 2.0;
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
            return;
        }
        if let Some((title, hint)) = &self.model.empty {
            let icon = Rectangle { x: body.center_x() - 32.0, y: body.y + body.height * 0.3 - 32.0, width: 64.0, height: 64.0 };
            renderer.draw_svg(Svg::new(self.icons.color("folder-open")), icon, body);
            for (content, size, font, c, dy) in [
                (title.clone(), 22.0, style::FONT_BOLD, color(p.ink_strong), 64.0),
                (hint.clone(), style::META, style::FONT, color(p.ink_muted), 96.0),
            ] {
                renderer.fill_text(
                    Text {
                        content,
                        bounds: Size::new(body.width, 30.0),
                        size: Pixels(size),
                        line_height: text::LineHeight::Relative(1.3),
                        font,
                        align_x: text::Alignment::Center,
                        align_y: Vertical::Center,
                        shaping: text::Shaping::Advanced,
                        wrapping: text::Wrapping::None,
                    },
                    Point::new(body.center_x(), icon.y + dy),
                    c,
                    body,
                );
            }
            return;
        }

        // ---- rows (visible slice only)
        let n = self.model.order.len();
        let first = (state.offset / style::ROW) as usize;
        let first = first.saturating_sub(OVERSCAN);
        let last = ((state.offset + body.height) / style::ROW).ceil() as usize + OVERSCAN;
        let last = last.min(n);
        let hover = cursor.position().and_then(|pt| self.row_at(state, body, pt));
        let l = self.model.listing;
        let mut buf = String::with_capacity(32);

        renderer.with_layer(body, |renderer| {
            for pos in first..last {
                let i = self.model.order[pos] as usize;
                let y = body.y + pos as f32 * style::ROW - state.offset;
                let row = Rectangle { x: bounds.x + style::SPACE_1, y, width: bounds.width - style::SPACE_1 * 2.0 - SCROLLBAR, height: style::ROW };
                let selected = is_selected(self.model.selected, i);
                let is_cursor = self.model.cursor == Some(pos);
                if selected || hover == Some(pos) || is_cursor {
                    let bg = if selected { color(p.selection) } else if hover == Some(pos) { color(p.state_hover) } else { Color::TRANSPARENT };
                    let border = if is_cursor {
                        Border { color: color(p.focus_ring), width: 1.0, radius: 2.0.into() }
                    } else {
                        Border { radius: 2.0.into(), ..Border::default() }
                    };
                    renderer.fill_quad(Quad { bounds: row, border, ..Quad::default() }, Background::Color(bg));
                }

                let name = l.name_bytes(i);
                let hidden = l.flags[i] & flags::HIDDEN != 0;
                let alpha = if hidden { 0.6 } else { 1.0 };
                let icon_box = Rectangle { x: cols.icon, y: y + (style::ROW - style::ICON_ROW) / 2.0, width: style::ICON_ROW, height: style::ICON_ROW };
                renderer.draw_svg(
                    Svg { opacity: alpha, ..Svg::new(self.icons.color(kinds::icon(name, l.kind[i]))) },
                    icon_box,
                    body,
                );

                let cy = y + style::ROW / 2.0;
                let full = String::from_utf8_lossy(name);
                let max_cells = (cols.name_w / ADVANCE).floor() as usize;
                let (stem, ext) = fit_name(&full, l.is_dir(i), max_cells);
                let shaping = if name.is_ascii() { text::Shaping::Basic } else { text::Shaping::Advanced };
                let mut ink = if selected { color(p.ink_strong) } else { color(p.ink) };
                ink.a *= alpha;
                let mut ink_ext = color(p.ink_muted);
                ink_ext.a *= alpha;
                let name_clip = Rectangle { x: cols.name, y, width: cols.name_w, height: style::ROW };
                let clip = name_clip.intersection(&body).unwrap_or(name_clip);
                let stem_cells: usize = stem.chars().map(cols_of).sum();
                for (content, x, c) in [(stem, cols.name, ink), (ext, cols.name + stem_cells as f32 * ADVANCE, ink_ext)] {
                    if content.is_empty() {
                        continue;
                    }
                    renderer.fill_text(
                        Text {
                            content,
                            bounds: Size::new(f32::INFINITY, style::ROW),
                            size: Pixels(style::BODY),
                            line_height: text::LineHeight::Absolute(Pixels(20.0)),
                            font: style::FONT,
                            align_x: text::Alignment::Left,
                            align_y: Vertical::Center,
                            shaping,
                            wrapping: text::Wrapping::None,
                        },
                        Point::new(x, cy),
                        c,
                        clip,
                    );
                }

                let meta = |renderer: &mut iced::Renderer, s: String, x: f32, right: bool, w: f32| {
                    let clip = Rectangle { x: if right { x - w } else { x }, y, width: w, height: style::ROW };
                    renderer.fill_text(
                        Text {
                            content: s,
                            bounds: Size::new(f32::INFINITY, style::ROW),
                            size: Pixels(style::META),
                            line_height: text::LineHeight::Absolute(Pixels(16.0)),
                            font: style::FONT,
                            align_x: if right { text::Alignment::Right } else { text::Alignment::Left },
                            align_y: Vertical::Center,
                            shaping: text::Shaping::Basic,
                            wrapping: text::Wrapping::None,
                        },
                        Point::new(x, cy),
                        color(p.ink_muted),
                        clip.intersection(&body).unwrap_or(clip),
                    );
                };
                let stated = l.flags[i] & flags::STATED != 0;
                buf.clear();
                if l.is_dir(i) {
                    buf.push('—');
                } else if stated {
                    fmt::size(l.size[i], &mut buf);
                }
                meta(renderer, buf.clone(), cols.size_right, true, style::COL_SIZE);
                buf.clear();
                kinds::label(name, l.kind[i], &mut buf);
                meta(renderer, buf.clone(), cols.kind, false, style::COL_KIND);
                if stated {
                    buf.clear();
                    self.dates.format(l.mtime[i], &mut buf);
                    meta(renderer, buf.clone(), cols.date, false, style::COL_DATE);
                }
            }
        });

        // ---- scrollbar
        let content = self.content_height();
        if content > body.height && body.height > 0.0 {
            let h = (body.height * body.height / content).max(24.0);
            let y = body.y + (body.height - h) * (state.offset / self.max_offset(body.height).max(1.0));
            let thumb = Rectangle { x: body.x + body.width - SCROLLBAR - 2.0, y, width: SCROLLBAR - 2.0, height: h };
            renderer.fill_quad(
                Quad { bounds: thumb, border: Border { radius: 3.0.into(), ..Border::default() }, ..Quad::default() },
                color(p.line_strong),
            );
        }

    }
}

impl<'a, Message: 'a> From<FileList<'a, Message>> for Element<'a, Message> {
    fn from(list: FileList<'a, Message>) -> Self {
        Element::new(list)
    }
}


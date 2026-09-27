//! Shared controls. Every clickable thing in EchoFiles comes from here, so hover, pressed,
//! active and disabled look the same everywhere (design system: States table):
//! hover = `state-hover` layer, pressed = `state-press`, active = `state-active` +
//! `accent-ink` glyph, disabled = 45% opacity and no hover.

use ef_theme::{Palette, Rgb, Semantic};
use iced::widget::{button, container, row, svg, text, text_input, toggler, Space};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::style::{self, color, Icons};

pub const DISABLED: f32 = 0.45;

fn faded(c: Color, on: bool) -> Color {
    if on { c } else { Color { a: c.a * DISABLED, ..c } }
}

pub fn glyph<'a, M: 'a>(icons: &Icons, name: &str, size: f32, tint: Color) -> Element<'a, M> {
    svg(icons.glyph(name)).width(size).height(size).style(move |_, _| svg::Style { color: Some(tint) }).into()
}

/// Background layer for a row/button in a given interaction state.
fn layer(p: &Palette, status: button::Status, active: bool) -> Option<Background> {
    match status {
        button::Status::Pressed => Some(Background::Color(color(p.state_press))),
        button::Status::Hovered => Some(Background::Color(color(if active { p.state_active } else { p.state_hover }))),
        button::Status::Disabled => None,
        button::Status::Active if active => Some(Background::Color(color(p.state_active))),
        button::Status::Active => None,
    }
}

/// 28px square glyph button (toolbar, panes). `msg: None` = disabled.
pub fn icon_button<'a, M: Clone + 'a>(p: &Palette, icons: &Icons, name: &str, msg: Option<M>, active: bool) -> Element<'a, M> {
    let enabled = msg.is_some();
    let tint = faded(if active { color(p.accent_ink) } else { color(p.ink_muted) }, enabled);
    let hover_tint = color(p.ink_strong);
    let pal = p.clone();
    let icon = svg(icons.glyph(name)).width(style::GLYPH).height(style::GLYPH).style(move |_, st| svg::Style {
        color: Some(if matches!(st, svg::Status::Hovered) && enabled && !active { hover_tint } else { tint }),
    });
    button(container(icon).center(28.0))
        .padding(0)
        .on_press_maybe(msg)
        .style(move |_, status| button::Style {
            background: layer(&pal, status, active),
            text_color: color(pal.ink),
            border: Border { radius: 4.0.into(), ..Border::default() },
            ..Default::default()
        })
        .into()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

/// Text button, 28px tall, optional leading glyph and trailing key hint.
pub fn text_button<'a, M: Clone + 'a>(p: &Palette, icons: &Icons, label: &str, icon: Option<&str>, kbd: Option<&str>, variant: Variant, msg: Option<M>) -> Element<'a, M> {
    let enabled = msg.is_some();
    let (fill, fg, edge) = match variant {
        Variant::Primary => (Some(color(p.accent)), color(p.on_accent), color(p.accent)),
        Variant::Danger => (Some(color(p.danger.base)), color(p.danger.on), color(p.danger.base)),
        Variant::Secondary => (None, color(p.ink), color(p.line_strong)),
        Variant::Ghost => (None, color(p.ink), Color::TRANSPARENT),
    };
    let fg = faded(fg, enabled);
    let mut content = row![].spacing(style::SPACE_3).align_y(Alignment::Center);
    if let Some(i) = icon {
        let t = if matches!(variant, Variant::Primary | Variant::Danger) { fg } else { faded(color(p.ink_muted), enabled) };
        content = content.push(glyph(icons, i, 14.0, t));
    }
    content = content.push(text(label.to_string()).size(style::BODY).font(style::FONT).color(fg).wrapping(text::Wrapping::None));
    if let Some(k) = kbd {
        content = content.push(kbd_chip(p, k));
    }
    let pal = p.clone();
    button(container(content).height(26).align_y(Alignment::Center))
        .padding([0, 12])
        .on_press_maybe(msg)
        .style(move |_, status| {
            let bg = match (fill, status) {
                (Some(c), button::Status::Hovered) => Some(Background::Color(Color { a: 0.88, ..c })),
                (Some(c), button::Status::Pressed) => Some(Background::Color(Color { a: 0.78, ..c })),
                (Some(c), button::Status::Disabled) => Some(Background::Color(Color { a: DISABLED, ..c })),
                (Some(c), _) => Some(Background::Color(c)),
                (None, s) => layer(&pal, s, false),
            };
            let edge = if status == button::Status::Disabled { Color { a: edge.a * DISABLED, ..edge } } else { edge };
            button::Style { background: bg, text_color: fg, border: Border { color: edge, width: 1.0, radius: 4.0.into() }, ..Default::default() }
        })
        .into()
}

/// A key hint chip (design system `Kbd`).
pub fn kbd_chip<'a, M: 'a>(p: &Palette, key: &str) -> Element<'a, M> {
    let (bg, edge) = (color(p.bg_deep), color(p.line_strong));
    container(text(key.to_string()).size(style::LABEL).font(style::FONT).color(color(p.ink_muted)))
        .padding([0, 5])
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border { color: edge, width: 1.0, radius: 2.0.into() },
            ..Default::default()
        })
        .into()
}

/// A full-width clickable row (sidebar item, settings nav, result row, drive).
pub fn row_button<'a, M: Clone + 'a>(p: &Palette, content: Element<'a, M>, active: bool, msg: Option<M>) -> Element<'a, M> {
    let pal = p.clone();
    button(content)
        .width(Length::Fill)
        .padding([0, 12])
        .on_press_maybe(msg)
        .style(move |_, status| button::Style {
            background: layer(&pal, status, active),
            text_color: color(pal.ink),
            border: Border { radius: 2.0.into(), ..Border::default() },
            ..Default::default()
        })
        .into()
}

/// Two to four exclusive options (design system `SegmentedControl`).
pub fn segmented<'a, M: Clone + 'a>(p: &Palette, options: &[(&str, M)], selected: usize) -> Element<'a, M> {
    let mut r = row![].spacing(2);
    for (i, (label, msg)) in options.iter().enumerate() {
        let on = i == selected;
        let pal = p.clone();
        let fg = if on { color(p.accent_ink) } else { color(p.ink_muted) };
        r = r.push(
            button(container(text(label.to_string()).size(style::META).font(style::FONT).color(fg).wrapping(text::Wrapping::None)).height(20).align_y(Alignment::Center))
                .padding([0, 10])
                .on_press(msg.clone())
                .style(move |_, status| button::Style {
                    background: if on {
                        Some(Background::Color(color(pal.bg_raised)))
                    } else {
                        layer(&pal, status, false)
                    },
                    text_color: fg,
                    border: Border { color: if on { color(pal.line) } else { Color::TRANSPARENT }, width: 1.0, radius: 2.0.into() },
                    ..Default::default()
                }),
        );
    }
    let (bg, edge) = (color(p.bg_deep), color(p.line_strong));
    container(r)
        .padding(2)
        .style(move |_| container::Style { background: Some(Background::Color(bg)), border: Border { color: edge, width: 1.0, radius: 4.0.into() }, ..Default::default() })
        .into()
}

/// On/off switch (design system `Switch`). `on_toggle: None` = disabled.
pub fn switch<'a, M: 'a>(p: &Palette, on: bool, on_toggle: Option<Box<dyn Fn(bool) -> M + 'a>>) -> Element<'a, M> {
    let pal = p.clone();
    let enabled = on_toggle.is_some();
    let mut t = toggler(on).size(18).style(move |_, status| {
        let hovered = matches!(status, toggler::Status::Hovered { .. });
        let a = if enabled { 1.0 } else { DISABLED };
        let track = if on { color(pal.accent) } else if hovered { color(pal.bg_raised) } else { color(pal.bg_deep) };
        toggler::Style {
            background: Background::Color(Color { a, ..track }),
            background_border_width: 1.0,
            background_border_color: Color { a, ..if on { color(pal.accent) } else { color(pal.line_strong) } },
            foreground: Background::Color(Color { a, ..if on { color(pal.on_accent) } else { color(pal.ink_muted) } }),
            foreground_border_width: 0.0,
            foreground_border_color: Color::TRANSPARENT,
            text_color: None,
            border_radius: None,
            padding_ratio: 0.18,
        }
    });
    if let Some(f) = on_toggle {
        t = t.on_toggle(f);
    }
    t.into()
}

/// Status word with a square mark (design system `StatePill`). `tone: None` = neutral.
pub fn pill<'a, M: 'a>(p: &Palette, label: &str, tone: Option<Semantic>) -> Element<'a, M> {
    let (bg, fg) = match tone {
        Some(t) => (color(t.soft), color(t.ink)),
        None => (color(p.state_hover), color(p.ink_muted)),
    };
    container(
        row![
            container(Space::new()).width(6).height(6).style(move |_| container::Style { background: Some(Background::Color(fg)), ..Default::default() }),
            text(label.to_string()).size(style::LABEL).font(style::FONT).color(fg).wrapping(text::Wrapping::None),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .padding([1, 6])
    .style(move |_| container::Style { background: Some(Background::Color(bg)), border: Border { radius: 2.0.into(), ..Border::default() }, ..Default::default() })
    .into()
}

pub fn hline<'a, M: 'a>(c: Rgb) -> Element<'a, M> {
    container(Space::new()).width(Length::Fill).height(1).style(move |_| container::Style { background: Some(Background::Color(color(c))), ..Default::default() }).into()
}

pub fn vline<'a, M: 'a>(c: Rgb) -> Element<'a, M> {
    container(Space::new()).width(1).height(Length::Fill).style(move |_| container::Style { background: Some(Background::Color(color(c))), ..Default::default() }).into()
}

/// 6px usage meter for drive cards and Properties.
pub fn usage_bar_large<'a, M: 'a>(p: &Palette, used: f32, fill: Rgb) -> Element<'a, M> {
    meter(p, used, fill, 6.0)
}

/// 3px usage meter; ≥90% warning, ≥97% danger.
pub fn usage_bar<'a, M: 'a>(p: &Palette, used: f32, fill: Rgb) -> Element<'a, M> {
    meter(p, used, fill, 3.0)
}

fn meter<'a, M: 'a>(p: &Palette, used: f32, fill: Rgb, h: f32) -> Element<'a, M> {
    let fill = if used >= 0.97 { color(p.danger.base) } else if used >= 0.9 { color(p.warning.base) } else { color(fill) };
    let track = color(p.line);
    let used = (used.clamp(0.0, 1.0) * 1000.0) as u16;
    row![
        container(Space::new()).width(Length::FillPortion(used.max(1))).height(h).style(move |_| container::Style {
            background: Some(Background::Color(fill)),
            border: Border { radius: 1.0.into(), ..Border::default() },
            ..Default::default()
        }),
        container(Space::new()).width(Length::FillPortion((1000 - used).max(1))).height(h).style(move |_| container::Style { background: Some(Background::Color(track)), ..Default::default() }),
    ]
    .into()
}

/// Linear progress (design system `TransferToast` bar): 4px, fed from atomics each frame.
/// `busy` draws a moving segment while the amount of work isn't known yet.
pub fn progress<'a, M: 'a>(p: &Palette, value: f32, fill: Rgb, busy: bool) -> Element<'a, M> {
    let (fill, track) = (color(fill), color(p.line));
    let bar = |w: u16, c: Color| -> Element<'a, M> {
        container(Space::new()).width(Length::FillPortion(w.max(1))).height(4).style(move |_| container::Style { background: Some(Background::Color(c)), ..Default::default() }).into()
    };
    if busy {
        let t = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0) % 1400) as u32;
        let lead = (t * 700 / 1400) as u16;
        return row![bar(lead.max(1), track), bar(300, fill), bar(700 - lead, track)].into();
    }
    let v = (value.clamp(0.0, 1.0) * 1000.0) as u16;
    row![bar(v, fill), bar(1000 - v, track)].into()
}

/// 16px box with its label as the hit target (design system `Checkbox`).
pub fn checkbox<'a, M: Clone + 'a>(p: &Palette, icons: &Icons, label: &str, checked: bool, on: impl Fn(bool) -> M + 'a) -> Element<'a, M> {
    let (accent, on_accent, edge) = (color(p.accent), color(p.on_accent), color(p.line_strong));
    let tick: Element<'a, M> = if checked { glyph(icons, "check", 12.0, on_accent) } else { Space::new().into() };
    let bx = container(tick).center(16).style(move |_| container::Style {
        background: checked.then_some(Background::Color(accent)),
        border: Border { color: if checked { accent } else { edge }, width: 1.0, radius: 2.0.into() },
        ..Default::default()
    });
    let r = row![bx, text(label.to_string()).size(style::BODY).font(style::FONT).color(color(p.ink))].spacing(style::SPACE_3).align_y(Alignment::Center);
    let pal = p.clone();
    button(r)
        .padding([2, 4])
        .on_press(on(!checked))
        .style(move |_, status| button::Style { background: layer(&pal, status, false), text_color: color(pal.ink), border: Border { radius: 2.0.into(), ..Border::default() }, ..Default::default() })
        .into()
}

/// Text field style on the `bg-deep` well; `invalid` draws the danger border.
pub fn field_style(p: &Palette, invalid: bool) -> impl Fn(&iced::Theme, text_input::Status) -> text_input::Style + use<> {
    let pal = p.clone();
    move |_, status| {
        let edge = match status {
            _ if invalid => color(pal.danger.base),
            text_input::Status::Focused { .. } => color(pal.focus_ring),
            text_input::Status::Hovered => color(pal.ink_muted),
            _ => color(pal.line_strong),
        };
        text_input::Style {
            background: Background::Color(color(pal.bg_deep)),
            border: Border { color: edge, width: 1.0, radius: 4.0.into() },
            icon: color(pal.ink_muted),
            placeholder: color(pal.ink_muted),
            value: color(pal.ink),
            selection: color(pal.accent_soft),
        }
    }
}

/// Plain container filled with a token.
pub fn fill<'a, M: 'a>(content: impl Into<Element<'a, M>>, c: Rgb) -> container::Container<'a, M> {
    let bg = color(c);
    container(content).style(move |_| container::Style { background: Some(Background::Color(bg)), ..Default::default() })
}

// ------------------------------------------------------------------------------ right-click

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{tree, Operation, Tree, Widget};
use iced::advanced::{mouse as amouse, overlay, renderer, Clipboard, Shell};
use iced::{mouse, Event, Point, Rectangle, Size, Vector};

/// Wraps content and reports right-clicks with the window position of the pointer, so the
/// context menu opens where the click was (`mouse_area` only says that a click happened).
pub struct ContextArea<'a, M> {
    content: Element<'a, M>,
    on_right: Box<dyn Fn(Point) -> M + 'a>,
}

pub fn context_area<'a, M: 'a>(content: impl Into<Element<'a, M>>, on_right: impl Fn(Point) -> M + 'a) -> Element<'a, M> {
    Element::new(ContextArea { content: content.into(), on_right: Box::new(on_right) })
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for ContextArea<'_, M> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operation: &mut dyn Operation) {
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: amouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(&mut tree.children[0], event, layout, cursor, renderer, clipboard, shell, viewport);
        if shell.is_event_captured() {
            return;
        }
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) = event && let Some(p) = cursor.position_over(layout.bounds()) {
            shell.publish((self.on_right)(p));
            shell.capture_event();
        }
    }

    fn mouse_interaction(&self, tree: &Tree, layout: Layout<'_>, cursor: amouse::Cursor, viewport: &Rectangle, renderer: &iced::Renderer) -> amouse::Interaction {
        self.content.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn draw(&self, tree: &Tree, renderer: &mut iced::Renderer, theme: &iced::Theme, style: &renderer::Style, layout: Layout<'_>, cursor: amouse::Cursor, viewport: &Rectangle) {
        self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

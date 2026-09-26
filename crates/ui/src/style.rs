//! Design-system tokens as Iced values: colours from the Omarchy-derived palette, plus the
//! fixed geometry from `design/tokens.json`.

use std::collections::HashMap;

use ef_theme::{Palette, Rgb};
use iced::advanced::svg;
use iced::{Color, Font};

pub fn color(c: Rgb) -> Color {
    Color { r: c.r, g: c.g, b: c.b, a: c.a }
}

/// Omarchy's default font; `omarchy-font-set` changes it system-wide.
pub const FONT: Font = Font::with_name("JetBrainsMono Nerd Font");
pub const FONT_BOLD: Font = Font { weight: iced::font::Weight::Bold, ..FONT };

pub const BODY: f32 = 13.0;
pub const META: f32 = 12.0;
pub const LABEL: f32 = 11.0;

pub const ROW: f32 = 28.0;
pub const HEADER: f32 = 26.0;
pub const TOOLBAR: f32 = 40.0;
pub const STATUSBAR: f32 = 24.0;
pub const SIDEBAR: f32 = 236.0;
pub const ICON_ROW: f32 = 18.0;
pub const GLYPH: f32 = 16.0;

pub const SPACE_1: f32 = 4.0;
pub const SPACE_3: f32 = 8.0;
pub const SPACE_4: f32 = 12.0;
pub const SPACE_5: f32 = 16.0;

/// Column widths of the details view (name takes the rest).
pub const COL_SIZE: f32 = 88.0;
pub const COL_KIND: f32 = 120.0;
pub const COL_DATE: f32 = 132.0;

/// Regular and bold files of `family`, found by file name only (no font parsing): the
/// family without spaces is the file-name prefix, e.g. `JetBrainsMonoNerdFont-Regular.ttf`.
pub fn find_font_files(family: &str) -> Vec<std::path::PathBuf> {
    let prefix = format!("{}-", family.replace(' ', ""));
    let mut dirs = vec![std::path::PathBuf::from("/usr/share/fonts")];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(std::path::PathBuf::from(home).join(".local/share/fonts"));
    }
    let mut found = Vec::new();
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                dirs.push(path);
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&prefix) && (name.contains("-Regular.") || name.contains("-Bold.")) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Theme-coloured SVG handles, rebuilt when the Omarchy theme changes.
pub struct Icons {
    color: HashMap<&'static str, svg::Handle>,
    glyph: HashMap<&'static str, svg::Handle>,
}

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/icons_gen.rs"));
}

impl Icons {
    pub fn new(palette: &Palette) -> Self {
        let color = embedded::COLOR
            .iter()
            .map(|(name, svg)| (*name, svg::Handle::from_memory(palette.recolor_icon(svg).into_bytes())))
            .collect();
        let glyph = embedded::GLYPHS
            .iter()
            .map(|(name, svg)| (*name, svg::Handle::from_memory(svg.as_bytes())))
            .collect();
        Self { color, glyph }
    }

    /// Colour icon by name, falling back to the generic file icon.
    pub fn color(&self, name: &str) -> svg::Handle {
        self.color.get(name).or_else(|| self.color.get("file")).cloned().expect("file icon")
    }

    /// Single-ink glyph; tint it with `svg::Svg::color`.
    pub fn glyph(&self, name: &str) -> svg::Handle {
        self.glyph.get(name).or_else(|| self.glyph.get("file")).cloned().expect("file glyph")
    }
}

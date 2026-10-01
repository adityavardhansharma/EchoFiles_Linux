//! Omarchy → EchoFiles colour derivation.
//!
//! Rust port of `design/build_tokens.py` (the reference implementation). Reads Omarchy's
//! `colors.toml` and derives every EchoFiles colour token with contrast guarantees.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// sRGB colour, 0–1 per channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgb {
    pub const WHITE: Rgb = Rgb { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Rgb = Rgb { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

    pub fn hex(s: &str) -> Option<Rgb> {
        let s = s.trim().trim_start_matches('#');
        if s.len() != 6 && s.len() != 8 {
            return None;
        }
        let ch = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok().map(|v| v as f32 / 255.0);
        Some(Rgb { r: ch(0)?, g: ch(2)?, b: ch(4)?, a: if s.len() == 8 { ch(6)? } else { 1.0 } })
    }

    /// Hex with each channel rounded like the Python reference, for parity tests.
    pub fn to_hex(self) -> String {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", q(self.r), q(self.g), q(self.b))
    }

    /// `t = 0` → self, `t = 1` → other (sRGB), quantised to 8 bits.
    pub fn mix(self, other: Rgb, t: f64) -> Rgb {
        // f64 + round-half-to-even mirror the Python reference exactly.
        let ch = |a: f32, b: f32| {
            let (a, b) = ((a * 255.0).round() as f64 / 255.0, (b * 255.0).round() as f64 / 255.0);
            let v = (a + (b - a) * t).clamp(0.0, 1.0);
            ((v * 255.0).round_ties_even() / 255.0) as f32
        };
        Rgb { r: ch(self.r, other.r), g: ch(self.g, other.g), b: ch(self.b, other.b), a: 1.0 }
    }

    pub fn with_alpha(self, a: f32) -> Rgb {
        Rgb { a, ..self }
    }

    pub fn luminance(self) -> f64 {
        let ch = |v: f32| {
            let v = (v * 255.0).round() as f64 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(self.r) + 0.7152 * ch(self.g) + 0.0722 * ch(self.b)
    }

    /// WCAG 2 contrast ratio.
    pub fn contrast(self, other: Rgb) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

/// Move `color` toward `toward` until it reaches `target` contrast on every ground.
fn ensure(color: Rgb, grounds: &[Rgb], target: f64, toward: Rgb) -> Rgb {
    for i in 0..=100 {
        let c = color.mix(toward, i as f64 / 100.0);
        if grounds.iter().all(|g| c.contrast(*g) >= target) {
            return c;
        }
    }
    toward
}

fn best_on(fill: Rgb, candidates: &[Rgb]) -> Rgb {
    *candidates
        .iter()
        .max_by(|a, b| a.contrast(fill).total_cmp(&b.contrast(fill)))
        .expect("candidates")
}

/// Every colour token of the EchoFiles design system (names match `design/tokens.json`).
#[derive(Clone, Debug)]
pub struct Palette {
    pub name: String,
    pub dark: bool,
    pub bg: Rgb,
    pub bg_sunken: Rgb,
    pub bg_deep: Rgb,
    pub bg_raised: Rgb,
    pub line: Rgb,
    pub line_strong: Rgb,
    pub ink: Rgb,
    pub ink_strong: Rgb,
    pub ink_muted: Rgb,
    pub ink_faint: Rgb,
    pub accent: Rgb,
    pub accent_ink: Rgb,
    pub on_accent: Rgb,
    pub accent_soft: Rgb,
    pub selection: Rgb,
    pub focus_ring: Rgb,
    pub state_hover: Rgb,
    pub state_active: Rgb,
    pub state_press: Rgb,
    pub scrim: Rgb,
    pub success: Semantic,
    pub warning: Semantic,
    pub danger: Semantic,
    pub info: Semantic,
    pub world_linux: Rgb,
    pub world_windows: Rgb,
    pub world_phone: Rgb,
    /// Network section mark (SMB, SFTP, FTP places).
    pub world_network: Rgb,
    /// Colour-icon slot values, keyed by slot name (`folder`, `blue`, …).
    pub icon_slots: Vec<(&'static str, Rgb)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Semantic {
    pub base: Rgb,
    pub soft: Rgb,
    pub ink: Rgb,
    pub on: Rgb,
}

/// Default ("Echo") slot hexes the colour SVGs are authored with (see `design/build_icons.py`).
pub const ICON_SLOT_DEFAULTS: [(&str, &str); 15] = [
    ("folder", "#FFC83D"),
    ("folder-back", "#F2AE1C"),
    ("folder-glyph", "#7A5306"),
    ("blue", "#2F7CF6"),
    ("blue-deep", "#1B5FD1"),
    ("green", "#22A55B"),
    ("red", "#EF4444"),
    ("orange", "#FB8A2E"),
    ("purple", "#7B4FF0"),
    ("yellow", "#F7C331"),
    ("slate", "#4B5565"),
    ("slate-deep", "#2E3440"),
    ("paper", "#E6EBF2"),
    ("paper-fold", "#BFC8D6"),
    ("on", "#FFFFFF"),
];

/// The brand theme used outside Omarchy.
pub const ECHO_COLORS: &str = r##"
mode = "dark"
accent = "#4cc3ff"
selection = "#1b2656"
muted = "#2a3566"
background = "#0b1030"
dark_background = "#080c26"
darker_background = "#050819"
lighter_background = "#131a45"
foreground = "#c9d3f5"
dark_foreground = "#5d6799"
bright_foreground = "#eef2ff"
red = "#ff6b81"
yellow = "#ffc857"
orange = "#ff8a3d"
green = "#5ee6a0"
cyan = "#3dd9f5"
blue = "#4c8dff"
magenta = "#a56bff"
"##;

/// Where Omarchy keeps the active theme.
pub fn omarchy_theme_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".local/state/omarchy/current/theme"))
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Toml(toml::de::Error),
    Missing(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "couldn't read colors.toml: {e}"),
            Error::Toml(e) => write!(f, "colors.toml isn't valid TOML: {e}"),
            Error::Missing(k) => write!(f, "colors.toml has no `{k}` colour"),
        }
    }
}

impl std::error::Error for Error {}

/// Load the active Omarchy theme, falling back to the Echo brand theme.
pub fn load_active() -> Palette {
    omarchy_theme_dir()
        .and_then(|dir| {
            let name = std::fs::read_to_string(dir.with_file_name("theme.name"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "omarchy".into());
            load(&dir.join("colors.toml"), &name).ok()
        })
        .unwrap_or_else(|| derive("echo", ECHO_COLORS).expect("built-in theme"))
}

pub fn load(path: &Path, name: &str) -> Result<Palette, Error> {
    let text = std::fs::read_to_string(path).map_err(Error::Io)?;
    derive(name, &text)
}

/// Derive the full palette from the text of an Omarchy `colors.toml`.
pub fn derive(name: &str, colors_toml: &str) -> Result<Palette, Error> {
    let table: HashMap<String, toml::Value> = toml::from_str(colors_toml).map_err(Error::Toml)?;
    let get = |k: &str| table.get(k).and_then(|v| v.as_str()).and_then(Rgb::hex);
    let need = |k: &'static str| get(k).ok_or(Error::Missing(k));

    let dark = table.get("mode").and_then(|v| v.as_str()).unwrap_or("dark") == "dark";
    let bg = need("background")?;
    let sunken = get("dark_background").unwrap_or_else(|| bg.mix(Rgb::BLACK, 0.2));
    let deep = get("darker_background").unwrap_or_else(|| bg.mix(Rgb::BLACK, 0.35));
    let raised = if dark {
        get("lighter_background").unwrap_or_else(|| bg.mix(Rgb::WHITE, 0.06))
    } else {
        bg.mix(Rgb::WHITE, 0.6)
    };
    let fg = need("foreground")?;
    let extreme = if dark { Rgb::WHITE } else { Rgb::BLACK };
    let accent = need("accent")?;

    let mut selection = get("selection").unwrap_or_else(|| accent.mix(bg, 0.75));
    let with_sel = |sel: Rgb| [bg, sunken, deep, raised, sel];
    let ink = ensure(fg, &with_sel(selection), 7.0, extreme);
    if ink.contrast(selection) < 7.0 {
        selection = ensure(selection, &[ink], 7.0, bg);
    }
    let grounds = with_sel(selection);
    let ink_strong = ensure(get("bright_foreground").unwrap_or(fg), &grounds, 7.0, extreme);
    let ink_muted = ensure(fg.mix(bg, 0.45), &grounds, 4.5, ink);
    let ink_faint = get("dark_foreground").unwrap_or_else(|| fg.mix(bg, 0.6));

    let semantic = |key: &'static str| -> Result<Semantic, Error> {
        let c = need(key)?;
        let soft = bg.mix(c, 0.14);
        Ok(Semantic {
            base: c,
            soft,
            ink: ensure(c, &[bg, raised, soft], 4.5, extreme),
            on: best_on(c, &[deep, Rgb::WHITE, Rgb::BLACK]),
        })
    };
    let world = |key: &'static str| -> Result<Rgb, Error> { Ok(ensure(need(key)?, &[sunken, bg, raised], 3.0, extreme)) };

    let yellow = need("yellow")?;
    let blue = need("blue")?;
    let icon_slots = vec![
        ("folder", yellow),
        ("folder-back", yellow.mix(deep, 0.16)),
        ("folder-glyph", yellow.mix(Rgb::BLACK, 0.62)),
        ("blue", blue),
        ("blue-deep", blue.mix(Rgb::BLACK, 0.25)),
        ("green", need("green")?),
        ("red", need("red")?),
        ("orange", get("orange").unwrap_or(yellow)),
        ("purple", need("magenta")?),
        ("yellow", yellow),
        ("slate", if dark { fg.mix(bg, 0.35) } else { fg.mix(bg, 0.15) }),
        ("slate-deep", if dark { fg.mix(bg, 0.62) } else { fg }),
        ("paper", if dark { fg.mix(bg, 0.12) } else { bg.mix(fg, 0.1) }),
        ("paper-fold", if dark { fg.mix(bg, 0.45) } else { bg.mix(fg, 0.3) }),
        ("on", if dark { fg.mix(Rgb::WHITE, 0.7) } else { Rgb::WHITE }),
    ];

    Ok(Palette {
        name: name.to_string(),
        dark,
        bg,
        bg_sunken: sunken,
        bg_deep: deep,
        bg_raised: raised,
        line: bg.mix(fg, if dark { 0.14 } else { 0.16 }),
        line_strong: ensure(bg.mix(fg, 0.3), &[bg, sunken, raised], 3.0, ink),
        ink,
        ink_strong,
        ink_muted,
        ink_faint,
        accent,
        accent_ink: ensure(accent, &grounds, 4.5, extreme),
        on_accent: best_on(accent, &[deep, Rgb::WHITE, Rgb::BLACK]),
        accent_soft: bg.mix(accent, 0.16),
        selection,
        focus_ring: ensure(accent, &grounds, 3.0, extreme),
        state_hover: fg.with_alpha(0.08),
        state_active: fg.with_alpha(0.18),
        state_press: fg.with_alpha(0.22),
        scrim: deep.with_alpha(0.55),
        success: semantic("green")?,
        warning: semantic("yellow")?,
        danger: semantic("red")?,
        info: semantic("cyan")?,
        world_linux: world("orange").or_else(|_| world("yellow"))?,
        world_windows: world("blue")?,
        world_phone: world("green")?,
        world_network: world("magenta")?,
        icon_slots,
    })
}

impl Palette {
    /// Recolour a colour-icon SVG authored with the default slot hexes.
    pub fn recolor_icon(&self, svg: &str) -> String {
        let mut out = svg.to_string();
        for (slot, default_hex) in ICON_SLOT_DEFAULTS {
            if let Some((_, c)) = self.icon_slots.iter().find(|(s, _)| *s == slot) {
                out = out.replace(default_hex, &c.to_hex());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parity with design/tokens.json (Python reference) for the Echo and Tokyo Night themes.
    #[test]
    fn matches_reference_derivation() {
        let p = derive("echo", ECHO_COLORS).unwrap();
        assert_eq!(p.ink_muted.to_hex(), "#878fb0");
        assert_eq!(p.line_strong.to_hex(), "#606788");
        assert_eq!(p.accent_soft.to_hex(), "#152d51");

        let tokyo = r##"mode = "dark"
accent = "#7aa2f7"
selection = "#292e42"
background = "#1a1b26"
dark_background = "#13141c"
darker_background = "#0e0e14"
lighter_background = "#24283b"
foreground = "#a9b1d6"
dark_foreground = "#565f89"
bright_foreground = "#c0caf5"
red = "#f7768e"
yellow = "#e0af68"
orange = "#eb927b"
green = "#9ece6a"
cyan = "#449dab"
blue = "#7aa2f7"
magenta = "#ad8ee6""##;
        let t = derive("tokyo-night", tokyo).unwrap();
        assert_eq!(t.ink.to_hex(), "#b3badb");
        assert_eq!(t.ink_muted.to_hex(), "#8f95b2");
        assert!(t.ink_muted.contrast(t.selection) >= 4.5);
    }

    #[test]
    fn every_omarchy_theme_meets_contrast() {
        let Ok(dir) = std::fs::read_dir("/usr/share/omarchy/themes") else { return };
        for entry in dir.flatten() {
            let path = entry.path().join("colors.toml");
            let Ok(p) = load(&path, &entry.file_name().to_string_lossy()) else { continue };
            for g in [p.bg, p.bg_sunken, p.bg_raised, p.selection] {
                assert!(p.ink.contrast(g) >= 7.0, "{}: ink", p.name);
                assert!(p.ink_muted.contrast(g) >= 4.5, "{}: ink-muted", p.name);
                assert!(p.focus_ring.contrast(g) >= 3.0, "{}: focus ring", p.name);
            }
        }
    }
}

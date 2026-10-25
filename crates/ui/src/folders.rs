//! What EchoFiles remembers per folder: sort order, icon size, a colour and an icon.
//! Kept in `~/.local/state/echofiles/folders`, one folder per line:
//! `sort=size:desc;zoom=3;color=green;icon=folder-music<TAB>/home/me/Music`. In the path,
//! `%`, control characters (a newline would split the record) and bytes that aren't UTF-8
//! are written as `%XX`, so every Linux path survives.
//! The layout (list, grid, columns) stays in `view-modes`, as before.

use std::collections::HashMap;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use ef_config as config;
use ef_core::sort::{SortBy, SortSpec};

/// Icon sizes, smallest first. Grid tiles show a picture this big; list rows scale their
/// icons with it.
pub const ZOOMS: [f32; 5] = [56.0, 72.0, 88.0, 112.0, 128.0];
pub const DEFAULT_ZOOM: u8 = 2;

/// Folder colours: (name, label, front, back). Front and back are the two shades the colour
/// folder icon is drawn with.
pub const COLORS: [(&str, &str, &str, &str); 8] = [
    ("red", "Red", "#F0656A", "#D5474C"),
    ("orange", "Orange", "#FF9F45", "#E8812A"),
    ("green", "Green", "#5CCB7A", "#3FAE5E"),
    ("teal", "Teal", "#3FC7C2", "#27A7A3"),
    ("blue", "Blue", "#5B9BFF", "#3E7DE6"),
    ("purple", "Purple", "#A982F5", "#8C63DE"),
    ("pink", "Pink", "#F178B6", "#D85B9A"),
    ("gray", "Gray", "#A3A9B8", "#858B9A"),
];

/// Folder icons offered in the menu: (colour icon name, label).
pub const ICONS: [(&str, &str); 14] = [
    ("folder-heart", "Favourite"),
    ("folder-image", "Pictures"),
    ("folder-video", "Videos"),
    ("folder-music", "Music"),
    ("folder-download", "Downloads"),
    ("folder-user", "Personal"),
    ("folder-users", "Shared with people"),
    ("folder-lock", "Private"),
    ("folder-settings", "Settings"),
    ("folder-clock", "Archive"),
    ("folder-share", "Shared"),
    ("folder-search", "Research"),
    ("folder-link", "Linked"),
    ("folder-plus", "Inbox"),
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prefs {
    pub sort: Option<SortSpec>,
    pub zoom: Option<u8>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

impl Prefs {
    fn is_empty(&self) -> bool {
        *self == Prefs::default()
    }

    fn encode(&self) -> String {
        let mut parts = Vec::new();
        if let Some(s) = self.sort {
            let by = match s.by {
                SortBy::Name => "name",
                SortBy::Size => "size",
                SortBy::Modified => "modified",
                SortBy::Kind => "kind",
            };
            parts.push(format!("sort={by}:{}", if s.descending { "desc" } else { "asc" }));
        }
        if let Some(z) = self.zoom {
            parts.push(format!("zoom={z}"));
        }
        if let Some(c) = &self.color {
            parts.push(format!("color={c}"));
        }
        if let Some(i) = &self.icon {
            parts.push(format!("icon={i}"));
        }
        parts.join(";")
    }

    fn decode(s: &str) -> Prefs {
        let mut p = Prefs::default();
        for part in s.split(';') {
            let Some((k, v)) = part.split_once('=') else { continue };
            match k {
                "sort" => {
                    let (by, dir) = v.split_once(':').unwrap_or((v, "asc"));
                    let by = match by {
                        "name" => SortBy::Name,
                        "size" => SortBy::Size,
                        "modified" => SortBy::Modified,
                        "kind" => SortBy::Kind,
                        _ => continue,
                    };
                    p.sort = Some(SortSpec { by, descending: dir == "desc", ..SortSpec::default() });
                }
                "zoom" => p.zoom = v.parse::<u8>().ok().filter(|z| (*z as usize) < ZOOMS.len()),
                "color" if COLORS.iter().any(|c| c.0 == v) => p.color = Some(v.to_string()),
                "icon" if ICONS.iter().any(|i| i.0 == v) => p.icon = Some(v.to_string()),
                _ => {}
            }
        }
        p
    }
}

#[derive(Default)]
pub struct Folders {
    map: HashMap<PathBuf, Prefs>,
}

fn escape(path: &Path) -> String {
    let mut out = String::new();
    for chunk in path.as_os_str().as_bytes().utf8_chunks() {
        for c in chunk.valid().chars() {
            if c == '%' || c.is_control() {
                let mut buf = [0; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            } else {
                out.push(c);
            }
        }
        for b in chunk.invalid() {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn unescape(s: &str) -> PathBuf {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && let Some(v) = b.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok()) {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    PathBuf::from(OsString::from_vec(out))
}

fn file() -> PathBuf {
    config::state_dir().join("folders")
}

impl Folders {
    pub fn load() -> Folders {
        Folders::parse(&std::fs::read_to_string(file()).unwrap_or_default())
    }

    fn parse(text: &str) -> Folders {
        let map = text
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(prefs, path)| (unescape(path), Prefs::decode(prefs)))
            .filter(|(_, p)| !p.is_empty())
            .collect();
        Folders { map }
    }

    fn text(&self) -> String {
        let mut lines: Vec<String> = self.map.iter().map(|(path, p)| format!("{}\t{}\n", p.encode(), escape(path))).collect();
        lines.sort();
        lines.concat()
    }

    pub fn get(&self, dir: &Path) -> Option<&Prefs> {
        self.map.get(dir)
    }

    /// Change one folder's preferences and save in the background.
    pub fn update(&mut self, dir: &Path, change: impl FnOnce(&mut Prefs)) {
        self.edit(dir, change);
        self.save();
    }

    fn edit(&mut self, dir: &Path, change: impl FnOnce(&mut Prefs)) {
        let p = self.map.entry(dir.to_path_buf()).or_default();
        change(p);
        if p.is_empty() {
            self.map.remove(dir);
        }
    }

    fn save(&self) {
        config::queue_state(file(), self.text().into_bytes());
    }

    pub fn sort(&self, dir: &Path) -> Option<SortSpec> {
        self.get(dir).and_then(|p| p.sort)
    }

    pub fn zoom(&self, dir: &Path) -> u8 {
        self.get(dir).and_then(|p| p.zoom).unwrap_or(DEFAULT_ZOOM)
    }

    /// The colour icon to draw for a folder, and its colour, when either was chosen.
    pub fn decor(&self, dir: &Path) -> Option<(&str, Option<&str>)> {
        let p = self.get(dir)?;
        if p.color.is_none() && p.icon.is_none() {
            return None;
        }
        Some((p.icon.as_deref().unwrap_or("folder"), p.color.as_deref()))
    }

    pub fn has_decor(&self) -> bool {
        self.map.values().any(|p| p.color.is_some() || p.icon.is_some())
    }

    /// Keep preferences with a folder that was renamed (and with what's inside it).
    pub fn moved(&mut self, from: &Path, to: &Path) {
        if self.rebase(from, to) {
            self.save();
        }
    }

    fn rebase(&mut self, from: &Path, to: &Path) -> bool {
        let keys: Vec<PathBuf> = self.map.keys().filter(|k| k.starts_with(from)).cloned().collect();
        for k in &keys {
            if let (Some(p), Ok(rest)) = (self.map.remove(k), k.strip_prefix(from)) {
                let dest = if rest.as_os_str().is_empty() { to.to_path_buf() } else { to.join(rest) };
                self.map.insert(dest, p);
            }
        }
        !keys.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_ignores_unknown_values() {
        let mut f = Folders::default();
        let music = PathBuf::from("/home/me/Music");
        f.map.insert(music.clone(), Prefs { sort: Some(SortSpec { by: SortBy::Size, descending: true, ..SortSpec::default() }), zoom: Some(3), color: Some("green".into()), icon: Some("folder-music".into()) });
        f.map.insert(PathBuf::from("/tmp/a b"), Prefs { zoom: Some(0), ..Prefs::default() });
        // Names no line-per-folder text could hold as they are: a newline, a tab, a literal
        // `%41`, and bytes that aren't UTF-8.
        f.map.insert(PathBuf::from("/tmp/two\nlines\tand %41 é"), Prefs { zoom: Some(1), ..Prefs::default() });
        f.map.insert(PathBuf::from(OsString::from_vec(b"/tmp/\xff\xferaw".to_vec())), Prefs { zoom: Some(4), ..Prefs::default() });
        let back = Folders::parse(&f.text());
        assert_eq!(back.map, f.map);
        assert_eq!(back.decor(&music), Some(("folder-music", Some("green"))));
        assert_eq!(back.zoom(Path::new("/elsewhere")), DEFAULT_ZOOM);
        let odd = Folders::parse("color=plaid;zoom=99;icon=../../x;sort=size:desc\t/x\n\t/empty\n");
        assert_eq!(odd.get(Path::new("/x")).unwrap(), &Prefs { sort: Some(SortSpec { by: SortBy::Size, descending: true, ..SortSpec::default() }), ..Prefs::default() });
        assert!(odd.get(Path::new("/empty")).is_none());
    }

    #[test]
    fn colours_parse_and_preferences_follow_a_move() {
        assert!(COLORS.iter().all(|c| u32::from_str_radix(&c.2[1..], 16).is_ok() && u32::from_str_radix(&c.3[1..], 16).is_ok()));
        let mut f = Folders::default();
        f.map.insert(PathBuf::from("/a/old"), Prefs { color: Some("red".into()), ..Prefs::default() });
        f.map.insert(PathBuf::from("/a/old/inner"), Prefs { zoom: Some(1), ..Prefs::default() });
        f.map.insert(PathBuf::from("/a/older"), Prefs { zoom: Some(4), ..Prefs::default() });
        assert!(f.rebase(Path::new("/a/old"), Path::new("/b/new")));
        assert!(f.get(Path::new("/b/new")).is_some_and(|p| p.color.as_deref() == Some("red")));
        assert_eq!(f.zoom(Path::new("/b/new/inner")), 1);
        assert_eq!(f.zoom(Path::new("/a/older")), 4);
    }
}

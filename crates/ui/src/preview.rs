//! The preview pane — Space toggles it, Alt+Enter (Properties) opens it (design system
//! `PreviewPane`): a large preview, every fact about the item, Windows attributes on NTFS,
//! and the first lines of text files. Folder sizes are counted in the background and fill
//! in while you watch.

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ef_config as config;
use ef_core::fmt;
use ef_core::listing::win;
use ef_core::ops::DirSize;
use iced::widget::{column, container, image, row, scrollable, text, Space};
use iced::{Alignment, Background, Border, Element, Length};

use crate::actions::FileMsg;
use crate::app::{App, Message};
use crate::style::{self, color};
use crate::widgets::{self as w, Variant};

pub const WIDTH: f32 = 280.0;

/// Everything shown about one item, gathered off the UI thread.
#[derive(Debug)]
pub struct PreviewData {
    pub name: String,
    pub is_dir: bool,
    pub kind: String,
    pub size: u64,
    pub modified: i64,
    pub accessed: i64,
    pub created: Option<i64>,
    pub mode: u32,
    pub owner: String,
    pub group: String,
    pub link: Option<PathBuf>,
    pub broken: bool,
    /// Entries directly inside a folder.
    pub items: Option<usize>,
    /// Whole-tree size of a folder, counted in the background.
    pub tree: Option<Arc<DirSize>>,
    pub text: Option<String>,
    pub attrs: Option<u32>,
    pub error: Option<String>,
    /// Stops the folder-size walk when the pane moves on.
    pub cancel: Arc<AtomicBool>,
}

fn name_of(file: &str, id: u32) -> String {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| t.lines().find_map(|l| {
            let mut f = l.split(':');
            let n = f.next()?;
            let _ = f.next();
            (f.next()?.parse::<u32>().ok()? == id).then(|| n.to_string())
        }))
        .unwrap_or_else(|| id.to_string())
}

/// `rwxr-xr-x`
pub fn perms(mode: u32) -> String {
    let mut s = String::with_capacity(9);
    for shift in [6, 3, 0] {
        let b = (mode >> shift) & 7;
        s.push(if b & 4 != 0 { 'r' } else { '-' });
        s.push(if b & 2 != 0 { 'w' } else { '-' });
        s.push(if b & 1 != 0 { 'x' } else { '-' });
    }
    s
}

/// Read up to 200 lines of a file that looks like text.
fn text_head(p: &Path) -> Option<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p).ok()?;
    let mut buf = vec![0u8; 64 * 1024];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    if buf.iter().take(8192).any(|&b| b == 0) {
        return None;
    }
    let s = String::from_utf8_lossy(&buf);
    if s.chars().take(4096).filter(|c| *c == '\u{FFFD}').count() > 8 {
        return None;
    }
    Some(s.lines().take(200).map(|l| l.chars().take(200).collect::<String>()).collect::<Vec<_>>().join("\n"))
}

/// Collect the facts for `p` (blocking; runs on a worker thread).
pub fn gather(p: &Path) -> PreviewData {
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string());
    let lm = match std::fs::symlink_metadata(p) {
        Ok(m) => m,
        Err(e) => {
            return PreviewData {
                name,
                is_dir: false,
                kind: String::new(),
                size: 0,
                modified: 0,
                accessed: 0,
                created: None,
                mode: 0,
                owner: String::new(),
                group: String::new(),
                link: None,
                broken: false,
                items: None,
                tree: None,
                text: None,
                attrs: None,
                error: Some(e.to_string()),
                cancel: Arc::default(),
            }
        }
    };
    let link = lm.file_type().is_symlink().then(|| std::fs::read_link(p).ok()).flatten();
    let target = std::fs::metadata(p).ok();
    let m = target.clone().unwrap_or_else(|| lm.clone());
    let is_dir = m.is_dir();
    let mut kind = String::new();
    crate::kinds::label(name.as_bytes(), if is_dir { ef_core::Kind::Dir } else { ef_core::Kind::File }, &mut kind);
    if link.is_some() {
        kind = format!("Link to {}", kind.to_lowercase());
    }
    let created = rustix::fs::statx(rustix::fs::CWD, p, rustix::fs::AtFlags::empty(), rustix::fs::StatxFlags::BTIME)
        .ok()
        .filter(|s| s.stx_mask & rustix::fs::StatxFlags::BTIME.bits() != 0)
        .map(|s| s.stx_btime.tv_sec);
    let items = is_dir.then(|| std::fs::read_dir(p).map(|r| r.count()).unwrap_or(0));
    let cancel: Arc<AtomicBool> = Arc::default();
    let tree = is_dir.then(|| {
        let size = Arc::new(DirSize::default());
        let (s2, root, c2) = (size.clone(), p.to_path_buf(), cancel.clone());
        std::thread::spawn(move || ef_core::ops::measure(&[root], &s2, &c2));
        size
    });
    let mut attrs = None;
    if crate::app::fs_of(p).is_some_and(|f| f.fs_type.starts_with("ntfs")) {
        let mut buf = [0u8; 4];
        if let Ok(4) = rustix::fs::lgetxattr(p, "system.ntfs_attrib", &mut buf) {
            attrs = Some(u32::from_le_bytes(buf));
        }
    }
    PreviewData {
        name,
        is_dir,
        kind,
        size: m.len(),
        modified: m.mtime(),
        accessed: m.atime(),
        created,
        mode: m.permissions().mode(),
        owner: name_of("/etc/passwd", m.uid()),
        group: name_of("/etc/group", m.gid()),
        link,
        broken: target.is_none() && lm.file_type().is_symlink(),
        items,
        tree,
        text: (!is_dir && m.len() > 0 && !crate::thumbs::thumbnailable(p)).then(|| text_head(p)).flatten(),
        attrs,
        error: None,
        cancel,
    }
}

impl App {
    pub(crate) fn preview_counting(&self) -> bool {
        self.preview.as_ref().is_some_and(|(_, d)| d.tree.as_ref().is_some_and(|t| !t.done.load(Ordering::Relaxed)))
    }

    fn prop<'a>(&self, label: &str, value: String) -> Element<'a, Message> {
        let p = &self.palette;
        row![
            text(label.to_string()).size(style::META).font(style::FONT).color(color(p.ink_muted)).width(84),
            text(value).size(style::META).font(style::FONT).color(color(p.ink)).width(Length::Fill),
        ]
        .spacing(style::SPACE_3)
        .into()
    }

    pub(crate) fn preview_view(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let pane = self.pane();
        let sel = pane.selected_entries();
        let mut col = column![].spacing(style::SPACE_4);
        let Some((path, d)) = self.preview.as_ref().filter(|(path, _)| Some(path) == self.preview_target().as_ref()) else {
            let msg = if pane.loaded.is_some() { "Select something to see its details." } else { "" };
            col = col.push(Space::new().height(40));
            col = col.push(container(w::glyph(&self.icons, "info", 40.0, color(p.ink_faint))).width(Length::Fill).align_x(Alignment::Center));
            col = col.push(container(text(msg).size(style::META).font(style::FONT).color(color(p.ink_muted))).width(Length::Fill).align_x(Alignment::Center));
            return w::fill(container(col).padding(16), p.bg_sunken).width(WIDTH).height(Length::Fill).into();
        };
        // Picture: thumbnail when there is one, else the 96px colour icon.
        let pic: Element<'_, Message> = match self.thumbs.get(path) {
            Some(h) => image(h).width(Length::Fill).height(180).content_fit(iced::ContentFit::Contain).into(),
            None => {
                let icon = crate::kinds::icon(d.name.as_bytes(), if d.is_dir { ef_core::Kind::Dir } else { ef_core::Kind::File });
                container(iced::widget::svg(self.icons.color(icon)).width(96).height(96)).width(Length::Fill).height(140).center_x(Length::Fill).center_y(140).into()
            }
        };
        col = col.push(pic);
        let mut head = column![text(d.name.clone()).size(15).font(style::FONT_BOLD).color(color(p.ink_strong)), text(d.kind.clone()).size(style::META).font(style::FONT).color(color(p.ink_muted))].spacing(2);
        if sel.len() > 1 {
            let (files, dirs, bytes) = pane.selection_stats();
            let mut s = format!("{} selected", files + dirs);
            if files > 0 {
                s.push_str(" · ");
                fmt::size(bytes, &mut s);
                if dirs > 0 {
                    s.push_str(" + folders");
                }
            }
            head = head.push(text(s).size(style::META).font(style::FONT_BOLD).color(color(p.accent_ink)));
        }
        col = col.push(head);
        if let Some(e) = &d.error {
            col = col.push(text(format!("Can't read it: {e}")).size(style::META).font(style::FONT).color(color(p.danger.ink)));
            return w::fill(scrollable(container(col).padding(16)).height(Length::Fill), p.bg_sunken).width(WIDTH).height(Length::Fill).into();
        }
        let mut props = column![].spacing(6);
        let mut s = String::new();
        match (&d.tree, d.is_dir) {
            (Some(t), true) => {
                let done = t.done.load(Ordering::Relaxed);
                fmt::size(t.bytes.load(Ordering::Relaxed), &mut s);
                if !done {
                    s.push('+');
                }
                let files = t.files.load(Ordering::Relaxed) as usize;
                let mut f = String::new();
                fmt::count(files, &mut f);
                props = props.push(self.prop("Size", format!("{s} · {f} {}", if files == 1 { "file" } else { "files" })));
                props = props.push(self.prop("Contains", format!("{} {}", d.items.unwrap_or(0), if d.items == Some(1) { "item" } else { "items" })));
            }
            _ => {
                fmt::size(d.size, &mut s);
                let mut exact = String::new();
                fmt::count(d.size as usize, &mut exact);
                props = props.push(self.prop("Size", format!("{s} ({exact} bytes)")));
            }
        }
        let date = |t: i64| {
            let mut s = String::new();
            self.dates.format(t, &mut s);
            s
        };
        props = props.push(self.prop("Modified", date(d.modified)));
        if let Some(c) = d.created {
            props = props.push(self.prop("Created", date(c)));
        }
        props = props.push(self.prop("Opened", date(d.accessed)));
        props = props.push(self.prop("Location", config::tilde(path.parent().unwrap_or(path))));
        if let Some(l) = &d.link {
            props = props.push(self.prop("Points to", format!("{}{}", l.display(), if d.broken { " (missing)" } else { "" })));
        }
        if d.attrs.is_none() {
            props = props.push(self.prop("Permissions", format!("{} ({:o})", perms(d.mode), d.mode & 0o7777)));
            props = props.push(self.prop("Owner", format!("{} · {}", d.owner, d.group)));
        }
        col = col.push(props);
        if let Some(a) = d.attrs {
            let mut attrs = column![text("WINDOWS ATTRIBUTES").size(style::LABEL).font(style::FONT_BOLD).color(color(p.ink_muted))].spacing(2);
            for (bit, label) in [(win::READONLY, "Read-only"), (win::HIDDEN, "Hidden"), (win::SYSTEM, "System"), (win::ARCHIVE, "Archive")] {
                let path = path.clone();
                attrs = attrs.push(w::checkbox(p, &self.icons, label, a & bit != 0, move |on| Message::File(FileMsg::SetAttr(path.clone(), bit, on))));
            }
            let mut notes = Vec::new();
            if a & win::COMPRESSED != 0 {
                notes.push("Compressed");
            }
            if a & win::ENCRYPTED != 0 {
                notes.push("Encrypted (EFS)");
            }
            if a & win::SPARSE != 0 {
                notes.push("Sparse");
            }
            if a & (win::OFFLINE | win::RECALL_ON_OPEN | win::RECALL_ON_DATA_ACCESS) != 0 {
                notes.push("Online-only (cloud)");
            }
            if a & win::REPARSE != 0 {
                notes.push("Junction or link");
            }
            if !notes.is_empty() {
                attrs = attrs.push(text(notes.join(" · ")).size(style::META).font(style::FONT).color(color(p.ink_muted)));
            }
            col = col.push(attrs);
            if let Some(wp) = self.windows_path(path) {
                col = col.push(self.prop("Windows", wp));
                col = col.push(w::text_button(p, &self.icons, "Copy Windows path", Some("link"), None, Variant::Secondary, Some(Message::File(FileMsg::CopyWindowsPath))));
            }
        }
        col = col.push(row![
            w::text_button(p, &self.icons, "Open", Some("external"), Some("Enter"), Variant::Secondary, Some(Message::File(FileMsg::Open))),
            w::text_button(p, &self.icons, "Rename", Some("rename"), None, Variant::Ghost, Some(Message::File(FileMsg::StartRename))),
        ].spacing(style::SPACE_3));
        if let Some(t) = &d.text {
            let pal = p.clone();
            col = col.push(
                container(text(t.clone()).size(style::LABEL).font(style::FONT).color(color(p.ink)))
                    .padding(8)
                    .width(Length::Fill)
                    .style(move |_| container::Style { background: Some(Background::Color(color(pal.bg_deep))), border: Border { color: color(pal.line), width: 1.0, radius: 2.0.into() }, ..Default::default() }),
            );
        }
        w::fill(scrollable(container(col).padding(16)).height(Length::Fill), p.bg_sunken).width(WIDTH).height(Length::Fill).into()
    }
}

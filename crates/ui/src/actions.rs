//! What EchoFiles does to files: clipboard, rename, new folder, trash, delete, copy and
//! move (with the conflict dialog and progress toasts), drag and drop, undo, pins.
//! Everything that touches the disk runs on a worker thread; results come back as
//! [`FileMsg`]s and end in a toast — with Undo when the step can be reversed.

use std::collections::HashMap;
use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use ef_config as config;
use ef_core::ops::{self, Kind, Outcome, Plan, Progress, Resolution, Undo};
use ef_core::trash::{self, Trashed};
use iced::Task;

use crate::app::{background, App, Message, RENAME_ID};
use crate::file_list;
use crate::overlay::{Dialog, MenuFor, Tone};

/// Cut or copied files, waiting for Paste.
#[derive(Debug, Clone)]
pub struct Clip {
    pub paths: Vec<PathBuf>,
    pub cut: bool,
}

/// The inline rename in progress.
#[derive(Debug, Clone)]
pub struct Rename {
    pub pane: u64,
    pub path: PathBuf,
    pub value: String,
    /// What's wrong with `value`, and the name that would fix it (Windows rules).
    pub error: Option<String>,
    pub fixed: Option<String>,
    /// Display position once the item is listed.
    pub pos: Option<usize>,
}

/// A copy or move in flight (design system `TransferToast`, `ConflictDialog`).
pub struct Transfer {
    pub id: u64,
    pub kind: Kind,
    pub dest: PathBuf,
    pub sources: Vec<PathBuf>,
    pub progress: Arc<Progress>,
    pub plan: Option<Arc<Plan>>,
    pub choices: HashMap<PathBuf, Resolution>,
    pub default: Resolution,
    pub fix_names: bool,
    pub started: Instant,
    /// Smoothed bytes per second.
    pub rate: f64,
    sample: (Instant, u64),
    pub outcome: Option<Arc<Outcome>>,
    pub finished: Option<Instant>,
    pub error: Option<String>,
}

impl Transfer {
    pub fn running(&self) -> bool {
        self.outcome.is_none() && self.error.is_none()
    }

    /// Update the smoothed rate from the progress counters (called per frame).
    pub fn sample(&mut self) {
        let now = Instant::now();
        let done = self.progress.bytes_done.load(Ordering::Relaxed);
        let dt = now.duration_since(self.sample.0).as_secs_f64();
        if dt >= 0.25 {
            let r = (done.saturating_sub(self.sample.1)) as f64 / dt;
            self.rate = if self.rate == 0.0 { r } else { self.rate * 0.7 + r * 0.3 };
            self.sample = (now, done);
        }
    }

    pub fn title(&self) -> String {
        let n = self.sources.len();
        let what = if n == 1 { format!("“{}”", self.sources[0].file_name().unwrap_or_default().to_string_lossy()) } else { format!("{n} items") };
        let verb = match (self.kind, self.running()) {
            (Kind::Copy, true) => "Copying",
            (Kind::Move, true) => "Moving",
            (Kind::Copy, false) => "Copied",
            (Kind::Move, false) => "Moved",
        };
        format!("{verb} {what} to {}", place_name(&self.dest))
    }
}

/// "Downloads", "Home", "AVS (D:)".
pub fn place_name(p: &Path) -> String {
    if p == config::home() {
        return "Home".into();
    }
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn items(paths: &[PathBuf]) -> String {
    match paths {
        [one] => format!("“{}”", one.file_name().unwrap_or_default().to_string_lossy()),
        _ => plural(paths.len(), "item", "items"),
    }
}

/// What a trash run did: items that went, and (path, reason) for those that couldn't.
pub type TrashResult = Arc<(Vec<Trashed>, Vec<(PathBuf, String)>)>;

#[derive(Debug, Clone)]
pub enum FileMsg {
    Open,
    OpenInTab,
    OpenInOther,
    OpenWith,
    Copy,
    Cut,
    Paste,
    PasteInto(PathBuf),
    SystemClipboard(Option<(Vec<PathBuf>, bool)>, PathBuf),
    CopyPath,
    CopyWindowsPath,
    StartRename,
    RenameDraft(String),
    RenameCommit,
    Renamed(Result<(PathBuf, PathBuf), String>),
    Trash,
    Trashed(TrashResult),
    AskDelete,
    Delete(Vec<PathBuf>),
    Deleted(Vec<PathBuf>, Vec<String>),
    NewFolder,
    NewFile,
    Created(Result<PathBuf, String>),
    Undo,
    Undone(Result<String, String>),
    Start { kind: Kind, sources: Vec<PathBuf>, dest: PathBuf },
    ToOther(Kind),
    Planned(u64, Result<Arc<Plan>, String>),
    Resolve(Resolution, bool),
    FixNames(bool),
    Finished(u64, Arc<Outcome>),
    Pause(u64),
    Cancel(u64),
    Dismiss(u64),
    Properties,
    Pin(PathBuf),
    Unpin(PathBuf),
    MenuAtCursor,
    Restore,
    Restored(Vec<String>),
    AskEmptyTrash,
    EmptyTrash,
    Emptied(Result<(), String>),
    SetAttr(PathBuf, u32, bool),
    /// Change Linux permission bits (the preview pane's switches).
    SetMode(PathBuf, u32),
    PropsLoaded(PathBuf, Arc<crate::preview::PreviewData>),
    SelectAll,
    OpenTerminal,
}

/// `file://` URI for the system clipboard.
fn uri(p: &Path) -> String {
    let mut out = String::from("file://");
    for &b in p.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn from_uri(s: &str) -> Option<PathBuf> {
    let rest = s.trim().strip_prefix("file://")?;
    let b = rest.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(&out)))
}

/// Offer the files to other apps too: GTK/Qt file managers read
/// `x-special/gnome-copied-files`; browsers and chat apps read `text/uri-list`.
fn write_system_clipboard(paths: &[PathBuf], cut: bool) {
    let uris: Vec<String> = paths.iter().map(|p| uri(p)).collect();
    let gnome = format!("{}\n{}", if cut { "cut" } else { "copy" }, uris.join("\n"));
    std::thread::spawn(move || {
        use std::io::Write;
        use std::process::{Command, Stdio};
        if let Ok(mut child) = Command::new("wl-copy").args(["--type", "x-special/gnome-copied-files"]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(gnome.as_bytes());
            }
            let _ = child.wait();
        }
    });
}

/// Files another app put on the clipboard, and whether they were cut.
fn read_system_clipboard() -> Option<(Vec<PathBuf>, bool)> {
    use std::process::Command;
    let types = Command::new("wl-paste").arg("--list-types").output().ok()?;
    let types = String::from_utf8_lossy(&types.stdout).into_owned();
    let get = |t: &str| Command::new("wl-paste").args(["--no-newline", "--type", t]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    if types.lines().any(|l| l == "x-special/gnome-copied-files") {
        let text = get("x-special/gnome-copied-files")?;
        let mut lines = text.lines();
        let cut = lines.next()? == "cut";
        let paths: Vec<PathBuf> = lines.filter_map(from_uri).collect();
        return (!paths.is_empty()).then_some((paths, cut));
    }
    if types.lines().any(|l| l == "text/uri-list") {
        let text = get("text/uri-list")?;
        let paths: Vec<PathBuf> = text.lines().filter(|l| !l.starts_with('#')).filter_map(from_uri).collect();
        return (!paths.is_empty()).then_some((paths, false));
    }
    None
}

impl App {
    /// Dim cut items in every pane showing their folder.
    pub(crate) fn refresh_cut(&mut self) {
        let clip = self.clip.clone();
        for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()) {
            let Some(l) = pane.loaded.clone() else { continue };
            let mut bits = vec![0u64; l.listing.len().div_ceil(64)];
            if let Some(c) = clip.as_ref().filter(|c| c.cut) {
                let names: Vec<&std::ffi::OsStr> = c.paths.iter().filter(|p| p.parent() == Some(pane.location.as_path())).filter_map(|p| p.file_name()).collect();
                if !names.is_empty() {
                    for i in 0..l.listing.len() {
                        if names.contains(&l.listing.name(i)) {
                            file_list::set_bit(&mut bits, i, true);
                        }
                    }
                }
            }
            pane.cut = bits;
        }
    }

    /// Put the pending rename's field on its item once it's listed.
    pub(crate) fn resume_rename(&mut self, pane_id: u64) -> Task<Message> {
        let Some(r) = self.rename.as_ref().filter(|r| r.pane == pane_id) else { return Task::none() };
        let Some(pane) = self.pane_by_id(pane_id) else { return Task::none() };
        let Some(l) = pane.loaded.clone() else { return Task::none() };
        let name = r.path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        let pos = pane.order.iter().position(|&i| l.listing.name(i as usize) == name);
        let had = r.pos.is_some();
        let r = self.rename.as_mut().unwrap();
        r.pos = pos;
        match pos {
            Some(p) if !had => {
                let pane = self.pane_by_id_mut(pane_id).unwrap();
                pane.select_only(p);
                focus_rename(&self.rename.as_ref().unwrap().value, self.rename.as_ref().unwrap().path.is_dir())
            }
            _ => Task::none(),
        }
    }

    pub(crate) fn toast_ok(&mut self, title: String, undo: bool) {
        let action = undo.then(|| ("Undo".to_string(), Message::File(FileMsg::Undo)));
        self.push_toast(Tone::Success, title, None, action);
    }

    pub(crate) fn toast_error(&mut self, title: String, body: String) {
        self.push_toast(Tone::Danger, title, Some(body), None);
    }

    fn is_windows_fs(&self, dir: &Path) -> bool {
        crate::app::fs_of(dir).is_some_and(|f| ops::windows_fs(f.fs_type))
    }

    /// Copy or move `paths` into `dest`: the drag-and-drop rule is move within a drive, copy
    /// between drives; Ctrl forces copy, Shift forces move.
    pub(crate) fn drop_paths(&mut self, paths: Vec<PathBuf>, dest: PathBuf) -> Task<Message> {
        if paths.iter().all(|p| p.parent() == Some(dest.as_path())) || paths.iter().any(|p| p == &dest) {
            return Task::none();
        }
        let same = |a: &Path, b: &Path| std::fs::metadata(a).ok().zip(std::fs::metadata(b).ok()).is_some_and(|(x, y)| x.dev() == y.dev());
        let kind = if self.modifiers.control() {
            Kind::Copy
        } else if self.modifiers.shift() || paths.iter().all(|p| same(p, &dest)) {
            Kind::Move
        } else {
            Kind::Copy
        };
        self.file_update(FileMsg::Start { kind, sources: paths, dest })
    }

    fn start_transfer(&mut self, kind: Kind, sources: Vec<PathBuf>, dest: PathBuf) -> Task<Message> {
        if sources.is_empty() {
            return Task::none();
        }
        let id = crate::pane::next_id();
        let progress = Arc::new(Progress::default());
        self.transfers.push(Transfer {
            id,
            kind,
            dest: dest.clone(),
            sources: sources.clone(),
            progress: progress.clone(),
            plan: None,
            choices: HashMap::new(),
            default: Resolution::Skip,
            fix_names: false,
            started: Instant::now(),
            rate: 0.0,
            sample: (Instant::now(), 0),
            outcome: None,
            finished: None,
            error: None,
        });
        background(move || ops::plan(kind, &sources, &dest, &progress).map(Arc::new), move |r| Message::File(FileMsg::Planned(id, r)))
    }

    fn run_transfer(&mut self, id: u64) -> Task<Message> {
        let Some(t) = self.transfers.iter().find(|t| t.id == id) else { return Task::none() };
        let Some(plan) = t.plan.clone() else { return Task::none() };
        let (choices, default, fix, progress) = (t.choices.clone(), t.default, t.fix_names, t.progress.clone());
        background(move || Arc::new(ops::execute(&plan, &choices, default, fix, &progress)), move |o| Message::File(FileMsg::Finished(id, o)))
    }

    /// Next question for a planned transfer, or run it.
    fn next_step(&mut self, id: u64, index: usize) -> Task<Message> {
        let Some(t) = self.transfers.iter().find(|t| t.id == id) else { return Task::none() };
        let Some(plan) = t.plan.clone() else { return Task::none() };
        if index < plan.conflicts.len() && t.default == Resolution::Skip && t.choices.len() < plan.conflicts.len() {
            self.dialog = Some(Dialog::Conflict { id, index, all: false, opened: Instant::now() });
            return Task::none();
        }
        if !plan.bad_names.is_empty() && !t.fix_names {
            self.dialog = Some(Dialog::BadNames { id, opened: Instant::now() });
            return Task::none();
        }
        self.run_transfer(id)
    }

    fn targets(&self) -> Vec<PathBuf> {
        self.pane().targets()
    }

    pub(crate) fn file_update(&mut self, msg: FileMsg) -> Task<Message> {
        match msg {
            FileMsg::Open => {
                let targets = self.targets();
                let mut tasks = Vec::new();
                for p in targets.iter().take(20) {
                    tasks.push(self.open_path(p.clone()));
                    if p.is_dir() {
                        break;
                    }
                }
                Task::batch(tasks)
            }
            FileMsg::OpenInTab => {
                let dirs: Vec<PathBuf> = self.targets().into_iter().filter(|p| p.is_dir()).collect();
                Task::batch(dirs.into_iter().map(|d| self.update(Message::OpenTab(d))))
            }
            FileMsg::OpenInOther => {
                let Some(dir) = self.targets().into_iter().find(|p| p.is_dir()) else { return Task::none() };
                if !self.dual() {
                    let t = self.update(Message::ToggleDual);
                    return Task::batch([t, self.update(Message::Navigate(dir))]);
                }
                let _ = self.update(Message::SwitchPane);
                self.update(Message::Navigate(dir))
            }
            FileMsg::OpenWith => {
                // The desktop's own chooser (handlr/xdg) when present; otherwise open normally.
                let targets = self.targets();
                for p in targets.iter().take(1) {
                    let chooser = ["handlr", "mimeo"].into_iter().find(|c| std::process::Command::new("which").arg(c).output().is_ok_and(|o| o.status.success()));
                    let r = match chooser {
                        Some("handlr") => std::process::Command::new("handlr").arg("open").arg(p).spawn(),
                        _ => std::process::Command::new("xdg-open").arg(p).spawn(),
                    };
                    if let Err(e) = r {
                        self.toast_error("Couldn't open that".into(), e.to_string());
                    }
                }
                Task::none()
            }
            FileMsg::OpenTerminal => {
                let dir = self.pane().location.clone();
                let r = std::process::Command::new("xdg-terminal-exec").current_dir(&dir).spawn().or_else(|_| std::process::Command::new("alacritty").current_dir(&dir).spawn());
                if let Err(e) = r {
                    self.toast_error("Couldn't open a terminal".into(), e.to_string());
                }
                Task::none()
            }
            FileMsg::Copy | FileMsg::Cut => {
                let cut = matches!(msg, FileMsg::Cut);
                let paths = self.targets();
                if paths.is_empty() {
                    return Task::none();
                }
                write_system_clipboard(&paths, cut);
                self.clip = Some(Clip { paths, cut });
                self.refresh_cut();
                Task::none()
            }
            FileMsg::Paste => {
                let dest = self.pane().location.clone();
                self.file_update(FileMsg::PasteInto(dest))
            }
            FileMsg::PasteInto(dest) => match self.clip.clone() {
                Some(c) => {
                    if c.cut {
                        self.clip = None;
                        self.refresh_cut();
                    }
                    self.start_transfer(if c.cut { Kind::Move } else { Kind::Copy }, c.paths, dest)
                }
                None => background(read_system_clipboard, move |r| Message::File(FileMsg::SystemClipboard(r, dest.clone()))),
            },
            FileMsg::SystemClipboard(r, dest) => match r {
                Some((paths, cut)) => self.start_transfer(if cut { Kind::Move } else { Kind::Copy }, paths, dest),
                None => {
                    self.push_toast(Tone::Accent, "Nothing to paste".into(), Some("Copy or cut files first (Ctrl+C / Ctrl+X).".into()), None);
                    Task::none()
                }
            },
            FileMsg::CopyPath => {
                let text: Vec<String> = self.targets().iter().map(|p| p.display().to_string()).collect();
                let text = if text.is_empty() { self.pane().location.display().to_string() } else { text.join("\n") };
                iced::clipboard::write(text)
            }
            FileMsg::CopyWindowsPath => {
                let paths = self.targets();
                let mut out = Vec::new();
                for p in &paths {
                    match self.windows_path(p) {
                        Some(w) => out.push(w),
                        None => {
                            self.toast_error("No drive letter yet".into(), "EchoFiles learns drive letters from Windows (C:) — mount it once.".into());
                            return Task::none();
                        }
                    }
                }
                iced::clipboard::write(out.join("\r\n"))
            }
            FileMsg::StartRename => {
                let pane = self.pane();
                let Some(l) = pane.loaded.clone() else { return Task::none() };
                let pos = pane.cursor.or_else(|| pane.selected_entries().first().and_then(|&i| pane.order.iter().position(|&x| x as usize == i)));
                let Some(pos) = pos else { return Task::none() };
                let Some(i) = pane.entry_at(pos) else { return Task::none() };
                let path = l.listing.path(i);
                let value = l.listing.name(i).to_string_lossy().into_owned();
                let id = pane.id;
                self.pane_mut().select_only(pos);
                self.rename = Some(Rename { pane: id, path: path.clone(), value: value.clone(), error: None, fixed: None, pos: Some(pos) });
                focus_rename(&value, l.listing.is_dir(i))
            }
            FileMsg::RenameDraft(v) => {
                let windows = self.rename.as_ref().is_some_and(|r| r.path.parent().is_some_and(|d| self.is_windows_fs(d)));
                let Some(r) = self.rename.as_mut() else { return Task::none() };
                r.error = ops::name_problem(&v).map(String::from);
                r.fixed = None;
                if r.error.is_none() && windows && let Some((why, fixed)) = ops::windows_name(&v) {
                    r.error = Some(format!("{why} Press Enter to use “{fixed}”."));
                    r.fixed = Some(fixed);
                }
                if r.error.is_none() && r.path.file_name().is_some_and(|n| n.to_string_lossy() != v) && r.path.with_file_name(&v).exists() {
                    // Case-only renames of the same file are fine.
                    let same = std::fs::metadata(r.path.with_file_name(&v)).ok().zip(std::fs::metadata(&r.path).ok()).is_some_and(|(a, b)| a.ino() == b.ino());
                    if !same {
                        r.error = Some(format!("“{v}” already exists here."));
                    }
                }
                r.value = v;
                Task::none()
            }
            FileMsg::RenameCommit => {
                let Some(r) = self.rename.take() else { return Task::none() };
                let original = r.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let name = match (&r.error, &r.fixed) {
                    (_, Some(f)) => f.clone(),
                    (Some(_), None) => {
                        // Keep editing: the message says what's wrong.
                        self.rename = Some(r);
                        return Task::none();
                    }
                    _ => r.value.clone(),
                };
                if name == original {
                    return Task::none();
                }
                let from = r.path.clone();
                background(move || ops::rename(&from, &name).map(|to| (from.clone(), to)).map_err(|e| e.to_string()), |res| Message::File(FileMsg::Renamed(res)))
            }
            FileMsg::Renamed(Ok((from, to))) => {
                self.undo.push(Undo::Rename { from: from.clone(), to: to.clone() });
                let dir = to.parent().map(Path::to_path_buf).unwrap_or_default();
                if let Some(name) = to.file_name() {
                    for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()).filter(|p| p.location == dir) {
                        pane.reveal = vec![name.to_os_string()];
                    }
                }
                self.refresh(&dir)
            }
            FileMsg::Renamed(Err(e)) => {
                self.toast_error("Couldn't rename".into(), e);
                Task::none()
            }
            FileMsg::Trash => {
                let paths = self.targets();
                if paths.is_empty() {
                    return Task::none();
                }
                // Items already in a trash can only be deleted for good.
                if paths.iter().any(|p| p.parent().is_some_and(trash::is_trash_files)) {
                    return self.file_update(FileMsg::AskDelete);
                }
                self.select_after_removal();
                background(move || Arc::new(ops::trash_all(&paths)), |r| Message::File(FileMsg::Trashed(r)))
            }
            FileMsg::Trashed(r) => {
                let (ok, failed) = (&r.0, &r.1);
                let mut dirs: Vec<PathBuf> = ok.iter().filter_map(|t| t.original.parent().map(Path::to_path_buf)).collect();
                dirs.dedup();
                if !ok.is_empty() {
                    let originals: Vec<PathBuf> = ok.iter().map(|t| t.original.clone()).collect();
                    self.undo.push(Undo::Trash(ok.clone()));
                    self.toast_ok(format!("Moved {} to the Trash", items(&originals)), true);
                }
                if !failed.is_empty() {
                    // No trash on this drive (read-only, or no permission to make one): offer
                    // to delete for good instead, never silently.
                    let paths: Vec<PathBuf> = failed.iter().map(|f| f.0.clone()).collect();
                    let reason = failed[0].1.clone();
                    self.dialog = Some(Dialog::Delete { paths, reason: Some(format!("It can't go to the Trash: {reason}.")), opened: Instant::now() });
                }
                Task::batch(dirs.iter().map(|d| self.refresh(d)).collect::<Vec<_>>())
            }
            FileMsg::AskDelete => {
                let paths = self.targets();
                if !paths.is_empty() {
                    self.dialog = Some(Dialog::Delete { paths, reason: None, opened: Instant::now() });
                }
                Task::none()
            }
            FileMsg::Delete(paths) => {
                self.select_after_removal();
                let p2 = paths.clone();
                background(move || ops::delete(&p2, &Progress::default()), move |errs| Message::File(FileMsg::Deleted(paths.clone(), errs)))
            }
            FileMsg::Deleted(paths, errs) => {
                if errs.is_empty() {
                    self.push_toast(Tone::Success, format!("Deleted {} permanently", items(&paths)), None, None);
                } else {
                    self.toast_error(format!("Couldn't delete {}", plural(errs.len(), "item", "items")), errs.join("\n"));
                }
                let mut dirs: Vec<PathBuf> = paths.iter().filter_map(|p| p.parent().map(Path::to_path_buf)).collect();
                dirs.dedup();
                Task::batch(dirs.iter().map(|d| self.refresh(d)).collect::<Vec<_>>())
            }
            FileMsg::NewFolder | FileMsg::NewFile => {
                let dir = self.pane().location.clone();
                let folder = matches!(msg, FileMsg::NewFolder);
                background(move || if folder { ops::new_folder(&dir) } else { ops::new_file(&dir) }.map_err(|e| e.to_string()), |r| Message::File(FileMsg::Created(r)))
            }
            FileMsg::Created(Ok(path)) => {
                self.undo.push(Undo::Create(path.clone()));
                let id = self.pane().id;
                let value = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                self.rename = Some(Rename { pane: id, path: path.clone(), value, error: None, fixed: None, pos: None });
                let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
                self.refresh(&dir)
            }
            FileMsg::Created(Err(e)) => {
                self.toast_error("Couldn't create it here".into(), e);
                Task::none()
            }
            FileMsg::Undo => match self.undo.pop() {
                Some(u) => background(move || u.apply(), |r| Message::File(FileMsg::Undone(r))),
                None => {
                    self.push_toast(Tone::Accent, "Nothing to undo".into(), None, None);
                    Task::none()
                }
            },
            FileMsg::Undone(r) => {
                match r {
                    Ok(m) => self.push_toast(Tone::Success, m, None, None),
                    Err(e) => self.toast_error("Couldn't undo".into(), e),
                }
                let dirs: Vec<PathBuf> = self.tabs[self.tab].panes.iter().map(|p| p.location.clone()).collect();
                Task::batch(dirs.iter().map(|d| self.refresh(d)).collect::<Vec<_>>())
            }
            FileMsg::Start { kind, sources, dest } => self.start_transfer(kind, sources, dest),
            FileMsg::ToOther(kind) => {
                let Some(dest) = self.other_pane().map(|p| p.location.clone()) else { return Task::none() };
                let sources = self.targets();
                self.start_transfer(kind, sources, dest)
            }
            FileMsg::Planned(id, r) => {
                let Some(t) = self.transfers.iter_mut().find(|t| t.id == id) else { return Task::none() };
                match r {
                    Ok(plan) => {
                        if plan.items.is_empty() {
                            self.transfers.retain(|t| t.id != id);
                            return Task::none();
                        }
                        t.plan = Some(plan);
                        self.next_step(id, 0)
                    }
                    Err(e) => {
                        let verb = if t.kind == Kind::Copy { "copy" } else { "move" };
                        self.transfers.retain(|t| t.id != id);
                        self.toast_error(format!("Couldn't {verb} those files"), e);
                        Task::none()
                    }
                }
            }
            FileMsg::Resolve(choice, all) => {
                let Some(Dialog::Conflict { id, index, .. }) = self.dialog.take() else { return Task::none() };
                let Some(t) = self.transfers.iter_mut().find(|t| t.id == id) else { return Task::none() };
                let Some(plan) = t.plan.clone() else { return Task::none() };
                if all {
                    t.default = choice;
                    for c in &plan.conflicts[index..] {
                        t.choices.insert(c.dst.clone(), choice);
                    }
                    self.next_step(id, plan.conflicts.len())
                } else {
                    t.choices.insert(plan.conflicts[index].dst.clone(), choice);
                    self.next_step(id, index + 1)
                }
            }
            FileMsg::FixNames(fix) => {
                let Some(Dialog::BadNames { id, .. }) = self.dialog.take() else { return Task::none() };
                if !fix {
                    self.transfers.retain(|t| t.id != id);
                    return Task::none();
                }
                if let Some(t) = self.transfers.iter_mut().find(|t| t.id == id) {
                    t.fix_names = true;
                }
                self.run_transfer(id)
            }
            FileMsg::Finished(id, outcome) => {
                let Some(t) = self.transfers.iter_mut().find(|t| t.id == id) else { return Task::none() };
                t.finished = Some(Instant::now());
                t.outcome = Some(outcome.clone());
                let (kind, dest, title) = (t.kind, t.dest.clone(), t.title());
                let quick = t.started.elapsed().as_millis() < 700;
                if !outcome.done.is_empty() && !outcome.cancelled {
                    self.undo.push(match kind {
                        Kind::Copy => Undo::Copy(outcome.done.iter().map(|(_, d)| d.clone()).collect()),
                        Kind::Move => Undo::Move(outcome.done.clone()),
                    });
                }
                if outcome.cancelled {
                    self.push_toast(Tone::Accent, if kind == Kind::Move { "Move cancelled — everything is back where it was".into() } else { "Copy cancelled".into() }, None, None);
                } else if !outcome.errors.is_empty() {
                    let n = outcome.errors.len();
                    self.toast_error(format!("{} couldn't be {}", plural(n, "item", "items"), if kind == Kind::Copy { "copied" } else { "moved" }), outcome.errors.iter().take(3).cloned().collect::<Vec<_>>().join("\n"));
                } else if quick && !outcome.done.is_empty() {
                    // Fast ones never showed a progress toast: say what happened, with Undo.
                    self.toast_ok(title, true);
                }
                if quick {
                    self.transfers.retain(|t| t.id != id);
                }
                // Select what arrived.
                let names: Vec<OsString> = outcome.done.iter().filter_map(|(_, d)| d.file_name().map(|n| n.to_os_string())).collect();
                for pane in self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()).filter(|p| p.location == dest) {
                    pane.reveal = names.clone();
                }
                let mut dirs: Vec<PathBuf> = outcome.done.iter().filter_map(|(s, _)| s.parent().map(Path::to_path_buf)).collect();
                dirs.push(dest);
                dirs.sort();
                dirs.dedup();
                Task::batch(dirs.iter().map(|d| self.refresh(d)).collect::<Vec<_>>())
            }
            FileMsg::Pause(id) => {
                if let Some(t) = self.transfers.iter().find(|t| t.id == id) {
                    let p = !t.progress.paused.load(Ordering::Relaxed);
                    t.progress.paused.store(p, Ordering::Relaxed);
                }
                Task::none()
            }
            FileMsg::Cancel(id) => {
                if let Some(t) = self.transfers.iter().find(|t| t.id == id) {
                    t.progress.cancel.store(true, Ordering::Relaxed);
                }
                Task::none()
            }
            FileMsg::Dismiss(id) => {
                self.transfers.retain(|t| t.id != id);
                Task::none()
            }
            FileMsg::Properties => {
                // The selection, or the open folder when nothing is selected.
                let mut paths = self.targets();
                if paths.is_empty() {
                    paths.push(self.pane().location.clone());
                }
                let cancel: Arc<std::sync::atomic::AtomicBool> = Arc::default();
                let mut total = None;
                let mut task = Task::none();
                if paths.len() == 1 {
                    let p = paths[0].clone();
                    task = background(move || Arc::new(crate::preview::gather(&p)), |d| {
                        let path = PathBuf::from(&d.path);
                        Message::File(FileMsg::PropsLoaded(path, d))
                    });
                } else {
                    let size = Arc::new(ops::DirSize::default());
                    let (s2, c2, p2) = (size.clone(), cancel.clone(), paths.clone());
                    std::thread::spawn(move || ops::measure(&p2, &s2, &c2));
                    total = Some(size);
                }
                self.menu = None;
                self.dialog = Some(Dialog::Properties { paths, data: None, total, cancel, opened: Instant::now() });
                task
            }
            FileMsg::PropsLoaded(path, d) => {
                match &mut self.dialog {
                    Some(Dialog::Properties { paths, data, .. }) if paths.len() == 1 && paths[0] == path => *data = Some(d),
                    _ => d.cancel.store(true, Ordering::Relaxed),
                }
                Task::none()
            }
            FileMsg::SetMode(path, mode) => {
                use std::os::unix::fs::PermissionsExt;
                if let Err(e) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)) {
                    self.toast_error("Couldn't change the permissions".into(), e.to_string());
                }
                self.preview = None;
                self.preview_want = None;
                let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
                self.refresh(&dir)
            }
            FileMsg::Pin(p) => {
                let s = config::tilde(&p);
                if !self.settings.sidebar.pinned.contains(&s) {
                    self.settings.sidebar.pinned.push(s);
                }
                self.persist_settings()
            }
            FileMsg::Unpin(p) => {
                let s = config::tilde(&p);
                self.settings.sidebar.pinned.retain(|x| x != &s && config::expand(x) != p);
                self.persist_settings()
            }
            FileMsg::MenuAtCursor => {
                let id = self.pane().id;
                let on_item = self.pane().cursor.is_some();
                let at = if self.mouse == iced::Point::ORIGIN { iced::Point::new(400.0, 200.0) } else { self.mouse };
                self.open_menu(MenuFor::Files { pane: id, on_item }, at);
                Task::none()
            }
            FileMsg::Restore => {
                let paths = self.targets();
                background(
                    move || {
                        let mut errs = Vec::new();
                        for p in paths {
                            match trash::entry_for(&p) {
                                Some(e) => {
                                    if let Err(err) = trash::restore(&e.trashed) {
                                        errs.push(format!("{}: {err}", p.display()));
                                    }
                                }
                                None => errs.push(format!("{}: no record of where it came from", p.display())),
                            }
                        }
                        errs
                    },
                    |errs| Message::File(FileMsg::Restored(errs)),
                )
            }
            FileMsg::Restored(errs) => {
                if errs.is_empty() {
                    self.push_toast(Tone::Success, "Restored from the Trash".into(), None, None);
                } else {
                    self.toast_error("Couldn't restore everything".into(), errs.join("\n"));
                }
                let dir = self.pane().location.clone();
                self.refresh(&dir)
            }
            FileMsg::AskEmptyTrash => {
                self.dialog = Some(Dialog::EmptyTrash { opened: Instant::now() });
                Task::none()
            }
            FileMsg::EmptyTrash => background(|| trash::empty_home().map_err(|e| e.to_string()), |r| Message::File(FileMsg::Emptied(r))),
            FileMsg::Emptied(r) => {
                match r {
                    Ok(()) => self.push_toast(Tone::Success, "Emptied the Trash".into(), None, None),
                    Err(e) => self.toast_error("Couldn't empty the Trash".into(), e),
                }
                let dir = trash::home_trash().join("files");
                self.refresh(&dir)
            }
            FileMsg::SetAttr(path, bit, on) => {
                let mut buf = [0u8; 4];
                let r = rustix::fs::lgetxattr(&path, "system.ntfs_attrib", &mut buf).and_then(|_| {
                    let mut a = u32::from_le_bytes(buf);
                    if on { a |= bit } else { a &= !bit }
                    rustix::fs::lsetxattr(&path, "system.ntfs_attrib", &a.to_le_bytes(), rustix::fs::XattrFlags::empty())
                });
                if let Err(e) = r {
                    self.toast_error("Couldn't change that attribute".into(), std::io::Error::from(e).to_string());
                }
                self.preview = None;
                self.preview_want = None;
                let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
                self.refresh(&dir)
            }
            FileMsg::SelectAll => {
                self.pane_mut().select_all();
                Task::none()
            }
        }
    }

    /// Before items disappear, put the cursor on the next item that stays.
    fn select_after_removal(&mut self) {
        let pane = self.pane_mut();
        let Some(l) = pane.loaded.clone() else { return };
        let sel = pane.selected_entries();
        let start = pane.cursor.unwrap_or(0);
        let keep = (start..pane.order.len()).chain((0..start).rev()).map(|p| pane.order[p] as usize).find(|i| !sel.contains(i) && Some(*i) != pane.cursor.and_then(|c| pane.entry_at(c)));
        if let Some(i) = keep {
            pane.reveal = vec![l.listing.name(i).to_os_string()];
        }
    }

    /// `D:\Work\file.txt` for a path on a Windows volume with a known letter.
    pub(crate) fn windows_path(&self, p: &Path) -> Option<String> {
        self.volumes.iter().find_map(|v| {
            let mp = v.mount_points.first()?;
            let rest = p.strip_prefix(mp).ok()?;
            let letter = v.letter?;
            Some(format!("{letter}:\\{}", rest.to_string_lossy().replace('/', "\\")))
        })
    }
}

/// Focus the rename field with the name's stem selected (the extension stays).
fn focus_rename(value: &str, is_dir: bool) -> Task<Message> {
    let chars = value.chars().count();
    let stem = if is_dir { chars } else { value.rfind('.').filter(|&p| p > 0).map(|p| value[..p].chars().count()).unwrap_or(chars) };
    Task::batch([iced::widget::operation::focus(RENAME_ID), iced::widget::operation::select_range(RENAME_ID, 0, stem)])
}

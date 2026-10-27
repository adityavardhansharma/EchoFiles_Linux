//! Copy, move, delete, rename and create (build plan §2.5).
//!
//! Every copy or move is planned first: totals, conflicts, Windows-name problems and free
//! space are known before a byte is written. Execution reports through [`Progress`]
//! atomics that the UI reads at frame rate. Copies try a reflink (`FICLONE`, instant on
//! btrfs), then `copy_file_range`, then a plain read/write loop; nothing is fsynced per
//! file. A batch has one durability barrier: a `syncfs` after every entry is committed and
//! before any cross-filesystem source is removed.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use rayon::prelude::*;

use crate::trash::{self, Trashed};

const CHUNK: usize = 8 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Copy,
    Move,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    Planning = 0,
    Running = 1,
    Syncing = 2,
    Done = 3,
}

/// Shared between the worker thread and the UI; the UI only reads (and sets cancel/pause).
#[derive(Default, Debug)]
pub struct Progress {
    pub bytes_done: AtomicU64,
    pub bytes_total: AtomicU64,
    pub files_done: AtomicU64,
    pub files_total: AtomicU64,
    pub cancel: AtomicBool,
    pub paused: AtomicBool,
    phase: AtomicU8,
    current: Mutex<PathBuf>,
}

impl Progress {
    pub fn phase(&self) -> Phase {
        match self.phase.load(Ordering::Relaxed) {
            0 => Phase::Planning,
            1 => Phase::Running,
            2 => Phase::Syncing,
            _ => Phase::Done,
        }
    }

    pub fn set_phase(&self, p: Phase) {
        self.phase.store(p as u8, Ordering::Relaxed);
    }

    pub fn current(&self) -> PathBuf {
        self.current.lock().unwrap().clone()
    }

    fn set_current(&self, p: &Path) {
        *self.current.lock().unwrap() = p.to_path_buf();
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Block while paused; false once cancelled.
    fn proceed(&self) -> bool {
        while self.paused.load(Ordering::Relaxed) && !self.cancelled() {
            std::thread::sleep(Duration::from_millis(50));
        }
        !self.cancelled()
    }
}

fn cancelled_err() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "cancelled")
}

// ------------------------------------------------------------------------------ naming

/// How a free name is made: `photo (2).jpg` for Keep both, `photo (copy).jpg` for a
/// copy into the same folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Naming {
    Numbered,
    Copy,
}

fn split_ext(name: &[u8], is_dir: bool) -> (&[u8], &[u8]) {
    if is_dir {
        return (name, b"");
    }
    match name.iter().rposition(|&c| c == b'.').filter(|&p| p > 0 && name.len() - p <= 8) {
        Some(p) => (&name[..p], &name[p..]),
        None => (name, b""),
    }
}

/// The first name next to `path` that doesn't exist yet.
pub fn unique_path(path: &Path, naming: Naming) -> PathBuf {
    let Some(parent) = path.parent() else { return path.to_path_buf() };
    let name = path.file_name().map(|n| n.as_bytes().to_vec()).unwrap_or_default();
    let is_dir = fs::symlink_metadata(path).is_ok_and(|m| m.is_dir());
    let (stem, ext) = split_ext(&name, is_dir);
    let candidate = |n: u32| {
        let tag = match (naming, n) {
            (Naming::Numbered, _) => format!(" ({})", n + 1),
            (Naming::Copy, 1) => " (copy)".to_string(),
            (Naming::Copy, _) => format!(" (copy {n})"),
        };
        parent.join(OsString::from_vec([stem, tag.as_bytes(), ext].concat()))
    };
    candidate(free_number(1, |n| fs::symlink_metadata(candidate(n)).is_ok()))
}

/// A number ≥ `from` for which `taken` is false, found in O(log n) probes: doubling steps
/// to a free number, then a binary search back towards the last taken one. Numbered names
/// are normally taken from 1 upwards, so this is the first gap; with holes it may skip one.
pub(crate) fn free_number(from: u32, taken: impl Fn(u32) -> bool) -> u32 {
    if !taken(from) { return from; }
    let (mut lo, mut step) = (from, 1u32);
    let mut hi = loop {
        let next = lo.saturating_add(step);
        if next == u32::MAX || !taken(next) { break next; }
        lo = next;
        step = step.saturating_mul(2);
    };
    // Invariant: `lo` is taken and `hi` is free.
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if taken(mid) { lo = mid } else { hi = mid }
    }
    hi
}

/// `rename` that never replaces an existing file, including on case-insensitive filesystems.
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    use rustix::fs::{renameat_with, RenameFlags, CWD};
    match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(()),
        // Never emulate NOREPLACE with a check followed by an overwriting rename.
        Err(e) => Err(e.into()),
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::symlink_metadata(a), fs::symlink_metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

/// Rename `path` to `new_name` in the same folder. Returns the new path.
pub fn rename(path: &Path, new_name: &str) -> io::Result<PathBuf> {
    let new_name = new_name.trim_end_matches(['\n', '\r']);
    if let Some(problem) = name_problem(new_name) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, problem));
    }
    let to = path.with_file_name(new_name);
    if to == path {
        return Ok(to);
    }
    match rename_noreplace(path, &to) {
        // A case-only change on a case-insensitive filesystem (FAT, exFAT) finds the
        // file itself at the new name. Go through a free temporary name instead.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && same_file(path, &to) => {
            let step = unique_path(&path.with_file_name(".echofiles-rename"), Naming::Numbered);
            rename_noreplace(path, &step)?;
            if let Err(e) = rename_noreplace(&step, &to) {
                let _ = rename_noreplace(&step, path);
                return Err(e);
            }
        }
        r => r?,
    }
    Ok(to)
}

/// Why `name` can't be a file name on Linux, if it can't.
pub fn name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() || name.trim().is_empty() {
        Some("A name can't be empty.")
    } else if name == "." || name == ".." {
        Some("“.” and “..” are reserved names.")
    } else if name.contains('/') {
        Some("A name can't contain “/”.")
    } else if name.contains('\0') {
        Some("A name can't contain a null character.")
    } else if name.len() > 255 {
        Some("That name is too long (255 bytes at most).")
    } else {
        None
    }
}

const WIN_RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5",
    "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Windows can't use this name: the reason and a fixed name.
pub fn windows_name(name: &str) -> Option<(String, String)> {
    let bad: Vec<char> = name.chars().filter(|c| "<>:\"\\|?*".contains(*c) || (*c as u32) < 32).collect();
    let stem = name.split('.').next().unwrap_or("").trim_end();
    let reserved = WIN_RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem));
    let trailing = name.ends_with('.') || name.ends_with(' ');
    if bad.is_empty() && !reserved && !trailing {
        return None;
    }
    let mut fixed: String = name.chars().map(|c| if "<>:\"\\|?*".contains(c) || (c as u32) < 32 { '_' } else { c }).collect();
    while fixed.ends_with('.') || fixed.ends_with(' ') {
        fixed.pop();
    }
    if reserved {
        fixed = match fixed.find('.') {
            Some(p) => format!("{}_{}", &fixed[..p], &fixed[p..]),
            None => format!("{fixed}_"),
        };
    }
    if fixed.is_empty() {
        fixed = "_".into();
    }
    let why = if !bad.is_empty() {
        let list: String = bad.iter().filter(|c| (**c as u32) >= 32).map(|c| format!("{c} ")).collect();
        format!("Windows doesn't allow {} in names.", list.trim())
    } else if reserved {
        format!("“{stem}” is a reserved name on Windows.")
    } else {
        "Windows drops a trailing dot or space.".to_string()
    };
    Some((why, fixed))
}

/// Filesystems that follow Windows naming rules.
pub fn windows_fs(fs_type: &str) -> bool {
    matches!(fs_type, "ntfs" | "ntfs3" | "vfat" | "exfat" | "fuseblk")
}

// ------------------------------------------------------------------------------ create

/// Make "New folder" (then "New folder 2", …) in `dir`.
pub fn new_folder(dir: &Path) -> io::Result<PathBuf> {
    new_item(dir, "New folder", |p| fs::create_dir(p))
}

/// Make an empty "Untitled.txt" (then "Untitled 2.txt", …) in `dir`.
pub fn new_file(dir: &Path) -> io::Result<PathBuf> {
    new_item(dir, "Untitled.txt", |p| OpenOptions::new().write(true).create_new(true).open(p).map(drop))
}

fn new_item(dir: &Path, base: &str, make: impl Fn(&Path) -> io::Result<()>) -> io::Result<PathBuf> {
    let (stem, ext) = match base.rfind('.') {
        Some(p) => (&base[..p], &base[p..]),
        None => (base, ""),
    };
    for n in 1..10_000 {
        let name = if n == 1 { base.to_string() } else { format!("{stem} {n}{ext}") };
        let p = dir.join(name);
        match make(&p) {
            Ok(()) => return Ok(p),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("too many items with that name"))
}

// ------------------------------------------------------------------------------ plan

#[derive(Clone, Debug)]
pub struct Conflict {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub src_size: u64,
    pub src_mtime: i64,
    pub dst_size: u64,
    pub dst_mtime: i64,
    /// Both are folders: Replace means merge.
    pub dirs: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Replace,
    Skip,
    KeepBoth,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub src: PathBuf,
    pub dst: PathBuf,
    source_identity: Identity,
    destination_identity: Option<Identity>,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub kind: Kind,
    pub dest: PathBuf,
    pub items: Vec<Item>,
    pub conflicts: Vec<Conflict>,
    /// Names the destination (NTFS, FAT) can't store: (path, reason, fixed name).
    pub bad_names: Vec<(PathBuf, String, String)>,
    pub bytes: u64,
    pub files: u64,
}

#[derive(Default)]
struct Tally {
    bytes: u64,
    files: u64,
    bad: Vec<(PathBuf, String, String)>,
}

fn tally(p: &Path, check_names: bool, progress: &Progress) -> io::Result<Tally> {
    let mut t = Tally::default();
    if progress.cancelled() {
        return Err(cancelled_err());
    }
    if check_names && let Some((why, fixed)) = p.file_name().and_then(|n| n.to_str()).and_then(windows_name) {
        t.bad.push((p.to_path_buf(), why, fixed));
    }
    let m = fs::symlink_metadata(p)?;
    if m.is_dir() {
        let children: Vec<PathBuf> = fs::read_dir(p)?.map(|e| e.map(|e| e.path())).collect::<io::Result<_>>()?;
        let parts: Vec<Tally> = children.par_iter().map(|c| tally(c, check_names, progress)).collect::<io::Result<_>>()?;
        for s in parts {
            t.bytes += s.bytes;
            t.files += s.files;
            t.bad.extend(s.bad);
        }
    } else {
        t.bytes += m.len();
        t.files += 1;
        progress.files_total.fetch_add(1, Ordering::Relaxed);
        progress.bytes_total.fetch_add(m.len(), Ordering::Relaxed);
    }
    Ok(t)
}

fn describe_io(e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::PermissionDenied => "you don't have permission".into(),
        io::ErrorKind::NotFound => "it no longer exists".into(),
        io::ErrorKind::StorageFull => "the drive is full".into(),
        io::ErrorKind::ReadOnlyFilesystem => "the drive is read-only".into(),
        _ => e.to_string(),
    }
}

/// Walk the sources and find everything that could go wrong before writing anything.
pub fn plan(kind: Kind, sources: &[PathBuf], dest: &Path, progress: &Progress) -> Result<Plan, String> {
    progress.set_phase(Phase::Planning);
    let dest_meta = fs::metadata(dest).map_err(|e| format!("Can't open the destination: {}.", describe_io(&e)))?;
    if !dest_meta.is_dir() {
        return Err("The destination isn't a folder.".into());
    }
    let dest_fs = crate::volume::fs_info(dest);
    let check_names = dest_fs.as_ref().is_some_and(|f| windows_fs(f.fs_type));
    let mut items = Vec::new();
    let mut conflicts = Vec::new();
    let mut cross_fs_bytes = 0u64;
    let mut totals = Tally::default();
    let dest_real = fs::canonicalize(dest).map_err(|e| e.to_string())?;
    for src in sources {
        let sm = fs::symlink_metadata(src).map_err(|e| format!("{}: {e}", src.display()))?;
        let Some(name) = src.file_name() else { continue };
        let mut dst = dest.join(name);
        if sm.is_dir() && dest_real.starts_with(fs::canonicalize(src).map_err(|e| e.to_string())?) {
            return Err(format!("You can't {} “{}” into itself.", if kind == Kind::Copy { "copy" } else { "move" }, name.to_string_lossy()));
        }
        if src == &dst || same_file(src, &dst) {
            if kind == Kind::Move {
                continue; // already here
            }
            dst = unique_path(&dst, Naming::Copy);
        }
        let same_dev = sm.dev() == dest_meta.dev();
        let t = if kind == Kind::Move && same_dev {
            // Count the same leaf entries as copies and merged moves, while a
            // same-device rename needs no byte-copy budget.
            let mut t = tally(src, check_names, &Progress::default()).map_err(|e| e.to_string())?;
            progress.files_total.fetch_add(t.files, Ordering::Relaxed);
            t.bytes = 0;
            t
        } else {
            let t = tally(src, check_names, progress).map_err(|e| e.to_string())?;
            cross_fs_bytes += t.bytes;
            t
        };
        totals.bytes += t.bytes;
        totals.files += t.files;
        totals.bad.extend(t.bad);
        if let Ok(dm) = fs::symlink_metadata(&dst)
            && (!same_file(src, &dst) || kind == Kind::Copy) {
                conflicts.push(Conflict {
                    src: src.clone(),
                    dst: dst.clone(),
                    src_size: sm.len(),
                    src_mtime: sm.mtime(),
                    dst_size: dm.len(),
                    dst_mtime: dm.mtime(),
                    dirs: sm.is_dir() && dm.is_dir(),
                });
            }
        items.push(Item { src: src.clone(), source_identity: Identity::of(&sm), destination_identity: fs::symlink_metadata(&dst).ok().map(|m| Identity::of(&m)), dst });
        if progress.cancelled() {
            return Err("Cancelled.".into());
        }
    }
    // A filesystem reporting no size at all (GVfs' FUSE bridge for network places, some
    // FUSE mounts) doesn't know its free space; let the copy find out instead.
    if let Some(f) = dest_fs.as_ref().filter(|f| f.total > 0)
        && cross_fs_bytes > f.free {
            let mut need = String::new();
            crate::fmt::size(cross_fs_bytes, &mut need);
            let mut free = String::new();
            crate::fmt::size(f.free, &mut free);
            return Err(format!("Not enough space: this needs {need} but only {free} is free there."));
        }
    Ok(Plan { kind, dest: dest.to_path_buf(), items, conflicts, bad_names: totals.bad, bytes: totals.bytes, files: totals.files })
}

// ------------------------------------------------------------------------------ execute

/// What a finished copy or move did.
#[derive(Debug, Default)]
pub struct Outcome {
    /// (source, where it is now) for every top-level item that landed.
    pub done: Vec<(PathBuf, PathBuf)>,
    pub journal: Vec<Undo>,
    pub errors: Vec<String>,
    pub cancelled: bool,
    /// `syncfs` finished: safe to unplug or reboot.
    pub synced: bool,
    pub skipped: usize,
}

/// Run a plan. `choices` resolves conflicts by destination path; anything missing uses
/// `default`. With `fix_names`, names the destination can't store are replaced by their
/// fixed form.
pub fn execute(plan: &Plan, choices: &HashMap<PathBuf, Resolution>, default: Resolution, fix_names: bool, progress: &Progress) -> Outcome {
    progress.set_phase(Phase::Running);
    let mut out = Outcome::default();
    let fixes: HashMap<&Path, &str> = if fix_names { plan.bad_names.iter().map(|(p, _, f)| (p.as_path(), f.as_str())).collect() } else { HashMap::new() };
    // A few plain files are made durable one by one; anything bigger syncs the filesystem
    // once (cheaper than thousands of fsyncs, but it also flushes unrelated dirty data).
    let small = plan.items.len() <= SMALL_BATCH && plan.items.iter().all(|i| fs::symlink_metadata(&i.src).is_ok_and(|m| m.is_file()));
    let mut cx = Transfer { kind: plan.kind, fixes: &fixes, progress, links: HashMap::new(), recovery_parent: PathBuf::new(), removals: Vec::new(), stages: HashMap::new(), stage_dirs: Vec::new(), fsync: small };
    for item in &plan.items {
        if fs::symlink_metadata(&item.src).ok().map(|m| Identity::of(&m)) != Some(item.source_identity.clone()) {
            out.errors.push(format!("{} changed since planning; please retry", item.src.display())); continue;
        }
        if fs::symlink_metadata(&item.dst).ok().map(|m| Identity::of(&m)) != item.destination_identity {
            out.errors.push(format!("{} changed since the conflict check; please retry", item.dst.display())); continue;
        }
        let dst = fixes.get(item.src.as_path()).map(|n| item.dst.with_file_name(n)).unwrap_or_else(|| item.dst.clone());
        let choice = choices.get(&item.dst).copied().unwrap_or(default);
        cx.recovery_parent = item.src.parent().unwrap_or(Path::new(".")).to_path_buf();
        match transfer_entry(&mut cx, &item.src, &dst, choice, &mut out) {
            Ok(()) => {},
            Err(e) if e.kind() == io::ErrorKind::Interrupted => { out.cancelled = true; break; },
            Err(e) => out.errors.push(format!("{}: {e}", item.src.display())),
        }
    }
    if out.cancelled && plan.kind == Kind::Move {
        // Sources of cross-filesystem moves are still in place: their copies are undone
        // like any other completed step.
        let journal = std::sync::Arc::new(Mutex::new(std::mem::take(&mut out.journal)));
        if let Err(e) = Undo::Journal(journal.clone()).apply() { out.errors.push(format!("Cancellation recovery incomplete: {e}")); }
        out.journal = std::mem::take(&mut *journal.lock().unwrap_or_else(|e| e.into_inner()));
        if out.journal.is_empty() { out.done.clear(); }
    }
    // Staging folders that ended up holding no replaced entries are not needed.
    for dir in std::mem::take(&mut cx.stage_dirs) { let _ = fs::remove_dir(dir); }
    progress.set_phase(Phase::Syncing);
    // One durability barrier for the whole batch: every copy reaches the disk before any
    // cross-filesystem source is removed.
    let synced = if small {
        // The files themselves were fsynced as they were written; make their names durable.
        let mut dirs: Vec<&Path> = out.done.iter().flat_map(|(s, d)| [s.parent(), d.parent()]).flatten().collect();
        dirs.sort(); dirs.dedup();
        dirs.iter().try_for_each(|d| File::open(d).and_then(|f| f.sync_all()).or_else(|e| if e.kind() == io::ErrorKind::NotFound { Ok(()) } else { Err(e) }))
    } else {
        File::open(&plan.dest).and_then(|d| rustix::fs::syncfs(&d).map_err(Into::into))
    };
    match synced {
        Ok(()) => out.synced = true,
        Err(e) => out.errors.push(format!("Couldn't sync the destination: {e}")),
    }
    if !out.cancelled {
        for removal in std::mem::take(&mut cx.removals) {
            match removal {
                Removal::Source { src, tree, journal } => {
                    let Some(Undo::Checked { path, action, .. }) = out.journal.get_mut(journal) else { continue };
                    if !out.synced {
                        out.errors.push(format!("Copy saved at {}; original retained at {}", path.display(), src.display()));
                        continue;
                    }
                    match remove_copied_source(&src, &tree) {
                        Ok(()) => **action = Undo::Move(vec![(src, path.clone())]),
                        Err(e) => out.errors.push(format!("Copy saved at {}; original retained at {}: {e}", path.display(), src.display())),
                    }
                }
                Removal::EmptiedDir { src, ino, recovery_parent } => {
                    if let Err(e) = retire_emptied_dir(&src, ino, &recovery_parent, &mut out) {
                        out.errors.push(format!("{}: {e}", src.display()));
                    }
                }
            }
        }
    }
    progress.set_phase(Phase::Done);
    out
}

/// State shared by every entry of one [`execute`] run.
struct Transfer<'a> {
    kind: Kind,
    fixes: &'a HashMap<&'a Path, &'a str>,
    progress: &'a Progress,
    /// Hard-linked sources already copied: (dev, ino) → their copy.
    links: HashMap<(u64, u64), PathBuf>,
    recovery_parent: PathBuf,
    /// Cross-filesystem move sources, removed after the batch's single sync.
    removals: Vec<Removal>,
    /// One staging folder per destination folder: (folder as addressed, staging folder,
    /// entries staged so far). Shared so a merge doesn't create a folder per file.
    stages: HashMap<PathBuf, (PathBuf, u32)>,
    /// Every staging folder made, removed at the end if nothing was retained in it.
    stage_dirs: Vec<PathBuf>,
    /// fsync each copied file (small batches; see [`execute`]).
    fsync: bool,
}

/// Up to this many plain files are made durable with targeted fsyncs instead of `syncfs`.
const SMALL_BATCH: usize = 64;

impl Transfer<'_> {
    /// A fresh slot in `parent`'s staging folder: (folder, slot number).
    fn stage(&mut self, parent: &Path) -> io::Result<(PathBuf, u32)> {
        if let Some((dir, n)) = self.stages.get_mut(parent) {
            *n += 1;
            return Ok((dir.clone(), *n));
        }
        let dir = tempfile::Builder::new().prefix(".echofiles-recovery-").tempdir_in(parent)?.keep();
        self.stage_dirs.push(dir.clone());
        self.stages.insert(parent.to_path_buf(), (dir.clone(), 0));
        Ok((dir, 0))
    }
}

/// Source cleanup that waits for the batch's sync, in the order it was queued: a merged
/// folder's children come before the folder itself.
enum Removal {
    /// A copied source; `journal` indexes its `Undo::Checked` copy step, upgraded to a
    /// move once the source is gone.
    Source { src: PathBuf, tree: Vec<(PathBuf, Identity)>, journal: usize },
    /// A merged-away source folder, retired once its children have moved.
    EmptiedDir { src: PathBuf, ino: u64, recovery_parent: PathBuf },
}

/// Keep an emptied merged folder (with its ACLs and attributes) in a recovery directory
/// rather than deleting it; undo restores it before its children.
fn retire_emptied_dir(src: &Path, ino: u64, recovery_parent: &Path, out: &mut Outcome) -> io::Result<()> {
    if fs::read_dir(src)?.next().is_some() { return Ok(()); }
    let recovery = tempfile::Builder::new().prefix(".echofiles-recovery-").tempdir_in(recovery_parent)?;
    let saved = recovery.path().join("directory");
    let mut record = File::create(recovery.path().join("RECOVERY.txt"))?;
    writeln!(record, "Empty merged directory from {:?}; restore directory there before undoing its children.", src.as_os_str().as_bytes())?;
    rename_noreplace(src, &saved)?;
    let recovery = recovery.keep();
    out.journal.push(Undo::Retained { original: src.to_path_buf(), kept: saved.clone() });
    if fs::symlink_metadata(&saved)?.ino() != ino || fs::read_dir(&saved)?.next().is_some() {
        return Err(io::Error::other(format!("Source changed during the move; retained in {} for undo", recovery.display())));
    }
    Ok(())
}

/// Each merged entry commits separately. Existing destinations are retained in a private
/// recovery directory until their journal entry has been undone.
fn transfer_entry(cx: &mut Transfer, src: &Path, dst: &Path, choice: Resolution, out: &mut Outcome) -> io::Result<()> {
    let (kind, progress) = (cx.kind, cx.progress);
    if !progress.proceed() { return Err(cancelled_err()); }
    let sm = fs::symlink_metadata(src)?;
    if same_file(src, dst) { return Err(io::Error::other("source and destination are the same file")); }
    if sm.is_dir() && fs::canonicalize(dst.parent().ok_or_else(|| io::Error::other("missing parent"))?)?.starts_with(fs::canonicalize(src)?) {
        return Err(io::Error::other("cannot transfer a directory into itself"));
    }
    let dm = match fs::symlink_metadata(dst) { Ok(m) => Some(m), Err(e) if e.kind() == io::ErrorKind::NotFound => None, Err(e) => return Err(e) };
    let dst = if dm.is_some() {
        match choice {
            Resolution::Skip => { out.skipped += 1; return Ok(()); },
            Resolution::KeepBoth => unique_path(dst, Naming::Numbered),
            Resolution::Replace if sm.is_dir() && dm.as_ref().is_some_and(|m| m.is_dir()) => {
                // Open the destination without following a substituted symlink. The fd
                // pins the directory for the entire recursive merge.
                use std::os::fd::AsRawFd;
                let dir = OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW).open(dst)?;
                let pinned = PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd()));
                for e in fs::read_dir(src)? {
                    let e = e?;
                    let from = e.path();
                    let name = cx.fixes.get(from.as_path()).map(OsString::from).unwrap_or_else(|| e.file_name());
                    let (start, done_start) = (out.journal.len(), out.done.len());
                    let result = transfer_entry(cx, &from, &pinned.join(&name), choice, out);
                    // Journal paths must survive closing the directory descriptor. Only
                    // what this entry added can mention it (rebasing everything was O(n²)).
                    for step in &mut out.journal[start..] { step.rebase(&pinned, dst); }
                    for (_, landed) in &mut out.done[done_start..] { if let Ok(rel) = landed.strip_prefix(&pinned) { *landed = dst.join(rel); } }
                    result?;
                }
                // Hard links and staging folders addressed through the descriptor get real
                // paths before it closes (a later descriptor may reuse the number).
                for path in cx.links.values_mut() { if let Ok(rel) = path.strip_prefix(&pinned) { *path = dst.join(rel); } }
                cx.stages.retain(|parent, _| !parent.starts_with(&pinned));
                for d in &mut cx.stage_dirs { if let Ok(rel) = d.strip_prefix(&pinned) { *d = dst.join(rel); } }
                if kind == Kind::Move {
                    // Checked once the batch's sources are gone (children of a
                    // cross-filesystem merge are removed after the sync).
                    cx.removals.push(Removal::EmptiedDir { src: src.to_path_buf(), ino: sm.ino(), recovery_parent: cx.recovery_parent.clone() });
                }
                return Ok(());
            },
            Resolution::Replace => dst.to_path_buf(),
        }
    } else { dst.to_path_buf() };
    let parent = dst.parent().ok_or_else(|| io::Error::other("missing destination parent"))?;
    let replaced = choice == Resolution::Replace && dm.is_some();
    // A same-device move commits with one rename and keeps its original until then.
    let direct_move = kind == Kind::Move && sm.dev() == fs::metadata(parent)?.dev();
    if direct_move && !replaced {
        let files = tally(src, false, &Progress::default())?.files;
        rename_noreplace(src, &dst)?;
        out.journal.push(Undo::Checked { path: dst.clone(), identity: Identity::of(&fs::symlink_metadata(&dst)?), action: Box::new(Undo::Move(vec![(src.to_path_buf(), dst.clone())])) });
        out.done.push((src.to_path_buf(), dst));
        progress.files_done.fetch_add(files, Ordering::Relaxed);
        return Ok(());
    }
    // Prepare a complete copy beside the destination before touching an existing entry,
    // in this folder's staging folder; a failed or cancelled copy removes its partial copy.
    let (stage, slot) = cx.stage(parent)?;
    let staged = stage.join(format!("new-{slot}"));
    let backup = stage.join(format!("previous-{slot}"));
    let renamed_files = if direct_move { tally(src, false, &Progress::default())?.files } else { 0 };
    let source_tree = if kind == Kind::Move && !direct_move { Some(snapshot(src)?) } else { None };
    let mut new_links = HashMap::new();
    if !direct_move
        && let Err(e) = copy_tree_inner(src, &staged, cx.fixes, progress, (&cx.links, &mut new_links), cx.fsync) {
            let _ = remove_any(&staged);
            return Err(e);
        }
    if replaced {
        // This record and the previous destination survive a crash. Paths are escaped raw
        // byte strings so non-UTF-8 names can be recovered without guessing.
        let mut record = OpenOptions::new().append(true).create(true).mode(0o600).open(stage.join("RECOVERY.txt"))?;
        writeln!(record, "EchoFiles {:?}\nsource={:?}\ndestination={:?}\nprevious-{slot} holds the destination before replacement; new-{slot} holds a prepared copy.\nDo not discard previous-{slot} until recovery or undo is no longer needed.\n", kind, src.as_os_str().as_bytes(), dst.as_os_str().as_bytes())?;
    }
    if !progress.proceed() { let _ = remove_any(&staged); return Err(cancelled_err()); }
    if replaced {
        // Preserve, never delete, the old entry. Unexpected replacements are recoverable.
        rename_noreplace(&dst, &backup)?;
        if fs::symlink_metadata(&backup).ok().map(|m| Identity::of(&m)) != dm.as_ref().map(Identity::of) {
            if let Err(e) = rename_noreplace(&backup, &dst) { return Err(io::Error::other(format!("Destination changed; retained at {}: {e}", backup.display()))); }
            return Err(io::Error::other("destination changed before replacement; please retry"));
        }
    }
    let from = if direct_move { src } else { &staged };
    if let Err(e) = rename_noreplace(from, &dst) {
        if replaced
            && let Err(restore) = rename_noreplace(&backup, &dst) {
                return Err(io::Error::other(format!("{e}; original retained at {} ({restore})", backup.display())));
            }
        if !direct_move { let _ = remove_any(&staged); }
        return Err(e);
    }
    if replaced {
        out.journal.push(Undo::Retained { original: dst.clone(), kept: backup });
    }
    let action = if direct_move { Undo::Move(vec![(src.to_path_buf(), dst.clone())]) } else { Undo::Copy(vec![dst.clone()]) };
    if let Some(tree) = source_tree {
        // Upgraded to a move once the batch is synced and the source removed.
        cx.removals.push(Removal::Source { src: src.to_path_buf(), tree, journal: out.journal.len() });
    }
    out.journal.push(Undo::Checked { path: dst.clone(), identity: Identity::of(&fs::symlink_metadata(&dst)?), action: Box::new(action) });
    for (key, mut path) in new_links { if let Ok(rel) = path.strip_prefix(&staged) { path = dst.join(rel); } cx.links.insert(key, path); }
    out.done.push((src.to_path_buf(), dst));
    progress.files_done.fetch_add(renamed_files, Ordering::Relaxed);
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path, progress: &Progress) -> io::Result<()> {
    copy_tree_inner(src, dst, &HashMap::new(), progress, (&HashMap::new(), &mut HashMap::new()), false)
}

/// Hard links already copied: (earlier entries' copies, this entry's copies so far).
type Links<'a> = (&'a HashMap<(u64, u64), PathBuf>, &'a mut HashMap<(u64, u64), PathBuf>);

fn copy_tree_inner(src: &Path, dst: &Path, fixes: &HashMap<&Path, &str>, progress: &Progress, links: Links, fsync: bool) -> io::Result<()> {
    if !progress.proceed() {
        return Err(cancelled_err());
    }
    let m = fs::symlink_metadata(src)?;
    let ft = m.file_type();
    if ft.is_symlink() {
        std::os::unix::fs::symlink(fs::read_link(src)?, dst)?;
        set_times(dst, &m)?;
        copy_xattrs(src, dst)?;
        progress.files_done.fetch_add(1, Ordering::Relaxed);
    } else if ft.is_dir() {
        fs::create_dir(dst)?;
        for e in fs::read_dir(src)? {
            let e = e?;
            let from = e.path();
            let name = fixes.get(from.as_path()).map(OsString::from).unwrap_or_else(|| e.file_name());
            copy_tree_inner(&from, &dst.join(name), fixes, progress, (links.0, &mut *links.1), fsync)?;
        }
        fs::set_permissions(dst, fs::Permissions::from_mode(m.mode() & 0o7777))?;
        set_times(dst, &m)?;
        copy_xattrs(src, dst)?;
    } else if ft.is_file() {
        progress.set_current(src);
        let key = (m.dev(), m.ino());
        if let Some(existing) = links.1.get(&key).or_else(|| links.0.get(&key)) { fs::hard_link(existing, dst)?; }
        else { copy_file(src, dst, &m, progress, fsync)?; if m.nlink() > 1 { links.1.insert(key, dst.to_path_buf()); } }
        progress.files_done.fetch_add(1, Ordering::Relaxed);
    }
    else { return Err(io::Error::new(io::ErrorKind::Unsupported, "cannot copy a socket, FIFO or device")); }
    Ok(())
}

fn set_times(p: &Path, m: &fs::Metadata) -> io::Result<()> {
    let ts = rustix::fs::Timestamps {
        last_access: rustix::fs::Timespec { tv_sec: m.atime(), tv_nsec: m.atime_nsec() as _ },
        last_modification: rustix::fs::Timespec { tv_sec: m.mtime(), tv_nsec: m.mtime_nsec() as _ },
    };
    rustix::fs::utimensat(rustix::fs::CWD, p, &ts, rustix::fs::AtFlags::SYMLINK_NOFOLLOW).map_err(Into::into)
}

// Includes user attributes and POSIX ACLs. Failure is an error, so a move retains its
// original when the destination cannot preserve the source's metadata.
fn copy_xattrs(src: &Path, dst: &Path) -> io::Result<()> {
    let src = std::ffi::CString::new(src.as_os_str().as_bytes())?;
    let dst = std::ffi::CString::new(dst.as_os_str().as_bytes())?;
    // SAFETY: all pointers reference live C strings or correctly sized byte buffers.
    unsafe {
        let n = libc::llistxattr(src.as_ptr(), std::ptr::null_mut(), 0);
        if n < 0 {
            let e = io::Error::last_os_error();
            return if e.raw_os_error() == Some(libc::ENOTSUP) { Ok(()) } else { Err(e) };
        }
        let mut names = vec![0u8; n as usize];
        let n = libc::llistxattr(src.as_ptr(), names.as_mut_ptr().cast(), names.len());
        if n < 0 { return Err(io::Error::last_os_error()); }
        for name in names[..n as usize].split(|b| *b == 0).filter(|n| !n.is_empty()) {
            let name = std::ffi::CString::new(name)?;
            let n = libc::lgetxattr(src.as_ptr(), name.as_ptr(), std::ptr::null_mut(), 0);
            if n < 0 { return Err(io::Error::last_os_error()); }
            let mut value = vec![0u8; n as usize];
            let n = libc::lgetxattr(src.as_ptr(), name.as_ptr(), value.as_mut_ptr().cast(), value.len());
            if n < 0 { return Err(io::Error::last_os_error()); }
            if libc::lsetxattr(dst.as_ptr(), name.as_ptr(), value.as_ptr().cast(), n as usize, 0) < 0 { return Err(io::Error::last_os_error()); }
        }
    }
    Ok(())
}

fn copy_file(src: &Path, dst: &Path, m: &fs::Metadata, progress: &Progress, fsync: bool) -> io::Result<()> {
    let mut from = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(src)?;
    if from.metadata()?.ino() != m.ino() || from.metadata()?.dev() != m.dev() { return Err(io::Error::other("source changed while copying")); }
    let mut to = OpenOptions::new().write(true).create_new(true).mode(0o600).open(dst)?;
    let size = m.len();
    let result = (|| {
        // Reflink: instant on btrfs/xfs, shares extents until either copy changes.
        if size > 0 && rustix::fs::ioctl_ficlone(&to, &from).is_ok() {
            progress.bytes_done.fetch_add(size, Ordering::Relaxed);
            return Ok(());
        }
        if size > 64 << 20 {
            let _ = rustix::fs::fadvise(&from, 0, None, rustix::fs::Advice::Sequential);
        }
        let mut kernel = true;
        let mut buf: Vec<u8> = Vec::new();
        loop {
            if !progress.proceed() {
                return Err(cancelled_err());
            }
            let n = if kernel {
                match rustix::fs::copy_file_range(&from, None, &to, None, CHUNK) {
                    Ok(n) => n,
                    Err(rustix::io::Errno::XDEV | rustix::io::Errno::INVAL | rustix::io::Errno::NOSYS | rustix::io::Errno::OPNOTSUPP) => {
                        kernel = false;
                        continue;
                    }
                    Err(e) => return Err(e.into()),
                }
            } else {
                if buf.is_empty() {
                    buf = vec![0; 1 << 20];
                }
                let n = from.read(&mut buf)?;
                to.write_all(&buf[..n])?;
                n
            };
            if n == 0 {
                return Ok(());
            }
            progress.bytes_done.fetch_add(n as u64, Ordering::Relaxed);
        }
    })();
    if let Err(e) = result {
        drop(to);
        let _ = fs::remove_file(dst);
        return Err(e);
    }
    rustix::fs::fchmod(&to, rustix::fs::Mode::from_raw_mode(m.mode() & 0o7777))?;
    let ts = rustix::fs::Timestamps {
        last_access: rustix::fs::Timespec { tv_sec: m.atime(), tv_nsec: m.atime_nsec() as _ },
        last_modification: rustix::fs::Timespec { tv_sec: m.mtime(), tv_nsec: m.mtime_nsec() as _ },
    };
    rustix::fs::futimens(&to, &ts)?;
    copy_xattrs(src, dst)?;
    if fsync { to.sync_all()?; }
    Ok(())
}

fn remove_any(p: &Path) -> io::Result<()> {
    match fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(p),
        Ok(_) => fs::remove_file(p),
        Err(e) => Err(e),
    }
}

// ------------------------------------------------------------------------------ delete

/// Permanently delete; returns one message per failure.
pub fn delete(paths: &[PathBuf], progress: &Progress) -> Vec<String> {
    progress.set_phase(Phase::Running);
    progress.files_total.store(paths.len() as u64, Ordering::Relaxed);
    let errors: Vec<String> = paths
        .par_iter()
        .filter_map(|p| {
            if progress.cancelled() {
                return None;
            }
            progress.set_current(p);
            let r = remove_any(p);
            progress.files_done.fetch_add(1, Ordering::Relaxed);
            r.err().map(|e| format!("Couldn't delete {}: {}.", p.display(), describe_io(&e)))
        })
        .collect();
    progress.set_phase(Phase::Done);
    errors
}

/// Move to the trash; returns what went and one message per failure.
pub fn trash_all(paths: &[PathBuf]) -> (Vec<Trashed>, Vec<(PathBuf, String)>) {
    let mut ok = Vec::new();
    let mut failed = Vec::new();
    for (p, r) in paths.iter().zip(trash::trash_many(paths)) {
        match r {
            Ok(t) => ok.push(t),
            Err(e) => failed.push((p.clone(), describe_io(&e))),
        }
    }
    (ok, failed)
}

// ------------------------------------------------------------------------------ undo

/// One step of the undo journal (Ctrl+Z).
#[derive(Clone, Debug)]
pub enum Undo {
    Rename { from: PathBuf, to: PathBuf },
    /// An entry the operation set aside in a `.echofiles-recovery-*` folder (a replaced
    /// destination, a merged-away folder). Undo puts it back; [`Undo::discard`] deletes it.
    Retained { original: PathBuf, kept: PathBuf },
    Journal(std::sync::Arc<Mutex<Vec<Undo>>>),
    Checked { path: PathBuf, identity: Identity, action: Box<Undo> },
    /// (original, where it is now)
    Move(Vec<(PathBuf, PathBuf)>),
    Trash(Vec<Trashed>),
    /// Copies that were made.
    Copy(Vec<PathBuf>),
    Create(PathBuf),
}

impl Undo {
    pub fn retryable(self) -> Self {
        let steps = match self {
            Undo::Copy(paths) => paths.into_iter().map(|p| Undo::Copy(vec![p])).collect(),
            Undo::Move(pairs) => pairs.into_iter().map(|p| Undo::Move(vec![p])).collect(),
            Undo::Trash(items) => items.into_iter().map(|p| Undo::Trash(vec![p])).collect(),
            other => return other,
        };
        Undo::Journal(std::sync::Arc::new(Mutex::new(steps)))
    }

    fn rebase(&mut self, old: &Path, new: &Path) {
        let rebase = |p: &mut PathBuf| { if let Ok(rel) = p.strip_prefix(old) { *p = new.join(rel); } };
        match self {
            Undo::Rename { from, to } => { rebase(from); rebase(to); },
            Undo::Retained { original, kept } => { rebase(original); rebase(kept); },
            Undo::Move(pairs) => for (a, b) in pairs { rebase(a); rebase(b); },
            Undo::Copy(paths) => for p in paths { rebase(p); },
            Undo::Checked { path, action, .. } => { rebase(path); action.rebase(old, new); },
            _ => {},
        }
    }

    /// "Undo rename" etc. for the toast and menus.
    pub fn label(&self) -> &'static str {
        match self {
            Undo::Journal(_) => "transfer",
            Undo::Checked { action, .. } => action.label(),
            Undo::Rename { .. } => "rename",
            Undo::Retained { .. } => "replacement",
            Undo::Move(_) => "move",
            Undo::Trash(_) => "move to Trash",
            Undo::Copy(_) => "copy",
            Undo::Create(_) => "new item",
        }
    }

    /// Reverse the step. Copies and new items go to the trash rather than being deleted.
    pub fn apply(&self) -> Result<String, String> {
        let err = |p: &Path, e: io::Error| format!("Couldn't undo for {}: {}.", p.display(), describe_io(&e));
        match self {
            Undo::Checked { path, identity, action } => {
                let actual = fs::symlink_metadata(path).map_err(|e| err(path, e))?;
                if actual.dev() != identity.dev || actual.ino() != identity.ino { return Err(format!("{} was replaced after the operation; leaving it untouched", path.display())); }
                action.apply()
            }
            Undo::Journal(journal) => {
                let mut journal = journal.lock().unwrap();
                let mut kept_in = Vec::new();
                let result = (|| {
                    while let Some(step) = journal.last() {
                        // A run of copies (or moves) at the end is undone as one batch: one
                        // Trash lookup and one sync rather than one per entry.
                        if let Some(kind) = step.batch() {
                            let start = journal.iter().rposition(|s| s.batch() != Some(kind) && !s.is_backup()).map_or(0, |i| i + 1);
                            let results = undo_batch(&journal[start..], kind);
                            let mut first_error = None;
                            let failed: Vec<Undo> = journal.drain(start..).zip(results).filter_map(|(step, r)| r.err().map(|e| { first_error.get_or_insert(e); step })).collect();
                            journal.extend(failed);
                            if let Some(e) = first_error { return Err(e); }
                            continue;
                        }
                        if let Undo::Retained { kept, .. } = step { kept_in.extend(kept.parent().map(Path::to_path_buf)); }
                        step.apply()?;
                        journal.pop();
                    }
                    Ok(())
                })();
                tidy_recovery(kept_in);
                result.map(|()| "Transfer undone".into())
            }
            Undo::Rename { from, to } => {
                rename_noreplace(to, from).map_err(|e| err(to, e))?;
                Ok(format!("Renamed back to {}", from.file_name().unwrap_or_default().to_string_lossy()))
            }
            Undo::Retained { original, kept } => {
                rename_noreplace(kept, original).map_err(|e| err(kept, e))?;
                tidy_recovery(kept.parent().map(Path::to_path_buf));
                Ok(format!("Restored {}", original.file_name().unwrap_or_default().to_string_lossy()))
            }
            Undo::Move(pairs) => {
                let mut n = 0;
                for (orig, now) in pairs.iter().rev() {
                    match rename_noreplace(now, orig) {
                        Ok(()) => n += 1,
                        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
                            let p = Progress::default();
                            let parent = orig.parent().ok_or_else(|| "Missing restore parent".to_string())?;
                            let source_tree = snapshot(now).map_err(|e| err(now, e))?;
                            let stage = tempfile::Builder::new().prefix(".echofiles-restore-").tempdir_in(parent).map_err(|e| err(orig, e))?;
                            let prepared = stage.path().join("restored");
                            copy_tree(now, &prepared, &p).map_err(|e| err(now, e))?;
                            // One sync before removing the moved copy is enough: until then
                            // that copy still exists.
                            rename_noreplace(&prepared, orig).map_err(|e| err(orig, e))?;
                            rustix::fs::syncfs(&File::open(parent).map_err(|e| err(orig, e))?).map_err(|e| err(orig, e.into()))?;
                            remove_copied_source(now, &source_tree).map_err(|e| err(now, e))?;
                            n += 1;
                        }
                        Err(e) => return Err(err(now, e)),
                    }
                }
                Ok(format!("Moved {n} {} back", if n == 1 { "item" } else { "items" }))
            }
            Undo::Trash(items) => {
                for t in items.iter().rev() {
                    trash::restore(t).map_err(|e| err(&t.original, e))?;
                }
                Ok(format!("Restored {} {} from the Trash", items.len(), if items.len() == 1 { "item" } else { "items" }))
            }
            Undo::Copy(made) => {
                for (p, r) in made.iter().zip(trash::trash_many(made)) {
                    r.map_err(|e| err(p, e))?;
                }
                Ok(format!("Moved {} {} to the Trash", made.len(), if made.len() == 1 { "copy" } else { "copies" }))
            }
            Undo::Create(p) => {
                let r = if p.is_dir() { fs::remove_dir(p) } else if fs::metadata(p).is_ok_and(|m| m.len() == 0) { fs::remove_file(p) } else { trash::trash(p).map(drop) };
                r.map_err(|e| err(p, e))?;
                Ok(format!("Removed {}", p.file_name().unwrap_or_default().to_string_lossy()))
            }
        }
    }
}

impl Undo {
    /// Which batch this step can join: copies (undone by trashing) or moves.
    fn batch(&self) -> Option<Kind> {
        match self {
            Undo::Copy(_) => Some(Kind::Copy),
            Undo::Move(_) => Some(Kind::Move),
            Undo::Checked { action, .. } => match **action { Undo::Copy(_) => Some(Kind::Copy), Undo::Move(_) => Some(Kind::Move), _ => None },
            _ => None,
        }
    }

    /// A replaced destination set aside during a copy or move. It is restored after the
    /// entry that replaced it is undone, so it can join that batch. (A merged-away folder
    /// is not: it has to be back before its children return.)
    fn is_backup(&self) -> bool {
        matches!(self, Undo::Retained { kept, .. } if kept.file_name().is_some_and(|n| n.as_bytes().starts_with(b"previous-")))
    }

    /// Delete what this step set aside for undo; the operation can then no longer be
    /// undone. Called when an undo entry is dropped (session end, history limit).
    pub fn discard(&self) {
        match self {
            Undo::Retained { kept, .. } => {
                let _ = remove_any(kept);
                tidy_recovery(kept.parent().map(Path::to_path_buf));
            }
            Undo::Journal(journal) => {
                let journal = journal.lock().unwrap_or_else(|e| e.into_inner());
                let mut dirs = Vec::new();
                for step in journal.iter() {
                    if let Undo::Retained { kept, .. } = step { let _ = remove_any(kept); dirs.extend(kept.parent().map(Path::to_path_buf)); }
                }
                tidy_recovery(dirs);
            }
            Undo::Checked { action, .. } => action.discard(),
            _ => {}
        }
    }
}

/// What Redo (Ctrl+Shift+Z) does again after an undo. Read from the [`Undo`] step before
/// it runs (see [`Undo::redo`]); applying it makes a fresh undo step.
#[derive(Clone, Debug)]
pub enum Redo {
    /// Rename `from` to `to` again, if `from` is still the (device, inode) that was renamed.
    Rename { from: PathBuf, to: PathBuf, dev: u64, ino: u64 },
    /// Put items back in the Trash: where they were restored to, and the (device, inode)
    /// they had in the Trash, so nothing that took their place since goes instead.
    Trash(Vec<(PathBuf, u64, u64)>),
    /// Make the empty folder or file again.
    Create { path: PathBuf, dir: bool },
}

impl Undo {
    /// How to redo this step once it has been undone, if it can be. A new item that had
    /// content when it was undone went to the Trash: making it again empty would not be
    /// the same thing, so that one has no redo.
    pub fn redo(&self) -> Option<Redo> {
        match self {
            Undo::Rename { from, to } => {
                let m = fs::symlink_metadata(to).ok()?;
                Some(Redo::Rename { from: from.clone(), to: to.clone(), dev: m.dev(), ino: m.ino() })
            }
            Undo::Trash(items) => {
                let back: Vec<(PathBuf, u64, u64)> = items.iter().filter_map(|t| fs::symlink_metadata(&t.file).ok().map(|m| (t.original.clone(), m.dev(), m.ino()))).collect();
                (!back.is_empty()).then_some(Redo::Trash(back))
            }
            Undo::Create(p) => {
                let m = fs::symlink_metadata(p).ok()?;
                (m.is_dir() || m.len() == 0).then(|| Redo::Create { path: p.clone(), dir: m.is_dir() })
            }
            Undo::Checked { action, .. } => action.redo(),
            _ => None,
        }
    }
}

impl Redo {
    /// "Redo rename" etc. for menus.
    pub fn label(&self) -> &'static str {
        match self {
            Redo::Rename { .. } => "rename",
            Redo::Trash(_) => "move to Trash",
            Redo::Create { .. } => "new item",
        }
    }

    /// Do the step again: the undo step for what was done, and a problem if part of it
    /// couldn't be (both can be set when only some items went).
    pub fn apply(&self) -> (Option<Undo>, Option<String>) {
        let err = |p: &Path, e: io::Error| format!("Couldn't redo for {}: {}.", p.display(), describe_io(&e));
        match self {
            Redo::Rename { from, to, dev, ino } => match fs::symlink_metadata(from) {
                Ok(m) if m.dev() != *dev || m.ino() != *ino => (None, Some(format!("{} was replaced after the undo; leaving it where it is.", from.display()))),
                Err(e) => (None, Some(err(from, e))),
                Ok(_) => match rename_noreplace(from, to) {
                    Ok(()) => (Some(Undo::Rename { from: from.clone(), to: to.clone() }), None),
                    Err(e) => (None, Some(err(from, e))),
                },
            },
            Redo::Trash(items) => {
                let mut problem = None;
                let paths: Vec<PathBuf> = items
                    .iter()
                    .filter(|(p, dev, ino)| match fs::symlink_metadata(p) {
                        Ok(m) if m.dev() == *dev && m.ino() == *ino => true,
                        Ok(_) => {
                            problem.get_or_insert_with(|| format!("{} was replaced after the undo; leaving it where it is.", p.display()));
                            false
                        }
                        Err(e) => {
                            problem.get_or_insert_with(|| err(p, e));
                            false
                        }
                    })
                    .map(|(p, _, _)| p.clone())
                    .collect();
                let (ok, failed) = trash_all(&paths);
                if let Some((p, why)) = failed.first() {
                    problem.get_or_insert_with(|| format!("Couldn't redo for {}: {why}.", p.display()));
                }
                ((!ok.is_empty()).then_some(Undo::Trash(ok)), problem)
            }
            Redo::Create { path, dir } => {
                let r = if *dir { fs::create_dir(path) } else { OpenOptions::new().write(true).create_new(true).open(path).map(drop) };
                match r {
                    Ok(()) => (Some(Undo::Create(path.clone())), None),
                    Err(e) => (None, Some(err(path, e))),
                }
            }
        }
    }
}

/// Remove recovery folders that hold nothing but their `RECOVERY.txt` note. Each folder is
/// listed once, however many of its entries were restored.
fn tidy_recovery(dirs: impl IntoIterator<Item = PathBuf>) {
    let mut dirs: Vec<PathBuf> = dirs.into_iter().filter(|d| d.file_name().is_some_and(|n| n.as_bytes().starts_with(b".echofiles-recovery-"))).collect();
    dirs.sort();
    dirs.dedup();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        if entries.flatten().all(|e| e.file_name() == "RECOVERY.txt") {
            let _ = fs::remove_file(dir.join("RECOVERY.txt"));
            let _ = fs::remove_dir(&dir);
        }
    }
}

/// Undo a run of copy or move steps together. Results line up with `steps`.
fn undo_batch(steps: &[Undo], kind: Kind) -> Vec<Result<(), String>> {
    let err = |p: &Path, e: io::Error| format!("Couldn't undo for {}: {}.", p.display(), describe_io(&e));
    // Checked steps must still find the entry the operation made.
    let checked: Vec<Result<&Undo, String>> = steps.iter().map(|s| match s {
        Undo::Checked { path, identity, action } => {
            let actual = fs::symlink_metadata(path).map_err(|e| err(path, e))?;
            if actual.dev() != identity.dev || actual.ino() != identity.ino { return Err(format!("{} was replaced after the operation; leaving it untouched", path.display())); }
            Ok(&**action)
        }
        other => Ok(other),
    }).collect();
    // Replaced destinations go back once the entries that replaced them are gone.
    let restore_backups = |results: &mut Vec<Result<(), String>>| {
        let mut dirs = Vec::new();
        for (i, step) in steps.iter().enumerate().rev() {
            if let Undo::Retained { original, kept } = step {
                results[i] = rename_noreplace(kept, original).map_err(|e| err(kept, e));
                dirs.extend(kept.parent().map(Path::to_path_buf));
            }
        }
        tidy_recovery(dirs);
    };
    let mut results: Vec<Result<(), String>> = checked.iter().map(|c| c.as_ref().map(|_| ()).map_err(Clone::clone)).collect();
    if kind == Kind::Copy {
        let mut owner = Vec::new();
        let mut paths = Vec::new();
        for (i, c) in checked.iter().enumerate() {
            if let Ok(Undo::Copy(made)) = c { for p in made { owner.push(i); paths.push(p.clone()); } }
        }
        for ((i, p), r) in owner.into_iter().zip(&paths).zip(trash::trash_many(&paths)) {
            if let Err(e) = r && results[i].is_ok() { results[i] = Err(err(p, e)); }
        }
        restore_backups(&mut results);
        return results;
    }
    // Moves: renames first; entries on another filesystem are copied back and published,
    // then one sync per filesystem, then the moved copies are removed.
    let mut pending: Vec<(usize, PathBuf, Vec<(PathBuf, Identity)>, PathBuf)> = Vec::new();
    for (i, c) in checked.iter().enumerate().rev() {
        let Ok(Undo::Move(pairs)) = c else { continue };
        for (orig, now) in pairs.iter().rev() {
            if results[i].is_err() { break; }
            let r = match rename_noreplace(now, orig) {
                Ok(()) => Ok(()),
                Err(e) if e.raw_os_error() == Some(libc::EXDEV) => (|| {
                    let parent = orig.parent().ok_or_else(|| "Missing restore parent".to_string())?;
                    let tree = snapshot(now).map_err(|e| err(now, e))?;
                    let stage = tempfile::Builder::new().prefix(".echofiles-restore-").tempdir_in(parent).map_err(|e| err(orig, e))?;
                    let prepared = stage.path().join("restored");
                    copy_tree(now, &prepared, &Progress::default()).map_err(|e| err(now, e))?;
                    rename_noreplace(&prepared, orig).map_err(|e| err(orig, e))?;
                    pending.push((i, now.clone(), tree, parent.to_path_buf()));
                    Ok(())
                })(),
                Err(e) => Err(err(now, e)),
            };
            if let Err(e) = r { results[i] = Err(e); }
        }
    }
    let mut synced: HashMap<u64, Result<(), String>> = HashMap::new();
    for (i, now, tree, parent) in pending {
        let dev = fs::metadata(&parent).map(|m| m.dev()).unwrap_or(0);
        let sync = synced.entry(dev).or_insert_with(|| File::open(&parent).and_then(|f| rustix::fs::syncfs(&f).map_err(Into::into)).map_err(|e| err(&parent, e))).clone();
        let r = sync.and_then(|()| remove_copied_source(&now, &tree).map_err(|e| err(&now, e)));
        if let Err(e) = r && results[i].is_ok() { results[i] = Err(e); }
    }
    restore_backups(&mut results);
    results
}

// ------------------------------------------------------------------------------ sizes

/// Totals for a folder tree, filled in as the walk goes (for Properties and the status bar).
#[derive(Default, Debug)]
pub struct DirSize {
    pub bytes: AtomicU64,
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub done: AtomicBool,
}

/// Walk `paths` in parallel without following symlinks; stops early when `cancel` is set.
pub fn measure(paths: &[PathBuf], out: &DirSize, cancel: &AtomicBool) {
    fn walk(p: &Path, out: &DirSize, cancel: &AtomicBool) {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let Ok(m) = fs::symlink_metadata(p) else { return };
        if m.is_dir() {
            out.dirs.fetch_add(1, Ordering::Relaxed);
            let kids: Vec<PathBuf> = fs::read_dir(p).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
            kids.par_iter().for_each(|k| walk(k, out, cancel));
        } else {
            out.files.fetch_add(1, Ordering::Relaxed);
            out.bytes.fetch_add(m.len(), Ordering::Relaxed);
        }
    }
    paths.par_iter().for_each(|p| walk(p, out, cancel));
    out.done.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ef-ops-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn names() {
        assert_eq!(windows_name("a:b?.txt").unwrap().1, "a_b_.txt");
        assert_eq!(windows_name("con.txt").unwrap().1, "con_.txt");
        assert_eq!(windows_name("notes. ").unwrap().1, "notes");
        assert!(windows_name("fine name.txt").is_none());
        assert!(name_problem("a/b").is_some());
    }

    #[test]
    fn unique_names() {
        let d = tmp("uniq");
        fs::write(d.join("photo.jpg"), b"").unwrap();
        assert_eq!(unique_path(&d.join("photo.jpg"), Naming::Numbered), d.join("photo (2).jpg"));
        assert_eq!(unique_path(&d.join("photo.jpg"), Naming::Copy), d.join("photo (copy).jpg"));
        assert_eq!(new_folder(&d).unwrap(), d.join("New folder"));
        assert_eq!(new_folder(&d).unwrap(), d.join("New folder 2"));
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn undo_then_redo_rename_and_new_items() {
        let d = tmp("redo");
        let (a, b) = (d.join("a.txt"), d.join("b.txt"));
        fs::write(&a, b"x").unwrap();
        let to = rename(&a, "b.txt").unwrap();
        assert_eq!(to, b);
        let step = Undo::Rename { from: a.clone(), to: b.clone() };
        let again = step.redo().unwrap();
        step.apply().unwrap();
        assert!(a.exists() && !b.exists());
        let (undo, problem) = again.apply();
        assert!(problem.is_none());
        assert!(b.exists() && !a.exists());
        // Redoing makes a fresh undo step that undoes again.
        undo.unwrap().apply().unwrap();
        assert!(a.exists());
        // A rename can't be redone over something that took the name meanwhile.
        fs::write(&b, b"other").unwrap();
        let (undo, problem) = again.apply();
        assert!(undo.is_none() && problem.is_some());
        assert_eq!(fs::read(&b).unwrap(), b"other");
        // Nor can it rename a different file that replaced the source.
        fs::remove_file(&b).unwrap();
        fs::write(d.join("c.txt"), b"replacement").unwrap();
        fs::rename(d.join("c.txt"), &a).unwrap();
        let (undo, problem) = again.apply();
        assert!(undo.is_none() && problem.is_some() && a.exists() && !b.exists());

        let folder = new_folder(&d).unwrap();
        let step = Undo::Create(folder.clone());
        let again = step.redo().unwrap();
        step.apply().unwrap();
        assert!(!folder.exists());
        let (undo, problem) = again.apply();
        assert!(folder.is_dir() && problem.is_none() && undo.is_some());
        // A new file that got content isn't recreated empty.
        let file = new_file(&d).unwrap();
        fs::write(&file, b"typed").unwrap();
        assert!(Undo::Create(file).redo().is_none());
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn copy_with_conflicts_and_undo() {
        let d = tmp("copy");
        let (a, b) = (d.join("a"), d.join("b"));
        fs::create_dir_all(a.join("sub")).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("sub/x.txt"), b"hello").unwrap();
        fs::write(a.join("y.txt"), b"new").unwrap();
        fs::write(b.join("y.txt"), b"old").unwrap();
        let p = Progress::default();
        let p1 = plan(Kind::Copy, &[a.join("sub"), a.join("y.txt")], &b, &p).unwrap();
        assert_eq!(p1.conflicts.len(), 1);
        assert_eq!(p1.bytes, 8);
        let out = execute(&p1, &HashMap::new(), Resolution::KeepBoth, false, &p);
        assert!(out.errors.is_empty() && out.synced);
        assert_eq!(fs::read(b.join("sub/x.txt")).unwrap(), b"hello");
        assert_eq!(fs::read(b.join("y (2).txt")).unwrap(), b"new");
        assert_eq!(fs::read(b.join("y.txt")).unwrap(), b"old");
        // Copy into the same folder makes "(copy)".
        let plan2 = plan(Kind::Copy, &[b.join("y.txt")], &b, &p).unwrap();
        let out2 = execute(&plan2, &HashMap::new(), Resolution::Skip, false, &p);
        assert_eq!(out2.done[0].1, b.join("y (copy).txt"));
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn move_and_undo() {
        let d = tmp("move");
        let (a, b) = (d.join("a"), d.join("b"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("f.txt"), b"1").unwrap();
        let p = Progress::default();
        let pm = plan(Kind::Move, &[a.join("f.txt")], &b, &p).unwrap();
        let out = execute(&pm, &HashMap::new(), Resolution::Skip, false, &p);
        assert!(b.join("f.txt").exists() && !a.join("f.txt").exists());
        Undo::Move(out.done).apply().unwrap();
        assert!(a.join("f.txt").exists() && !b.join("f.txt").exists());
        assert!(plan_into_self(&a));
        fs::remove_dir_all(&d).unwrap();
    }

    fn plan_into_self(a: &Path) -> bool {
        fs::create_dir_all(a.join("inner")).unwrap();
        plan(Kind::Move, &[a.to_path_buf()], &a.join("inner"), &Progress::default()).is_err()
    }

    #[test]
    fn rename_refuses_to_clobber() {
        let d = tmp("rename");
        fs::write(d.join("a"), b"").unwrap();
        fs::write(d.join("b"), b"").unwrap();
        assert!(rename(&d.join("a"), "b").is_err());
        assert_eq!(rename(&d.join("a"), "c").unwrap(), d.join("c"));
        fs::remove_dir_all(&d).unwrap();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity { dev: u64, ino: u64, len: u64, mtime: i64, nanos: i64 }
impl Identity {
    fn of(m: &fs::Metadata) -> Self { Self { dev: m.dev(), ino: m.ino(), len: m.len(), mtime: m.mtime(), nanos: m.mtime_nsec() } }
}

fn snapshot(root: &Path) -> io::Result<Vec<(PathBuf, Identity)>> {
    fn visit(root: &Path, relative: &Path, entries: &mut Vec<(PathBuf, Identity)>) -> io::Result<()> {
        // `root.join("")` would add a trailing slash, which fails on a file.
        let path = if relative.as_os_str().is_empty() { root.to_path_buf() } else { root.join(relative) };
        let m = fs::symlink_metadata(&path)?;
        entries.push((relative.to_path_buf(), Identity::of(&m)));
        if m.is_dir() { for entry in fs::read_dir(path)? { let entry = entry?; visit(root, &relative.join(entry.file_name()), entries)?; } }
        Ok(())
    }
    let mut entries = Vec::new(); visit(root, Path::new(""), &mut entries)?; entries.sort_by(|a,b| a.0.cmp(&b.0)); Ok(entries)
}

/// Claim the source name atomically before deleting it. A changed source is restored (or
/// retained in an explicitly reported recovery folder), never recursively removed.
fn remove_copied_source(src: &Path, expected: &[(PathBuf, Identity)]) -> io::Result<()> {
    let parent = src.parent().ok_or_else(|| io::Error::other("missing source parent"))?;
    let staging = tempfile::Builder::new().prefix(".echofiles-source-").tempdir_in(parent)?;
    let saved = staging.path().join("original");
    rename_noreplace(src, &saved)?;
    let staging = staging.keep();
    let result = snapshot(&saved);
    if result.as_ref().is_ok_and(|actual| actual == expected) {
        remove_any(&saved)?;
        fs::remove_dir(staging)?;
        return Ok(());
    }
    match rename_noreplace(&saved, src) {
        Ok(()) => { let _ = fs::remove_dir(staging); Err(io::Error::other("source changed during transfer; original retained")) },
        Err(e) => Err(io::Error::other(format!("source changed; original retained at {}: {e}", saved.display()))),
    }
}

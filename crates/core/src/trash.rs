//! The freedesktop Trash (build plan §2.5): a rename into the trash on the same filesystem,
//! never a copy. Home files go to `$XDG_DATA_HOME/Trash`; files on other volumes (NTFS
//! partitions, USB drives) go to `$topdir/.Trash/$uid` or `$topdir/.Trash-$uid`.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::io::{self, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// Where one item went, enough to put it back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trashed {
    pub original: PathBuf,
    /// The item inside `Trash/files`.
    pub file: PathBuf,
    /// Its `Trash/info/*.trashinfo`.
    pub info: PathBuf,
}

fn uid() -> u32 {
    unsafe { libc::getuid() }
}

/// `$XDG_DATA_HOME/Trash`.
pub fn home_trash() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into())).join(".local/share"));
    data.join("Trash")
}

/// Device of `p` or of its nearest existing ancestor.
fn dev_of(p: &Path) -> Option<u64> {
    let mut cur = Some(p);
    while let Some(c) = cur {
        if let Ok(m) = fs::metadata(c) {
            return Some(m.dev());
        }
        cur = c.parent();
    }
    None
}

/// The mount point holding `p`: the highest ancestor still on the same device.
pub fn topdir(p: &Path) -> PathBuf {
    let Ok(dev) = fs::symlink_metadata(p).map(|m| m.dev()) else { return PathBuf::from("/") };
    let mut top = p.to_path_buf();
    while let Some(parent) = top.parent() {
        match fs::metadata(parent) {
            Ok(m) if m.dev() == dev => top = parent.to_path_buf(),
            _ => break,
        }
    }
    top
}

/// The trash directory for `path` and the `Path=` value its info file records.
fn trash_for(path: &Path) -> io::Result<(PathBuf, PathBuf, File)> {
    let dev = fs::symlink_metadata(path)?.dev();
    let home = home_trash();
    if dev_of(&home) == Some(dev) {
        if let Some(parent) = home.parent() { fs::create_dir_all(parent)?; }
        let fd = secure_dir(&home)?;
        return Ok((home, path.to_path_buf(), fd));
    }
    let top = topdir(path);
    let rel = path.strip_prefix(&top).map(Path::to_path_buf).unwrap_or_else(|_| path.to_path_buf());
    // $topdir/.Trash/$uid, only when .Trash is a real sticky directory (spec).
    let shared = top.join(".Trash");
    if let Ok(shared_fd) = OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW).open(&shared) {
        let m = shared_fd.metadata()?;
        if m.is_dir() && m.mode() & 0o1000 != 0 {
            let dir = shared.join(uid().to_string());
            let pinned = PathBuf::from(format!("/proc/self/fd/{}", shared_fd.as_raw_fd())).join(uid().to_string());
            if let Ok(fd) = secure_dir(&pinned) { return Ok((dir, rel, fd)); }
        }
    }
    let dir = top.join(format!(".Trash-{}", uid()));
    let fd = secure_dir(&dir)?;
    Ok((dir, rel, fd))
}

/// Percent-encode a path for `Path=` (RFC 2396, keeping `/`).
fn encode(p: &Path) -> String {
    let mut out = String::new();
    for &b in p.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/!$&'()*+,;=:@".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode(s: &str) -> PathBuf {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
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
    PathBuf::from(OsString::from_vec(out))
}

/// Local time as `YYYY-MM-DDThh:mm:ss`.
fn now_local() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    // SAFETY: localtime_r writes only into `tm`.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
}

/// `name`, then `name.2`, `name.3`… keeping the extension last (`report.2.pdf`).
fn numbered(name: &[u8], n: usize) -> Vec<u8> {
    if n == 1 {
        return name.to_vec();
    }
    match name.iter().rposition(|&c| c == b'.').filter(|&p| p > 0) {
        Some(p) => [&name[..p], format!(".{n}").as_bytes(), &name[p..]].concat(),
        None => [name, format!(".{n}").as_bytes()].concat(),
    }
}

/// Move `path` to the trash.
pub fn trash(path: &Path) -> io::Result<Trashed> {
    trash_many(&[path.to_path_buf()]).pop().unwrap_or_else(|| Err(io::Error::other("nothing to trash")))
}

/// One filesystem's trash, resolved and validated once per batch.
struct Bin {
    files: PathBuf,
    info: PathBuf,
    /// The mount point `Path=` is relative to (`None`: the home trash records full paths).
    top: Option<PathBuf>,
    /// Descriptors pinning `files` and `info` (and the trash root) for the whole batch.
    _root: File,
    files_fd: File,
    info_fd: File,
}

impl Bin {
    fn pinned(fd: &File) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", fd.as_raw_fd()))
    }
}

fn bin_for(path: &Path) -> io::Result<Bin> {
    let (dir, recorded, root) = trash_for(path)?;
    let top = (recorded != path).then(|| topdir(path));
    let pinned = Bin::pinned(&root);
    let files_fd = secure_dir(&pinned.join("files"))?;
    let info_fd = secure_dir(&pinned.join("info"))?;
    Ok(Bin { files: dir.join("files"), info: dir.join("info"), top, _root: root, files_fd, info_fd })
}

/// Move many paths to their trash. Each filesystem's trash is found once, numbered names
/// continue from the last one used (not from 1 for every item), every `.trashinfo` is
/// written before any item moves, and the batch has one durability barrier. Results are in
/// input order.
pub fn trash_many(paths: &[PathBuf]) -> Vec<io::Result<Trashed>> {
    let small = paths.len() <= 64;
    let date = now_local();
    let mut bins: std::collections::HashMap<u64, Result<Bin, (io::ErrorKind, String)>> = Default::default();
    let mut next: std::collections::HashMap<(u64, Vec<u8>), usize> = Default::default();
    // Phase 1: reserve a name and write the info file for every item.
    let mut reserved: Vec<io::Result<(u64, PathBuf, OsString, OsString)>> = Vec::with_capacity(paths.len());
    for path in paths {
        reserved.push((|| {
            let path = if path.is_absolute() { path.clone() } else { std::env::current_dir()?.join(path) };
            let dev = fs::symlink_metadata(&path)?.dev();
            let bin = bins.entry(dev).or_insert_with(|| bin_for(&path).map_err(|e| (e.kind(), e.to_string())));
            let bin = bin.as_ref().map_err(|(k, m)| io::Error::new(*k, m.clone()))?;
            let base = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "can't trash a filesystem root"))?.as_bytes().to_vec();
            let recorded = bin.top.as_ref().and_then(|t| path.strip_prefix(t).ok()).unwrap_or(&path);
            let body = format!("[Trash Info]\nPath={}\nDeletionDate={}\n", encode(recorded), date);
            let (files, info) = (Bin::pinned(&bin.files_fd), Bin::pinned(&bin.info_fd));
            let info_of = |n: usize| { let mut i = OsString::from_vec(numbered(&base, n)); i.push(".trashinfo"); i };
            let taken = |n: u32| fs::symlink_metadata(files.join(OsString::from_vec(numbered(&base, n as usize)))).is_ok() || fs::symlink_metadata(info.join(info_of(n as usize))).is_ok();
            let mut from = *next.get(&(dev, base.clone())).unwrap_or(&1);
            loop {
                let n = crate::ops::free_number(from as u32, taken) as usize;
                if n >= 1_000_000 { return Err(io::Error::other("too many items with this name in the trash")); }
                let name = OsString::from_vec(numbered(&base, n));
                // The info file is created exclusively first: it reserves the name (spec).
                match OpenOptions::new().write(true).create_new(true).mode(0o600).open(info.join(info_of(n))) {
                    Ok(mut f) => {
                        f.write_all(body.as_bytes())?;
                        if small { f.sync_all()?; }
                        let info_name = info_of(n);
                        next.insert((dev, base.clone()), n + 1);
                        return Ok((dev, path, name, info_name));
                    }
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => from = n + 1,
                    Err(e) => return Err(e),
                }
            }
        })());
    }
    // Phase 2: every info file is durable before any item moves.
    for bin in bins.values().flatten() {
        let _ = if small { bin.info_fd.sync_all() } else { rustix::fs::syncfs(&bin.info_fd).map_err(io::Error::from) };
    }
    // Phase 3: move the items.
    reserved.into_iter().map(|r| {
        let (dev, path, name, info_name) = r?;
        let Some(Ok(bin)) = bins.get(&dev) else { return Err(io::Error::other("trash unavailable")) };
        let info_path = Bin::pinned(&bin.info_fd).join(&info_name);
        if let Err(e) = crate::ops::rename_noreplace(&path, &Bin::pinned(&bin.files_fd).join(&name)) {
            let _ = fs::remove_file(&info_path);
            return Err(e);
        }
        Ok(Trashed { original: path, file: bin.files.join(name), info: bin.info.join(PathBuf::from(info_name)) })
    }).collect()
}

/// Put a trashed item back. Returns where it landed (a new name if the original is taken).
pub fn restore(t: &Trashed) -> io::Result<PathBuf> {
    let mut dest = t.original.clone();
    if fs::symlink_metadata(&dest).is_ok() {
        dest = crate::ops::unique_path(&dest, crate::ops::Naming::Numbered);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    crate::ops::rename_noreplace(&t.file, &dest)?;
    let _ = fs::remove_file(&t.info);
    Ok(dest)
}

/// An item in the home trash (the Trash view).
#[derive(Clone, Debug)]
pub struct Entry {
    pub trashed: Trashed,
    pub deleted: String,
}

/// Read `info/<name>.trashinfo` for an item in `files/`.
pub fn entry_for(file: &Path) -> Option<Entry> {
    let files = file.parent()?;
    let dir = files.parent()?;
    let mut info_name = file.file_name()?.to_os_string();
    info_name.push(".trashinfo");
    let info = dir.join("info").join(info_name);
    let text = fs::read_to_string(&info).ok()?;
    let mut original = None;
    let mut deleted = String::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("Path=") {
            let p = decode(v);
            original = Some(if p.is_absolute() { p } else { topdir(dir).join(p) });
        } else if let Some(v) = line.strip_prefix("DeletionDate=") {
            deleted = v.replace('T', " ");
        }
    }
    Some(Entry { trashed: Trashed { original: original?, file: file.to_path_buf(), info }, deleted })
}

/// Is `dir` the `files` folder of a trash?
pub fn is_trash_files(dir: &Path) -> bool {
    if dir.file_name().is_none_or(|n| n != "files") { return false; }
    let Some(root) = dir.parent() else { return false };
    let top = topdir(root);
    let correct = root == home_trash() || root == top.join(format!(".Trash-{}", uid())) || root == top.join(".Trash").join(uid().to_string());
    correct && valid_dir(root) && valid_dir(dir) && valid_dir(&root.join("info"))
}

fn valid_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && m.uid() == uid() && m.mode() & 0o077 == 0)
}

fn secure_dir(path: &Path) -> io::Result<File> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}, Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}, Err(e) => return Err(e),
    }
    let file = OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW).open(path)?;
    let m = file.metadata()?;
    if !m.is_dir() || m.uid() != uid() || m.mode() & 0o077 != 0 { return Err(io::Error::new(io::ErrorKind::PermissionDenied, "unsafe Trash directory ownership or permissions")); }
    Ok(file)
}

/// Mount types that never hold a trash, or where looking could hang (network, FUSE).
const NO_TRASH_FS: &[&str] = &[
    "proc", "sysfs", "devtmpfs", "devpts", "cgroup", "cgroup2", "mqueue", "debugfs", "tracefs",
    "securityfs", "pstore", "bpf", "configfs", "fusectl", "hugetlbfs", "autofs", "binfmt_misc", "efivarfs",
    "rpc_pipefs", "nsfs", "overlay", "squashfs", "nfs", "nfs4",
];

/// Mount points of local drives, from `/proc/self/mountinfo`.
fn local_mounts() -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string("/proc/self/mountinfo") else { return Vec::new() };
    let mut out: Vec<PathBuf> = Vec::new();
    for line in text.lines() {
        // id parent major:minor root MOUNT options… - FSTYPE source super-options
        let mut halves = line.splitn(2, " - ");
        let (Some(left), Some(right)) = (halves.next(), halves.next()) else { continue };
        let Some(mount) = left.split(' ').nth(4) else { continue };
        let Some(fstype) = right.split(' ').next() else { continue };
        if NO_TRASH_FS.contains(&fstype) || fstype.starts_with("fuse") || matches!(fstype, "cifs" | "smb3" | "9p" | "sshfs") {
            continue;
        }
        // mountinfo escapes space, tab, newline and backslash as \ooo.
        let p = PathBuf::from(OsString::from_vec(unescape_octal(mount.as_bytes())));
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

fn unescape_octal(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 3 < b.len() && b[i + 1..i + 4].iter().all(|c| (b'0'..=b'7').contains(c)) {
            out.push((b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0'));
            i += 4;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// The trashes on other drives (`$topdir/.Trash/$uid`, `$topdir/.Trash-$uid`) that exist —
/// where files trashed from those drives go, since a trash never crosses a drive.
pub fn drive_trashes() -> Vec<PathBuf> {
    let home = home_trash();
    let mut out = Vec::new();
    for mount in local_mounts() {
        for dir in [mount.join(".Trash").join(uid().to_string()), mount.join(format!(".Trash-{}", uid()))] {
            if dir != home && !out.contains(&dir) && is_trash_files(&dir.join("files")) {
                out.push(dir);
            }
        }
    }
    out
}

/// "Trash" or "Trash on <drive>" when `files` is a trash's `files/` folder, from the path
/// alone (no disk access, so views can call it).
pub fn label(files: &Path) -> Option<String> {
    if files == home_trash().join("files") {
        return Some("Trash".into());
    }
    if files.file_name()? != "files" {
        return None;
    }
    let root = files.parent()?;
    let name = root.file_name()?.to_string_lossy();
    let mount = if name == format!(".Trash-{}", uid()) {
        root.parent()?
    } else if name == uid().to_string() && root.parent()?.file_name()? == ".Trash" {
        root.parent()?.parent()?
    } else {
        return None;
    };
    let drive = mount.file_name().map_or_else(|| "Computer".into(), |n| n.to_string_lossy().into_owned());
    Some(format!("Trash on {drive}"))
}

/// Every trash on this computer: the home trash first, then each drive's.
pub fn all_trashes() -> Vec<PathBuf> {
    let mut out = vec![home_trash()];
    out.extend(drive_trashes());
    out
}

/// Items waiting in every trash.
pub fn count_all() -> usize {
    all_trashes().iter().map(|t| fs::read_dir(t.join("files")).map(|r| r.count()).unwrap_or(0)).sum()
}

/// After `file` (an item in a trash's `files/`) was deleted for good, drop its
/// `.trashinfo` too, so no other app lists it or offers to restore it.
pub fn forget(file: &Path) {
    let (Some(files), Some(name)) = (file.parent(), file.file_name()) else { return };
    let Some(root) = files.parent() else { return };
    let mut info = name.to_os_string();
    info.push(".trashinfo");
    let _ = fs::remove_file(root.join("info").join(info));
}

/// Permanently remove everything in every trash on this computer. Keeps going past a
/// drive that fails and returns the first error.
pub fn empty_all() -> io::Result<()> {
    let mut first = None;
    for t in all_trashes() {
        if let Err(e) = empty(&t) {
            first.get_or_insert(e);
        }
    }
    first.map_or(Ok(()), Err)
}

/// Permanently remove everything in the home trash.
pub fn empty_home() -> io::Result<()> {
    empty(&home_trash())
}

fn empty(t: &Path) -> io::Result<()> {
    for sub in ["files", "info"] {
        let d = t.join(sub);
        if let Ok(rd) = fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                let r = if e.file_type().is_ok_and(|t| t.is_dir()) { fs::remove_dir_all(&p) } else { fs::remove_file(&p) };
                r?;
            }
        }
    }
    let _ = fs::remove_file(t.join("directorysizes"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_paths() {
        let p = Path::new("/home/a/My file#1 é.txt");
        assert_eq!(decode(&encode(p)), p);
        assert_eq!(numbered(b"report.pdf", 2), b"report.2.pdf");
        assert_eq!(numbered(b".bashrc", 3), b".bashrc.3");
    }

    #[test]
    fn trashes_and_restores() {
        let base = std::env::temp_dir().join(format!("ef-trash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        // Use a private XDG_DATA_HOME so the test never touches the real trash.
        // SAFETY: tests in this module run on one thread per test; nothing else reads it.
        unsafe { std::env::set_var("XDG_DATA_HOME", base.join("data")) };
        let f = base.join("a.txt");
        fs::write(&f, b"x").unwrap();
        let t = trash(&f).unwrap();
        assert!(!f.exists() && t.file.exists() && t.info.exists());
        let e = entry_for(&t.file).unwrap();
        assert_eq!(e.trashed.original, f);
        assert_eq!(restore(&t).unwrap(), f);
        assert!(f.exists() && !t.info.exists());
        // Deleting an item from the trash removes its record too.
        let t = trash(&f).unwrap();
        assert!(crate::ops::delete(std::slice::from_ref(&t.file), &crate::ops::Progress::default()).is_empty());
        assert!(!t.file.exists() && !t.info.exists());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn names_drive_trashes_from_the_path() {
        let u = uid();
        assert_eq!(label(&PathBuf::from(format!("/media/data106/.Trash-{u}/files"))).as_deref(), Some("Trash on data106"));
        assert_eq!(label(&PathBuf::from(format!("/run/media/a/USB/.Trash/{u}/files"))).as_deref(), Some("Trash on USB"));
        assert_eq!(label(&PathBuf::from(format!("/.Trash-{u}/files"))).as_deref(), Some("Trash on Computer"));
        assert_eq!(label(Path::new("/home/a/files")), None);
        assert_eq!(label(&PathBuf::from(format!("/media/x/.Trash-{}/files", u + 1))), None);
    }

    #[test]
    fn reads_escaped_mount_points() {
        assert_eq!(unescape_octal(br"/media/My\040Drive"), b"/media/My Drive");
        assert_eq!(unescape_octal(br"/a\134b"), b"/a\\b");
        assert_eq!(unescape_octal(br"/end\04"), br"/end\04");
        assert!(local_mounts().contains(&PathBuf::from("/")));
    }
}

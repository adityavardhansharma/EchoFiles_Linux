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
    let path = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    let (dir, recorded, root_fd) = trash_for(&path)?;
    let (files, info) = (dir.join("files"), dir.join("info"));
    let pinned = PathBuf::from(format!("/proc/self/fd/{}", root_fd.as_raw_fd()));
    let files_fd = secure_dir(&pinned.join("files"))?;
    let info_fd = secure_dir(&pinned.join("info"))?;
    let pinned_files = PathBuf::from(format!("/proc/self/fd/{}", files_fd.as_raw_fd()));
    let pinned_info = PathBuf::from(format!("/proc/self/fd/{}", info_fd.as_raw_fd()));
    let base = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "can't trash a filesystem root"))?.as_bytes().to_vec();
    let body = format!("[Trash Info]\nPath={}\nDeletionDate={}\n", encode(&recorded), now_local());
    for n in 1..10_000 {
        let name = OsString::from_vec(numbered(&base, n));
        let mut info_name = name.clone(); info_name.push(".trashinfo");
        let info_path = pinned_info.join(&info_name);
        let target = pinned_files.join(&name);
        if fs::symlink_metadata(&target).is_ok() {
            continue;
        }
        // The info file is created exclusively first: it reserves the name (spec).
        match OpenOptions::new().write(true).create_new(true).mode(0o600).open(&info_path) {
            Ok(mut f) => {
                f.write_all(body.as_bytes())?;
                f.sync_all()?;
                if let Err(e) = crate::ops::rename_noreplace(&path, &target) {
                    let _ = fs::remove_file(&info_path);
                    return Err(e);
                }
                return Ok(Trashed { original: path, file: files.join(name), info: info.join(info_name) });
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("too many items with this name in the trash"))
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

/// Permanently remove everything in the home trash.
pub fn empty_home() -> io::Result<()> {
    let t = home_trash();
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
        fs::remove_dir_all(&base).unwrap();
    }
}

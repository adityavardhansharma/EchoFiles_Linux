//! Two-phase directory listing (build plan §2.2).
//!
//! Phase A reads names and `d_type` with raw `getdents64` into a struct-of-arrays
//! [`Listing`]; phase B fills size/mtime/mode with dirfd-relative `statx` calls spread
//! across the rayon pool.

use std::ffi::{CStr, OsStr};
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use rustix::fs::{AtFlags, Mode, OFlags, RawDir, StatxFlags};

/// Buffer size for `getdents64`; 1 MB measured 30% faster than 32 KB on 100k entries.
const DENTS_BUF: usize = 1 << 20;
/// Names per `statx` batch handed to one rayon task.
const STAT_CHUNK: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Kind {
    Dir = 0,
    File = 1,
    Symlink = 2,
    Other = 3,
    Unknown = 4,
}

pub mod flags {
    /// Dotfile (Windows Hidden/System attributes are added later for NTFS).
    pub const HIDDEN: u16 = 1 << 0;
    /// Metadata has been filled by phase B.
    pub const STATED: u16 = 1 << 1;
    /// `statx` failed (entry vanished, permission denied…).
    pub const STAT_FAILED: u16 = 1 << 2;
    /// A symlink whose target is a folder: opens like one.
    pub const LINK_DIR: u16 = 1 << 3;
    /// A symlink whose target is gone.
    pub const BROKEN: u16 = 1 << 4;
    /// NTFS: a cloud placeholder (OneDrive "online-only").
    pub const CLOUD: u16 = 1 << 5;
    /// NTFS: EFS-encrypted; unreadable without the Windows certificate.
    pub const LOCKED: u16 = 1 << 6;
}

/// NTFS attribute bits (`FILE_ATTRIBUTE_*`), read through ntfs3's `system.ntfs_attrib`.
pub mod win {
    pub const READONLY: u32 = 0x1;
    pub const HIDDEN: u32 = 0x2;
    pub const SYSTEM: u32 = 0x4;
    pub const ARCHIVE: u32 = 0x20;
    pub const SPARSE: u32 = 0x200;
    pub const REPARSE: u32 = 0x400;
    pub const COMPRESSED: u32 = 0x800;
    pub const OFFLINE: u32 = 0x1000;
    pub const ENCRYPTED: u32 = 0x4000;
    pub const RECALL_ON_OPEN: u32 = 0x40000;
    pub const RECALL_ON_DATA_ACCESS: u32 = 0x400000;
}

/// Files Windows keeps at a volume's top level that Explorer never shows.
const WINDOWS_JUNK: [&str; 13] = [
    "$recycle.bin", "system volume information", "pagefile.sys", "hiberfil.sys", "swapfile.sys", "dumpstack.log.tmp", "dumpstack.log",
    "thumbs.db", "desktop.ini", "$winreagent", "config.msi", "$sysreset", "recovery",
];

/// A directory's entries as parallel columns; ~40 bytes per entry, no per-entry heap allocation.
#[derive(Clone, Debug, Default)]
pub struct Listing {
    pub dir: PathBuf,
    names: Vec<u8>,
    name_off: Vec<u32>,
    pub kind: Vec<Kind>,
    pub size: Vec<u64>,
    /// Modification time, seconds since the epoch.
    pub mtime: Vec<i64>,
    pub mode: Vec<u32>,
    pub flags: Vec<u16>,
    /// NTFS attributes; empty on other filesystems.
    pub attrs: Vec<u32>,
}

impl Listing {
    pub fn len(&self) -> usize {
        self.kind.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kind.is_empty()
    }

    /// Raw name bytes (without the NUL terminator kept for `statx`).
    pub fn name_bytes(&self, i: usize) -> &[u8] {
        let (a, b) = (self.name_off[i] as usize, self.name_off[i + 1] as usize);
        &self.names[a..b - 1]
    }

    pub fn name(&self, i: usize) -> &OsStr {
        OsStr::from_bytes(self.name_bytes(i))
    }

    fn name_cstr(&self, i: usize) -> &CStr {
        let (a, b) = (self.name_off[i] as usize, self.name_off[i + 1] as usize);
        // Safety net: every stored name ends with exactly one NUL and holds no interior NUL.
        CStr::from_bytes_with_nul(&self.names[a..b]).expect("name arena is NUL-terminated")
    }

    pub fn path(&self, i: usize) -> PathBuf {
        self.dir.join(self.name(i))
    }

    pub fn is_dir(&self, i: usize) -> bool {
        self.kind[i] == Kind::Dir
    }

    pub fn is_hidden(&self, i: usize) -> bool {
        self.flags[i] & flags::HIDDEN != 0
    }

    /// Bytes held by this listing's buffers.
    pub fn heap_bytes(&self) -> usize {
        self.names.capacity()
            + self.name_off.capacity() * 4
            + self.kind.capacity()
            + self.size.capacity() * 8
            + self.mtime.capacity() * 8
            + self.mode.capacity() * 4
            + self.flags.capacity() * 2
            + self.attrs.capacity() * 4
    }

    /// The Windows attribute bits of entry `i`, if this is an NTFS listing.
    pub fn win_attrs(&self, i: usize) -> Option<u32> {
        self.attrs.get(i).copied()
    }

    fn push(&mut self, name: &[u8], kind: Kind) {
        if self.name_off.is_empty() {
            self.name_off.push(0);
        }
        self.names.extend_from_slice(name);
        self.names.push(0);
        self.name_off.push(self.names.len() as u32);
        self.kind.push(kind);
        self.size.push(0);
        self.mtime.push(0);
        self.mode.push(0);
        self.flags.push(if name.first() == Some(&b'.') { flags::HIDDEN } else { 0 });
    }
}

/// Open a directory for listing; the fd is reused for every `statx` in phase B.
pub fn open_dir(dir: &Path) -> io::Result<OwnedFd> {
    Ok(rustix::fs::open(dir, OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC, Mode::empty())?)
}

/// Phase A: names and types only. Enough to paint the first frame.
pub fn read_names(dir: &Path, fd: &OwnedFd) -> io::Result<Listing> {
    let mut listing = Listing { dir: dir.to_path_buf(), ..Default::default() };
    let mut buf: Vec<MaybeUninit<u8>> = Vec::with_capacity(DENTS_BUF);
    buf.resize(DENTS_BUF, MaybeUninit::uninit());
    let mut raw = RawDir::new(fd.as_fd(), &mut buf);
    while let Some(entry) = raw.next() {
        let entry = entry?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        let kind = match entry.file_type() {
            rustix::fs::FileType::Directory => Kind::Dir,
            rustix::fs::FileType::RegularFile => Kind::File,
            rustix::fs::FileType::Symlink => Kind::Symlink,
            rustix::fs::FileType::Unknown => Kind::Unknown,
            _ => Kind::Other,
        };
        listing.push(name, kind);
    }
    if listing.name_off.is_empty() {
        listing.name_off.push(0);
    }
    Ok(listing)
}

/// Phase B: size, mtime and mode for every entry, in parallel, relative to the dirfd.
pub fn fill_metadata(listing: &mut Listing, fd: &OwnedFd) {
    let n = listing.len();
    if n == 0 {
        return;
    }
    let mask = StatxFlags::SIZE | StatxFlags::MTIME | StatxFlags::TYPE | StatxFlags::MODE;
    let at = AtFlags::SYMLINK_NOFOLLOW | AtFlags::STATX_DONT_SYNC;
    let results: Vec<Option<(u64, i64, u32)>> = (0..n)
        .into_par_iter()
        .with_min_len(STAT_CHUNK)
        .map(|i| {
            rustix::fs::statx(fd.as_fd(), listing.name_cstr(i), at, mask)
                .ok()
                .map(|st| (st.stx_size, st.stx_mtime.tv_sec, st.stx_mode as u32))
        })
        .collect();
    for (i, r) in results.into_iter().enumerate() {
        match r {
            Some((size, mtime, mode)) => {
                listing.size[i] = size;
                listing.mtime[i] = mtime;
                listing.mode[i] = mode;
                listing.flags[i] |= flags::STATED;
                if listing.kind[i] == Kind::Unknown {
                    listing.kind[i] = kind_from_mode(mode);
                }
            }
            None => listing.flags[i] |= flags::STAT_FAILED,
        }
    }
    // Symlinks: where do they point? One more statx, following the link, for links only.
    let links: Vec<usize> = (0..n).filter(|&i| listing.kind[i] == Kind::Symlink).collect();
    let targets: Vec<(usize, Option<u32>)> = links
        .par_iter()
        .map(|&i| (i, rustix::fs::statx(fd.as_fd(), listing.name_cstr(i), AtFlags::STATX_DONT_SYNC, StatxFlags::TYPE).ok().map(|st| st.stx_mode as u32)))
        .collect();
    for (i, mode) in targets {
        match mode {
            Some(m) if kind_from_mode(m) == Kind::Dir => listing.flags[i] |= flags::LINK_DIR,
            Some(_) => {}
            None => listing.flags[i] |= flags::BROKEN,
        }
    }
}

/// Hide Windows' own bookkeeping files the way Explorer does (names phase, no I/O).
pub fn mark_windows_junk(listing: &mut Listing) {
    for i in 0..listing.len() {
        let n = listing.name_bytes(i);
        if n.len() <= 26 && WINDOWS_JUNK.iter().any(|j| j.as_bytes().eq_ignore_ascii_case(n)) || n.starts_with(b"found.") {
            listing.flags[i] |= flags::HIDDEN;
        }
    }
}

/// NTFS: read each entry's attributes; Hidden/System entries become hidden like dotfiles,
/// cloud placeholders and EFS files get their badges.
pub fn fill_windows_attributes(listing: &mut Listing, _fd: &OwnedFd) {
    let n = listing.len();
    let dir = listing.dir.clone();
    let attrs: Vec<u32> = (0..n)
        .into_par_iter()
        .with_min_len(STAT_CHUNK)
        .map(|i| {
            let mut buf = [0u8; 4];
            match rustix::fs::lgetxattr(dir.join(listing.name(i)), "system.ntfs_attrib", &mut buf) {
                Ok(4) => u32::from_le_bytes(buf),
                _ => 0,
            }
        })
        .collect();
    for (i, a) in attrs.iter().enumerate() {
        if a & (win::HIDDEN | win::SYSTEM) != 0 {
            listing.flags[i] |= flags::HIDDEN;
        }
        if a & (win::OFFLINE | win::RECALL_ON_OPEN | win::RECALL_ON_DATA_ACCESS) != 0 {
            listing.flags[i] |= flags::CLOUD;
        }
        if a & win::ENCRYPTED != 0 {
            listing.flags[i] |= flags::LOCKED;
        }
    }
    listing.attrs = attrs;
}

fn kind_from_mode(mode: u32) -> Kind {
    match mode & 0o170000 {
        0o040000 => Kind::Dir,
        0o100000 => Kind::File,
        0o120000 => Kind::Symlink,
        _ => Kind::Other,
    }
}

/// Both phases, back to back. Callers that paint between phases call them separately.
pub fn list(dir: &Path) -> io::Result<Listing> {
    let fd = open_dir(dir)?;
    let mut listing = read_names(dir, &fd)?;
    fill_metadata(&mut listing, &fd);
    Ok(listing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn lists_names_kinds_and_sizes() {
        let dir = std::env::temp_dir().join(format!("ef-core-list-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), b"hello").unwrap();
        fs::write(dir.join(".hidden"), b"").unwrap();
        std::os::unix::fs::symlink("a.txt", dir.join("link")).unwrap();

        let l = list(&dir).unwrap();
        assert_eq!(l.len(), 4);
        let find = |n: &str| (0..l.len()).find(|&i| l.name(i) == n).unwrap();
        assert_eq!(l.kind[find("sub")], Kind::Dir);
        assert_eq!(l.kind[find("link")], Kind::Symlink);
        assert_eq!(l.size[find("a.txt")], 5);
        assert!(l.is_hidden(find(".hidden")));
        assert!((0..l.len()).all(|i| l.flags[i] & flags::STATED != 0));
        fs::remove_dir_all(&dir).unwrap();
    }
}

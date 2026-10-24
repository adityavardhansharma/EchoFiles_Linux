//! What makes an entry a result: name patterns, extension, type, size, time, owner and
//! depth. Shared by the indexed and the live search so both answer alike.
//!
//! Checks run cheapest first — name, extension and the type the directory listing already
//! gave — and the entry is `stat`ed only when a filter needs it, once.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::time::SystemTime;

use rustix::fd::BorrowedFd;
use rustix::fs::{AtFlags, FileType};

use crate::pattern::Pattern;
use crate::units::{OwnerFilter, SizeFilter};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FType {
    File,
    Dir,
    Symlink,
    Socket,
    Fifo,
    Block,
    Char,
    Unknown,
}

impl FType {
    pub fn from_file_type(t: FileType) -> FType {
        match t {
            FileType::RegularFile => FType::File,
            FileType::Directory => FType::Dir,
            FileType::Symlink => FType::Symlink,
            FileType::Socket => FType::Socket,
            FileType::Fifo => FType::Fifo,
            FileType::BlockDevice => FType::Block,
            FileType::CharacterDevice => FType::Char,
            _ => FType::Unknown,
        }
    }

    pub fn from_mode(mode: u32) -> FType {
        FType::from_file_type(FileType::from_raw_mode(mode))
    }

    pub fn from_kind(k: crate::Kind) -> FType {
        match k {
            crate::Kind::File => FType::File,
            crate::Kind::Dir => FType::Dir,
            crate::Kind::Symlink => FType::Symlink,
            crate::Kind::Other => FType::Unknown,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            FType::File => "file",
            FType::Dir => "dir",
            FType::Symlink => "symlink",
            FType::Socket => "socket",
            FType::Fifo => "pipe",
            FType::Block => "block-device",
            FType::Char => "char-device",
            FType::Unknown => "other",
        }
    }
}

/// The `stat` fields results and filters use.
#[derive(Clone, Copy, Debug)]
pub struct Meta {
    pub ftype: FType,
    pub size: u64,
    pub mtime: SystemTime,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub nlink: u64,
    pub dev: u64,
    pub ino: u64,
}

impl Meta {
    pub fn from_stat(st: &rustix::fs::Stat) -> Meta {
        let secs = st.st_mtime;
        let nanos = st.st_mtime_nsec as u32;
        let mtime = crate::units::from_unix(secs) + std::time::Duration::from_nanos(nanos as u64);
        Meta {
            ftype: FType::from_mode(st.st_mode),
            size: st.st_size as u64,
            mtime,
            mode: st.st_mode,
            uid: st.st_uid,
            gid: st.st_gid,
            nlink: st.st_nlink,
            dev: st.st_dev,
            ino: st.st_ino,
        }
    }

    pub fn executable(&self) -> bool {
        self.mode & 0o111 != 0
    }
}

/// `stat` one entry, at most once each way: relative to its open folder when the walk has
/// one, by absolute path otherwise.
pub struct Probe<'a> {
    dir: Option<BorrowedFd<'a>>,
    name: &'a [u8],
    abs: &'a [u8],
    lstat: Option<Option<Meta>>,
    stat: Option<Option<Meta>>,
}

impl<'a> Probe<'a> {
    pub fn new(dir: Option<BorrowedFd<'a>>, name: &'a [u8], abs: &'a [u8]) -> Probe<'a> {
        Probe { dir, name, abs, lstat: None, stat: None }
    }

    fn run(&self, follow: bool) -> Option<Meta> {
        let flags = if follow { AtFlags::empty() } else { AtFlags::SYMLINK_NOFOLLOW };
        let st = match self.dir {
            Some(d) => rustix::fs::statat(d, OsStr::from_bytes(self.name), flags),
            None => rustix::fs::statat(rustix::fs::CWD, OsStr::from_bytes(self.abs), flags),
        };
        st.ok().map(|s| Meta::from_stat(&s))
    }

    pub fn lstat(&mut self) -> Option<Meta> {
        if self.lstat.is_none() {
            self.lstat = Some(self.run(false));
        }
        self.lstat.unwrap()
    }

    /// Follows symlinks; a broken link falls back to the link itself.
    pub fn stat(&mut self) -> Option<Meta> {
        if self.stat.is_none() {
            let s = self.run(true);
            self.stat = Some(s.or_else(|| self.lstat()));
        }
        self.stat.unwrap()
    }

    /// Whether a folder has no entries (read live).
    pub fn empty_dir(&self) -> bool {
        std::fs::read_dir(OsStr::from_bytes(self.abs)).is_ok_and(|mut d| d.next().is_none())
    }
}

/// Which entry types are wanted (`-t`). An empty set means "any".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Types {
    pub file: bool,
    pub dir: bool,
    pub symlink: bool,
    pub socket: bool,
    pub fifo: bool,
    pub block: bool,
    pub char: bool,
    /// `-t x`: only executables.
    pub executable: bool,
    /// `-t e`: only empty files and folders.
    pub empty: bool,
    /// `ef --files`: anything but folders.
    pub not_dir: bool,
}

impl Types {
    pub fn any(&self) -> bool {
        *self == Types::default()
    }

    /// Add one `-t` value: `f`, `d`, `l`, `s`, `p`, `b`, `c`, `x`, `e` or the long names.
    pub fn add(&mut self, v: &str) -> Result<(), String> {
        match v {
            "f" | "file" => self.file = true,
            "d" | "dir" | "directory" => self.dir = true,
            "l" | "symlink" => self.symlink = true,
            "s" | "socket" => self.socket = true,
            "p" | "pipe" => self.fifo = true,
            "b" | "block-device" => self.block = true,
            "c" | "char-device" => self.char = true,
            "x" | "executable" => {
                self.executable = true;
                self.file = true;
            }
            "e" | "empty" => self.empty = true,
            _ => return Err(format!("unknown type '{v}' (use f, d, l, s, p, b, c, x or e)")),
        }
        Ok(())
    }

    /// fd: `-t e` without `-t f` or `-t d` means empty files and folders.
    pub fn finish(&mut self) {
        if self.empty && !(self.file || self.dir) {
            self.file = true;
            self.dir = true;
        }
    }

    fn kind_ok(&self, t: FType) -> bool {
        if self.not_dir {
            return t != FType::Dir;
        }
        match t {
            FType::File => self.file,
            FType::Dir => self.dir,
            FType::Symlink => self.symlink,
            FType::Socket => self.socket,
            FType::Fifo => self.fifo,
            FType::Block => self.block,
            FType::Char => self.char,
            FType::Unknown => false,
        }
    }
}

/// Everything a result must satisfy, apart from where the walk goes (hidden, ignore files,
/// depth limits — the walkers handle those).
#[derive(Default)]
pub struct Filters {
    /// All must match (fd's `--and`).
    pub patterns: Vec<Pattern>,
    /// Any of these extensions, lowercase, without the dot.
    pub exts: Vec<Vec<u8>>,
    /// fd's rule: `-e rs` matches a file named just `.rs`. EchoFiles' rule: it doesn't.
    pub ext_matches_bare_dotfile: bool,
    pub types: Types,
    pub sizes: Vec<SizeFilter>,
    /// `--changed-within`: modified at or after this.
    pub newer: Option<SystemTime>,
    /// `--changed-before`: modified before this.
    pub older: Option<SystemTime>,
    pub owner: Option<OwnerFilter>,
    pub min_depth: u32,
    pub max_depth: Option<u32>,
    pub hidden: bool,
    pub follow: bool,
    pub prune: bool,
    pub one_file_system: bool,
    /// Skip folders holding an entry with one of these names.
    pub ignore_contain: Vec<Vec<u8>>,
    /// Keep metadata on every hit (for `--json`, `-l`, sorting by size or time).
    pub want_meta: bool,
}

/// One result.
#[derive(Clone, Debug)]
pub struct Hit {
    /// Absolute path.
    pub path: Vec<u8>,
    /// Which search root it came from (index into the roots given to the search).
    pub root: u16,
    pub depth: u32,
    pub ftype: FType,
    pub meta: Option<Box<Meta>>,
    pub rank: u64,
}

impl Hit {
    pub fn name(&self) -> &[u8] {
        let p = &self.path;
        match memchr::memrchr(b'/', p) {
            Some(i) if i + 1 < p.len() => &p[i + 1..],
            _ => p,
        }
    }
}

impl Filters {
    /// Whether any filter needs `stat`.
    pub fn needs_stat(&self) -> bool {
        self.want_meta
            || !self.sizes.is_empty()
            || self.newer.is_some()
            || self.older.is_some()
            || self.owner.is_some()
            || self.types.executable
            || self.types.empty
            || self.follow
    }

    pub fn needs_path(&self) -> bool {
        self.patterns.iter().any(|p| p.full_path)
    }

    /// Whether name checks or ranking read the folded name (EchoFiles word patterns).
    pub fn needs_fold(&self, rank: bool) -> bool {
        self.patterns.iter().any(|p| p.syntax == crate::pattern::Syntax::Words && !p.full_path)
            || (rank && self.patterns.first().is_some_and(|p| p.syntax == crate::pattern::Syntax::Words))
    }

    /// Name-only checks: patterns and extension. `folded` is the folded name if at hand.
    pub fn name_ok(&self, name: &[u8], folded: Option<&[u8]>, abs: &[u8]) -> bool {
        if !self.exts.is_empty() {
            let ok = self.exts.iter().any(|e| {
                let n = e.len() + 1;
                name.len() >= n
                    && (self.ext_matches_bare_dotfile || name.len() > n)
                    && name[name.len() - n] == b'.'
                    && name[name.len() - e.len()..].eq_ignore_ascii_case(e)
            });
            if !ok {
                return false;
            }
        }
        self.patterns.iter().all(|p| if p.full_path { p.is_match(abs, None) } else { p.is_match(name, folded) })
    }

    /// The full check for an entry whose name passed [`name_ok`](Filters::name_ok).
    /// `listed` is the type from the directory listing (or the index).
    pub fn rest_ok(&self, listed: FType, probe: &mut Probe) -> Option<(FType, Option<Meta>)> {
        let mut t = listed;
        if t == FType::Unknown {
            t = probe.lstat().map_or(FType::Unknown, |m| m.ftype);
        }
        let meta_now = |probe: &mut Probe| if self.follow { probe.stat() } else { probe.lstat() };
        if self.follow && t == FType::Symlink {
            // With -L a link counts as what it points to; a broken link stays a link.
            t = probe.stat().map_or(FType::Symlink, |m| m.ftype);
        }
        if !self.types.any() {
            if !self.types.kind_ok(t) {
                return None;
            }
            if self.types.executable && !meta_now(probe).is_some_and(|m| m.executable()) {
                return None;
            }
            if self.types.empty {
                let empty = match t {
                    FType::File => meta_now(probe).is_some_and(|m| m.size == 0),
                    FType::Dir => probe.empty_dir(),
                    _ => false,
                };
                if !empty {
                    return None;
                }
            }
        }
        if let Some(o) = &self.owner {
            let m = meta_now(probe)?;
            if !o.accepts(m.uid, m.gid) {
                return None;
            }
        }
        if !self.sizes.is_empty() {
            // fd: size filters only pass regular files (following links to see that).
            if probe.stat().map(|m| m.ftype) != Some(FType::File) {
                return None;
            }
            let m = meta_now(probe)?;
            if !self.sizes.iter().all(|s| s.accepts(m.size)) {
                return None;
            }
        }
        if self.newer.is_some() || self.older.is_some() {
            let m = meta_now(probe)?;
            if self.newer.is_some_and(|t| m.mtime < t) || self.older.is_some_and(|t| m.mtime >= t) {
                return None;
            }
        }
        let meta = if self.want_meta { meta_now(probe) } else { None };
        Some((t, meta))
    }

    pub fn rank(&self, name: &[u8], folded: Option<&[u8]>) -> u64 {
        match self.patterns.first() {
            Some(p) => p.rank(name, folded),
            // Everything matches: no "best", keep path order.
            None => 0,
        }
    }
}

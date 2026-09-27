//! Filesystem facts for the status bar and drive list: type, free and total space.

use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsInfo {
    /// `btrfs`, `ntfs3`, `ext4`, …
    pub fs_type: &'static str,
    pub free: u64,
    pub total: u64,
    /// Mounted read-only.
    pub read_only: bool,
}

impl FsInfo {
    pub fn used_fraction(&self) -> f32 {
        if self.total == 0 { 0.0 } else { 1.0 - self.free as f32 / self.total as f32 }
    }
}

/// One `statfs` call; cheap enough for every navigation.
pub fn fs_info(path: &Path) -> Option<FsInfo> {
    let st = rustix::fs::statfs(path).ok()?;
    let block = st.f_bsize as u64;
    Some(FsInfo {
        fs_type: fs_name(st.f_type as i64),
        free: st.f_bavail as u64 * block,
        total: st.f_blocks as u64 * block,
        read_only: (st.f_flags as u64) & 1 != 0, // ST_RDONLY
    })
}

fn fs_name(magic: i64) -> &'static str {
    match magic as u32 {
        0x9123_683E => "btrfs",
        0xEF53 => "ext4",
        0x5846_5342 => "xfs",
        0x7366_746E => "ntfs3",
        0x5346_544E => "ntfs",
        0x6573_5546 => "fuse",
        0x4d44 => "vfat",
        0x2011_BAB0 => "exfat",
        0x0102_1994 => "tmpfs",
        0xF15F => "ecryptfs",
        0x6969 => "nfs",
        0xFF53_4D42 | 0xFE53_4D42 => "cifs",
        0x794C_7630 => "overlay",
        0x9FA0 => "proc",
        _ => "unknown",
    }
}

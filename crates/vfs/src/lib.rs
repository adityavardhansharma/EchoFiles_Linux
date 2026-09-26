//! Locations independent of where things are mounted (build plan §1). v0.1 only has local
//! paths; the phone backend (v0.3+) adds a remote variant without touching callers.

use std::path::PathBuf;

/// Stable identity of a volume. Mount paths move (duplicate labels mount as `AVS` or `AVS1`
/// depending on order), so caches and bookmarks key on this instead.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VolumeId {
    /// The Linux root filesystem and everything that isn't a known volume.
    LinuxRoot,
    /// A partition, by filesystem UUID.
    Uuid(String),
}

/// A place the file manager can show.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Location {
    Local(PathBuf),
}

impl Location {
    pub fn local_path(&self) -> Option<&std::path::Path> {
        match self {
            Location::Local(p) => Some(p),
        }
    }
}

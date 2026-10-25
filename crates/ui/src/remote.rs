//! Places that live on another computer: GVfs network places and mounted Google Drives.
//! Each caller decides what "remote" means for it (no thumbnails, no Trash, …).

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remote {
    /// SMB, SFTP, FTP through GVfs' FUSE bridge.
    Network,
    /// Google Drive through rclone's mount.
    Cloud,
}

pub fn place(p: &Path) -> Option<Remote> {
    if ef_net::gvfs::is_network_path(p) {
        Some(Remote::Network)
    } else if ef_config::is_cloud_path(p) {
        Some(Remote::Cloud)
    } else {
        None
    }
}

/// Slow to read: every byte crosses the network.
pub fn is_remote(p: &Path) -> bool {
    place(p).is_some()
}

//! Where Drive things live. Our own rclone config, never the user's `~/.config/rclone`.

use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

use ef_config as config;

/// `~/.config/echofiles/rclone/rclone.conf` — holds refresh tokens, so 0600 in a 0700 folder.
pub fn config_file() -> PathBuf {
    config::cloud_rclone_dir().join("rclone.conf")
}

/// `$XDG_RUNTIME_DIR/echofiles/rclone.sock` — rc, reachable only by this user.
pub fn socket() -> PathBuf {
    config::runtime_dir().join("rclone.sock")
}

/// `~/.local/state/echofiles/rclone.log`
pub fn log_file() -> PathBuf {
    config::state_dir().join("rclone.log")
}

/// Where an account's Drive appears as a folder.
pub fn mount_point(account: &str) -> PathBuf {
    config::cloud_mount_root().join(account)
}

/// The rclone remote for an account (`ef-k3f9q2`). Prefixed so a hand-edited config can
/// never collide with the user's own remote names.
pub fn remote(account: &str) -> String {
    format!("ef-{account}")
}

/// `ef-k3f9q2:` — the rclone "fs" string for an account's Drive root.
pub fn fs(account: &str) -> String {
    format!("{}:", remote(account))
}

/// Catalog and delta files for an account (only with `[cloud] index`).
pub fn catalog(account: &str) -> PathBuf {
    config::cloud_index_dir().join(format!("{account}.jsonl"))
}

pub fn delta(account: &str) -> PathBuf {
    config::cloud_index_dir().join(format!("{account}.delta.jsonl"))
}

/// Make `dir` (and parents) and set it to 0700 — for the socket, cache, config and index.
pub fn private_dir(dir: &Path) -> io::Result<()> {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

/// Make sure our rclone config exists with 0600, so rclone never creates it world-readable.
pub fn ensure_config() -> io::Result<PathBuf> {
    let file = config_file();
    if let Some(dir) = file.parent() {
        private_dir(dir)?;
    }
    if !file.exists() {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new().write(true).create(true).truncate(false).mode(0o600).open(&file)?;
    }
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600))?;
    Ok(file)
}

/// A new account ID: 6 characters of `[a-z0-9]` from the kernel's random source.
pub fn new_account_id() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut buf = [0u8; 6];
    // SAFETY: getrandom writes at most buf.len() bytes into buf.
    let n = unsafe { libc::getrandom(buf.as_mut_ptr().cast(), buf.len(), 0) };
    if n != buf.len() as isize {
        // Fallback: time and pid mixed; IDs only need to be unique among this user's accounts.
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) ^ (std::process::id() as u128) << 32;
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (t >> (i * 8)) as u8;
        }
    }
    buf.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect()
}

/// An account ID we wrote ourselves: short and plain, so it is safe in file names and URIs.
pub fn valid_account_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_plain_and_distinct() {
        let a = new_account_id();
        let b = new_account_id();
        assert_eq!(a.len(), 6);
        assert!(valid_account_id(&a) && valid_account_id(&b));
        assert_ne!(a, b);
        assert!(!valid_account_id("../x") && !valid_account_id("") && !valid_account_id("A1"));
        assert_eq!(fs("k3f9q2"), "ef-k3f9q2:");
    }
}

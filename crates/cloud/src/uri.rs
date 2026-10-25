//! `gdrive://<account>/<path inside Drive>` — how Drive files are named in pins, search
//! results and catalogs. Mount paths move (runtime dir), so they are never identity.

use std::path::{Component, Path, PathBuf};

use crate::paths;

pub const SCHEME: &str = "gdrive://";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DriveUri {
    pub account: String,
    /// `/`-separated, relative, as rclone shows it ("" for the Drive root).
    pub path: String,
}

impl DriveUri {
    pub fn new(account: impl Into<String>, path: impl Into<String>) -> Option<DriveUri> {
        let u = DriveUri { account: account.into(), path: path.into().trim_matches('/').to_string() };
        u.valid().then_some(u)
    }

    pub fn parse(text: &str) -> Option<DriveUri> {
        let rest = text.strip_prefix(SCHEME)?;
        let (account, path) = rest.split_once('/').unwrap_or((rest, ""));
        DriveUri::new(account, path)
    }

    fn valid(&self) -> bool {
        paths::valid_account_id(&self.account)
            && !self.path.starts_with('/')
            && self.path.split('/').all(|seg| seg != ".." && seg != "." && !seg.contains('\0'))
            && (self.path.is_empty() || self.path.split('/').all(|seg| !seg.is_empty()))
    }

    /// Where it is while the account is mounted.
    pub fn mount_path(&self) -> PathBuf {
        let root = paths::mount_point(&self.account);
        if self.path.is_empty() { root } else { root.join(&self.path) }
    }

    /// The Drive URI for a path inside a mount.
    pub fn from_mount_path(p: &Path) -> Option<DriveUri> {
        let rel = p.strip_prefix(ef_config::cloud_mount_root()).ok()?;
        let mut parts = rel.components();
        let Some(Component::Normal(account)) = parts.next() else { return None };
        let path: Vec<String> = parts.map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        }).collect::<Option<_>>()?;
        DriveUri::new(account.to_string_lossy(), path.join("/"))
    }

    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or("")
    }

    pub fn parent(&self) -> Option<DriveUri> {
        if self.path.is_empty() {
            return None;
        }
        Some(DriveUri { account: self.account.clone(), path: self.path.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_default() })
    }

    pub fn join(&self, name: &str) -> DriveUri {
        DriveUri { account: self.account.clone(), path: if self.path.is_empty() { name.to_string() } else { format!("{}/{name}", self.path) } }
    }
}

impl std::fmt::Display for DriveUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{SCHEME}{}/{}", self.account, self.path)
    }
}

/// A Drive name as one path segment. Drive allows `/` in names; rclone shows it as `／`
/// (FULLWIDTH SOLIDUS), so a slash in a name never becomes a folder.
pub fn segment(drive_name: &str) -> String {
    drive_name.replace('/', "／")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_rejects_escapes() {
        let u = DriveUri::parse("gdrive://k3f9q2/Work/2026/report.pdf").unwrap();
        assert_eq!(u.account, "k3f9q2");
        assert_eq!(u.name(), "report.pdf");
        assert_eq!(u.to_string(), "gdrive://k3f9q2/Work/2026/report.pdf");
        assert_eq!(DriveUri::from_mount_path(&u.mount_path()), Some(u.clone()));
        assert_eq!(u.parent().unwrap().path, "Work/2026");
        assert_eq!(DriveUri::parse("gdrive://k3f9q2").unwrap().path, "");
        assert_eq!(DriveUri::parse("gdrive://k3f9q2/").unwrap().mount_path(), paths::mount_point("k3f9q2"));
        for bad in ["gdrive://k3f9q2/../etc", "gdrive://../x", "gdrive://AB/x", "phone://a/b", "gdrive://k3f9q2/a//b"] {
            assert!(DriveUri::parse(bad).is_none(), "{bad}");
        }
        assert!(DriveUri::from_mount_path(Path::new("/tmp/x")).is_none());
        assert_eq!(segment("A/B testing"), "A／B testing");
        assert_eq!(DriveUri::parse("gdrive://a1").unwrap().join("x").join("y").path, "x/y");
    }
}

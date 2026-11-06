//! Finding rclone and checking it is new enough. Never installs or downloads anything.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Oldest rclone with everything the design needs: rc on a Unix socket, `mount/mount`,
/// `backend/command`, Drive's `query` and `lsjson` with IDs and hashes (phase 0 spike).
pub const MIN_VERSION: Version = Version(1, 64, 0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

impl Version {
    /// The first line of `rclone version`: `rclone v1.68.2` (also `-beta.…`, `-DEV`).
    pub fn parse(output: &str) -> Option<Version> {
        let line = output.lines().next()?;
        let v = line.split_whitespace().find_map(|w| w.strip_prefix('v'))?;
        let core = v.split(['-', '+']).next()?;
        let mut parts = core.split('.').map(|x| x.parse::<u32>());
        Some(Version(parts.next()?.ok()?, parts.next()?.ok()?, parts.next().unwrap_or(Ok(0)).ok()?))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rclone {
    pub path: PathBuf,
    pub version: Version,
}

impl Rclone {
    /// A command for this rclone with our config file and nothing from the environment that
    /// could point it at the user's own setup.
    pub fn command(&self) -> Command {
        let mut c = Command::new(&self.path);
        c.arg("--config").arg(crate::paths::config_file());
        c.env_remove("RCLONE_CONFIG").env_remove("RCLONE_CONFIG_PASS");
        c.stdin(Stdio::null());
        c
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    Ready(Rclone),
    Missing,
    TooOld { found: Version, path: PathBuf },
}

const PLACES: [&str; 2] = ["/usr/bin/rclone", "/usr/local/bin/rclone"];

/// `rclone` on PATH, then the usual places.
pub fn locate() -> Option<PathBuf> {
    let on_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).map(|d| d.join("rclone"));
    on_path.chain(PLACES.iter().map(PathBuf::from)).find(|p| is_executable(p))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Find rclone and check its version. Runs a process, so call it off the UI thread.
pub fn check() -> Availability {
    let Some(path) = locate() else { return Availability::Missing };
    let out = Command::new(&path).arg("version").stdin(Stdio::null()).stderr(Stdio::null()).output();
    let Some(found) = out.ok().and_then(|o| Version::parse(&String::from_utf8_lossy(&o.stdout))) else { return Availability::Missing };
    if found < MIN_VERSION {
        return Availability::TooOld { found, path };
    }
    Availability::Ready(Rclone { path, version: found })
}

/// FUSE 3's mount helper; without it Drive can't show as a folder.
pub fn fuse_available() -> bool {
    std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).map(|d| d.join("fusermount3")).chain([PathBuf::from("/usr/bin/fusermount3")]).any(|p| is_executable(&p))
}

/// The install command for this distribution, from `/etc/os-release`.
pub fn install_command() -> String {
    install_command_for(&std::fs::read_to_string("/etc/os-release").unwrap_or_default())
}

pub fn install_command_for(os_release: &str) -> String {
    let field = |k: &str| os_release.lines().find_map(|l| l.strip_prefix(k).and_then(|v| v.strip_prefix('='))).map(|v| v.trim_matches('"').to_ascii_lowercase()).unwrap_or_default();
    let ids = format!("{} {}", field("ID"), field("ID_LIKE"));
    let has = |n: &str| ids.split_whitespace().any(|w| w == n);
    if has("arch") {
        "sudo pacman -S rclone".into()
    } else if has("debian") || has("ubuntu") {
        "sudo apt install rclone".into()
    } else if has("fedora") || has("rhel") {
        "sudo dnf install rclone".into()
    } else if has("suse") || has("opensuse") {
        "sudo zypper install rclone".into()
    } else {
        "Install rclone from your distribution's packages".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(Version::parse("rclone v1.68.2\n- os/version: arch"), Some(Version(1, 68, 2)));
        assert_eq!(Version::parse("rclone v1.70.0-beta.8551.abc\n"), Some(Version(1, 70, 0)));
        assert_eq!(Version::parse("rclone v1.64\n"), Some(Version(1, 64, 0)));
        assert_eq!(Version::parse("nonsense"), None);
        assert!(Version(1, 63, 9) < MIN_VERSION && Version(1, 64, 0) >= MIN_VERSION && Version(2, 0, 0) > MIN_VERSION);
    }

    #[test]
    fn install_commands() {
        assert_eq!(install_command_for("NAME=\"Arch Linux\"\nID=arch\n"), "sudo pacman -S rclone");
        assert_eq!(install_command_for("ID=omarchy\nID_LIKE=arch\n"), "sudo pacman -S rclone");
        assert_eq!(install_command_for("ID=ubuntu\nID_LIKE=debian\n"), "sudo apt install rclone");
        assert_eq!(install_command_for("ID=\"fedora\"\n"), "sudo dnf install rclone");
        assert!(install_command_for("").starts_with("Install rclone"));
    }
}

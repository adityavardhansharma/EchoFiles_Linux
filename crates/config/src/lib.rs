//! EchoFiles settings, shared by the app and the `ef` command: one TOML file,
//! `~/.config/echofiles/settings.toml`, edited in Settings.
//!
//! ```toml
//! [general]
//! background = false          # keep running after the window closes
//! start_at_login = false      # launch in the background when you log in
//! open_to = "home"            # "home" or "last"
//! show_hidden = false
//!
//! [search]
//! index = true                # off: search reads folders live instead
//! default_scope = "folder"    # "folder" or "everywhere"
//! roots = ["~"]
//! exclude_paths = []
//! exclude_names = ["node_modules", …]
//! skip_cache_folders = true
//!
//! [agents]
//! cli = true                  # off: `ef` refuses to run
//! skill = false               # EchoFiles skill linked into global agent skill folders
//!
//! [appearance]
//! density = "default"         # "compact", "default" or "comfortable"
//! preview = false             # the preview pane (Space)
//! view = "auto"               # "auto" (grid in picture/video folders), "list" or "grid"
//!
//! [sidebar]
//! pinned = ["~/Projects"]     # the Pinned section, in order
//! width = 236                 # 180–360, dragged at its edge
//! hidden = false              # Ctrl+B
//! windows = true              # show the Windows section (drives)
//! network = true              # show the Network section
//! windows_open = false        # sections start collapsed; the head row toggles them
//! network_open = false
//!
//! phone = true                # show the Phone section
//! phone_open = true
//!
//! [phone]                     # pairings live in ~/.config/echofiles/phone/
//! files = true                # browse the phone's files and photos
//! clipboard = true            # shared clipboard
//! messages = false            # texts (SMS)
//! notifications = "app"       # "off", "app" (Notifications page only) or "desktop"
//! muted_apps = []
//! battery_warning = true
//! auto_accept = true          # save files the phone sends without asking
//! save_to = "~/Downloads/Phone"
//! picture = "photo"           # what the phone on the hub shows: "photo" or "echofiles"
//!
//! [[network.servers]]           # the Network section, in order
//! uri = "smb://nas.local/Media" # never holds a password (GVfs keeps those in the keyring)
//! name = "Media"                # optional; the address's own name otherwise
//! ```

use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Folder names whose contents search skips by default: package and toolchain stores,
/// version-control internals and caches — where most files on a typical laptop live.
pub const DEFAULT_EXCLUDE_NAMES: [&str; 19] = [
    "node_modules", ".git", ".hg", ".svn", "__pycache__", ".cache", ".npm", ".pnpm-store", ".yarn",
    ".gradle", ".m2", ".cargo", ".rustup", ".venv", ".tox", ".mypy_cache", ".pytest_cache", ".next", ".nuxt",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OpenTo {
    #[default]
    Home,
    Last,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Folder,
    Everywhere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    Compact,
    #[default]
    Default,
    Comfortable,
}

impl Density {
    /// List row height in logical pixels (design tokens `row-compact`, `row`, `row-comfy`).
    pub fn row_height(self) -> f32 {
        match self {
            Density::Compact => 24.0,
            Density::Default => 28.0,
            Density::Comfortable => 34.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub background: bool,
    pub start_at_login: bool,
    pub open_to: OpenTo,
    pub show_hidden: bool,
}

impl Default for General {
    fn default() -> Self {
        General { background: false, start_at_login: false, open_to: OpenTo::Home, show_hidden: false }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    pub index: bool,
    pub default_scope: Scope,
    pub roots: Vec<String>,
    pub exclude_paths: Vec<String>,
    pub exclude_names: Vec<String>,
    pub skip_cache_folders: bool,
    /// Also index every drive mounted on this computer (see [`drives`]).
    pub index_drives: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            index: true,
            default_scope: Scope::Folder,
            roots: vec!["~".into()],
            exclude_paths: Vec::new(),
            exclude_names: DEFAULT_EXCLUDE_NAMES.iter().map(|s| s.to_string()).collect(),
            skip_cache_folders: true,
            index_drives: true,
        }
    }
}

impl SearchConfig {
    /// The folders search covers: the ones listed in Settings, plus every mounted drive
    /// that isn't already inside one of them (with `index_drives`).
    pub fn roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = self.roots.iter().map(|r| expand(r)).collect();
        if self.index_drives {
            for d in drives() {
                if !roots.iter().any(|r| d.starts_with(r)) {
                    roots.push(d);
                }
            }
        }
        roots
    }

    /// Excluded paths, expanded, that lie inside `root` (and aren't `root` itself).
    pub fn exclude_paths_in(&self, root: &Path) -> Vec<PathBuf> {
        let root = expand(&root.to_string_lossy());
        self.exclude_paths.iter().map(|p| expand(p)).filter(|p| p.starts_with(&root) || root.starts_with(p)).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Agents {
    pub cli: bool,
    pub skill: bool,
}

impl Default for Agents {
    fn default() -> Self {
        Agents { cli: true, skill: false }
    }
}

/// How folders open: every folder as a list, every folder as a grid, or grid only where
/// pictures and videos live.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DefaultView {
    #[default]
    Auto,
    List,
    Grid,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub density: Density,
    pub preview: bool,
    pub view: DefaultView,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sidebar {
    pub pinned: Vec<String>,
    pub width: u16,
    pub hidden: bool,
    /// Show the Windows section.
    pub windows: bool,
    /// Show the Network section.
    pub network: bool,
    /// The Windows section is expanded (collapsed by default).
    pub windows_open: bool,
    /// The Network section is expanded (collapsed by default).
    pub network_open: bool,
    /// Show the Phone section.
    pub phone: bool,
    /// The Phone section is expanded (open by default).
    pub phone_open: bool,
}

impl Default for Sidebar {
    fn default() -> Self {
        Sidebar { pinned: Vec::new(), width: 236, hidden: false, windows: true, network: true, windows_open: false, network_open: false, phone: true, phone_open: true }
    }
}

/// A saved network place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Network {
    pub servers: Vec<Server>,
}

/// Where phone notifications show up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PhoneNotifications {
    /// Nothing comes over.
    Off,
    /// On the phone's Notifications page only.
    #[default]
    App,
    /// Also as desktop notifications.
    Desktop,
}

/// What the phone drawn on the hub shows on its screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PhonePicture {
    #[default]
    Photo,
    Echofiles,
}

/// Settings → Phone. Pairings themselves live in `~/.config/echofiles/phone/`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Phone {
    pub otp: bool,
    pub dnd_sync: bool,
    pub autolock: bool,
    pub phone_index: bool,
    pub backup: bool,
    /// Browse the phone's files and photos.
    pub files: bool,
    pub clipboard: bool,
    pub messages: bool,
    pub notifications: PhoneNotifications,
    /// App names whose notifications are dropped.
    pub muted_apps: Vec<String>,
    pub battery_warning: bool,
    /// Save files the phone sends without asking.
    pub auto_accept: bool,
    pub save_to: String,
    pub picture: PhonePicture,
}

impl Default for Phone {
    fn default() -> Self {
        Phone {
            otp: false, dnd_sync: false, autolock: false, phone_index: false, backup: false,
            files: true,
            clipboard: true,
            messages: false,
            notifications: PhoneNotifications::App,
            muted_apps: Vec::new(),
            battery_warning: true,
            auto_accept: true,
            save_to: "~/Downloads/Phone".into(),
            picture: PhonePicture::Photo,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub general: General,
    pub search: SearchConfig,
    pub agents: Agents,
    pub appearance: Appearance,
    pub sidebar: Sidebar,
    pub network: Network,
    pub phone: Phone,
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| "/".into())
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

/// `$XDG_CONFIG_HOME/echofiles`
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("echofiles")
}

/// `$XDG_CACHE_HOME/echofiles/index` — one index file per indexed folder.
pub fn index_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache").join("echofiles/index")
}

/// `$XDG_STATE_HOME/echofiles` — last folder and similar things that aren't settings.
pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("echofiles")
}

/// `$XDG_RUNTIME_DIR/echofiles.sock` — the running app listens here.
pub fn socket_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| {
        use std::os::unix::fs::DirBuilderExt;
        let dir = config_dir().join("runtime");
        let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir);
        dir
    }).join("echofiles.sock")
}

/// The index file for one indexed folder: a stable name derived from its path.
pub fn index_file_for(root: &Path) -> PathBuf {
    // FNV-1a: stable across runs and Rust versions, unlike `DefaultHasher`.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in root.as_os_str().as_bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
    }
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "root".into());
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(32).collect();
    index_dir().join(format!("{safe}-{h:016x}.efidx"))
}

impl Settings {
    pub fn path() -> PathBuf {
        config_dir().join("settings.toml")
    }

    /// Saved settings, or defaults when there is no file yet.
    pub fn load_from(path: &Path) -> io::Result<Settings> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(e) => Err(e),
        }
    }

    pub fn load() -> io::Result<Settings> {
        Settings::load_from(&Settings::path())
    }

    /// Atomic write: temp file + rename.
    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(io::Error::other)?;
        atomic_write(path, text.as_bytes())
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to(&Settings::path())
    }
}

/// Where the drives attached to this computer are mounted: every disk filesystem (internal
/// or removable) except the one the system runs from, skipping system places (`/boot`,
/// `/var`, …), network and virtual filesystems, folders inside the home folder (already
/// searched) and places this user can't read. One mount point per drive.
pub fn drives() -> Vec<PathBuf> {
    drives_from(&std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default(), &home())
}

/// [`drives`] for a given `/proc/self/mountinfo` text.
pub fn drives_from(mountinfo: &str, home: &Path) -> Vec<PathBuf> {
    // `\040` and friends: octal escapes for spaces, tabs, newlines and backslashes.
    fn unescape(s: &str) -> PathBuf {
        let b = s.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\' && i + 3 < b.len() && b[i + 1..i + 4].iter().all(|c| (b'0'..=b'7').contains(c)) {
                out.push((b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0'));
                i += 4;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        PathBuf::from(std::ffi::OsStr::from_bytes(&out))
    }
    const SYSTEM: [&str; 13] = ["/boot", "/efi", "/usr", "/var", "/opt", "/srv", "/tmp", "/proc", "/sys", "/dev", "/snap", "/etc", "/root"];
    // (device, mount point, filesystem, source)
    let mounts: Vec<(&str, PathBuf, &str, &str)> = mountinfo
        .lines()
        .filter_map(|l| {
            let (left, right) = l.split_once(" - ")?;
            let f: Vec<&str> = left.split(' ').collect();
            let r: Vec<&str> = right.split(' ').collect();
            Some((*f.get(2)?, unescape(f.get(4)?), *r.first()?, *r.get(1)?))
        })
        .collect();
    let system_dev = mounts.iter().find(|m| m.1 == Path::new("/")).map(|m| m.0);
    let mut out: Vec<(&str, PathBuf)> = Vec::new();
    for (dev, at, fs, source) in &mounts {
        let virtual_fs = matches!(*fs, "squashfs" | "iso9660" | "udf" | "overlay" | "tmpfs" | "devtmpfs" | "ramfs") || (fs.starts_with("fuse") && *fs != "fuseblk");
        let system_place = at == Path::new("/")
            || SYSTEM.iter().any(|s| at.starts_with(s))
            || (at.starts_with("/run") && !at.starts_with("/run/media"));
        if !source.starts_with("/dev/") || virtual_fs || system_place || Some(*dev) == system_dev || at.starts_with(home) {
            continue;
        }
        if rustix::fs::access(at.as_path(), rustix::fs::Access::READ_OK | rustix::fs::Access::EXEC_OK).is_err() {
            continue;
        }
        match out.iter_mut().find(|o| o.0 == *dev) {
            // The same drive mounted twice (bind mounts, btrfs subvolumes): keep the shortest path.
            Some(o) if at.as_os_str().len() < o.1.as_os_str().len() => o.1 = at.clone(),
            Some(_) => {}
            None => out.push((dev, at.clone())),
        }
    }
    let mut v: Vec<PathBuf> = out.into_iter().map(|o| o.1).collect();
    v.sort();
    v
}

/// Roots from `searched` that lie strictly inside `root`. Each is searched on its own, so a
/// search of `root` skips them rather than listing their contents twice.
pub fn nested_roots(searched: &[PathBuf], root: &Path) -> Vec<PathBuf> {
    searched.iter().filter(|r| r.as_path() != root && r.starts_with(root)).cloned().collect()
}

/// `~` and `~/…` become the home folder; a trailing slash is dropped.
pub fn expand(p: &str) -> PathBuf {
    let p = p.trim();
    let path = match p.strip_prefix('~') {
        Some("") => home(),
        Some(rest) if rest.starts_with('/') => home().join(&rest[1..]),
        _ => PathBuf::from(p),
    };
    let s = path.as_os_str().as_bytes();
    let trimmed = if s.len() > 1 { s.strip_suffix(b"/").unwrap_or(s) } else { s };
    PathBuf::from(std::ffi::OsStr::from_bytes(trimmed))
}

/// `/home/me/Work` → `~/Work`, the way people write paths.
pub fn tilde(p: &Path) -> String {
    match p.strip_prefix(home()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.toml");
        assert_eq!(Settings::load_from(&file).unwrap(), Settings::default());
        let mut s = Settings::default();
        s.general.background = true;
        s.search.index = false;
        s.agents.cli = false;
        s.appearance.density = Density::Compact;
        s.network.servers.push(Server { uri: "smb://nas/Media".into(), name: None });
        s.network.servers.push(Server { uri: "sftp://me@box/".into(), name: Some("Box".into()) });
        s.save_to(&file).unwrap();
        assert_eq!(Settings::load_from(&file).unwrap(), s);
        // A file with only one section keeps defaults for the rest.
        std::fs::write(&file, "[agents]\ncli = false\n").unwrap();
        let partial = Settings::load_from(&file).unwrap();
        assert!(!partial.agents.cli);
        assert_eq!(partial.search, SearchConfig::default());
    }

    #[test]
    fn drives_are_disks_outside_the_system() {
        let info = "\
22 1 0:25 /@ / rw - btrfs /dev/mapper/root rw
23 22 0:25 /@home /home rw - btrfs /dev/mapper/root rw
24 22 0:25 /@log /var/log rw - btrfs /dev/mapper/root rw
25 22 259:8 / /boot rw - vfat /dev/nvme0n1p8 rw
26 22 0:40 / /media/data106 rw - btrfs /dev/mapper/data106 rw
27 22 0:40 /sub /media/data106-bind rw - btrfs /dev/mapper/data106 rw
28 22 8:17 / /run/media/me/USB\040Stick rw - exfat /dev/sdb1 rw
29 22 0:50 / /run/user/1000/doc rw - fuse.portal portal rw
30 22 0:51 / /mnt/nas rw - cifs //nas/share rw
31 22 0:52 / /tmp rw - tmpfs tmpfs rw
32 22 8:33 / /home/me/inside rw - ext4 /dev/sdc1 rw
33 22 8:49 / /mnt/ntfs rw - fuseblk /dev/sdd1 rw
34 22 7:0 / /var/lib/snapd/snap/core rw - squashfs /dev/loop0 ro
";
        // Mount points that don't exist here fail the readability check, so the realistic
        // table above can only lose entries; every rule is checked below with "/media",
        // which exists on any test machine.
        for d in drives_from(info, Path::new("/home/me")) {
            assert!(!d.starts_with("/boot") && !d.starts_with("/home") && !d.starts_with("/tmp") && !d.starts_with("/mnt/nas") && !d.starts_with("/var"), "{d:?}");
        }
        let names = |i: &str| drives_from(i, Path::new("/home/me"));
        assert!(names("22 1 0:25 /@ / rw - btrfs /dev/mapper/root rw\n23 22 0:25 /@home /media rw - btrfs /dev/mapper/root rw\n").is_empty(), "same device as the system");
        assert_eq!(names("22 1 0:25 /@ / rw - btrfs /dev/mapper/root rw\n26 22 0:40 / /media rw - btrfs /dev/mapper/data rw\n27 22 0:40 /x /media/deeper rw - btrfs /dev/mapper/data rw\n"), vec![PathBuf::from("/media")], "one path per drive, the shortest");
        assert!(names("22 1 0:25 /@ / rw - btrfs /dev/mapper/root rw\n30 22 0:51 / /media rw - cifs //nas/share rw\n").is_empty(), "network");
        assert!(names("22 1 0:25 /@ / rw - btrfs /dev/mapper/root rw\n29 22 0:50 / /media rw - fuse.portal portal rw\n").is_empty(), "virtual");
        assert_eq!(names("22 1 0:25 /@ / rw - btrfs /dev/mapper/root rw\n33 22 8:49 / /media rw - fuseblk /dev/sdd1 rw\n"), vec![PathBuf::from("/media")], "ntfs-3g");
        assert!(names("22 1 0:25 /@ / rw - btrfs /dev/mapper/root rw\n25 22 259:8 / /boot rw - vfat /dev/nvme0n1p8 rw\n").is_empty(), "system places");
    }

    #[test]
    fn paths_expand_and_index_files_are_stable() {
        assert_eq!(expand("~"), home());
        assert_eq!(expand("~/Work/"), home().join("Work"));
        assert_eq!(tilde(&home().join("Work")), "~/Work");
        let a = index_file_for(Path::new("/home/me"));
        assert_eq!(a, index_file_for(Path::new("/home/me")));
        assert_ne!(a, index_file_for(Path::new("/home/you")));
        assert!(a.file_name().unwrap().to_string_lossy().starts_with("me-"));
    }
}

/// Publish a private, complete snapshot; concurrent writers never share a staging inode.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let dir = path.parent().ok_or_else(|| io::Error::other("missing parent"))?;
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    std::fs::File::open(dir)?.sync_all()
}

/// FIFO snapshots, enqueued before spawning work so older state cannot land last.
pub fn queue_state(path: PathBuf, bytes: Vec<u8>) {
    static WRITER: std::sync::OnceLock<std::sync::mpsc::Sender<(PathBuf, Vec<u8>)>> = std::sync::OnceLock::new();
    let tx = WRITER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<(PathBuf, Vec<u8>)>();
        std::thread::spawn(move || for (path, bytes) in rx { if let Err(e) = atomic_write(&path, &bytes) { eprintln!("Couldn't save {}: {e}", path.display()); } });
        tx
    });
    let _ = tx.send((path, bytes));
}

#[cfg(test)]
mod review_regressions {
    use super::*;
    #[test]
    fn competing_settings_writers_publish_valid_complete_snapshots() {
        let root = tempfile::tempdir().unwrap(); let file = root.path().join("settings.toml");
        std::thread::scope(|scope| for n in 0..8 { let file = &file; scope.spawn(move || {
            let mut s = Settings::default(); s.sidebar.width = 200 + n;
            for _ in 0..20 { s.save_to(file).unwrap(); let loaded = Settings::load_from(file).unwrap(); assert!((200..208).contains(&loaded.sidebar.width)); }
        }); });
    }
}

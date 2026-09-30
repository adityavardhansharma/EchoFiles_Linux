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
        }
    }
}

impl SearchConfig {
    pub fn roots(&self) -> Vec<PathBuf> {
        self.roots.iter().map(|r| expand(r)).collect()
    }

    /// Excluded paths, expanded, that lie inside `root` (and aren't `root` itself).
    pub fn exclude_paths_in(&self, root: &Path) -> Vec<PathBuf> {
        let root = expand(&root.to_string_lossy());
        self.exclude_paths.iter().map(|p| expand(p)).filter(|p| p.starts_with(&root) && p != &root).collect()
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
}

impl Default for Sidebar {
    fn default() -> Self {
        Sidebar { pinned: Vec::new(), width: 236, hidden: false }
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub general: General,
    pub search: SearchConfig,
    pub agents: Agents,
    pub appearance: Appearance,
    pub sidebar: Sidebar,
    pub network: Network,
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
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("echofiles.sock")
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
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, path)
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to(&Settings::path())
    }
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

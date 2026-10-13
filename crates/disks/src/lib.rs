//! Drives through udisks2 over the system D-Bus (build plan §3): discovery, drive letters,
//! mount and unmount. Mounting an internal drive needs an administrator password; see
//! [`polkit`] for how EchoFiles asks for it.

pub mod letters;
pub mod polkit;

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

/// Partitions smaller than this (recovery, EFI) are never shown.
const MIN_SIZE: u64 = 1 << 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    /// udisks object path of the block device.
    pub object: String,
    /// `/dev/nvme0n1p4`
    pub device: String,
    /// Filesystem UUID — the stable identity (labels can repeat, mount paths move).
    pub uuid: String,
    pub label: String,
    /// GPT partition name (`Marchy`); for an unlocked LUKS volume, the name of the
    /// partition it lives on.
    pub part_name: String,
    /// `ntfs`, `btrfs`, …
    pub fs_type: String,
    pub size: u64,
    pub mount_points: Vec<String>,
    /// GPT partition GUID — how Windows' registry names the partition.
    pub part_uuid: String,
    /// Encrypted and still locked (BitLocker, or LUKS with no cleartext device yet):
    /// nothing to mount until it's unlocked.
    pub locked: bool,
    /// Windows drive letter, once known from the registry.
    pub letter: Option<char>,
    /// Holds Windows itself (C:); mounted read-only unless writing is allowed.
    pub system: bool,
}

impl Volume {
    pub fn is_ntfs(&self) -> bool {
        self.fs_type.starts_with("ntfs")
    }

    pub fn is_bitlocker(&self) -> bool {
        self.fs_type == "BitLocker"
    }

    /// Windows sees this partition (NTFS or BitLocker).
    pub fn is_windows(&self) -> bool {
        self.is_ntfs() || self.is_bitlocker()
    }

    /// "BitLocker", "LUKS", "btrfs" — what the drive is, for facts and tooltips.
    pub fn kind(&self) -> &str {
        match self.fs_type.as_str() {
            "crypto_LUKS" => "LUKS",
            t if t.starts_with("ntfs") => "NTFS",
            t => t,
        }
    }

    pub fn is_mounted(&self) -> bool {
        !self.mount_points.is_empty()
    }

    /// Name shown before drive letters are known (M2 reads them from the Windows registry).
    /// A Linux drive goes by the partition name you gave it, then its filesystem label,
    /// then where it's mounted.
    pub fn display_name(&self) -> String {
        if !self.is_windows() && !self.part_name.is_empty() && !generic_part_name(&self.part_name) {
            return self.part_name.clone();
        }
        if !self.label.is_empty() {
            return self.label.clone();
        }
        if let Some(base) = self.mount_points.first().and_then(|m| m.rsplit('/').next()).filter(|b| !b.is_empty()) {
            return base.to_string();
        }
        format!("{} {}", self.kind(), &self.uuid[..self.uuid.len().min(8)])
    }

    /// Part of the running system (/, /boot, /home, …) rather than a drive of its own.
    fn is_system(&self) -> bool {
        const SYSTEM: &[&str] = &["/boot", "/efi", "/home", "/nix", "/opt", "/root", "/snap", "/srv", "/tmp", "/usr", "/var"];
        self.mount_points.iter().any(|m| m == "/" || SYSTEM.iter().any(|s| m == s || m.starts_with(&format!("{s}/"))))
    }
}

#[derive(Debug)]
pub struct Error(zbus::Error);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "couldn't ask udisks for drives: {}", self.0)
    }
}

impl std::error::Error for Error {}

/// Names partitioning tools give by default — not something anyone chose.
fn generic_part_name(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    n.starts_with("linux ") || n.starts_with("microsoft ") || n.starts_with("efi ") || matches!(n.as_str(), "basic data partition" | "primary" | "logical" | "extended" | "data" | "linux")
}

impl From<zbus::Error> for Error {
    fn from(e: zbus::Error) -> Self {
        Error(e)
    }
}

type Interfaces = HashMap<String, HashMap<String, OwnedValue>>;

/// All NTFS and BitLocker volumes of at least 1 GB that udisks doesn't ask us to hide,
/// with their drive letters when known.
pub fn windows_volumes() -> Result<Vec<Volume>, Error> {
    let cache = letters::load();
    let mut out: Vec<Volume> = all_volumes()?.into_iter().filter(Volume::is_windows).collect();
    for v in &mut out {
        v.letter = cache.letters.get(&v.part_uuid).copied();
        v.system = cache.system.contains(&v.part_uuid);
    }
    // Windows order: by drive letter, then the rest by device.
    out.sort_by(|a, b| (a.letter.is_none(), a.letter, &a.device).cmp(&(b.letter.is_none(), b.letter, &b.device)));
    Ok(out)
}

/// Linux drives: every filesystem that isn't Windows' or part of the running system
/// (mounted at /, /boot, /home, …), plus LUKS partitions still waiting to be unlocked.
pub fn linux_volumes() -> Result<Vec<Volume>, Error> {
    let mut out: Vec<Volume> = all_volumes()?.into_iter().filter(|v| !v.is_windows() && !v.is_system() && v.fs_type != "squashfs").collect();
    out.sort_by_cached_key(|v| (v.display_name().to_lowercase(), v.device.clone()));
    Ok(out)
}

/// Every filesystem-bearing block device udisks knows, sorted by device path.
pub fn all_volumes() -> Result<Vec<Volume>, Error> {
    let conn = Connection::system()?;
    let reply = conn.call_method(
        Some("org.freedesktop.UDisks2"),
        "/org/freedesktop/UDisks2",
        Some("org.freedesktop.DBus.ObjectManager"),
        "GetManagedObjects",
        &(),
    )?;
    let objects: HashMap<OwnedObjectPath, Interfaces> = reply.body().deserialize()?;
    let mut out: Vec<Volume> = objects
        .iter()
        .filter_map(|(path, ifaces)| volume(path.as_str(), ifaces, &objects))
        .collect();
    out.sort_by(|a, b| a.device.cmp(&b.device));
    Ok(out)
}

fn volume(object: &str, ifaces: &Interfaces, all: &HashMap<OwnedObjectPath, Interfaces>) -> Option<Volume> {
    let block = ifaces.get("org.freedesktop.UDisks2.Block")?;
    let fs_type = block.get("IdType").and_then(as_string).unwrap_or_default();
    // An unlocked LUKS container shows up again as its cleartext device; list that one.
    let luks_locked = ifaces
        .get("org.freedesktop.UDisks2.Encrypted")
        .is_some_and(|e| e.get("CleartextDevice").and_then(as_object_path).is_none_or(|p| p == "/"));
    let locked = fs_type == "BitLocker" || (fs_type == "crypto_LUKS" && luks_locked);
    let fs = ifaces.get("org.freedesktop.UDisks2.Filesystem");
    if fs.is_none() && !locked {
        return None;
    }
    if block.get("HintIgnore").and_then(as_bool).unwrap_or(false) {
        return None;
    }
    let size = block.get("Size").and_then(as_u64).unwrap_or(0);
    if size < MIN_SIZE {
        return None;
    }
    Some(Volume {
        object: object.to_string(),
        device: block.get("Device").and_then(as_bytestring).unwrap_or_default(),
        uuid: block.get("IdUUID").and_then(as_string).unwrap_or_default(),
        label: block.get("IdLabel").and_then(as_string).unwrap_or_default(),
        fs_type,
        size,
        mount_points: fs.and_then(|f| f.get("MountPoints")).map(as_bytestrings).unwrap_or_default(),
        part_name: part_name(ifaces).or_else(|| {
            let backing = block.get("CryptoBackingDevice").and_then(as_object_path).filter(|p| p != "/")?;
            all.iter().find(|(o, _)| o.as_str() == backing).and_then(|(_, i)| part_name(i))
        }).unwrap_or_default(),
        part_uuid: ifaces.get("org.freedesktop.UDisks2.Partition").and_then(|p| p.get("UUID")).and_then(as_string).unwrap_or_default().to_lowercase(),
        locked,
        letter: None,
        system: false,
    })
}

fn part_name(ifaces: &Interfaces) -> Option<String> {
    ifaces.get("org.freedesktop.UDisks2.Partition")?.get("Name").and_then(as_string).filter(|n| !n.is_empty())
}

fn as_object_path(v: &OwnedValue) -> Option<String> {
    match &**v {
        Value::ObjectPath(p) => Some(p.to_string()),
        _ => None,
    }
}

fn as_bool(v: &OwnedValue) -> Option<bool> {
    match &**v {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

fn as_u64(v: &OwnedValue) -> Option<u64> {
    match &**v {
        Value::U64(n) => Some(*n),
        _ => None,
    }
}

fn as_string(v: &OwnedValue) -> Option<String> {
    match &**v {
        Value::Str(s) => Some(s.to_string()),
        _ => None,
    }
}

/// udisks sends paths as NUL-terminated byte arrays (`ay`).
fn bytes_to_string(v: &Value<'_>) -> Option<String> {
    let Value::Array(arr) = v else { return None };
    let mut bytes: Vec<u8> = arr.iter().filter_map(|b| if let Value::U8(b) = b { Some(*b) } else { None }).collect();
    while bytes.last() == Some(&0) {
        bytes.pop();
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn as_bytestring(v: &OwnedValue) -> Option<String> {
    bytes_to_string(v)
}

fn as_bytestrings(v: &OwnedValue) -> Vec<String> {
    match &**v {
        Value::Array(arr) => arr.iter().filter_map(bytes_to_string).collect(),
        _ => Vec::new(),
    }
}

// ------------------------------------------------------------------------------ mounting

/// Why a mount didn't happen, in words for a banner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountError {
    /// The password prompt was cancelled or failed.
    NotAuthorized,
    Failed(String),
}

impl std::fmt::Display for MountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MountError::NotAuthorized => write!(f, "mounting needs your password, and it wasn't given"),
            MountError::Failed(m) => write!(f, "{m}"),
        }
    }
}

/// A finished mount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mounted {
    pub mount_point: String,
    pub read_only: bool,
    /// Why it's read-only when we didn't ask for that (Windows Fast Startup, hibernation).
    pub forced_read_only: Option<String>,
    /// This partition holds Windows (letters were read from its registry).
    pub system: bool,
}

fn udisks_error(e: zbus::Error) -> MountError {
    let text = e.to_string();
    if text.contains("NotAuthorized") {
        return MountError::NotAuthorized;
    }
    // "GDBus.Error:org.freedesktop.UDisks2.Error.Failed: Error mounting /dev/…: wrong fs type…"
    let short = text.rsplit(": ").next().unwrap_or(&text).trim().to_string();
    MountError::Failed(short)
}

fn call_mount(conn: &Connection, object: &str, read_only: bool) -> Result<String, MountError> {
    // SAFETY: getuid/getgid can't fail.
    let (uid, gid) = unsafe { (getuid(), getgid()) };
    let mut opts = format!("uid={uid},gid={gid},windows_names,prealloc,iocharset=utf8");
    if read_only {
        opts.push_str(",ro");
    }
    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("fstype", Value::from("ntfs3"));
    options.insert("options", Value::from(opts.as_str()));
    let reply = conn
        .call_method(Some("org.freedesktop.UDisks2"), object, Some("org.freedesktop.UDisks2.Filesystem"), "Mount", &(options,))
        .map_err(udisks_error)?;
    reply.body().deserialize::<String>().map_err(|e| MountError::Failed(e.to_string()))
}

fn call_unmount(conn: &Connection, object: &str) -> Result<(), MountError> {
    let options: HashMap<&str, Value> = HashMap::new();
    conn.call_method(Some("org.freedesktop.UDisks2"), object, Some("org.freedesktop.UDisks2.Filesystem"), "Unmount", &(options,))
        .map_err(udisks_error)?;
    Ok(())
}

unsafe extern "C" {
    fn getuid() -> u32;
    fn getgid() -> u32;
}

/// A Linux filesystem mounts as itself, with no ntfs3 options; udisks picks the folder.
fn call_mount_native(conn: &Connection, object: &str) -> Result<String, MountError> {
    let options: HashMap<&str, Value> = HashMap::new();
    let reply = conn
        .call_method(Some("org.freedesktop.UDisks2"), object, Some("org.freedesktop.UDisks2.Filesystem"), "Mount", &(options,))
        .map_err(udisks_error)?;
    reply.body().deserialize::<String>().map_err(|e| MountError::Failed(e.to_string()))
}

/// Mount a Windows volume with ntfs3. The Windows system partition (C:) mounts read-only
/// unless `write_system` is set; a volume Windows didn't shut down cleanly (Fast Startup,
/// hibernation) falls back to read-only instead of failing. Reading the registry on the
/// way records every partition's drive letter.
pub fn mount(v: &Volume, write_system: bool) -> Result<Mounted, MountError> {
    let conn = Connection::system().map_err(|e| MountError::Failed(e.to_string()))?;
    if !v.is_windows() {
        let mp = call_mount_native(&conn, &v.object)?;
        return Ok(Mounted { mount_point: mp, read_only: false, forced_read_only: None, system: false });
    }
    let want_ro = !write_system;
    let (mp, forced) = match call_mount(&conn, &v.object, want_ro) {
        Ok(mp) => (mp, None),
        Err(MountError::NotAuthorized) => return Err(MountError::NotAuthorized),
        Err(first) if !want_ro => match call_mount(&conn, &v.object, true) {
            Ok(mp) => (mp, Some(format!("Windows didn't shut down fully (Fast Startup or hibernation), so it opened read-only. ({first})"))),
            Err(_) => return Err(first),
        },
        Err(e) => return Err(e),
    };
    let mut system = v.system;
    if let Some(hive) = letters::system_hive(std::path::Path::new(&mp)) {
        system = true;
        let mut cache = letters::load();
        if let Ok(found) = letters::read(&hive) {
            cache.letters.extend(found);
        }
        if !cache.system.contains(&v.part_uuid) {
            cache.system.push(v.part_uuid.clone());
        }
        letters::save(&cache);
    }
    // Identify an unfamiliar volume while read-only. Only a non-system volume is
    // reopened writable automatically; a Windows system volume needs explicit consent.
    if !system && want_ro && forced.is_none() {
        call_unmount(&conn, &v.object)?;
        return match call_mount(&conn, &v.object, false) {
            Ok(mp) => Ok(Mounted { mount_point: mp, read_only: false, forced_read_only: None, system }),
            Err(e) => {
                let mp = call_mount(&conn, &v.object, true)?;
                Ok(Mounted { mount_point: mp, read_only: true, forced_read_only: Some(format!("Opened read-only: {e}")), system })
            }
        };
    }
    let read_only = want_ro || forced.is_some();
    Ok(Mounted { mount_point: mp, read_only, forced_read_only: forced, system })
}

/// Remount read-write (the "Allow writing" action on C:) or read-only.
pub fn remount(v: &Volume, read_only: bool) -> Result<Mounted, MountError> {
    let conn = Connection::system().map_err(|e| MountError::Failed(e.to_string()))?;
    if v.is_mounted() {
        call_unmount(&conn, &v.object)?;
    }
    let mp = call_mount(&conn, &v.object, read_only)?;
    Ok(Mounted { mount_point: mp, read_only, forced_read_only: None, system: v.system })
}

/// Unmount; fails with udisks' reason when files are still open there.
pub fn unmount(v: &Volume) -> Result<(), MountError> {
    let conn = Connection::system().map_err(|e| MountError::Failed(e.to_string()))?;
    call_unmount(&conn, &v.object)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vol(fs_type: &str, part_name: &str, label: &str, mounts: &[&str]) -> Volume {
        Volume {
            object: String::new(),
            device: "/dev/dm-1".into(),
            uuid: "f2af5807-5436".into(),
            label: label.into(),
            part_name: part_name.into(),
            fs_type: fs_type.into(),
            size: 100 << 30,
            mount_points: mounts.iter().map(|m| m.to_string()).collect(),
            part_uuid: String::new(),
            locked: false,
            letter: None,
            system: false,
        }
    }

    #[test]
    fn linux_drives_go_by_the_partition_name_you_chose() {
        assert_eq!(vol("btrfs", "Marchy", "data106", &["/media/data106"]).display_name(), "Marchy");
        assert_eq!(vol("ext4", "Linux filesystem", "Backup", &[]).display_name(), "Backup");
        assert_eq!(vol("ext4", "", "", &["/run/media/me/stick"]).display_name(), "stick");
        assert_eq!(vol("crypto_LUKS", "", "", &[]).display_name(), "LUKS f2af5807");
        // Windows keeps its own naming: "Basic data partition" is never a name.
        assert_eq!(vol("ntfs", "Basic data partition", "AVS", &[]).display_name(), "AVS");
    }

    #[test]
    fn system_mounts_are_not_drives() {
        assert!(vol("btrfs", "", "", &["/", "/home"]).is_system());
        assert!(vol("vfat", "", "", &["/boot"]).is_system());
        assert!(vol("ext4", "", "", &["/var/lib/docker"]).is_system());
        assert!(!vol("btrfs", "", "", &["/media/data106"]).is_system());
        assert!(!vol("ext4", "", "", &["/homework"]).is_system());
    }
}

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
    /// `ntfs`, `btrfs`, …
    pub fs_type: String,
    pub size: u64,
    pub mount_points: Vec<String>,
    /// GPT partition GUID — how Windows' registry names the partition.
    pub part_uuid: String,
    /// BitLocker-encrypted: nothing to mount until it's unlocked.
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

    /// Windows sees this partition (NTFS or BitLocker).
    pub fn is_windows(&self) -> bool {
        self.is_ntfs() || self.locked
    }

    pub fn is_mounted(&self) -> bool {
        !self.mount_points.is_empty()
    }

    /// Name shown before drive letters are known (M2 reads them from the Windows registry).
    pub fn display_name(&self) -> String {
        if !self.label.is_empty() {
            self.label.clone()
        } else {
            format!("NTFS {}", &self.uuid[..self.uuid.len().min(8)])
        }
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
        .into_iter()
        .filter_map(|(path, ifaces)| volume(path.as_str(), &ifaces))
        .collect();
    out.sort_by(|a, b| a.device.cmp(&b.device));
    Ok(out)
}

fn volume(object: &str, ifaces: &Interfaces) -> Option<Volume> {
    let block = ifaces.get("org.freedesktop.UDisks2.Block")?;
    let fs_type = block.get("IdType").and_then(as_string).unwrap_or_default();
    let locked = fs_type == "BitLocker";
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
        part_uuid: ifaces.get("org.freedesktop.UDisks2.Partition").and_then(|p| p.get("UUID")).and_then(as_string).unwrap_or_default().to_lowercase(),
        locked,
        letter: None,
        system: false,
    })
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

/// Mount a Windows volume with ntfs3. The Windows system partition (C:) mounts read-only
/// unless `write_system` is set; a volume Windows didn't shut down cleanly (Fast Startup,
/// hibernation) falls back to read-only instead of failing. Reading the registry on the
/// way records every partition's drive letter.
pub fn mount(v: &Volume, write_system: bool) -> Result<Mounted, MountError> {
    let conn = Connection::system().map_err(|e| MountError::Failed(e.to_string()))?;
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

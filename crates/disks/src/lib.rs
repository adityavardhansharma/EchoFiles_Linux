//! Drive discovery through udisks2 over the system D-Bus (build plan §3). Read-only in M1:
//! mounting arrives in M2.

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
}

impl Volume {
    pub fn is_ntfs(&self) -> bool {
        self.fs_type.starts_with("ntfs")
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

/// All NTFS volumes of at least 1 GB that udisks doesn't ask us to hide.
pub fn windows_volumes() -> Result<Vec<Volume>, Error> {
    Ok(all_volumes()?.into_iter().filter(Volume::is_ntfs).collect())
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
    let fs = ifaces.get("org.freedesktop.UDisks2.Filesystem")?;
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
        fs_type: block.get("IdType").and_then(as_string).unwrap_or_default(),
        size,
        mount_points: fs.get("MountPoints").map(as_bytestrings).unwrap_or_default(),
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

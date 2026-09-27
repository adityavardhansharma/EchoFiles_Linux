//! Windows drive letters from the SYSTEM registry hive (`HKLM\SYSTEM\MountedDevices`), so
//! partitions show as "Windows (C:)" and "AVS (D:)" (build plan §3).
//!
//! GPT partitions are recorded as `DMIO:ID:` + the partition GUID, which udisks reports as
//! `Partition.UUID`. Letters are cached by partition GUID so unmounted drives keep theirs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Where the SYSTEM hive sits inside a mounted Windows volume.
pub fn system_hive(mount_point: &Path) -> Option<PathBuf> {
    // NTFS is case-insensitive on Windows but ntfs3 may be case-sensitive: try the usual casing.
    for p in ["Windows/System32/config/SYSTEM", "WINDOWS/system32/config/SYSTEM", "Windows/system32/config/SYSTEM"] {
        let f = mount_point.join(p);
        if f.is_file() {
            return Some(f);
        }
    }
    None
}

/// Format a Windows (mixed-endian) GUID as udisks prints it.
fn guid(b: &[u8]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Partition GUID → drive letter, read from a SYSTEM hive file.
pub fn read(hive_path: &Path) -> Result<HashMap<String, char>, String> {
    let bytes = std::fs::read(hive_path).map_err(|e| e.to_string())?;
    let hive = nt_hive::Hive::without_validation(bytes.as_slice()).map_err(|e| e.to_string())?;
    let root = hive.root_key_node().map_err(|e| e.to_string())?;
    let devices = root.subpath("MountedDevices").ok_or("no MountedDevices key")?.map_err(|e| e.to_string())?;
    let mut out = HashMap::new();
    let values = devices.values().ok_or("no values")?.map_err(|e| e.to_string())?;
    for v in values.flatten() {
        let Ok(name) = v.name() else { continue };
        let name = name.to_string_lossy();
        let Some(letter) = name.strip_prefix("\\DosDevices\\").and_then(|r| r.strip_suffix(':')).and_then(|r| r.chars().next()) else { continue };
        let Ok(data) = v.data().and_then(|d| d.into_vec()) else { continue };
        if data.len() == 24 && data.starts_with(b"DMIO:ID:") {
            out.insert(guid(&data[8..24]), letter.to_ascii_uppercase());
        }
    }
    Ok(out)
}

fn cache_file() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
    state.join("echofiles/drive-letters")
}

/// Cached letters (`guid=C` lines), plus which partitions hold Windows itself (`system=guid`).
#[derive(Default, Clone, Debug)]
pub struct Cache {
    pub letters: HashMap<String, char>,
    pub system: Vec<String>,
}

pub fn load() -> Cache {
    let mut c = Cache::default();
    let Ok(text) = std::fs::read_to_string(cache_file()) else { return c };
    for line in text.lines() {
        match line.split_once('=') {
            Some(("system", g)) => c.system.push(g.to_string()),
            Some((g, l)) => {
                if let Some(ch) = l.chars().next() {
                    c.letters.insert(g.to_string(), ch);
                }
            }
            None => {}
        }
    }
    c
}

pub fn save(c: &Cache) {
    let mut s = String::new();
    for (g, l) in &c.letters {
        s.push_str(&format!("{g}={l}\n"));
    }
    for g in &c.system {
        s.push_str(&format!("system={g}\n"));
    }
    let f = cache_file();
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(f, s);
}

#[cfg(test)]
mod tests {
    #[test]
    fn formats_guids() {
        let b = [0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
        assert_eq!(super::guid(&b), "00112233-4455-6677-8899-aabbccddeeff");
    }
}

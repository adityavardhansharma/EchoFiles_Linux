//! One Google Drive account through the rcd: create the remote, mount it, ask about quota,
//! uploads and folder contents, delete for good, remove the account.
//!
//! Option names and value shapes follow rclone's rc docs (phase 0); the ones a real Drive
//! hasn't confirmed yet are noted in design/docs/google-drive.md §18.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use serde_json::{json, Map, Value};

use crate::error::{Error, Kind};
use crate::paths;
use crate::rc::{self, Rc};

/// What a sign-in may do (rclone's Drive `scope`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Full,
    Read,
}

impl Access {
    pub fn scope(self) -> &'static str {
        match self {
            Access::Full => "drive",
            Access::Read => "drive.readonly",
        }
    }
}

impl From<ef_config::CloudAccess> for Access {
    fn from(a: ef_config::CloudAccess) -> Access {
        match a {
            ef_config::CloudAccess::Full => Access::Full,
            ef_config::CloudAccess::Read => Access::Read,
        }
    }
}

/// The user's own Google app. Only ever sent over the socket, never on a command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Client {
    pub id: String,
    pub secret: String,
}

/// A client ID Google would accept the shape of.
pub fn valid_client_id(id: &str) -> bool {
    let id = id.trim();
    id.ends_with(".apps.googleusercontent.com") && id.len() > ".apps.googleusercontent.com".len() && !id.contains(char::is_whitespace)
}

/// Add the account's remote to our config, without signing in yet.
pub fn create(rc: &Rc, account: &str, access: Access, client: Option<&Client>) -> Result<(), Error> {
    let mut params = Map::new();
    params.insert("scope".into(), json!(access.scope()));
    // Google Docs show in the mount as small HTML files that open the document in the browser.
    params.insert("export_formats".into(), json!("link.html"));
    if let Some(c) = client {
        params.insert("client_id".into(), json!(c.id.trim()));
        params.insert("client_secret".into(), json!(c.secret.trim()));
    }
    rc.call("config/create", &json!({ "name": paths::remote(account), "type": "drive", "parameters": params, "opt": { "nonInteractive": true, "obscure": true } }), rc::METADATA)?;
    Ok(())
}

/// Change the remote's client (empty values go back to rclone's own app). Signing in
/// again is the caller's next step: a login belongs to the app that made it.
pub fn set_client(rc: &Rc, account: &str, client: Option<&Client>) -> Result<(), Error> {
    let (id, secret) = client.map_or((String::new(), String::new()), |c| (c.id.trim().to_string(), c.secret.trim().to_string()));
    rc.call("config/update", &json!({ "name": paths::remote(account), "parameters": { "client_id": id, "client_secret": secret }, "opt": { "nonInteractive": true, "obscure": true } }), rc::METADATA)?;
    Ok(())
}

pub fn set_scope(rc: &Rc, account: &str, access: Access) -> Result<(), Error> {
    rc.call("config/update", &json!({ "name": paths::remote(account), "parameters": { "scope": access.scope() }, "opt": { "nonInteractive": true } }), rc::METADATA)?;
    Ok(())
}

/// The client ID the remote uses, if it's the user's own (for "Using your app (…bf2a…)").
pub fn client_id(rc: &Rc, account: &str) -> Result<Option<String>, Error> {
    let v = rc.call("config/get", &json!({ "name": paths::remote(account) }), rc::METADATA)?;
    Ok(v.get("client_id").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string))
}

/// Shared drives the account can see: (Drive's ID, name).
pub fn shared_drives(rc: &Rc, account: &str) -> Result<Vec<(String, String)>, Error> {
    let v = rc.call("backend/command", &json!({ "command": "drives", "fs": paths::fs(account) }), rc::METADATA)?;
    Ok(parse_drives(&v))
}

pub fn parse_drives(v: &Value) -> Vec<(String, String)> {
    v.get("result").unwrap_or(v).as_array().map(|a| {
        a.iter().filter_map(|d| Some((d.get("id")?.as_str()?.to_string(), d.get("name").and_then(Value::as_str).unwrap_or("Shared drive").to_string()))).collect()
    }).unwrap_or_default()
}

/// What a new remote under an account shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    SharedWithMe,
    SharedDrive(String),
}

/// A remote for a shared place, reusing the parent account's sign-in (token and client are
/// copied over the socket; nothing on a command line, no second browser trip).
pub fn create_place(rc: &Rc, parent: &str, account: &str, place: &Place) -> Result<(), Error> {
    let v = rc.call("config/get", &json!({ "name": paths::remote(parent) }), rc::METADATA)?;
    let mut params = Map::new();
    for k in ["scope", "token", "client_id", "client_secret"] {
        if let Some(x) = v.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()) {
            params.insert(k.into(), json!(x));
        }
    }
    if !params.contains_key("token") {
        return Err(Error::new(Kind::SignedOut, "the account isn't signed in"));
    }
    params.insert("export_formats".into(), json!("link.html"));
    match place {
        Place::SharedWithMe => {
            params.insert("shared_with_me".into(), json!("true"));
        }
        Place::SharedDrive(id) => {
            params.insert("team_drive".into(), json!(id));
        }
    }
    rc.call("config/create", &json!({ "name": paths::remote(account), "type": "drive", "parameters": params, "opt": { "nonInteractive": true } }), rc::METADATA)?;
    Ok(())
}

/// Remove the remote (and with it the token) from our config.
pub fn delete_remote(rc: &Rc, account: &str) -> Result<(), Error> {
    rc.call("config/delete", &json!({ "name": paths::remote(account) }), rc::METADATA)?;
    Ok(())
}

/// Options for one mount, from Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MountOptions {
    pub cache_max_gb: u32,
    pub read_only: bool,
}

/// The `vfsOpt` for a mount: full cache (other apps can open, seek and save), uploads 5 s
/// after close, folders cached 5 min with Drive's change feed polled every minute.
pub fn vfs_options(o: MountOptions) -> Value {
    json!({
        "CacheMode": "full",
        "CacheMaxSize": format!("{}G", o.cache_max_gb.clamp(1, 500)),
        "CacheMaxAge": "24h",
        "WriteBack": "5s",
        "DirCacheTime": "5m",
        "PollInterval": "1m",
        "ReadOnly": o.read_only,
    })
}

/// Mount the account at its mount point, cleaning up a dead mount left by a crash first.
pub fn mount(rc: &Rc, account: &str, o: MountOptions) -> Result<(), Error> {
    let mp = paths::mount_point(account);
    clear_stale(&mp);
    paths::private_dir(&mp).map_err(|e| Error::new(Kind::Io, e.to_string()))?;
    if is_mount_point(&mp) {
        return Ok(());
    }
    rc.call("mount/mount", &json!({ "fs": paths::fs(account), "mountPoint": mp, "vfsOpt": vfs_options(o), "mountOpt": { "AllowOther": false, "VolumeName": "Google Drive" } }), rc::SLOW)?;
    Ok(())
}

/// Mount again with fresh credentials (after a new client ID or a new sign-in): rclone
/// caches the Drive connection, so drop it first.
pub fn remount(rc: &Rc, account: &str, o: MountOptions) -> Result<(), Error> {
    let mp = paths::mount_point(account);
    if is_mount_point(&mp) {
        unmount(rc, account)?;
    }
    let _ = rc.call("fscache/clear", &json!({}), rc::METADATA);
    mount(rc, account, o)
}

/// The remote's client settings as saved, to put back if signing in with new ones fails.
pub fn client_params(rc: &Rc, account: &str) -> Result<Option<Client>, Error> {
    let v = rc.call("config/get", &json!({ "name": paths::remote(account) }), rc::METADATA)?;
    let id = v.get("client_id").and_then(Value::as_str).unwrap_or("").trim().to_string();
    let secret = v.get("client_secret").and_then(Value::as_str).unwrap_or("").to_string();
    Ok((!id.is_empty()).then_some(Client { id, secret }))
}

pub fn unmount(rc: &Rc, account: &str) -> Result<(), Error> {
    rc.call("mount/unmount", &json!({ "mountPoint": paths::mount_point(account) }), rc::SLOW)?;
    Ok(())
}

/// Unmount without waiting for files other apps hold open ("Disconnect anyway").
pub fn unmount_lazy(account: &str) -> Result<(), Error> {
    let mp = paths::mount_point(account);
    let out = std::process::Command::new("fusermount3").arg("-uz").arg(&mp).output().map_err(|e| Error::new(Kind::Io, e.to_string()))?;
    if out.status.success() || !is_mount_point(&mp) {
        Ok(())
    } else {
        Err(Error::new(Kind::Other, String::from_utf8_lossy(&out.stderr).trim().to_string()))
    }
}

/// A mount whose rclone died answers "transport endpoint is not connected": unmount it lazily.
pub fn clear_stale(mp: &Path) {
    if let Err(e) = std::fs::metadata(mp)
        && e.raw_os_error() == Some(libc::ENOTCONN)
    {
        let _ = std::process::Command::new("fusermount3").arg("-uz").arg(mp).output();
    }
}

/// Something is mounted exactly here (its device differs from its parent's).
pub fn is_mount_point(p: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let (Ok(a), Some(Ok(b))) = (std::fs::metadata(p), p.parent().map(std::fs::metadata)) else { return false };
    a.dev() != b.dev()
}

/// Drive quota: (used, total). `total` is 0 for unlimited plans.
pub fn about(rc: &Rc, account: &str) -> Result<(u64, u64), Error> {
    let v = rc.call("operations/about", &json!({ "fs": paths::fs(account) }), rc::METADATA)?;
    let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    Ok((n("used"), n("total")))
}

/// Files copied in that aren't on Google yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Uploads {
    pub in_progress: u64,
    pub queued: u64,
    pub errored: u64,
    pub out_of_space: bool,
}

impl Uploads {
    pub fn pending(&self) -> u64 {
        self.in_progress + self.queued
    }
}

pub fn uploads(rc: &Rc, account: &str) -> Result<Uploads, Error> {
    let v = rc.call("vfs/stats", &json!({ "fs": paths::fs(account) }), Duration::from_secs(2))?;
    Ok(parse_uploads(&v))
}

pub fn parse_uploads(v: &Value) -> Uploads {
    let c = &v["diskCache"];
    let n = |k: &str| c.get(k).and_then(Value::as_u64).unwrap_or(0);
    Uploads { in_progress: n("uploadsInProgress"), queued: n("uploadsQueued"), errored: n("erroredFiles"), out_of_space: c.get("outOfSpace").and_then(Value::as_bool).unwrap_or(false) }
}

/// What the Drive API says about one entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    /// `/`-separated path inside Drive, as rclone shows it.
    pub path: String,
    pub name: String,
    pub dir: bool,
    pub size: i64,
    pub mime: String,
    /// Unix seconds.
    pub modified: i64,
}

/// `operations/list` of one folder (not recursive): IDs and mime types the mount can't give.
pub fn list(rc: &Rc, account: &str, dir: &str) -> Result<Vec<Entry>, Error> {
    let v = rc.call("operations/list", &json!({ "fs": paths::fs(account), "remote": dir, "opt": { "showOrigIDs": false } }), rc::METADATA)?;
    Ok(v.get("list").and_then(Value::as_array).map(|a| a.iter().filter_map(entry_of).collect()).unwrap_or_default())
}

/// One `lsjson` / `operations/list` item.
pub fn entry_of(v: &Value) -> Option<Entry> {
    let path = v.get("Path")?.as_str()?.to_string();
    let name = v.get("Name").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| path.rsplit('/').next().unwrap_or("").to_string());
    Some(Entry {
        id: v.get("ID").and_then(Value::as_str).unwrap_or("").to_string(),
        name,
        dir: v.get("IsDir").and_then(Value::as_bool).unwrap_or(false),
        size: v.get("Size").and_then(Value::as_i64).unwrap_or(-1),
        mime: v.get("MimeType").and_then(Value::as_str).unwrap_or("").to_string(),
        modified: v.get("ModTime").and_then(Value::as_str).and_then(parse_rfc3339).unwrap_or(0),
        path,
    })
}

/// Delete for good, skipping Google Drive's trash (Shift+Delete).
pub fn delete_permanently(rc: &Rc, account: &str, path: &str, dir: bool) -> Result<(), Error> {
    let fs = format!("{},use_trash=false:", paths::remote(account));
    if dir {
        rc.call("operations/purge", &json!({ "fs": fs, "remote": path }), rc::SLOW)?;
    } else {
        rc.call("operations/deletefile", &json!({ "fs": fs, "remote": path }), rc::SLOW)?;
    }
    Ok(())
}

/// Server-side copy inside one Drive (no download, no upload).
pub fn copy_within(rc: &Rc, account: &str, from: &str, to: &str) -> Result<(), Error> {
    rc.call("operations/copyfile", &json!({ "srcFs": paths::fs(account), "srcRemote": from, "dstFs": paths::fs(account), "dstRemote": to }), rc::SLOW)?;
    Ok(())
}

/// Export a Google Doc in `format` (`pdf`, `docx`, …) to a local folder.
pub fn export(rc: &Rc, account: &str, doc_path: &str, format: &str, to_dir: &Path, to_name: &str) -> Result<(), Error> {
    let src = format!("{},export_formats={format}:", paths::remote(account));
    // The doc as rclone shows it with this export format: the link.html name's stem + format.
    let stem = doc_path.strip_suffix(".link.html").unwrap_or(doc_path);
    rc.call("operations/copyfile", &json!({ "srcFs": src, "srcRemote": format!("{stem}.{format}"), "dstFs": to_dir, "dstRemote": to_name }), rc::SLOW)?;
    Ok(())
}

/// Put files back from Drive's trash under `dir` (rclone restores everything there).
pub fn untrash(rc: &Rc, account: &str, dir: &str) -> Result<(u64, u64), Error> {
    let v = rc.call("backend/command", &json!({ "command": "untrash", "fs": format!("{}{dir}", paths::fs(account)) }), rc::SLOW)?;
    let r = &v["result"];
    Ok((r.get("Untrashed").and_then(Value::as_u64).unwrap_or(0), r.get("Errors").and_then(Value::as_u64).unwrap_or(0)))
}

/// Google's own file types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GoogleKind {
    Doc,
    Sheet,
    Slides,
    Form,
    Drawing,
    Other,
}

impl GoogleKind {
    pub fn from_mime(mime: &str) -> Option<GoogleKind> {
        let t = mime.strip_prefix("application/vnd.google-apps.")?;
        Some(match t {
            "document" => GoogleKind::Doc,
            "spreadsheet" => GoogleKind::Sheet,
            "presentation" => GoogleKind::Slides,
            "form" => GoogleKind::Form,
            "drawing" => GoogleKind::Drawing,
            "folder" | "shortcut" => return None,
            _ => GoogleKind::Other,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            GoogleKind::Doc => "Google Doc",
            GoogleKind::Sheet => "Google Sheet",
            GoogleKind::Slides => "Google Slides",
            GoogleKind::Form => "Google Form",
            GoogleKind::Drawing => "Google Drawing",
            GoogleKind::Other => "Google file",
        }
    }

    /// Colour icon from the design system.
    pub fn icon(self) -> &'static str {
        match self {
            GoogleKind::Doc => "file-doc",
            GoogleKind::Sheet => "file-sheet",
            GoogleKind::Slides => "file-slides",
            GoogleKind::Drawing => "file-vector",
            GoogleKind::Form | GoogleKind::Other => "file-text",
        }
    }

    /// "Download as…" choices: (label, rclone export extension).
    pub fn exports(self) -> &'static [(&'static str, &'static str)] {
        match self {
            GoogleKind::Doc => &[("Word", "docx"), ("PDF", "pdf"), ("OpenDocument", "odt")],
            GoogleKind::Sheet => &[("Excel", "xlsx"), ("CSV", "csv"), ("PDF", "pdf")],
            GoogleKind::Slides => &[("PowerPoint", "pptx"), ("PDF", "pdf")],
            GoogleKind::Drawing => &[("SVG", "svg"), ("PDF", "pdf")],
            GoogleKind::Form | GoogleKind::Other => &[],
        }
    }
}

/// The suffix rclone gives Google Docs exported as links.
pub const LINK_SUFFIX: &str = ".link.html";

/// Google types in one folder listing: name as the mount shows it → kind.
pub fn google_kinds(entries: &[Entry]) -> HashMap<String, GoogleKind> {
    entries.iter().filter(|e| !e.dir).filter_map(|e| GoogleKind::from_mime(&e.mime).map(|k| (e.name.clone(), k))).collect()
}

/// `2026-09-14T18:22:33.123Z` / `…+02:00` → Unix seconds (no date library for one format).
pub fn parse_rfc3339(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (num(0..4)?, num(5..7)?, num(8..10)?, num(11..13)?, num(14..16)?, num(17..19)?);
    let mut rest = &s[19..];
    if let Some(r) = rest.strip_prefix('.') {
        rest = r.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let offset = match rest.as_bytes().first() {
        Some(b'Z') | Some(b'z') | None => 0,
        Some(&sign @ (b'+' | b'-')) => {
            let oh: i64 = rest.get(1..3)?.parse().ok()?;
            let om: i64 = rest.get(4..6)?.parse().ok()?;
            let o = oh * 3600 + om * 60;
            if sign == b'+' { o } else { -o }
        }
        _ => return None,
    };
    // Days from civil (Howard Hinnant).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + se - offset)
}

/// Unix seconds → `2026-09-14T18:22:33Z` (for Drive queries).
pub fn rfc3339(t: i64) -> String {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", secs / 3600, secs % 3600 / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2026-09-14T18:22:33.123Z"), Some(1_789_410_153));
        assert_eq!(parse_rfc3339("2026-09-14T20:22:33+02:00"), Some(1_789_410_153));
        assert_eq!(rfc3339(1_789_410_153), "2026-09-14T18:22:33Z");
        assert_eq!(parse_rfc3339("2000-02-29T12:00:00Z").map(rfc3339).as_deref(), Some("2000-02-29T12:00:00Z"));
        assert_eq!(parse_rfc3339("nope"), None);
    }

    #[test]
    fn entries_kinds_and_uploads() {
        let v = json!({"Path": "Work/Budget 2026.link.html", "Name": "Budget 2026.link.html", "Size": -1, "MimeType": "application/vnd.google-apps.spreadsheet", "ModTime": "2026-09-14T18:22:33Z", "IsDir": false, "ID": "1abc"});
        let e = entry_of(&v).unwrap();
        assert_eq!((e.id.as_str(), e.size, e.modified), ("1abc", -1, 1_789_410_153));
        let kinds = google_kinds(&[e, entry_of(&json!({"Path": "Work/x.pdf", "MimeType": "application/pdf"})).unwrap()]);
        assert_eq!(kinds.get("Budget 2026.link.html"), Some(&GoogleKind::Sheet));
        assert_eq!(kinds.len(), 1);
        assert_eq!(GoogleKind::from_mime("application/vnd.google-apps.folder"), None);
        assert_eq!(GoogleKind::Doc.exports()[0], ("Word", "docx"));
        let u = parse_uploads(&json!({"diskCache": {"uploadsInProgress": 1, "uploadsQueued": 2, "erroredFiles": 0, "outOfSpace": false}}));
        assert_eq!(u.pending(), 3);
        assert!(valid_client_id("1234-abcd.apps.googleusercontent.com"));
        assert!(!valid_client_id(".apps.googleusercontent.com") && !valid_client_id("abc"));
        assert_eq!(vfs_options(MountOptions { cache_max_gb: 900, read_only: true })["CacheMaxSize"], "500G");
        assert_eq!(parse_drives(&json!({"result": [{"id": "0AB", "name": "Team", "kind": "drive#drive"}, {"name": "no id"}]})), vec![("0AB".to_string(), "Team".to_string())]);
    }
}

//! A full crawl: `rclone lsjson -R --fast-list` as a child, read line by line (constant
//! memory however big the Drive is; rc would hand back one giant reply). Also used for the
//! folders-only crawl behind the in-memory folder map when the index is off.

use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::Value;

use crate::catalog::{Catalog, Entry};
use crate::drive;
use crate::error::{Error, Kind};
use crate::paths;
use crate::rclone::Rclone;

#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Folders only (the folder map), no files.
    pub dirs_only: bool,
    /// Requests per second, leaving room for browsing on rclone's shared app.
    pub tps: u32,
    /// Folders with these names are listed but their contents aren't (Settings → Search).
    pub exclude_names: Vec<String>,
}

/// Progress and cancel for a crawl in flight.
#[derive(Clone, Default)]
pub struct Progress {
    pub items: Arc<AtomicU64>,
    pub cancel: Arc<AtomicBool>,
}

/// One `lsjson` output line (array punctuation around it is fine).
pub fn parse_line(line: &str) -> Option<Entry> {
    let t = line.trim().trim_start_matches('[').trim_end_matches(']').trim().trim_end_matches(',');
    if t.is_empty() {
        return None;
    }
    let v: Value = serde_json::from_str(t).ok()?;
    let d = drive::entry_of(&v)?;
    let hashes = v.get("Hashes");
    let md5 = hashes.and_then(|h| h.get("md5").or_else(|| h.get("MD5"))).and_then(Value::as_str).unwrap_or("").to_string();
    // Shortcut IDs can come as "target<TAB>shortcut"; keep the shortcut's own.
    let mut e = Entry::from_drive(&d, md5);
    if let Some((_, own)) = e.id.split_once('\t') {
        e.id = own.to_string();
    }
    Some(e)
}

fn excluded(path: &str, names: &[String]) -> bool {
    // The excluded folder itself is listed; what's inside isn't.
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    parts.iter().any(|p| names.iter().any(|n| n == p))
}

pub fn crawl(rclone: &Rclone, account: &str, o: &Options, progress: &Progress) -> Result<Catalog, Error> {
    let mut cmd = rclone.command();
    cmd.arg("lsjson").arg("-R").arg("--fast-list").arg("--drive-skip-shortcuts").arg("--tpslimit").arg(o.tps.max(1).to_string());
    if o.dirs_only {
        cmd.arg("--dirs-only");
    } else {
        cmd.arg("--hash").arg("--hash-type").arg("md5");
    }
    cmd.arg(paths::fs(account)).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| Error::new(Kind::NotRunning, format!("couldn't start rclone: {e}")))?;
    let stderr = child.stderr.take();
    let err_text = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = stderr {
            for l in BufReader::new(e).lines().map_while(Result::ok) {
                if s.len() < 8192 {
                    s.push_str(&l);
                    s.push('\n');
                }
            }
        }
        s
    });
    let mut entries = Vec::new();
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if progress.cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::new(Kind::Other, "Cancelled."));
            }
            if let Some(e) = parse_line(&line)
                && !excluded(&e.path, &o.exclude_names)
            {
                entries.push(e);
                progress.items.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    let status = child.wait().map_err(|e| Error::new(Kind::Io, e.to_string()))?;
    let err = err_text.join().unwrap_or_default();
    if !status.success() {
        let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("rclone stopped without saying why");
        return Err(Error::from_rclone(last));
    }
    let mut c = Catalog::from_entries(entries);
    c.link_parents();
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lsjson_lines() {
        assert!(parse_line("[").is_none());
        assert!(parse_line("]").is_none());
        let e = parse_line(r#"{"Path":"Work/a.pdf","Name":"a.pdf","Size":42,"MimeType":"application/pdf","ModTime":"2026-09-14T18:22:33Z","IsDir":false,"ID":"1x","Hashes":{"md5":"abc"}},"#).unwrap();
        assert_eq!((e.path.as_str(), e.size, e.md5.as_str(), e.id.as_str()), ("Work/a.pdf", 42, "abc", "1x"));
        let s = parse_line(r#"[{"Path":"Link","Name":"Link","IsDir":true,"ID":"target\tshort"}"#).unwrap();
        assert_eq!(s.id, "short");
        assert!(excluded("node_modules/x.js", &["node_modules".into()]));
        assert!(!excluded("node_modules", &["node_modules".into()]));
    }
}

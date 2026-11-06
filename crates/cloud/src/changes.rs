//! Google Drive's change feed (design §12.4), only for people using their own client ID:
//! with rclone's shared app, calling Google ourselves would act as rclone's app, which we
//! never do. Every 60 s, "what changed since this page token?" keeps the Drive index almost
//! live instead of waiting for the daily crawl.
//!
//! The access token comes from our rclone config. We never refresh it ourselves (two
//! writers of one config lose tokens): when it's about to expire, a cheap rc call makes
//! rclone refresh and save it, and we read it again.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;

use crate::drive::parse_rfc3339;
use crate::error::{Error, Kind};
use crate::paths;
use crate::rc::Rc;

const API: &str = "https://www.googleapis.com/drive/v3";

/// One line of `[section]` key = value from an rclone config.
pub fn section(conf: &str, name: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut inside = false;
    for line in conf.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            inside = &t[1..t.len() - 1] == name;
            continue;
        }
        if inside && let Some((k, v)) = t.split_once('=') {
            out.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

#[derive(Clone, Debug)]
pub struct Token {
    pub access: String,
    /// Unix seconds.
    pub expiry: i64,
}

/// The access token, if this account uses the person's own client ID.
pub fn read_token(account: &str) -> Option<Token> {
    let conf = std::fs::read_to_string(paths::config_file()).ok()?;
    token_from(&section(&conf, &paths::remote(account)))
}

pub fn token_from(s: &HashMap<String, String>) -> Option<Token> {
    if s.get("client_id").is_none_or(|c| c.is_empty()) {
        return None;
    }
    let t: Value = serde_json::from_str(s.get("token")?).ok()?;
    Some(Token { access: t.get("access_token")?.as_str()?.to_string(), expiry: t.get("expiry").and_then(Value::as_str).and_then(parse_rfc3339).unwrap_or(0) })
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// A token good for at least a minute: let rclone refresh it if needed.
pub fn fresh_token(rc: &Rc, account: &str) -> Result<Option<Token>, Error> {
    let Some(t) = read_token(account) else { return Ok(None) };
    if t.expiry > now() + 60 {
        return Ok(Some(t));
    }
    crate::drive::about(rc, account)?;
    Ok(read_token(account))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).http_status_as_error(false).build().into()
}

fn get(token: &Token, url: &str) -> Result<Value, Error> {
    let mut r = agent().get(url).header("Authorization", &format!("Bearer {}", token.access)).call().map_err(|e| Error::new(Kind::Offline, e.to_string()))?;
    let status = r.status().as_u16();
    let text = r.body_mut().read_to_string().map_err(|e| Error::new(Kind::Io, e.to_string()))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| Error::new(Kind::Io, e.to_string()))?;
    if status >= 400 {
        let msg = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("Google said no");
        let reason = v.pointer("/error/errors/0/reason").and_then(Value::as_str).unwrap_or("");
        return Err(Error::from_rclone(&format!("Error {status}: {msg}, {reason}")));
    }
    Ok(v)
}

/// Where the feed starts now.
pub fn start(token: &Token) -> Result<String, Error> {
    let v = get(token, &format!("{API}/changes/startPageToken"))?;
    v.get("startPageToken").and_then(Value::as_str).map(str::to_string).ok_or_else(|| Error::new(Kind::Other, "Google sent no start token"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub id: String,
    /// Deleted or trashed: leaves the index.
    pub gone: bool,
    pub name: String,
    pub parents: Vec<String>,
    pub mime: String,
    pub size: i64,
    pub modified: i64,
}

pub fn parse_changes(v: &Value) -> Vec<Change> {
    v.get("changes").and_then(Value::as_array).map(|a| {
        a.iter()
            .filter_map(|c| {
                let id = c.get("fileId").and_then(Value::as_str)?.to_string();
                let f = c.get("file");
                let s = |k: &str| f.and_then(|f| f.get(k)).and_then(Value::as_str).unwrap_or("").to_string();
                let trashed = f.and_then(|f| f.get("trashed")).and_then(Value::as_bool).unwrap_or(false);
                Some(Change {
                    gone: c.get("removed").and_then(Value::as_bool).unwrap_or(false) || trashed || f.is_none(),
                    name: s("name"),
                    parents: f.and_then(|f| f.get("parents")).and_then(Value::as_array).map(|p| p.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default(),
                    mime: s("mimeType"),
                    size: f.and_then(|f| f.get("size")).and_then(|v| v.as_str().and_then(|t| t.parse().ok())).unwrap_or(-1),
                    modified: f.and_then(|f| f.get("modifiedTime")).and_then(Value::as_str).and_then(parse_rfc3339).unwrap_or(0),
                    id,
                })
            })
            .collect()
    }).unwrap_or_default()
}

/// Everything since `page`, and the token to ask with next time.
pub fn since(token: &Token, page: &str) -> Result<(Vec<Change>, String), Error> {
    let mut out = Vec::new();
    let mut page = page.to_string();
    for _ in 0..50 {
        let url = format!("{API}/changes?pageToken={page}&pageSize=1000&spaces=drive&fields=nextPageToken,newStartPageToken,changes(fileId,removed,file(name,parents,mimeType,size,modifiedTime,trashed))");
        let v = get(token, &url)?;
        out.extend(parse_changes(&v));
        if let Some(next) = v.get("newStartPageToken").and_then(Value::as_str) {
            return Ok((out, next.to_string()));
        }
        match v.get("nextPageToken").and_then(Value::as_str) {
            Some(n) => page = n.to_string(),
            None => break,
        }
    }
    Ok((out, page))
}

/// The saved page token (next to the catalog; only while the index is on).
pub fn page_file(account: &str) -> std::path::PathBuf {
    ef_config::cloud_index_dir().join(format!("{account}.changes"))
}

/// One poll for an account: `Ok(None)` when it uses rclone's shared app.
pub fn poll(rc: &Rc, account: &str) -> Result<Option<Vec<Change>>, Error> {
    let Some(token) = fresh_token(rc, account)? else { return Ok(None) };
    let file = page_file(account);
    let page = std::fs::read_to_string(&file).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let (changes, next) = match page {
        Some(p) => since(&token, &p)?,
        // First time: start from now (the crawl already has the past).
        None => (Vec::new(), start(&token)?),
    };
    if file.parent().is_some_and(|d| d.exists()) {
        let _ = std::fs::write(&file, next);
    }
    Ok(Some(changes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_config_and_changes() {
        let conf = "[ef-a]\ntype = drive\nclient_id = 1-x.apps.googleusercontent.com\nclient_secret = s\ntoken = {\"access_token\":\"ya29\",\"expiry\":\"2026-09-14T18:22:33Z\"}\n\n[ef-b]\ntype = drive\ntoken = {\"access_token\":\"zz\"}\n";
        let t = token_from(&section(conf, "ef-a")).unwrap();
        assert_eq!((t.access.as_str(), t.expiry), ("ya29", 1_789_410_153));
        assert!(token_from(&section(conf, "ef-b")).is_none(), "rclone's shared app: no feed");
        let v = json!({"changes": [
            {"fileId": "1", "removed": false, "file": {"name": "a.pdf", "parents": ["w"], "mimeType": "application/pdf", "size": "9", "modifiedTime": "2026-09-14T18:22:33Z", "trashed": false}},
            {"fileId": "2", "removed": true},
            {"fileId": "3", "file": {"name": "old", "trashed": true}}
        ], "newStartPageToken": "77"});
        let c = parse_changes(&v);
        assert_eq!(c.len(), 3);
        assert!(!c[0].gone && c[1].gone && c[2].gone);
        assert_eq!((c[0].size, c[0].parents[0].as_str()), (9, "w"));
    }
}

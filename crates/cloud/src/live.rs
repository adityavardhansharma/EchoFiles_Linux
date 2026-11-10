//! Step 2 of search (design §10): our query in Drive's query language, sent through rclone's
//! Drive `query` command. Drive's `name contains` matches word *prefixes*, so this only ever
//! adds candidates; our own matcher decides what is shown.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::drive::{parse_rfc3339, rfc3339, LINK_SUFFIX};
use crate::error::Error;
use crate::paths;
use crate::rc::{self, Rc};
use crate::uri::segment;

pub const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
/// At most this many matches from Drive are kept per search.
pub const CAP: usize = 200;

/// What our search asked for, in the parts Drive can narrow by.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ask {
    /// Plain words (already split).
    pub words: Vec<String>,
    pub dirs_only: bool,
    pub files_only: bool,
    /// Modified after this (Unix seconds).
    pub modified_after: Option<i64>,
    /// Also match words inside files (Drive's full-text search).
    pub fulltext: bool,
}

/// Why step 2 doesn't run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// No word of 2+ characters.
    TooShort,
    /// A regex or glob without usable literal words.
    Pattern,
}

/// Escape a word for a Drive query string: backslash, then single quote.
pub fn escape(word: &str) -> String {
    word.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Our query → Drive's. Only filters that can't drop a real match are sent.
pub fn translate(ask: &Ask) -> Result<String, Skip> {
    let words: Vec<&str> = ask.words.iter().map(String::as_str).filter(|w| w.chars().count() >= 2).collect();
    if words.is_empty() {
        return Err(Skip::TooShort);
    }
    let mut parts: Vec<String> = words
        .iter()
        .map(|w| if ask.fulltext { format!("(name contains '{0}' or fullText contains '{0}')", escape(w)) } else { format!("name contains '{}'", escape(w)) })
        .collect();
    if ask.dirs_only {
        parts.push(format!("mimeType = '{FOLDER_MIME}'"));
    } else if ask.files_only {
        parts.push(format!("mimeType != '{FOLDER_MIME}'"));
    }
    if let Some(t) = ask.modified_after {
        parts.push(format!("modifiedTime > '{}'", rfc3339(t)));
    }
    parts.push("trashed = false".into());
    Ok(parts.join(" and "))
}

/// The literal words inside a regex or glob that are safe to ask Drive for.
pub fn literal_words(pattern: &str) -> Vec<String> {
    pattern
        .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
        .filter(|w| w.chars().count() >= 2)
        // `\d`, `\w`… leave single letters after splitting; skip regex class letters glued to words.
        .map(str::to_string)
        .collect()
}

/// One match as Drive describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub id: String,
    pub name: String,
    pub parents: Vec<String>,
    pub mime: String,
    pub size: i64,
    pub modified: i64,
    pub web: String,
}

impl Found {
    pub fn is_dir(&self) -> bool {
        self.mime == FOLDER_MIME
    }

    /// The name as the mount shows it: `/` becomes `／`, Google Docs get `.link.html`.
    pub fn mount_name(&self) -> String {
        let mut n = segment(&self.name);
        if self.mime.starts_with("application/vnd.google-apps.") && !self.is_dir() && self.mime != "application/vnd.google-apps.shortcut" {
            n.push_str(LINK_SUFFIX);
        }
        n
    }

    /// Its path inside Drive, if a parent folder's path is known.
    pub fn path_in(&self, folders: &HashMap<String, String>) -> Option<String> {
        self.parents.iter().find_map(|p| folders.get(p)).map(|dir| if dir.is_empty() { self.mount_name() } else { format!("{dir}/{}", self.mount_name()) })
    }
}

pub fn parse(v: &Value) -> Vec<Found> {
    let list = v.get("result").unwrap_or(v);
    list.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    let s = |k: &str| x.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                    let id = s("id");
                    let name = s("name");
                    if id.is_empty() || name.is_empty() {
                        return None;
                    }
                    Some(Found {
                        parents: x.get("parents").and_then(Value::as_array).map(|p| p.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default(),
                        mime: s("mimeType"),
                        size: x.get("size").and_then(|v| v.as_str().and_then(|t| t.parse().ok()).or_else(|| v.as_i64())).unwrap_or(-1),
                        modified: x.get("modifiedTime").and_then(Value::as_str).and_then(parse_rfc3339).unwrap_or(0),
                        web: s("webViewLink"),
                        id,
                        name,
                    })
                })
                .take(CAP)
                .collect()
        })
        .unwrap_or_default()
}

/// Run a Drive query for one account.
pub fn query(rc: &Rc, account: &str, q: &str) -> Result<Vec<Found>, Error> {
    let v = rc.call("backend/command", &json!({ "command": "query", "fs": paths::fs(account), "arg": [q] }), rc::SEARCH)?;
    Ok(parse(&v))
}

/// The ID of "My Drive" itself (top-level items name it as their parent).
pub fn root_id(rc: &Rc, account: &str) -> Result<Option<String>, Error> {
    let found = query(rc, account, "'root' in parents and trashed = false")?;
    Ok(found.into_iter().find_map(|f| f.parents.into_iter().next()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_safely() {
        let a = Ask { words: vec!["annual".into(), "report".into()], ..Default::default() };
        assert_eq!(translate(&a).unwrap(), "name contains 'annual' and name contains 'report' and trashed = false");
        let a = Ask { words: vec!["O'Brien\\x".into()], dirs_only: true, modified_after: Some(1_789_410_153), ..Default::default() };
        assert_eq!(translate(&a).unwrap(), "name contains 'O\\'Brien\\\\x' and mimeType = 'application/vnd.google-apps.folder' and modifiedTime > '2026-09-14T18:22:33Z' and trashed = false");
        assert_eq!(translate(&Ask { words: vec!["a".into()], ..Default::default() }), Err(Skip::TooShort));
        let f = Ask { words: vec!["über".into(), "日本".into()], files_only: true, ..Default::default() };
        assert!(translate(&f).unwrap().contains("name contains '日本'") && translate(&f).unwrap().contains("mimeType != "));
        assert_eq!(literal_words("^report-\\d+\\.pdf$"), vec!["report-", "pdf"]);
        let ft = Ask { words: vec!["budget".into()], fulltext: true, ..Default::default() };
        assert_eq!(translate(&ft).unwrap(), "(name contains 'budget' or fullText contains 'budget') and trashed = false");
        assert!(literal_words("^\\d+$").is_empty() || literal_words("^\\d+$").iter().all(|w| w.len() >= 2));
    }

    #[test]
    fn parses_and_places() {
        let v = json!({"result": [
            {"id": "1", "name": "Budget 2026", "mimeType": "application/vnd.google-apps.spreadsheet", "parents": ["w"], "modifiedTime": "2026-09-14T18:22:33.000Z", "webViewLink": "https://docs.google.com/x"},
            {"id": "2", "name": "A/B test.pdf", "mimeType": "application/pdf", "parents": ["root0"], "size": "42"},
            {"id": "3", "name": "Lost", "mimeType": "application/pdf", "parents": ["unknown"]},
            {"name": "no id"}
        ]});
        let f = parse(&v);
        assert_eq!(f.len(), 3);
        let folders: HashMap<String, String> = [("w".to_string(), "Work".to_string()), ("root0".to_string(), String::new())].into();
        assert_eq!(f[0].path_in(&folders).unwrap(), "Work/Budget 2026.link.html");
        assert_eq!(f[1].path_in(&folders).unwrap(), "A／B test.pdf");
        assert_eq!(f[1].size, 42);
        assert_eq!(f[0].modified, 1_789_410_153);
        assert_eq!(f[2].path_in(&folders), None);
    }
}

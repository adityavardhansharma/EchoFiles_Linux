//! The search-box language, shared by the app's search field and `ef find`: words to look
//! for, plus `key:value` filters that map onto the same options as the command line.
//!
//! ```text
//! report ext:pdf size:>1m modified:<7d          a PDF named "report", over 1 MB, changed this week
//! invoice in:~/Documents type:file               only files, only in ~/Documents
//! "annual report" after:2026-01-01 hidden:yes    a phrase, changed since New Year, dotfiles too
//! re:^IMG_\d+\.jpe?g$                            a regular expression (also glob:*.rs)
//! ```
//!
//! Keys: `ext`, `type`, `size`, `modified`/`changed`, `after`, `before`, `in`, `depth`,
//! `owner`, `hidden`, `ignored`, `case`, `re`/`regex`, `glob`. A `key:value` with an unknown
//! key is an ordinary word (so names like `12:30` still search).

use super::args::FindArgs;
use crate::pattern::Syntax;
use crate::units::{self, SizeFilter};

/// Split on whitespace, keeping `"quoted phrases"` (and `key:"quoted values"`) together.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                cur.push(c);
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn unquote(v: &str) -> &str {
    v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(v)
}

/// `<7d`, `>2026-01-01`, `today`, `week`… → (changed at or after, changed before).
fn when(key: &str, v: &str) -> Result<(Option<String>, Option<String>), String> {
    let today = || units::format_local(std::time::SystemTime::now())[..10].to_string();
    let bad = || format!("{key}: can't read '{v}' (try <7d, >2026-01-01, today, week, month)");
    Ok(match v {
        "today" => (Some(today()), None),
        "yesterday" => {
            let t = units::parse_time("1d", std::time::SystemTime::now()).ok_or_else(bad)?;
            (Some(units::format_local(t)[..10].to_string()), Some(today()))
        }
        "hour" => (Some("1h".into()), None),
        "week" => (Some("7d".into()), None),
        "month" => (Some("30d".into()), None),
        "year" => (Some("365d".into()), None),
        _ => {
            let (op, rest) = match v.as_bytes().first() {
                Some(b'<') => ('<', &v[1..]),
                Some(b'>') => ('>', &v[1..]),
                _ => ('=', v),
            };
            let rest = rest.strip_prefix('=').unwrap_or(rest);
            let is_duration = units::parse_duration(rest).is_some();
            if units::parse_time(rest, std::time::SystemTime::now()).is_none() {
                return Err(bad());
            }
            // A duration counts back from now ("<7d": less than 7 days ago); a date is a
            // point in time (">2026-01-01": after it).
            match (op, is_duration) {
                ('<', true) | ('>', false) | ('=', true) => (Some(rest.to_string()), None),
                ('>', true) | ('<', false) => (None, Some(rest.to_string())),
                _ => {
                    // `modified:2026-01-05`: that whole day.
                    let start = units::parse_time(rest, std::time::SystemTime::now()).ok_or_else(bad)?;
                    let end = start + std::time::Duration::from_secs(86_400);
                    (Some(rest.to_string()), Some(format!("@{}", units::to_unix(end))))
                }
            }
        }
    })
}

fn size(v: &str) -> Result<Vec<SizeFilter>, String> {
    let bad = || format!("size: can't read '{v}' (try >1m, <500k, 10k..2m)");
    if let Some((lo, hi)) = v.split_once("..") {
        let lo = SizeFilter::parse(&format!("+{lo}")).ok_or_else(bad)?;
        let hi = SizeFilter::parse(&format!("-{hi}")).ok_or_else(bad)?;
        return Ok(vec![lo, hi]);
    }
    let f = match v.as_bytes().first() {
        Some(b'>') => SizeFilter::parse(&format!("+{}", v[1..].trim_start_matches('='))),
        Some(b'<') => SizeFilter::parse(&format!("-{}", v[1..].trim_start_matches('='))),
        _ => SizeFilter::parse(v),
    };
    Ok(vec![f.ok_or_else(bad)?])
}

fn yes(key: &str, v: &str) -> Result<bool, String> {
    match v {
        "yes" | "y" | "on" | "true" | "1" => Ok(true),
        "no" | "n" | "off" | "false" | "0" => Ok(false),
        _ => Err(format!("{key}: use yes or no, not '{v}'")),
    }
}

/// Read `text`'s filters into `a`; the remaining words become `a.pattern` (`None` if only
/// filters were given: everything they allow matches).
pub fn apply(text: &str, a: &mut FindArgs) -> Result<(), String> {
    let mut words: Vec<String> = Vec::new();
    for tok in tokens(text) {
        let Some((key, raw)) = tok.split_once(':') else {
            words.push(tok);
            continue;
        };
        let v = unquote(raw);
        let key_l = key.to_ascii_lowercase();
        match key_l.as_str() {
            "ext" | "extension" => a.exts.extend(v.split(',').map(|e| e.trim_start_matches('.').to_string()).filter(|e| !e.is_empty())),
            "type" | "kind" => {
                for t in v.split(',') {
                    match t {
                        "folder" | "folders" | "dir" | "dirs" => a.types.add("d")?,
                        "file" | "files" => a.types.add("f")?,
                        "link" | "symlink" => a.types.add("l")?,
                        "exe" | "executable" | "program" => a.types.add("x")?,
                        "empty" => a.types.add("e")?,
                        other => a.types.add(other).map_err(|_| format!("type: use file, folder, link, executable or empty, not '{other}'"))?,
                    }
                }
                a.types.finish();
            }
            "size" => a.sizes.extend(size(v)?),
            "modified" | "changed" | "mod" | "date" => {
                let (n, o) = when(&key_l, v)?;
                a.newer = n.or(a.newer.take());
                a.older = o.or(a.older.take());
            }
            "after" | "since" => {
                units::parse_time(v, std::time::SystemTime::now()).ok_or_else(|| format!("after: can't read '{v}' (try 2026-01-05 or 7d)"))?;
                a.newer = Some(v.to_string());
            }
            "before" | "until" => {
                units::parse_time(v, std::time::SystemTime::now()).ok_or_else(|| format!("before: can't read '{v}' (try 2026-01-05 or 7d)"))?;
                a.older = Some(v.to_string());
            }
            "in" | "path" | "folder" => a.paths.push(v.to_string()),
            "depth" => {
                let (op, n) = match v.as_bytes().first() {
                    Some(b'<') => ('<', &v[1..]),
                    Some(b'>') => ('>', &v[1..]),
                    _ => ('=', v),
                };
                let n: u32 = n.trim_start_matches('=').parse().map_err(|_| format!("depth: use a number like 2, <3 or >1, not '{v}'"))?;
                match op {
                    '<' => a.max_depth = Some(if v.contains('=') { n } else { n.saturating_sub(1) }),
                    '>' => a.min_depth = Some(if v.contains('=') { n } else { n + 1 }),
                    _ => {
                        a.min_depth = Some(n);
                        a.max_depth = Some(n);
                    }
                }
            }
            "owner" | "user" => a.owner = Some(if v == "me" { std::env::var("USER").unwrap_or_default() } else { v.to_string() }),
            "hidden" => a.hidden = yes(&key_l, v)?,
            "ignored" | "gitignored" => a.no_ignore = yes(&key_l, v)?,
            "case" => a.case = if yes(&key_l, v)? { crate::pattern::Case::Sensitive } else { crate::pattern::Case::Insensitive },
            "re" | "regex" => {
                a.syntax = Some(Syntax::Regex);
                words.push(v.to_string());
            }
            "glob" => {
                a.syntax = Some(Syntax::Glob);
                words.push(v.to_string());
            }
            // Not a filter: an ordinary word that happens to contain a colon.
            _ => words.push(tok),
        }
    }
    a.pattern = (!words.is_empty()).then(|| words.join(" "));
    Ok(())
}

/// Whether `text` uses any filter key (so `ef find` knows to read it as a query).
pub fn has_filters(text: &str) -> bool {
    const KEYS: &[&str] = &[
        "ext", "extension", "type", "kind", "size", "modified", "changed", "mod", "date", "after", "since", "before", "until", "in", "path", "folder",
        "depth", "owner", "user", "hidden", "ignored", "gitignored", "case", "re", "regex", "glob",
    ];
    tokens(text).iter().any(|t| t.split_once(':').is_some_and(|(k, v)| !v.is_empty() && KEYS.contains(&k.to_ascii_lowercase().as_str())))
}

#[cfg(test)]
mod tests {
    use super::super::args::{Flavor, parse};
    use super::*;

    fn q(text: &str) -> FindArgs {
        let mut a = parse(Flavor::Ef, &[]).unwrap();
        apply(text, &mut a).unwrap();
        a
    }

    #[test]
    fn filters_and_words() {
        let a = q("annual report ext:pdf,docx size:>1m modified:<7d");
        assert_eq!(a.pattern.as_deref(), Some("annual report"));
        assert_eq!(a.exts, vec!["pdf", "docx"]);
        assert_eq!(a.sizes, vec![SizeFilter::AtLeast(1_000_000)]);
        assert_eq!(a.newer.as_deref(), Some("7d"));
        let a = q("\"q3 report\" in:\"~/My Docs\" type:file hidden:yes");
        assert_eq!(a.pattern.as_deref(), Some("\"q3 report\""));
        assert_eq!(a.paths, vec!["~/My Docs"]);
        assert!(a.types.file && a.hidden);
        let a = q("ext:pdf");
        assert_eq!(a.pattern, None);
        let a = q("meeting 12:30 notes");
        assert_eq!(a.pattern.as_deref(), Some("meeting 12:30 notes"));
        let a = q("size:10k..2m modified:>2026-01-01 depth:<3");
        assert_eq!(a.sizes, vec![SizeFilter::AtLeast(10_000), SizeFilter::AtMost(2_000_000)]);
        assert_eq!(a.newer.as_deref(), Some("2026-01-01"));
        assert_eq!(a.max_depth, Some(2));
        let a = q("re:^img_\\d+");
        assert_eq!(a.syntax, Some(Syntax::Regex));
        assert_eq!(a.pattern.as_deref(), Some("^img_\\d+"));
        let a = q("modified:>2w");
        assert_eq!((a.newer.as_deref(), a.older.as_deref()), (None, Some("2w")));
    }

    #[test]
    fn bad_values_say_why() {
        let mut a = parse(Flavor::Ef, &[]).unwrap();
        assert!(apply("size:>huge", &mut a).unwrap_err().contains("size"));
        assert!(apply("modified:<soon", &mut a).unwrap_err().contains("modified"));
        assert!(apply("type:rock", &mut a).unwrap_err().contains("type"));
        assert!(has_filters("report ext:pdf"));
        assert!(!has_filters("meeting 12:30"));
    }
}

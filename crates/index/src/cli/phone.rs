//! `ef phone photos …`: copy recent photos and videos from a paired phone, by date or by
//! count, and print where they landed — so an agent can look at "today's photos".
//!
//! Which photos exist comes from a saved list of the phone's pictures (newest first),
//! kept current with the phone's own photo library: when the phone is reachable only the
//! pages newer than the saved list are asked for, so "the last 5" is one short round
//! trip. Photos already copied are reused from the cache without asking the phone again.
//! When the phone is away, the saved list (and the full file index, if that's on)
//! answers, and photos already on the laptop are still returned.
//!
//! The phone connection lives in the app, so fetching needs EchoFiles running; `ef`
//! reaches it over the socket it already uses ([`crate::phone::link`]).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use super::Ctx;
use crate::phone::{Link, LinkPhone};
use crate::units;

const OFF: &str = "ef can't reach phone photos: turn on EchoFiles → Settings → AI agents → Let ef fetch phone photos.";
/// Most files one run copies unless `--max` says otherwise.
const DEFAULT_MAX: usize = 50;
/// Most bytes one run copies unless `--max-size` says otherwise.
const DEFAULT_MAX_BYTES: u64 = 1 << 30;
/// Newest photos when neither a count nor a date is given.
const DEFAULT_COUNT: usize = 10;
/// Pages of 100 the phone is asked for in one run, at most (30,000 photos).
const MAX_PAGES: usize = 300;
/// Copies running at once (the phone link allows 8 transfers).
const PARALLEL: usize = 4;
/// Saved list entries kept per phone.
const MAX_SAVED: usize = 100_000;

const PHOTO_EXT: [&str; 11] = ["jpg", "jpeg", "png", "heic", "heif", "webp", "gif", "dng", "avif", "bmp", "raw"];
const VIDEO_EXT: [&str; 7] = ["mp4", "mov", "mkv", "webm", "3gp", "m4v", "avi"];

fn fail(ctx: &Ctx, code: u8, msg: impl std::fmt::Display) -> u8 {
    errln!(ctx, "ef: {msg}");
    code
}

pub fn main(args: &[String], ctx: &Ctx) -> u8 {
    let sub = args.first().map(String::as_str).unwrap_or("status");
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };
    match sub {
        "status" | "list-phones" => status(ctx),
        "photos" | "photo" | "pics" | "pictures" => photos(rest, ctx, Kind::Photo),
        "videos" | "video" => photos(rest, ctx, Kind::Video),
        "media" => photos(rest, ctx, Kind::All),
        "cache" => cache(rest, ctx),
        "-h" | "--help" | "help" => {
            outln!(ctx, "{}", HELP);
            0
        }
        _ => fail(ctx, 2, format!("unknown `ef phone {sub}`; try `ef phone photos today` or `ef phone help`")),
    }
}

const HELP: &str = "ef phone — photos and videos from your paired phone (needs EchoFiles running to fetch)

  ef phone photos [WHEN…] [options]   copy matching photos, print their paths (newest first)
  ef phone videos [WHEN…]             the same for videos;  ef phone media  both
  ef phone status                     paired phones, reachable or not, saved photo lists
  ef phone cache [--clear]            size of the local copies (or delete them)

WHEN (combine freely; with none, the newest 10)
  today  yesterday  week (7 days)  month (30 days)  all
  3d  2w  6h                 last 3 calendar days, 2 weeks, or a rolling 6 hours
  5                          the newest 5 (same as -n 5)
  2026-10-20                 that day;  2026-10-20..2026-10-25  a range (..B / A.. open)

Options
  -n, --last N         newest N of what matches       --since T / --until T (2d, 2026-10-01, @unix)
  --camera  --screenshots  --chats  --other   only these folders (repeatable)
  --videos             photos and videos               --only-videos
  --to DIR             also copy into DIR (never overwrites; names get (2), (3)…)
  --list               show what matches; copy nothing    --json   JSON with paths and details
  --from auto|phone|saved   auto: ask the phone for what's new, else use the saved list
  --phone NAME         which phone, when several are paired
  --max N              copy at most N files (default 50)   --max-size 2g  bytes (default 1g)";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Photo,
    Video,
    All,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum From {
    Auto,
    Phone,
    Saved,
}

/// One picture or video on the phone.
#[derive(Clone, Debug)]
struct Item {
    /// The phone's media library id (fetch by id); `None` for entries only known from the
    /// file index (fetch by path).
    id: Option<u64>,
    /// Relative to the phone's shared storage: `DCIM/Camera/IMG_1.jpg`.
    path: String,
    /// When it was added, Unix seconds.
    date: i64,
    size: u64,
    video: bool,
}

impl Item {
    fn key(&self) -> (i64, u64) {
        (self.date, self.id.unwrap_or(0))
    }
    fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
    fn source(&self) -> &'static str {
        let p = self.path.to_ascii_lowercase();
        if p.contains("screenshot") {
            "screenshots"
        } else if p.starts_with("dcim/") {
            "camera"
        } else if p.contains("whatsapp") || p.contains("telegram") || p.contains("signal") {
            "chats"
        } else {
            "other"
        }
    }
    fn to_json(&self) -> Value {
        json!({"id": self.id, "path": self.path, "date": self.date, "size": self.size, "video": self.video})
    }
    fn from_json(v: &Value) -> Option<Item> {
        let path = clean_path(v["path"].as_str()?)?;
        Some(Item { id: v["id"].as_u64(), path, date: v["date"].as_i64()?, size: v["size"].as_u64().unwrap_or(0), video: v["video"].as_bool().unwrap_or(false) })
    }
}

/// A phone-supplied relative path, or `None` if it could leave the cache folder.
fn clean_path(p: &str) -> Option<String> {
    let p = p.trim_start_matches('/');
    if p.is_empty() || p.contains('\0') || p.split('/').any(|c| c.is_empty() || c == "." || c == "..") {
        return None;
    }
    Some(p.to_owned())
}

fn valid_device(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn media_kind(name: &str) -> Option<bool> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    if PHOTO_EXT.contains(&ext.as_str()) {
        Some(false)
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Some(true)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------
// What to fetch
// ---------------------------------------------------------------------------------------

struct Want {
    since: Option<i64>,
    until: Option<i64>,
    count: Option<usize>,
    kind: Kind,
    sources: Vec<&'static str>,
}

impl Want {
    fn accepts(&self, i: &Item) -> bool {
        let kind = match self.kind {
            Kind::Photo => !i.video,
            Kind::Video => i.video,
            Kind::All => true,
        };
        kind && self.since.is_none_or(|s| i.date >= s)
            && self.until.is_none_or(|u| i.date < u)
            && (self.sources.is_empty() || self.sources.contains(&i.source()))
    }

    /// Whether a newest-first list already holds everything this wants.
    fn met(&self, list: &[Item]) -> bool {
        if let Some(n) = self.count
            && list.iter().filter(|i| self.accepts(i)).count() >= n
        {
            return true;
        }
        match self.since {
            Some(s) => list.last().is_some_and(|l| l.date < s),
            None => false,
        }
    }
}

/// Local midnight `days_back` days before today, as Unix seconds.
fn midnight(days_back: i64) -> i64 {
    let now = units::to_unix(SystemTime::now());
    let local = now + units::tz().offset_at(now);
    units::local_to_utc((local.div_euclid(86_400) - days_back) * 86_400)
}

/// A day (`2026-10-20`) as [start, end) in Unix seconds.
fn day(s: &str) -> Option<(i64, i64)> {
    let start = units::to_unix(units::parse_time(s, SystemTime::now())?);
    let local = start + units::tz().offset_at(start);
    Some((start, units::local_to_utc((local.div_euclid(86_400) + 1) * 86_400)))
}

struct Opts {
    want: Want,
    to: Option<String>,
    list: bool,
    json: bool,
    from: From,
    phone: Option<String>,
    max: usize,
    max_bytes: u64,
}

fn parse_bytes(s: &str) -> Option<u64> {
    let s = s.trim().to_ascii_lowercase();
    let s = s.strip_suffix("ib").or_else(|| s.strip_suffix('b')).unwrap_or(&s);
    let (num, mul) = match s.chars().last()? {
        'k' => (&s[..s.len() - 1], 1u64 << 10),
        'm' => (&s[..s.len() - 1], 1 << 20),
        'g' => (&s[..s.len() - 1], 1 << 30),
        't' => (&s[..s.len() - 1], 1 << 40),
        _ => (s, 1),
    };
    let v: f64 = num.trim().parse().ok()?;
    (v >= 0.0).then(|| (v * mul as f64) as u64)
}

fn parse(args: &[String], kind: Kind) -> Result<Opts, String> {
    let mut o = Opts {
        want: Want { since: None, until: None, count: None, kind, sources: Vec::new() },
        to: None,
        list: false,
        json: false,
        from: From::Auto,
        phone: None,
        max: DEFAULT_MAX,
        max_bytes: DEFAULT_MAX_BYTES,
    };
    let mut all = false;
    let now = SystemTime::now();
    let time = |s: &str| units::parse_time(s, now).map(units::to_unix).ok_or_else(|| format!("can't read the time {s:?} (try 2d, 2026-10-01 or @unix)"));
    // Windows from several words widen to cover them all.
    let window = |o: &mut Opts, since: i64, until: Option<i64>| {
        o.want.since = Some(o.want.since.map_or(since, |s| s.min(since)));
        o.want.until = match (o.want.until, until) {
            (Some(a), Some(b)) => Some(a.max(b)),
            _ => None,
        };
    };
    let mut first_window = true;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| it.next().cloned().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "-n" | "--last" | "--limit" => o.want.count = Some(val(a)?.parse().map_err(|_| format!("{a} needs a number"))?),
            "--since" | "--after" => o.want.since = Some(time(&val(a)?)?),
            "--until" | "--before" => o.want.until = Some(time(&val(a)?)?),
            "--on" => {
                let d = val(a)?;
                let (s, u) = day(&d).ok_or_else(|| format!("can't read the day {d:?}"))?;
                (o.want.since, o.want.until) = (Some(s), Some(u));
            }
            "--camera" => o.want.sources.push("camera"),
            "--screenshots" => o.want.sources.push("screenshots"),
            "--chats" => o.want.sources.push("chats"),
            "--other" => o.want.sources.push("other"),
            "--videos" => o.want.kind = Kind::All,
            "--only-videos" => o.want.kind = Kind::Video,
            "--to" => o.to = Some(val(a)?),
            "--list" | "--dry-run" | "-l" => o.list = true,
            "--json" => o.json = true,
            "--from" => {
                o.from = match val(a)?.as_str() {
                    "auto" => From::Auto,
                    "phone" | "live" => From::Phone,
                    "saved" | "index" | "cache" => From::Saved,
                    v => return Err(format!("--from takes auto, phone or saved, not {v:?}")),
                }
            }
            "--phone" | "--device" => o.phone = Some(val(a)?),
            "--max" => o.max = val(a)?.parse().map_err(|_| "--max needs a number".to_string())?,
            "--max-size" => o.max_bytes = parse_bytes(&val(a)?).ok_or("--max-size takes a size like 500m or 2g")?,
            w if w.starts_with('-') && w.len() > 1 && !w[1..].starts_with(|c: char| c.is_ascii_digit()) => return Err(format!("unknown option {w}; see `ef phone help`")),
            w => {
                let w = w.to_ascii_lowercase();
                let mut set = |o: &mut Opts, s: i64, u: Option<i64>| {
                    if first_window {
                        (o.want.since, o.want.until) = (Some(s), u);
                        first_window = false;
                    } else {
                        window(o, s, u)
                    }
                };
                if let Ok(n) = w.parse::<usize>() {
                    o.want.count = Some(n);
                } else if w == "today" {
                    set(&mut o, midnight(0), None);
                } else if w == "yesterday" {
                    set(&mut o, midnight(1), Some(midnight(0)));
                } else if w == "week" || w == "this-week" || w == "last-week" {
                    set(&mut o, midnight(6), None);
                } else if w == "month" {
                    set(&mut o, midnight(29), None);
                } else if w == "all" || w == "everything" {
                    all = true;
                } else if let Some((n, unit)) = w.find(|c: char| !c.is_ascii_digit()).filter(|&p| p > 0).map(|p| w.split_at(p))
                    && let Ok(n) = n.parse::<i64>()
                    && matches!(unit, "d" | "day" | "days" | "w" | "week" | "weeks" | "h" | "hour" | "hours")
                {
                    match unit.as_bytes()[0] {
                        b'd' => set(&mut o, midnight(n.max(1) - 1), None),
                        b'w' => set(&mut o, midnight(n.max(1) * 7 - 1), None),
                        _ => set(&mut o, units::to_unix(now) - n * 3600, None),
                    }
                } else if let Some((a, b)) = w.split_once("..") {
                    let s = if a.is_empty() { 0 } else { day(a).ok_or_else(|| format!("can't read the day {a:?}"))?.0 };
                    let u = if b.is_empty() { None } else { Some(day(b).ok_or_else(|| format!("can't read the day {b:?}"))?.1) };
                    set(&mut o, s, u);
                } else if let Some((s, u)) = day(&w) {
                    set(&mut o, s, Some(u));
                } else {
                    return Err(format!("don't know {w:?}: try today, yesterday, week, 3d, 5 or 2026-10-20 (see `ef phone help`)"));
                }
            }
        }
    }
    if !all && o.want.count.is_none() && o.want.since.is_none() && o.want.until.is_none() {
        o.want.count = Some(DEFAULT_COUNT);
    }
    if all && o.want.since.is_none() {
        o.want.since = Some(0);
    }
    Ok(o)
}

// ---------------------------------------------------------------------------------------
// The saved photo list
// ---------------------------------------------------------------------------------------

fn saved_dir() -> PathBuf {
    ef_config::state_dir().join("phone-photo-list")
}

/// The saved list for `device`, newest first, and whether it reaches the phone's oldest photo.
fn load_saved(device: &str) -> (Vec<Item>, bool, Option<SystemTime>) {
    let path = saved_dir().join(format!("{device}.jsonl"));
    let Ok(text) = std::fs::read_to_string(&path) else { return (Vec::new(), false, None) };
    let when = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    let mut lines = text.lines();
    let bottom = lines.next().and_then(|l| serde_json::from_str::<Value>(l).ok()).and_then(|v| v["bottom"].as_bool()).unwrap_or(false);
    let mut items: Vec<Item> = lines.filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter_map(|v| Item::from_json(&v)).collect();
    items.sort_by_key(|i| std::cmp::Reverse(i.key()));
    (items, bottom, when)
}

fn save(device: &str, items: &[Item], bottom: bool) {
    let dir = saved_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut out = json!({"bottom": bottom}).to_string();
    out.push('\n');
    for i in items.iter().take(MAX_SAVED) {
        out.push_str(&i.to_json().to_string());
        out.push('\n');
    }
    let _ = ef_config::atomic_write(&dir.join(format!("{device}.jsonl")), out.as_bytes());
}

/// Photos and videos the full file index knows about, for when the phone is away.
fn from_file_index(device: &str) -> Vec<Item> {
    crate::phone::indexed_files(device)
        .into_iter()
        .filter(|(p, _, _, dir)| !dir && !p.split('/').any(|c| c.starts_with('.')) && !p.starts_with("Android/"))
        .filter_map(|(p, size, modified, _)| Some(Item { video: media_kind(&p)?, id: None, path: clean_path(&p)?, date: modified, size }))
        .collect()
}

/// One page (100, newest first) of the phone's media library, older than `before`.
fn page(link: &dyn Link, device: &str, before: Option<(i64, u64)>) -> Result<(Vec<Item>, bool), String> {
    let body = match before {
        Some((d, i)) => json!({"before": d, "beforeId": i}),
        None => json!({}),
    };
    let r = link.ask(device, "echofiles.photos.request", body, Duration::from_secs(20))?;
    let Some(list) = r["photos"].as_array() else {
        return Err(r["message"].as_str().or(r["error"].as_str()).unwrap_or("The phone didn't send its photo list").to_owned());
    };
    let mut items = Vec::new();
    for e in list {
        let (Some(id), Some(name), Some(date)) = (e["id"].as_u64(), e["name"].as_str(), e["date"].as_i64()) else { continue };
        // Trashed and pending items are dot-files.
        if name.starts_with('.') || name.contains('/') {
            continue;
        }
        let folder = e["folder"].as_str().unwrap_or("").trim_matches('/');
        let Some(path) = clean_path(&if folder.is_empty() { name.to_owned() } else { format!("{folder}/{name}") }) else { continue };
        items.push(Item { id: Some(id), path, date, size: e["size"].as_u64().unwrap_or(0), video: e["video"].as_bool().unwrap_or(false) });
    }
    Ok((items, r["more"].as_bool().unwrap_or(false)))
}

/// The phone's photo list, newest first, complete enough for `want`: asks only for what's
/// newer than the saved list (and older pages if the window reaches past it), then saves.
fn refresh(link: &dyn Link, device: &str, want: &Want) -> Result<Vec<Item>, String> {
    let (old, old_bottom, _) = load_saved(device);
    let mut rest = Some(old);
    let mut list: Vec<Item> = Vec::new();
    let mut bottom = false;
    let mut cursor = None;
    let mut extra = 0;
    for _ in 0..MAX_PAGES {
        let (items, more) = page(link, device, cursor)?;
        let empty = items.is_empty();
        list.extend(items);
        if !more || empty {
            bottom = true;
            break;
        }
        let last = list.last().map(Item::key).unwrap_or_default();
        // Reached the saved list: everything below `last` is already known.
        if let Some(r) = rest.as_mut()
            && r.first().is_some_and(|top| last <= top.key())
        {
            r.retain(|i| i.key() < last);
            list.append(r);
            rest = None;
            bottom = old_bottom;
            if bottom || want.met(&list) {
                break;
            }
            cursor = list.last().map(Item::key);
            continue;
        }
        if want.met(&list) {
            // Walking a few more pages joins a slightly stale saved list to this one;
            // past that the saved list is dropped rather than left with a gap.
            extra += 1;
            if rest.as_ref().is_none_or(Vec::is_empty) || extra > 3 {
                break;
            }
        }
        cursor = Some(last);
    }
    let mut seen = std::collections::HashSet::new();
    list.retain(|i| seen.insert(i.key()));
    save(device, &list, bottom);
    Ok(list)
}

// ---------------------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------------------

fn local_path(device: &str, item: &Item) -> PathBuf {
    ef_config::phone_cache_dir().join(device).join(&item.path)
}

/// Already on the laptop, complete.
fn cached(device: &str, item: &Item) -> Option<PathBuf> {
    let p = local_path(device, item);
    let m = std::fs::symlink_metadata(&p).ok()?;
    (m.is_file() && (item.size == 0 || m.len() == item.size)).then_some(p)
}

fn private_dir(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(p)?;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700));
    Ok(())
}

fn fetch(link: &dyn Link, device: &str, item: &Item) -> Result<PathBuf, String> {
    let root = ef_config::phone_cache_dir();
    private_dir(&root).map_err(|e| format!("can't create {}: {e}", root.display()))?;
    let dest = local_path(device, item);
    let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
    static N: AtomicUsize = AtomicUsize::new(0);
    let incoming = root.join(device).join(".incoming").join(format!("{stamp}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    private_dir(&incoming).map_err(|e| e.to_string())?;
    struct Gone(PathBuf);
    impl Drop for Gone {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _gone = Gone(incoming.clone());
    // A minute to start, then time for the bytes at a slow 1 MB/s.
    let wait = Duration::from_secs(60 + item.size / (1 << 20));
    let by_path = || link.fetch(device, "echofiles.files.read", json!({"path": item.path}), &incoming, wait);
    let got = match item.id {
        Some(id) => link.fetch(device, "echofiles.photos.read", json!({"id": id}), &incoming, wait).or_else(|e| by_path().map_err(|_| e))?,
        None => by_path()?,
    };
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&got, &dest).map_err(|e| format!("can't save {}: {e}", dest.display()))?;
    Ok(dest)
}

/// `dir/name`, or `name (2).ext`… — never an existing file.
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if std::fs::symlink_metadata(&first).is_err() {
        return first;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(p) if p > 0 => (&name[..p], &name[p..]),
        _ => (name, ""),
    };
    (2..10_000).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| std::fs::symlink_metadata(p).is_err()).unwrap_or(first)
}

fn copy_to(src: &Path, dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let target = free_name(dir, name);
    if std::fs::hard_link(src, &target).is_err() {
        use std::os::unix::fs::OpenOptionsExt;
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&target)?;
        std::io::copy(&mut std::fs::File::open(src)?, &mut out)?;
    }
    Ok(target)
}

// ---------------------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------------------

fn saved_devices() -> Vec<String> {
    let mut ids: Vec<String> = [saved_dir(), ef_config::state_dir().join("phone-index")]
        .iter()
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".jsonl").map(str::to_owned))
        .filter(|id| valid_device(id))
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The phone to use: `--phone` (id or name), else the reachable EchoConnect phone, else
/// the only one known.
fn pick(phones: &[LinkPhone], wanted: Option<&str>) -> Result<(String, String, bool), String> {
    if let Some(w) = wanted {
        if let Some(p) = phones.iter().find(|p| p.id == w || p.name.eq_ignore_ascii_case(w)) {
            return Ok((p.id.clone(), p.name.clone(), p.online && p.echoconnect));
        }
        if valid_device(w) && saved_devices().iter().any(|d| d == w) {
            return Ok((w.to_owned(), w.to_owned(), false));
        }
        let names: Vec<&str> = phones.iter().map(|p| p.name.as_str()).collect();
        return Err(format!("no paired phone called {w:?}{}", if names.is_empty() { String::new() } else { format!(" (paired: {})", names.join(", ")) }));
    }
    let online: Vec<&LinkPhone> = phones.iter().filter(|p| p.online && p.echoconnect).collect();
    match online.as_slice() {
        [p] => return Ok((p.id.clone(), p.name.clone(), true)),
        [_, _, ..] => return Err(format!("several phones are nearby ({}); pick one with --phone", online.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", "))),
        [] => {}
    }
    if let [p] = phones {
        return Ok((p.id.clone(), p.name.clone(), false));
    }
    let saved = saved_devices();
    match (phones.len(), saved.as_slice()) {
        (0, [id]) => Ok((id.clone(), id.clone(), false)),
        (0, []) => Err("no phone is paired with EchoFiles (pair one in EchoFiles → Phone)".into()),
        _ => Err("more than one phone is paired; pick one with --phone".into()),
    }
}

fn status(ctx: &Ctx) -> u8 {
    if !ctx.settings.agents.phone {
        return fail(ctx, 3, OFF);
    }
    let link = crate::phone::link();
    if link.is_none() {
        outln!(ctx, "EchoFiles isn't running here: phones can't be reached; saved lists below.");
    }
    let phones = link.map(|l| l.phones()).unwrap_or_default();
    let now = SystemTime::now();
    for p in &phones {
        let state = if !p.online {
            "away"
        } else if p.echoconnect {
            "nearby"
        } else {
            "nearby, without EchoConnect (photos need EchoConnect)"
        };
        outln!(ctx, "{}  {}  [{}]", p.name, state, p.id);
    }
    for id in saved_devices() {
        let (items, bottom, when) = load_saved(&id);
        if items.is_empty() {
            continue;
        }
        let name = phones.iter().find(|p| p.id == id).map_or(id.as_str(), |p| p.name.as_str());
        let age = when.map(|w| units::ago(w, now)).unwrap_or_default();
        outln!(ctx, "  {name}: {} photos and videos saved{}, updated {age}", items.len(), if bottom { " (all)" } else { "" });
    }
    if phones.is_empty() && saved_devices().is_empty() {
        outln!(ctx, "No phone is paired with EchoFiles.");
    }
    if !ctx.settings.phone.files {
        outln!(ctx, "Settings → Phone → Files is off: EchoFiles won't accept files from the phone.");
    }
    0
}

fn dir_size(p: &Path) -> (u64, usize) {
    let mut total = (0, 0);
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => {
                    total.0 += e.metadata().map(|m| m.len()).unwrap_or(0);
                    total.1 += 1;
                }
                _ => {}
            }
        }
    }
    total
}

fn cache(args: &[String], ctx: &Ctx) -> u8 {
    let root = ef_config::phone_cache_dir();
    if args.iter().any(|a| a == "--clear") {
        let (bytes, files) = dir_size(&root);
        return match std::fs::remove_dir_all(&root) {
            Ok(()) => {
                outln!(ctx, "Deleted {files} copied files ({}) from {}", units::human_size(bytes), root.display());
                0
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                outln!(ctx, "Nothing to delete.");
                0
            }
            Err(e) => fail(ctx, 1, format!("can't delete {}: {e}", root.display())),
        };
    }
    let (bytes, files) = dir_size(&root);
    outln!(ctx, "{files} files, {} in {}", units::human_size(bytes), root.display());
    0
}

fn photos(args: &[String], ctx: &Ctx, kind: Kind) -> u8 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        outln!(ctx, "{}", HELP);
        return 0;
    }
    if !ctx.settings.agents.phone {
        return fail(ctx, 3, OFF);
    }
    let o = match parse(args, kind) {
        Ok(o) => o,
        Err(e) => return fail(ctx, 2, e),
    };
    let link = crate::phone::link();
    let phones = link.map(|l| l.phones()).unwrap_or_default();
    let (device, name, reachable) = match pick(&phones, o.phone.as_deref()) {
        Ok(p) => p,
        Err(e) => return fail(ctx, 1, e),
    };
    let reachable = reachable && link.is_some();

    // Which photos.
    let use_phone = match o.from {
        From::Saved => false,
        From::Phone if !reachable => {
            return fail(ctx, 1, if link.is_none() { "EchoFiles isn't running, so the phone can't be asked (start EchoFiles, or use --from saved)".to_string() } else { format!("{name} isn't nearby; use --from saved for the saved list") });
        }
        _ => reachable,
    };
    let mut items = None;
    if use_phone && let Some(l) = link {
        match refresh(l, &device, &o.want) {
            Ok(list) => items = Some(list),
            Err(e) if o.from == From::Phone => return fail(ctx, 1, format!("{name}: {e}")),
            Err(e) => errln!(ctx, "ef: {name} didn't send its photo list ({e}); using the saved list"),
        }
    }
    let items = items.unwrap_or_else(|| {
        let (mut list, _, when) = load_saved(&device);
        let known: std::collections::HashSet<String> = list.iter().map(|i| i.path.clone()).collect();
        list.extend(from_file_index(&device).into_iter().filter(|i| !known.contains(&i.path)));
        list.sort_by_key(|i| std::cmp::Reverse(i.key()));
        if o.from != From::Saved {
            let why = if link.is_none() { "EchoFiles isn't running".to_string() } else { format!("{name} isn't nearby") };
            let age = when.map(|w| format!(", updated {}", units::ago(w, SystemTime::now()))).unwrap_or_default();
            errln!(ctx, "ef: {why}: answering from the saved photo list{age}; only photos already copied can be returned");
        }
        list
    });

    let mut chosen: Vec<&Item> = items.iter().filter(|i| o.want.accepts(i)).collect();
    if let Some(n) = o.want.count {
        chosen.truncate(n);
    }
    if chosen.is_empty() {
        errln!(ctx, "ef: no {} on {name} match", match o.want.kind { Kind::Photo => "photos", Kind::Video => "videos", Kind::All => "photos or videos" });
        if o.json {
            outln!(ctx, "[]");
        }
        return 1;
    }

    if o.list {
        return report(ctx, &o, &device, &name, chosen.iter().map(|i| (*i, cached(&device, i).ok_or(String::new()), false)).collect());
    }

    // Safeguards: how many, and how many bytes, one run copies.
    let matched = chosen.len();
    if chosen.len() > o.max {
        chosen.truncate(o.max);
    }
    let mut bytes = 0u64;
    let mut keep = 0;
    for i in &chosen {
        let new = if cached(&device, i).is_some() { 0 } else { i.size };
        if keep > 0 && bytes + new > o.max_bytes {
            break;
        }
        bytes += new;
        keep += 1;
    }
    chosen.truncate(keep);
    if chosen.len() < matched {
        errln!(ctx, "ef: {matched} match; copying the newest {} (raise --max / --max-size for more)", chosen.len());
    }

    let already: Vec<bool> = chosen.iter().map(|i| cached(&device, i).is_some()).collect();
    let missing = already.iter().filter(|c| !**c).count();
    if missing > 0 && (!reachable || !ctx.settings.phone.files) {
        let why = if !ctx.settings.phone.files {
            "Settings → Phone → Files is off".to_string()
        } else if link.is_none() {
            "EchoFiles isn't running".to_string()
        } else {
            format!("{name} isn't nearby")
        };
        errln!(ctx, "ef: {why}: {missing} of {} aren't on the laptop yet", chosen.len());
    } else if missing > 0 && !o.json && ctx.out_tty {
        errln!(ctx, "ef: copying {missing} from {name}…");
    }

    // Copy, a few at a time; results keep the newest-first order.
    let results: Vec<Mutex<Option<Result<PathBuf, String>>>> = chosen.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let can_fetch = reachable && ctx.settings.phone.files;
    std::thread::scope(|s| {
        for _ in 0..PARALLEL.min(chosen.len()) {
            s.spawn(|| {
                loop {
                    let n = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = chosen.get(n) else { break };
                    let r = match cached(&device, item) {
                        Some(p) => Ok(p),
                        None if can_fetch => fetch(link.unwrap(), &device, item),
                        None => Err("not on the laptop yet".to_string()),
                    };
                    *results[n].lock().unwrap() = Some(r);
                }
            });
        }
    });
    let mut rows = Vec::new();
    for ((i, r), was_cached) in chosen.iter().zip(results).zip(already) {
        let mut r = r.into_inner().unwrap().unwrap_or_else(|| Err("not copied".into()));
        if let (Ok(p), Some(to)) = (&r, &o.to) {
            let dir = ctx.abs(to);
            r = std::fs::create_dir_all(&dir).and_then(|_| copy_to(p, &dir, i.name())).map_err(|e| format!("can't copy into {}: {e}", dir.display()));
        }
        rows.push((*i, r, was_cached));
    }
    report(ctx, &o, &device, &name, rows)
}

fn report(ctx: &Ctx, o: &Opts, device: &str, name: &str, rows: Vec<(&Item, Result<PathBuf, String>, bool)>) -> u8 {
    let mut failed = 0;
    if o.json {
        let list: Vec<Value> = rows
            .iter()
            .map(|(i, r, was_cached)| {
                let mut v = json!({
                    "phone": name,
                    "phone_id": device,
                    "phone_path": i.path,
                    "name": i.name(),
                    "taken": units::format_rfc3339(units::from_unix(i.date)),
                    "size": i.size,
                    "video": i.video,
                    "source": i.source(),
                    "path": r.as_ref().ok().map(|p| p.to_string_lossy()),
                });
                if !o.list {
                    v["copied_now"] = json!(r.is_ok() && !*was_cached);
                }
                if let Err(e) = r
                    && !o.list
                {
                    v["error"] = json!(e);
                }
                v
            })
            .collect();
        failed = rows.iter().filter(|(_, r, _)| r.is_err()).count();
        outln!(ctx, "{}", serde_json::to_string_pretty(&list).unwrap_or_default());
        return if o.list || failed == 0 { 0 } else { 1 };
    }
    for (i, r, _) in &rows {
        if o.list {
            let mut size = String::new();
            if i.size > 0 {
                size = units::human_size(i.size);
            }
            let here = r.as_ref().map(|p| format!("  → {}", p.display())).unwrap_or_default();
            outln!(ctx, "{}  {:>9}  {:<11}  {}{here}", units::format_local(units::from_unix(i.date)), size, i.source(), i.path);
            continue;
        }
        match r {
            Ok(p) => outln!(ctx, "{}", p.display()),
            Err(e) => {
                failed += 1;
                errln!(ctx, "ef: {}: {e}", i.path);
            }
        }
    }
    if failed > 0 { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(date: i64, id: u64, path: &str, video: bool) -> Item {
        Item { id: Some(id), path: path.into(), date, size: 10, video }
    }

    #[test]
    fn paths_from_the_phone_stay_inside_the_cache() {
        assert_eq!(clean_path("DCIM/Camera/a.jpg").as_deref(), Some("DCIM/Camera/a.jpg"));
        assert_eq!(clean_path("/DCIM/a.jpg").as_deref(), Some("DCIM/a.jpg"));
        for bad in ["", "../a.jpg", "DCIM/../../a", "DCIM//a.jpg", "DCIM/./a", "a\0b"] {
            assert_eq!(clean_path(bad), None, "{bad:?}");
        }
        assert!(!valid_device("../x") && !valid_device("") && valid_device("abc_12-3"));
    }

    #[test]
    fn words_become_windows_and_counts() {
        let o = parse(&["5".into()], Kind::Photo).unwrap();
        assert_eq!((o.want.count, o.want.since), (Some(5), None));
        let o = parse(&[], Kind::Photo).unwrap();
        assert_eq!(o.want.count, Some(DEFAULT_COUNT));
        let o = parse(&["today".into()], Kind::Photo).unwrap();
        assert_eq!((o.want.since, o.want.until, o.want.count), (Some(midnight(0)), None, None));
        let o = parse(&["yesterday".into()], Kind::Photo).unwrap();
        assert_eq!((o.want.since, o.want.until), (Some(midnight(1)), Some(midnight(0))));
        // Two words widen: today and yesterday.
        let o = parse(&["today".into(), "yesterday".into()], Kind::Photo).unwrap();
        assert_eq!((o.want.since, o.want.until), (Some(midnight(1)), None));
        let o = parse(&["2d".into(), "3".into()], Kind::Photo).unwrap();
        assert_eq!((o.want.since, o.want.count), (Some(midnight(1)), Some(3)));
        let o = parse(&["week".into()], Kind::Photo).unwrap();
        assert_eq!(o.want.since, Some(midnight(6)));
        assert!(parse(&["tomorrowish".into()], Kind::Photo).is_err());
        assert!(parse(&["--bogus".into()], Kind::Photo).is_err());
        assert_eq!(parse(&["--max-size".into(), "2g".into()], Kind::Photo).unwrap().max_bytes, 2 << 30);
    }

    #[test]
    fn wants_filter_kind_source_and_window() {
        let w = Want { since: Some(100), until: Some(200), count: None, kind: Kind::Photo, sources: vec!["camera"] };
        assert!(w.accepts(&item(150, 1, "DCIM/Camera/a.jpg", false)));
        assert!(!w.accepts(&item(150, 1, "DCIM/Camera/a.mp4", true)));
        assert!(!w.accepts(&item(250, 1, "DCIM/Camera/a.jpg", false)));
        assert!(!w.accepts(&item(150, 1, "Pictures/Screenshots/a.png", false)));
        let list = [item(300, 3, "DCIM/a.jpg", false), item(90, 2, "DCIM/b.jpg", false)];
        assert!(w.met(&list), "walked past the window's start");
        let w = Want { since: None, until: None, count: Some(2), kind: Kind::All, sources: vec![] };
        assert!(w.met(&list) && !w.met(&list[..1]));
    }

    /// A phone with `n` photos, one per second, answering pages like EchoConnect.
    struct Fake(Vec<Item>, AtomicUsize);
    impl Link for Fake {
        fn phones(&self) -> Vec<LinkPhone> {
            vec![LinkPhone { id: "p1".into(), name: "Pixel".into(), online: true, echoconnect: true }]
        }
        fn ask(&self, _: &str, _: &str, body: Value, _: Duration) -> Result<Value, String> {
            self.1.fetch_add(1, Ordering::Relaxed);
            let before = (body["before"].as_i64().unwrap_or(i64::MAX), body["beforeId"].as_u64().unwrap_or(u64::MAX));
            let page: Vec<Value> = self.0.iter().filter(|i| i.key() < before).take(100).map(|i| json!({"id": i.id, "name": i.name(), "folder": "DCIM/Camera/", "date": i.date, "size": i.size, "video": false})).collect();
            let more = page.len() == 100;
            Ok(json!({"photos": page, "more": more}))
        }
        fn fetch(&self, _: &str, _: &str, _: Value, _: &Path, _: Duration) -> Result<PathBuf, String> {
            Err("no".into())
        }
    }

    #[test]
    fn refresh_asks_only_for_whats_new() {
        let state = tempfile::tempdir().unwrap();
        // SAFETY: tests in this module are the only ones reading XDG_STATE_HOME.
        unsafe { std::env::set_var("XDG_STATE_HOME", state.path()) };
        let photos = |n: u64| (1..=n).rev().map(|i| item(i as i64, i, &format!("DCIM/Camera/{i}.jpg"), false)).collect::<Vec<_>>();
        let want = |count| Want { since: None, until: None, count: Some(count), kind: Kind::Photo, sources: vec![] };
        let phone = Fake(photos(450), AtomicUsize::new(0));
        let list = refresh(&phone, "p1", &want(250)).unwrap();
        assert_eq!(list.len(), 300);
        assert_eq!(phone.1.load(Ordering::Relaxed), 3);
        // Five new photos: one page reaches the saved list, which supplies the rest.
        let phone = Fake(photos(455), AtomicUsize::new(0));
        let list = refresh(&phone, "p1", &want(250)).unwrap();
        assert_eq!(phone.1.load(Ordering::Relaxed), 1);
        assert_eq!(list.len(), 305);
        assert_eq!(list[0].date, 455);
        assert!(list.windows(2).all(|w| w[0].key() > w[1].key()));
        // Asking past the saved list continues from its oldest entry, to the end.
        let phone = Fake(photos(455), AtomicUsize::new(0));
        let list = refresh(&phone, "p1", &Want { since: Some(0), until: None, count: None, kind: Kind::Photo, sources: vec![] }).unwrap();
        assert_eq!(list.len(), 455);
        assert_eq!(load_saved("p1").1, true);
    }
}

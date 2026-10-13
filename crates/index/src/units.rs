//! Parsing for filter values: sizes (`+1m`), durations (`2weeks`), dates (`2024-01-05`),
//! owners (`me:staff`), and the local time zone (read from TZif files, no C library).

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------------------
// Sizes
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeFilter {
    AtLeast(u64),
    AtMost(u64),
    Exactly(u64),
}

impl SizeFilter {
    /// `+10k` (at least), `-1m` (at most), `500` (exactly). Units: `b`, `k`/`kb` (1000),
    /// `ki`/`kib` (1024), likewise `m`, `g`, `t`. Case-insensitive.
    pub fn parse(s: &str) -> Option<SizeFilter> {
        let s = s.trim();
        let (sign, rest) = match s.as_bytes().first()? {
            b'+' => (1, &s[1..]),
            b'-' => (-1, &s[1..]),
            _ => (0, s),
        };
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let n: u64 = rest[..digits].parse().ok()?;
        let unit = rest[digits..].to_ascii_lowercase();
        let mult: u64 = match unit.as_str() {
            "" | "b" => 1,
            "k" | "kb" => 1000,
            "ki" | "kib" => 1 << 10,
            "m" | "mb" => 1000u64.pow(2),
            "mi" | "mib" => 1 << 20,
            "g" | "gb" => 1000u64.pow(3),
            "gi" | "gib" => 1 << 30,
            "t" | "tb" => 1000u64.pow(4),
            "ti" | "tib" => 1 << 40,
            _ => return None,
        };
        let v = n.checked_mul(mult)?;
        Some(match sign {
            1 => SizeFilter::AtLeast(v),
            -1 => SizeFilter::AtMost(v),
            _ => SizeFilter::Exactly(v),
        })
    }

    pub fn accepts(&self, size: u64) -> bool {
        match *self {
            SizeFilter::AtLeast(v) => size >= v,
            SizeFilter::AtMost(v) => size <= v,
            SizeFilter::Exactly(v) => size == v,
        }
    }
}

/// `1.2 MB`-style size for people.
pub fn human_size(n: u64) -> String {
    const U: [&str; 6] = ["B", "kB", "MB", "GB", "TB", "PB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 { format!("{n} B") } else if v < 10.0 { format!("{v:.1} {}", U[i]) } else { format!("{v:.0} {}", U[i]) }
}

// ---------------------------------------------------------------------------------------
// Durations and points in time
// ---------------------------------------------------------------------------------------

/// `10h`, `2weeks`, `1h 30min`, `3d12h`, `90s` — the units fd (humantime) accepts.
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let b = s.as_bytes();
    let mut i = 0;
    let mut total: u128 = 0;
    let mut any = false;
    while i < b.len() {
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        if i == b.len() {
            break;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if start == i {
            return None;
        }
        let n: u128 = s[start..i].parse().ok()?;
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        let ustart = i;
        while i < b.len() && b[i].is_ascii_alphabetic() {
            i += 1;
        }
        let nanos: u128 = match &s[ustart..i] {
            "nsec" | "ns" | "nanos" => 1,
            "usec" | "us" | "micros" => 1_000,
            "msec" | "ms" | "millis" => 1_000_000,
            "seconds" | "second" | "secs" | "sec" | "s" => 1_000_000_000,
            "minutes" | "minute" | "mins" | "min" | "m" => 60_000_000_000,
            "hours" | "hour" | "hrs" | "hr" | "h" => 3_600_000_000_000,
            "days" | "day" | "d" => 86_400_000_000_000,
            "weeks" | "week" | "w" => 604_800_000_000_000,
            "months" | "month" | "M" => 2_630_016_000_000_000,
            "years" | "year" | "y" => 31_557_600_000_000_000,
            _ => return None,
        };
        total = total.checked_add(n.checked_mul(nanos)?)?;
        any = true;
    }
    any.then(|| Duration::new((total / 1_000_000_000) as u64, (total % 1_000_000_000) as u32))
}

/// A point in time for `--changed-within`/`--changed-before`: a duration back from now, an
/// RFC 3339 time, `YYYY-MM-DD` (local midnight), `YYYY-MM-DD HH:MM:SS` (local), or
/// `@unix-seconds`.
pub fn parse_time(s: &str, now: SystemTime) -> Option<SystemTime> {
    let s = s.trim();
    if let Some(d) = parse_duration(s) {
        return now.checked_sub(d);
    }
    if let Some(secs) = s.strip_prefix('@') {
        let v: i64 = secs.parse().ok()?;
        return Some(from_unix(v));
    }
    let b = s.as_bytes();
    let num = |a: usize, n: usize| -> Option<i64> {
        let part = s.get(a..a + n)?;
        part.bytes().all(|c| c.is_ascii_digit()).then(|| part.parse().ok())?
    };
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (y, mo, d) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let (mut h, mut mi, mut sec, mut frac) = (0, 0, 0, 0u32);
    let mut rest = &s[10..];
    if !rest.is_empty() {
        let sep = rest.as_bytes()[0];
        if !matches!(sep, b' ' | b'T' | b't') {
            return None;
        }
        let r = &rest[1..];
        let rb = r.as_bytes();
        if rb.len() < 5 || rb[2] != b':' {
            return None;
        }
        let n2 = |a: usize| -> Option<i64> { r.get(a..a + 2).filter(|p| p.bytes().all(|c| c.is_ascii_digit()))?.parse().ok() };
        h = n2(0)?;
        mi = n2(3)?;
        let mut at = 5;
        if rb.len() >= 8 && rb[5] == b':' {
            sec = n2(6)?;
            at = 8;
            if rb.get(8) == Some(&b'.') {
                let digits: String = r[9..].chars().take_while(|c| c.is_ascii_digit()).collect();
                at = 9 + digits.len();
                let padded = format!("{digits:0<9}");
                frac = padded[..9].parse().unwrap_or(0);
            }
        }
        if h > 23 || mi > 59 || sec > 60 {
            return None;
        }
        rest = &r[at..];
    }
    let local = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + sec;
    let utc = if rest.is_empty() {
        local_to_utc(local)
    } else if rest == "Z" || rest == "z" {
        local
    } else {
        let sign = match rest.as_bytes()[0] {
            b'+' => 1,
            b'-' => -1,
            _ => return None,
        };
        let o = &rest[1..];
        let (oh, om) = match o.len() {
            5 if o.as_bytes()[2] == b':' => (o[..2].parse::<i64>().ok()?, o[3..].parse::<i64>().ok()?),
            4 => (o[..2].parse::<i64>().ok()?, o[2..].parse::<i64>().ok()?),
            2 => (o.parse::<i64>().ok()?, 0),
            _ => return None,
        };
        local - sign * (oh * 3600 + om * 60)
    };
    Some(from_unix(utc) + Duration::from_nanos(frac as u64))
}

pub fn from_unix(secs: i64) -> SystemTime {
    if secs >= 0 { UNIX_EPOCH + Duration::from_secs(secs as u64) } else { UNIX_EPOCH - Duration::from_secs(secs.unsigned_abs()) }
}

pub fn to_unix(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// (year, month, day) of a day count since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `2026-10-13 14:05` in local time.
pub fn format_local(t: SystemTime) -> String {
    let u = to_unix(t);
    let l = u + tz().offset_at(u);
    let (y, m, d) = civil_from_days(l.div_euclid(86_400));
    let s = l.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", s / 3600, s / 60 % 60)
}

/// RFC 3339 in UTC, for JSON.
pub fn format_rfc3339(t: SystemTime) -> String {
    let u = to_unix(t);
    let (y, m, d) = civil_from_days(u.div_euclid(86_400));
    let s = u.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / 3600, s / 60 % 60, s % 60)
}

/// `3 min ago`, `2 h ago`, `5 days ago`.
pub fn ago(t: SystemTime, now: SystemTime) -> String {
    let s = now.duration_since(t).map_or(0, |d| d.as_secs());
    match s {
        0..=59 => format!("{s} s ago"),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=172_799 => format!("{} h ago", s / 3600),
        _ => format!("{} days ago", s / 86_400),
    }
}

// ---------------------------------------------------------------------------------------
// Time zone
// ---------------------------------------------------------------------------------------

/// The local time zone: transitions from a TZif file, then its POSIX rule for later times.
#[derive(Debug, Default)]
pub struct Tz {
    /// (UTC transition time, offset in seconds from then on), ascending.
    transitions: Vec<(i64, i64)>,
    /// Offset before the first transition (or always, with no transitions and no rule).
    initial: i64,
    rule: Option<PosixTz>,
}

#[derive(Debug, Clone)]
struct PosixTz {
    std_off: i64,
    dst: Option<(i64, RuleDate, i64, RuleDate, i64)>,
}

/// `Mm.w.d`, `Jn` or `n`.
#[derive(Debug, Clone, Copy)]
enum RuleDate {
    MonthWeekDay(i64, i64, i64),
    Julian1(i64),
    Julian0(i64),
}

pub fn tz() -> &'static Tz {
    static TZ: std::sync::OnceLock<Tz> = std::sync::OnceLock::new();
    TZ.get_or_init(Tz::local)
}

impl Tz {
    fn local() -> Tz {
        let file = match std::env::var("TZ") {
            Ok(v) if !v.is_empty() => {
                let name = v.trim_start_matches(':');
                if name.starts_with('/') {
                    std::path::PathBuf::from(name)
                } else if let Some(t) = parse_posix(name) {
                    return Tz { rule: Some(t), ..Default::default() };
                } else {
                    Path::new("/usr/share/zoneinfo").join(name)
                }
            }
            _ => "/etc/localtime".into(),
        };
        std::fs::read(file).ok().and_then(|b| Tz::parse_tzif(&b)).unwrap_or_default()
    }

    fn parse_tzif(b: &[u8]) -> Option<Tz> {
        let be32 = |at: usize| -> Option<i64> { Some(i32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?) as i64) };
        let be64 = |at: usize| -> Option<i64> { Some(i64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?)) };
        if b.get(..4)? != b"TZif" {
            return None;
        }
        let version = *b.get(4)?;
        let counts = |at: usize| -> Option<[usize; 6]> {
            let mut c = [0usize; 6];
            for (k, v) in c.iter_mut().enumerate() {
                *v = be32(at + 20 + 4 * k)? as usize;
            }
            Some(c)
        };
        // [isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt]
        let c1 = counts(0)?;
        let v1_len = 44 + c1[3] * 5 + c1[4] * 6 + c1[5] + c1[2] * 8 + c1[1] + c1[0];
        let (base, c, tsize) = if version >= b'2' { (v1_len, counts(v1_len)?, 8) } else { (0, c1, 4) };
        let mut at = base + 44;
        let mut times = Vec::with_capacity(c[3]);
        for _ in 0..c[3] {
            times.push(if tsize == 8 { be64(at)? } else { be32(at)? });
            at += tsize;
        }
        let idx: Vec<usize> = b.get(at..at + c[3])?.iter().map(|&x| x as usize).collect();
        at += c[3];
        let mut offs = Vec::with_capacity(c[4]);
        for k in 0..c[4] {
            offs.push((be32(at + 6 * k)?, *b.get(at + 6 * k + 4)? != 0));
        }
        at += 6 * c[4] + c[5] + c[2] * (tsize + 4) + c[1] + c[0];
        let transitions = times.iter().zip(&idx).filter_map(|(&t, &i)| offs.get(i).map(|o| (t, o.0))).collect();
        // The first non-DST type is the offset before any transition.
        let initial = offs.iter().find(|o| !o.1).or(offs.first()).map_or(0, |o| o.0);
        let rule = if version >= b'2' {
            b.get(at..).and_then(|rest| {
                let rest = rest.strip_prefix(b"\n")?;
                let end = rest.iter().position(|&c| c == b'\n')?;
                parse_posix(std::str::from_utf8(&rest[..end]).ok()?)
            })
        } else {
            None
        };
        Some(Tz { transitions, initial, rule })
    }

    /// Seconds to add to a UTC time to get local time.
    pub fn offset_at(&self, utc: i64) -> i64 {
        let n = self.transitions.partition_point(|&(t, _)| t <= utc);
        if n == 0 && !self.transitions.is_empty() {
            return self.initial;
        }
        if n == self.transitions.len()
            && let Some(r) = &self.rule
        {
            return r.offset_at(utc);
        }
        if n == 0 { self.initial } else { self.transitions[n - 1].1 }
    }
}

/// Local wall-clock seconds to UTC (the earlier of two candidates in a repeated hour).
pub fn local_to_utc(local: i64) -> i64 {
    let tz = tz();
    let mut utc = local - tz.offset_at(local);
    // Two refinements settle near transitions.
    for _ in 0..2 {
        utc = local - tz.offset_at(utc);
    }
    utc
}

fn parse_posix(s: &str) -> Option<PosixTz> {
    let b = s.as_bytes();
    let mut i = 0;
    let name = |i: &mut usize| -> Option<()> {
        if b.get(*i) == Some(&b'<') {
            *i += b[*i..].iter().position(|&c| c == b'>')? + 1;
        } else {
            let start = *i;
            while *i < b.len() && b[*i].is_ascii_alphabetic() {
                *i += 1;
            }
            if *i - start < 3 {
                return None;
            }
        }
        Some(())
    };
    // POSIX offsets are west-positive: "EST5" means UTC-5.
    let offset = |i: &mut usize| -> Option<i64> {
        let mut sign = 1;
        if matches!(b.get(*i), Some(b'+' | b'-')) {
            if b[*i] == b'-' {
                sign = -1;
            }
            *i += 1;
        }
        let mut parts = [0i64; 3];
        for (k, p) in parts.iter_mut().enumerate() {
            let start = *i;
            while *i < b.len() && b[*i].is_ascii_digit() {
                *i += 1;
            }
            if start == *i {
                if k == 0 {
                    return None;
                }
                break;
            }
            *p = s[start..*i].parse().ok()?;
            if b.get(*i) == Some(&b':') && k < 2 {
                *i += 1;
            } else {
                break;
            }
        }
        Some(sign * (parts[0] * 3600 + parts[1] * 60 + parts[2]))
    };
    name(&mut i)?;
    let std_west = offset(&mut i)?;
    if i == b.len() {
        return Some(PosixTz { std_off: -std_west, dst: None });
    }
    name(&mut i)?;
    let dst_west = if matches!(b.get(i), Some(c) if c.is_ascii_digit() || *c == b'+' || *c == b'-') { offset(&mut i)? } else { std_west - 3600 };
    let date = |i: &mut usize| -> Option<(RuleDate, i64)> {
        if b.get(*i) != Some(&b',') {
            return None;
        }
        *i += 1;
        let num = |i: &mut usize| -> Option<i64> {
            let start = *i;
            while *i < b.len() && b[*i].is_ascii_digit() {
                *i += 1;
            }
            s[start..*i].parse().ok()
        };
        let d = match b.get(*i)? {
            b'M' => {
                *i += 1;
                let m = num(i)?;
                (b.get(*i) == Some(&b'.')).then_some(())?;
                *i += 1;
                let w = num(i)?;
                (b.get(*i) == Some(&b'.')).then_some(())?;
                *i += 1;
                RuleDate::MonthWeekDay(m, w, num(i)?)
            }
            b'J' => {
                *i += 1;
                RuleDate::Julian1(num(i)?)
            }
            _ => RuleDate::Julian0(num(i)?),
        };
        let t = if b.get(*i) == Some(&b'/') {
            *i += 1;
            offset(i)?
        } else {
            7200
        };
        Some((d, t))
    };
    let (start, st) = date(&mut i)?;
    let (end, et) = date(&mut i)?;
    Some(PosixTz { std_off: -std_west, dst: Some((-dst_west, start, st, end, et)) })
}

impl RuleDate {
    /// Day number (since epoch) in `year`.
    fn day(self, year: i64) -> i64 {
        let jan1 = days_from_civil(year, 1, 1);
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        match self {
            RuleDate::Julian1(n) => jan1 + n - 1 + i64::from(leap && n >= 60),
            RuleDate::Julian0(n) => jan1 + n,
            RuleDate::MonthWeekDay(m, w, d) => {
                let first = days_from_civil(year, m, 1);
                // 1970-01-01 was a Thursday (weekday 4).
                let wd = (first + 4).rem_euclid(7);
                let mut day = first + (d - wd).rem_euclid(7) + (w - 1) * 7;
                let next = if m == 12 { days_from_civil(year + 1, 1, 1) } else { days_from_civil(year, m + 1, 1) };
                while day >= next {
                    day -= 7;
                }
                day
            }
        }
    }
}

impl PosixTz {
    fn offset_at(&self, utc: i64) -> i64 {
        let Some((dst_off, start, st, end, et)) = self.dst else { return self.std_off };
        let year = civil_from_days((utc + self.std_off).div_euclid(86_400)).0;
        // Transition instants in UTC: start is given in standard time, end in DST.
        let s = start.day(year) * 86_400 + st - self.std_off;
        let e = end.day(year) * 86_400 + et - dst_off;
        let in_dst = if s < e { utc >= s && utc < e } else { utc >= s || utc < e };
        if in_dst { dst_off } else { self.std_off }
    }
}

// ---------------------------------------------------------------------------------------
// Owners
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdMatch {
    pub id: u32,
    pub negated: bool,
}

/// `-o user`, `-o :group`, `-o user:group`, ids or names, `!` negates either part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OwnerFilter {
    pub uid: Option<IdMatch>,
    pub gid: Option<IdMatch>,
}

impl OwnerFilter {
    pub fn parse(s: &str) -> Result<OwnerFilter, String> {
        let (u, g) = match s.split_once(':') {
            Some((u, g)) => (u, Some(g)),
            None => (s, None),
        };
        let part = |p: &str, db: &[(String, u32)], what: &str| -> Result<Option<IdMatch>, String> {
            let (negated, p) = match p.strip_prefix('!') {
                Some(r) => (true, r),
                None => (false, p),
            };
            if p.is_empty() {
                return Ok(None);
            }
            let id = match p.parse::<u32>() {
                Ok(n) => n,
                Err(_) => db.iter().find(|(n, _)| n == p).map(|e| e.1).ok_or_else(|| format!("no {what} named '{p}'"))?,
            };
            Ok(Some(IdMatch { id, negated }))
        };
        let f = OwnerFilter { uid: part(u, users(), "user")?, gid: match g { Some(g) => part(g, groups(), "group")?, None => None } };
        if f.uid.is_none() && f.gid.is_none() {
            return Err(format!("'{s}' names no user or group"));
        }
        Ok(f)
    }

    pub fn accepts(&self, uid: u32, gid: u32) -> bool {
        let ok = |m: Option<IdMatch>, v: u32| m.is_none_or(|m| (m.id == v) != m.negated);
        ok(self.uid, uid) && ok(self.gid, gid)
    }
}

fn read_db(path: &str) -> Vec<(String, u32)> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut f = l.split(':');
            let name = f.next()?.to_string();
            f.next()?;
            Some((name, f.next()?.parse().ok()?))
        })
        .collect()
}

pub fn users() -> &'static [(String, u32)] {
    static U: std::sync::OnceLock<Vec<(String, u32)>> = std::sync::OnceLock::new();
    U.get_or_init(|| read_db("/etc/passwd"))
}

pub fn groups() -> &'static [(String, u32)] {
    static G: std::sync::OnceLock<Vec<(String, u32)>> = std::sync::OnceLock::new();
    G.get_or_init(|| read_db("/etc/group"))
}

pub fn user_name(uid: u32) -> String {
    users().iter().find(|e| e.1 == uid).map_or_else(|| uid.to_string(), |e| e.0.clone())
}

pub fn group_name(gid: u32) -> String {
    groups().iter().find(|e| e.1 == gid).map_or_else(|| gid.to_string(), |e| e.0.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(SizeFilter::parse("+1m"), Some(SizeFilter::AtLeast(1_000_000)));
        assert_eq!(SizeFilter::parse("-500k"), Some(SizeFilter::AtMost(500_000)));
        assert_eq!(SizeFilter::parse("10KiB"), Some(SizeFilter::Exactly(10_240)));
        assert_eq!(SizeFilter::parse("3"), Some(SizeFilter::Exactly(3)));
        assert_eq!(SizeFilter::parse("1x"), None);
        assert_eq!(SizeFilter::parse("+k"), None);
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("10h"), Some(Duration::from_secs(36_000)));
        assert_eq!(parse_duration("2weeks"), Some(Duration::from_secs(1_209_600)));
        assert_eq!(parse_duration("1h 30min"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("1h30m"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("1M"), Some(Duration::from_secs(2_630_016)));
        assert_eq!(parse_duration("2018-10-27"), None);
        assert_eq!(parse_duration("h"), None);
    }

    #[test]
    fn dates() {
        let now = SystemTime::now();
        assert_eq!(parse_time("@0", now), Some(UNIX_EPOCH));
        assert_eq!(to_unix(parse_time("2018-10-27T10:00:00Z", now).unwrap()), 1_540_634_400);
        assert_eq!(to_unix(parse_time("2018-10-27T12:00:00+02:00", now).unwrap()), 1_540_634_400);
        assert!(parse_time("2018-10-27", now).is_some());
        assert!(parse_time("2018-10-27 10:00:00", now).is_some());
        assert!(parse_time("2018-13-27", now).is_none());
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn posix_rules() {
        let ny = parse_posix("EST5EDT,M3.2.0,M11.1.0").unwrap();
        // 2024-07-01 12:00 UTC is summer: UTC-4.
        assert_eq!(ny.offset_at(1_719_835_200), -4 * 3600);
        // 2024-01-15 is winter: UTC-5.
        assert_eq!(ny.offset_at(1_705_320_000), -5 * 3600);
        let india = parse_posix("IST-5:30").unwrap();
        assert_eq!(india.offset_at(0), 19_800);
        let syd = parse_posix("AEST-10AEDT,M10.1.0,M4.1.0/3").unwrap();
        assert_eq!(syd.offset_at(1_705_320_000), 11 * 3600);
        assert_eq!(syd.offset_at(1_719_835_200), 10 * 3600);
    }

    #[test]
    fn owners() {
        let f = OwnerFilter::parse("0").unwrap();
        assert!(f.accepts(0, 5));
        assert!(!f.accepts(1, 5));
        let f = OwnerFilter::parse("!0:").unwrap();
        assert!(f.accepts(1, 0));
        let f = OwnerFilter::parse(":!0").unwrap();
        assert!(!f.accepts(1, 0));
        assert!(OwnerFilter::parse("root").is_ok());
        assert!(OwnerFilter::parse("no-such-user-xyz").is_err());
    }
}

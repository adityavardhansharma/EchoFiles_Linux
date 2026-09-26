//! Display formatting, per the design system's content rules: base-1024 sizes with
//! Explorer's labels, and relative dates within a week.

use std::fmt::Write;

/// `4.2 MB`, `84 KB`, `282 B` — base 1024 so numbers match Windows Explorer.
pub fn size(bytes: u64, out: &mut String) {
    const UNITS: [&str; 6] = ["KB", "MB", "GB", "TB", "PB", "EB"];
    if bytes < 1024 {
        let _ = write!(out, "{bytes} B");
        return;
    }
    let mut v = bytes as f64 / 1024.0;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if v < 10.0 {
        let _ = write!(out, "{v:.1} {}", UNITS[u]);
    } else {
        let _ = write!(out, "{v:.0} {}", UNITS[u]);
    }
}

/// `1,284`
pub fn count(n: usize, out: &mut String) {
    let s = n.to_string();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
}

#[derive(Clone, Copy)]
struct Local {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    yday: i32,
}

fn local(secs: i64) -> Option<Local> {
    let t: libc::time_t = secs;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: localtime_r writes into our stack `tm` and reads only `t`.
    let ok = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };
    ok.then(|| Local {
        year: tm.tm_year + 1900,
        month: tm.tm_mon as u32,
        day: tm.tm_mday as u32,
        hour: tm.tm_hour as u32,
        min: tm.tm_min as u32,
        yday: tm.tm_yday,
    })
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Formats dates relative to `now`: `Today 18:22`, `Yesterday`, `14 Sep`, `14 Sep 2025`.
pub struct DateFormatter {
    now: Option<Local>,
    now_secs: i64,
}

impl DateFormatter {
    pub fn new() -> Self {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Self { now: local(now_secs), now_secs }
    }

    pub fn format(&self, secs: i64, out: &mut String) {
        let (Some(now), Some(t)) = (self.now, local(secs)) else {
            return;
        };
        let age = self.now_secs - secs;
        if t.year == now.year && t.yday == now.yday {
            let _ = write!(out, "Today {:02}:{:02}", t.hour, t.min);
        } else if (0..2 * 86_400).contains(&age) && t.year == now.year && now.yday - t.yday == 1 {
            out.push_str("Yesterday");
        } else if t.year == now.year {
            let _ = write!(out, "{} {}", t.day, MONTHS[t.month as usize]);
        } else {
            let _ = write!(out, "{} {} {}", t.day, MONTHS[t.month as usize], t.year);
        }
    }
}

impl Default for DateFormatter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        let f = |b| {
            let mut s = String::new();
            size(b, &mut s);
            s
        };
        assert_eq!(f(282), "282 B");
        assert_eq!(f(86_016), "84 KB");
        assert_eq!(f(4_404_019), "4.2 MB");
        assert_eq!(f(2_576_980_378), "2.4 GB");
    }

    #[test]
    fn counts() {
        let mut s = String::new();
        count(1_284, &mut s);
        assert_eq!(s, "1,284");
    }
}

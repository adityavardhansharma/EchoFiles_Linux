//! Corpus generator for index tests and benchmarks (build plan §2.9).
//!
//! Names are deliberately hard for search: families of near-identical names ("report",
//! "Report Final FINAL", "reportage", "rep ort"), accents in NFC and NFD, `ß`, CJK,
//! Devanagari, emoji (including ZWJ sequences), fullwidth letters, shell metacharacters,
//! quotes, tabs and newlines, bytes that are not UTF-8, 250-byte names, and dotfiles.
//! Every name carries its number, so `n` files always means `n` distinct entries.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    /// Every file directly in the root folder.
    Flat,
    /// Three levels of folders, ~100 files per leaf folder.
    Tree,
}

const REPORTS: &[&str] = &[
    "report",
    "Report",
    "REPORT",
    "report_final",
    "report-final-v2",
    "Report Final FINAL",
    "reports",
    "reportage",
    "2026-report",
    "rep ort",
    "annual report",
    "Q3 Report (copy)",
];

const UNICODE: &[&str] = &[
    "Résumé",
    "Re\u{301}sume\u{301}",
    "Straße",
    "日本語メモ",
    "🎉 party",
    "Ｒｅｐｏｒｔ",
    "ΣΊΣΥΦΟΣ",
    "naïve café",
    "हिंदी नोट्स",
    "emoji 👨\u{200d}👩\u{200d}👧",
    "İstanbul",
];

const SPECIAL: &[&str] = &[
    "[draft] (v2) {braces}",
    "it's \"quoted\"",
    "$HOME & more",
    "back\\slash",
    "star*q?",
    "-dash",
    "#hash %",
    "semi;colon:",
    "tab\there",
    "new\nline",
    "  spaces  ",
];

const EXTS: &[&str] = &["pdf", "PDF", "txt", "jpg", "docx", "md", "tar.gz", "rs", ""];

const TOP: &[&str] = &["Documents", "Photos 2026", "Projects", "Archive (old)", "Ünïcödé Ordner", ".cache", "reports", "Downloads"];

/// Name of file `i`, as raw bytes (some are not UTF-8).
pub fn file_name(i: usize) -> Vec<u8> {
    let r = mix(i as u64);
    let ext = EXTS[(r >> 8) as usize % EXTS.len()];
    let dot = |stem: String| if ext.is_empty() { stem } else { format!("{stem}.{ext}") };
    let name = match r % 16 {
        0..=3 => dot(format!("{} {i}", REPORTS[(r >> 16) as usize % REPORTS.len()])),
        4 => format!("IMG_2026{:02}{:02}_{i:06}.jpg", 1 + i % 12, 1 + i % 28),
        5 => format!("Screenshot 2026-05-01 at 10.{i}.png"),
        6 => format!("DSC_{i:05}.JPG"),
        7 => format!("invoice-{i}.pdf"),
        8 => format!("notes {i}.md"),
        9 => format!("budget_2026_{i}.xlsx"),
        10 => format!("main_{i}.rs"),
        11 => format!("index-{i}.ts"),
        12 | 13 => dot(format!("{} {i}", UNICODE[(r >> 16) as usize % UNICODE.len()])),
        14 => {
            if (r >> 16).is_multiple_of(8) {
                let mut b = format!("bad{i} ").into_bytes();
                b.extend_from_slice(b"\xff\xfe latin1 caf\xe9.txt");
                return b;
            }
            dot(format!("{} {i}", SPECIAL[(r >> 16) as usize % SPECIAL.len()]))
        }
        _ => {
            if (r >> 16).is_multiple_of(2) {
                format!(".config-{i}")
            } else {
                format!(".report-{i}.pdf")
            }
        }
    };
    if i % 1000 == 999 {
        // Near the 255-byte NAME_MAX.
        let mut long = format!("report long {i} ");
        long.extend(std::iter::repeat_n('x', 240 - long.len()));
        return format!("{long}.txt").into_bytes();
    }
    name.into_bytes()
}

/// Folder (relative to the root) that file `i` goes into.
pub fn folder_of(i: usize, layout: Layout) -> PathBuf {
    match layout {
        Layout::Flat => PathBuf::new(),
        Layout::Tree => {
            let leaf = i / 100;
            let mut p = PathBuf::from(TOP[leaf % TOP.len()]);
            let mid = leaf / TOP.len() % 10;
            p.push(if mid.is_multiple_of(3) { format!("reports {mid}") } else { format!("part {mid}") });
            p.push(format!("batch {leaf}"));
            p
        }
    }
}

/// Fill `root` (created if missing) with `files` files. Returns the number of folders made.
pub fn generate(root: &Path, files: usize, layout: Layout) -> io::Result<usize> {
    fs::create_dir_all(root)?;
    let mut made = std::collections::HashSet::new();
    for i in 0..files {
        let dir = root.join(folder_of(i, layout));
        if made.insert(dir.clone()) {
            fs::create_dir_all(&dir)?;
        }
        fs::File::create(dir.join(OsStr::from_bytes(&file_name(i))))?;
    }
    Ok(made.len())
}

/// A corpus under `~/.cache/echofiles-bench/index/`, generated on first use.
pub fn cached(files: usize, layout: Layout) -> PathBuf {
    let home = std::env::var_os("HOME").expect("HOME");
    let tag = match layout {
        Layout::Flat => "flat",
        Layout::Tree => "tree",
    };
    let root = PathBuf::from(home).join(format!(".cache/echofiles-bench/index/{tag}-{files}"));
    let done = root.with_extension("done");
    if !done.exists() {
        let _ = fs::remove_dir_all(&root);
        generate(&root, files, layout).expect("generate corpus");
        fs::write(&done, b"").expect("mark corpus");
    }
    root
}

/// SplitMix64: a fixed, well-spread choice per file number.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

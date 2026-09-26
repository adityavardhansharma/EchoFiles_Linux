//! Benchmark corpus generator (build plan §2.9).
//!
//! `mkcorpus <dir> <count>` fills `dir` with `count` entries: realistic camera, document
//! and code names, ~5% folders, unicode names, and a sprinkle of dotfiles.

use std::fs;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("usage: mkcorpus <dir> <count>"));
    let count: usize = args.next().and_then(|s| s.parse().ok()).expect("usage: mkcorpus <dir> <count>");
    fs::create_dir_all(&dir).expect("create corpus dir");
    let stems = ["Report", "Invoice", "notes", "Screenshot", "DSC", "track", "main", "Übersicht", "日本語メモ", "draft"];
    let exts = ["jpg", "pdf", "txt", "xlsx", "rs", "mp4", "flac", "7z", "md", "HEIC"];
    for i in 0..count {
        let name = match i % 20 {
            0 => format!("Folder {i}"),
            1 => format!(".cache-{i}"),
            2..=7 => format!("IMG_2026{:02}{:02}_{:06}.jpg", 1 + i % 12, 1 + i % 28, i),
            _ => format!("{} {} ({}).{}", stems[i % stems.len()], i / 7, i % 5, exts[i % exts.len()]),
        };
        let p = dir.join(&name);
        if i % 20 == 0 {
            fs::create_dir_all(&p).expect("mkdir");
        } else {
            fs::write(&p, vec![b'x'; (i * 37) % 4096]).expect("write");
        }
    }
    println!("{count} entries in {}", dir.display());
}

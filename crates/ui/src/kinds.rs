//! File-type presentation: colour icon and human label by extension (extension only — no
//! content sniffing on the listing path, build plan §2.2).

use ef_core::Kind;

/// (extension, colour icon, label)
const TYPES: &[(&str, &str, &str)] = &[
    ("jpg", "file-image", "JPEG image"),
    ("jpeg", "file-image", "JPEG image"),
    ("png", "file-image", "PNG image"),
    ("webp", "file-image", "WebP image"),
    ("heic", "file-image", "HEIC image"),
    ("gif", "file-image", "GIF image"),
    ("avif", "file-image", "AVIF image"),
    ("svg", "file-vector", "SVG image"),
    ("mp4", "file-video", "MP4 video"),
    ("mkv", "file-video", "Matroska video"),
    ("mov", "file-video", "QuickTime video"),
    ("webm", "file-video", "WebM video"),
    ("mp3", "file-audio", "MP3 audio"),
    ("flac", "file-audio", "FLAC audio"),
    ("ogg", "file-audio", "Ogg audio"),
    ("wav", "file-audio", "WAV audio"),
    ("m4a", "file-audio", "AAC audio"),
    ("pdf", "file-pdf", "PDF"),
    ("doc", "file-doc", "Word document"),
    ("docx", "file-doc", "Word document"),
    ("odt", "file-doc", "Text document"),
    ("md", "file-text", "Markdown"),
    ("txt", "file-lines", "Text"),
    ("xls", "file-sheet", "Spreadsheet"),
    ("xlsx", "file-sheet", "Spreadsheet"),
    ("csv", "file-sheet", "CSV"),
    ("ods", "file-sheet", "Spreadsheet"),
    ("ppt", "file-slides", "Presentation"),
    ("pptx", "file-slides", "Presentation"),
    ("odp", "file-slides", "Presentation"),
    ("rs", "file-code", "Rust source"),
    ("js", "file-code", "JavaScript"),
    ("ts", "file-code", "TypeScript"),
    ("py", "file-code", "Python"),
    ("c", "file-code", "C source"),
    ("h", "file-code", "C header"),
    ("html", "file-code", "HTML"),
    ("json", "file-code", "JSON"),
    ("toml", "file-config", "TOML"),
    ("ini", "file-config", "Settings"),
    ("conf", "file-config", "Config"),
    ("yaml", "file-config", "YAML"),
    ("yml", "file-config", "YAML"),
    ("sh", "file-script", "Shell script"),
    ("ps1", "file-script", "PowerShell script"),
    ("bat", "file-script", "Batch file"),
    ("exe", "file-script", "Windows program"),
    ("zip", "file-archive", "ZIP archive"),
    ("7z", "file-archive", "7-Zip archive"),
    ("tar", "file-archive-alt", "Tar archive"),
    ("gz", "file-archive-alt", "Gzip archive"),
    ("zst", "file-archive-alt", "Zstd archive"),
    ("xz", "file-archive-alt", "XZ archive"),
    ("rar", "file-package", "RAR archive"),
    ("ttf", "file-font", "Font"),
    ("otf", "file-font", "Font"),
    ("woff2", "file-font", "Font"),
    ("lnk", "file", "Shortcut"),
    ("iso", "drive", "Disc image"),
];

pub fn extension(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&c| c == b'.') {
        Some(p) if p > 0 => &name[p + 1..],
        _ => b"",
    }
}

fn lookup(name: &[u8]) -> Option<&'static (&'static str, &'static str, &'static str)> {
    let ext = extension(name);
    if ext.is_empty() || ext.len() > 5 {
        return None;
    }
    TYPES.iter().find(|(e, _, _)| e.as_bytes().eq_ignore_ascii_case(ext))
}

pub fn icon(name: &[u8], kind: Kind) -> &'static str {
    match kind {
        Kind::Dir => "folder",
        Kind::Symlink => "file",
        _ => lookup(name).map_or("file", |t| t.1),
    }
}

pub fn label(name: &[u8], kind: Kind, out: &mut String) {
    match kind {
        Kind::Dir => out.push_str("Folder"),
        Kind::Symlink => out.push_str("Link"),
        Kind::Other => out.push_str("Special file"),
        _ => match lookup(name) {
            Some(t) => out.push_str(t.2),
            None => {
                let ext = extension(name);
                if ext.is_empty() || ext.len() > 8 {
                    out.push_str("File");
                } else {
                    out.push_str(&String::from_utf8_lossy(ext).to_uppercase());
                    out.push_str(" file");
                }
            }
        },
    }
}

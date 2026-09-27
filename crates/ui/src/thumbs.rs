//! Thumbnails for the grid and the preview pane (build plan §2.6, design system `FileGrid`).
//!
//! Lookup order: memory → the freedesktop cache shared with other apps
//! (`~/.cache/thumbnails/normal/<md5 of the URI>.png`, checked against `Thumb::MTime`) →
//! an in-process decode for JPEG/PNG/WebP/GIF/BMP → the system `.thumbnailer`s for video,
//! PDF, office files and the rest. New thumbnails are written back to the shared cache.
//! At most two decodes run at once, on their own threads.

use std::collections::{HashMap, VecDeque};
use std::io::BufWriter;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use iced::advanced::image::Handle;

const SIZE: u32 = 128;
const MAX_JOBS: usize = 2;
const KEEP: usize = 1500;

pub enum Slot {
    Pending,
    Ready(Handle),
    Failed,
}

#[derive(Default)]
pub struct Thumbs {
    /// Path → (the file's mtime when made, thumbnail).
    map: HashMap<PathBuf, (i64, Slot)>,
    lru: VecDeque<PathBuf>,
    queue: VecDeque<(PathBuf, i64)>,
    running: usize,
}

/// A decoded thumbnail, ready to become an image handle on the UI thread.
#[derive(Debug, Clone)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: std::sync::Arc<Vec<u8>>,
}

impl Thumbs {
    pub fn get(&self, path: &Path) -> Option<Handle> {
        match self.map.get(path) {
            Some((_, Slot::Ready(h))) => Some(h.clone()),
            _ => None,
        }
    }

    /// Queue these (visible) files, newest request first. Returns jobs to start.
    pub fn want(&mut self, items: Vec<(PathBuf, i64)>) -> Vec<(PathBuf, i64)> {
        // What's on screen now matters more than what was on screen before.
        self.queue.clear();
        for (p, m) in items {
            // A changed file (new mtime) gets a new thumbnail.
            let fresh = self.map.get(&p).is_some_and(|(t, _)| *t == m);
            if !fresh && thumbnailable(&p) {
                self.queue.push_back((p, m));
            }
        }
        self.start()
    }

    fn start(&mut self) -> Vec<(PathBuf, i64)> {
        let mut jobs = Vec::new();
        while self.running < MAX_JOBS {
            let Some((p, m)) = self.queue.pop_front() else { break };
            self.map.insert(p.clone(), (m, Slot::Pending));
            self.running += 1;
            jobs.push((p, m));
        }
        jobs
    }

    /// A job finished; returns the next jobs to start.
    pub fn done(&mut self, path: PathBuf, px: Option<Pixels>) -> Vec<(PathBuf, i64)> {
        self.running = self.running.saturating_sub(1);
        let slot = match px {
            Some(px) => Slot::Ready(Handle::from_rgba(px.width, px.height, (*px.rgba).clone())),
            None => Slot::Failed,
        };
        let mtime = self.map.get(&path).map_or(0, |e| e.0);
        self.map.insert(path.clone(), (mtime, slot));
        self.lru.push_back(path);
        while self.lru.len() > KEEP {
            if let Some(old) = self.lru.pop_front() {
                self.map.remove(&old);
            }
        }
        self.start()
    }
}

const DECODE: [&str; 7] = ["jpg", "jpeg", "png", "webp", "gif", "bmp", "jfif"];

fn ext(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

/// MIME types by extension, for picking a system thumbnailer.
fn mime(e: &str) -> Option<&'static str> {
    Some(match e {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/vnd.avi",
        "wmv" => "video/x-ms-wmv",
        "mpg" | "mpeg" => "video/mpeg",
        "ts" | "m2ts" => "video/mp2t",
        "3gp" => "video/3gpp",
        "flv" => "video/x-flv",
        "pdf" => "application/pdf",
        "epub" => "application/epub+zip",
        "cbz" => "application/vnd.comicbook+zip",
        "heic" | "heif" => "image/heif",
        "avif" => "image/avif",
        "jxl" => "image/jxl",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        _ => return None,
    })
}

pub fn thumbnailable(p: &Path) -> bool {
    let e = ext(p);
    DECODE.contains(&e.as_str()) || mime(&e).is_some_and(|m| thumbnailer_for(m).is_some())
}

/// `file://` URI as the thumbnail spec hashes it.
fn uri(p: &Path) -> String {
    let mut out = String::from("file://");
    for &b in p.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/!$&'()*+,;=:@".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| ef_config::home().join(".cache"))
        .join("thumbnails")
}

/// (MIME types, Exec line) of each installed thumbnailer.
fn thumbnailers() -> &'static Vec<(Vec<String>, String)> {
    static T: OnceLock<Vec<(Vec<String>, String)>> = OnceLock::new();
    T.get_or_init(|| {
        let mut out = Vec::new();
        for dir in ["/usr/share/thumbnailers", "/usr/local/share/thumbnailers"] {
            let Ok(rd) = std::fs::read_dir(dir) else { continue };
            for e in rd.flatten() {
                let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
                let mut exec = None;
                let mut mimes = Vec::new();
                let mut try_exec_ok = true;
                for line in text.lines() {
                    if let Some(v) = line.strip_prefix("Exec=") {
                        exec = Some(v.to_string());
                    } else if let Some(v) = line.strip_prefix("MimeType=") {
                        mimes = v.split(';').filter(|s| !s.is_empty()).map(String::from).collect();
                    } else if let Some(v) = line.strip_prefix("TryExec=") {
                        try_exec_ok = which(v);
                    }
                }
                if let (Some(x), true) = (exec, try_exec_ok) {
                    out.push((mimes, x));
                }
            }
        }
        out
    })
}

fn which(prog: &str) -> bool {
    if prog.starts_with('/') {
        return Path::new(prog).exists();
    }
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(prog).exists()))
}

fn thumbnailer_for(mime: &str) -> Option<&'static str> {
    thumbnailers().iter().find(|(m, _)| m.iter().any(|x| x == mime)).map(|(_, e)| e.as_str())
}

/// Read a cached PNG if it's for this version of the file.
fn read_cached(file: &Path, mtime: i64) -> Option<Pixels> {
    let f = std::fs::File::open(file).ok()?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(f));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let fresh = reader.info().uncompressed_latin1_text.iter().any(|t| t.keyword == "Thumb::MTime" && t.text.trim().parse::<i64>().ok() == Some(mtime));
    if !fresh {
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buf[..info.buffer_size()].chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf[..info.buffer_size()].chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => buf[..info.buffer_size()].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Pixels { width: info.width, height: info.height, rgba: std::sync::Arc::new(rgba) })
}

fn write_cached(file: &Path, px: &Pixels, uri: &str, mtime: i64) {
    let Some(dir) = file.parent() else { return };
    let _ = std::fs::create_dir_all(dir);
    let tmp = file.with_extension(format!("tmp{}", std::process::id()));
    let Ok(f) = std::fs::File::create(&tmp) else { return };
    let mut enc = png::Encoder::new(BufWriter::new(f), px.width, px.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let _ = enc.add_text_chunk("Thumb::URI".into(), uri.into());
    let _ = enc.add_text_chunk("Thumb::MTime".into(), mtime.to_string());
    let ok = enc.write_header().and_then(|mut w| w.write_image_data(&px.rgba)).is_ok();
    if ok {
        let _ = std::fs::rename(&tmp, file);
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
}

fn decode(p: &Path) -> Option<Pixels> {
    let mut reader = image::ImageReader::open(p).ok()?.with_guessed_format().ok()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(512 << 20);
    reader.limits(limits);
    let img = reader.decode().ok()?;
    let t = img.thumbnail(SIZE, SIZE).to_rgba8();
    Some(Pixels { width: t.width(), height: t.height(), rgba: std::sync::Arc::new(t.into_raw()) })
}

fn run_thumbnailer(p: &Path, uri: &str) -> Option<Pixels> {
    let exec = thumbnailer_for(mime(&ext(p))?)?;
    let out = std::env::temp_dir().join(format!("ef-thumb-{}-{}.png", std::process::id(), md5::compute(uri.as_bytes()).0[0]));
    let args: Vec<String> = exec
        .split_whitespace()
        .map(|a| a.replace("%s", &SIZE.to_string()).replace("%u", uri).replace("%i", &p.to_string_lossy()).replace("%o", &out.to_string_lossy()))
        .collect();
    let (prog, rest) = args.split_first()?;
    let status = Command::new(prog).args(rest).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().ok()?;
    let img = status.success().then(|| image::open(&out).ok()).flatten();
    let _ = std::fs::remove_file(&out);
    let t = img?.thumbnail(SIZE, SIZE).to_rgba8();
    Some(Pixels { width: t.width(), height: t.height(), rgba: std::sync::Arc::new(t.into_raw()) })
}

/// Produce a thumbnail for `p` (blocking; run on a worker thread).
pub fn load(p: &Path, mtime: i64) -> Option<Pixels> {
    let u = uri(p);
    let name = format!("{:x}.png", md5::compute(u.as_bytes()));
    let base = cache_dir();
    for size in ["normal", "large", "x-large"] {
        if let Some(px) = read_cached(&base.join(size).join(&name), mtime) {
            return Some(px);
        }
    }
    // A failure recorded by another app (or us) for this version: don't retry.
    let fail = base.join("fail/echofiles").join(&name);
    if std::fs::read_to_string(&fail).is_ok_and(|m| m.trim().parse::<i64>().ok() == Some(mtime)) {
        return None;
    }
    let px = if DECODE.contains(&ext(p).as_str()) { decode(p) } else { run_thumbnailer(p, &u) };
    match &px {
        Some(px) => write_cached(&base.join("normal").join(&name), px, &u, mtime),
        None => {
            let _ = std::fs::create_dir_all(base.join("fail/echofiles"));
            let _ = std::fs::write(&fail, mtime.to_string());
        }
    }
    px
}

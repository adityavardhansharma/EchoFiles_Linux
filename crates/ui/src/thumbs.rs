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
use std::sync::{OnceLock, Arc, atomic::{AtomicBool, Ordering}};
pub type Job = (PathBuf, i64, Arc<AtomicBool>);

use iced::advanced::image::Handle;

const SIZE: u32 = 128;
const MAX_JOBS: usize = 2;
// Every cached handle is at most SIZE² RGBA bytes (32 MiB at this entry budget).
const KEEP: usize = 512;

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
    cancels: HashMap<PathBuf, Arc<AtomicBool>>,
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
    pub fn want(&mut self, items: Vec<(PathBuf, i64)>) -> Vec<Job> {
        // What's on screen now matters more than what was on screen before.
        self.queue.clear();
        for (path, cancel) in &self.cancels {
            if !items.iter().any(|(p, m)| p == path && self.map.get(path).is_some_and(|(old, _)| old == m)) { cancel.store(true, Ordering::Relaxed); }
        }
        for (p, m) in items {
            // A changed file (new mtime) gets a new thumbnail.
            let fresh = self.map.get(&p).is_some_and(|(t, _)| *t == m);
            // Like Nautilus: no thumbnails for network files (each one reads the whole file).
            if !fresh && thumbnailable(&p) && !ef_net::gvfs::is_network_path(&p) {
                self.queue.push_back((p, m));
            }
        }
        self.start()
    }

    fn start(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        while self.running < MAX_JOBS {
            let Some((p, m)) = self.queue.pop_front() else { break };
            self.map.insert(p.clone(), (m, Slot::Pending));
            self.running += 1;
            let cancel = Arc::new(AtomicBool::new(false));
            self.cancels.insert(p.clone(), cancel.clone());
            jobs.push((p, m, cancel));
        }
        jobs
    }

    /// A job finished; returns the next jobs to start.
    pub fn done(&mut self, path: PathBuf, mtime: i64, token: &Arc<AtomicBool>, px: Option<Pixels>) -> Vec<Job> {
        self.running = self.running.saturating_sub(1);
        if !self.cancels.get(&path).is_some_and(|c| Arc::ptr_eq(c, token)) { return self.start(); }
        self.cancels.remove(&path);
        if token.load(Ordering::Relaxed) { self.map.remove(&path); return self.start(); }
        let slot = match px {
            Some(px) => Slot::Ready(Handle::from_rgba(px.width, px.height, (*px.rgba).clone())),
            None => Slot::Failed,
        };
        self.map.insert(path.clone(), (mtime, slot));
        self.lru.retain(|p| p != &path);
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
    if f.metadata().ok()?.len() > 8 << 20 { return None; }
    let mut dec = png::Decoder::new(std::io::BufReader::new(f));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    if reader.info().width > SIZE || reader.info().height > SIZE { return None; }
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
    let Ok(tmp) = tempfile::NamedTempFile::new_in(dir) else { return };
    let Ok(f) = tmp.reopen() else { return };
    let mut enc = png::Encoder::new(BufWriter::new(f), px.width, px.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let _ = enc.add_text_chunk("Thumb::URI".into(), uri.into());
    let _ = enc.add_text_chunk("Thumb::MTime".into(), mtime.to_string());
    let ok = enc.write_header().and_then(|mut w| w.write_image_data(&px.rgba)).is_ok();
    if ok {
        let _ = tmp.persist(file);
    } else {
        drop(tmp);
    }
}

fn decode(p: &Path) -> Option<Pixels> {
    let mut reader = image::ImageReader::open(p).ok()?.with_guessed_format().ok()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 << 20);
    reader.limits(limits);
    let img = reader.decode().ok()?;
    let t = img.thumbnail(SIZE, SIZE).to_rgba8();
    Some(Pixels { width: t.width(), height: t.height(), rgba: std::sync::Arc::new(t.into_raw()) })
}

fn run_thumbnailer(p: &Path, uri: &str, cancel: &AtomicBool) -> Option<Pixels> {
    let exec = thumbnailer_for(mime(&ext(p))?)?;
    let work = tempfile::Builder::new().prefix("ef-thumb-").tempdir().ok()?;
    let out = work.path().join("result.png");
    let args: Vec<String> = parse_exec(exec)?.into_iter()
        .map(|a| expand_codes(&a, p, uri, &out)).collect();
    let (prog, rest) = args.split_first()?;
    let mut child = Command::new(prog).args(rest).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success().then(|| decode(&out)).flatten(),
            Err(_) => { let _ = child.kill(); let _ = child.wait(); return None; },
            Ok(None) => {},
        }
        if cancel.load(Ordering::Relaxed) || std::time::Instant::now() >= deadline { let _ = child.kill(); let _ = child.wait(); return None; }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Produce a thumbnail for `p` (blocking; run on a worker thread).
pub fn load_cancellable(p: &Path, mtime: i64, cancel: &AtomicBool) -> Option<Pixels> {
    if cancel.load(Ordering::Relaxed) { return None; }
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
    let px = if DECODE.contains(&ext(p).as_str()) { decode(p) } else { run_thumbnailer(p, &u, cancel) };
    if cancel.load(Ordering::Relaxed) { return None; }
    match &px {
        Some(px) => write_cached(&base.join("normal").join(&name), px, &u, mtime),
        None => {
            let _ = std::fs::create_dir_all(base.join("fail/echofiles"));
            let _ = std::fs::write(&fail, mtime.to_string());
        }
    }
    px
}

fn parse_exec(line: &str) -> Option<Vec<String>> {
    let mut args = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut started = false;
    for c in line.chars() {
        if escaped { word.push(c); escaped = false; started = true; continue; }
        match c {
            '\\' => { escaped = true; started = true; },
            '\"' => { quoted = !quoted; started = true; },
            c if c.is_whitespace() && !quoted => { if started { args.push(std::mem::take(&mut word)); started = false; } },
            c => { word.push(c); started = true; },
        }
    }
    if quoted || escaped { return None; }
    if started { args.push(word); }
    Some(args)
}

fn expand_codes(arg: &str, path: &Path, uri: &str, out: &Path) -> String {
    let mut result = String::new();
    let mut chars = arg.chars();
    while let Some(c) = chars.next() {
        if c != '%' { result.push(c); continue; }
        match chars.next() {
            Some('s') => result.push_str(&SIZE.to_string()), Some('u') => result.push_str(uri),
            Some('i') => result.push_str(&path.to_string_lossy()), Some('o') => result.push_str(&out.to_string_lossy()),
            Some('%') => result.push('%'), Some(c) => { result.push('%'); result.push(c); }, None => result.push('%'),
        }
    }
    result
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn quoted_helpers_and_field_codes() {
        assert_eq!(parse_exec("\"/opt/My Helper/bin\" --name \"a b\" %i %o").unwrap(), vec!["/opt/My Helper/bin", "--name", "a b", "%i", "%o"]);
        assert!(parse_exec("\"unterminated").is_none());
        assert_eq!(expand_codes("%i", Path::new("/tmp/literal%o"), "unused", Path::new("output")), "/tmp/literal%o");
        assert_eq!(expand_codes("%%", Path::new(""), "", Path::new("")), "%");
    }

    #[test]
    fn older_thumbnail_cannot_complete_a_newer_request() {
        let mut thumbs = Thumbs::default();
        let path = PathBuf::from("/fixture/photo.jpg");
        let first = thumbs.want(vec![(path.clone(), 1)]).pop().unwrap();
        let second = thumbs.want(vec![(path.clone(), 2)]).pop().unwrap();
        assert!(first.2.load(Ordering::Relaxed));
        let pixels = || Some(Pixels { width: 1, height: 1, rgba: Arc::new(vec![0; 4]) });
        thumbs.done(second.0, second.1, &second.2, pixels());
        thumbs.done(first.0, first.1, &first.2, None);
        assert!(thumbs.get(&path).is_some());
        assert_eq!(thumbs.map[&path].0, 2);
        assert_eq!(thumbs.running, 0);
    }
}

//! The phone's photos: the lock-screen picture made from the newest one's thumbnail
//! (EchoConnect sends the list and thumbnails as packets) and the keys that remember
//! which were imported.

use super::*;

pub(super) fn thumb_cache(path: &Path, mtime: i64, tag: &str) -> PathBuf {
    let key = crate::codec::md5_hex(format!("{}\0{mtime}\0{tag}", path.display()).as_bytes());
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home().join(".cache"))
        .join("echofiles/phone-thumbs")
        .join(format!("{key}.png"))
}

/// The thumbnail JPEG inside a JPEG's EXIF block, and the photo's orientation — read from
/// the first bytes only, so a thumbnail costs ~64 KB over Wi-Fi instead of the whole photo.
pub(super) fn exif_thumbnail(head: &[u8]) -> Option<(&[u8], u16)> {
    if head.len() < 4 || head[0] != 0xFF || head[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 4 <= head.len() {
        if head[i] != 0xFF {
            return None;
        }
        let marker = head[i + 1];
        let len = u16::from_be_bytes([head[i + 2], head[i + 3]]) as usize;
        if marker == 0xDA {
            return None;
        }
        if marker == 0xE1 && head.get(i + 4..i + 10) == Some(b"Exif\0\0") {
            let tiff = head.get(i + 10..(i + 2 + len).min(head.len()))?;
            return parse_tiff_thumb(tiff);
        }
        i += 2 + len;
    }
    None
}

pub(super) fn parse_tiff_thumb(t: &[u8]) -> Option<(&[u8], u16)> {
    let le = match t.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |o: usize| -> Option<u16> {
        let b = t.get(o..o + 2)?;
        Some(if le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })
    };
    let u32_at = |o: usize| -> Option<u32> {
        let b = t.get(o..o + 4)?;
        Some(if le { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) } else { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) })
    };
    let ifd0 = u32_at(4)? as usize;
    let n0 = u16_at(ifd0)? as usize;
    let mut orientation = 1;
    for k in 0..n0 {
        let e = ifd0 + 2 + 12 * k;
        if u16_at(e)? == 0x0112 {
            orientation = u16_at(e + 8)?;
        }
    }
    let ifd1 = u32_at(ifd0 + 2 + 12 * n0)? as usize;
    if ifd1 == 0 {
        return None;
    }
    let n1 = u16_at(ifd1)? as usize;
    let (mut off, mut len) = (0usize, 0usize);
    for k in 0..n1 {
        let e = ifd1 + 2 + 12 * k;
        match u16_at(e)? {
            0x0201 => off = u32_at(e + 8)? as usize,
            0x0202 => len = u32_at(e + 8)? as usize,
            _ => {}
        }
    }
    let jpeg = t.get(off..off.checked_add(len)?)?;
    (len > 0 && jpeg.starts_with(&[0xFF, 0xD8])).then_some((jpeg, orientation))
}

pub(super) fn orient(img: image::DynamicImage, o: u16) -> image::DynamicImage {
    match o {
        3 => img.rotate180(),
        6 => img.rotate90(),
        8 => img.rotate270(),
        _ => img,
    }
}

/// A photo (or its thumbnail), sized for the drawn phone's screen, as a base64 JPEG.
pub(super) fn screen_image(path: &Path, mtime: i64) -> Option<String> {
    let cache = thumb_cache(path, mtime, "screen").with_extension("jpg");
    let bytes = match read_image_limited(&cache) {
        Some(b) => b,
        None => {
            let data = read_image_limited(path)?;
            let o = exif_thumbnail(&data).map_or(1, |(_, o)| o);
            let img = orient(decode_limited(&data)?, o).resize_to_fill(300, 620, image::imageops::FilterType::Triangle);
            let mut out = Vec::new();
            img.to_rgb8().write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Jpeg).ok()?;
            if let Some(d) = cache.parent() {
                let _ = std::fs::create_dir_all(d);
                let _ = config::atomic_write(&cache, &out);
                trim_phone_cache(d);
            }
            out
        }
    };
    Some(crate::codec::base64_encode(&bytes))
}

pub(super) fn import_key(id: &str, root: Option<&Path>, p: &Photo) -> String {
    use std::os::unix::ffi::OsStrExt;
    let relative = root.and_then(|root| p.path.strip_prefix(root).ok()).unwrap_or(&p.path);
    use std::fmt::Write;
    let bytes = relative.as_os_str().as_bytes();
    let mut key = String::with_capacity(id.len() + 2 * bytes.len() + 24);
    key.push_str(id);
    key.push(':');
    for b in bytes {
        let _ = write!(key, "{b:02x}");
    }
    let _ = write!(key, ":{}:{}", p.size, p.mtime);
    key
}

pub(super) fn month_of(secs: i64) -> String {
    // Civil date from days since the epoch (Howard Hinnant's algorithm).
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}")
}

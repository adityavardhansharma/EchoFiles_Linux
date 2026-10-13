//! Fast JPEG thumbnails: decode only each 8×8 block's DC coefficient.
//!
//! The DC coefficient of a block is its average colour, so the DC values alone are the
//! image at 1/8 scale (what libjpeg calls `scale_denom = 8`). That skips the inverse DCT,
//! upsampling and colour conversion of 63 of every 64 pixels. Baseline files still have
//! their AC coefficients Huffman-decoded (to find where the next block starts) but not
//! used; progressive files are read only up to the end of their DC scans.
//!
//! Handles baseline and extended (8-bit) Huffman and progressive JPEGs, any sampling
//! factors, restart intervals, grayscale, YCbCr and Adobe RGB. Anything else (arithmetic
//! coding, lossless, CMYK, 12-bit) returns `None` and the caller uses the full decoder.

/// An RGB image at 1/8 of the JPEG's size (rounded up), 3 bytes per pixel.
pub struct Small {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

/// Bits looked up at once when skipping AC coefficients.
const AC_BITS: u32 = 11;

struct Huff {
    /// (code length, symbol) for codes up to 9 bits, indexed by the next 9 bits.
    fast: Vec<(u8, u8)>,
    /// For AC skipping, indexed by the next `AC_BITS` bits: code and magnitude bits
    /// together (low 5 bits), coefficients advanced (next 5), end of block (bit 10);
    /// 0 when the pair doesn't fit.
    ac: Vec<u16>,
    maxcode: [i32; 18],
    valptr: [i32; 17],
    mincode: [i32; 17],
    vals: Vec<u8>,
}

impl Huff {
    fn new(counts: &[u8; 16], vals: Vec<u8>) -> Option<Huff> {
        let mut h = Huff { fast: vec![(0, 0); 512], ac: vec![0; 1 << AC_BITS], maxcode: [-1; 18], valptr: [0; 17], mincode: [0; 17], vals };
        let mut code: i32 = 0;
        let mut k: i32 = 0;
        for len in 1..=16usize {
            let n = counts[len - 1] as i32;
            h.valptr[len] = k;
            h.mincode[len] = code;
            for i in 0..n {
                let sym = *h.vals.get((k + i) as usize)?;
                let (r, sz) = ((sym >> 4) as u32, (sym & 15) as u32);
                let total = len as u32 + sz;
                if total <= AC_BITS {
                    let entry = if sz == 0 && r == 0 { total as u16 | 1 << 10 } else { total as u16 | ((if sz == 0 { 16 } else { r + 1 }) as u16) << 5 };
                    let c = ((code + i) as u32) << (AC_BITS - len as u32);
                    let span = 1u32 << (AC_BITS - len as u32);
                    for f in 0..span {
                        h.ac[(c + f) as usize] = entry;
                    }
                }
                if len <= 9 {
                    let c = (code + i) << (9 - len);
                    for f in 0..(1 << (9 - len)) {
                        *h.fast.get_mut((c + f) as usize)? = (len as u8, *h.vals.get((k + i) as usize)?);
                    }
                }
            }
            code += n;
            k += n;
            h.maxcode[len] = if n > 0 { code - 1 } else { -1 };
            if code > (1 << len) {
                return None;
            }
            code <<= 1;
        }
        h.maxcode[17] = i32::MAX;
        Some(h)
    }
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    acc: u64,
    n: u32,
    /// A marker was reached; only zero bits follow.
    marker: bool,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8], pos: usize) -> Bits<'a> {
        Bits { data, pos, acc: 0, n: 0, marker: false }
    }

    fn fill(&mut self) {
        // Fast path: the next 8 bytes hold no 0xFF (so no stuffing or marker): take as
        // many whole bytes as fit at once.
        if !self.marker && self.pos + 8 <= self.data.len() {
            let w = u64::from_be_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
            let x = !w;
            if x.wrapping_sub(0x0101_0101_0101_0101) & !x & 0x8080_8080_8080_8080 == 0 {
                let take = (64 - self.n) / 8;
                if take > 0 {
                    let chunk = if take == 8 { w } else { w >> (64 - 8 * take) };
                    self.acc |= chunk << (64 - self.n - 8 * take);
                    self.n += 8 * take;
                    self.pos += take as usize;
                }
                return;
            }
        }
        while self.n <= 56 {
            let mut b = 0u8;
            if !self.marker && self.pos < self.data.len() {
                b = self.data[self.pos];
                if b == 0xFF {
                    let next = self.data.get(self.pos + 1).copied().unwrap_or(0);
                    if next == 0 {
                        self.pos += 2;
                    } else {
                        self.marker = true;
                        b = 0;
                    }
                } else {
                    self.pos += 1;
                }
            }
            self.acc |= (b as u64) << (56 - self.n);
            self.n += 8;
        }
    }

    fn peek(&mut self, k: u32) -> u32 {
        if self.n < k {
            self.fill();
        }
        (self.acc >> (64 - k)) as u32
    }

    fn skip(&mut self, k: u32) {
        self.acc <<= k;
        self.n -= k;
    }

    fn get(&mut self, k: u32) -> u32 {
        if k == 0 {
            return 0;
        }
        let v = self.peek(k);
        self.skip(k);
        v
    }

    fn decode(&mut self, h: &Huff) -> Option<u8> {
        let (len, sym) = h.fast[self.peek(9) as usize];
        if len > 0 {
            self.skip(len as u32);
            return Some(sym);
        }
        let code16 = self.peek(16) as i32;
        for len in 10..=16usize {
            let code = code16 >> (16 - len);
            if code <= h.maxcode[len] {
                self.skip(len as u32);
                return h.vals.get((h.valptr[len] + code - h.mincode[len]) as usize).copied();
            }
        }
        None
    }

    /// Receive and sign-extend a `k`-bit value (JPEG F.2.2.1).
    fn extend(&mut self, k: u32) -> i32 {
        if k == 0 {
            return 0;
        }
        let v = self.get(k) as i32;
        if v < (1 << (k - 1)) { v - (1 << k) + 1 } else { v }
    }

    /// Byte-align and step over a restart marker.
    fn restart(&mut self) {
        self.acc = 0;
        self.n = 0;
        self.marker = false;
        while self.pos + 1 < self.data.len() {
            if self.data[self.pos] == 0xFF && (0xD0..=0xD7).contains(&self.data[self.pos + 1]) {
                self.pos += 2;
                return;
            }
            self.pos += 1;
        }
    }

    /// The position of the marker that ends this entropy-coded segment.
    fn end(&self) -> usize {
        let mut p = self.pos;
        while let Some(i) = memchr_ff(&self.data[p..]) {
            p += i;
            match self.data.get(p + 1) {
                Some(&b) if b != 0 && b != 0xFF && !(0xD0..=0xD7).contains(&b) => return p,
                Some(_) => p += 1,
                None => break,
            }
        }
        self.data.len()
    }
}

fn memchr_ff(s: &[u8]) -> Option<usize> {
    s.iter().position(|&b| b == 0xFF)
}

struct Comp {
    id: u8,
    h: usize,
    v: usize,
    tq: usize,
    /// Blocks per line / column in this component (padded to whole MCUs).
    bw: usize,
    bh: usize,
    dc: Vec<i32>,
}

/// Decode `data` at 1/8 scale. `None` when the file isn't one this decoder handles.
pub fn decode_eighth(data: &[u8]) -> Option<Small> {
    if data.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut q = [[0u16; 64]; 4];
    let mut dc_tables: [Option<Huff>; 4] = [None, None, None, None];
    let mut ac_tables: [Option<Huff>; 4] = [None, None, None, None];
    let mut comps: Vec<Comp> = Vec::new();
    let (mut width, mut height) = (0usize, 0usize);
    let (mut hmax, mut vmax) = (1usize, 1usize);
    let mut progressive = false;
    let mut restart = 0usize;
    let mut adobe_rgb = false;
    let mut pos = 2;
    loop {
        // Next marker.
        while *data.get(pos)? != 0xFF {
            pos += 1;
        }
        while *data.get(pos)? == 0xFF {
            pos += 1;
        }
        let m = *data.get(pos)?;
        pos += 1;
        if m == 0xD9 {
            break;
        }
        if (0xD0..=0xD7).contains(&m) || m == 0x01 {
            continue;
        }
        let len = u16::from_be_bytes([*data.get(pos)?, *data.get(pos + 1)?]) as usize;
        let seg = data.get(pos + 2..pos + len)?;
        let next = pos + len;
        match m {
            0xDB => {
                let mut s = seg;
                while !s.is_empty() {
                    let (pq, tq) = (s[0] >> 4, (s[0] & 15) as usize);
                    if tq > 3 {
                        return None;
                    }
                    let n = if pq == 0 { 64 } else { 128 };
                    let body = s.get(1..1 + n)?;
                    // Only entry 0 (the DC quantizer) matters here; it is first in zigzag order.
                    q[tq][0] = if pq == 0 { body[0] as u16 } else { u16::from_be_bytes([body[0], body[1]]) };
                    s = &s[1 + n..];
                }
            }
            0xC4 => {
                let mut s = seg;
                while !s.is_empty() {
                    let (tc, th) = (s[0] >> 4, (s[0] & 15) as usize);
                    let counts: [u8; 16] = s.get(1..17)?.try_into().ok()?;
                    let total: usize = counts.iter().map(|&c| c as usize).sum();
                    let vals = s.get(17..17 + total)?.to_vec();
                    let h = Huff::new(&counts, vals)?;
                    match (tc, th) {
                        (0, 0..=3) => dc_tables[th] = Some(h),
                        (1, 0..=3) => ac_tables[th] = Some(h),
                        _ => return None,
                    }
                    s = &s[17 + total..];
                }
            }
            0xC0..=0xC2 => {
                progressive = m == 0xC2;
                if *seg.first()? != 8 {
                    return None;
                }
                height = u16::from_be_bytes([seg[1], seg[2]]) as usize;
                width = u16::from_be_bytes([seg[3], seg[4]]) as usize;
                let nc = seg[5] as usize;
                if width == 0 || height == 0 || !(nc == 1 || nc == 3) {
                    return None;
                }
                for i in 0..nc {
                    let c = seg.get(6 + 3 * i..9 + 3 * i)?;
                    let (h, v) = ((c[1] >> 4) as usize, (c[1] & 15) as usize);
                    if !(1..=4).contains(&h) || !(1..=4).contains(&v) || c[2] > 3 {
                        return None;
                    }
                    comps.push(Comp { id: c[0], h, v, tq: c[2] as usize, bw: 0, bh: 0, dc: Vec::new() });
                }
                hmax = comps.iter().map(|c| c.h).max()?;
                vmax = comps.iter().map(|c| c.v).max()?;
                let (mcux, mcuy) = (width.div_ceil(8 * hmax), height.div_ceil(8 * vmax));
                if mcux * mcuy * hmax * vmax > 64 << 20 {
                    return None;
                }
                for c in comps.iter_mut() {
                    c.bw = mcux * c.h;
                    c.bh = mcuy * c.v;
                    c.dc = vec![0; c.bw * c.bh];
                }
            }
            // Lossless, hierarchical, arithmetic coding: not here.
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return None,
            0xDD => restart = u16::from_be_bytes([*seg.first()?, *seg.get(1)?]) as usize,
            0xEE => {
                // Adobe: transform 0 means the components are RGB, not YCbCr.
                if seg.starts_with(b"Adobe") && seg.len() >= 12 {
                    adobe_rgb = seg[11] == 0;
                }
            }
            0xDA => {
                if comps.is_empty() {
                    return None;
                }
                let ns = *seg.first()? as usize;
                let mut sel: Vec<(usize, usize, usize)> = Vec::new();
                for i in 0..ns {
                    let id = *seg.get(1 + 2 * i)?;
                    let t = *seg.get(2 + 2 * i)?;
                    let ci = comps.iter().position(|c| c.id == id)?;
                    sel.push((ci, (t >> 4) as usize, (t & 15) as usize));
                }
                let rest = seg.get(1 + 2 * ns..)?;
                let (ss, se, ah, al) = (rest[0], rest[1], rest[2] >> 4, rest[2] & 15);
                let mut bits = Bits::new(data, next);
                if progressive && ss > 0 {
                    // An AC scan: not needed for DC values. Jump to its end.
                    pos = bits.end();
                    continue;
                }
                let _ = se;
                // Each scan's DC values build on the previous ones; read every DC scan.
                scan(&mut comps, &sel, &dc_tables, &ac_tables, &mut bits, progressive, ah, al, restart, hmax, vmax, width, height)?;
                pos = bits.end();
                continue;
            }
            _ => {}
        }
        pos = next;
    }
    if comps.is_empty() {
        return None;
    }
    // Assemble the 1/8 image: one pixel per luma block.
    let (ow, oh) = (width.div_ceil(8), height.div_ceil(8));
    let mut rgb = Vec::with_capacity(ow * oh * 3);
    let level = |c: &Comp, x: usize, y: usize| -> u8 {
        let bx = (x * c.h / hmax).min(c.bw - 1);
        let by = (y * c.v / vmax).min(c.bh - 1);
        let v = c.dc[by * c.bw + bx] * q[c.tq][0] as i32;
        // DC = 8 × (average − 128); rounded.
        ((v + if v >= 0 { 4 } else { -4 }) / 8 + 128).clamp(0, 255) as u8
    };
    for y in 0..oh {
        for x in 0..ow {
            if comps.len() == 1 {
                let g = level(&comps[0], x, y);
                rgb.extend_from_slice(&[g, g, g]);
            } else if adobe_rgb {
                rgb.extend_from_slice(&[level(&comps[0], x, y), level(&comps[1], x, y), level(&comps[2], x, y)]);
            } else {
                let (yy, cb, cr) = (level(&comps[0], x, y) as f32, level(&comps[1], x, y) as f32 - 128.0, level(&comps[2], x, y) as f32 - 128.0);
                let r = yy + 1.402 * cr;
                let g = yy - 0.344_136 * cb - 0.714_136 * cr;
                let b = yy + 1.772 * cb;
                rgb.extend_from_slice(&[r.round().clamp(0.0, 255.0) as u8, g.round().clamp(0.0, 255.0) as u8, b.round().clamp(0.0, 255.0) as u8]);
            }
        }
    }
    Some(Small { width: ow as u32, height: oh as u32, rgb })
}

#[allow(clippy::too_many_arguments)]
fn scan(
    comps: &mut [Comp],
    sel: &[(usize, usize, usize)],
    dc_tables: &[Option<Huff>; 4],
    ac_tables: &[Option<Huff>; 4],
    bits: &mut Bits,
    progressive: bool,
    ah: u8,
    al: u8,
    restart: usize,
    hmax: usize,
    vmax: usize,
    width: usize,
    height: usize,
) -> Option<()> {
    let mut pred = vec![0i32; sel.len()];
    // One block of one component: DC (first or refinement pass), then skip its AC.
    let block = |bits: &mut Bits, pred: &mut [i32], k: usize, ci: usize, bi: usize, comps: &mut [Comp]| -> Option<()> {
        let (_, td, ta) = sel[k];
        if progressive && ah > 0 {
            // DC refinement: one more bit.
            if bits.get(1) == 1 {
                comps[ci].dc[bi] |= 1 << al;
            }
            return Some(());
        }
        let t = bits.decode(dc_tables[td].as_ref()?)?;
        if t > 16 {
            return None;
        }
        pred[k] += bits.extend(t as u32);
        comps[ci].dc[bi] = pred[k] << al;
        if !progressive {
            let ac = ac_tables[ta].as_ref()?;
            let mut i = 1;
            while i < 64 {
                // Code and magnitude in one lookup (the common case).
                let e = ac.ac[bits.peek(AC_BITS) as usize];
                if e != 0 {
                    bits.skip((e & 31) as u32);
                    if e & 1 << 10 != 0 {
                        break;
                    }
                    i += (e >> 5 & 31) as usize;
                    continue;
                }
                let rs = bits.decode(ac)?;
                let (r, s) = (rs >> 4, rs & 15);
                if s == 0 {
                    if r != 15 {
                        break;
                    }
                    i += 16;
                } else {
                    i += r as usize + 1;
                    bits.skip_bits(s as u32);
                }
            }
        }
        Some(())
    };
    let mut done = 0usize;
    if sel.len() == 1 {
        // Non-interleaved: blocks of one component in raster order, over its real extent.
        let ci = sel[0].0;
        let c = &comps[ci];
        let bw = (width * c.h).div_ceil(8 * hmax);
        let bh = (height * c.v).div_ceil(8 * vmax);
        let stride = c.bw;
        for by in 0..bh {
            for bx in 0..bw {
                if restart > 0 && done > 0 && done % restart == 0 {
                    bits.restart();
                    pred.iter_mut().for_each(|p| *p = 0);
                }
                block(bits, &mut pred, 0, ci, by * stride + bx, comps)?;
                done += 1;
            }
        }
    } else {
        let (mcux, mcuy) = (width.div_ceil(8 * hmax), height.div_ceil(8 * vmax));
        for my in 0..mcuy {
            for mx in 0..mcux {
                if restart > 0 && done > 0 && done % restart == 0 {
                    bits.restart();
                    pred.iter_mut().for_each(|p| *p = 0);
                }
                for (k, &(ci, _, _)) in sel.iter().enumerate() {
                    let (h, v, stride) = (comps[ci].h, comps[ci].v, comps[ci].bw);
                    for y in 0..v {
                        for x in 0..h {
                            let bi = (my * v + y) * stride + mx * h + x;
                            block(bits, &mut pred, k, ci, bi, comps)?;
                        }
                    }
                }
                done += 1;
            }
        }
    }
    Some(())
}

impl Bits<'_> {
    fn skip_bits(&mut self, k: u32) {
        if self.n < k {
            self.fill();
        }
        self.skip(k);
    }
}

/// The image size from the frame header.
pub fn dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let mut pos = 2;
    while pos + 4 <= data.len() {
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let m = data[pos + 1];
        if m == 0xFF || (0xD0..=0xD7).contains(&m) || m == 0x01 {
            pos += 1;
            continue;
        }
        let len = u16::from_be_bytes([data[pos + 2], *data.get(pos + 3)?]) as usize;
        if matches!(m, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            let h = u16::from_be_bytes([*data.get(pos + 5)?, *data.get(pos + 6)?]) as u32;
            let w = u16::from_be_bytes([*data.get(pos + 7)?, *data.get(pos + 8)?]) as u32;
            return Some((w, h));
        }
        if m == 0xDA {
            return None;
        }
        pos += 2 + len;
    }
    None
}

/// The preview JPEG most cameras store in EXIF (IFD1), if there is one.
pub fn exif_thumbnail(data: &[u8]) -> Option<&[u8]> {
    // APP1 "Exif\0\0" is among the first segments.
    let mut pos = 2;
    let tiff = loop {
        if *data.get(pos)? != 0xFF {
            return None;
        }
        let m = *data.get(pos + 1)?;
        let len = u16::from_be_bytes([*data.get(pos + 2)?, *data.get(pos + 3)?]) as usize;
        if m == 0xE1 && data.get(pos + 4..pos + 10)? == b"Exif\0\0" {
            break data.get(pos + 10..pos + 2 + len)?;
        }
        if m == 0xDA || !(0xE0..=0xFE).contains(&m) && m != 0xDB && m != 0xC4 {
            return None;
        }
        pos += 2 + len;
    };
    let le = match tiff.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |o: usize| -> Option<u32> {
        let b = tiff.get(o..o + 2)?;
        Some(if le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) } as u32)
    };
    let u32_at = |o: usize| -> Option<u32> {
        let b: [u8; 4] = tiff.get(o..o + 4)?.try_into().ok()?;
        Some(if le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
    };
    let ifd0 = u32_at(4)? as usize;
    let n0 = u16_at(ifd0)? as usize;
    let ifd1 = u32_at(ifd0 + 2 + 12 * n0)? as usize;
    if ifd1 == 0 {
        return None;
    }
    let n1 = u16_at(ifd1)? as usize;
    let (mut off, mut len) = (None, None);
    for i in 0..n1.min(64) {
        let e = ifd1 + 2 + 12 * i;
        match u16_at(e)? {
            0x0201 => off = Some(u32_at(e + 8)? as usize),
            0x0202 => len = Some(u32_at(e + 8)? as usize),
            _ => {}
        }
    }
    let (off, len) = (off?, len?);
    let t = tiff.get(off..off.checked_add(len)?)?;
    (t.starts_with(&[0xFF, 0xD8])).then_some(t)
}

/// Fit `s` into `size`×`size` (keeping its shape) by averaging the pixels each output
/// pixel covers; RGBA out.
pub fn shrink(s: &Small, size: u32) -> (u32, u32, Vec<u8>) {
    let (w, h) = (s.width as usize, s.height as usize);
    let scale = (size as f64 / w as f64).min(size as f64 / h as f64).min(1.0);
    let (ow, oh) = (((w as f64 * scale).round() as usize).max(1), ((h as f64 * scale).round() as usize).max(1));
    let mut out = Vec::with_capacity(ow * oh * 4);
    for oy in 0..oh {
        let (y0, y1) = (oy * h / oh, ((oy + 1) * h / oh).max(oy * h / oh + 1));
        for ox in 0..ow {
            let (x0, x1) = (ox * w / ow, ((ox + 1) * w / ow).max(ox * w / ow + 1));
            let mut acc = [0u32; 3];
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = (y * w + x) * 3;
                    acc[0] += s.rgb[p] as u32;
                    acc[1] += s.rgb[p + 1] as u32;
                    acc[2] += s.rgb[p + 2] as u32;
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            out.extend_from_slice(&[((acc[0] + n / 2) / n) as u8, ((acc[1] + n / 2) / n) as u8, ((acc[2] + n / 2) / n) as u8, 255]);
        }
    }
    (ow as u32, oh as u32, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind of JPEG ImageMagick and cjpeg write, decoded at 1/8 and compared with a
    /// full decode averaged over 8×8 blocks.
    #[test]
    fn matches_full_decode() {
        if std::process::Command::new("magick").arg("-version").output().is_err() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.png");
        assert!(std::process::Command::new("magick").args(["-size", "1001x777", "-seed", "7", "plasma:fractal"]).arg(&src).status().unwrap().success());
        let variants: &[(&str, &[&str])] = &[
            ("420.jpg", &["-sampling-factor", "4:2:0", "-quality", "90"]),
            ("444.jpg", &["-sampling-factor", "4:4:4"]),
            ("422.jpg", &["-sampling-factor", "4:2:2"]),
            ("prog.jpg", &["-interlace", "Plane"]),
            ("gray.jpg", &["-colorspace", "Gray"]),
            ("q10.jpg", &["-quality", "10"]),
        ];
        let mut files = Vec::new();
        for (name, args) in variants {
            let out = dir.path().join(name);
            assert!(std::process::Command::new("magick").arg(&src).args(*args).arg(&out).status().unwrap().success());
            files.push(out);
        }
        // Restart markers every MCU, and optimized tables, via cjpeg.
        if let Ok(ppm) = std::process::Command::new("magick").arg(&src).arg("ppm:-").output() {
            for (name, args) in [("restart.jpg", &["-restart", "1B"][..]), ("optimize.jpg", &["-optimize"][..]), ("progcjpeg.jpg", &["-progressive"][..])] {
                let out = dir.path().join(name);
                let mut c = std::process::Command::new("cjpeg").args(args).arg("-outfile").arg(&out).stdin(std::process::Stdio::piped()).spawn();
                if let Ok(child) = c.as_mut() {
                    use std::io::Write;
                    child.stdin.take().unwrap().write_all(&ppm.stdout).unwrap();
                    if child.wait().unwrap().success() {
                        files.push(out);
                    }
                }
            }
        }
        for f in &files {
            let data = std::fs::read(f).unwrap();
            let small = decode_eighth(&data).unwrap_or_else(|| panic!("{} not decoded", f.display()));
            let full = image::open(f).unwrap().to_rgb8();
            assert_eq!((small.width, small.height), (full.width().div_ceil(8), full.height().div_ceil(8)), "{}", f.display());
            let mut diff = 0u64;
            let mut n = 0u64;
            for by in 0..full.height() / 8 {
                for bx in 0..full.width() / 8 {
                    for ch in 0..3 {
                        let mut acc = 0u32;
                        for y in 0..8 {
                            for x in 0..8 {
                                acc += full.get_pixel(bx * 8 + x, by * 8 + y)[ch] as u32;
                            }
                        }
                        let want = (acc + 32) / 64;
                        let got = small.rgb[((by * small.width + bx) * 3) as usize + ch] as u32;
                        diff += want.abs_diff(got) as u64;
                        n += 1;
                    }
                }
            }
            let mean = diff as f64 / n as f64;
            assert!(mean < 4.0, "{}: mean difference {mean:.2}", f.display());
        }
        // Speed against a full decode + resize (printed with --nocapture).
        for f in &files {
            let data = std::fs::read(f).unwrap();
            let t = std::time::Instant::now();
            for _ in 0..5 {
                let s = decode_eighth(&data).unwrap();
                std::hint::black_box(shrink(&s, 128));
            }
            let ours = t.elapsed() / 5;
            let t = std::time::Instant::now();
            for _ in 0..5 {
                std::hint::black_box(image::load_from_memory(&data).unwrap().thumbnail(128, 128));
            }
            let full = t.elapsed() / 5;
            eprintln!("{:<16} dc {:>8.2?}  full {:>8.2?}  ({:.1}x)", f.file_name().unwrap().to_string_lossy(), ours, full, full.as_secs_f64() / ours.as_secs_f64());
        }
        // Real photos, if any are around.
        for f in ["/usr/share/sddm/themes/maldives/background.jpg", "/usr/share/omarchy/themes/ethereal/backgrounds/1-cosmic.jpg"] {
            let Ok(data) = std::fs::read(f) else { continue };
            let t = std::time::Instant::now();
            let Some(s) = decode_eighth(&data) else { continue };
            std::hint::black_box(shrink(&s, 128));
            let ours = t.elapsed();
            let t = std::time::Instant::now();
            let img = image::load_from_memory(&data).unwrap();
            std::hint::black_box(img.thumbnail(128, 128));
            let full = t.elapsed();
            eprintln!("{f}: {}x{} dc {ours:.2?} full {full:.2?} ({:.1}x)", img.width(), img.height(), full.as_secs_f64() / ours.as_secs_f64());
        }
        // A camera-style file: an APP1 EXIF block whose IFD1 points at a preview JPEG.
        let thumb = dir.path().join("thumb.jpg");
        assert!(std::process::Command::new("magick").arg(&src).args(["-resize", "160x123!"]).arg(&thumb).status().unwrap().success());
        let preview = std::fs::read(&thumb).unwrap();
        for le in [true, false] {
            let w16 = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
            let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
            let mut tiff: Vec<u8> = if le { b"II".to_vec() } else { b"MM".to_vec() };
            tiff.extend(w16(42));
            tiff.extend(w32(8)); // IFD0 at 8
            tiff.extend(w16(0)); // no entries
            tiff.extend(w32(14)); // IFD1 at 14
            tiff.extend(w16(2));
            let data_at = 14 + 2 + 2 * 12 + 4;
            for (tag, val) in [(0x0201u16, data_at as u32), (0x0202, preview.len() as u32)] {
                tiff.extend(w16(tag));
                tiff.extend(w16(4));
                tiff.extend(w32(1));
                tiff.extend(w32(val));
            }
            tiff.extend(w32(0));
            tiff.extend_from_slice(&preview);
            let mut app1 = b"Exif\0\0".to_vec();
            app1.extend(tiff);
            let main = std::fs::read(&files[0]).unwrap();
            let mut file = vec![0xFF, 0xD8, 0xFF, 0xE1];
            file.extend(((app1.len() + 2) as u16).to_be_bytes());
            file.extend(app1);
            file.extend_from_slice(&main[2..]);
            assert_eq!(exif_thumbnail(&file), Some(preview.as_slice()));
            assert_eq!(dimensions(&file), Some((1001, 777)));
            assert!(decode_eighth(&file).is_some());
        }
        assert_eq!(exif_thumbnail(&std::fs::read(&files[0]).unwrap()), None);
        // Arithmetic coding isn't handled: the caller falls back.
        let arith = dir.path().join("arith.jpg");
        if std::process::Command::new("magick").arg(&src).arg("ppm:-").output().is_ok() {
            let _ = std::process::Command::new("sh").arg("-c").arg(format!("magick {} ppm:- | cjpeg -arithmetic -outfile {}", src.display(), arith.display())).status();
            if let Ok(d) = std::fs::read(&arith) {
                assert!(decode_eighth(&d).is_none());
            }
        }
    }
}

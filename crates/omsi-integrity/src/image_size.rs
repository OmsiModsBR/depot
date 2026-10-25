//! Picture size from the file header, without decoding the picture.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// The smallest power of two that is at least `n` (1 for 0); `u32::MAX` where none fits.
pub fn next_pow2(n: u32) -> u32 {
    n.max(1).checked_next_power_of_two().unwrap_or(u32::MAX)
}

/// No texture the game can hold is wider than this: a header that says more is damaged.
const MAX_SIDE: u32 = 32768;

/// Width and height of a bmp, tga, dds, png or jpg, told by the content and not by the
/// extension (mods keep a dds under a `.jpg` name). `None` when it is none of them.
pub fn image_size(path: &Path) -> Option<(u32, u32)> {
    match std::fs::File::open(path) {
        Ok(mut f) => size_of(&mut f, path),
        // a file inside a mounted `.zip`
        Err(_) => {
            let bytes = omsi_cfg::vfs::read(path).ok()?;
            size_of(&mut std::io::Cursor::new(bytes), path)
        }
    }
}

fn size_of<R: Read + Seek>(r: &mut R, path: &Path) -> Option<(u32, u32)> {
    let mut head = [0u8; 32];
    let n = read_up_to(r, &mut head);
    let h = &head[..n];
    let be = |o: usize| u32::from_be_bytes(h[o..o + 4].try_into().unwrap());
    let le = |o: usize| u32::from_le_bytes(h[o..o + 4].try_into().unwrap());
    if h.starts_with(b"DDS ") && n >= 20 {
        return nonzero(le(16), le(12));
    }
    if h.starts_with(b"\x89PNG\r\n\x1a\n") && n >= 24 {
        return nonzero(be(16), be(20));
    }
    if h.starts_with(b"BM") && n >= 26 {
        // (a negative height is a top-down bitmap)
        let w = i32::from_le_bytes(h[18..22].try_into().unwrap());
        let ht = i32::from_le_bytes(h[22..26].try_into().unwrap());
        return nonzero(w.unsigned_abs(), ht.unsigned_abs());
    }
    if h.starts_with(&[0xFF, 0xD8]) {
        return jpeg_size(r);
    }
    // a tga has no magic number: the extension and a plausible header decide
    let is_tga = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("tga"))
        .unwrap_or(false);
    if is_tga && n >= 18 && matches!(h[2], 1 | 2 | 3 | 9 | 10 | 11) {
        let w = u16::from_le_bytes([h[12], h[13]]) as u32;
        let ht = u16::from_le_bytes([h[14], h[15]]) as u32;
        return nonzero(w, ht);
    }
    None
}

fn nonzero(w: u32, h: u32) -> Option<(u32, u32)> {
    (w > 0 && h > 0 && w <= MAX_SIDE && h <= MAX_SIDE).then_some((w, h))
}

fn read_up_to<R: Read>(r: &mut R, buf: &mut [u8]) -> usize {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => n += k,
        }
    }
    n
}

/// Walk the segments up to the first start-of-frame marker.
fn jpeg_size<R: Read + Seek>(r: &mut R) -> Option<(u32, u32)> {
    r.seek(SeekFrom::Start(2)).ok()?;
    loop {
        let mut m = [0u8; 2];
        r.read_exact(&mut m).ok()?;
        if m[0] != 0xFF {
            return None;
        }
        let marker = m[1];
        if marker == 0xFF {
            // fill byte
            r.seek(SeekFrom::Current(-1)).ok()?;
            continue;
        }
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        let mut l = [0u8; 2];
        r.read_exact(&mut l).ok()?;
        let len = u16::from_be_bytes(l) as i64;
        if len < 2 {
            return None;
        }
        let sof = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
        if sof {
            let mut d = [0u8; 5];
            r.read_exact(&mut d).ok()?;
            let h = u16::from_be_bytes([d[1], d[2]]) as u32;
            let w = u16::from_be_bytes([d[3], d[4]]) as u32;
            return nonzero(w, h);
        }
        r.seek(SeekFrom::Current(len - 2)).ok()?;
    }
}

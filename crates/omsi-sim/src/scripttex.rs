//! `[scripttexture]`: RGBA images the scripts draw into through the `ST*` callbacks
//! (matrix displays). Materials use them with `[useScriptTexture] n` or as a transparency
//! map `[matl_transmap] \S:n`.

use omsi_content::font::FontAtlas;
use std::sync::Arc;

/// Largest side, in texels, of a script texture. Real displays are a few hundred texels wide; the
/// ceiling keeps `w * h * 4` far inside `u32`/`usize` and a hostile `model.cfg` from asking for gigabytes.
/// (A shared `checked_texture_size` helper may replace this local check later.)
pub const MAX_SCRIPT_TEXTURE_SIDE: u32 = 4096;

pub struct ScriptTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Drawing buffer changed since the last unlock or upload.
    pub dirty: bool,
    pub locked: bool,
    /// Last image released by STUnlock, retained even when the script locks again
    /// before the renderer runs (the Atron AFR4 keeps its drawing buffer locked).
    pending: Option<Vec<u8>>,
    /// `STFilter` was called after this texture was unlocked. OMSI then generates a mip
    /// chain, which keeps a distant matrix or IBIS screen stable instead of sampling its
    /// full-resolution pixels directly.
    pub mipmaps: bool,
    /// Current draw colour as set by `STSetColor` (r, g, b, a).
    pub color: [u8; 4],
    /// The size the `[scripttexture]` entry declares, which `STNewTex` comes back to
    /// (`STLoadTex` makes the texture the size of its file).
    declared: (u32, u32),
}

impl ScriptTexture {
    pub fn new(width: i32, height: i32) -> ScriptTexture {
        let side = |v: i32| (v.max(1) as u32).min(MAX_SCRIPT_TEXTURE_SIDE);
        let (w, h) = (side(width), side(height));
        if width > MAX_SCRIPT_TEXTURE_SIDE as i32 || height > MAX_SCRIPT_TEXTURE_SIDE as i32 {
            log::warn!("scripttexture: size {width}x{height} above {MAX_SCRIPT_TEXTURE_SIDE}, using {w}x{h}");
        }
        ScriptTexture {
            width: w,
            height: h,
            rgba: vec![0; w as usize * h as usize * 4],
            dirty: true,
            locked: false,
            pending: None,
            mipmaps: false,
            color: [255, 255, 255, 255],
            declared: (w, h),
        }
    }

    /// `STNewTex`: a new, empty texture of the declared size (OMSI
    /// releases it and calls D3DXCreateTexture with the `[scripttexture]` entry's size).
    pub fn renew(&mut self) {
        let (w, h) = self.declared;
        if (w, h) != (self.width, self.height) {
            self.width = w;
            self.height = h;
            self.rgba = vec![0; w as usize * h as usize * 4];
            // (an image released at the old size must not go up at the new one)
            self.pending = None;
        }
        self.clear();
        // `STNewTex` starts a new drawing surface.  RHLib's transparency-map scaler
        // checks the target's current alpha before its first `STSetColor`; this must
        // therefore match the new transparent canvas rather than the prior draw state.
        self.color = [0; 4];
    }

    pub fn clear(&mut self) {
        self.rgba.iter_mut().for_each(|b| *b = 0);
        self.dirty = true;
    }

    pub fn unlock(&mut self) {
        self.locked = false;
        if self.dirty {
            self.pending = Some(self.rgba.clone());
            self.dirty = false;
        }
    }

    /// Take the latest released image. A locked buffer may already contain edits
    /// for the next unlock; those must not replace the image just released.
    pub fn take_upload(&mut self) -> Option<Vec<u8>> {
        if !self.locked && self.dirty {
            self.dirty = false;
            self.pending = None;
            Some(self.rgba.clone())
        } else {
            self.pending.take()
        }
    }

    /// The image the displays show: the one last released by `STUnlock` while the script
    /// keeps drawing into a locked buffer, else the buffer itself. What a situation saves.
    pub fn shown(&self) -> &[u8] {
        match &self.pending {
            Some(p) if self.locked => p,
            _ => &self.rgba,
        }
    }

    pub fn put(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        self.rgba[i..i + 4].copy_from_slice(&c);
        self.dirty = true;
    }

    pub fn get(&self, x: i32, y: i32) -> [u8; 4] {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return [0; 4];
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    pub fn rect(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) {
        let c = self.color;
        // clip to the texture first: the arguments come from the script and may span the whole i32 range
        let (x0, x1) = (x1.min(x2).max(0), x1.max(x2).min(self.width as i32 - 1));
        let (y0, y1) = (y1.min(y2).max(0), y1.max(y2).min(self.height as i32 - 1));
        if x0 > x1 || y0 > y1 {
            return;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.put(x, y, c);
            }
        }
    }

    /// Draw `text` with the font at (x, y) (top-left), `spacing` extra pixels between
    /// letters. Glyph coverage scales the colour's alpha; RGB is the draw colour.
    /// `STTextOut` as OMSI draws it: `mode` bit 0 takes the colour
    /// from the font's colour bitmap (else the `STSetColor` colour), the alpha is the font's
    /// mask; `mode & 3 == 2` writes only the glyphs' covered pixels (a transparent
    /// background), any other mode the whole glyph cell.
    pub fn text_out(
        &mut self,
        atlas: &FontAtlas,
        x: i32,
        y: i32,
        spacing: i32,
        mode: u8,
        text: &str,
    ) {
        let glyph_h = atlas.font.height.max(1) as i32;
        let mut cx = x;
        let c = self.color;
        let from_font = mode & 1 != 0;
        let transparent = mode & 3 == 2;
        for ch in text.chars() {
            if ch.is_whitespace() {
                cx = cx.saturating_add(
                    atlas
                        .font
                        .space_width()
                        .saturating_add(atlas.font.gap)
                        .saturating_add(spacing),
                );
                continue;
            }
            let Some(g) = atlas.font.glyph(ch) else {
                cx = cx
                    .saturating_add(atlas.font.gap.max(1))
                    .saturating_add(spacing);
                continue;
            };
            let gw = (g.x1 - g.x0).max(0);
            for gy in 0..glyph_h {
                let sy = g.y + gy;
                if sy < 0 || sy >= atlas.height as i32 {
                    continue;
                }
                for gx in 0..gw {
                    let sx = g.x0 + gx;
                    if sx < 0 || sx >= atlas.width as i32 {
                        continue;
                    }
                    let si = ((sy as u32 * atlas.width + sx as u32) * 4) as usize;
                    let cov = atlas.alpha[si];
                    if cov == 0 && transparent {
                        continue;
                    }
                    let rgb = if from_font && si + 2 < atlas.color.len() {
                        [atlas.color[si], atlas.color[si + 1], atlas.color[si + 2]]
                    } else {
                        [c[0], c[1], c[2]]
                    };
                    let (Some(px), Some(py)) = (cx.checked_add(gx), y.checked_add(gy)) else {
                        continue;
                    };
                    self.put(px, py, [rgb[0], rgb[1], rgb[2], cov]);
                }
            }
            cx = cx
                .saturating_add(gw)
                .saturating_add(atlas.font.gap)
                .saturating_add(spacing);
        }
    }

    /// `STLoadTex`: the texture becomes the file's picture. OMSI
    /// releases it and calls D3DXCreateTextureFromFileExA with width and height
    /// D3DX_DEFAULT and a full mip chain: the file's own size rounded up to a power of two,
    /// the picture stretched to fill it, so the mesh's UVs always cover the whole bitmap
    /// (a 256x64 bitmap into a 256x32 entry is not cut to its top half).
    pub fn load(&mut self, w: u32, h: u32, rgba: &[u8]) {
        if w == 0 || h == 0 || w > MAX_SCRIPT_TEXTURE_SIDE || h > MAX_SCRIPT_TEXTURE_SIDE {
            log::warn!("scripttexture: STLoadTex picture {w}x{h} outside 1..={MAX_SCRIPT_TEXTURE_SIDE}, ignored");
            return;
        }
        // both sides are at most 4096 here, so none of these products can overflow
        if rgba.len() < w as usize * h as usize * 4 {
            return;
        }
        let (tw, th) = (w.next_power_of_two(), h.next_power_of_two());
        self.width = tw;
        self.height = th;
        self.rgba = if (tw, th) == (w, h) {
            rgba[..w as usize * h as usize * 4].to_vec()
        } else {
            resample(w, h, rgba, tw, th)
        };
        self.mipmaps = true;
        // the file replaces whatever was released at the old size
        self.pending = None;
        self.dirty = true;
    }
}

/// Stretch an RGBA picture to another size, bilinearly (texel centres onto texel centres).
fn resample(w: u32, h: u32, src: &[u8], tw: u32, th: u32) -> Vec<u8> {
    let mut out = vec![0u8; tw as usize * th as usize * 4];
    let axis = |t: u32, n: u32, size: u32| -> (usize, usize, f32) {
        let f = ((t as f32 + 0.5) * n as f32 / size as f32 - 0.5).clamp(0.0, (n - 1) as f32);
        let i = f.floor() as usize;
        (i, (i + 1).min(n as usize - 1), f - i as f32)
    };
    for y in 0..th {
        let (y0, y1, fy) = axis(y, h, th);
        for x in 0..tw {
            let (x0, x1, fx) = axis(x, w, tw);
            let px = |xx: usize, yy: usize, c: usize| src[(yy * w as usize + xx) * 4 + c] as f32;
            for c in 0..4 {
                let top = px(x0, y0, c) * (1.0 - fx) + px(x1, y0, c) * fx;
                let bot = px(x0, y1, c) * (1.0 - fx) + px(x1, y1, c) * fx;
                out[(y as usize * tw as usize + x as usize) * 4 + c] =
                    (top * (1.0 - fy) + bot * fy).round() as u8;
            }
        }
    }
    out
}

/// Registered fonts of a host: index = `GetFontIndex` result.
#[derive(Default)]
pub struct FontTable {
    pub entries: Vec<(String, Option<Arc<FontAtlas>>)>,
}

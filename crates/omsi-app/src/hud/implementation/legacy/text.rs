//! The texts: Roboto rendered with its outline into textures kept while they are shown, and
//! the measuring, wrapping and clipping of a text to a width.

use crate::hud::interface::output::{Renderer, Scene, TextureId};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont, VariableFont};

/// Roboto (Apache 2.0), the interface font.
pub const ROBOTO: &[u8] =
    include_bytes!("../../../../../../assets/fonts/Roboto-VariableFont_wdth,wght.ttf");

/// A rendered text: its texture and size in pixels.
#[derive(Clone, Copy)]
pub(super) struct Label {
    pub(super) tex: TextureId,
    pub(super) w: u32,
    pub(super) h: u32,
    pub(super) used: u64,
}

/// Or'ed into a label's pixel size: the text in the bold weight.
pub(super) const BOLD: u32 = 1 << 31;

/// Texts rendered into textures, kept while they are used.
pub struct TextCache {
    font: FontVec,
    /// The same Roboto at 700: the menu's titles and its primary button, as the launcher
    /// draws them (`Weight::Bold`); asked for with `BOLD` in the size.
    bold: FontVec,
    pub(super) labels: hashbrown::HashMap<(String, u32, [u8; 4]), Label>,
    pub(super) frame: u64,
    /// How strongly the background plates are drawn this frame (`backdrop`).
    pub(super) backdrop: f32,
    /// No outline round the texts that ask for none (the game menu's: flat text on its card).
    pub(super) flat: bool,
}

impl TextCache {
    pub fn new() -> Option<TextCache> {
        let mut font = FontVec::try_from_vec(ROBOTO.to_vec()).ok()?;
        // a little heavier than the regular 400: light text on a dark panel is thin and
        // greyish at menu sizes otherwise
        let _ = font.set_variation(b"wght", 500.0);
        let mut bold = FontVec::try_from_vec(ROBOTO.to_vec()).ok()?;
        let _ = bold.set_variation(b"wght", 700.0);
        Some(TextCache {
            font,
            bold,
            labels: hashbrown::HashMap::new(),
            frame: 0,
            backdrop: 1.0,
            flat: false,
        })
    }

    /// The texture of `text` at `px` pixels in `color` (alpha = opacity of the outline), and
    /// its size.
    pub(super) fn label(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        text: &str,
        px: u32,
        color: [u8; 4],
    ) -> Label {
        self.label_with(r, scene, &omsi_ui::tr(text), px, color)
    }

    /// Like [`label`](Self::label) for a text that is not an interface text (a name from the
    /// map): drawn as it is.
    pub(super) fn label_raw(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        text: &str,
        px: u32,
        color: [u8; 4],
    ) -> Label {
        self.label_with(r, scene, text, px, color)
    }

    fn label_with(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        text: &str,
        px: u32,
        color: [u8; 4],
    ) -> Label {
        let color = [
            color[0],
            color[1],
            color[2],
            outline_for(color, if self.flat { 1.0 } else { self.backdrop }),
        ];
        // (in the interface's language: the menu, the notes, the windows; a letter and its
        // combining mark as one, as macOS gives file names - the weather "Eiseska\u{308}lte"
        // showed a box after its "a" in the menu's list)
        let text = &*omsi_ui::text::composed(text);
        let key = (text.to_string(), px, color);
        if let Some(l) = self.labels.get_mut(&key) {
            l.used = self.frame;
            return *l;
        }
        let font = if px & BOLD != 0 {
            &self.bold
        } else {
            &self.font
        };
        let img = render_text(font, text, (px & !BOLD) as f32, color);
        let tex = r.add_texture(scene, &img, false);
        let l = Label {
            tex,
            w: img.width,
            h: img.height,
            used: self.frame,
        };
        self.labels.insert(key, l);
        l
    }

    /// Text width in pixels, without rendering it.
    pub fn width(&self, text: &str, px: f32) -> f32 {
        let text = &*omsi_ui::tr(text);
        self.width_raw(text, px)
    }

    /// `width` of a text drawn in the bold weight (`BOLD`).
    pub fn width_bold(&self, text: &str, px: f32) -> f32 {
        let text = &*omsi_ui::tr(text);
        self.width_in(&self.bold, text, px)
    }

    pub(super) fn width_raw(&self, text: &str, px: f32) -> f32 {
        self.width_in(&self.font, text, px)
    }

    fn width_in(&self, base: &FontVec, text: &str, px: f32) -> f32 {
        let text = omsi_ui::text::composed(text);
        let mut w = 0.0;
        let mut prev: Option<(ab_glyph::GlyphId, *const FontVec)> = None;
        for c in text.chars() {
            let font = font_for(base, c);
            let f = font.as_scaled(PxScale::from(px));
            let id = f.glyph_id(c);
            if let Some((p, pf)) = prev {
                if std::ptr::eq(pf, font) {
                    w += f.kern(p, id);
                }
            }
            w += f.h_advance(id);
            prev = Some((id, font as *const FontVec));
        }
        w + outline_px(px) * 2.0 + 2.0
    }

    /// End of a frame: labels not used for a few seconds are released.
    pub fn end_frame(&mut self, r: &Renderer, scene: &mut Scene) {
        self.frame += 1;
        if self.frame % 120 == 0 {
            let old: Vec<_> = self
                .labels
                .iter()
                .filter(|(_, l)| self.frame.saturating_sub(l.used) > 240)
                .map(|(k, _)| k.clone())
                .collect();
            for k in old {
                if let Some(l) = self.labels.remove(&k) {
                    r.free_texture(scene, l.tex);
                }
            }
        }
    }
}

/// How opaque a label's dark outline is (the alpha of its `color`): as asked, or - for a
/// panel's text, asked without one - more as the opacity setting thins the panels
/// (`backdrop` below 1): on a see-through panel over a bright sky the dim lines could not be
/// read. Light texts only: round a dark one (the highlighted menu line's, on the solid
/// accent) the outline is as dark as the glyphs and smeared them, like a shadow.
pub fn outline_for(color: [u8; 4], backdrop: f32) -> u8 {
    let light = 0.299 * color[0] as f32 + 0.587 * color[1] as f32 + 0.114 * color[2] as f32 > 100.0;
    match color[3] {
        0 if backdrop < 1.0 && light => (((1.0 - backdrop) * 1.6).min(0.9) * 255.0) as u8,
        a => a,
    }
}

/// The outline (UIStroke) around the glyphs, in pixels.
fn outline_px(px: f32) -> f32 {
    (px / 9.0).clamp(1.0, 3.0)
}

/// The font that draws `c`: Roboto, else the system's font for the script (Chinese,
/// Japanese, Korean, Thai, Hindi - the menu was a column of boxes in those languages).
pub fn font_for(roboto: &FontVec, c: char) -> &FontVec {
    if omsi_ui::text::needs_fallback(roboto, c) {
        if let Some(f) = omsi_ui::text::fallback_font(c) {
            return f;
        }
    }
    roboto
}

/// `text` as straight-alpha RGBA: the glyphs in `color` over a dark outline.
pub fn render_text(font: &FontVec, text: &str, px: f32, color: [u8; 4]) -> omsi_texture::Image {
    let f = font.as_scaled(PxScale::from(px));
    let stroke = outline_px(px);
    let pad = stroke.ceil() as i32 + 1;
    let asc = f.ascent();
    let h = (asc - f.descent()).ceil() as i32 + pad * 2;
    // the baseline and every glyph on whole pixels: glyphs drawn between pixels came out
    // different from letter to letter and soft (the pen itself still runs on fractions, so
    // the text keeps its width)
    let base = (pad as f32 + asc).round();
    // lay the glyphs out
    let mut glyphs: Vec<(&FontVec, ab_glyph::Glyph)> = Vec::new();
    let mut x = pad as f32;
    let mut prev: Option<(ab_glyph::GlyphId, *const FontVec)> = None;
    for c in text.chars() {
        let gf = font_for(font, c);
        let sf = gf.as_scaled(PxScale::from(px));
        let id = sf.glyph_id(c);
        if let Some((p, pf)) = prev {
            if std::ptr::eq(pf, gf) {
                x += sf.kern(p, id);
            }
        }
        glyphs.push((
            gf,
            id.with_scale_and_position(PxScale::from(px), ab_glyph::point(x.round(), base)),
        ));
        x += sf.h_advance(id);
        prev = Some((id, gf as *const FontVec));
    }
    let w = (x.ceil() as i32 + pad).max(1);
    let (wu, hu) = (w as usize, h.max(1) as usize);
    let mut cov = vec![0f32; wu * hu];
    for (gf, g) in glyphs {
        if let Some(o) = gf.outline_glyph(g) {
            let b = o.px_bounds();
            o.draw(|gx, gy, c| {
                let xx = b.min.x as i32 + gx as i32;
                let yy = b.min.y as i32 + gy as i32;
                if xx >= 0 && yy >= 0 && (xx as usize) < wu && (yy as usize) < hu {
                    let i = yy as usize * wu + xx as usize;
                    cov[i] = (cov[i] + c).min(1.0);
                }
            });
        }
    }
    // the texture is sRGB and blended in linear light: light text on a dark panel comes out
    // too fat and blurry, dark text on a light one too thin - bend the coverage to match
    let light = 0.299 * color[0] as f32 + 0.587 * color[1] as f32 + 0.114 * color[2] as f32 > 100.0;
    let gamma = if light { 1.45 } else { 0.8 };
    for c in cov.iter_mut() {
        if *c > 0.0 && *c < 1.0 {
            *c = c.powf(gamma);
        }
    }
    // the outline: the coverage grown by the stroke radius
    let r = stroke;
    let ri = r.ceil() as i32;
    let mut edge = vec![0f32; wu * hu];
    for y in 0..hu as i32 {
        for x in 0..wu as i32 {
            let mut m = 0f32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    if d > r + 0.5 {
                        continue;
                    }
                    let (sx, sy) = (x + dx, y + dy);
                    if sx < 0 || sy < 0 || sx >= wu as i32 || sy >= hu as i32 {
                        continue;
                    }
                    let k = (r + 0.5 - d).clamp(0.0, 1.0);
                    m = m.max(cov[sy as usize * wu + sx as usize] * k);
                }
            }
            edge[y as usize * wu + x as usize] = m;
        }
    }
    let oa = color[3] as f32 / 255.0;
    let mut rgba = vec![0u8; wu * hu * 4];
    for i in 0..wu * hu {
        let a_text = cov[i];
        let a_edge = edge[i] * oa;
        let a = a_text + a_edge * (1.0 - a_text);
        if a <= 0.0 {
            continue;
        }
        for c in 0..3 {
            let v = (color[c] as f32 * a_text + 12.0 * a_edge * (1.0 - a_text)) / a;
            rgba[i * 4 + c] = v.round().clamp(0.0, 255.0) as u8;
        }
        rgba[i * 4 + 3] = (a * 255.0).round() as u8;
    }
    omsi_texture::Image {
        width: wu as u32,
        height: hu as u32,
        rgba,
        has_alpha: true,
    }
}

/// `text` clipped to `width` pixels in the bold weight (with "...").
pub(super) fn clip_bold(tc: &TextCache, text: &str, px: f32, width: f32) -> String {
    let text = &*omsi_ui::tr(text);
    if tc.width_bold(text, px) <= width {
        return text.to_string();
    }
    let mut out: String = text.chars().collect();
    while !out.is_empty() && tc.width_bold(&format!("{out}…"), px) > width {
        out.pop();
    }
    format!("{}…", out.trim_end())
}

/// `text` broken into lines of at most `width` pixels, between words.
pub(super) fn wrap(tc: &TextCache, text: &str, px: f32, width: f32) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let try_line = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if tc.width(&try_line, px) > width && !line.is_empty() {
            out.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = try_line;
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

/// `text` cut at the end to fit `width` pixels ("…").
pub(super) fn clip_to(tc: &TextCache, text: &str, px: f32, width: f32) -> String {
    if tc.width(text, px) <= width {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let head = |n: usize| {
        let mut t: String = chars[..n].iter().collect();
        t.push('…');
        t
    };
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if tc.width_raw(&head(mid), px) <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    head(lo)
}

/// `text` in rows that fit `width` (as `measure` measures): broken between words, and inside a
/// word too long for a row. The rows after the first fit `width - indent` (they are drawn
/// indented, under the first).
pub fn wrap_rows(
    text: &str,
    width: f32,
    indent: f32,
    measure: &impl Fn(&str) -> f32,
) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let mut cur = String::new();
    let room = |rows: &Vec<String>| {
        if rows.is_empty() {
            width
        } else {
            width - indent
        }
    };
    for word in text.split(' ') {
        let joined = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if measure(&joined) <= room(&rows) {
            cur = joined;
            continue;
        }
        if !cur.is_empty() {
            rows.push(std::mem::take(&mut cur));
        }
        // the word alone, cut where a row is full (a long link, a run of letters)
        let mut w: Vec<char> = word.chars().collect();
        while w.len() > 1 && measure(&w.iter().collect::<String>()) > room(&rows) {
            let mut k = w.len() - 1;
            while k > 1 && measure(&w[..k].iter().collect::<String>()) > room(&rows) {
                k -= 1;
            }
            rows.push(w[..k].iter().collect());
            w.drain(..k);
        }
        cur = w.into_iter().collect();
    }
    if !cur.is_empty() || rows.is_empty() {
        rows.push(cur);
    }
    rows
}

/// `text` cut at the start to fit (the end of what is being typed stays visible).
pub(super) fn clip_left(tc: &TextCache, text: &str, px: f32, width: f32) -> String {
    let mut t: Vec<char> = text.chars().collect();
    while t.len() > 1 && tc.width(&t.iter().collect::<String>(), px) > width {
        t.remove(0);
    }
    t.into_iter().collect()
}

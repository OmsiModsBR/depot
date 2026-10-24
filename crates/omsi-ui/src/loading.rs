//! The loading screen: the player's own screenshots of the map as a slow slideshow behind a
//! progress bar and the bus and map being loaded. Drawn with `Ui`; the caller loads the
//! pictures into the GPU (`Gpu::add_image`) and says which texture each is.
//!
//! Laid out after the loading concept (v2): the logo top left, the version top right, the
//! map, the stage and the bar bottom left, and a "Loading" card with the bus and the map
//! bottom right, all over the full-screen picture under a veil that darkens its top and its
//! foot. Measures are in design units (the 1672x941 concept; the caller's scale fits it to the
//! window), so a wider window only moves the card further right.
//!
//! Nothing here reads a file or starts a thread: `Slideshow` is arithmetic over time and
//! `layout` is arithmetic over the text's widths, so both are tested without a window.

use crate::paint::Align;
use crate::theme::accent;
use crate::widgets::Ui;
use crate::{Color, Rect, Weight};
use glam::Vec2;

/// How long one picture fades into the next, in seconds.
pub const FADE_SECONDS: f32 = 3.5;
/// How long a picture stays whole before it fades, in seconds.
pub const HOLD_SECONDS: f32 = 6.0;

// the colours of the v5 look (`omsi-launcher-ui`'s `look`), and the concept's own
const BG: Color = Color::hex(0x101215);
const LINE: Color = Color::hex(0x2b2f37);
const FG: Color = Color::hex(0xe9eaec);
const MUTED: Color = Color::hex(0x9aa0aa);
// The pipeline blends in linear light, the concept (a web page) in sRGB: a see-through colour
// here shows much more of what is under it than the same opacity there. The opacities below
// are the concept's carried over so the screen looks as it does (`over_black` for the veils).
/// The card: near black, a little see-through (the concept's 88%); its header band lighter.
const CARD: Color = Color::rgba(15, 16, 20, 0.96);
const CARD_HEAD: Color = Color::rgba(30, 34, 41, 0.97);
const THUMB: Color = Color::hex(0x2a2f38);
/// The author's text in the card, and the stage lines under the heading.
const BODY: Color = Color::hex(0xb9bec7);
const SOFT: Color = Color::hex(0xc2c7cf);
/// The empty part of the bar: white over the picture (the concept's 18%), so it reads on
/// any photo.
const TRACK: Color = Color::rgba(255, 255, 255, 0.04);
/// The version: white at the concept's 67%.
const VERSION: Color = Color::rgba(255, 255, 255, 0.45);
/// The soft shadow under the text straight over the picture.
const SHADOW: Color = Color::rgba(0, 0, 0, 0.7);
/// The text is drawn this much larger than its nominal size (the concept's face is wider).
const TS: f32 = 1.16;

/// Space between the screen's edge and what is drawn near it.
pub const MARGIN: f32 = 40.0;
pub const CARD_W: f32 = 400.0;
/// The status block (heading, stage, bar) at its widest.
pub const STATUS_W: f32 = 640.0;
/// The header band of the card, its rule included.
const HEAD_H: f32 = 34.0;
const PAD_X: f32 = 18.0;
const PAD_Y: f32 = 14.0;
const THUMB_W: f32 = 96.0;
const THUMB_H: f32 = 60.0;
/// Between the thumbnail and the text.
const THUMB_GAP: f32 = 14.0;
/// The lines of a card's text: the title, the line under it, the last line.
const TITLE_LH: f32 = 23.0;
const SUB_LH: f32 = 14.5;

/// Which pictures show now: `current`, and `next` fading in over it by `mix` (0 to 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub current: usize,
    pub next: Option<usize>,
    pub mix: f32,
}

/// The slideshow over `count` pictures.
#[derive(Debug, Clone)]
pub struct Slideshow {
    count: usize,
    t: f32,
}

impl Slideshow {
    pub fn new(count: usize) -> Slideshow {
        Slideshow { count, t: 0.0 }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// More pictures arrived (or fewer are kept): the show goes on from where it is.
    pub fn set_count(&mut self, count: usize) {
        self.count = count;
        let round = count as f32 * (HOLD_SECONDS + FADE_SECONDS);
        if round > 0.0 {
            self.t %= round;
        } else {
            self.t = 0.0;
        }
    }

    /// Move on by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        if self.count > 1 {
            self.t = (self.t + dt.max(0.0)) % (self.count as f32 * (HOLD_SECONDS + FADE_SECONDS));
        }
    }

    /// What to draw now. With no picture there is no frame (the default background shows);
    /// with one, it stays.
    pub fn frame(&self) -> Option<Frame> {
        match self.count {
            0 => None,
            1 => Some(Frame {
                current: 0,
                next: None,
                mix: 0.0,
            }),
            n => {
                let slot = HOLD_SECONDS + FADE_SECONDS;
                let current = ((self.t / slot) as usize).min(n - 1);
                let phase = self.t - current as f32 * slot;
                if phase < HOLD_SECONDS {
                    Some(Frame {
                        current,
                        next: None,
                        mix: 0.0,
                    })
                } else {
                    let x = ((phase - HOLD_SECONDS) / FADE_SECONDS).clamp(0.0, 1.0);
                    // (eased, so the change starts and ends slowly)
                    let mix = x * x * (3.0 - 2.0 * x);
                    Some(Frame {
                        current,
                        next: Some((current + 1) % n),
                        mix,
                    })
                }
            }
        }
    }
}

/// A picture in the GPU: its texture index and its width over its height.
#[derive(Debug, Clone, Copy)]
pub struct Slide {
    pub tex: usize,
    pub aspect: f32,
}

/// The bus or map block of the loading card.
pub struct Card<'a> {
    pub title: &'a str,
    /// The line under the title, in the accent (the bus's make, the map's folder).
    pub sub: &'a str,
    /// A last, quieter line ("Skin: Default paint"); empty for none.
    pub detail: &'a str,
    /// The icon in the thumbnail (a Material Symbol: `directions_bus`, `map`), drawn until
    /// there is a picture.
    pub icon: &'a str,
    /// The thumbnail's picture, its texture and width over height (the map's own picture).
    pub thumb: Option<(usize, f32)>,
}

/// What the screen says. All text is the caller's: the activity is translated here; the
/// heading and the stage come translated (their values are filled in), and names and the
/// authors' texts are drawn as they are.
pub struct View<'a> {
    pub version: &'a str,
    /// "Example map · Line 50 · 08:00"
    pub heading: &'a str,
    /// What is being done now ("Loading map tiles (first area) · 62 of 100").
    pub stage: &'a str,
    /// Under the bar, left of the percentage ("Preparing the world").
    pub activity: &'a str,
    /// Done over total, 0 to 1.
    pub progress: f32,
    /// The logo (texture, height over width).
    pub logo: Option<(usize, f32)>,
    pub bus: Option<Card<'a>>,
    pub map: Option<Card<'a>>,
    /// The screenshots, in the order they were taken.
    pub slides: &'a [Slide],
}

/// One block of the card, laid out: its box, the thumbnail and the text column.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub rect: Rect,
    pub thumb: Rect,
    /// The text column: left edge, top and width.
    pub text: Rect,
}

/// Where everything goes on a screen of `ui.size` design units.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub logo: Rect,
    /// The version's line: right-aligned to its right edge.
    pub version: Rect,
    /// The status block, from the heading's top to the percentage's foot.
    pub status: Rect,
    pub bar: Rect,
    /// The card and its blocks (none without a bus or a map).
    pub card: Option<Rect>,
    pub blocks: Vec<Block>,
}

/// Lay the screen out for `view`.
pub fn layout(ui: &Ui, view: &View) -> Layout {
    let (w, h) = (ui.size.x, ui.size.y);
    // the logo: its letters (the picture has a thin margin round them) start near the
    // concept's x 47 and are about 248 wide, from y 52
    let logo_w = 254.0;
    let logo_h = view.logo.map_or(39.0, |(_, a)| logo_w * a);
    let logo = Rect::new(MARGIN + 4.0, 51.0, logo_w, logo_h);
    let version = Rect::new(w * 0.5, MARGIN, w * 0.5 - MARGIN, 14.0);

    // the card, from the foot up, each block as tall as its text
    let cards: Vec<&Card> = view.bus.iter().chain(view.map.iter()).collect();
    let text_w = CARD_W - 1.0 - PAD_X - THUMB_W - THUMB_GAP - PAD_X - 1.0;
    let mut blocks = Vec::new();
    let mut y = 0.0;
    for c in &cards {
        let mut text_h = TITLE_LH;
        if !c.sub.trim().is_empty() {
            text_h += 4.0 + SUB_LH;
        }
        if !c.detail.trim().is_empty() {
            text_h += 4.0 + SUB_LH;
        }
        let bh = (PAD_Y * 2.0 + text_h.max(THUMB_H)).round();
        blocks.push(Block {
            rect: Rect::new(0.0, y, CARD_W, bh),
            thumb: Rect::new(1.0 + PAD_X, y + PAD_Y, THUMB_W, THUMB_H),
            text: Rect::new(1.0 + PAD_X + THUMB_W + THUMB_GAP, y + PAD_Y, text_w, text_h),
        });
        // (the rule between two blocks)
        y += bh + 1.0;
    }
    let card = (!blocks.is_empty()).then(|| {
        let ch = 1.0 + HEAD_H + y;
        let r = Rect::new(w - MARGIN - CARD_W, h - MARGIN - ch, CARD_W, ch);
        for b in &mut blocks {
            let dy = r.y + 1.0 + HEAD_H;
            b.rect = Rect::new(r.x, b.rect.y + dy, r.w, b.rect.h);
            b.thumb = Rect::new(r.x + b.thumb.x, b.thumb.y + dy, b.thumb.w, b.thumb.h);
            b.text = Rect::new(r.x + b.text.x, b.text.y + dy, b.text.w, b.text.h);
        }
        r
    });

    // the status block, as wide as the concept's unless the card leaves less room
    let room = match card {
        Some(c) => c.x - MARGIN - MARGIN,
        None => w - 2.0 * MARGIN,
    };
    let sw = STATUS_W.min(room).max(240.0);
    let bottom = h - MARGIN;
    let status = Rect::new(MARGIN, bottom - 78.5, sw, 78.5);
    let bar = Rect::new(MARGIN, bottom - 29.5, sw, 8.0);
    Layout {
        logo,
        version,
        status,
        bar,
        card,
        blocks,
    }
}

/// Draw the loading screen over the whole of `ui`'s window (design units: the window's size
/// divided by the scale the caller chose).
pub fn draw(ui: &mut Ui, view: &View, show: &Slideshow) {
    let (w, h) = (ui.size.x, ui.size.y);
    let full = Rect::new(0.0, 0.0, w, h);
    let l = layout(ui, view);
    ui.p().rect(full, BG);
    // the pictures, or the default background when the player has made none
    match show.frame() {
        Some(f) if f.current < view.slides.len() => {
            let a = view.slides[f.current];
            ui.image_cover(full, a.tex, a.aspect, 1.0);
            if let (Some(n), true) = (f.next, f.mix > 0.0) {
                if let Some(b) = view.slides.get(n) {
                    ui.image_cover(full, b.tex, b.aspect, f.mix);
                }
            }
        }
        _ => default_background(ui, w, h),
    }
    // the vignette: the edges darkened all round, so a screenshot of the game does not read
    // as the game itself
    vignette(ui, full);
    // the veil: the top darkened under the logo, the foot under the text
    veil(ui, Rect::new(0.0, 0.0, w, h * 0.22), 0.55, 0.0);
    veil(ui, Rect::new(0.0, h * 0.55, w, h * 0.45), 0.0, 0.78);

    if let Some((tex, _)) = view.logo {
        ui.image(l.logo, tex, 0.0);
    }
    spaced(
        ui,
        view.version,
        l.version.right(),
        l.version.y + 7.0,
        11.0,
        Weight::Regular,
        VERSION,
        Align::Right,
        1.0,
        true,
    );

    // the status block: what loads, how far, what is being done
    let s = l.status;
    let line = Line {
        x: s.x,
        max: s.w,
        shadow: true,
    };
    line.draw(ui, view.heading, s.y + 9.0, 15.0, Weight::Bold, FG, false);
    line.draw(
        ui,
        view.stage,
        s.y + 29.5,
        12.5,
        Weight::Regular,
        SOFT,
        false,
    );
    ui.p().rect(l.bar, TRACK);
    let done = view.progress.clamp(0.0, 1.0);
    if done > 0.0 {
        ui.p().rect(
            Rect::new(l.bar.x, l.bar.y, (l.bar.w * done).max(1.0), l.bar.h),
            accent(),
        );
    }
    let pct = format!("{}%", (done * 100.0).round() as u32);
    let pct_cy = s.bottom() - 7.25;
    let pw = Line {
        x: l.bar.right(),
        max: s.w,
        shadow: true,
    }
    .right(ui, &pct, pct_cy, 12.0, SOFT);
    Line {
        x: s.x,
        max: (s.w - pw - 16.0).max(0.0),
        shadow: true,
    }
    .draw(ui, view.activity, pct_cy, 12.0, Weight::Regular, SOFT, true);

    // the card: the bus and the map being loaded
    let Some(c) = l.card else {
        return;
    };
    let cards: Vec<&Card> = view.bus.iter().chain(view.map.iter()).collect();
    ui.solid(c);
    ui.p().rect(c, CARD);
    ui.p().rect(
        Rect::new(c.x + 1.0, c.y + 1.0, c.w - 2.0, HEAD_H - 1.0),
        CARD_HEAD,
    );
    ui.p().rounded_border(c, 0.0, 1.0, LINE);
    ui.p()
        .rect(Rect::new(c.x + 1.0, c.y + HEAD_H, c.w - 2.0, 1.0), LINE);
    spaced(
        ui,
        &crate::tr("Loading"),
        c.x + 1.0 + PAD_X,
        c.y + 1.0 + (HEAD_H - 1.0) * 0.5,
        11.0,
        Weight::Bold,
        MUTED,
        Align::Left,
        2.0,
        false,
    );
    for (i, (card, b)) in cards.iter().zip(&l.blocks).enumerate() {
        if i > 0 {
            ui.p()
                .rect(Rect::new(c.x + 1.0, b.rect.y - 1.0, c.w - 2.0, 1.0), LINE);
        }
        ui.p().rect(b.thumb, THUMB);
        match card.thumb {
            Some((tex, aspect)) => {
                ui.push_clip(b.thumb, 0.0);
                ui.image_cover(b.thumb, tex, aspect, 1.0);
                ui.pop_clip();
            }
            None => ui.icon(card.icon, b.thumb.center(), 26.0, MUTED.alpha(0.8)),
        }
        let t = Line {
            x: b.text.x,
            max: b.text.w,
            shadow: false,
        };
        let mut y = b.text.y;
        t.draw(
            ui,
            card.title,
            y + TITLE_LH * 0.5,
            19.0,
            Weight::Bold,
            FG,
            false,
        );
        y += TITLE_LH;
        if !card.sub.trim().is_empty() {
            y += 4.0;
            t.draw(
                ui,
                card.sub,
                y + SUB_LH * 0.5,
                12.0,
                Weight::Regular,
                accent(),
                false,
            );
            y += SUB_LH;
        }
        if !card.detail.trim().is_empty() {
            y += 4.0;
            t.draw(
                ui,
                card.detail,
                y + SUB_LH * 0.5,
                12.0,
                Weight::Regular,
                BODY,
                false,
            );
        }
    }
}

/// The opacity of black that darkens a colour here as much as black at `a` does in sRGB
/// (the concept's): what is left of the colour, (1 - a) in sRGB, is about (1 - a)^2.2 in
/// linear light.
pub fn over_black(a: f32) -> f32 {
    1.0 - (1.0 - a.clamp(0.0, 1.0)).powf(2.2)
}

/// The strength of the vignette at the very edge (the concept's black opacity), and how far
/// in it reaches as a share of the screen's width.
const VIGNETTE: f32 = 0.62;
const VIGNETTE_REACH: f32 = 0.24;

/// The edges of `r` darkened towards black, the corners most (the sides overlap there).
fn vignette(ui: &mut Ui, r: Rect) {
    const BANDS: usize = 4;
    let reach = r.w * VIGNETTE_REACH;
    // (the same reach on every side, so the sides of a wide screen are not darker)
    for k in 0..BANDS {
        let (t0, t1) = (k as f32 / BANDS as f32, (k + 1) as f32 / BANDS as f32);
        let a = |t: f32| Color::BLACK.alpha(over_black(VIGNETTE * (1.0 - t) * (1.0 - t)));
        let (x0, x1) = (reach * t0, reach * t1);
        let p = ui.p();
        p.gradient_h(Rect::new(r.x + x0, r.y, x1 - x0, r.h), a(t0), a(t1));
        p.gradient_h(Rect::new(r.right() - x1, r.y, x1 - x0, r.h), a(t1), a(t0));
        p.gradient(Rect::new(r.x, r.y + x0, r.w, x1 - x0), a(t0), a(t1));
        p.gradient(Rect::new(r.x, r.bottom() - x1, r.w, x1 - x0), a(t1), a(t0));
    }
}

/// A black veil over `r`, from the concept's opacity `top` to `bottom` (a few bands, so the
/// ramp darkens as the concept's does and not all at its dark end).
fn veil(ui: &mut Ui, r: Rect, top: f32, bottom: f32) {
    const BANDS: usize = 4;
    let p = ui.p();
    for k in 0..BANDS {
        let (t0, t1) = (k as f32 / BANDS as f32, (k + 1) as f32 / BANDS as f32);
        let a = |t: f32| Color::BLACK.alpha(over_black(top + (bottom - top) * t));
        p.gradient(
            Rect::new(r.x, r.y + r.h * t0, r.w, r.h * (t1 - t0)),
            a(t0),
            a(t1),
        );
    }
}

/// The background when the player has no screenshot of the map: the concept's evening sky
/// over a road running to the horizon.
fn default_background(ui: &mut Ui, w: f32, h: f32) {
    let horizon = h * 0.46;
    let p = ui.p();
    p.gradient(
        Rect::new(0.0, 0.0, w, horizon),
        Color::hex(0x2b3a4d),
        Color::hex(0x5d7490),
    );
    p.gradient(
        Rect::new(0.0, horizon, w, h - horizon),
        Color::hex(0x3a3d40),
        Color::hex(0x1c1e20),
    );
    let road = Color::hex(0x4a4d50);
    let (far, near) = (road.alpha(0.55), road.alpha(0.4));
    let (tl, tr) = (Vec2::new(w * 0.315, horizon), Vec2::new(w * 0.696, horizon));
    let (bl, br) = (Vec2::new(w * 0.10, h), Vec2::new(w * 0.80, h));
    p.tri(tl, tr, br, far, far, near);
    p.tri(tl, br, bl, far, near, near);
}

/// Where a line of text goes: its left (or right) edge, how wide it may be, and whether a
/// soft shadow lifts it off the picture (the text straight over a photo).
struct Line {
    x: f32,
    max: f32,
    shadow: bool,
}

impl Line {
    /// `s` left at `x`, centred on `cy`, cut with an ellipsis to `max`; translated when
    /// `translate` (the interface's own text). Returns its width.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        ui: &mut Ui,
        s: &str,
        cy: f32,
        px: f32,
        weight: Weight,
        c: Color,
        translate: bool,
    ) -> f32 {
        self.put(ui, s, cy, px, weight, c, translate, Align::Left)
    }

    /// `s` (a number: never translated) ending at `x`. Returns its width.
    fn right(&self, ui: &mut Ui, s: &str, cy: f32, px: f32, c: Color) -> f32 {
        self.put(ui, s, cy, px, Weight::Regular, c, false, Align::Right)
    }

    #[allow(clippy::too_many_arguments)]
    fn put(
        &self,
        ui: &mut Ui,
        s: &str,
        cy: f32,
        px: f32,
        weight: Weight,
        c: Color,
        translate: bool,
        align: Align,
    ) -> f32 {
        let px = px * TS;
        let s = if translate {
            crate::tr(s)
        } else {
            std::borrow::Cow::Borrowed(s)
        };
        let s = fit(ui, &s, px, weight, self.max);
        if s.is_empty() {
            return 0.0;
        }
        let base = cy + ui.fonts.cap_height(px, weight) * 0.5;
        if self.shadow {
            ui.text_as_is(&s, Vec2::new(self.x, base + 1.0), px, weight, SHADOW, align);
        }
        ui.text_as_is(&s, Vec2::new(self.x, base), px, weight, c, align)
    }
}

/// Small letter-spaced capitals centred on `cy` (the version, the card's header), drawn as
/// `s` is (the caller translates the interface's own words).
#[allow(clippy::too_many_arguments)]
fn spaced(
    ui: &mut Ui,
    s: &str,
    x: f32,
    cy: f32,
    px: f32,
    weight: Weight,
    c: Color,
    align: Align,
    tracking: f32,
    shadow: bool,
) {
    let px = px * TS;
    let s = s.to_uppercase();
    let base = cy + ui.fonts.cap_height(px, weight) * 0.5;
    if shadow {
        let at = Vec2::new(x, base + 1.0);
        let shade = SHADOW;
        ui.text_spaced(&s, at, px, weight, shade, align, tracking);
    }
    ui.text_spaced(&s, Vec2::new(x, base), px, weight, c, align, tracking);
}

/// A picture drawn on black (the wordmark: white and orange letters on a black field) made
/// see-through: each pixel's brightest channel becomes its opacity and its colour is what it
/// was before the black was under it. Over a photo the letters then stand alone, with no
/// black box round them; over black it looks as it did.
pub fn unmatte_black(rgba: &mut [u8]) {
    // (the faint noise of the field stays clear, the letters' own colours stay whole)
    const LO: u32 = 12;
    const HI: u32 = 240;
    for px in rgba.chunks_exact_mut(4) {
        let a = px[0].max(px[1]).max(px[2]) as u32;
        if a <= LO {
            px[3] = 0;
            continue;
        }
        for c in &mut px[..3] {
            *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
        }
        let k = ((a - LO) * 255 / (HI - LO)).min(255);
        px[3] = ((px[3] as u32 * k + 127) / 255) as u8;
    }
}

/// The longest start of `s` that fits in `max`, with an ellipsis when cut (measured as it
/// is: `s` is already in the language shown).
fn fit(ui: &Ui, s: &str, px: f32, weight: Weight, max: f32) -> String {
    let width = |t: &str| ui.fonts.width_as_is(t, px, weight);
    if width(s) <= max {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let cut = |n: usize| chars[..n].iter().collect::<String>().trim_end().to_string() + "\u{2026}";
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if width(&cut(mid)) <= max {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        String::new()
    } else {
        cut(lo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(count: usize, t: f32) -> Option<Frame> {
        let mut s = Slideshow::new(count);
        s.update(t);
        s.frame()
    }

    #[test]
    fn each_picture_is_held_six_seconds_and_fades_over_three_and_a_half() {
        assert_eq!(HOLD_SECONDS, 6.0);
        assert_eq!(FADE_SECONDS, 3.5);
    }

    #[test]
    fn pictures_arriving_keep_the_show_going() {
        let mut show = Slideshow::new(1);
        show.update(100.0);
        assert_eq!(show.frame().unwrap().current, 0);
        show.set_count(3);
        show.update(HOLD_SECONDS + FADE_SECONDS + 1.0);
        assert_eq!(show.frame().unwrap().current, 1);
        show.set_count(0);
        assert!(show.frame().is_none());
    }

    #[test]
    fn no_pictures_means_the_default_background_and_one_picture_stays() {
        assert_eq!(at(0, 100.0), None);
        let one = at(1, 100.0).unwrap();
        assert_eq!((one.current, one.next, one.mix), (0, None, 0.0));
    }

    #[test]
    fn a_picture_is_held_then_fades_into_the_next_over_the_fade_time() {
        let held = at(3, HOLD_SECONDS - 0.1).unwrap();
        assert_eq!((held.current, held.next), (0, None));
        let start = at(3, HOLD_SECONDS + 0.001).unwrap();
        assert_eq!(start.next, Some(1));
        assert!(start.mix < 0.01);
        let mid = at(3, HOLD_SECONDS + FADE_SECONDS * 0.5).unwrap();
        assert!((mid.mix - 0.5).abs() < 0.01);
        // the next picture is whole as the fade ends
        let after = at(3, HOLD_SECONDS + FADE_SECONDS + 0.01).unwrap();
        assert_eq!((after.current, after.next), (1, None));
    }

    #[test]
    fn the_last_picture_fades_into_the_first_and_time_wraps() {
        let slot = HOLD_SECONDS + FADE_SECONDS;
        let last = at(3, 2.0 * slot + HOLD_SECONDS + 1.0).unwrap();
        assert_eq!((last.current, last.next), (2, Some(0)));
        let wrapped = at(3, 3.0 * slot + 0.5).unwrap();
        assert_eq!(wrapped.current, 0);
    }

    fn view<'a>(long: bool) -> View<'a> {
        View {
            version: "example version",
            heading: if long {
                "An example map with a very long name indeed \u{b7} Line 50 \u{b7} 08:00 \
                 and more words that do not fit"
            } else {
                "Example map \u{b7} Line 50 \u{b7} 08:00"
            },
            stage: "Loading map tiles (first area) \u{b7} 62 of 100",
            activity: "Preparing the world",
            progress: 0.62,
            logo: Some((1, 305.0 / 2188.0)),
            bus: Some(Card {
                title: if long {
                    "Example bus A with a name far too long for the card"
                } else {
                    "Example bus A"
                },
                sub: "Example Maker",
                detail: "Skin: Default paint",
                icon: "directions_bus",
                thumb: None,
            }),
            map: Some(Card {
                title: "Example map",
                sub: "Example City",
                detail: "",
                icon: "map",
                thumb: Some((5, 1.5)),
            }),
            slides: &[],
        }
    }

    fn inside(r: Rect, outer: Rect) -> bool {
        r.x >= outer.x - 0.01
            && r.y >= outer.y - 0.01
            && r.right() <= outer.right() + 0.01
            && r.bottom() <= outer.bottom() + 0.01
    }

    fn overlap(a: Rect, b: Rect) -> bool {
        a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
    }

    /// The design sizes the screen is drawn at: 1920x1080, 1672x941 and 2560x1080 at the
    /// game's scale (the 1672x941 design fitted to the window), and half as large again.
    fn sizes() -> Vec<Vec2> {
        let mut out = Vec::new();
        for (w, h) in [(1920.0f32, 1080.0f32), (1672.0, 941.0), (2560.0, 1080.0)] {
            let natural = (w / 1672.0).min(h / 941.0);
            for k in [1.0, 1.5] {
                out.push(Vec2::new(w, h) / (natural * k));
            }
        }
        out
    }

    #[test]
    fn nothing_overlaps_or_leaves_the_screen_at_the_three_sizes_and_both_scales() {
        let mut ui = Ui::new();
        for size in sizes() {
            ui.begin(size, 1.0, 0.0);
            for long in [false, true] {
                let v = view(long);
                let l = layout(&ui, &v);
                let screen = Rect::new(0.0, 0.0, size.x, size.y).inset(MARGIN - 0.01);
                let card = l.card.expect("a card");
                for r in [l.status, l.bar, card, l.version] {
                    assert!(inside(r, screen), "{r:?} leaves {size:?}");
                }
                assert!(
                    !overlap(l.status, card),
                    "status under the card at {size:?}"
                );
                assert!(card.x - l.status.right() >= MARGIN - 0.01, "{size:?}");
                assert!(
                    !overlap(l.logo, card) && l.logo.bottom() < card.y,
                    "{size:?}"
                );
                assert!(inside(l.bar, l.status));
                // every block inside the card, one under the other, its text in its box
                let mut y = card.y + HEAD_H;
                for b in &l.blocks {
                    assert!(inside(b.rect, card), "{b:?}");
                    assert!(b.rect.y >= y, "blocks overlap at {size:?}");
                    y = b.rect.bottom();
                    assert!(inside(b.thumb, b.rect) && inside(b.text, b.rect), "{b:?}");
                    assert!(!overlap(b.thumb, b.text));
                }
                assert!(y <= card.bottom() + 0.01);
            }
        }
    }

    #[test]
    fn the_status_block_keeps_the_concept_width_unless_the_card_needs_the_room() {
        let mut ui = Ui::new();
        ui.begin(Vec2::new(1672.0, 941.0), 1.0, 0.0);
        let l = layout(&ui, &view(false));
        assert_eq!(l.status.w, STATUS_W);
        assert_eq!(l.card.unwrap().w, CARD_W);
        assert_eq!(l.card.unwrap().right(), 1672.0 - MARGIN);
        // the concept's foot: the bar 8 high, its bottom 61.5 above the screen's foot
        assert_eq!((l.bar.h, l.bar.bottom()), (8.0, 941.0 - 61.5));
        ui.begin(Vec2::new(1115.0, 627.0), 1.0, 0.0);
        let l = layout(&ui, &view(false));
        assert!(l.status.w < STATUS_W);
        // with no card the block keeps its width on any screen
        let mut v = view(false);
        v.bus = None;
        v.map = None;
        let l = layout(&ui, &v);
        assert_eq!((l.status.w, l.card), (STATUS_W, None));
    }

    #[test]
    fn a_card_with_only_a_title_is_shorter_than_one_with_text() {
        let mut ui = Ui::new();
        ui.begin(Vec2::new(1672.0, 941.0), 1.0, 0.0);
        let mut v = view(false);
        v.bus = Some(Card {
            title: "MAN_SD200",
            sub: "",
            detail: "",
            icon: "directions_bus",
            thumb: None,
        });
        let l = layout(&ui, &v);
        assert_eq!(l.blocks[0].rect.h, PAD_Y * 2.0 + THUMB_H);
        // a block with its make and no last line is as tall as the thumbnail asks for
        assert_eq!(
            l.blocks[1].rect.h,
            PAD_Y * 2.0 + THUMB_H.max(TITLE_LH + 4.0 + SUB_LH)
        );
        // the bus's, with the skin line too, has one line more
        let mut v = view(false);
        v.bus.as_mut().unwrap().sub = "Example Maker";
        assert!(layout(&ui, &v).blocks[0].rect.h >= l.blocks[1].rect.h);
    }

    #[test]
    fn the_logo_loses_its_black_field_and_keeps_its_colours() {
        let mut px = vec![
            0, 0, 0, 255, // the field
            255, 255, 255, 255, // a white letter
            240, 120, 40, 255, // an orange one
            120, 60, 20, 255, // the orange half over black (an edge)
        ];
        unmatte_black(&mut px);
        assert_eq!(px[3], 0);
        assert_eq!(&px[4..8], &[255, 255, 255, 255]);
        assert_eq!(&px[8..12], &[255, 128, 43, 255]);
        assert_eq!(&px[12..15], &[255, 128, 43]);
        assert_eq!(px[15], 120);
        // the field's faint noise is clear too
        let mut noise = vec![9, 3, 0, 255];
        unmatte_black(&mut noise);
        assert_eq!(noise[3], 0);
    }

    #[test]
    fn the_screen_draws_with_and_without_pictures() {
        let mut ui = Ui::new();
        for slides in [
            &[][..],
            &[
                Slide {
                    tex: 3,
                    aspect: 1.5,
                },
                Slide {
                    tex: 4,
                    aspect: 1.5,
                },
            ][..],
        ] {
            ui.begin(Vec2::new(1672.0, 941.0), 1.0, 0.0);
            let mut v = view(true);
            v.slides = slides;
            let mut show = Slideshow::new(slides.len());
            show.update(HOLD_SECONDS + 1.0);
            draw(&mut ui, &v, &show);
            let (layers, verts, ranges) = ui.finish();
            assert!(!verts.is_empty() && !layers.is_empty());
            let textures: Vec<usize> = ranges.iter().map(|(_, t)| *t).collect();
            // the pictures are drawn only when there are some (the fade draws both)
            assert_eq!(textures.contains(&3), !slides.is_empty());
            assert_eq!(textures.contains(&4), !slides.is_empty());
            assert!(textures.contains(&1), "the logo");
            // the map card's own picture, in its thumbnail
            assert!(textures.contains(&5));
        }
    }
}

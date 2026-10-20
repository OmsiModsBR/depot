//! The loading screen: the player's own screenshots of the map as a slow slideshow behind a
//! progress bar and the bus and map being loaded. Drawn with `Ui`; the caller loads the
//! pictures into the GPU (`Gpu::add_image`) and says which texture each is.
//!
//! Nothing here reads a file or starts a thread: `Slideshow` is arithmetic over time, so it
//! is tested without a window.

use crate::paint::Align;
use crate::widgets::Ui;
use crate::{Color, Rect, Weight};
use glam::Vec2;

/// How long one picture fades into the next, in seconds.
pub const FADE_SECONDS: f32 = 3.5;
/// How long a picture stays whole before it fades, in seconds.
pub const HOLD_SECONDS: f32 = 6.0;

const BG: Color = Color::hex(0x101215);
const PANEL: Color = Color::hex(0x171a1f);
const PANEL_2: Color = Color::hex(0x1e2229);
const LINE: Color = Color::hex(0x2b2f37);
const FG: Color = Color::hex(0xe9eaec);
const MUTED: Color = Color::hex(0x9aa0aa);
const ACCENT: Color = Color::hex(0xf0a30a);
/// Panels are 8% see-through.
const PANEL_ALPHA: f32 = 0.92;
/// The text is drawn this much larger than its nominal size (the concept's face is wider).
const TS: f32 = 1.16;

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

/// The bus or map card of the loading panel.
pub struct Card<'a> {
    pub title: &'a str,
    pub sub: &'a str,
    /// The author's text, plain, cut to what fits.
    pub description: &'a str,
}

/// What the screen says. All text is the caller's, already in the player's language.
pub struct View<'a> {
    pub version: &'a str,
    /// "Example map · Line 50 · 08:00"
    pub heading: &'a str,
    /// What is being done now ("Loading map tiles (first area) · 62 of 100").
    pub stage: &'a str,
    /// Done over total, 0 to 1.
    pub progress: f32,
    /// The logo (texture, height over width).
    pub logo: Option<(usize, f32)>,
    pub bus: Option<Card<'a>>,
    pub map: Option<Card<'a>>,
    /// The screenshots, in the order they were taken.
    pub slides: &'a [Slide],
}

fn text(ui: &mut Ui, s: &str, x: f32, cy: f32, px: f32, weight: Weight, c: Color) {
    let px = px * TS;
    let w = ui.width(s, px, weight);
    ui.text_in(
        s,
        Rect::new(x, cy - px, w + 2.0, px * 2.0),
        px,
        weight,
        c,
        Align::Left,
    );
}

/// Draw the loading screen over the whole of `ui`'s window (design units: the window's size
/// divided by the scale the caller chose).
pub fn draw(ui: &mut Ui, view: &View, show: &Slideshow) {
    let (w, h) = (ui.size.x, ui.size.y);
    let full = Rect::new(0.0, 0.0, w, h);
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
        _ => {
            ui.p().gradient(
                Rect::new(0.0, 0.0, w, h * 0.46),
                Color::hex(0x172030),
                Color::hex(0x5d7494),
            );
            ui.p().gradient(
                Rect::new(0.0, h * 0.46, w, h * 0.54),
                Color::hex(0x3d3f42),
                Color::hex(0x1b1c1e),
            );
        }
    }
    // a veil under the text, darker towards the foot
    ui.p().gradient(
        Rect::new(0.0, h * 0.6, w, h * 0.4),
        Color::BLACK.alpha(0.0),
        Color::BLACK.alpha(0.55),
    );
    if let Some((tex, aspect)) = view.logo {
        ui.image(Rect::new(40.0, 32.0, 260.0, 260.0 * aspect), tex, 0.0);
    }
    let vw = ui.width(view.version, 11.0 * TS, Weight::Regular);
    text(
        ui,
        &view.version.to_uppercase(),
        w - 41.0 - vw,
        47.0,
        11.0,
        Weight::Regular,
        MUTED,
    );

    text(ui, view.heading, 40.0, h - 110.0, 15.0, Weight::Bold, FG);
    text(
        ui,
        view.stage,
        40.0,
        h - 89.0,
        13.0,
        Weight::Regular,
        Color::hex(0xc9ccd2),
    );
    let bar = Rect::new(40.0, h - 70.0, 640.0, 8.0);
    ui.p().rect(bar, Color::hex(0x44464a));
    let done = view.progress.clamp(0.0, 1.0);
    ui.p()
        .rect(Rect::new(bar.x, bar.y, bar.w * done, bar.h), ACCENT);
    let pct = format!("{}%", (done * 100.0).round() as u32);
    let pw = ui.width(&pct, 12.0 * TS, Weight::Regular);
    text(
        ui,
        &pct,
        bar.right() - pw,
        h - 47.0,
        12.0,
        Weight::Regular,
        MUTED,
    );

    let cards: Vec<&Card> = view.bus.iter().chain(view.map.iter()).collect();
    if cards.is_empty() {
        return;
    }
    let p = Rect::new(
        w - 440.0,
        h - 40.0 - 37.0 - cards.len() as f32 * 115.0,
        400.0,
        37.0 + cards.len() as f32 * 115.0,
    );
    ui.solid(p);
    ui.p().rect(p, PANEL.alpha(PANEL_ALPHA));
    ui.p().rounded_border(p, 0.0, 1.0, LINE);
    text(
        ui,
        "LOADING",
        p.x + 18.0,
        p.y + 18.0,
        11.0,
        Weight::Medium,
        MUTED,
    );
    ui.p()
        .rect(Rect::new(p.x + 1.0, p.y + 35.0, p.w - 2.0, 1.0), LINE);
    for (i, c) in cards.iter().enumerate() {
        let y = p.y + 36.0 + i as f32 * 115.0;
        if i > 0 {
            ui.p().rect(Rect::new(p.x + 1.0, y, p.w - 2.0, 1.0), LINE);
        }
        ui.p()
            .rect(Rect::new(p.x + 19.0, y + 15.0, 96.0, 60.0), PANEL_2);
        let tx = p.x + 129.0;
        text(ui, c.title, tx, y + 26.0, 19.0, Weight::Bold, FG);
        text(ui, c.sub, tx, y + 49.0, 12.0, Weight::Regular, ACCENT);
        let (w, px) = (p.w - 129.0 - 16.0, 13.0 * TS);
        let text = fit_lines(ui, c.description, w, px, 2);
        ui.paragraph(
            &text,
            Vec2::new(tx, y + 72.0),
            w,
            px,
            Weight::Regular,
            MUTED,
        );
    }
}

/// `text` cut at a word, with an ellipsis, so that it wraps to at most `lines` lines of `width`.
fn fit_lines(ui: &Ui, text: &str, width: f32, px: f32, lines: usize) -> String {
    let max_h = px * 1.38 * lines as f32 + 0.5;
    if ui.paragraph_height(text, width, px, Weight::Regular) <= max_h {
        return text.to_string();
    }
    let mut words: Vec<&str> = text.split_whitespace().collect();
    while !words.is_empty() {
        words.pop();
        let cut = format!("{}\u{2026}", words.join(" "));
        if ui.paragraph_height(&cut, width, px, Weight::Regular) <= max_h {
            return cut;
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
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

    use super::*;

    fn at(count: usize, t: f32) -> Option<Frame> {
        let mut s = Slideshow::new(count);
        s.update(t);
        s.frame()
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
}

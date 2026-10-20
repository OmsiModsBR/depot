//! Small pieces of the v5 look the pages share: cards, status dots, labels. They draw with
//! the colours of `look` over `omsi_ui`'s `Ui`.

use crate::look::*;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::widgets::Ui;
use omsi_ui::{Color, Rect, Weight};

/// What is known about a thing of the content. The launcher never says "compatible": a thing
/// is found, missing, not checked yet, found with a warning, or in error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Found,
    Missing,
    Unverified,
    Warning,
    Error,
}

impl Status {
    pub fn color(self) -> Color {
        match self {
            Status::Found => OK,
            Status::Warning => WARN,
            Status::Missing | Status::Error => omsi_ui::theme::DANGER,
            Status::Unverified => MUTED,
        }
    }
}

/// A card: a panel with a letter-spaced title and an optional note at its right.
pub fn card(ui: &mut Ui, r: Rect, title: &str, note: Option<&str>) {
    ui.solid(r);
    ui.p().rounded(r, 8.0, PANEL);
    ui.p().rounded_border(r, 8.0, 1.0, LINE);
    spaced(
        ui,
        &omsi_ui::tr(title).to_uppercase(),
        r.x + 19.0,
        r.y + 23.0,
        13.0 * TEXT_SCALE,
        Weight::Bold,
        FG,
    );
    if let Some(n) = note {
        right(
            ui,
            n,
            r.right() - 19.0,
            r.y + 23.0,
            12.0,
            Weight::Regular,
            MUTED,
        );
    }
}

/// A card's title, letter-spaced as `card` draws it, at the top of `r` (the panel less its
/// padding, as `omsi_ui`'s `Ui::heading` takes it); returns the rest of `r` under it.
pub fn heading(ui: &mut Ui, r: Rect, title: &str) -> Rect {
    let px = 13.0 * TEXT_SCALE;
    let s = fit_spaced(ui, title, px, r.w - 1.0);
    spaced(ui, &s, r.x + 1.0, r.y + 9.0, px, Weight::Bold, FG);
    Rect::new(r.x, r.y + 26.0, r.w, (r.h - 26.0).max(0.0))
}

/// The title of a part further down a card (as the concept's "AUTHOR'S DESCRIPTION"):
/// smaller, quieter, letter-spaced, at the top of `r`.
pub fn subheading(ui: &mut Ui, r: Rect, title: &str) {
    let px = 12.0 * TEXT_SCALE;
    let s = fit_spaced(ui, title, px, r.w - 1.0);
    spaced(ui, &s, r.x + 1.0, r.y + 12.0, px, Weight::Bold, MUTED);
}

/// `title` translated, in capitals and cut with an ellipsis so that `spaced` draws it in `room`
/// points at `px` (a card's title may be a name: a device's, a line's).
fn fit_spaced(ui: &mut Ui, title: &str, px: f32, room: f32) -> String {
    let full = omsi_ui::tr(title).to_uppercase();
    let width = |ui: &mut Ui, s: &str| {
        let gap = px * 0.14;
        let mut buf = [0u8; 4];
        let w: f32 = s
            .chars()
            .map(|ch| ui.width(ch.encode_utf8(&mut buf), px, Weight::Bold) + gap)
            .sum();
        (w - gap).max(0.0)
    };
    let mut n = full.chars().count();
    let mut out = full.clone();
    while n > 1 && width(ui, &out) > room {
        n -= 1;
        out = shorten(&full, n);
    }
    out
}

/// Text left-aligned at `x`, centred on the line `cy`; returns its width.
pub fn text(ui: &mut Ui, s: &str, x: f32, cy: f32, px: f32, weight: Weight, c: Color) -> f32 {
    let px = px * TEXT_SCALE;
    let w = ui.width(s, px, weight);
    ui.text_in(
        s,
        Rect::new(x, cy - px, w + 2.0, px * 2.0),
        px,
        weight,
        c,
        Align::Left,
    );
    w
}

/// Text letter-spaced (as the concept's card titles and headings), left-aligned at `x` and
/// centred on the line `cy`, `px` already scaled; returns its width.
pub fn spaced(ui: &mut Ui, s: &str, x: f32, cy: f32, px: f32, weight: Weight, c: Color) -> f32 {
    let gap = px * 0.14;
    let mut at = x;
    let mut buf = [0u8; 4];
    for ch in s.chars() {
        let g = ch.encode_utf8(&mut buf);
        let w = ui.width(g, px, weight);
        ui.text_in(
            g,
            Rect::new(at, cy - px, w + 2.0, px * 2.0),
            px,
            weight,
            c,
            Align::Left,
        );
        at += w + gap;
    }
    (at - x - gap).max(0.0)
}

/// Text ending at `x`.
pub fn right(ui: &mut Ui, s: &str, x: f32, cy: f32, px: f32, weight: Weight, c: Color) {
    let px = px * TEXT_SCALE;
    let w = ui.width(s, px, weight);
    ui.text_in(
        s,
        Rect::new(x - w - 2.0, cy - px, w + 2.0, px * 2.0),
        px,
        weight,
        c,
        Align::Right,
    );
}

/// Text cut to `w`.
#[allow(clippy::too_many_arguments)]
pub fn clipped(ui: &mut Ui, s: &str, x: f32, cy: f32, w: f32, px: f32, weight: Weight, c: Color) {
    let px = px * TEXT_SCALE;
    ui.text_in(
        s,
        Rect::new(x, cy - px, w, px * 2.0),
        px,
        weight,
        c,
        Align::Left,
    );
}

/// A dot of a status's colour.
pub fn dot(ui: &mut Ui, x: f32, cy: f32, status: Status) {
    ui.p().circle(Vec2::new(x, cy), 4.5, status.color());
}

/// `s` shortened to at most `n` characters, with an ellipsis, on a character boundary.
pub fn shorten(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('\u{2026}');
        out
    }
}

/// The selected entry of `list`, only while the search still shows it (`None` when the
/// search matches nothing, so the detail cards say so instead of showing a hidden entry).
pub fn chosen<'a, T>(list: &'a [T], shown: &[usize], selected: usize) -> Option<&'a T> {
    if shown.contains(&selected) {
        list.get(selected)
    } else {
        None
    }
}

/// The accent button at the foot of a "Launch preparation" card, with a quiet line under it.
/// A dim one when `enabled` is false. True when it was clicked.
pub fn action(ui: &mut Ui, id: &str, b: Rect, label: &str, note: &str, enabled: bool) -> bool {
    let (hover, down, clicked) = if enabled {
        ui.interact_key(omsi_ui::widgets::id_of(id), b)
    } else {
        (false, false, false)
    };
    let fill = match (enabled, hover || down) {
        (false, _) => PANEL_2,
        (true, true) => ACCENT.mix(Color::WHITE, 0.12),
        (true, false) => ACCENT,
    };
    ui.solid(b);
    ui.p().rounded(b, 6.0, fill);
    ui.text_in(
        label,
        b,
        14.0 * TEXT_SCALE,
        Weight::Bold,
        if enabled { ON_ACCENT } else { MUTED },
        Align::Center,
    );
    clipped(
        ui,
        note,
        b.x,
        b.bottom() + 22.0,
        b.w,
        12.0,
        Weight::Regular,
        MUTED,
    );
    enabled && clicked
}

/// The page body's width in the design (the window less the sidebar and the margins).
pub const BODY_W: f32 = DESIGN_W - SIDEBAR_W - 56.0;

/// A column `w` points wide in the design, in a body `body_w` wide: as designed, or narrower in
/// a smaller window (down to 60 %), so the column between keeps room.
pub fn fit(w: f32, body_w: f32) -> f32 {
    w * (body_w / BODY_W).clamp(0.6, 1.0)
}

/// `s` cut with an ellipsis to fit `room` pixels at `px`.
pub fn fit_text(ui: &mut Ui, s: &str, px: f32, room: f32) -> String {
    let mut n = s.chars().count();
    let mut out = s.to_string();
    while n > 1 && ui.width(&out, px, Weight::Regular) > room {
        n -= 1;
        out = shorten(s, n);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::shorten;

    #[test]
    fn shorten_cuts_on_a_character_boundary() {
        assert_eq!(shorten("abc", 5), "abc");
        assert_eq!(shorten("abcdef", 4), "abc\u{2026}");
        assert_eq!(shorten("äöüäöü", 3), "äö\u{2026}");
    }
}

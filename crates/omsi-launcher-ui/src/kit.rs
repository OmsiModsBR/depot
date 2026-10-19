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
    text(
        ui,
        &title.to_uppercase(),
        r.x + 19.0,
        r.y + 23.0,
        13.0,
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

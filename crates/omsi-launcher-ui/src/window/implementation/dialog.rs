//! A dialog over the page: its frame and its foot.

use super::super::interface::output::Ui;
use omsi_ui::paint::Align;
use omsi_ui::theme::{EDGE, PANEL, RADIUS, TEXT, TEXT_DIM};
use omsi_ui::{Rect, Weight};

/// How a dialog darkens the page under it: as Windows 11's, enough to set the dialog apart,
/// the page under it still seen (at 0.62 it was almost hidden).
pub const SCRIM: omsi_ui::Color = omsi_ui::Color::rgba(0, 0, 0, 0.45);

/// The dialog's frame, `w` x `h` in the middle of the window, over the page darkened: its
/// title and a note at the right of it. Returns its inside, under the title.
pub fn dialog(ui: &mut Ui, w: f32, h: f32, title: &str, note: &str) -> Rect {
    let size = ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    ui.solid(full);
    ui.p().rect(full, SCRIM);
    let (w, h) = (w.min(size.x - 48.0), h.min(size.y - 48.0));
    let r = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
    ui.p().rounded(r, RADIUS, PANEL);
    ui.p().rounded_border(r, RADIUS, 1.0, EDGE);
    ui.text_in(
        title,
        Rect::new(r.x + 20.0, r.y + 14.0, r.w - 40.0, 30.0),
        18.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    ui.text_in(
        note,
        Rect::new(r.x + 20.0, r.y + 14.0, r.w - 40.0, 30.0),
        12.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Right,
    );
    Rect::new(r.x + 20.0, r.y + 56.0, r.w - 40.0, r.h - 56.0)
}

/// The dialog's foot (a line over it) at the bottom of `inside`; returns the row its buttons
/// sit in, and the inside above it.
pub fn dialog_foot(ui: &mut Ui, inside: Rect) -> (Rect, Rect) {
    let foot = Rect::new(inside.x, inside.bottom() - 62.0, inside.w, 62.0);
    ui.p().rect(
        Rect::new(inside.x - 20.0, foot.y, inside.w + 40.0, 1.0),
        EDGE,
    );
    let body = Rect::new(
        inside.x,
        inside.y,
        inside.w,
        (foot.y - 14.0 - inside.y).max(0.0),
    );
    (Rect::new(foot.x, foot.y + 14.0, foot.w, 34.0), body)
}

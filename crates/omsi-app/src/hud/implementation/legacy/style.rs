//! The game menu's look: flat near-black greys and one amber accent, as the launcher has
//! them, its sizes, and the small helpers its parts share.

use super::super::super::domain::Frame;

// ---------------------------------------------------------------------------------------
// the game menu and its lists: flat near-black greys and one amber accent, as the launcher

/// How long a fade of the menu takes (a line lit, a switch turned over): a short moment.
pub(super) const FADE_SECS: f32 = 0.15;
pub(super) const BAR_SECS: f32 = 0.19;

// (the launcher's values, see `launcher/theme.rs`: neutral greys, no tint)
pub(super) const PANEL: [u8; 4] = [22, 22, 22, 255];
pub(super) const PANEL_ALT: [u8; 4] = [31, 31, 31, 255];
pub(super) const BORDER: [u8; 4] = [255, 255, 255, 15];
pub(super) const DANGER: [u8; 4] = [222, 78, 68, 255];
/// A line under the mouse, and the line chosen: the launcher's `HOVER` and `SELECTED`, as
/// solid greys. (They were white at 6 % and 9 %, which the overlays blend in linear light:
/// on the 22-grey card that came out 73 and 92, twice as light as the launcher's 38 and 44.)
pub(super) const LIT: [u8; 4] = [38, 38, 38, 255];
pub(super) const SELECTED: [u8; 4] = [44, 44, 44, 255];
/// "End the session" under the mouse: the danger colour at 14 % on the card, solid.
pub(super) const LIT_DANGER: [u8; 4] = [50, 30, 28, 255];
/// A stepper's or a button's field: the launcher's `FIELD`.
pub(super) const CHIP: [u8; 4] = [31, 31, 31, 255];
/// A switch's track when off, a slider's empty track, their knob (the launcher's).
pub(super) const TRACK_OFF: [u8; 4] = [62, 62, 62, 255];
pub(super) const SLIDER_TRACK: [u8; 4] = [58, 58, 58, 255];
pub(super) const KNOB: [u8; 4] = [240, 240, 240, 255];
/// A scroll bar's thumb, idle and with the mouse over its list (the launcher's white at
/// 10 % and 35 %; no track, no accent).
pub(super) const THUMB: [u8; 4] = [255, 255, 255, 26];
pub(super) const THUMB_HOT: [u8; 4] = [255, 255, 255, 90];
/// The small capitals over a title (the launcher's section headings: 11 px, bold, dim).
pub(super) const DIM: [u8; 4] = [142, 142, 142, 0];
/// The ink on the accent (the launcher's primary button).
pub(super) const ON_ACCENT: [u8; 4] = [18, 14, 8, 0];
/// Text colours (alpha 0: no outline on the flat panel).
pub(super) const WHITE: [u8; 4] = [236, 236, 236, 0];
pub(super) const SOFT: [u8; 4] = [200, 200, 200, 0];
pub(super) const MUTED: [u8; 4] = [142, 142, 142, 0];

/// The card's radius, a line's, and the inset of lines from the card's edge and of their
/// text from the line's edge (all times the scale).
pub(super) const CARD_R: f32 = 8.0;
pub(super) const ROW_R: f32 = 6.0;
pub const PAD: f32 = 12.0;
pub(super) const TEXT_IN: f32 = 16.0;

/// `v` (0 to 1) in eighths.
pub(super) fn quant(v: f32) -> f32 {
    (v.clamp(0.0, 1.0) * 8.0).round() / 8.0
}

/// The interface's accent (the player's choice, `omsi_ui::theme::accent`), opaque.
pub(super) fn accent() -> [u8; 4] {
    let c = omsi_ui::theme::accent().0;
    let b = |v: f32| (v * 255.0).round() as u8;
    [b(c[0]), b(c[1]), b(c[2]), 255]
}

/// The accent, faint: a chip's field.
pub(super) fn accent_soft() -> [u8; 4] {
    let [r, g, b, _] = accent();
    [r, g, b, 34]
}

/// The accent's fill under the mouse.
pub(super) fn accent_hot() -> [u8; 4] {
    mix(accent(), [255, 255, 255, 255], 0.25)
}

/// Text in the accent, lighter than it (alpha 0: no outline on the flat panel): the times in
/// lists, a value being typed, the current stop, a tutorial's title.
pub(super) fn accent_ink() -> [u8; 4] {
    let [r, g, b, _] = mix(accent(), [255, 255, 255, 255], 0.4);
    [r, g, b, 0]
}

/// The colour `t` (0 to 1) of the way from `a` to `b`.
pub(super) fn mix(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), m(a[3], b[3])]
}

/// `c` with its opacity times `k` (0 to 1).
pub(super) fn fade(c: [u8; 4], k: f32) -> [u8; 4] {
    [
        c[0],
        c[1],
        c[2],
        (c[3] as f32 * k.clamp(0.0, 1.0)).round() as u8,
    ]
}

/// `c` as a text colour (no outline).
pub(super) fn txt(c: [u8; 4]) -> [u8; 4] {
    [c[0], c[1], c[2], 0]
}

/// The game menu's size: the screen's scale times the interface size (`Frame::ui_scale`, the
/// "Interface size" setting and the growth with tall windows, as the rest of the HUD has it),
/// held where the settings window (880 x 600 as designed) would no longer fit the window -
/// but never below the screen's scale alone, the size it always had.
pub(super) fn menu_scale(f: &Frame) -> f32 {
    menu_scale_for(f.scale, f.ui_scale, f.width, f.height, f.vr)
}

pub fn menu_scale_for(scale: f32, ui_scale: f32, width: f32, height: f32, vr: bool) -> f32 {
    let base = scale.max(0.5);
    if vr {
        return base;
    }
    let fit = (width / (880.0 + 24.0)).min(height * 0.94 / 600.0);
    (base * ui_scale.max(0.5)).min(fit.max(base))
}

/// A list line `"<code>  <name>"` (a number, maybe padded in front, two spaces, a name): its
/// code and name.
pub fn code_and_name(label: &str) -> Option<(&str, &str)> {
    let (code, name) = label.trim_start().split_once("  ")?;
    (!code.is_empty() && code.chars().all(|c| c.is_ascii_digit()) && !name.trim().is_empty())
        .then(|| (code, name.trim()))
}

/// The cursor is in `rect`.
pub(super) fn over_rect(c: (f32, f32), rect: [f32; 4]) -> bool {
    c.0 >= rect[0] && c.0 <= rect[2] && c.1 >= rect[1] && c.1 <= rect[3]
}

/// The distance of the point (`px`, `py`) from the rounded rectangle at (`x0`, `y0`) of
/// `w` x `h` with corners of `rad`: negative inside.
pub(super) fn rr_dist(px: f32, py: f32, x0: f32, y0: f32, w: f32, h: f32, rad: f32) -> f32 {
    let (cx, cy) = (x0 + w * 0.5, y0 + h * 0.5);
    let qx = (px - cx).abs() - (w * 0.5 - rad);
    let qy = (py - cy).abs() - (h * 0.5 - rad);
    (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt() + qx.max(qy).min(0.0) - rad
}

/// A line that opens another list ends in dots: the text without them, and whether it did.
pub(super) fn strip_more(label: &str) -> (&str, bool) {
    match label
        .strip_suffix("...")
        .or_else(|| label.strip_suffix('…'))
    {
        Some(t) => (t.trim_end(), true),
        None => (label, false),
    }
}

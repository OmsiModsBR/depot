//! The in-game windows' building blocks: a window frame with its title and a
//! note, a table with a chosen row, a list of label and value, and a row of buttons. The
//! windows of the concept (vehicles in the session, choose a vehicle, end-of-trip report,
//! menu, passengers and stop) are made of these by the game, with its own data.
//!
//! Opening these windows never pauses the simulation by itself: what the caller shows in
//! them (for example the end-of-trip report's note) is the caller's business.

use crate::paint::Align;
use crate::theme::{accent, on_accent};
use crate::widgets::{id_of, Ui};
use crate::{Color, Rect, Weight};
use glam::Vec2;

/// The window's face (under [`PANEL_ALPHA`]); what a window's tints are mixed over.
pub const PANEL: Color = Color::hex(0x202124);
const LINE: Color = Color::hex(0x2e2f33);
/// The window's edge (the Windows 11 concept's thin light stroke, mixed over the panel).
const EDGE: Color = Color::hex(0x3a3b3f);
/// A plain button's face.
const BUTTON: Color = Color::hex(0x2c2d31);
const ROW_LINE: Color = Color::hex(0x20242b);
const FG: Color = Color::hex(0xe9eaec);
const MUTED: Color = Color::hex(0x9aa0aa);
const FIELD: Color = Color::hex(0x1b1d22);
/// The window's and a button's corners.
const WINDOW_R: f32 = 8.0;
const BUTTON_R: f32 = 4.0;
const TS: f32 = 1.16;

pub const TITLE_H: f32 = 48.0;
pub const ROW_H: f32 = 34.0;

pub(crate) fn text(ui: &mut Ui, s: &str, x: f32, cy: f32, px: f32, weight: Weight, c: Color) {
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

pub(crate) fn text_right(ui: &mut Ui, s: &str, x: f32, cy: f32, px: f32, weight: Weight, c: Color) {
    let px = px * TS;
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

/// How much of a window's own colour covers the scene behind it: the picture shows through
/// a little, as it does through the concept's glass (no blur: it costs a pass of its own, and
/// the DirectX 9 output has none).
const PANEL_ALPHA: f32 = 0.88;

/// A window, as the Windows 11 concept draws it: the panel with rounded corners, a soft
/// shadow and a light stroke along its top edge, its title at the left and a note at the right
/// in a small pill. Returns the area under the title.
pub fn window(ui: &mut Ui, r: Rect, title: &str, note: &str) -> Rect {
    window_with(ui, r, title, note, 0.0)
}

/// Room kept free at the right of the title for the close button (see [`window_close`]).
pub const CLOSE_GUTTER: f32 = 44.0;

/// [`window`] with `gutter` pixels kept free at the right of the title, so the note's pill
/// does not sit under the close button.
pub fn window_with(ui: &mut Ui, r: Rect, title: &str, note: &str, gutter: f32) -> Rect {
    ui.solid(r);
    ui.p().shadow(r, WINDOW_R, 40.0, Color::rgba(0, 0, 0, 0.35));
    ui.p().shadow(r, WINDOW_R, 8.0, Color::rgba(0, 0, 0, 0.30));
    if let Some(bar) = ui.opaque_under {
        // (where the window covers the Alt bar the tiles would show through it: that part is opaque)
        let c = WINDOW_R * 0.4;
        let (x0, y0) = (bar.x.max(r.x + c), bar.y.max(r.y + c));
        let (x1, y1) = (
            bar.right().min(r.right() - c),
            bar.bottom().min(r.bottom() - c),
        );
        if x1 > x0 && y1 > y0 {
            ui.p().rect(Rect::new(x0, y0, x1 - x0, y1 - y0), PANEL);
        }
    }
    ui.p().rounded(r, WINDOW_R, PANEL.alpha(PANEL_ALPHA));
    ui.p().rounded_border(r, WINDOW_R, 1.0, EDGE);
    ui.p().rect(
        Rect::new(r.x + WINDOW_R, r.y, r.w - 2.0 * WINDOW_R, 1.0),
        Color::WHITE.alpha(0.14),
    );
    text(
        ui,
        title,
        r.x + 20.0,
        r.y + TITLE_H * 0.5,
        17.0,
        Weight::Bold,
        FG,
    );
    if !note.is_empty() {
        let px = 12.0 * TS;
        let w = ui.width(note, px, Weight::Regular) + 20.0;
        let pill = Rect::new(
            r.right() - 20.0 - gutter - w,
            r.y + TITLE_H * 0.5 - 12.0,
            w,
            24.0,
        );
        ui.p().rounded(pill, 12.0, Color::WHITE.alpha(0.07));
        ui.text_in(note, pill, px, Weight::Regular, MUTED, Align::Center);
    }
    Rect::new(
        r.x,
        r.y + TITLE_H + 1.0,
        r.w,
        (r.h - TITLE_H - 1.0).max(0.0),
    )
}

/// The close button at the right of a window's title (`r` is the window): a square with a
/// cross, named `name`. Returns whether it was clicked. It does what the window's Cancel or
/// Close does.
pub fn window_close(ui: &mut Ui, name: &str, r: Rect) -> bool {
    let b = Rect::new(r.right() - 44.0, r.y + (TITLE_H - 36.0) * 0.5, 36.0, 36.0);
    let (hover, _, click) = ui.interact(id_of(name), b);
    if hover {
        ui.p().rounded(b, BUTTON_R, Color::WHITE.alpha(0.10));
    }
    ui.icon("close", b.center(), 18.0, if hover { FG } else { MUTED });
    ui.tooltip(b, "Close");
    click
}

/// A table: column heads, rows of cells, the chosen row marked. `columns` are the x offsets
/// of the columns from the area's left edge. Returns the row clicked, if one was. Cells are
/// cut to their column; `colors` may colour a cell (row, column).
pub fn table(
    ui: &mut Ui,
    name: &str,
    area: Rect,
    columns: &[(&str, f32)],
    rows: &[Vec<String>],
    selected: Option<usize>,
    colors: &dyn Fn(usize, usize) -> Option<Color>,
) -> Option<usize> {
    for (title, x) in columns {
        text(
            ui,
            title,
            area.x + x,
            area.y + 17.0,
            12.0,
            Weight::Regular,
            MUTED,
        );
    }
    let mut clicked = None;
    for (i, row) in rows.iter().enumerate() {
        let r = Rect::new(
            area.x + 1.0,
            area.y + 32.0 + i as f32 * ROW_H,
            area.w - 2.0,
            ROW_H,
        );
        let (hover, _, click) = ui.interact(id_of(&format!("{name}:row:{i}")), r);
        if selected == Some(i) {
            chosen_row(ui, r, 1.0);
        } else {
            if hover {
                ui.p().rect(r, Color::WHITE.alpha(0.05));
            }
            ui.p().rect(Rect::new(r.x, r.y, r.w, 1.0), ROW_LINE);
        }
        for (c, (cell, (_, x))) in row.iter().zip(columns).enumerate() {
            let end = columns.get(c + 1).map(|n| n.1).unwrap_or(area.w - 8.0);
            let px = 14.0 * TS;
            ui.text_in(
                cell,
                Rect::new(area.x + x, r.y, (end - x - 8.0).max(0.0), r.h),
                px,
                Weight::Regular,
                colors(i, c).unwrap_or(FG),
                Align::Left,
            );
        }
        if click {
            clicked = Some(i);
        }
    }
    clicked
}

/// Label at the left, value at the right, one per row, hairlines between. Returns the height.
pub fn key_values(ui: &mut Ui, area: Rect, rows: &[(&str, String, Option<Color>)]) -> f32 {
    for (i, (k, v, c)) in rows.iter().enumerate() {
        let y = area.y + i as f32 * ROW_H;
        if i > 0 {
            ui.p()
                .rect(Rect::new(area.x + 1.0, y, area.w - 2.0, 1.0), ROW_LINE);
        }
        text(
            ui,
            k,
            area.x + 16.0,
            y + ROW_H * 0.5,
            14.0,
            Weight::Regular,
            MUTED,
        );
        text_right(
            ui,
            v,
            area.right() - 16.0,
            y + ROW_H * 0.5,
            14.0,
            Weight::Bold,
            c.unwrap_or(FG),
        );
    }
    rows.len() as f32 * ROW_H
}

/// How a button of a window is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// The window's main button: the accent colour.
    Primary,
    Normal,
}

/// The height of a window's button.
pub const BUTTON_H: f32 = 36.0;

/// The width a button with `label` takes in a row of [`buttons`].
pub fn button_width(ui: &Ui, label: &str) -> f32 {
    (ui.width(label, 14.0 * TS, Weight::Medium) + 44.0).max(88.0)
}

/// One button in `r`; a dim one (`enabled` false) is drawn but cannot be clicked. Returns
/// whether it was clicked.
pub fn button(
    ui: &mut Ui,
    id: &str,
    r: Rect,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
) -> bool {
    let (hover, held, click) = ui.interact(id_of(id), r);
    let primary = kind == ButtonKind::Primary;
    let fill = match (primary, enabled) {
        (true, true) => accent(),
        (true, false) => accent().alpha(0.22),
        (false, _) => BUTTON,
    };
    ui.p().rounded(
        r,
        BUTTON_R,
        if held && hover && enabled {
            fill.mix(Color::BLACK, 0.12)
        } else if hover && enabled {
            fill.lighten(0.08)
        } else {
            fill
        },
    );
    if !primary {
        ui.p().rounded_border(r, BUTTON_R, 1.0, EDGE);
    }
    let fg = match (primary, enabled) {
        (true, true) => on_accent(),
        (true, false) => FG.alpha(0.45),
        (false, true) => FG,
        (false, false) => FG.alpha(0.35),
    };
    ui.text_in(
        label,
        r,
        14.0 * TS,
        if primary {
            Weight::Bold
        } else {
            Weight::Medium
        },
        fg,
        Align::Center,
    );
    click && enabled
}

/// A row of buttons from `at`; the first is the accent one. Returns the index clicked.
pub fn buttons(ui: &mut Ui, name: &str, at: glam::Vec2, labels: &[&str]) -> Option<usize> {
    let row: Vec<(&str, bool)> = labels.iter().map(|l| (*l, true)).collect();
    buttons_enabled(ui, name, at, &row)
}

/// [`buttons`] where some may be dim: (label, enabled). A dim one cannot be clicked.
pub fn buttons_enabled(
    ui: &mut Ui,
    name: &str,
    at: glam::Vec2,
    labels: &[(&str, bool)],
) -> Option<usize> {
    row_of_buttons(ui, name, at, labels, true)
}

/// [`buttons_enabled`], the first button the accent one only when `accent`.
fn row_of_buttons(
    ui: &mut Ui,
    name: &str,
    at: glam::Vec2,
    labels: &[(&str, bool)],
    accent: bool,
) -> Option<usize> {
    let mut x = at.x;
    let mut clicked = None;
    for (i, (label, enabled)) in labels.iter().enumerate() {
        let w = button_width(ui, label);
        let r = Rect::new(x, at.y, w, BUTTON_H);
        let kind = if i == 0 && accent {
            ButtonKind::Primary
        } else {
            ButtonKind::Normal
        };
        if button(ui, &format!("{name}:btn:{i}"), r, label, kind, *enabled) {
            clicked = Some(i);
        }
        x += w + 8.0;
    }
    clicked
}

/// The height of a window's foot: a hairline, its buttons and a hint.
pub const FOOT_H: f32 = 64.0;

/// A window's foot inside `r` (the window): the hairline over it, `labels` at the right (the
/// first the accent one, see [`buttons_enabled`]) and `hint` at the left. Returns the button
/// clicked.
pub fn foot(
    ui: &mut Ui,
    name: &str,
    r: Rect,
    labels: &[(&str, bool)],
    hint: &str,
) -> Option<usize> {
    foot_with(ui, name, r, labels, hint, true)
}

/// A window's foot with only Close in it, grey (a window that only shows something has no
/// accent action). Returns whether Close was clicked.
pub fn foot_close(ui: &mut Ui, name: &str, r: Rect, hint: &str) -> bool {
    foot_with(ui, name, r, &[("Close", true)], hint, false).is_some()
}

fn foot_with(
    ui: &mut Ui,
    name: &str,
    r: Rect,
    labels: &[(&str, bool)],
    hint: &str,
    accent: bool,
) -> Option<usize> {
    ui.p().rect(
        Rect::new(r.x + 1.0, r.bottom() - FOOT_H, r.w - 2.0, 1.0),
        LINE,
    );
    // (the buttons at the right, as the concept's dialogs have them)
    let used: f32 = labels
        .iter()
        .map(|(l, _)| button_width(ui, l) + 8.0)
        .sum::<f32>()
        - 8.0;
    let at = Vec2::new(r.right() - 20.0 - used, r.bottom() - FOOT_H + 14.0);
    let clicked = row_of_buttons(ui, name, at, labels, accent);
    // (the hint keeps clear of the buttons: cut, never under them)
    let room = (r.w - 40.0 - used - 16.0).max(0.0);
    let mut hint_x = r.x + 20.0;
    let mut room = room;
    if !hint.is_empty() {
        // (a mark of information before the hint)
        ui.icon(
            "info",
            Vec2::new(hint_x + 8.0, r.bottom() - FOOT_H * 0.5),
            16.0,
            MUTED,
        );
        hint_x += 24.0;
        room = (room - 24.0).max(0.0);
    }
    ui.text_in(
        hint,
        Rect::new(hint_x, r.bottom() - FOOT_H, room, FOOT_H),
        12.0 * TS,
        Weight::Regular,
        MUTED,
        Align::Left,
    );
    clicked
}

/// The row chosen in a table or a list: a light face and a small accent pill at its left,
/// as Windows 11 marks the chosen item (`alpha` dims it with a list that cannot be used).
fn chosen_row(ui: &mut Ui, r: Rect, alpha: f32) {
    ui.p().rounded(
        Rect::new(r.x + 4.0, r.y + 2.0, r.w - 8.0, r.h - 4.0),
        BUTTON_R,
        Color::WHITE.alpha(0.08 * alpha),
    );
    ui.p().rounded(
        Rect::new(r.x + 4.0, r.center().y - 8.0, 3.0, 16.0),
        1.5,
        accent().alpha(alpha),
    );
}

/// A small heading over a part of a window (muted, as a table's column heads are), its
/// middle at `cy`.
pub fn head(ui: &mut Ui, s: &str, x: f32, cy: f32) {
    text(ui, s, x, cy, 12.0, Weight::Regular, MUTED);
}

/// A line of muted text cut to `r`, its middle at the middle of `r`.
pub fn note(ui: &mut Ui, s: &str, r: Rect) {
    ui.text_in(s, r, 13.0 * TS, Weight::Regular, MUTED, Align::Left);
}

/// The height of a row with a check box ([`check_row`]).
pub const CHECK_H: f32 = 40.0;

/// A row with a switch at its right, for an option that applies at once: `label` at the
/// left, on when `on`. A dim one (`disabled`: the reason, shown before the switch) cannot be
/// clicked. Returns whether it was clicked. (An option a window applies only on its OK has
/// the box of [`check_row`].)
pub fn switch_row(
    ui: &mut Ui,
    id: &str,
    r: Rect,
    label: &str,
    on: bool,
    disabled: Option<&str>,
) -> bool {
    let enabled = disabled.is_none();
    let (hover, _, click) = ui.interact(id_of(id), r);
    if hover && enabled {
        ui.p().rect(r, Color::WHITE.alpha(0.04));
    }
    // (a switch at the right, as Windows 11 has a setting that applies at once)
    let a = if enabled { 1.0 } else { 0.4 };
    let track = Rect::new(
        r.right() - 16.0 - 40.0,
        (r.center().y - 10.0).round(),
        40.0,
        20.0,
    );
    let t = ui.tween(id_of(id) ^ 0x5a17, if on { 1.0 } else { 0.0 }, 0.12);
    ui.p().rounded(track, 10.0, accent().alpha(t * a));
    if t < 1.0 {
        let edge = if hover && enabled {
            MUTED
        } else {
            MUTED.alpha(0.7)
        };
        ui.p()
            .rounded_border(track, 10.0, 1.0, edge.alpha((1.0 - t) * a));
    }
    let kx = track.x + 10.0 + 20.0 * t;
    ui.p().rounded(
        Rect::new(kx - 5.0, track.center().y - 5.0, 10.0, 10.0),
        5.0,
        MUTED.mix(on_accent(), t).alpha(a),
    );
    let lx = r.x + 16.0;
    let reason_w = disabled
        .map(|d| (ui.width(d, 12.0 * TS, Weight::Regular) + 16.0).min(r.w * 0.55))
        .unwrap_or(0.0);
    ui.text_in(
        label,
        Rect::new(lx, r.y, (track.x - 12.0 - reason_w - lx).max(0.0), r.h),
        14.0 * TS,
        Weight::Regular,
        if enabled { FG } else { FG.alpha(0.4) },
        Align::Left,
    );
    if let Some(why) = disabled {
        ui.text_in(
            why,
            Rect::new(track.x - 12.0 - reason_w, r.y, reason_w, r.h),
            12.0 * TS,
            Weight::Regular,
            MUTED,
            Align::Right,
        );
    }
    click && enabled
}

/// A row with a check box: `label` at the right of the box, ticked when `on`. A dim one
/// (`disabled`: the reason, shown at the right) cannot be clicked. Returns whether it was
/// clicked.
pub fn check_row(
    ui: &mut Ui,
    id: &str,
    r: Rect,
    label: &str,
    on: bool,
    disabled: Option<&str>,
) -> bool {
    let enabled = disabled.is_none();
    let (hover, _, click) = ui.interact(id_of(id), r);
    if hover && enabled {
        ui.p().rect(r, Color::WHITE.alpha(0.04));
    }
    let b = Rect::new(r.x + 16.0, (r.center().y - 9.0).round(), 18.0, 18.0);
    let a = if enabled { 1.0 } else { 0.35 };
    if on {
        ui.p().rect(b, accent().alpha(a));
        ui.icon("check", b.center(), 16.0, on_accent());
    } else {
        ui.p().rect(b, FIELD);
        let edge = if hover && enabled { MUTED } else { LINE };
        ui.p().rounded_border(b, 0.0, 1.0, edge);
    }
    let lx = b.right() + 12.0;
    let reason_w = disabled
        .map(|d| (ui.width(d, 12.0 * TS, Weight::Regular) + 16.0).min(r.w * 0.55))
        .unwrap_or(0.0);
    ui.text_in(
        label,
        Rect::new(lx, r.y, (r.right() - 16.0 - reason_w - lx).max(0.0), r.h),
        14.0 * TS,
        Weight::Regular,
        if enabled { FG } else { FG.alpha(0.4) },
        Align::Left,
    );
    if let Some(why) = disabled {
        ui.text_in(
            why,
            Rect::new(r.right() - 16.0 - reason_w, r.y, reason_w, r.h),
            12.0 * TS,
            Weight::Regular,
            MUTED,
            Align::Right,
        );
    }
    click && enabled
}

/// The height of a [`stepper`].
pub const STEPPER_H: f32 = 40.0;

/// A value with a button on each side that steps it down or up (the wheel over it does the
/// same). Returns -1 or 1 when it was stepped, else 0. A dim one does nothing.
pub fn stepper(ui: &mut Ui, id: &str, r: Rect, value: &str, enabled: bool) -> i32 {
    let base = id_of(id);
    ui.p().rect(r, FIELD);
    ui.p().rounded_border(r, 0.0, 1.0, LINE);
    let side = 30.0f32.min(r.w / 3.0);
    let down = Rect::new(r.x, r.y, side, r.h);
    let up = Rect::new(r.right() - side, r.y, side, r.h);
    let mut step = 0;
    for (k, (part, icon, by)) in [(down, "chevron_left", -1), (up, "chevron_right", 1)]
        .into_iter()
        .enumerate()
    {
        let (hover, held, click) = ui.interact(base ^ (k as u64 + 1), part);
        if hover && enabled {
            let a = if held { 0.08 } else { 0.05 };
            ui.p().rect(part.inset(1.0), Color::WHITE.alpha(a));
        }
        let c = match (enabled, hover) {
            (false, _) => MUTED.alpha(0.3),
            (true, true) => accent(),
            (true, false) => MUTED,
        };
        ui.icon(icon, part.center(), 20.0, c);
        if click && enabled {
            step = by;
        }
    }
    if enabled && ui.hover(r) && ui.input.wheel.y.abs() > 0.0 {
        step = ui.input.wheel.y.signum() as i32;
        ui.input.wheel.y = 0.0;
    }
    ui.text_in(
        value,
        Rect::new(down.right(), r.y, (up.x - down.right()).max(0.0), r.h),
        15.0 * TS,
        Weight::Medium,
        if enabled { FG } else { FG.alpha(0.4) },
        Align::Center,
    );
    step
}

/// The height of a [`text_field`].
pub const FIELD_H: f32 = 40.0;

/// A one-line text field showing `value`, with a caret at its end while `focused` (the
/// typing itself is the caller's). `placeholder` shows while it is empty and not focused.
/// Returns whether it was clicked.
pub fn text_field(
    ui: &mut Ui,
    id: &str,
    r: Rect,
    value: &str,
    focused: bool,
    placeholder: &str,
) -> bool {
    let (hover, _, click) = ui.interact(id_of(id), r);
    ui.p().rect(r, FIELD);
    let edge = if focused {
        accent()
    } else if hover {
        MUTED
    } else {
        LINE
    };
    ui.p().rounded_border(r, 0.0, 1.0, edge);
    let px = 15.0 * TS;
    let inner = Rect::new(r.x + 12.0, r.y, (r.w - 24.0).max(0.0), r.h);
    if value.is_empty() && !focused {
        let c = MUTED.alpha(0.7);
        ui.text_in(
            placeholder,
            inner,
            14.0 * TS,
            Weight::Regular,
            c,
            Align::Left,
        );
        return click;
    }
    // (what is typed is the player's, not the interface's: drawn as it is; the end of a
    // long one shows, where the caret is)
    let mut shown: &str = value;
    while !shown.is_empty() && ui.fonts.width_as_is(shown, px, Weight::Regular) > inner.w - 4.0 {
        let mut cs = shown.chars();
        cs.next();
        shown = cs.as_str();
    }
    let base = r.center().y + ui.fonts.cap_height(px, Weight::Regular) * 0.5;
    let at = Vec2::new(inner.x, base);
    let w = ui.text_as_is(shown, at, px, Weight::Regular, FG, Align::Left);
    if focused && (ui.time * 1.6).fract() < 0.6 {
        let x = (inner.x + w + 1.0).min(inner.right());
        ui.p()
            .rect(Rect::new(x, r.center().y - 10.0, 1.5, 20.0), FG);
    }
    click
}

/// A list of `labels` in a scrolled column of `area`, the chosen one marked. Returns the one
/// clicked, if one was.
pub fn list(
    ui: &mut Ui,
    name: &str,
    area: Rect,
    labels: &[String],
    selected: Option<usize>,
) -> Option<usize> {
    list_rows(ui, name, area, labels, selected, &ListLook::default())
}

/// How a list of [`list_rows`] looks.
#[derive(Clone, Copy, Debug)]
pub struct ListLook<'a> {
    /// A short value before each label (a time), in a column this wide; none when empty.
    pub leads: &'a [String],
    pub lead_w: f32,
    /// A dim list cannot be clicked.
    pub enabled: bool,
    /// The labels are the game's data (names of places, files), not the interface's own
    /// words: drawn as they are, not translated.
    pub as_is: bool,
    /// Shown in the middle of the area while the list is empty.
    pub empty: &'a str,
}

impl Default for ListLook<'_> {
    fn default() -> Self {
        ListLook {
            leads: &[],
            lead_w: 0.0,
            enabled: true,
            as_is: false,
            empty: "",
        }
    }
}

/// [`list`] as `look` says. The first time a list of this `name` is drawn it scrolls to show
/// the chosen row (a window opened on the setting in force).
pub fn list_rows(
    ui: &mut Ui,
    name: &str,
    area: Rect,
    labels: &[String],
    selected: Option<usize>,
    look: &ListLook,
) -> Option<usize> {
    if labels.is_empty() && !look.empty.is_empty() {
        let c = MUTED.alpha(0.8);
        ui.text_in(
            look.empty,
            area.inset(12.0),
            13.0 * TS,
            Weight::Regular,
            c,
            Align::Center,
        );
        return None;
    }
    let shown = id_of(name) ^ 0x7e7;
    if !ui.scroll.contains_key(&shown) {
        if let Some(s) = selected {
            let content = labels.len() as f32 * ROW_H;
            let to =
                (s as f32 * ROW_H - (area.h - ROW_H) * 0.5).clamp(0.0, (content - area.h).max(0.0));
            ui.scroll.insert(id_of(name), to);
            ui.scroll.insert(id_of(name) ^ 0xabc, to);
        }
        ui.scroll.insert(shown, 0.0);
    }
    let alpha = if look.enabled { 1.0 } else { 0.4 };
    let mut clicked = None;
    ui.scroll_area(name, area, &mut |ui, view| {
        for (i, label) in labels.iter().enumerate() {
            let r = Rect::new(view.x, view.y + i as f32 * ROW_H, view.w - 8.0, ROW_H);
            // (rows scrolled out of the area are neither drawn nor clickable)
            if r.bottom() < area.y || r.y > area.bottom() {
                continue;
            }
            let (hover, _, click) = ui.interact(id_of(&format!("{name}:row:{i}")), r);
            let chosen = selected == Some(i);
            if chosen {
                chosen_row(ui, r, alpha);
            } else {
                if hover && look.enabled {
                    ui.p().rect(r, Color::WHITE.alpha(0.05));
                }
                ui.p().rect(Rect::new(r.x, r.y, r.w, 1.0), ROW_LINE);
            }
            let fg = if chosen { FG } else { MUTED }.alpha(alpha);
            let mut x = r.x + 12.0;
            if let Some(lead) = look.leads.get(i) {
                let lr = Rect::new(x, r.y, look.lead_w, r.h);
                ui.text_in_tip(lead, lr, 14.0 * TS, Weight::Medium, fg, Align::Left);
            }
            if !look.leads.is_empty() {
                x += look.lead_w;
            }
            let lr = Rect::new(x, r.y, (r.right() - 8.0 - x).max(0.0), r.h);
            if look.as_is {
                let px = 14.0 * TS;
                let fit = fit_as_is(ui, label, px, lr.w);
                // (a name cut to the row: the whole of it once the mouse rests)
                if fit != *label {
                    ui.tooltip_whole(lr, label);
                }
                let base = r.center().y + ui.fonts.cap_height(px, Weight::Regular) * 0.5;
                ui.text_as_is(
                    &fit,
                    Vec2::new(x, base),
                    px,
                    Weight::Regular,
                    fg,
                    Align::Left,
                );
            } else {
                ui.text_in_tip(label, lr, 14.0 * TS, Weight::Regular, fg, Align::Left);
            }
            if click && look.enabled && area.contains(ui.input.mouse) {
                clicked = Some(i);
            }
        }
        labels.len() as f32 * ROW_H
    });
    clicked
}

/// `s` cut with an ellipsis to `max` points, not translated.
fn fit_as_is(ui: &Ui, s: &str, px: f32, max: f32) -> String {
    if ui.fonts.width_as_is(s, px, Weight::Regular) <= max {
        return s.to_string();
    }
    let mut cut: Vec<char> = s.chars().collect();
    while !cut.is_empty() {
        cut.pop();
        let t: String = cut.iter().collect::<String>().trim_end().to_string() + "…";
        if ui.fonts.width_as_is(&t, px, Weight::Regular) <= max {
            return t;
        }
    }
    String::new()
}

/// The "Add vehicle" window: a picture of the vehicle chosen, its manufacturer, model, paint
/// and depot file in dropdowns, the author's description, and the fleet number and plate.
/// Every list is the game's own; this only shows them.
pub struct AddVehicle<'a> {
    pub makers: &'a [String],
    pub maker: usize,
    /// The models of the manufacturer chosen.
    pub vehicles: &'a [String],
    pub vehicle: usize,
    pub liveries: &'a [String],
    pub livery: usize,
    pub depots: &'a [String],
    pub depot: usize,
    /// The chosen vehicle's fleet numbers (its prefixes); empty when it has none.
    pub numbers: &'a [String],
    pub number: usize,
    /// The registration typed (empty: the vehicle's own), whether its field has the keyboard,
    /// and what the field says while it is empty.
    pub plate: &'a str,
    pub plate_focused: bool,
    pub plate_hint: &'a str,
    /// The author's description of the vehicle chosen (drawn as it is).
    pub description: &'a str,
    pub preview: Preview,
    /// The player turned the live 3D view on (its button lit; the box shows `Preview::Live`,
    /// `Starting` or `Failed` then).
    pub live: bool,
}

/// What the picture box of the "Add vehicle" window shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Preview {
    /// A picture: its texture, its width over its height, and the part of it the vehicle
    /// stands in (left, top, right, bottom, 0 to 1): only that part is shown, as large as
    /// the box allows.
    Picture {
        tex: usize,
        aspect: f32,
        part: [f32; 4],
    },
    /// The picture is being read.
    Loading,
    /// No picture was drawn of this vehicle and paint yet.
    Missing,
    /// The live 3D view's latest frame, drawn at the box's size.
    Live { tex: usize },
    /// The live 3D view is starting (the vehicle is being read).
    Starting,
    /// The live 3D view could not be shown.
    Failed,
}

/// What a click in the "Add vehicle" window asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AddVehicleEvent {
    Maker(usize),
    Vehicle(usize),
    Livery(usize),
    Depot(usize),
    /// The fleet number at this line of the list.
    Number(usize),
    /// The registration's field was clicked.
    Plate,
    Place,
    Cancel,
    /// The live 3D view's button: on or off.
    Live(bool),
    /// The mouse dragged over the live view (points across, down): it turns the vehicle.
    Orbit(f32, f32),
    /// The wheel over the live view: the distance to the vehicle times this.
    Zoom(f32),
}

/// Where the "Add vehicle" window is in a picture of `size` points: in the middle.
pub fn add_vehicle_rect(size: glam::Vec2) -> Rect {
    let w = 880.0f32.min(size.x - 32.0);
    let h = 600.0f32.min(size.y - 32.0);
    Rect::new(
        ((size.x - w) * 0.5).floor(),
        ((size.y - h) * 0.5).floor(),
        w,
        h,
    )
}

/// Where the picture box of the "Add vehicle" window is in a picture of `size` points (the
/// live 3D view is drawn at its size).
pub fn add_vehicle_box(size: glam::Vec2) -> Rect {
    let r = add_vehicle_rect(size);
    let body = Rect::new(
        r.x,
        r.y + TITLE_H + 1.0,
        r.w,
        (r.h - TITLE_H - 1.0).max(0.0),
    );
    let lw = ((body.w - 64.0) * 0.43).floor();
    Rect::new(
        body.x + 20.0,
        body.y + 14.0,
        lw,
        (body.h - FOOT_H - 14.0 - 4.0 * 62.0 - 18.0).clamp(80.0, 210.0),
    )
}

/// The part of a picture (`aspect` wide over high) to show in a box of `box_aspect`: the
/// vehicle standing in `part` in the middle, as large as it fits with a margin round it, the
/// picture's own backdrop round it filling the box. Kept inside the picture (moved, or a
/// little larger, where the vehicle stands near an edge). Left, top, right, bottom, 0 to 1.
pub fn preview_part(box_aspect: f32, aspect: f32, part: [f32; 4]) -> [f32; 4] {
    const MARGIN: f32 = PREVIEW_MARGIN;
    let a = box_aspect.max(1e-3);
    // (in the picture's height as the unit, so that both sides measure alike)
    let (x0, x1) = (part[0] * aspect, part[2] * aspect);
    let (y0, y1) = (part[1], part[3]);
    let mut w = ((x1 - x0) * MARGIN).max((y1 - y0) * MARGIN * a);
    let mut h = w / a;
    if w > aspect {
        w = aspect;
        h = w / a;
    }
    if h > 1.0 {
        h = 1.0;
        w = h * a;
    }
    let cx = ((x0 + x1) * 0.5).clamp(w * 0.5, aspect - w * 0.5);
    let cy = ((y0 + y1) * 0.5).clamp(h * 0.5, 1.0 - h * 0.5);
    [
        (cx - w * 0.5) / aspect,
        cy - h * 0.5,
        (cx + w * 0.5) / aspect,
        cy + h * 0.5,
    ]
}

/// How much room is left round the vehicle in the picture box (1.12: 12 % of its size).
const PREVIEW_MARGIN: f32 = 1.12;

/// Where in the box `b` to draw a picture (`aspect` wide over high) whose vehicle stands in
/// `part`, and which part of the picture: the whole box with `preview_part` when a part of
/// the box's shape holds the vehicle; else (a box much wider or taller than the picture) the
/// vehicle with its margin, whole, as large as it fits in the middle of the box.
pub fn preview_fit(b: Rect, aspect: f32, part: [f32; 4]) -> (Rect, [f32; 4]) {
    let uv = preview_part(b.w / b.h.max(1.0), aspect, part);
    const E: f32 = 1e-4;
    if uv[0] <= part[0] + E && uv[1] <= part[1] + E && uv[2] >= part[2] - E && uv[3] >= part[3] - E
    {
        return (b, uv);
    }
    // (in the picture's height as the unit, so that both sides measure alike)
    let around = |a: f32, z: f32, end: f32| {
        let (c, half) = ((a + z) * 0.5, (z - a) * PREVIEW_MARGIN * 0.5);
        ((c - half).max(0.0), (c + half).min(end))
    };
    let (x0, x1) = around(part[0] * aspect, part[2] * aspect, aspect);
    let (y0, y1) = around(part[1], part[3], 1.0);
    let ra = (x1 - x0).max(E) / (y1 - y0).max(E);
    let (w, h) = if ra > b.w / b.h.max(1.0) {
        (b.w, b.w / ra)
    } else {
        (b.h * ra, b.h)
    };
    (
        Rect::new(b.x + (b.w - w) * 0.5, b.y + (b.h - h) * 0.5, w, h),
        [x0 / aspect, y0, x1 / aspect, y1],
    )
}

/// The part of a picture drawn ahead of time (`w` x `h`, RGBA) the vehicle stands in, as
/// (left, top, right, bottom) from 0 to 1, without the empty floor and sky round it. The
/// pictures have a backdrop that changes from top to bottom but not across (a sky, the
/// horizon, a plain floor) and the vehicle clear of the left and right edges: a pixel unlike
/// its row's edges (or those of the rows next to it, for the curved horizon) is the vehicle's,
/// when the pixel over or under it is too (a line one pixel thin is the horizon's blend).
/// None when nothing stands out.
pub fn subject_part(w: usize, h: usize, rgba: &[u8]) -> Option<[f32; 4]> {
    const EDGE: usize = 3;
    const NEAR: usize = 2;
    const DIFF: i32 = 48;
    if w <= EDGE * 4 || h < 3 || rgba.len() < w * h * 4 {
        return None;
    }
    let px = |x: usize, y: usize| {
        let i = (y * w + x) * 4;
        [rgba[i] as i32, rgba[i + 1] as i32, rgba[i + 2] as i32]
    };
    let mean = |y: usize, xs: std::ops::Range<usize>| {
        let n = xs.len() as i32;
        let mut a = [0; 3];
        for x in xs {
            let p = px(x, y);
            for c in 0..3 {
                a[c] += p[c];
            }
        }
        a.map(|v| v / n)
    };
    let refs: Vec<[[i32; 3]; 2]> = (0..h)
        .map(|y| [mean(y, 0..EDGE), mean(y, w - EDGE..w)])
        .collect();
    let mut odd = vec![false; w * h];
    for y in 0..h {
        let near = &refs[y.saturating_sub(NEAR)..(y + NEAR + 1).min(h)];
        for x in 0..w {
            let p = px(x, y);
            odd[y * w + x] = near
                .iter()
                .flatten()
                .all(|r| (p[0] - r[0]).abs() + (p[1] - r[1]).abs() + (p[2] - r[2]).abs() > DIFF);
        }
    }
    let (mut cols, mut rows) = (vec![0u32; w], vec![0u32; h]);
    for y in 1..h - 1 {
        for x in 0..w {
            if odd[y * w + x] && (odd[(y - 1) * w + x] || odd[(y + 1) * w + x]) {
                cols[x] += 1;
                rows[y] += 1;
            }
        }
    }
    let first = |v: &[u32]| v.iter().position(|&n| n >= 2);
    let last = |v: &[u32]| v.iter().rposition(|&n| n >= 2);
    let (x0, x1, y0, y1) = (first(&cols)?, last(&cols)?, first(&rows)?, last(&rows)?);
    let (fw, fh) = (w as f32, h as f32);
    Some([
        x0.saturating_sub(2) as f32 / fw,
        y0.saturating_sub(2) as f32 / fh,
        (x1 + 3).min(w) as f32 / fw,
        (y1 + 3).min(h) as f32 / fh,
    ])
}

/// The "Add vehicle" window in a picture of `size` points. It never pauses anything: the
/// game goes on behind it.
pub fn add_vehicle(ui: &mut Ui, size: glam::Vec2, v: &AddVehicle) -> Option<AddVehicleEvent> {
    let r = add_vehicle_rect(size);
    let body = window(ui, r, "Add vehicle", "");
    let mut event = None;
    let x0 = body.x + 20.0;
    let lw = ((body.w - 64.0) * 0.43).floor();
    let x1 = x0 + lw + 24.0;
    let rw = body.right() - 20.0 - x1;
    // the picture of the vehicle, the bus alone filling the box
    let box_ = add_vehicle_box(size);
    ui.p().rounded(box_, 6.0, FIELD);
    ui.p().rounded_border(box_, 6.0, 1.0, LINE);
    match v.preview {
        Preview::Picture { tex, aspect, part } => {
            let (at, uv) = preview_fit(box_.inset(1.0), aspect, part);
            ui.image_part(at, tex, uv, 5.0);
        }
        Preview::Loading => {
            ui.text_in(
                "Loading…",
                box_,
                13.0 * TS,
                Weight::Regular,
                MUTED,
                Align::Center,
            );
        }
        Preview::Missing => {
            ui.icon(
                "directions_bus",
                Vec2::new(box_.center().x, box_.center().y - 22.0),
                40.0,
                MUTED.alpha(0.5),
            );
            ui.text_in(
                "No picture of this vehicle yet",
                Rect::new(box_.x + 12.0, box_.center().y + 6.0, box_.w - 24.0, 20.0),
                13.0 * TS,
                Weight::Regular,
                MUTED,
                Align::Center,
            );
            ui.text_in(
                "The launcher draws them on its Vehicles page",
                Rect::new(box_.x + 12.0, box_.center().y + 28.0, box_.w - 24.0, 18.0),
                11.5 * TS,
                Weight::Regular,
                MUTED.alpha(0.75),
                Align::Center,
            );
        }
        Preview::Live { tex } => ui.image_part(box_.inset(1.0), tex, [0.0, 0.0, 1.0, 1.0], 5.0),
        Preview::Starting | Preview::Failed => {
            let (s, c) = if v.preview == Preview::Starting {
                ("Starting the 3D view…", MUTED)
            } else {
                ("The 3D view could not be shown", MUTED.alpha(0.85))
            };
            ui.text_in(
                s,
                box_.pad(12.0, 0.0),
                13.0 * TS,
                Weight::Regular,
                c,
                Align::Center,
            );
        }
    }
    // the live view turns with the mouse dragged over it and comes nearer with the wheel
    if v.live {
        if let Some(d) = ui.drag(id_of("add:turn"), box_) {
            if d != Vec2::ZERO {
                event = Some(AddVehicleEvent::Orbit(d.x, d.y));
            }
        }
        let wheel = ui.input.wheel.y;
        if wheel.abs() > 0.0 && ui.hover(box_) {
            event = Some(AddVehicleEvent::Zoom((1.0 - wheel * 0.08).clamp(0.8, 1.25)));
        }
    }
    // (the live view only when the player asks for it: it reads the vehicle and draws it
    // beside the game)
    let tip = if v.live {
        "Back to the picture"
    } else {
        "Show it in 3D: drag to turn it, the wheel brings it nearer"
    };
    if live_button(
        ui,
        Vec2::new(box_.right() - 20.0, box_.y + 20.0),
        v.live,
        tip,
    ) {
        event = Some(AddVehicleEvent::Live(!v.live));
    }
    // the four dropdowns under it, as the original's lists by manufacturer, then model, then
    // paint and depot file
    let field = |k: usize| box_.bottom() + 14.0 + k as f32 * 62.0;
    let drop = |ui: &mut Ui, k: usize, label: &str, name: &str, opts: &[String], at: usize| {
        let y = field(k);
        head(ui, label, x0, y + 8.0);
        let mut i = at;
        let changed = ui.select_as_is(name, Rect::new(x0, y + 20.0, lw, 34.0), &mut i, opts);
        changed.then_some(i)
    };
    if let Some(i) = drop(ui, 0, "Manufacturer", "add:makers", v.makers, v.maker) {
        event = Some(AddVehicleEvent::Maker(i));
    }
    if let Some(i) = drop(ui, 1, "Model", "add:vehicles", v.vehicles, v.vehicle) {
        event = Some(AddVehicleEvent::Vehicle(i));
    }
    if let Some(i) = drop(ui, 2, "Paint", "add:liveries", v.liveries, v.livery) {
        event = Some(AddVehicleEvent::Livery(i));
    }
    if let Some(i) = drop(ui, 3, "Garage", "add:depots", v.depots, v.depot) {
        event = Some(AddVehicleEvent::Depot(i));
    }
    // the author's description at the right, scrolling, as the author wrote it
    head(ui, "Description", x1, body.y + 22.0);
    let last = field(3);
    let desc = Rect::new(
        x1,
        body.y + 34.0,
        rw,
        (last - 14.0 - body.y - 34.0).max(40.0),
    );
    ui.p().rounded(desc, 6.0, FIELD);
    ui.p().rounded_border(desc, 6.0, 1.0, LINE);
    let text = v.description.trim();
    let inner = desc.pad(14.0, 10.0);
    let px = 13.5 * TS;
    if text.is_empty() {
        ui.text_in(
            "The author wrote no description.",
            Rect::new(inner.x, inner.y, inner.w, 24.0),
            px,
            Weight::Regular,
            MUTED,
            Align::Left,
        );
    } else {
        // (the box starts at the top for every vehicle: its scroll is named after the text)
        let name = format!("add:about:{:016x}", id_of(text));
        ui.scroll_area(&name, inner, &mut |ui, view| {
            ui.paragraph_as_is(
                text,
                Vec2::new(view.x, view.y),
                view.w - 10.0,
                px,
                Weight::Regular,
                FG,
            ) + 8.0
        });
    }
    // the fleet number (the prefix) and the registration (the plate) under it
    let half = ((rw - 12.0) * 0.5).floor();
    head(ui, "Prefix", x1, last + 8.0);
    head(ui, "Plate", x1 + half + 12.0, last + 8.0);
    let nr = Rect::new(x1, last + 20.0, half, 34.0);
    if v.numbers.is_empty() {
        ui.p().rounded(nr, 4.0, FIELD);
        ui.p().rounded_border(nr, 4.0, 1.0, LINE);
        ui.text_in(
            "-",
            nr.pad(12.0, 0.0),
            13.0,
            Weight::Regular,
            MUTED,
            Align::Left,
        );
    } else {
        let mut i = v.number;
        if ui.select_as_is("add:number", nr, &mut i, v.numbers) {
            event = Some(AddVehicleEvent::Number(i));
        }
    }
    if text_field(
        ui,
        "add:plate",
        Rect::new(x1 + half + 12.0, last + 20.0, rw - half - 12.0, 34.0),
        v.plate,
        v.plate_focused,
        v.plate_hint,
    ) {
        event = Some(AddVehicleEvent::Plate);
    }
    let hint = "The vehicle follows the mouse until a click puts it down";
    match foot(
        ui,
        "add",
        r,
        &[("Place", !v.vehicles.is_empty()), ("Cancel", true)],
        hint,
    ) {
        Some(0) => event = Some(AddVehicleEvent::Place),
        Some(_) => event = Some(AddVehicleEvent::Cancel),
        None => {}
    }
    event
}

/// The button in the picture box's corner that turns the live 3D view on and off ("3D" by
/// a turning arrow): lit in the accent while it is on.
fn live_button(ui: &mut Ui, top_right: Vec2, on: bool, tip: &str) -> bool {
    const H: f32 = 26.0;
    const W: f32 = 54.0;
    let r = Rect::new(top_right.x - W, top_right.y, W, H);
    let id = id_of("add:live");
    let (h, _, clicked) = ui.interact(id, r);
    let fill = match (on, h) {
        (true, _) => accent(),
        (false, true) => BUTTON,
        (false, false) => PANEL.alpha(0.85),
    };
    ui.p().rounded(r, H * 0.5, fill);
    if !on {
        ui.p().rounded_border(r, H * 0.5, 1.0, EDGE);
    }
    let c = if on { on_accent() } else { FG };
    ui.icon("360", Vec2::new(r.x + 17.0, r.center().y), 17.0, c);
    ui.text_in_as_is(
        "3D",
        Rect::new(r.x + 28.0, r.y, W - 30.0, H),
        12.5,
        Weight::Bold,
        c,
    );
    ui.tooltip(r, tip);
    clicked
}

const GOOD: Color = Color::hex(0x4fc27a);
const BAD: Color = Color::hex(0xe8523f);
const SOON: Color = Color::hex(0xe0a030);

/// How a stop of the end-of-trip report went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopState {
    OnTime,
    Late,
    LeftEarly,
    /// Arrived late and left before its time.
    LateLeftEarly,
    /// Passed without being served, with a later stop served.
    Missed,
    /// Skipped on purpose by the driver: no mark, neither missed nor late.
    Skipped,
    /// Not reached (the trip ended before it, or it is not known).
    None,
}

/// One stop of the end-of-trip report, as the game words it.
#[derive(Clone, Debug, PartialEq)]
pub struct ReportRow {
    pub stop: String,
    pub scheduled: String,
    pub actual: String,
    pub deviation: String,
    pub state: StopState,
}

/// The end-of-trip report: the trip's title, its stops and the totals under them.
pub struct TripReport<'a> {
    pub title: &'a str,
    pub note: &'a str,
    pub rows: &'a [ReportRow],
    pub totals: &'a [(&'a str, String)],
}

/// Where the end-of-trip report is in a picture of `size` points: in the middle.
pub fn trip_report_rect(size: glam::Vec2) -> Rect {
    let w = 900.0f32.min(size.x - 32.0);
    let h = 620.0f32.min(size.y - 32.0);
    Rect::new(
        ((size.x - w) * 0.5).floor(),
        ((size.y - h) * 0.5).floor(),
        w,
        h,
    )
}

/// A trip's stops as the end-of-trip report shows them: the column heads at the top of
/// `area` (a window's width) and under them a row per stop, scrolled by the wheel under the
/// id `scroll`. Returns the area of the rows.
pub fn stops_table(ui: &mut Ui, scroll: &str, area: Rect, rows: &[ReportRow]) -> Rect {
    let columns = [
        ("Stop", 16.0),
        ("Scheduled", 0.44),
        ("Actual", 0.58),
        ("Deviation", 0.71),
        ("Status", 0.84),
    ];
    let col_x = |c: f32| if c < 1.0 { (area.w * c).floor() } else { c };
    for (title, c) in columns {
        text(
            ui,
            title,
            area.x + col_x(c),
            area.y + 17.0,
            12.0,
            Weight::Regular,
            MUTED,
        );
    }
    let rows_area = Rect::new(
        area.x + 1.0,
        area.y + 34.0,
        area.w - 2.0,
        (area.h - 34.0).max(ROW_H),
    );
    ui.scroll_area(scroll, rows_area, &mut |ui, view| {
        for (i, row) in rows.iter().enumerate() {
            let rr = Rect::new(view.x, view.y + i as f32 * ROW_H, view.w - 8.0, ROW_H);
            if rr.bottom() < rows_area.y || rr.y > rows_area.bottom() {
                continue;
            }
            ui.p().rect(Rect::new(rr.x, rr.y, rr.w, 1.0), ROW_LINE);
            let (status, color) = match row.state {
                StopState::OnTime => ("OK", Some(GOOD)),
                StopState::Late => ("late", Some(BAD)),
                StopState::LeftEarly => ("left early", Some(SOON)),
                StopState::LateLeftEarly => ("late, left early", Some(BAD)),
                StopState::Missed => ("missed", Some(BAD)),
                StopState::Skipped => ("skipped", None),
                StopState::None => ("", None),
            };
            let cells = [
                row.stop.as_str(),
                row.scheduled.as_str(),
                row.actual.as_str(),
                row.deviation.as_str(),
                status,
            ];
            for (c, cell) in cells.iter().enumerate() {
                let x = col_x(columns[c].1) - 1.0;
                let end = columns
                    .get(c + 1)
                    .map(|n| col_x(n.1) - 1.0)
                    .unwrap_or(rr.w - 8.0);
                let fg = if c >= 3 { color.unwrap_or(MUTED) } else { FG };
                ui.text_in(
                    cell,
                    Rect::new(rr.x + x, rr.y, (end - x - 8.0).max(0.0), rr.h),
                    14.0 * TS,
                    Weight::Regular,
                    fg,
                    Align::Left,
                );
            }
        }
        rows.len() as f32 * ROW_H
    });
    rows_area
}

/// The end-of-trip report in a picture of `size` points. True when "Continue" was clicked.
pub fn trip_report(ui: &mut Ui, size: glam::Vec2, v: &TripReport) -> bool {
    let r = trip_report_rect(size);
    let body = window(ui, r, "End of trip report", v.note);
    text(
        ui,
        v.title,
        body.x + 16.0,
        body.y + 22.0,
        14.0,
        Weight::Medium,
        FG,
    );
    let foot_h = FOOT_H;
    // the totals as tiles under the trip's title, before the stops
    let tiles_y = body.y + 40.0;
    let tiles_h = if v.totals.is_empty() { 0.0 } else { 64.0 };
    if !v.totals.is_empty() {
        let n = v.totals.len() as f32;
        let gap = 12.0;
        let w = ((body.w - 32.0 - gap * (n - 1.0)) / n).floor();
        for (i, (k, val)) in v.totals.iter().enumerate() {
            let t = Rect::new(body.x + 16.0 + i as f32 * (w + gap), tiles_y, w, tiles_h);
            ui.p().rounded(t, BUTTON_R, Color::WHITE.alpha(0.05));
            ui.p().rounded_border(t, BUTTON_R, 1.0, EDGE);
            ui.text_in(
                k,
                Rect::new(t.x + 14.0, t.y + 6.0, t.w - 28.0, 20.0),
                12.0 * TS,
                Weight::Regular,
                MUTED,
                Align::Left,
            );
            ui.text_in(
                val,
                Rect::new(t.x + 14.0, t.y + 26.0, (t.w - 28.0).max(0.0), 32.0),
                20.0 * TS,
                Weight::Bold,
                FG,
                Align::Left,
            );
        }
    }
    let head_y = tiles_y + tiles_h + if tiles_h > 0.0 { 8.0 } else { 4.0 };
    let area = Rect::new(
        body.x,
        head_y,
        body.w,
        (r.bottom() - foot_h - 8.0 - head_y).max(ROW_H + 34.0),
    );
    // (each trip's report starts at its first stop: the id is the trip's)
    stops_table(ui, &format!("report:rows:{}", v.title), area, v.rows);
    foot(
        ui,
        "report",
        r,
        &[("Continue", true)],
        "Enter or Esc also continues",
    )
    .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn rows() -> Vec<Vec<String>> {
        vec![vec!["A".into(), "1".into()], vec!["B".into(), "2".into()]]
    }

    #[test]
    fn a_window_over_the_alt_bar_is_opaque_where_they_overlap() {
        let mut ui = Ui::new();
        ui.begin(glam::Vec2::new(800.0, 600.0), 1.0, 0.05);
        let bar = Rect::new(10.0, 10.0, 100.0, 300.0);
        ui.opaque_under = Some(bar);
        let before = ui.p().len();
        window(&mut ui, Rect::new(60.0, 100.0, 300.0, 200.0), "T", "");
        let over = ui.p().len();
        ui.opaque_under = None;
        window(&mut ui, Rect::new(60.0, 100.0, 300.0, 200.0), "T", "");
        let clear = ui.p().len() - over;
        // (more is drawn for the part over the bar)
        assert!(over - before > clear);
    }

    #[test]
    fn a_click_on_a_table_row_and_a_button_is_reported() {
        let mut ui = Ui::new();
        let cols = [("Name", 16.0), ("Number", 200.0)];
        let run = |ui: &mut Ui, click: Option<Vec2>| {
            let frame = |ui: &mut Ui| {
                ui.begin(Vec2::new(800.0, 600.0), 1.0, 0.016);
                let area = window(ui, Rect::new(20.0, 20.0, 400.0, 300.0), "Test", "note");
                let row = table(ui, "t", area, &cols, &rows(), Some(0), &|_, _| None);
                let button = buttons(ui, "t", Vec2::new(40.0, 280.0), &["Ok", "Cancel"]);
                ui.finish();
                (row, button)
            };
            if let Some(p) = click {
                ui.input.mouse = p;
                ui.input.pressed = true;
                ui.input.down = true;
                frame(ui);
                ui.input.released = true;
                ui.input.down = false;
            }
            frame(ui)
        };
        run(&mut ui, None);
        let second = ui.drawn[&id_of("t:row:1")].center();
        assert_eq!(run(&mut ui, Some(second)).0, Some(1));
        let cancel = ui.drawn[&id_of("t:btn:1")].center();
        assert_eq!(run(&mut ui, Some(cancel)).1, Some(1));
        // a click on nothing reports nothing
        assert_eq!(run(&mut ui, Some(Vec2::new(700.0, 500.0))), (None, None));
    }

    #[test]
    fn the_add_vehicle_window_reports_its_clicks() {
        let vehicles: Vec<String> = (0..40).map(|i| format!("Bus {i}")).collect();
        let makers = vec!["Maker A".to_string(), "Maker B".to_string()];
        let liveries = vec!["Random livery".to_string(), "Red".to_string()];
        let depots = vec!["The map's depot file".to_string()];
        let numbers = vec!["4711".to_string(), "4712".to_string()];
        let size = Vec2::new(1672.0, 941.0);
        let mut ui = Ui::new();
        let view = AddVehicle {
            makers: &makers,
            maker: 0,
            vehicles: &vehicles,
            vehicle: 2,
            liveries: &liveries,
            livery: 0,
            depots: &depots,
            depot: 0,
            numbers: &numbers,
            number: 0,
            plate: "",
            plate_focused: false,
            plate_hint: "Automatic",
            description: "Example bus\nBuilt from 1990 to 1999.",
            preview: Preview::Missing,
            live: false,
        };
        let frame = |ui: &mut Ui| {
            ui.begin(size, 1.0, 0.016);
            let e = add_vehicle(ui, size, &view);
            ui.finish();
            e
        };
        // a click is a press and its release; what it chose comes back on one of the frames
        let click = |ui: &mut Ui, p: Vec2| {
            ui.input.mouse = p;
            ui.input.pressed = true;
            ui.input.down = true;
            let a = frame(ui);
            ui.input.released = true;
            ui.input.down = false;
            let b = frame(ui);
            // (a list opened needs a moment before it takes a click)
            let mut c = None;
            for _ in 0..30 {
                c = c.or(frame(ui));
            }
            a.or(b).or(c)
        };
        frame(&mut ui);
        // a dropdown opens its list; a line of it chosen is reported
        let pick = |ui: &mut Ui, name: &str, line: usize| {
            let anchor = ui.drawn[&id_of(name)];
            assert_eq!(click(ui, anchor.center()), None, "{name} only opens");
            assert!(ui.popup_open());
            let row = Vec2::new(
                anchor.x + 20.0,
                anchor.bottom() + 6.0 + 4.0 + line as f32 * 34.0 + 17.0,
            );
            click(ui, row)
        };
        assert_eq!(
            pick(&mut ui, "add:makers", 1),
            Some(AddVehicleEvent::Maker(1))
        );
        assert!(!ui.popup_open());
        assert_eq!(
            pick(&mut ui, "add:vehicles", 3),
            Some(AddVehicleEvent::Vehicle(3))
        );
        assert_eq!(
            pick(&mut ui, "add:liveries", 1),
            Some(AddVehicleEvent::Livery(1))
        );
        assert_eq!(
            pick(&mut ui, "add:number", 1),
            Some(AddVehicleEvent::Number(1))
        );
        let plate = ui.drawn[&id_of("add:plate")].center();
        assert_eq!(click(&mut ui, plate), Some(AddVehicleEvent::Plate));
        let place = ui.drawn[&id_of("add:btn:0")].center();
        assert_eq!(click(&mut ui, place), Some(AddVehicleEvent::Place));
        let cancel = ui.drawn[&id_of("add:btn:1")].center();
        assert_eq!(click(&mut ui, cancel), Some(AddVehicleEvent::Cancel));
        let live = ui.drawn[&id_of("add:live")].center();
        assert_eq!(click(&mut ui, live), Some(AddVehicleEvent::Live(true)));
    }

    #[test]
    fn the_live_view_turns_with_the_mouse_dragged_over_it() {
        let size = Vec2::new(1672.0, 941.0);
        let none: Vec<String> = Vec::new();
        let vehicles = vec!["Bus".to_string()];
        let mut ui = Ui::new();
        let mut view = AddVehicle {
            makers: &vehicles,
            maker: 0,
            vehicles: &vehicles,
            vehicle: 0,
            liveries: &none,
            livery: 0,
            depots: &none,
            depot: 0,
            numbers: &none,
            number: 0,
            plate: "",
            plate_focused: false,
            plate_hint: "",
            description: "",
            preview: Preview::Live { tex: 1 },
            live: true,
        };
        let mut frame = |ui: &mut Ui, view: &AddVehicle| {
            ui.begin(size, 1.0, 0.016);
            let e = add_vehicle(ui, size, view);
            ui.finish();
            e
        };
        let b = add_vehicle_box(size);
        frame(&mut ui, &view);
        assert_eq!(
            ui.drawn[&id_of("add:turn")],
            b,
            "the box the overlay sizes the view to"
        );
        // pressed in the box and moved: the moves come back, none before the mouse moved
        ui.input.mouse = b.center();
        ui.input.pressed = true;
        ui.input.down = true;
        assert_eq!(frame(&mut ui, &view), None);
        ui.input.pressed = false;
        ui.input.mouse += Vec2::new(12.0, -3.0);
        assert_eq!(
            frame(&mut ui, &view),
            Some(AddVehicleEvent::Orbit(12.0, -3.0))
        );
        ui.input.mouse += Vec2::new(-5.0, 0.0);
        assert_eq!(
            frame(&mut ui, &view),
            Some(AddVehicleEvent::Orbit(-5.0, 0.0))
        );
        ui.input.released = true;
        ui.input.down = false;
        frame(&mut ui, &view);
        ui.input.released = false;
        // the wheel over it comes nearer
        ui.input.wheel = Vec2::new(0.0, 1.0);
        assert!(matches!(
            frame(&mut ui, &view),
            Some(AddVehicleEvent::Zoom(k)) if k < 1.0
        ));
        ui.input.wheel = Vec2::ZERO;
        // with the live view off the mouse dragged over the picture turns nothing
        view.live = false;
        view.preview = Preview::Missing;
        ui.input.pressed = true;
        ui.input.down = true;
        frame(&mut ui, &view);
        ui.input.pressed = false;
        ui.input.mouse += Vec2::new(20.0, 0.0);
        assert_eq!(frame(&mut ui, &view), None);
    }

    #[test]
    fn the_vehicle_is_found_on_a_sky_and_a_floor_without_the_horizon() {
        let (w, h) = (120usize, 80usize);
        let mut img = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                // a sky lighter at the top, the horizon at row 30 (a row lower in the middle,
                // with a one-pixel blend), a plain grey floor
                let horizon = if (40..80).contains(&x) { 31 } else { 30 };
                let mut c = if y < horizon {
                    [200 - y as u8, 210 - y as u8, 230 - y as u8]
                } else {
                    [70, 73, 75]
                };
                if y == horizon {
                    c = [135, 150, 165];
                }
                // a vehicle from (30, 40) to (90, 62), dark with light windows
                if (30..90).contains(&x) && (40..62).contains(&y) {
                    c = if y < 48 {
                        [220, 220, 210]
                    } else {
                        [20, 20, 24]
                    };
                }
                img[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        let p = subject_part(w, h, &img).unwrap();
        let px = |v: f32, n: usize| (v * n as f32).round() as i32;
        assert_eq!((px(p[0], w), px(p[2], w)), (28, 92), "{p:?}");
        assert_eq!((px(p[1], h), px(p[3], h)), (38, 64), "{p:?}");
        // a plain backdrop has nothing in it
        let plain: Vec<u8> = (0..w * h).flat_map(|_| [70u8, 73, 75, 255]).collect();
        assert_eq!(subject_part(w, h, &plain), None);
    }

    #[test]
    fn the_vehicle_stands_in_the_middle_of_the_box_as_large_as_it_fits() {
        let aspect = 336.0 / 208.0;
        // a bus in the middle half of the picture: the part shown has the box's shape, holds
        // the bus with a margin, and the bus is in its middle
        let bus = [0.3, 0.35, 0.8, 0.75];
        let uv = preview_part(2.0, aspect, bus);
        let (w, h) = ((uv[2] - uv[0]) * aspect, uv[3] - uv[1]);
        assert!((w / h - 2.0).abs() < 1e-4, "{uv:?}");
        assert!(uv[0] < bus[0] && uv[2] > bus[2] && uv[1] < bus[1] && uv[3] > bus[3]);
        assert!(((uv[0] + uv[2]) - (bus[0] + bus[2])).abs() < 1e-4);
        // as large as it fits: the bus's width and the margin, no more
        assert!((w - (bus[2] - bus[0]) * aspect * 1.12).abs() < 1e-4);
        // near an edge it moves inside the picture
        let edge = preview_part(2.0, aspect, [0.0, 0.4, 0.4, 0.7]);
        assert!(edge[0] >= 0.0 && edge[2] <= 1.0);
        // a box wider than the picture can fill: the whole width, the bus inside
        let wide = preview_part(4.0, aspect, [0.05, 0.1, 0.95, 0.9]);
        assert!(
            wide[0] >= -1e-6 && wide[2] <= 1.0 + 1e-6 && wide[1] >= -1e-6 && wide[3] <= 1.0 + 1e-6
        );
        // ... where no part of the box's shape holds the bus, the bus is drawn whole, as large
        // as it fits in the middle of the box
        let b = Rect::new(10.0, 20.0, 400.0, 100.0);
        let bus = [0.05, 0.1, 0.95, 0.9];
        let (at, uv) = preview_fit(b, aspect, bus);
        assert!(uv[0] <= bus[0] && uv[1] <= bus[1] && uv[2] >= bus[2] && uv[3] >= bus[3]);
        assert!(uv[0] >= 0.0 && uv[1] >= 0.0 && uv[2] <= 1.0 && uv[3] <= 1.0);
        assert!((at.h - b.h).abs() < 1e-3 && at.w < b.w);
        assert!((at.center().x - b.center().x).abs() < 1e-3);
        let shown = (uv[2] - uv[0]) * aspect / (uv[3] - uv[1]);
        assert!((at.w / at.h - shown).abs() < 1e-3, "not stretched");
        // and where one does, the whole box
        let (at, uv) = preview_fit(
            Rect::new(0.0, 0.0, 200.0, 100.0),
            aspect,
            [0.3, 0.35, 0.8, 0.75],
        );
        assert_eq!(at, Rect::new(0.0, 0.0, 200.0, 100.0));
        assert_eq!(uv, preview_part(2.0, aspect, [0.3, 0.35, 0.8, 0.75]));
    }

    #[test]
    fn the_trip_report_says_when_continue_is_clicked() {
        let rows: Vec<ReportRow> = (0..30)
            .map(|i| ReportRow {
                stop: format!("Stop {i}"),
                scheduled: "08:00".into(),
                actual: "08:01".into(),
                deviation: "+1:00".into(),
                state: StopState::Late,
            })
            .collect();
        let totals = [("Stops served", "30".to_string())];
        let size = Vec2::new(1672.0, 941.0);
        let view = TripReport {
            title: "Line 1 to Terminus",
            note: "paused",
            rows: &rows,
            totals: &totals,
        };
        let mut ui = Ui::new();
        let frame = |ui: &mut Ui| {
            ui.begin(size, 1.0, 0.016);
            let c = trip_report(ui, size, &view);
            ui.finish();
            c
        };
        assert!(!frame(&mut ui));
        let b = ui.drawn[&id_of("report:btn:0")].center();
        assert!(trip_report_rect(size).contains(b));
        ui.input.mouse = b;
        ui.input.pressed = true;
        ui.input.down = true;
        frame(&mut ui);
        ui.input.released = true;
        ui.input.down = false;
        assert!(frame(&mut ui));
    }
}

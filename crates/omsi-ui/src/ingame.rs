//! The in-game windows' building blocks: a window frame with its title and a
//! note, a table with a chosen row, a list of label and value, and a row of buttons. The
//! windows of the concept (vehicles in the session, choose a vehicle, end-of-trip report,
//! menu, passengers and stop) are made of these by the game, with its own data.
//!
//! Opening these windows never pauses the simulation by itself: what the caller shows in
//! them (for example the end-of-trip report's note) is the caller's business.

use crate::paint::Align;
use crate::theme::{accent, accent_soft, on_accent};
use crate::widgets::{id_of, Ui};
use crate::{Color, Rect, Weight};
use glam::Vec2;

const PANEL: Color = Color::hex(0x202124);
const LINE: Color = Color::hex(0x2e2f33);
/// The window's edge (the Windows 11 concept's thin light stroke, mixed over the panel).
const EDGE: Color = Color::hex(0x3a3b3f);
/// A plain button's face.
const BUTTON: Color = Color::hex(0x2c2d31);
const ROW_LINE: Color = Color::hex(0x20242b);
const FG: Color = Color::hex(0xe9eaec);
const MUTED: Color = Color::hex(0x9aa0aa);
const FIELD: Color = Color::hex(0x1b1d22);
const PANEL_ALPHA: f32 = 0.97;
/// The window's and a button's corners.
const WINDOW_R: f32 = 8.0;
const BUTTON_R: f32 = 4.0;
const TS: f32 = 1.16;

pub const TITLE_H: f32 = 40.0;
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

/// A window, as the Windows 11 concept draws it: the panel with rounded corners and a
/// shadow, its title at the left and a note at the right. Returns the area under the title.
pub fn window(ui: &mut Ui, r: Rect, title: &str, note: &str) -> Rect {
    ui.solid(r);
    ui.p().shadow(r, WINDOW_R, 24.0, Color::rgba(0, 0, 0, 0.5));
    ui.p().rounded(r, WINDOW_R, PANEL.alpha(PANEL_ALPHA));
    ui.p().rounded_border(r, WINDOW_R, 1.0, EDGE);
    text(ui, title, r.x + 16.0, r.y + 24.0, 15.5, Weight::Bold, FG);
    text_right(
        ui,
        note,
        r.right() - 20.0,
        r.y + 24.0,
        12.0,
        Weight::Regular,
        MUTED,
    );
    Rect::new(
        r.x,
        r.y + TITLE_H + 1.0,
        r.w,
        (r.h - TITLE_H - 1.0).max(0.0),
    )
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
            ui.p().rect(r, accent_soft());
            ui.p().rect(Rect::new(r.x, r.y, 3.0, r.h), accent());
        } else {
            if hover {
                ui.p().rect(r, Color::WHITE.alpha(0.04));
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
pub const BUTTON_H: f32 = 39.0;

/// The width a button with `label` takes in a row of [`buttons`].
pub fn button_width(ui: &Ui, label: &str) -> f32 {
    ui.width(label, 14.0 * TS, Weight::Medium) + 44.0
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
    let (hover, _, click) = ui.interact(id_of(id), r);
    let primary = kind == ButtonKind::Primary;
    let fill = match (primary, enabled) {
        (true, true) => accent(),
        (true, false) => accent().alpha(0.22),
        (false, _) => BUTTON,
    };
    ui.p().rounded(
        r,
        BUTTON_R,
        if hover && enabled {
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
    let mut x = at.x;
    let mut clicked = None;
    for (i, (label, enabled)) in labels.iter().enumerate() {
        let w = button_width(ui, label);
        let r = Rect::new(x, at.y, w, BUTTON_H);
        let kind = if i == 0 {
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
    let at = Vec2::new(r.right() - 20.0 - used, r.bottom() - FOOT_H + 12.0);
    let clicked = buttons_enabled(ui, name, at, labels);
    // (the hint keeps clear of the buttons: cut, never under them)
    let room = (r.w - 40.0 - used - 16.0).max(0.0);
    ui.text_in(
        hint,
        Rect::new(r.x + 20.0, r.bottom() - FOOT_H, room, FOOT_H),
        12.0 * TS,
        Weight::Regular,
        MUTED,
        Align::Left,
    );
    clicked
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
                ui.p().rect(r, accent_soft().alpha(alpha));
                ui.p()
                    .rect(Rect::new(r.x, r.y, 3.0, r.h), accent().alpha(alpha));
            } else {
                if hover && look.enabled {
                    ui.p().rect(r, Color::WHITE.alpha(0.04));
                }
                ui.p().rect(Rect::new(r.x, r.y, r.w, 1.0), ROW_LINE);
            }
            let fg = if chosen { FG } else { MUTED }.alpha(alpha);
            let mut x = r.x + 12.0;
            if let Some(lead) = look.leads.get(i) {
                let lr = Rect::new(x, r.y, look.lead_w, r.h);
                ui.text_in(lead, lr, 14.0 * TS, Weight::Medium, fg, Align::Left);
            }
            if !look.leads.is_empty() {
                x += look.lead_w;
            }
            let lr = Rect::new(x, r.y, (r.right() - 8.0 - x).max(0.0), r.h);
            if look.as_is {
                let px = 14.0 * TS;
                let fit = fit_as_is(ui, label, px, lr.w);
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
                ui.text_in(label, lr, 14.0 * TS, Weight::Regular, fg, Align::Left);
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

/// The "Add vehicle" window: the vehicles, the chosen one's liveries and depot files, and
/// the buttons. Every list is the game's own; this only shows them.
pub struct AddVehicle<'a> {
    pub vehicles: &'a [String],
    pub vehicle: Option<usize>,
    pub liveries: &'a [String],
    pub livery: usize,
    pub depots: &'a [String],
    pub depot: usize,
}

/// What a click in the "Add vehicle" window asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddVehicleEvent {
    Vehicle(usize),
    Livery(usize),
    Depot(usize),
    Place,
    Cancel,
}

/// Where the "Add vehicle" window is in a picture of `size` points: in the middle.
pub fn add_vehicle_rect(size: glam::Vec2) -> Rect {
    let w = 860.0f32.min(size.x - 32.0);
    let h = 560.0f32.min(size.y - 32.0);
    Rect::new(
        ((size.x - w) * 0.5).floor(),
        ((size.y - h) * 0.5).floor(),
        w,
        h,
    )
}

/// The "Add vehicle" window in a picture of `size` points. It never pauses anything: the
/// game goes on behind it.
pub fn add_vehicle(ui: &mut Ui, size: glam::Vec2, v: &AddVehicle) -> Option<AddVehicleEvent> {
    let r = add_vehicle_rect(size);
    let body = window(ui, r, "Add vehicle", "the game keeps running");
    let mut event = None;
    let foot = 64.0;
    let lists_h = (body.h - foot - 36.0).max(ROW_H);
    let left_w = (body.w * 0.5).floor();
    let right_x = body.x + left_w + 1.0;
    let right_w = body.right() - right_x;
    let head = |ui: &mut Ui, s: &str, x: f32, y: f32| {
        text(ui, s, x + 16.0, y + 17.0, 12.0, Weight::Regular, MUTED)
    };
    // the vehicles at the left, the livery and the depot file at the right
    head(ui, "Vehicle", body.x, body.y);
    let vehicles = Rect::new(body.x + 4.0, body.y + 34.0, left_w - 8.0, lists_h);
    if let Some(i) = list(ui, "add:vehicles", vehicles, v.vehicles, v.vehicle) {
        event = Some(AddVehicleEvent::Vehicle(i));
    }
    ui.p().rect(
        Rect::new(body.x + left_w, body.y + 8.0, 1.0, body.h - foot - 8.0),
        LINE,
    );
    let half = ((lists_h - 34.0) * 0.5).floor();
    head(ui, "Livery", right_x, body.y);
    let liveries = Rect::new(right_x + 4.0, body.y + 34.0, right_w - 8.0, half);
    if let Some(i) = list(ui, "add:liveries", liveries, v.liveries, Some(v.livery)) {
        event = Some(AddVehicleEvent::Livery(i));
    }
    let y2 = liveries.bottom() + 4.0;
    head(ui, "Depot file", right_x, y2);
    let depots = Rect::new(right_x + 4.0, y2 + 34.0, right_w - 8.0, half);
    if let Some(i) = list(ui, "add:depots", depots, v.depots, Some(v.depot)) {
        event = Some(AddVehicleEvent::Depot(i));
    }
    ui.p().rect(
        Rect::new(r.x + 1.0, r.bottom() - foot, r.w - 2.0, 1.0),
        LINE,
    );
    let labels: &[&str] = if v.vehicle.is_some() {
        &["Place", "Cancel"]
    } else {
        &["Cancel"]
    };
    let at = glam::Vec2::new(r.x + 16.0, r.bottom() - foot + 12.0);
    match buttons(ui, "add", at, labels) {
        Some(0) if v.vehicle.is_some() => event = Some(AddVehicleEvent::Place),
        Some(_) => event = Some(AddVehicleEvent::Cancel),
        None => {}
    }
    text_right(
        ui,
        "The vehicle follows the mouse until a click puts it down",
        r.right() - 16.0,
        r.bottom() - foot * 0.5,
        12.0,
        Weight::Regular,
        MUTED,
    );
    event
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
    let foot = 64.0;
    let totals_h = v.totals.len() as f32 * ROW_H;
    let head_y = body.y + 44.0;
    let area = Rect::new(
        body.x,
        head_y,
        body.w,
        (r.bottom() - foot - totals_h - 8.0 - head_y).max(ROW_H + 34.0),
    );
    // (each trip's report starts at its first stop: the id is the trip's)
    let rows_area = stops_table(ui, &format!("report:rows:{}", v.title), area, v.rows);
    let totals = Rect::new(r.x, rows_area.bottom() + 8.0, r.w, totals_h);
    ui.p()
        .rect(Rect::new(r.x + 1.0, totals.y - 1.0, r.w - 2.0, 1.0), LINE);
    let rows: Vec<(&str, String, Option<Color>)> = v
        .totals
        .iter()
        .map(|(k, val)| (*k, val.clone(), None))
        .collect();
    key_values(ui, totals, &rows);
    ui.p().rect(
        Rect::new(r.x + 1.0, r.bottom() - foot, r.w - 2.0, 1.0),
        LINE,
    );
    let at = glam::Vec2::new(r.x + 16.0, r.bottom() - foot + 12.0);
    let clicked = buttons(ui, "report", at, &["Continue"]).is_some();
    text_right(
        ui,
        "Enter or Esc also continues",
        r.right() - 16.0,
        r.bottom() - foot * 0.5,
        12.0,
        Weight::Regular,
        MUTED,
    );
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn rows() -> Vec<Vec<String>> {
        vec![vec!["A".into(), "1".into()], vec!["B".into(), "2".into()]]
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
        let liveries = vec!["Random livery".to_string(), "Red".to_string()];
        let depots = vec!["The map's depot file".to_string()];
        let size = Vec2::new(1672.0, 941.0);
        let mut ui = Ui::new();
        let run = |ui: &mut Ui, chosen: Option<usize>, click: Option<Vec2>| {
            let view = AddVehicle {
                vehicles: &vehicles,
                vehicle: chosen,
                liveries: &liveries,
                livery: 0,
                depots: &depots,
                depot: 0,
            };
            let frame = |ui: &mut Ui| {
                ui.begin(size, 1.0, 0.016);
                let e = add_vehicle(ui, size, &view);
                ui.finish();
                e
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
        run(&mut ui, None, None);
        let row = ui.drawn[&id_of("add:vehicles:row:2")].center();
        assert_eq!(
            run(&mut ui, None, Some(row)),
            Some(AddVehicleEvent::Vehicle(2))
        );
        // without a vehicle chosen the only button is Cancel
        let first = ui.drawn[&id_of("add:btn:0")].center();
        assert_eq!(
            run(&mut ui, None, Some(first)),
            Some(AddVehicleEvent::Cancel)
        );
        run(&mut ui, Some(2), None);
        let place = ui.drawn[&id_of("add:btn:0")].center();
        assert_eq!(
            run(&mut ui, Some(2), Some(place)),
            Some(AddVehicleEvent::Place)
        );
        let red = ui.drawn[&id_of("add:liveries:row:1")].center();
        assert_eq!(
            run(&mut ui, Some(2), Some(red)),
            Some(AddVehicleEvent::Livery(1))
        );
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

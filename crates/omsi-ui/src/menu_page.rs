//! The pages behind the Esc menu's lines, in the new look. A page is either a settings window
//! (its pages in a card on the left, the rows of the one shown in a card beside it: a label,
//! a description, and a switch, a slider, a stepper, a value or a button at the right) or a
//! list (lines of text, maybe a line sign or a code before them, the way back pinned under
//! them, and beside them the timetable of the line or tour chosen). Both sit in the area the
//! pause menu leaves right of its column, under its bar.
//!
//! Nothing here knows the game: the caller hands over what the page shows, [`layout`] says
//! where everything is, and [`draw`] draws it there. Clicks are not handled here: the caller
//! hit-tests the rectangles of the layout, the same ones `draw` used.

use crate::paint::Align;
use crate::widgets::{id_of, Ui};
use crate::{Color, Rect, Weight};
use glam::Vec2;

const CARD: Color = Color::hex(0x15171b);
const LINE: Color = Color::hex(0x2b2f37);
const ROW_LINE: Color = Color::hex(0x20242b);
const FG: Color = Color::hex(0xe9eaec);
const MUTED: Color = Color::hex(0x9aa0aa);
const ACCENT: Color = Color::hex(0xf0a30a);
const ON_ACCENT: Color = Color::hex(0x14120a);
const FIELD: Color = Color::hex(0x1d2025);
const SELECTED: Color = Color::hex(0x2a2316);
const TRACK_OFF: Color = Color::hex(0x3a3e46);
const KNOB: Color = Color::hex(0xf2f2f2);
/// The text is drawn this much larger than its nominal size, as in the pause menu's column.
const TS: f32 = 1.16;

/// Space round the cards, and between them.
pub const MARGIN: f32 = 24.0;
pub const GAP: f32 = 16.0;
/// The band of a card's title.
pub const HEAD: f32 = 52.0;
pub const TABS_W: f32 = 236.0;
pub const TAB_H: f32 = 40.0;
/// A settings row, a list line, a list line with a line sign.
pub const FIELD_ROW: f32 = 58.0;
pub const LIST_ROW: f32 = 44.0;
pub const SIGN_ROW: f32 = 48.0;
pub const FOOT_H: f32 = 44.0;
pub const PANE_W: f32 = 380.0;
/// A row of the timetable beside a list.
pub const PANE_ROW: f32 = 28.0;
pub const DD_ITEM: f32 = 34.0;
/// The tallest a page gets (the window does not change size between pages).
const MAX_H: f32 = 760.0;
/// A slider's track, its value's column, a stepper's box.
const SLIDER_W: f32 = 220.0;
const VALUE_W: f32 = 64.0;
const STEPPER_W: f32 = 180.0;

/// What a row has at its right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Control<'a> {
    /// Nothing (a plain line of a list).
    None,
    /// A switch, on or off.
    Switch(bool),
    /// A slider at `frac` of its way, its value's text beside it.
    Slider { frac: f32, value: &'a str },
    /// A value the arrows step through (the left half of it steps down).
    Stepper(&'a str),
    /// Opens another list or a drop-down: its value now (may be empty) and an arrow.
    Opens(&'a str),
    /// A value that is only read.
    Info(&'a str),
    /// A value being typed.
    Editing(&'a str),
    /// A button: its text (a key binding, an action), may be empty.
    Button(&'a str),
}

/// What stands before a list line's text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lead<'a> {
    None,
    /// A line's number on a solid sign, as on a bus.
    Sign(&'a str),
    /// A code in a column of its own (a destination's number), the names after it lined up.
    Code(&'a str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row<'a> {
    pub label: &'a str,
    /// A second, smaller line under the label (settings rows).
    pub desc: &'a str,
    pub control: Control<'a>,
    pub lead: Lead<'a>,
    /// Heads the rows under it: never lit, never chosen.
    pub heading: bool,
}

impl<'a> Row<'a> {
    pub fn plain(label: &'a str) -> Row<'a> {
        Row {
            label,
            desc: "",
            control: Control::None,
            lead: Lead::None,
            heading: false,
        }
    }
}

/// Where the way back is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    /// None drawn (Esc goes back).
    None,
    /// The last row is the way back: pinned under the others.
    Footer,
    /// At the foot of the pages' card, after the pages.
    Side,
}

/// The timetable beside a list: a title, a line of facts and rows of (what, time).
pub struct Pane<'a> {
    pub title: &'a str,
    pub meta: &'a str,
    pub rows: &'a [(&'a str, &'a str)],
    /// The row chosen, when the rows can be chosen (the stops to start a tour from).
    pub chosen: Option<usize>,
    /// The button under the rows, when there is one.
    pub button: Option<&'a str>,
    /// The time the rows are of, between arrows that step it, when it can be set.
    pub time: Option<&'a str>,
}

/// A drop-down open over a row: its entries, the one the keyboard is on, the first shown and
/// the one in force now.
pub struct Dropdown<'a> {
    pub row: usize,
    pub items: &'a [&'a str],
    pub sel: usize,
    pub top: usize,
    pub current: Option<usize>,
}

pub struct Page<'a> {
    /// The rows' card's title (the page shown, or the list).
    pub title: &'a str,
    /// Small, right of that title (what the keys do).
    pub note: &'a str,
    /// The pages' card's title, and its pages (none: a page alone, no card for them).
    pub tabs_title: &'a str,
    pub tabs: &'a [&'a str],
    pub tab: usize,
    pub rows: &'a [Row<'a>],
    /// Settings rows (a label, a description and a control) rather than list lines.
    pub fields: bool,
    /// The row chosen, lit when the keyboard leads (`kbd`) and the mouse is off the rows.
    pub selected: usize,
    pub kbd: bool,
    /// A row marked whatever the mouse is over (the tour whose stops are shown).
    pub active: Option<usize>,
    /// The first row shown, when the wheel or a drag has scrolled the rows (else the chosen
    /// row is kept in view).
    pub top: Option<f32>,
    pub back: Back,
    pub pane: Option<Pane<'a>>,
    /// The first row of the timetable shown, when it has been scrolled.
    pub pane_first: Option<usize>,
    pub dropdown: Option<Dropdown<'a>>,
}

/// Where everything of a page is, in the points `draw` works in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub tabs_card: Option<Rect>,
    /// One per page, then the way back when it is on the side.
    pub tabs: Vec<Rect>,
    pub card: Rect,
    /// The rows shown, from row `start` on: `rows[k]` is row `start + k`. With the way back as
    /// a footer, its rectangle is at the index of its row, the rows out of view between as
    /// empty rectangles far away.
    pub rows: Vec<Rect>,
    /// Per row of `rows`, the control a click sets a value on (a slider's track, a stepper).
    pub controls: Vec<Option<Rect>>,
    pub footer: Option<Rect>,
    pub start: usize,
    pub shown: usize,
    pub row_h: f32,
    /// A long list's scroll bar: its track and its thumb (widened to be hit).
    pub scroll: Option<(Rect, Rect)>,
    pub pane_card: Option<Rect>,
    /// The timetable's rows that can be chosen (from `pane_first`), its button, the arrows round
    /// its time (before, after) and its scroll bar (track, thumb, rows, rows shown).
    pub pane_rows: Vec<Rect>,
    pub pane_first: usize,
    pub pane_go: Option<Rect>,
    pub time: Vec<Rect>,
    pub pane_scroll: Option<(Rect, Rect, usize, usize)>,
    pub dd_panel: Option<Rect>,
    /// The drop-down's entries shown, from `dd_top`, and its scroll bar.
    pub dd: Vec<Rect>,
    pub dd_top: usize,
    pub dd_rows: usize,
    pub dd_scroll: Option<(Rect, Rect)>,
}

/// Far out of the window: the rectangle of a row out of view.
const NOWHERE: Rect = Rect::new(-1.0e9, -1.0e9, 0.0, 0.0);

/// A scroll bar's thumb in `track` for `shown` of `n` rows from `first` (at least 28 points
/// long), and the same widened to be hit.
fn thumb(track: Rect, first: usize, shown: usize, n: usize) -> (Rect, Rect) {
    let n = n.max(1) as f32;
    let len = (track.h * shown as f32 / n).max(28.0f32.min(track.h));
    let t0 =
        track.y + (track.h - len) * (first as f32 / (n - shown as f32).max(1.0)).clamp(0.0, 1.0);
    let t = Rect::new(track.x, t0, track.w, len);
    (t, Rect::new(t.x - 6.0, t.y, t.w + 10.0, t.h))
}

/// The row height of a page's rows.
pub fn row_height(page: &Page) -> f32 {
    if page.fields {
        FIELD_ROW
    } else if page.rows.iter().any(|r| matches!(r.lead, Lead::Sign(_))) {
        SIGN_ROW
    } else {
        LIST_ROW
    }
}

/// Where a page's parts go in `area` (the room right of the pause menu's column, under its bar).
pub fn layout(page: &Page, area: Rect) -> Layout {
    let mut out = Layout::default();
    let inner = Rect::new(
        area.x + MARGIN,
        area.y + MARGIN,
        (area.w - MARGIN * 2.0).max(120.0),
        (area.h - MARGIN * 2.0).max(120.0),
    );
    let h = inner.h.min(MAX_H);
    let top = inner.y;
    let mut x = inner.x;
    // the pages' card
    if !page.tabs.is_empty() {
        let w = TABS_W.min(inner.w * 0.3).max(120.0);
        let tc = Rect::new(x, top, w, h);
        out.tabs_card = Some(tc);
        x += w + GAP;
        let back_h = if page.back == Back::Side {
            TAB_H + 16.0
        } else {
            0.0
        };
        let first = tc.y + HEAD;
        let room = tc.bottom() - 8.0 - back_h - first;
        let step = (room / page.tabs.len() as f32).clamp(24.0, TAB_H + 2.0);
        for i in 0..page.tabs.len() {
            out.tabs.push(Rect::new(
                tc.x + 8.0,
                first + i as f32 * step,
                tc.w - 16.0,
                step - 2.0,
            ));
        }
        if page.back == Back::Side {
            out.tabs.push(Rect::new(
                tc.x + 8.0,
                tc.bottom() - 8.0 - TAB_H,
                tc.w - 16.0,
                TAB_H,
            ));
        }
    }
    // the rows' card and the timetable beside it
    let avail = inner.right() - x;
    let want = if page.fields {
        820.0
    } else if page.pane.is_some() {
        480.0
    } else {
        600.0
    };
    let with_pane = page.pane.is_some() && avail >= 300.0 + GAP + 320.0;
    let pane_w = if with_pane {
        PANE_W.min(avail - GAP - 300.0)
    } else {
        0.0
    };
    let card_w = if with_pane {
        (avail - GAP - pane_w).min(want)
    } else {
        avail.min(want)
    };
    let card = Rect::new(x, top, card_w.max(120.0), h);
    out.card = card;
    let n = page.rows.len();
    let footer = page.back == Back::Footer && n > 0;
    let nl = n - footer as usize;
    let foot = if footer { FOOT_H + 12.0 } else { 0.0 };
    let body = Rect::new(
        card.x + 8.0,
        card.y + HEAD,
        card.w - 16.0,
        (card.h - HEAD - 8.0 - foot).max(1.0),
    );
    let row_h = row_height(page);
    let shown = ((body.h / row_h).floor() as usize).clamp(1, nl.max(1));
    let sel = page.selected.min(nl.saturating_sub(1));
    let start = match (nl > shown, page.top) {
        (false, _) => 0,
        (true, Some(t)) => (t.max(0.0).round() as usize).min(nl - shown),
        (true, None) => sel.saturating_sub(shown / 2).min(nl - shown),
    };
    out.start = start;
    out.shown = shown;
    out.row_h = row_h;
    let scrolls = nl > shown;
    let row_w = body.w - if scrolls { 10.0 } else { 0.0 };
    for k in start..(start + shown).min(nl) {
        let r = Rect::new(body.x, body.y + (k - start) as f32 * row_h, row_w, row_h);
        out.rows.push(r);
        out.controls.push(if page.fields {
            control_rect(&page.rows[k].control, r)
        } else {
            None
        });
    }
    if scrolls {
        let track = Rect::new(
            card.right() - 9.0,
            body.y + 4.0,
            4.0,
            shown as f32 * row_h - 8.0,
        );
        let (_, grip) = thumb(track, start, shown, nl);
        out.scroll = Some((track, grip));
    }
    if footer {
        let fr = Rect::new(
            card.x + 8.0,
            card.bottom() - 8.0 - FOOT_H,
            card.w - 16.0,
            FOOT_H,
        );
        while out.rows.len() < n - 1 - start {
            out.rows.push(NOWHERE);
            out.controls.push(None);
        }
        out.rows.push(fr);
        out.controls.push(None);
        out.footer = Some(fr);
    }
    // the timetable
    if let (Some(p), true) = (page.pane.as_ref(), with_pane) {
        let pc = Rect::new(card.right() + GAP, top, pane_w, h);
        out.pane_card = Some(pc);
        let mut y = pc.y + 80.0;
        if p.time.is_some() {
            let (bw, bh) = (44.0, 32.0);
            out.time.push(Rect::new(pc.x + 16.0, y, bw, bh));
            out.time.push(Rect::new(pc.right() - 16.0 - bw, y, bw, bh));
            y += bh + 10.0;
        }
        let n = p.rows.len();
        if let (Some(chosen), Some(_)) = (p.chosen, p.button) {
            let go = Rect::new(pc.x + 16.0, pc.bottom() - 16.0 - 40.0, pc.w - 32.0, 40.0);
            out.pane_go = Some(go);
            let fit = ((go.y - 10.0 - y) / PANE_ROW).floor().max(1.0) as usize;
            let first = match page.pane_first {
                Some(f) if n > fit => f.min(n - fit),
                _ if n > fit => chosen.saturating_sub(fit / 2).min(n - fit),
                _ => 0,
            };
            out.pane_first = first;
            for i in first..(first + fit).min(n) {
                out.pane_rows.push(Rect::new(
                    pc.x + 8.0,
                    y + (i - first) as f32 * PANE_ROW,
                    pc.w - 16.0 - if n > fit { 8.0 } else { 0.0 },
                    PANE_ROW,
                ));
            }
            if n > fit {
                let track = Rect::new(pc.right() - 9.0, y, 4.0, go.y - 10.0 - y);
                let (_, grip) = thumb(track, first, fit, n);
                out.pane_scroll = Some((track, grip, n, fit));
            }
        }
    }
    // the drop-down, over the rows' card: under its row, over it where there is no room under
    if let Some(dd) = page.dropdown.as_ref() {
        if dd.row >= start && dd.row < start + shown && dd.row < nl && !dd.items.is_empty() {
            let rr = out.rows[dd.row - start];
            let item_h = DD_ITEM.min(row_h);
            let below = body.bottom() - rr.bottom() - 4.0;
            let above = rr.y - body.y - 4.0;
            let want = dd.items.len().min(8);
            let fits = |room: f32| (((room - 8.0) / item_h).floor().max(0.0) as usize).min(want);
            let down = fits(below) >= want || fits(below) >= fits(above);
            let n_vis = (if down { fits(below) } else { fits(above) }).max(1);
            let ph = n_vis as f32 * item_h + 8.0;
            let pw = 320.0f32.min(rr.w - 16.0);
            let px1 = rr.right() - 8.0;
            let py0 = if down {
                rr.bottom() + 2.0
            } else {
                rr.y - 2.0 - ph
            };
            let panel = Rect::new(px1 - pw, py0, pw, ph);
            out.dd_panel = Some(panel);
            let first = dd.top.min(dd.items.len() - n_vis.min(dd.items.len()));
            out.dd_top = first;
            out.dd_rows = n_vis;
            let more = dd.items.len() > n_vis;
            for i in 0..n_vis.min(dd.items.len()) {
                out.dd.push(Rect::new(
                    panel.x + 4.0,
                    panel.y + 4.0 + i as f32 * item_h,
                    panel.w - 8.0 - if more { 10.0 } else { 0.0 },
                    item_h,
                ));
            }
            if more {
                let track = Rect::new(panel.right() - 10.0, panel.y + 4.0, 4.0, ph - 8.0);
                let (_, grip) = thumb(track, first, n_vis, dd.items.len());
                out.dd_scroll = Some((track, grip));
            }
        }
    }
    out
}

/// Where a settings row's control takes a click that sets a value: a slider's track, a
/// stepper's box (its left half steps down).
fn control_rect(c: &Control, r: Rect) -> Option<Rect> {
    let rx = r.right() - 16.0;
    match c {
        Control::Slider { .. } => {
            let tw = SLIDER_W.min((r.w - 32.0) * 0.4);
            let x1 = rx - VALUE_W - 14.0;
            Some(Rect::new(x1 - tw, r.y, tw, r.h))
        }
        Control::Stepper(_) => Some(Rect::new(rx - STEPPER_W, r.y, STEPPER_W, r.h)),
        _ => None,
    }
}

fn put(ui: &mut Ui, s: &str, x: f32, cy: f32, max: f32, px: f32, weight: Weight, c: Color) -> f32 {
    let px = px * TS;
    let w = ui.width(s, px, weight).min(max.max(0.0));
    ui.text_in(
        s,
        Rect::new(x, cy - px, w + 1.0, px * 2.0),
        px,
        weight,
        c,
        Align::Left,
    )
}

fn put_right(
    ui: &mut Ui,
    s: &str,
    right: f32,
    cy: f32,
    max: f32,
    px: f32,
    weight: Weight,
    c: Color,
) -> f32 {
    let px = px * TS;
    let w = ui.width(s, px, weight).min(max.max(0.0));
    ui.text_in(
        s,
        Rect::new(right - w - 1.0, cy - px, w + 1.0, px * 2.0),
        px,
        weight,
        c,
        Align::Right,
    );
    w
}

fn put_center(ui: &mut Ui, s: &str, r: Rect, px: f32, weight: Weight, c: Color) {
    ui.text_in(s, r, px * TS, weight, c, Align::Center);
}

/// A card: the flat panel, a hairline round it, its title in spaced capitals and a note at
/// the right of the title.
fn card(ui: &mut Ui, r: Rect, title: &str, note: &str) {
    ui.solid(r);
    ui.p().shadow(r, 0.0, 18.0, Color::BLACK.alpha(0.35));
    ui.p().rect(r, CARD.alpha(0.97));
    ui.p().rounded_border(r, 0.0, 1.0, LINE);
    let px = 12.0;
    let base = r.y + HEAD * 0.5 + ui.fonts.cap_height(px, Weight::Bold) * 0.5;
    let tw = ui.caps_spaced(
        title,
        Vec2::new(r.x + 20.0, base),
        px,
        Weight::Bold,
        FG,
        1.8,
    );
    if !note.is_empty() {
        put_right(
            ui,
            note,
            r.right() - 20.0,
            r.y + HEAD * 0.5,
            r.w - tw - 64.0,
            10.5,
            Weight::Regular,
            MUTED,
        );
    }
}

/// The light of a row under the mouse or the keyboard: a faint fill and the accent at its
/// left edge.
fn lit_row(ui: &mut Ui, r: Rect, t: f32) {
    if t <= 0.01 {
        return;
    }
    ui.p().rect(r, Color::WHITE.alpha(0.05 * t));
    ui.p().rect(Rect::new(r.x, r.y, 3.0, r.h), ACCENT.alpha(t));
}

/// A field's box (a select, a stepper, a value being typed): `edge` round it.
fn field_box(ui: &mut Ui, r: Rect, edge: Color) {
    ui.p().rect(r, FIELD);
    ui.p().rounded_border(r, 0.0, 1.0, edge);
}

/// The way back (at the foot of the pages or under a list): an arrow, "Back", and Esc.
fn back_row(ui: &mut Ui, r: Rect, lit: bool) {
    let t = ui.anim(
        id_of(&format!("page:back:{}:{}", r.x as i32, r.y as i32)),
        lit as u8 as f32,
        0.06,
    );
    ui.p().rect(r, Color::WHITE.alpha(0.03 + 0.04 * t));
    if t > 0.01 {
        ui.p().rect(Rect::new(r.x, r.y, 3.0, r.h), ACCENT.alpha(t));
    }
    let cy = r.y + r.h * 0.5;
    ui.icon(
        "arrow_back",
        Vec2::new(r.x + 22.0, cy),
        16.0,
        MUTED.mix(FG, t),
    );
    put(
        ui,
        "Back",
        r.x + 42.0,
        cy,
        r.w - 100.0,
        14.0,
        Weight::Regular,
        FG,
    );
    put_right(
        ui,
        "Esc",
        r.right() - 16.0,
        cy,
        40.0,
        10.5,
        Weight::Regular,
        MUTED,
    );
}

/// Draw `page` in `area`; returns its layout (the rectangles to hit-test).
pub fn draw(ui: &mut Ui, page: &Page, area: Rect) -> Layout {
    let l = layout(page, area);
    let mouse = ui.input.mouse;
    // the pages
    if let Some(tc) = l.tabs_card {
        card(ui, tc, page.tabs_title, "");
        for (i, title) in page.tabs.iter().enumerate() {
            let r = l.tabs[i];
            let on = i == page.tab;
            let hov = ui.hover(r) && page.dropdown.is_none();
            let t = ui.anim(id_of(&format!("page:tab:{title}")), hov as u8 as f32, 0.06);
            if on {
                ui.p().rect(r, SELECTED);
                ui.p().rect(Rect::new(r.x, r.y, 3.0, r.h), ACCENT);
            } else if t > 0.01 {
                ui.p().rect(r, Color::WHITE.alpha(0.05 * t));
            }
            let c = if on { FG } else { MUTED.mix(FG, t) };
            let px = (14.0f32).min(r.h * 0.42);
            put(
                ui,
                title,
                r.x + 16.0,
                r.y + r.h * 0.5,
                r.w - 28.0,
                px,
                Weight::Regular,
                c,
            );
        }
        if page.back == Back::Side {
            if let Some(&r) = l.tabs.get(page.tabs.len()) {
                ui.p()
                    .rect(Rect::new(r.x + 8.0, r.y - 8.0, r.w - 16.0, 1.0), ROW_LINE);
                let lit = ui.hover(r) && page.dropdown.is_none();
                back_row(ui, r, lit);
            }
        }
    }
    // the rows
    let c = l.card;
    card(ui, c, page.title, page.note);
    let any = c.contains(mouse);
    let nl = page.rows.len() - l.footer.is_some() as usize;
    // (the line signs as wide as the widest, the codes in a column as wide as the widest)
    let px = 14.0;
    let mut sign_w = 44.0f32;
    let mut code_w = 0.0f32;
    for r in &page.rows[..nl] {
        match r.lead {
            Lead::Sign(s) => sign_w = sign_w.max(ui.width(s, px * TS, Weight::Bold) + 22.0),
            Lead::Code(s) => code_w = code_w.max(ui.width(s, px * TS, Weight::Medium)),
            Lead::None => {}
        }
    }
    let mut prev_heading = true;
    for (i, &r) in l.rows.iter().enumerate() {
        let k = l.start + i;
        if Some(r) == l.footer || k >= nl {
            continue;
        }
        let row = &page.rows[k];
        if row.heading {
            ui.caps_spaced(
                row.label,
                Vec2::new(r.x + 16.0, r.bottom() - 13.0),
                10.5,
                Weight::Bold,
                MUTED,
                1.6,
            );
            prev_heading = true;
            continue;
        }
        if !prev_heading {
            ui.p()
                .rect(Rect::new(r.x + 12.0, r.y, r.w - 24.0, 1.0), ROW_LINE);
        }
        prev_heading = false;
        let hot =
            page.dropdown.is_none() && (ui.hover(r) || (k == page.selected && page.kbd && !any));
        let t = ui.anim(
            id_of(&format!("page:row:{k}:{}", row.label)),
            hot as u8 as f32,
            0.06,
        );
        if page.active == Some(k) {
            ui.p().rect(r, SELECTED);
            ui.p().rect(Rect::new(r.x, r.y, 3.0, r.h), ACCENT);
        }
        lit_row(ui, r, t);
        let cy = r.y + r.h * 0.5;
        let x = r.x + 16.0;
        if page.fields {
            let open = page.dropdown.as_ref().is_some_and(|d| d.row == k);
            let left = draw_control(ui, row, r, l.controls[i], t, open, k);
            let avail = left - 20.0 - x;
            if row.desc.is_empty() {
                put(ui, row.label, x, cy, avail, 14.0, Weight::Regular, FG);
            } else {
                put(ui, row.label, x, cy - 9.5, avail, 14.0, Weight::Regular, FG);
                put(
                    ui,
                    row.desc,
                    x,
                    cy + 11.0,
                    avail,
                    11.0,
                    Weight::Regular,
                    MUTED,
                );
            }
        } else {
            let mut rx = r.right() - 16.0;
            match row.control {
                Control::Opens(v) => {
                    ui.icon(
                        "chevron_right",
                        Vec2::new(rx - 8.0, cy),
                        18.0,
                        MUTED.mix(FG, t),
                    );
                    rx -= 26.0;
                    if !v.is_empty() {
                        rx -= put_right(ui, v, rx, cy, r.w * 0.4, 13.0, Weight::Regular, MUTED)
                            + 10.0;
                    }
                }
                Control::Info(v) | Control::Button(v) | Control::Editing(v) => {
                    rx -= put_right(ui, v, rx, cy, r.w * 0.4, 13.0, Weight::Regular, MUTED) + 12.0;
                }
                _ => {}
            }
            let mut tx = x;
            match row.lead {
                Lead::Sign(s) => {
                    let tile = Rect::new(x, cy - 14.0, sign_w, 28.0);
                    ui.p().rect(tile, ACCENT.lighten(0.1 * t));
                    put_center(ui, s, tile, px, Weight::Bold, ON_ACCENT);
                    tx = tile.right() + 14.0;
                }
                Lead::Code(s) => {
                    put_right(
                        ui,
                        s,
                        x + code_w,
                        cy,
                        code_w,
                        px,
                        Weight::Medium,
                        MUTED.mix(ACCENT, t),
                    );
                    tx = x + code_w + 14.0;
                }
                Lead::None => {}
            }
            let c = if page.active == Some(k) {
                FG
            } else {
                FG.mix(Color::WHITE, t)
            };
            put(ui, row.label, tx, cy, rx - tx, px, Weight::Regular, c);
        }
    }
    if let Some((track, _)) = l.scroll {
        let (t, _) = thumb(track, l.start, l.shown, nl);
        ui.p()
            .rounded(t, 2.0, Color::WHITE.alpha(if any { 0.32 } else { 0.16 }));
    }
    if let Some(fr) = l.footer {
        ui.p().rect(
            Rect::new(fr.x + 8.0, fr.y - 7.0, fr.w - 16.0, 1.0),
            ROW_LINE,
        );
        let back = page.rows.len() - 1;
        let lit = page.dropdown.is_none()
            && (ui.hover(fr) || (page.selected == back && page.kbd && !any));
        back_row(ui, fr, lit);
    }
    if let (Some(p), Some(pc)) = (page.pane.as_ref(), l.pane_card) {
        draw_pane(ui, p, pc, &l);
    }
    if let (Some(dd), Some(panel)) = (page.dropdown.as_ref(), l.dd_panel) {
        ui.p().shadow(panel, 0.0, 16.0, Color::BLACK.alpha(0.55));
        ui.p().rect(panel, Color::hex(0x1b1e23));
        ui.p().rounded_border(panel, 0.0, 1.0, LINE);
        let over = panel.contains(mouse);
        for (i, &r) in l.dd.iter().enumerate() {
            let idx = l.dd_top + i;
            let cur = dd.current == Some(idx);
            let hot = ui.hover(r) || (idx == dd.sel && page.kbd && !over);
            if cur {
                ui.p().rect(r, SELECTED);
                ui.p().rect(Rect::new(r.x, r.y, 3.0, r.h), ACCENT);
            }
            if hot {
                ui.p().rect(r, Color::WHITE.alpha(0.07));
            }
            let c = if cur || hot { FG } else { FG.mix(MUTED, 0.3) };
            put(
                ui,
                dd.items[idx],
                r.x + 14.0,
                r.y + r.h * 0.5,
                r.w - 24.0,
                13.5,
                Weight::Regular,
                c,
            );
        }
        if let Some((track, _)) = l.dd_scroll {
            let (t, _) = thumb(track, l.dd_top, l.dd_rows, dd.items.len());
            ui.p().rounded(t, 2.0, Color::WHITE.alpha(0.3));
        }
    }
    l
}

/// A settings row's control at the right of `r`; returns its left edge.
fn draw_control(
    ui: &mut Ui,
    row: &Row,
    r: Rect,
    ctl: Option<Rect>,
    t: f32,
    open: bool,
    k: usize,
) -> f32 {
    let rx = r.right() - 16.0;
    let cy = r.y + r.h * 0.5;
    match row.control {
        Control::None => rx,
        Control::Switch(on) => {
            let (tw, th) = (38.0, 20.0);
            let track = Rect::new(rx - tw, cy - th * 0.5, tw, th);
            let a = ui.anim(
                id_of(&format!("page:switch:{k}:{}", row.label)),
                on as u8 as f32,
                0.07,
            );
            ui.p().rounded(track, th * 0.5, TRACK_OFF.mix(ACCENT, a));
            let kx = track.x + th * 0.5 + (tw - th) * a;
            ui.p().circle(Vec2::new(kx, cy), th * 0.5 - 3.0, KNOB);
            track.x
        }
        Control::Slider { frac, value } => {
            let tr = ctl.unwrap_or(r);
            put_right(ui, value, rx, cy, VALUE_W, 13.0, Weight::Medium, FG);
            let th = 4.0;
            let track = Rect::new(tr.x, cy - th * 0.5, tr.w, th);
            let f = ui.anim(
                id_of(&format!("page:slider:{k}:{}", row.label)),
                frac.clamp(0.0, 1.0),
                0.06,
            );
            ui.p().rounded(track, th * 0.5, TRACK_OFF);
            if f * track.w >= 1.0 {
                ui.p().rounded(
                    Rect::new(track.x, track.y, track.w * f, th),
                    th * 0.5,
                    ACCENT,
                );
            }
            ui.p()
                .circle(Vec2::new(track.x + track.w * f, cy), 7.0 + t, KNOB);
            tr.x
        }
        Control::Stepper(v) => {
            let b = ctl.unwrap_or(r);
            let bx = Rect::new(b.x, cy - 15.0, b.w, 30.0);
            field_box(ui, bx, if t > 0.5 { ACCENT.alpha(0.6) } else { LINE });
            ui.icon(
                "chevron_left",
                Vec2::new(bx.x + 15.0, cy),
                18.0,
                MUTED.mix(FG, t),
            );
            ui.icon(
                "chevron_right",
                Vec2::new(bx.right() - 15.0, cy),
                18.0,
                MUTED.mix(FG, t),
            );
            put_center(
                ui,
                v,
                Rect::new(bx.x + 30.0, bx.y, bx.w - 60.0, bx.h),
                13.0,
                Weight::Medium,
                FG,
            );
            bx.x
        }
        Control::Opens(v) if v.is_empty() => {
            ui.icon(
                "chevron_right",
                Vec2::new(rx - 8.0, cy),
                20.0,
                MUTED.mix(FG, t),
            );
            rx - 18.0
        }
        Control::Opens(v) => {
            // (a select: the value in a field, an arrow at its end)
            let w = (ui.width(v, 13.0 * TS, Weight::Regular) + 50.0).clamp(130.0, 280.0);
            let bx = Rect::new(rx - w, cy - 15.0, w, 30.0);
            let edge = if open {
                ACCENT
            } else if t > 0.5 {
                ACCENT.alpha(0.6)
            } else {
                LINE
            };
            field_box(ui, bx, edge);
            put(
                ui,
                v,
                bx.x + 12.0,
                cy,
                bx.w - 44.0,
                13.0,
                Weight::Regular,
                FG,
            );
            ui.icon(
                if open { "expand_less" } else { "expand_more" },
                Vec2::new(bx.right() - 16.0, cy),
                18.0,
                if open { ACCENT } else { MUTED },
            );
            bx.x
        }
        Control::Info(v) => rx - put_right(ui, v, rx, cy, r.w * 0.45, 13.0, Weight::Regular, MUTED),
        Control::Editing(v) => {
            let w = (ui.width(v, 13.0 * TS, Weight::Medium) + 36.0).clamp(120.0, 280.0);
            let bx = Rect::new(rx - w, cy - 15.0, w, 30.0);
            field_box(ui, bx, ACCENT);
            let tw = put(
                ui,
                v,
                bx.x + 12.0,
                cy,
                bx.w - 30.0,
                13.0,
                Weight::Medium,
                FG,
            );
            if (ui.time * 1.6).fract() < 0.6 {
                ui.p()
                    .rect(Rect::new(bx.x + 13.0 + tw, cy - 8.0, 1.5, 16.0), ACCENT);
            }
            bx.x
        }
        Control::Button(v) if v.is_empty() => {
            ui.icon(
                "chevron_right",
                Vec2::new(rx - 8.0, cy),
                20.0,
                MUTED.mix(FG, t),
            );
            rx - 18.0
        }
        Control::Button(v) => {
            // (a key cap: the binding, or what the button does)
            let w = (ui.width(v, 12.5 * TS, Weight::Medium) + 28.0).clamp(64.0, 300.0);
            let bx = Rect::new(rx - w, cy - 14.0, w, 28.0);
            field_box(ui, bx, if t > 0.5 { ACCENT.alpha(0.6) } else { LINE });
            put_center(ui, v, bx.pad(-10.0, 0.0), 12.5, Weight::Medium, FG);
            bx.x
        }
    }
}

/// The timetable beside a list.
fn draw_pane(ui: &mut Ui, p: &Pane, pc: Rect, l: &Layout) {
    card(ui, pc, p.title, "");
    put(
        ui,
        p.meta,
        pc.x + 20.0,
        pc.y + 54.0,
        pc.w - 40.0,
        11.0,
        Weight::Regular,
        MUTED,
    );
    ui.p().rect(
        Rect::new(pc.x + 16.0, pc.y + 70.0, pc.w - 32.0, 1.0),
        ROW_LINE,
    );
    let mut y = pc.y + 80.0;
    if let Some(time) = p.time {
        for (j, &r) in l.time.iter().enumerate() {
            let hov = ui.hover(r);
            ui.p().rect(r, if hov { ACCENT } else { FIELD });
            ui.p()
                .rounded_border(r, 0.0, 1.0, if hov { ACCENT } else { LINE });
            ui.icon(
                if j == 0 {
                    "chevron_left"
                } else {
                    "chevron_right"
                },
                r.center(),
                20.0,
                if hov { ON_ACCENT } else { ACCENT },
            );
        }
        if let (Some(a), Some(b)) = (l.time.first(), l.time.get(1)) {
            put_center(
                ui,
                time,
                Rect::new(a.right() + 8.0, a.y, b.x - a.right() - 16.0, a.h),
                18.0,
                Weight::Bold,
                ACCENT,
            );
            y = a.bottom() + 10.0;
        }
    }
    let n = p.rows.len();
    if let (Some(chosen), Some(button)) = (p.chosen, p.button) {
        for (i, &r) in l.pane_rows.iter().enumerate() {
            let idx = l.pane_first + i;
            let (what, when) = p.rows[idx];
            let on = idx == chosen;
            if on {
                ui.p().rect(r, SELECTED);
                ui.p()
                    .rect(Rect::new(r.x, r.y + 4.0, 3.0, r.h - 8.0), ACCENT);
            } else if ui.hover(r) {
                ui.p().rect(r, Color::WHITE.alpha(0.05));
            }
            let cy = r.y + r.h * 0.5;
            let tw = put_right(
                ui,
                when,
                r.right() - 12.0,
                cy,
                70.0,
                12.5,
                Weight::Medium,
                ACCENT,
            );
            put(
                ui,
                what,
                r.x + 12.0,
                cy,
                r.w - tw - 36.0,
                12.5,
                Weight::Regular,
                if on { FG } else { FG.mix(MUTED, 0.35) },
            );
        }
        if let Some((track, _, n, fit)) = l.pane_scroll {
            let (t, _) = thumb(track, l.pane_first, fit, n);
            let a = if pc.contains(ui.input.mouse) {
                0.32
            } else {
                0.16
            };
            ui.p().rounded(t, 2.0, Color::WHITE.alpha(a));
        }
        if let Some(go) = l.pane_go {
            let hov = ui.hover(go);
            ui.p()
                .rect(go, if hov { ACCENT.lighten(0.1) } else { ACCENT });
            put_center(ui, button, go, 14.0, Weight::Bold, ON_ACCENT);
        }
    } else {
        // (only to read: the tours of a line, their first stop's time)
        let fit = ((pc.bottom() - 12.0 - y) / PANE_ROW).floor().max(1.0) as usize;
        let shown = if n > fit { fit.saturating_sub(1) } else { n };
        let tour = format!("{} ", crate::tr("Tour"));
        let tile_w = p.rows[..shown]
            .iter()
            .filter_map(|r| r.0.strip_prefix(tour.as_str()))
            .map(|rest| {
                rest.split_once("  ›  ")
                    .map(|(n, _)| n)
                    .unwrap_or(rest)
                    .trim()
            })
            .map(|num| ui.width(num, 12.5 * TS, Weight::Bold) + 16.0)
            .fold(32.0f32, f32::max);
        for (i, (what, when)) in p.rows[..shown].iter().enumerate() {
            let r = Rect::new(pc.x + 8.0, y + i as f32 * PANE_ROW, pc.w - 16.0, PANE_ROW);
            let cy = r.y + r.h * 0.5;
            let tw = put_right(
                ui,
                when,
                r.right() - 12.0,
                cy,
                70.0,
                12.5,
                Weight::Medium,
                ACCENT,
            );
            match what.strip_prefix(tour.as_str()) {
                Some(rest) => {
                    let (num, dest) = rest.split_once("  ›  ").unwrap_or((rest, ""));
                    let tile = Rect::new(r.x + 12.0, cy - 10.0, tile_w, 20.0);
                    ui.p().rect(tile, ACCENT.alpha(0.16));
                    put_center(ui, num.trim(), tile, 12.5, Weight::Bold, ACCENT);
                    let dx = tile.right() + 10.0;
                    put(
                        ui,
                        dest,
                        dx,
                        cy,
                        r.right() - 24.0 - tw - dx,
                        12.5,
                        Weight::Regular,
                        FG.mix(MUTED, 0.35),
                    );
                }
                None => {
                    put(
                        ui,
                        what,
                        r.x + 12.0,
                        cy,
                        r.w - tw - 36.0,
                        12.5,
                        Weight::Regular,
                        FG.mix(MUTED, 0.35),
                    );
                }
            }
        }
        if n > shown {
            let cy = y + shown as f32 * PANE_ROW + PANE_ROW * 0.5;
            let more = crate::tr("+{} more").replacen("{}", &(n - shown).to_string(), 1);
            put(
                ui,
                &more,
                pc.x + 20.0,
                cy,
                pc.w - 40.0,
                11.5,
                Weight::Regular,
                MUTED,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Row<'static>> {
        let mut v = vec![Row {
            label: "Steering",
            heading: true,
            ..Row::plain("")
        }];
        for k in 0..30 {
            v.push(Row {
                label: "Row",
                desc: if k % 2 == 0 { "What it does" } else { "" },
                control: match k % 4 {
                    0 => Control::Switch(true),
                    1 => Control::Slider {
                        frac: 0.4,
                        value: "40 %",
                    },
                    2 => Control::Stepper("Medium"),
                    _ => Control::Opens("Rain"),
                },
                ..Row::plain("")
            });
        }
        v
    }

    fn page<'a>(rows: &'a [Row<'a>], tabs: &'a [&'a str]) -> Page<'a> {
        Page {
            title: "Graphics",
            note: "",
            tabs_title: "Options",
            tabs,
            tab: 1,
            rows,
            fields: true,
            selected: 0,
            kbd: true,
            active: None,
            top: None,
            back: Back::Side,
            pane: None,
            pane_first: None,
            dropdown: None,
        }
    }

    const AREA: Rect = Rect::new(340.0, 45.0, 1332.0, 896.0);

    #[test]
    fn the_rows_shown_fit_the_card_and_keep_the_chosen_one_in_view() {
        let rows = rows();
        let tabs = ["General", "Graphics", "Sound"];
        let mut p = page(&rows, &tabs);
        p.selected = 25;
        let l = layout(&p, AREA);
        assert!(l.shown < rows.len());
        assert_eq!(l.rows.len(), l.shown);
        assert!(l.start <= 25 && 25 < l.start + l.shown);
        for r in &l.rows {
            assert!(r.y >= l.card.y + HEAD - 0.01 && r.bottom() <= l.card.bottom() + 0.01);
            assert!(r.x >= l.card.x && r.right() <= l.card.right());
        }
        assert!(l.scroll.is_some());
        // the pages, and the way back last at the foot of their card
        assert_eq!(l.tabs.len(), tabs.len() + 1);
        let tc = l.tabs_card.unwrap();
        assert!(l.tabs[3].bottom() <= tc.bottom() && l.tabs[2].bottom() < l.tabs[3].y);
        // a scrolled list starts where it was scrolled to
        p.top = Some(3.0);
        assert_eq!(layout(&p, AREA).start, 3);
    }

    #[test]
    fn a_slider_and_a_stepper_take_clicks_on_their_control_only() {
        let rows = rows();
        let l = layout(&page(&rows, &["A"]), AREA);
        for (k, (r, c)) in l.rows.iter().zip(&l.controls).enumerate() {
            match rows[l.start + k].control {
                Control::Slider { .. } | Control::Stepper(_) => {
                    let c = c.expect("a control");
                    assert!(c.x > r.x + r.w * 0.4 && c.right() <= r.right());
                }
                _ => assert!(c.is_none()),
            }
        }
    }

    #[test]
    fn the_way_back_of_a_list_is_pinned_at_its_rows_index() {
        let labels: Vec<String> = (0..40).map(|k| format!("Driver {k}")).collect();
        let mut rows: Vec<Row> = labels.iter().map(|s| Row::plain(s)).collect();
        rows.push(Row::plain("Back"));
        let mut p = page(&rows, &[]);
        p.fields = false;
        p.back = Back::Footer;
        p.selected = 30;
        let l = layout(&p, AREA);
        assert!(l.tabs_card.is_none() && l.tabs.is_empty());
        let back = rows.len() - 1;
        assert_eq!(l.rows.len(), back - l.start + 1);
        assert_eq!(Some(l.rows[back - l.start]), l.footer);
        // (the rows between the last shown and the way back are nowhere)
        let last = l.start + l.shown;
        assert!(l.rows[last - l.start].x < -1.0e8);
        assert!(l.footer.unwrap().bottom() <= l.card.bottom());
    }

    #[test]
    fn a_drop_down_opens_under_its_row_or_over_it_and_stays_in_the_card() {
        let rows = rows();
        let items = ["Clear", "Cloudy", "Rain", "Snow", "Fog"];
        for row in [1usize, 10] {
            let mut p = page(&rows, &["A"]);
            p.selected = row;
            p.top = Some(0.0);
            p.dropdown = Some(Dropdown {
                row,
                items: &items,
                sel: 0,
                top: 0,
                current: Some(2),
            });
            let l = layout(&p, AREA);
            let panel = l.dd_panel.expect("open");
            assert_eq!(l.dd.len(), items.len());
            assert!(
                panel.y >= l.card.y && panel.bottom() <= l.card.bottom(),
                "{row}"
            );
            let rr = l.rows[row - l.start];
            assert!(panel.y >= rr.bottom() || panel.bottom() <= rr.y, "{row}");
        }
    }

    #[test]
    fn the_timetable_beside_a_list_has_its_stops_its_arrows_and_its_button() {
        let labels: Vec<String> = (0..6).map(|k| format!("Tour {k}")).collect();
        let mut rows: Vec<Row> = labels.iter().map(|s| Row::plain(s)).collect();
        rows.push(Row::plain("Back"));
        let stops: Vec<(&str, &str)> = (0..40).map(|_| ("Main Street", "08:42")).collect();
        let mut p = page(&rows, &[]);
        p.fields = false;
        p.back = Back::Footer;
        p.pane = Some(Pane {
            title: "Tour 3",
            meta: "Line 50",
            rows: &stops,
            chosen: Some(20),
            button: Some("Start trip"),
            time: Some("08:42"),
        });
        let l = layout(&p, AREA);
        let pc = l.pane_card.expect("beside");
        assert!(pc.x >= l.card.right());
        assert_eq!(l.time.len(), 2);
        let go = l.pane_go.unwrap();
        assert!(l
            .pane_rows
            .iter()
            .all(|r| r.bottom() <= go.y && r.y >= l.time[0].bottom()));
        assert!(l.pane_first <= 20 && 20 < l.pane_first + l.pane_rows.len());
        assert!(l.pane_scroll.is_some());
        // too narrow for it: no timetable
        let narrow = layout(&p, Rect::new(340.0, 45.0, 500.0, 600.0));
        assert!(narrow.pane_card.is_none() && narrow.pane_rows.is_empty());
    }

    #[test]
    fn draw_puts_everything_where_layout_says() {
        let rows = rows();
        let tabs = ["General", "Graphics"];
        let p = page(&rows, &tabs);
        let mut ui = Ui::new();
        ui.begin(Vec2::new(1672.0, 941.0), 1.0, 0.016);
        let l = draw(&mut ui, &p, AREA);
        ui.finish();
        assert_eq!(l, layout(&p, AREA));
    }
}

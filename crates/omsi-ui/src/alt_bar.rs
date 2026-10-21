//! The Alt icon bar: a panel of square tiles in rows of five, with the name of the hovered
//! (or keyboard-chosen) tile in a tip over it and an arrow that folds the bar down to a small
//! tab in the corner; and the bar over the top of the screen that says where the bus is. The
//! caller supplies the buttons; what they do is the game's.

use crate::paint::Align;
use crate::theme::v5::{BAR, FG, MUTED, OK, TS};
use crate::theme::{accent, on as on_color};
use crate::widgets::{id_of, Ui};
use crate::{Color, Rect, Weight};
use glam::Vec2;

pub const COLUMNS: usize = 5;
pub const CELL_W: f32 = 76.0;
pub const CELL_H: f32 = 76.0;
pub const GAP: f32 = 8.0;
/// The panel's padding round the tiles.
pub const PAD: f32 = 12.0;
/// The column at the right of the tiles with the arrow that folds the bar.
pub const ARROW_W: f32 = 26.0;
pub const BAR_H: f32 = 40.0;
/// The folded bar's tab.
pub const TAB: Vec2 = Vec2::new(30.0, 40.0);

/// The panel behind the tiles.
const PANEL: Color = Color::rgba(14, 15, 17, 0.92);
const EDGE: Color = Color::rgba(255, 255, 255, 0.12);
/// A tile that cannot be used now, and its icon.
const DIM_FILL: Color = Color::rgba(255, 255, 255, 0.07);
const DIM_ICON: Color = Color::hex(0x6f747e);

/// What a button shows: a Material icon, or a short word (the "AI" button).
#[derive(Debug, Clone, Copy)]
pub enum Glyph<'a> {
    Icon(&'a str),
    Text(&'a str),
}

pub struct Button<'a> {
    pub id: &'a str,
    pub glyph: Glyph<'a>,
    /// What the button does, shown over the bar while the mouse is over it.
    pub title: &'a str,
    /// Why it cannot be used now (drawn dim, the reason shows instead of the title).
    pub disabled: Option<&'a str>,
    /// What it switches is on now (a tour is running): drawn filled.
    pub on: bool,
}

/// The state the game keeps between frames: which button was chosen last.
#[derive(Default, Debug, Clone)]
pub struct State {
    pub chosen: Option<String>,
    /// The button the keyboard is on (a ring round it), if the game drives the bar by keys.
    pub focus: Option<String>,
}

/// What a click on the bar asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// Button `.0`.
    Button(usize),
    /// The arrow: fold the bar, or unfold it.
    Arrow,
}

/// The panel's size for `n` buttons.
pub fn panel_size(n: usize) -> Vec2 {
    let rows = n.div_ceil(COLUMNS).max(1) as f32;
    Vec2::new(
        2.0 * PAD + COLUMNS as f32 * CELL_W + (COLUMNS - 1) as f32 * GAP + ARROW_W,
        2.0 * PAD + rows * CELL_H + (rows - 1.0) * GAP,
    )
}

/// Where button `i` is, for a bar whose top left corner is `at`.
pub fn cell_rect(at: Vec2, i: usize) -> Rect {
    let (col, row) = ((i % COLUMNS) as f32, (i / COLUMNS) as f32);
    Rect::new(
        at.x + PAD + col * (CELL_W + GAP),
        at.y + PAD + row * (CELL_H + GAP),
        CELL_W,
        CELL_H,
    )
}

/// The arrow that folds the bar, in its bottom right corner.
pub fn arrow_rect(at: Vec2, n: usize) -> Rect {
    let size = panel_size(n);
    Rect::new(
        at.x + size.x - PAD - ARROW_W + 4.0,
        at.y + size.y - PAD - 30.0,
        ARROW_W - 4.0,
        30.0,
    )
}

/// The panel's rectangle for `n` buttons with its top left corner at `at`.
pub fn panel_rect(at: Vec2, n: usize) -> Rect {
    let size = panel_size(n);
    Rect::new(at.x, at.y, size.x, size.y)
}

/// The folded bar's tab, whose bottom right corner is `corner`.
pub fn tab_rect(corner: Vec2) -> Rect {
    Rect::new(corner.x - TAB.x, corner.y - TAB.y, TAB.x, TAB.y)
}

fn inside(r: Rect, p: Vec2) -> bool {
    p.x >= r.x && p.x < r.x + r.w && p.y >= r.y && p.y < r.y + r.h
}

/// The button under `point` (`None` over the gaps and outside the panel). A disabled button
/// is found too: the caller knows which can be used.
pub fn hit(at: Vec2, n: usize, point: Vec2) -> Option<usize> {
    (0..n).find(|&i| inside(cell_rect(at, i), point))
}

/// What is under `point`: a button or the arrow.
pub fn hit_any(at: Vec2, n: usize, point: Vec2) -> Option<Hit> {
    hit(at, n, point)
        .map(Hit::Button)
        .or_else(|| inside(arrow_rect(at, n), point).then_some(Hit::Arrow))
}

#[allow(clippy::too_many_arguments)]
fn text(ui: &mut Ui, s: &str, x: f32, cy: f32, px: f32, weight: Weight, c: Color, a: Align) {
    let px = px * TS;
    let w = ui.width(s, px, weight);
    let r = match a {
        Align::Right => Rect::new(x - w - 2.0, cy - px, w + 2.0, px * 2.0),
        _ => Rect::new(x, cy - px, w + 2.0, px * 2.0),
    };
    ui.text_in(s, r, px, weight, c, a);
}

/// The tip over the bar: what the tile under the mouse (or the keyboard) does, or why it
/// cannot be used; centred over the tile, kept inside the picture.
fn tip(ui: &mut Ui, s: &str, over: Rect, panel: Rect) {
    let px = 12.0 * TS;
    let w = ui.width(s, px, Weight::Regular) + 20.0;
    // (over the bar, not out past its left edge, where a window may stand)
    let lo = panel.x.min(ui.size.x - w - 8.0).max(8.0);
    let x = (over.center().x - w * 0.5).clamp(lo, (ui.size.x - w - 8.0).max(lo));
    let r = Rect::new(x, panel.y - 30.0, w, 24.0);
    ui.p().rounded(r, 4.0, Color::rgba(14, 15, 17, 0.95));
    ui.p().rounded_border(r, 4.0, 1.0, EDGE);
    ui.text_in(s, r, px, Weight::Regular, FG, Align::Center);
}

/// Draw the bar at `at` (its top left corner) and return the button clicked this frame.
pub fn draw<'a>(
    ui: &mut Ui,
    at: Vec2,
    buttons: &'a [Button<'a>],
    state: &mut State,
) -> Option<&'a str> {
    let size = panel_size(buttons.len());
    let p = Rect::new(at.x, at.y, size.x, size.y);
    ui.solid(p);
    ui.p().shadow(p, 8.0, 24.0, Color::rgba(0, 0, 0, 0.55));
    ui.p().rounded(p, 8.0, PANEL);
    ui.p().rounded_border(p, 8.0, 1.0, EDGE);
    let mut clicked = None;
    let mut shown: Option<(&Button, Rect)> = None;
    for (i, b) in buttons.iter().enumerate() {
        let r = cell_rect(at, i);
        let (hover, _, click) = ui.interact(id_of(&format!("alt:{}", b.id)), r);
        let enabled = b.disabled.is_none();
        let a = accent();
        let focus = state.focus.as_deref() == Some(b.id);
        let chosen = state.chosen.as_deref() == Some(b.id);
        // a tile on is filled; one that can be used is tinted; one that cannot is grey
        let (fill, edge, ink) = if b.on {
            (a.alpha(0.85), Color::rgba(0, 0, 0, 0.0), on_color(a))
        } else if !enabled {
            (DIM_FILL, Color::rgba(0, 0, 0, 0.0), DIM_ICON)
        } else if hover {
            (a.alpha(0.38), a.alpha(0.7), a)
        } else {
            (a.alpha(0.22), a.alpha(0.4), a)
        };
        ui.p().rounded(r, 6.0, fill);
        ui.p().rounded_border(r, 6.0, 1.0, edge);
        if focus || (chosen && b.on) {
            ui.p().rounded_border(
                Rect::new(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0),
                8.0,
                2.0,
                a,
            );
        }
        match b.glyph {
            Glyph::Icon(name) => ui.icon(name, r.center(), 28.0, ink),
            Glyph::Text(t) => {
                let px = 20.0 * TS;
                ui.text_in(t, r, px, Weight::Bold, ink, Align::Center);
            }
        }
        if hover || focus {
            shown = Some((b, r));
        }
        if click && enabled {
            state.chosen = Some(b.id.to_string());
            clicked = Some(b.id);
        }
    }
    // the arrow that folds the bar
    let ar = arrow_rect(at, buttons.len());
    let (hover, _, _) = ui.interact(id_of("alt:fold"), ar);
    ui.icon(
        "chevron_right",
        ar.center(),
        18.0,
        if hover { FG } else { MUTED },
    );
    // the tip: the hovered tile's name or reason, else the open window's
    let shown = shown.or_else(|| {
        buttons
            .iter()
            .enumerate()
            .find(|(_, b)| b.on && state.chosen.as_deref() == Some(b.id))
            .map(|(i, b)| (b, cell_rect(at, i)))
    });
    if let Some((b, r)) = shown {
        tip(ui, b.disabled.unwrap_or(b.title), r, p);
    } else if hover {
        tip(ui, "Fold the bar", ar, p);
    }
    clicked
}

/// The folded bar: a small tab with an arrow, its bottom right corner at `corner`.
pub fn draw_folded(ui: &mut Ui, corner: Vec2) {
    let r = tab_rect(corner);
    ui.solid(r);
    let (hover, _, _) = ui.interact(id_of("alt:unfold"), r);
    ui.p().rounded(r, 6.0, PANEL);
    ui.p().rounded_border(r, 6.0, 1.0, EDGE);
    ui.icon(
        "chevron_left",
        r.center(),
        18.0,
        if hover { FG } else { MUTED },
    );
    if hover {
        let px = 12.0 * TS;
        let s = "Unfold the bar";
        let w = ui.width(s, px, Weight::Regular) + 20.0;
        let t = Rect::new(r.right() - w, r.y - 30.0, w, 24.0);
        ui.p().rounded(t, 4.0, Color::rgba(14, 15, 17, 0.95));
        ui.p().rounded_border(t, 4.0, 1.0, EDGE);
        ui.text_in(s, t, px, Weight::Regular, FG, Align::Center);
    }
}

/// The line at the bottom left while the bar is shown: how it closes, and that the game
/// goes on.
pub fn hint(ui: &mut Ui, s: &str) {
    let px = 12.0 * TS;
    let w = ui.width(s, px, Weight::Regular) + 20.0;
    let r = Rect::new(20.0, ui.size.y - 18.0 - 24.0, w, 24.0);
    ui.p().rounded(r, 4.0, Color::rgba(14, 15, 17, 0.6));
    ui.text_in(s, r, px, Weight::Regular, FG, Align::Center);
}

/// What the top bar says.
pub struct TopBar<'a> {
    /// "Line 50 · Example route"
    pub route: &'a str,
    /// "Central Station"
    pub next_stop: Option<&'a str>,
    /// "08:42"
    pub clock: &'a str,
    /// "open" or "closed", when a door state is to be shown.
    pub doors: Option<(&'a str, bool)>,
    /// The key hints at the right.
    pub hint: &'a str,
}

/// The bar over the top of the screen.
pub fn top_bar(ui: &mut Ui, bar: &TopBar) {
    let w = ui.size.x;
    ui.p().rect(Rect::new(0.0, 0.0, w, BAR_H), BAR.alpha(0.96));
    let cy = BAR_H * 0.5;
    let mut x = 20.0;
    let route = bar.route.to_string();
    text(ui, &route, x, cy, 14.0, Weight::Bold, FG, Align::Left);
    x += ui.width(&route, 14.0 * TS, Weight::Bold) + 29.0;
    if let Some(stop) = bar.next_stop {
        text(
            ui,
            "Next stop:",
            x,
            cy,
            14.0,
            Weight::Regular,
            MUTED,
            Align::Left,
        );
        x += ui.width("Next stop: ", 14.0 * TS, Weight::Regular);
        text(ui, stop, x, cy, 14.0, Weight::Bold, FG, Align::Left);
        x += ui.width(stop, 14.0 * TS, Weight::Bold) + 29.0;
    }
    text(ui, "Time", x, cy, 14.0, Weight::Regular, MUTED, Align::Left);
    x += ui.width("Time ", 14.0 * TS, Weight::Regular);
    text(ui, bar.clock, x, cy, 14.0, Weight::Bold, FG, Align::Left);
    x += ui.width(bar.clock, 14.0 * TS, Weight::Bold) + 29.0;
    if let Some((state, open)) = bar.doors {
        let c = if open { accent() } else { OK };
        text(
            ui,
            &format!("Doors: {state}"),
            x,
            cy,
            14.0,
            Weight::Regular,
            c,
            Align::Left,
        );
    }
    text(
        ui,
        bar.hint,
        w - 20.0,
        cy,
        13.0,
        Weight::Regular,
        MUTED,
        Align::Right,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buttons() -> Vec<Button<'static>> {
        let b = |id, icon, disabled| Button {
            id,
            glyph: Glyph::Icon(icon),
            title: id,
            disabled,
            on: false,
        };
        vec![
            b("add", "add", None),
            b("swap", "sync_alt", None),
            b("warn", "warning", Some("not available now")),
        ]
    }

    fn frame(ui: &mut Ui, bs: &[Button], state: &mut State, click: Option<Vec2>) -> Option<String> {
        let run = |ui: &mut Ui, state: &mut State| {
            ui.begin(Vec2::new(1672.0, 941.0), 1.0, 0.016);
            let out = draw(ui, Vec2::new(1366.0, 723.0), bs, state).map(String::from);
            ui.finish();
            out
        };
        if let Some(p) = click {
            ui.input.mouse = p;
            ui.input.pressed = true;
            ui.input.down = true;
            run(ui, state);
            ui.input.released = true;
            ui.input.down = false;
        }
        run(ui, state)
    }

    #[test]
    fn a_click_chooses_a_button_and_a_disabled_one_cannot_be_chosen() {
        let bs = buttons();
        let mut ui = Ui::new();
        let mut st = State::default();
        frame(&mut ui, &bs, &mut st, None);
        let centre = |ui: &Ui, id: &str| ui.drawn[&id_of(&format!("alt:{id}"))].center();
        let swap = centre(&ui, "swap");
        assert_eq!(
            frame(&mut ui, &bs, &mut st, Some(swap)).as_deref(),
            Some("swap")
        );
        assert_eq!(st.chosen.as_deref(), Some("swap"));
        let warn = centre(&ui, "warn");
        assert_eq!(frame(&mut ui, &bs, &mut st, Some(warn)), None);
        assert_eq!(st.chosen.as_deref(), Some("swap"));
    }

    #[test]
    fn hit_finds_the_buttons_where_draw_puts_them() {
        let bs = buttons();
        let mut ui = Ui::new();
        let mut st = State::default();
        frame(&mut ui, &bs, &mut st, None);
        let at = Vec2::new(1366.0, 723.0);
        for (i, b) in bs.iter().enumerate() {
            let c = ui.drawn[&id_of(&format!("alt:{}", b.id))].center();
            assert_eq!(hit(at, bs.len(), c), Some(i));
        }
        assert_eq!(hit(at, bs.len(), Vec2::new(0.0, 0.0)), None);
        // the padding and the gap between two buttons are no button, the arrow is the arrow
        assert_eq!(hit(at, bs.len(), at + Vec2::new(6.0, 6.0)), None);
        let gap = cell_rect(at, 0).right() + GAP * 0.5;
        assert_eq!(hit(at, bs.len(), Vec2::new(gap, at.y + PAD + 10.0)), None);
        let arrow = arrow_rect(at, bs.len()).center();
        assert_eq!(hit_any(at, bs.len(), arrow), Some(Hit::Arrow));
        assert_eq!(
            hit_any(at, bs.len(), cell_rect(at, 1).center()),
            Some(Hit::Button(1))
        );
        let r = panel_rect(at, bs.len());
        assert!(r.w > 0.0 && r.h > 0.0);
    }

    #[test]
    fn a_focused_button_is_not_a_choice() {
        let bs = buttons();
        let mut ui = Ui::new();
        let mut st = State {
            focus: Some("swap".into()),
            ..Default::default()
        };
        assert_eq!(frame(&mut ui, &bs, &mut st, None), None);
        assert!(st.chosen.is_none());
    }

    #[test]
    fn the_panel_has_a_row_for_every_five_buttons() {
        let one = panel_size(1);
        assert_eq!(panel_size(5), one);
        assert!(panel_size(6).y > one.y);
        assert_eq!(panel_size(15).y - one.y, 2.0 * (CELL_H + GAP));
    }
}

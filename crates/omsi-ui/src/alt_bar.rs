//! The Alt icon bar: a panel of square icon buttons in rows of five, with the name of the
//! hovered (or last chosen) button above them, and the bar over the top of the screen that
//! says where the bus is. The caller supplies the buttons; what they do is the game's.

use crate::paint::Align;
use crate::widgets::{id_of, Ui};
use crate::{Color, Rect, Weight};
use glam::Vec2;

const PANEL: Color = Color::hex(0x171a1f);
const CELL: Color = Color::hex(0x1b1d22);
const BAR: Color = Color::hex(0x0e1013);
const LINE: Color = Color::hex(0x2b2f37);
const ROW_LINE: Color = Color::hex(0x20242b);
const FG: Color = Color::hex(0xe9eaec);
const MUTED: Color = Color::hex(0x9aa0aa);
const ACCENT: Color = Color::hex(0xf0a30a);
const OK: Color = Color::hex(0x4fc27a);
const PANEL_ALPHA: f32 = 0.92;
const TS: f32 = 1.16;

pub const COLUMNS: usize = 5;
pub const CELL_W: f32 = 50.0;
pub const CELL_H: f32 = 45.0;
pub const GAP: f32 = 5.6;
pub const HEADER_H: f32 = 34.0;
pub const BAR_H: f32 = 40.0;

/// What a button shows: a Material icon, or a short word (the "AI" button).
#[derive(Debug, Clone, Copy)]
pub enum Glyph<'a> {
    Icon(&'a str),
    Text(&'a str),
}

pub struct Button<'a> {
    pub id: &'a str,
    pub glyph: Glyph<'a>,
    /// What the button does, shown above the grid while the mouse is over it.
    pub title: &'a str,
    /// Why it cannot be used now (drawn dim, the reason shows instead of the title).
    pub disabled: Option<&'a str>,
}

/// The state the game keeps between frames: which button was chosen last.
#[derive(Default, Debug, Clone)]
pub struct State {
    pub chosen: Option<String>,
}

/// The panel's size for `n` buttons.
pub fn panel_size(n: usize) -> Vec2 {
    let rows = n.div_ceil(COLUMNS).max(1) as f32;
    Vec2::new(
        18.0 + COLUMNS as f32 * CELL_W + (COLUMNS - 1) as f32 * GAP,
        HEADER_H + 9.0 + rows * (CELL_H + 6.0) + 4.0,
    )
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
    ui.p().rect(p, PANEL.alpha(PANEL_ALPHA));
    ui.p().rounded_border(p, 0.0, 1.0, LINE);
    let mut clicked = None;
    let mut hovered = None;
    for (i, b) in buttons.iter().enumerate() {
        let (col, row) = ((i % COLUMNS) as f32, (i / COLUMNS) as f32);
        let r = Rect::new(
            p.x + 9.0 + col * (CELL_W + GAP),
            p.y + HEADER_H + 9.0 + row * (CELL_H + 6.0),
            CELL_W,
            CELL_H,
        );
        let (hover, _, click) = ui.interact(id_of(&format!("alt:{}", b.id)), r);
        let enabled = b.disabled.is_none();
        let chosen = state.chosen.as_deref() == Some(b.id);
        ui.p().rect(
            r,
            if hover && enabled {
                CELL.lighten(0.08)
            } else {
                CELL
            },
        );
        ui.p()
            .rounded_border(r, 0.0, 1.0, if chosen { ACCENT } else { LINE });
        let c = if enabled { ACCENT } else { ACCENT.alpha(0.3) };
        match b.glyph {
            Glyph::Icon(name) => ui.icon(name, r.center(), 20.0, c),
            Glyph::Text(t) => {
                let px = 17.0 * TS;
                let w = ui.width(t, px, Weight::Bold);
                ui.text_in(
                    t,
                    Rect::new(r.center().x - w * 0.5, r.y, w + 2.0, r.h),
                    px,
                    Weight::Bold,
                    c,
                    Align::Left,
                );
            }
        }
        if hover {
            hovered = Some(b);
        }
        if click && enabled {
            state.chosen = Some(b.id.to_string());
            clicked = Some(b.id);
        }
    }
    // the header: the hovered button's name or reason, else the last chosen one's
    let shown = hovered.or_else(|| {
        buttons
            .iter()
            .find(|b| state.chosen.as_deref() == Some(b.id))
    });
    if let Some(b) = shown {
        ui.icon("play_arrow", Vec2::new(p.x + 15.0, p.y + 17.0), 9.0, ACCENT);
        text(
            ui,
            b.disabled.unwrap_or(b.title),
            p.x + 27.0,
            p.y + 17.0,
            13.0,
            Weight::Regular,
            FG,
            Align::Left,
        );
    }
    ui.p().rect(
        Rect::new(p.x + 1.0, p.y + HEADER_H, p.w - 2.0, 1.0),
        ROW_LINE,
    );
    clicked
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
        let c = if open { ACCENT } else { OK };
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
    fn the_panel_has_a_row_for_every_five_buttons() {
        let one = panel_size(1);
        assert_eq!(panel_size(5), one);
        assert!(panel_size(6).y > one.y);
        assert_eq!(panel_size(15).y - one.y, 2.0 * (CELL_H + 6.0));
    }
}

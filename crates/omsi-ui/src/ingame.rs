//! The in-game windows' building blocks: a window frame with a letter-spaced title and a
//! note, a table with a chosen row, a list of label and value, and a row of buttons. The
//! windows of the concept (vehicles in the session, choose a vehicle, end-of-trip report,
//! menu, passengers and stop) are made of these by the game, with its own data.
//!
//! Whether a window pauses the simulation is not decided here (#166).

use crate::paint::Align;
use crate::widgets::{id_of, Ui};
use crate::{Color, Rect, Weight};

const PANEL: Color = Color::hex(0x171a1f);
const LINE: Color = Color::hex(0x2b2f37);
const ROW_LINE: Color = Color::hex(0x20242b);
const FG: Color = Color::hex(0xe9eaec);
const MUTED: Color = Color::hex(0x9aa0aa);
const ACCENT: Color = Color::hex(0xf0a30a);
const ON_ACCENT: Color = Color::hex(0x14120a);
const FIELD: Color = Color::hex(0x1b1d22);
const SELECTED: Color = Color::hex(0x2a2316);
const PANEL_ALPHA: f32 = 0.92;
const TS: f32 = 1.16;

pub const TITLE_H: f32 = 40.0;
pub const ROW_H: f32 = 34.0;

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

fn text_right(ui: &mut Ui, s: &str, x: f32, cy: f32, px: f32, weight: Weight, c: Color) {
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

/// A window: the panel, its title in capitals and a note at the right. Returns the area
/// under the title bar.
pub fn window(ui: &mut Ui, r: Rect, title: &str, note: &str) -> Rect {
    ui.solid(r);
    ui.p().rect(r, PANEL.alpha(PANEL_ALPHA));
    ui.p().rounded_border(r, 0.0, 1.0, LINE);
    text(
        ui,
        &title.to_uppercase(),
        r.x + 16.0,
        r.y + 20.0,
        13.0,
        Weight::Bold,
        FG,
    );
    text_right(
        ui,
        note,
        r.right() - 16.0,
        r.y + 20.0,
        12.0,
        Weight::Regular,
        MUTED,
    );
    ui.p()
        .rect(Rect::new(r.x + 1.0, r.y + TITLE_H, r.w - 2.0, 1.0), LINE);
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
            ui.p().rect(r, SELECTED);
            ui.p().rect(Rect::new(r.x, r.y, 3.0, r.h), ACCENT);
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

/// A row of buttons from `at`; the first is the accent one. Returns the index clicked.
pub fn buttons(ui: &mut Ui, name: &str, at: glam::Vec2, labels: &[&str]) -> Option<usize> {
    let mut x = at.x;
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let w = ui.width(label, 14.0 * TS, Weight::Medium) + 44.0;
        let r = Rect::new(x, at.y, w, 39.0);
        let (hover, _, click) = ui.interact(id_of(&format!("{name}:btn:{i}")), r);
        let primary = i == 0;
        let fill = if primary { ACCENT } else { FIELD };
        ui.p()
            .rect(r, if hover { fill.lighten(0.08) } else { fill });
        if !primary {
            ui.p().rounded_border(r, 0.0, 1.0, LINE);
        }
        ui.text_in(
            label,
            r,
            14.0 * TS,
            if primary {
                Weight::Bold
            } else {
                Weight::Medium
            },
            if primary { ON_ACCENT } else { FG },
            Align::Center,
        );
        if click {
            clicked = Some(i);
        }
        x += w + 8.0;
    }
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
        let mut run = |ui: &mut Ui, click: Option<Vec2>| {
            let mut frame = |ui: &mut Ui| {
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
}

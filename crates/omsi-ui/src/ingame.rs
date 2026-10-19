//! The in-game windows' building blocks: a window frame with a letter-spaced title and a
//! note, a table with a chosen row, a list of label and value, and a row of buttons. The
//! windows of the concept (vehicles in the session, choose a vehicle, end-of-trip report,
//! menu, passengers and stop) are made of these by the game, with its own data.
//!
//! Opening these windows never pauses the simulation by itself: what the caller shows in
//! them (for example the end-of-trip report's note) is the caller's business.

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

/// A list of `labels` in a scrolled column of `area`, the chosen one marked. Returns the one
/// clicked, if one was.
pub fn list(
    ui: &mut Ui,
    name: &str,
    area: Rect,
    labels: &[String],
    selected: Option<usize>,
) -> Option<usize> {
    let mut clicked = None;
    ui.scroll_area(name, area, &mut |ui, view| {
        for (i, label) in labels.iter().enumerate() {
            let r = Rect::new(view.x, view.y + i as f32 * ROW_H, view.w - 8.0, ROW_H);
            // (rows scrolled out of the area are neither drawn nor clickable)
            if r.bottom() < area.y || r.y > area.bottom() {
                continue;
            }
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
            ui.text_in(
                label,
                Rect::new(r.x + 12.0, r.y, (r.w - 20.0).max(0.0), r.h),
                14.0 * TS,
                Weight::Regular,
                if selected == Some(i) { FG } else { MUTED },
                Align::Left,
            );
            if click && area.contains(ui.input.mouse) {
                clicked = Some(i);
            }
        }
        labels.len() as f32 * ROW_H
    });
    clicked
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
    /// Passed without being served, with a later stop served.
    Missed,
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
    let columns = [
        ("Stop", 16.0),
        ("Scheduled", 0.44),
        ("Actual", 0.58),
        ("Deviation", 0.71),
        ("Status", 0.84),
    ];
    let col_x = |c: f32| if c < 1.0 { (body.w * c).floor() } else { c };
    let head_y = body.y + 44.0;
    for (title, c) in columns {
        text(
            ui,
            title,
            body.x + col_x(c),
            head_y + 17.0,
            12.0,
            Weight::Regular,
            MUTED,
        );
    }
    let foot = 64.0;
    let totals_h = v.totals.len() as f32 * ROW_H;
    let rows_area = Rect::new(
        body.x + 1.0,
        head_y + 34.0,
        body.w - 2.0,
        (r.bottom() - foot - totals_h - 8.0 - (head_y + 34.0)).max(ROW_H),
    );
    ui.scroll_area("report:rows", rows_area, &mut |ui, view| {
        for (i, row) in v.rows.iter().enumerate() {
            let rr = Rect::new(view.x, view.y + i as f32 * ROW_H, view.w - 8.0, ROW_H);
            if rr.bottom() < rows_area.y || rr.y > rows_area.bottom() {
                continue;
            }
            ui.p().rect(Rect::new(rr.x, rr.y, rr.w, 1.0), ROW_LINE);
            let (status, color) = match row.state {
                StopState::OnTime => ("OK", Some(GOOD)),
                StopState::Late => ("late", Some(BAD)),
                StopState::LeftEarly => ("left early", Some(SOON)),
                StopState::Missed => ("missed", Some(BAD)),
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
        v.rows.len() as f32 * ROW_H
    });
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

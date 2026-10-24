//! The Alt icon bar and the in-game windows over a made-up scene, with placeholder data:
//! `cargo run -p omsi-ui --example ingame_demo -- out_dir`
#[path = "common/mod.rs"]
mod common;

use glam::Vec2;
use omsi_ui::alt_bar::{self, Button, Glyph, State, TopBar};
use omsi_ui::ingame::{buttons, key_values, table, window};
use omsi_ui::widgets::Ui;
use omsi_ui::{Color, Rect};

fn scene(ui: &mut Ui) {
    let (w, h) = (ui.size.x, ui.size.y);
    ui.p().gradient(
        Rect::new(0.0, 0.0, w, h * 0.55),
        Color::hex(0x3b4a63),
        Color::hex(0x1d2638),
    );
    ui.p().gradient(
        Rect::new(0.0, h * 0.55, w, h * 0.45),
        Color::hex(0x2a2a2a),
        Color::hex(0x181818),
    );
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(".".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut o = common::Offscreen::new((1672, 941));
    let ic = |id, icon, title| Button {
        id,
        glyph: Glyph::Icon(icon),
        title,
        disabled: None,
        on: false,
    };
    let off = |id, glyph, title, why| Button {
        id,
        glyph,
        title,
        disabled: Some(why),
        on: false,
    };
    // (the game's bar: the original's fifteen, in its order, five by three)
    let bar = [
        ic("add", "alt-add", "Add vehicle"),
        ic("swap", "alt-swap", "Switch vehicle"),
        ic("remove", "alt-remove", "Remove vehicle"),
        ic(
            "reposition",
            "alt-reposition",
            "Reposition at an entry point",
        ),
        off(
            "ai",
            Glyph::Icon("alt-ai"),
            "AI drives my vehicle",
            "Not in the game yet",
        ),
        Button {
            on: true,
            ..ic("tour", "alt-tour", "Timetable")
        },
        Button {
            glyph: Glyph::Sign("92E"),
            ..ic("destination", "", "Line and destination")
        },
        off(
            "accident",
            Glyph::Icon("alt-accident"),
            "Clear the accident",
            "Not in the game yet",
        ),
        ic("repair", "alt-repair", "Repair"),
        ic("service", "alt-fuel", "Wash and refuel"),
        off(
            "help",
            Glyph::Icon("alt-help"),
            "Help marker",
            "Not in the game yet",
        ),
        ic("time", "alt-clock", "Date and time"),
        ic("weather", "alt-weather", "Weather"),
        ic("controls", "alt-wheel", "Controls"),
        ic(
            "driver",
            "alt-driver",
            "Timetable evaluation and driver profile",
        ),
    ];
    let mut state = State {
        chosen: Some("reposition".into()),
        focus: None,
    };
    o.shot(&format!("{dir}/alt-bar.png"), |ui| {
        scene(ui);
        alt_bar::top_bar(
            ui,
            &TopBar {
                route: "Line 50 \u{b7} Example route",
                next_stop: Some("Central Station"),
                clock: "08:42",
                doors: None,
                hint: "Alt = icon bar \u{b7} F4 = select/move vehicle",
            },
        );
        let size = alt_bar::panel_size(bar.len());
        alt_bar::draw(
            ui,
            Vec2::new(ui.size.x - size.x - 16.0, ui.size.y - size.y - 20.0),
            &bar,
            &mut state,
        );
        alt_bar::hint(ui, "Alt closes the bar \u{b7} the simulation keeps running");
    });
    // folded with its arrow: only a tab is left at the corner
    o.shot(&format!("{dir}/alt-bar-folded.png"), |ui| {
        scene(ui);
        alt_bar::draw_folded(ui, Vec2::new(ui.size.x - 16.0, ui.size.y - 20.0));
    });
    o.shot(&format!("{dir}/ingame-windows.png"), |ui| {
        scene(ui);
        alt_bar::top_bar(
            ui,
            &TopBar {
                route: "Line 50 \u{b7} Example route",
                next_stop: Some("Central Station"),
                clock: "08:42",
                doors: Some(("open", true)),
                hint: "Alt = menu \u{b7} F4 = select/move vehicle",
            },
        );
        // vehicles in the session
        let a = window(
            ui,
            Rect::new(40.0, 70.0, 520.0, 243.0),
            "Session \u{b7} Vehicles",
            "add / swap / remove",
        );
        table(
            ui,
            "fleet",
            a,
            &[("Vehicle", 16.0), ("Paint", 216.0), ("Position", 336.0)],
            &[
                s(&["Example bus A", "Default", "Player"]),
                s(&["Example bus B", "Paint 2", "AI \u{b7} line 50"]),
                s(&["Example bus C", "Paint 3", "Parked \u{b7} depot"]),
            ],
            Some(0),
            &|_, _| None,
        );
        buttons(
            ui,
            "fleet",
            Vec2::new(57.0, 259.0),
            &["Add vehicle...", "Swap", "Remove"],
        );
        // end of trip report
        let r = window(
            ui,
            Rect::new(620.0, 120.0, 780.0, 348.0),
            "End of trip report",
            "sim paused",
        );
        let green = Color::hex(0x4fc27a);
        let red = Color::hex(0xe8523f);
        table(
            ui,
            "report",
            r,
            &[
                ("Stop", 16.0),
                ("Scheduled", 204.0),
                ("Actual", 338.0),
                ("Deviation", 440.0),
                ("Boarded", 563.0),
                ("Alighted", 678.0),
            ],
            &[
                s(&["Depot", "08:00", "08:00", "0:00", "0", "0"]),
                s(&["Main Street", "08:12", "08:13", "+1:05", "14", "2"]),
                s(&["Market", "08:25", "08:25", "0:00", "9", "11"]),
                s(&["Central Station", "08:42", "08:41", "-0:40", "3", "18"]),
            ],
            None,
            &|row, col| (col == 3).then_some(if row == 1 { red } else { green }),
        );
        let rows = [
            ("Total passengers", "26".to_string(), None),
            ("Punctuality", "3 of 4 stops on time".to_string(), None),
        ];
        key_values(
            ui,
            Rect::new(r.x, r.y + 32.0 + 4.0 * 34.0, r.w, 80.0),
            &rows,
        );
        buttons(
            ui,
            "report",
            Vec2::new(637.0, 415.0),
            &["Continue", "New trip", "Back to menu"],
        );
        // passengers and stop
        let p = window(
            ui,
            Rect::new(1030.0, 520.0, 370.0, 186.0),
            "Passengers & stop",
            "live",
        );
        key_values(
            ui,
            p,
            &[
                ("Onboard", "26 / 94".into(), None),
                ("Standing", "4".into(), None),
                ("Doors", "open".into(), Some(green)),
                ("Stop distance", "0.4 m".into(), None),
            ],
        );
    });
    o.shot(&format!("{dir}/add-vehicle.png"), |ui| {
        scene(ui);
        let makers = s(&["Example maker", "Another maker", "Third maker"]);
        let vehicles: Vec<String> = (1..=12).map(|i| format!("Example bus {i}")).collect();
        let liveries = s(&["Random livery", "Default", "Paint 2", "Paint 3"]);
        let depots = s(&[
            "The map's depot file",
            "Depot A  (a.hof)",
            "Depot B  (b.hof)",
        ]);
        let size = ui.size;
        omsi_ui::ingame::add_vehicle(
            ui,
            size,
            &omsi_ui::ingame::AddVehicle {
                makers: &makers,
                maker: Some(0),
                vehicles: &vehicles,
                vehicle: Some(2),
                liveries: &liveries,
                livery: 2,
                depots: &depots,
                depot: 0,
                numbers: &["4711".to_string(), "4712".to_string()],
                number: 1,
                plate: "",
                plate_focused: false,
                plate_hint: "Automatic",
            },
        );
    });
    o.shot(&format!("{dir}/trip-report.png"), |ui| {
        scene(ui);
        use omsi_ui::ingame::{ReportRow, StopState};
        let row = |stop: &str, plan: &str, real: &str, diff: &str, state| ReportRow {
            stop: stop.into(),
            scheduled: plan.into(),
            actual: real.into(),
            deviation: diff.into(),
            state,
        };
        let rows = vec![
            row(
                "Depot",
                "08:00:00",
                "08:00:20",
                "+00:00:20",
                StopState::OnTime,
            ),
            row(
                "Main Street",
                "08:12:00",
                "08:15:05",
                "+00:03:05",
                StopState::Late,
            ),
            row("Market", "08:25:00", "", "", StopState::Missed),
            row(
                "Town Hall",
                "08:31:00",
                "08:30:10",
                "-00:00:50",
                StopState::LeftEarly,
            ),
            row(
                "Central Station",
                "08:42:00",
                "08:41:20",
                "-00:00:40",
                StopState::OnTime,
            ),
        ];
        let totals = [
            ("Stops served", "4 of 5".to_string()),
            ("Late arrivals", "1".to_string()),
            ("Early departures", "1".to_string()),
        ];
        let size = ui.size;
        ui.p().rect(
            Rect::new(0.0, 0.0, size.x, size.y),
            Color::hex(0x000000).alpha(0.35),
        );
        omsi_ui::ingame::trip_report(
            ui,
            size,
            &omsi_ui::ingame::TripReport {
                title: "Trip 3  \u{b7}  50 to Central Station  \u{b7}  08:00:00",
                note: "paused",
                rows: &rows,
                totals: &totals,
            },
        );
    });
    // the Alt bar's windows, each over the scene with the bar waiting under it
    use omsi_ui::alt_windows::{self as aw, View};
    // the bar while the window of button `open` is open over it
    let waiting = |open: &str| -> Vec<Button> {
        // (the wash and refuel menu holds nothing up)
        if open == "service" {
            return bar
                .iter()
                .map(|b| Button {
                    on: b.id == open,
                    ..*b
                })
                .collect();
        }
        bar.iter()
            .map(|b| Button {
                disabled: Some(if b.id == open {
                    "Open: Esc or Cancel closes it"
                } else {
                    "Close the open window first"
                }),
                on: b.id == open,
                ..*b
            })
            .collect()
    };
    let views = [
        (
            "alt-time",
            View::Time(aw::Time {
                hour: 8,
                minute: 42,
                day: 20,
                month: 10,
                year: 2026,
                weekday: "Tuesday".into(),
                quick: vec![
                    ("Dawn".into(), "07:21".into()),
                    ("Day".into(), "12:00".into()),
                    ("Dusk".into(), "17:54".into()),
                    ("Night".into(), "23:00".into()),
                ],
                real: false,
            }),
        ),
        (
            "alt-weather",
            View::Weather(aw::Weather {
                real: false,
                real_label: "Real weather (METAR)  \u{b7}  EDDB".into(),
                presets: s(&[
                    "Clear",
                    "Cloudy",
                    "Fog",
                    "Light rain",
                    "Overcast",
                    "Rain",
                    "Snow",
                    "Storm",
                    "Sunny",
                    "Winter morning",
                ]),
                selected: Some(4),
                stations: s(&["Automatic (nearest the map)", "EDDB", "EDDF", "EDDM"]),
                station: Some(1),
            }),
        ),
        (
            "alt-reposition",
            View::Pick(aw::Pick {
                title: "Reposition at an entry point".into(),
                head: "Entry point".into(),
                rows: s(&[
                    "Depot",
                    "Central Station",
                    "Market",
                    "Town Hall",
                    "Ring road north",
                ]),
                selected: Some(1),
                ok: "Reposition".into(),
                hint: "The bus is put at the entry point".into(),
                empty: "This map has no entry points".into(),
            }),
        ),
        (
            "alt-controls",
            View::Menu(aw::Menu {
                title: "Controls".into(),
                entries: vec![
                    aw::Entry {
                        label: "Game controller".into(),
                        checked: Some(false),
                        disabled: Some("No game controller found".into()),
                    },
                    aw::Entry {
                        label: "Mouse steering".into(),
                        checked: Some(false),
                        disabled: None,
                    },
                    aw::Entry {
                        label: "Keyboard only".into(),
                        checked: Some(true),
                        disabled: None,
                    },
                    aw::Entry {
                        label: "Head tracking".into(),
                        checked: Some(true),
                        disabled: None,
                    },
                ],
                hint: "Each choice applies at once".into(),
            }),
        ),
        (
            "alt-driver",
            View::Menu(aw::Menu {
                title: "Timetable and driver".into(),
                entries: vec![
                    aw::Entry {
                        label: "Timetable evaluation".into(),
                        checked: None,
                        disabled: None,
                    },
                    aw::Entry {
                        label: "Driver profile".into(),
                        checked: None,
                        disabled: None,
                    },
                ],
                hint: "Each trip's report opens at its last stop".into(),
            }),
        ),
        (
            "alt-driver-no-tour",
            View::Menu(aw::Menu {
                title: "Timetable and driver".into(),
                entries: vec![
                    aw::Entry {
                        label: "Timetable evaluation".into(),
                        checked: None,
                        disabled: Some("No tour running".into()),
                    },
                    aw::Entry {
                        label: "Driver profile".into(),
                        checked: None,
                        disabled: None,
                    },
                ],
                hint: "Each trip's report opens at its last stop".into(),
            }),
        ),
        (
            "alt-evaluation",
            View::Evaluation(aw::Evaluation {
                title: "Trip 3  \u{b7}  50 to Central Station  \u{b7}  08:00:00".into(),
                trip: "Trip 3 / 6".into(),
                trip_steps: (true, false),
                rows: {
                    use omsi_ui::ingame::{ReportRow, StopState};
                    let row = |stop: &str, plan: &str, real: &str, diff: &str, state| ReportRow {
                        stop: stop.into(),
                        scheduled: plan.into(),
                        actual: real.into(),
                        deviation: diff.into(),
                        state,
                    };
                    vec![
                        row(
                            "Depot",
                            "08:00:00",
                            "08:00:20",
                            "+00:00:20",
                            StopState::OnTime,
                        ),
                        row(
                            "Main Street",
                            "08:12:00",
                            "08:15:05",
                            "+00:03:05",
                            StopState::Late,
                        ),
                        row(
                            "Market",
                            "08:25:00",
                            "08:26:40",
                            "+00:01:40",
                            StopState::OnTime,
                        ),
                        row("Town Hall", "08:31:00", "", "", StopState::None),
                        row("Central Station", "08:42:00", "", "", StopState::None),
                    ]
                },
                totals: vec![
                    (
                        "Deviation at the last stop reached".into(),
                        "+00:01:40".into(),
                    ),
                    ("Stops served".into(), "3 of 5".into()),
                    ("Late arrivals".into(), "1".into()),
                    ("Early departures".into(), "0".into()),
                ],
                hint: "Updated at every stop  \u{b7}  the journey log is in Journeys".into(),
            }),
        ),
        (
            "alt-service",
            View::Menu(aw::Menu {
                title: "Wash and refuel".into(),
                entries: vec![
                    aw::Entry {
                        label: "Wash".into(),
                        checked: Some(false),
                        disabled: None,
                    },
                    aw::Entry {
                        label: "Refuel".into(),
                        checked: Some(true),
                        disabled: None,
                    },
                ],
                hint: "Each runs until it is done or the bus drives off".into(),
            }),
        ),
        (
            "alt-tour",
            View::Tour(aw::Tour {
                lines: s(&[
                    "Line 50  (12 tours)",
                    "Line 51  (4 tours)",
                    "Line N5  (2 tours)",
                ]),
                line: Some(0),
                tours: s(&[
                    "Tour 1  \u{203a}  Central Station",
                    "Tour 2  \u{203a}  Depot",
                    "Tour 3  \u{203a}  Central Station",
                ]),
                tour_times: s(&["08:12", "08:27", "08:42"]),
                tour: Some(1),
                stops: s(&[
                    "Depot",
                    "Main Street",
                    "Market",
                    "Town Hall",
                    "Central Station",
                ]),
                stop_times: s(&["08:27", "08:31", "08:36", "08:40", "08:47"]),
                stop: Some(0),
                trip: "Trip 2 / 6".into(),
                trip_steps: (true, true),
            }),
        ),
        (
            "alt-running",
            View::Running(aw::Running {
                sub: "Line 50  \u{b7}  trip 08:27".into(),
                rows: {
                    use aw::{RunRow, RunStop};
                    let row = |stop: &str, detail: &str, state, skip: Option<&str>| RunRow {
                        stop: stop.into(),
                        detail: detail.into(),
                        state,
                        skip: skip.map(str::to_string),
                    };
                    vec![
                        row("Depot", "08:27  \u{b7}  served", RunStop::Done, None),
                        row("Main Street", "08:31  \u{b7}  skipped", RunStop::Passed, None),
                        row("Market", "08:36  \u{b7}  next stop", RunStop::Next, Some("Skip this stop")),
                        row("Town Hall", "08:40  \u{b7}  to be skipped", RunStop::Ahead, Some("Keep this stop")),
                        row("Central Station", "08:47", RunStop::Ahead, Some("Skip this stop")),
                    ]
                },
                hint: "Skipping a stop tells the timetable you did not stop there for a reason on the trip. In the evaluation it counts with no mark: neither missed nor late.".into(),
            }),
        ),
        (
            "alt-end-tour",
            View::Confirm(aw::Confirm {
                title: "Cancel the timetable?".into(),
                text: "The bus stops following line 50. This does not change the saved timetable.".into(),
                ok: "Yes, cancel it".into(),
                cancel: "Keep it".into(),
            }),
        ),
        (
            "alt-destination",
            View::Destination(aw::Destination {
                line: "50".into(),
                focused: true,
                codes: s(&["1", "2", "12", "50", "99"]),
                destinations: s(&[
                    "Central Station",
                    "Depot",
                    "Market",
                    "Ring road",
                    "Not in service",
                ]),
                selected: Some(0),
                empty: String::new(),
            }),
        ),
    ];
    for (name, view) in &views {
        let open = match name.trim_start_matches("alt-") {
            "end-tour" | "running" => "tour",
            "driver-no-tour" | "evaluation" => "driver",
            other => other,
        };
        let waiting = waiting(open);
        o.shot(&format!("{dir}/{name}.png"), |ui| {
            scene(ui);
            let size = alt_bar::panel_size(waiting.len());
            let mut st = State {
                chosen: Some(open.into()),
                ..State::default()
            };
            alt_bar::draw(
                ui,
                Vec2::new(ui.size.x - size.x - 20.0, ui.size.y - size.y - 20.0),
                &waiting,
                &mut st,
            );
            let sz = ui.size;
            view.draw(ui, sz, 0);
        });
    }
}

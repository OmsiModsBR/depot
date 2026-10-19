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
    };
    let bar = [
        ic("add", "add", "Add vehicle"),
        ic("swap", "sync_alt", "Swap vehicle"),
        ic("remove", "close", "Remove vehicle"),
        ic("reposition", "download", "Reposition at entry point"),
        Button {
            id: "ai",
            glyph: Glyph::Text("AI"),
            title: "AI vehicles",
            disabled: None,
        },
        ic("fleet", "grid_view", "Vehicles in session"),
        ic("list", "view_list", "Timetable"),
        Button {
            id: "warn",
            glyph: Glyph::Icon("warning"),
            title: "Warnings",
            disabled: Some("No warnings now"),
        },
        ic("tools", "construction", "Repair"),
        Button {
            id: "power",
            glyph: Glyph::Icon("bolt"),
            title: "Power",
            disabled: Some("Not available for this vehicle"),
        },
        ic("help", "help", "Help"),
        ic("clock", "schedule", "Time"),
        ic("weather", "cloud", "Weather"),
        ic("locate", "my_location", "Locate"),
        ic("menu", "menu", "Menu"),
    ];
    let mut state = State {
        chosen: Some("reposition".into()),
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
                hint: "Alt = toggle the icon bar \u{b7} the simulation keeps running",
            },
        );
        let size = alt_bar::panel_size(bar.len());
        alt_bar::draw(
            ui,
            Vec2::new(ui.size.x - size.x - 16.0, ui.size.y - size.y - 20.0),
            &bar,
            &mut state,
        );
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
}

//! The pause menu with the city map's place drawn over a made-up street grid:
//! `cargo run -p omsi-ui --example pause_demo -- out_dir`
#[path = "common/mod.rs"]
mod common;

use glam::Vec2;
use omsi_ui::pause_menu::{self, Item, Kind, Section, View};
use omsi_ui::{Color, Rect};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(".".into());
    std::fs::create_dir_all(&dir).unwrap();
    let item = |id, icon, label, key, kind, disabled| Item {
        id,
        icon,
        label,
        key,
        kind,
        disabled,
    };
    let n = Kind::Normal;
    let sections = vec![
        Section {
            title: "Session",
            items: vec![
                item("resume", "play_arrow", "Resume", "Esc", Kind::Primary, None),
                item("line", "route", "Line and tour...", "", n, None),
                item(
                    "skip",
                    "turn_slight_right",
                    "Skip the next stop",
                    "",
                    n,
                    Some("There is no next stop on this tour"),
                ),
                item("end_tour", "stop_circle", "End the tour", "", n, None),
            ],
        },
        Section {
            title: "Settings",
            items: vec![
                item("options", "settings", "Options...", "", n, None),
                item("controls", "keyboard", "Controls...", "", n, None),
                item("camera", "videocam", "Camera...", "", n, None),
                item(
                    "vehicle",
                    "directions_bus",
                    "Vehicle options...",
                    "",
                    n,
                    None,
                ),
                item("world", "cloud", "World options...", "", n, None),
                item(
                    "city_map",
                    "map",
                    "City map",
                    "Shift+M",
                    Kind::Current,
                    None,
                ),
            ],
        },
        Section {
            title: "Game",
            items: vec![
                item("save", "save", "Save the situation", "", n, None),
                item("save_new", "download", "Save to a new slot", "", n, None),
                item("quicksave", "upload", "Load the quicksave", "", n, None),
                item("shot", "photo_camera", "Screenshot", "", n, None),
                item(
                    "quit",
                    "power_settings_new",
                    "End the session",
                    "",
                    Kind::Danger,
                    None,
                ),
            ],
        },
    ];
    let view = View {
        logo: None,
        sections: &sections,
        title: "City map",
        subtitle: "Line 50 \u{b7} Example route",
        clock: "08:42",
        state: "paused",
    };
    let mut o = common::Offscreen::new((1672, 941));
    o.shot(&format!("{dir}/pause-menu.png"), |ui| {
        let out = pause_menu::draw(ui, &view);
        // (the caller's map, here a made-up grid, goes in the rectangle; drawn after, under nothing)
        let m = out.map;
        ui.p().rect(m, Color::hex(0x1b212b));
        for k in 0..12 {
            let x = m.x + 14.0 + k as f32 * (m.w / 12.0);
            ui.p()
                .rect(Rect::new(x, m.y, 8.0, m.h), Color::hex(0x2a2f3a));
            let y = m.y + 10.0 + k as f32 * (m.h / 8.0);
            ui.p()
                .rect(Rect::new(m.x, y, m.w, 8.0), Color::hex(0x2a2f3a));
        }
        let (a, b) = (
            Vec2::new(m.x + 40.0, m.bottom() - 120.0),
            Vec2::new(m.right() - 60.0, m.y + 140.0),
        );
        ui.p().line(a, b, 5.0, Color::hex(0xf0a30a));
    });
}

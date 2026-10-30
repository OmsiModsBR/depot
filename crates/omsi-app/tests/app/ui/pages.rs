use glam::Vec2;
use omsi_ui::menu_page::Back;
use omsi_ui::pause_menu;
use openomsi_game::testing;
use openomsi_game::testing::hud::lists::{row, ListKind};
use openomsi_game::testing::hud::MenuKind;
use openomsi_game::testing::hud::{DropdownView, Preview};
use openomsi_game::testing::ui::menu::{Area, MenuFrameInternals};
use openomsi_game::testing::ui::pages::*;

#[test]
fn every_kind_of_settings_row_maps_to_its_control() {
    let o = MenuKind::Options;
    let r = row_of(
        "fullscreen",
        &row("Fullscreen", 's', "on", "Window or screen", None),
        o,
    );
    assert_eq!(
        (r.label.as_str(), r.desc.as_str()),
        ("Fullscreen", "Window or screen")
    );
    assert_eq!(r.ctl, Ctl::Switch(true));
    assert_eq!(
        row_of("x", &row("A", 's', "off", "", None), o).ctl,
        Ctl::Switch(false)
    );
    assert_eq!(
        row_of("volume", &row("Volume", 'v', "60 %", "", Some(0.6)), o).ctl,
        Ctl::Slider(0.6, "60 %".into())
    );
    assert_eq!(
        row_of("x", &row("A", 'c', "Medium", "", None), o).ctl,
        Ctl::Stepper("Medium".into())
    );
    assert_eq!(
        row_of("preset", &row("Preset", 'o', "High", "", None), o).ctl,
        Ctl::Opens("High".into())
    );
    assert_eq!(
        row_of("keyboard", &row("Keyboard", 'o', "", "", None), o).ctl,
        Ctl::Opens(String::new())
    );
    assert_eq!(
        row_of("noop", &row("Connection", 'i', "Connected", "", None), o).ctl,
        Ctl::Info("Connected".into())
    );
    assert_eq!(
        row_of("time_edit", &row("Exact time", 'E', "0842", "", None), o).ctl,
        Ctl::Editing("0842".into())
    );
    assert_eq!(
        row_of(
            "time_edit",
            &row("Exact time", 'e', "08:42:00", "", None),
            o
        )
        .ctl,
        Ctl::Button("08:42:00".into())
    );
    assert_eq!(
        row_of("keybind v 0", &row("Horn", 'a', "H", "", None), o).ctl,
        Ctl::Button("H".into())
    );
    // a heading: never chosen, its label alone
    let h = row_of(testing::hud::lists::HEADING, "Steering", o);
    assert!(h.heading);
    assert_eq!(h.label, "Steering");
    assert!(!r.heading);
}

#[test]
fn list_lines_keep_their_text_and_get_their_sign_code_or_arrow() {
    let l = row_of("line 50", "Line 50  (12 tours)", MenuKind::Lines);
    assert_eq!(l.lead, Before::Sign("50".into()));
    assert_eq!(l.label, "12 tours");
    assert_eq!(l.ctl, Ctl::Opens(String::new()));
    let t = row_of("tour 50\u{1}3", "Tour 3", MenuKind::Tours);
    assert_eq!(t.label, "Tour 3");
    let d = row_of("dest 4", " 12  Central Station", MenuKind::List);
    assert_eq!(d.lead, Before::Code("12".into()));
    assert_eq!(d.label, "Central Station");
    let m = row_of("routes", "Route number: 50...", MenuKind::List);
    assert_eq!(m.label, "Route number: 50");
    assert_eq!(m.ctl, Ctl::Opens(String::new()));
    let p = row_of("driver Ana", "Ana  (now)", MenuKind::List);
    assert_eq!(
        (p.label.as_str(), &p.ctl, &p.lead),
        ("Ana  (now)", &Ctl::None, &Before::None)
    );
}

#[test]
fn a_settings_window_has_its_pages_its_rows_and_the_way_back_at_their_foot() {
    let a = row("Fullscreen", 's', "on", "", None);
    let b = row("Volume", 'v', "60 %", "", Some(0.6));
    let items = [("fullscreen", a.as_str()), ("volume", b.as_str())];
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu = Some((1, &items));
    f.menu_tabs = Some((
        vec!["General".into(), "Graphics".into(), "Camera".into()],
        2,
    ));
    let p = page_frame(&f, Some(&ListKind::Options(2)), test_column(true)).unwrap();
    assert!(p.fields);
    assert_eq!(p.rows.len(), 2);
    assert_eq!(p.selected, 1);
    assert_eq!(p.back, Back::Side);
    assert_eq!(p.title, "Camera");
    // the column marks the line the page is behind: "Camera...", which has its own
    assert_eq!(p.column.current(), Some("camera"));
    assert_eq!(p.column.bar(), ("Options", "Camera", "paused"));
    // the rectangles the HUD hit-tests: a row per line, the pages and the way back last
    let (s, l) = p.layout((1672, 941));
    assert_eq!(s, 1.0);
    assert_eq!(l.rows.len(), 2);
    assert_eq!(l.tabs.len(), 4);
    assert!(l.controls[0].is_none() && l.controls[1].is_some());
    // another page of the options: "Options..." is the line marked
    let p = page_frame(&f, Some(&ListKind::Options(0)), test_column(true));
    let mut f2 = test_frame(&items, MenuKind::Options);
    f2.menu_tabs = Some((vec!["General".into()], 0));
    let p2 = page_frame(&f2, Some(&ListKind::Options(0)), test_column(true)).unwrap();
    assert!(p.is_some());
    assert_eq!(p2.column.current(), Some("options"));
}

#[test]
fn a_list_has_the_way_back_under_it_and_its_tour_and_timetable_marked() {
    let items = [
        ("tour 50\u{1}1", "Tour 1"),
        ("tour 50\u{1}2", "Tour 2"),
        ("back", "Back"),
    ];
    let mut f = test_frame(&items, MenuKind::Tours);
    f.menu = Some((1, &items));
    f.menu_head = Some(("Line and tour".into(), "Line 50".into()));
    f.menu_preview = Some(Preview {
        title: "Tour 2".into(),
        meta: "Line 50".into(),
        rows: vec![
            ("Depot".into(), "08:00".into()),
            ("Market".into(), "08:10".into()),
        ],
        chosen: Some(1),
        button: Some("Start trip".into()),
        time: Some("08:00".into()),
    });
    let p = page_frame(
        &f,
        Some(&ListKind::Tours("50".into(), None)),
        test_column(true),
    )
    .unwrap();
    assert_eq!(p.back, Back::Footer);
    assert_eq!(p.active, Some(1));
    assert_eq!(p.note, "Line 50");
    assert_eq!(p.column.current(), Some("duty"));
    let mut ui = testing::hud::Ui::new().unwrap();
    p.write_hits(&mut ui.hits, Area::whole((1672, 941)));
    // the way back at its row's index, the stops, their button and the arrows of the time
    assert_eq!(ui.hits.menu_rects.len(), 3);
    assert_eq!(ui.hits.menu_pane.len(), 2);
    assert!(ui.hits.menu_pane_go.is_some() && ui.hits.menu_pane_box.is_some());
    assert_eq!(ui.hits.menu_time.len(), 2);
    assert!(ui.hits.menu_side.is_empty());
    // the way back chosen is not a tour marked
    f.menu = Some((2, &items));
    let p = page_frame(
        &f,
        Some(&ListKind::Tours("50".into(), None)),
        test_column(true),
    )
    .unwrap();
    assert_eq!(p.active, None);
}

#[test]
fn the_hud_gets_the_rectangles_in_window_pixels() {
    let a = row("Preset", 'o', "High", "", None);
    let items = [("preset", a.as_str())];
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu_tabs = Some((vec!["Graphics".into()], 0));
    let names = ["Low", "Medium", "High"];
    f.dropdown = Some(DropdownView {
        row: 0,
        items: names.to_vec(),
        sel: 2,
        top: 0,
        current: Some(2),
    });
    let p = page_frame(&f, Some(&ListKind::Options(0)), test_column(true)).unwrap();
    for size in [(1672u32, 941u32), (1920, 1080), (3840, 2160)] {
        let mut ui = testing::hud::Ui::new().unwrap();
        p.write_hits(&mut ui.hits, Area::whole(size));
        let (s, l) = p.layout(size);
        assert_eq!(
            ui.hits.menu_rects[0],
            px(l.rows[0], s, Vec2::ZERO),
            "{size:?}"
        );
        assert_eq!(ui.hits.dd_rects.len(), 3, "{size:?}");
        assert_eq!(ui.hits.dd_rows, 3);
        // (all inside the window, right of the column)
        for r in ui
            .hits
            .menu_rects
            .iter()
            .chain(&ui.hits.menu_side)
            .chain(&ui.hits.dd_rects)
        {
            assert!(
                r[0] >= pause_menu::COLUMN_W * s && r[2] <= size.0 as f32 && r[3] <= size.1 as f32,
                "{size:?}: {r:?}"
            );
        }
    }
}

#[test]
fn on_a_triple_screen_the_page_is_clicked_where_the_centre_screen_shows_it() {
    let items = [("a", "One"), ("b", "Two"), ("back", "Back")];
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu_tabs = Some((vec!["General".into(), "Graphics".into()], 0));
    let names = ["Low", "Medium", "High"];
    f.dropdown = Some(DropdownView {
        row: 0,
        items: names.to_vec(),
        sel: 1,
        top: 0,
        current: Some(1),
    });
    let p = page_frame(&f, Some(&ListKind::Options(0)), test_column(true)).unwrap();
    // the page of one 1920x1080 screen, and the same on the centre one of three
    let mut one = testing::hud::Ui::new().unwrap();
    p.write_hits(&mut one.hits, Area::whole((1920, 1080)));
    let mut three = testing::hud::Ui::new().unwrap();
    let area = testing::ui::menu::menu_area(true, false, (5760, 1080));
    p.write_hits(&mut three.hits, area);
    let moved = |r: &[f32; 4]| [r[0] + 1920.0, r[1], r[2] + 1920.0, r[3]];
    for (a, b) in [
        (&one.hits.menu_rects, &three.hits.menu_rects),
        (&one.hits.menu_side, &three.hits.menu_side),
        (&one.hits.dd_rects, &three.hits.dd_rects),
    ] {
        assert!(!a.is_empty());
        let a: Vec<_> = a.iter().map(moved).collect();
        assert_eq!(&a, b);
        // (inside the centre screen, right of its column)
        for r in b {
            assert!(
                r[0] >= 1920.0 + pause_menu::COLUMN_W && r[2] <= 3840.0,
                "{r:?}"
            );
        }
    }
    // in the headset the window's picture is the menu plane's: the window's rectangles
    let mut vr = testing::hud::Ui::new().unwrap();
    p.write_hits(
        &mut vr.hits,
        testing::ui::menu::menu_area(true, true, (1920, 1080)),
    );
    assert_eq!(vr.hits.menu_rects, one.hits.menu_rects);
}

#[test]
fn opening_a_page_keeps_the_game_paused_and_closing_it_returns_to_the_top_level() {
    // the Esc menu open: its top level
    assert_eq!(showing(true, false), Showing::Top);
    // a line opens a list: the page, the column still beside it saying the game is paused
    assert_eq!(showing(true, true), Showing::Page);
    let items = [("driver Ana", "Ana"), ("back", "Back")];
    let mut f = test_frame(&items, MenuKind::List);
    f.menu_head = Some(("Driver".into(), String::new()));
    let p = page_frame(&f, Some(&ListKind::Drivers), test_column(true)).unwrap();
    assert_eq!(p.column.bar().2, "paused");
    assert_eq!(p.column.current(), Some("vehicle"));
    // closed (Esc, "Back"): the top level again; the menu closed: nothing
    assert_eq!(showing(true, false), Showing::Top);
    assert_eq!(showing(false, false), Showing::None);
    // a list a plugin opened with no Esc menu: the HUD's, whose clicks it has
    assert_eq!(showing(false, true), Showing::None);
    // no list open in the HUD's frame: no page
    let mut f = test_frame(&items, MenuKind::List);
    f.menu = None;
    assert!(page_frame(&f, None, test_column(true)).is_none());
}

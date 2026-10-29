use omsi_sim::SimClock;
use openomsi_game::testing;
use openomsi_game::testing::ui::windows::*;
use winit::keyboard::KeyCode;

fn clock(y: i32, m: i32, d: i32, secs: f64) -> SimClock {
    let mut c = SimClock::default();
    c.set_date(y, m, d);
    c.time = secs;
    c
}

#[test]
fn the_time_form_goes_round_and_keeps_the_day_in_its_month() {
    let mut f = TimeForm::new(&clock(2024, 1, 31, 23.0 * 3600.0 + 59.0 * 60.0), false);
    assert_eq!(
        (f.hour, f.minute, f.day, f.month, f.year),
        (23, 59, 31, 1, 2024)
    );
    f.step(0, 1);
    f.step(1, 1);
    assert_eq!((f.hour, f.minute), (0, 0));
    f.step(1, -1);
    assert_eq!(f.minute, 59);
    // the 31st of January a month on is the 29th of February (2024 is a leap year)
    f.step(3, 1);
    assert_eq!((f.day, f.month), (29, 2));
    f.step(2, 1);
    assert_eq!(f.day, 1);
    f.step(3, -2);
    assert_eq!(f.month, 12);
    f.step(4, 500);
    assert_eq!(f.year, YEARS.1);
    // the clock it sets
    let mut f = TimeForm::new(&clock(2026, 10, 20, 8.0 * 3600.0), false);
    f.quick = vec![("Dawn", 6 * 60 + 12), ("Night", 23 * 60)];
    f.choose_quick(1);
    let c = f.clock(&SimClock::default());
    assert_eq!(
        (c.year, c.day_month(), c.time),
        (2026, (20, 10), 23.0 * 3600.0)
    );
    assert_eq!(f.weekday(), "Tuesday");
    // a choice that is not there changes nothing
    f.choose_quick(7);
    assert_eq!(f.hour, 23);
    assert_eq!(f.view().quick[0].1, "06:12");
}

#[test]
fn the_destination_form_changes_only_what_was_changed() {
    let mut d = DestForm {
        line: "50".into(),
        line_before: "50".into(),
        destinations: vec![
            ("1".into(), "Central Station".into(), 4),
            ("2".into(), "Depot".into(), 7),
        ],
        selected: Some(0),
        selected_before: Some(0),
        ..Default::default()
    };
    assert_eq!(d.changes(), (None, None));
    d.selected = Some(1);
    assert_eq!(d.changes(), (None, Some(7)));
    d.line.clear();
    d.type_text("-10\r");
    assert_eq!(d.line, "-10");
    assert_eq!(d.changes(), (Some("-10".into()), Some(7)));
    // an emptied field leaves the route number as it is
    d.line.clear();
    assert_eq!(d.changes().0, None);
    d.type_text(&"x".repeat(ROUTE_MAX + 10));
    assert_eq!(d.line.chars().count(), ROUTE_MAX);
}

#[test]
fn a_window_opens_over_the_last_one_and_esc_closes_it() {
    let mut t = Tools::default();
    t.open(ToolWindow::Controls);
    assert_eq!(t.opened, 1);
    assert_eq!(t.window.as_ref().map(|w| w.button()), Some("controls"));
    t.open(ToolWindow::EndTour);
    assert_eq!(t.opened, 2);
    assert_eq!(t.window.as_ref().map(|w| w.button()), Some("tour"));
    // a driving key is the game's; Esc closes the window
    assert_eq!(t.key(KeyCode::KeyW, true), KeyAnswer::Game);
    assert_eq!(t.key(KeyCode::KeyW, false), KeyAnswer::Game);
    assert_eq!(t.key(KeyCode::Escape, true), KeyAnswer::Close);
    // (its release is the window's too, not the game's)
    assert_eq!(t.key(KeyCode::Escape, false), KeyAnswer::Taken);
    t.close();
    assert!(t.window.is_none());
}

#[test]
fn the_keys_are_the_fields_only_while_it_has_the_keyboard() {
    let mut t = Tools::default();
    t.open(ToolWindow::Destination(DestForm {
        line: "50".into(),
        ..Default::default()
    }));
    assert_eq!(t.key(KeyCode::KeyW, true), KeyAnswer::Game);
    assert_eq!(t.key(KeyCode::KeyW, false), KeyAnswer::Game);
    if let Some(ToolWindow::Destination(d)) = t.window.as_mut() {
        d.focused = true;
    }
    assert!(t.window.as_ref().is_some_and(ToolWindow::typing));
    // (a press that brought no text types what its key stands for)
    assert_eq!(t.key(KeyCode::KeyW, true), KeyAnswer::Taken);
    assert_eq!(t.key(KeyCode::Minus, true), KeyAnswer::Taken);
    if let Some(ToolWindow::Destination(d)) = t.window.as_ref() {
        assert_eq!(d.line, "50W-");
    }
    for _ in 0..3 {
        assert_eq!(t.key(KeyCode::Backspace, true), KeyAnswer::Taken);
    }
    // (the modifiers stay the game's: Alt still shows and hides the bar)
    assert_eq!(t.key(KeyCode::ShiftLeft, true), KeyAnswer::Game);
    assert_eq!(t.key(KeyCode::Enter, true), KeyAnswer::Ok);
    assert_eq!(t.key(KeyCode::KeyW, false), KeyAnswer::Taken);
    if let Some(ToolWindow::Destination(d)) = t.window.as_ref() {
        assert_eq!(d.line, "5");
    }
    // the window lost the keys: what it held is forgotten and the field lets go
    t.forget_input();
    assert!(!t.window.as_ref().is_some_and(ToolWindow::typing));
    assert_eq!(t.key(KeyCode::Backspace, false), KeyAnswer::Game);
}

#[test]
fn the_tour_form_offers_its_trips_in_their_bounds() {
    let f = TourForm {
        lines: vec![("Line 1".into(), "1".into())],
        line: Some(0),
        tours: vec![("Tour 1".into(), "08:00".into(), "1".into())],
        tour: Some(0),
        trip: 0,
        trips: 3,
        stops: vec![("Depot".into(), "08:00".into())],
        stop: Some(0),
    };
    let v = f.view();
    assert_eq!(v.trip_steps, (false, true));
    assert!(v.trip.ends_with("1 / 3"));
    assert_eq!(f.chosen(), Some(("1".into(), "1".into())));
    let last = TourForm {
        trip: 2,
        ..f.clone()
    };
    assert_eq!(last.view().trip_steps, (true, false));
    let none = TourForm { tour: None, ..f };
    assert_eq!(none.chosen(), None);
}

#[test]
fn the_bars_windows_never_touch_the_games_pause() {
    // (only the Esc menu and the end-of-trip report pause the game: nothing of the bar,
    // its windows or the "Add vehicle" window writes the pause)
    for (name, src) in [
        (
            "ui/domain/alt.rs",
            include_str!("../../../src/ui/domain/alt.rs"),
        ),
        (
            "ui/implementation/alt.rs",
            include_str!("../../../src/ui/implementation/alt.rs"),
        ),
        (
            "ui/domain/windows.rs",
            include_str!("../../../src/ui/domain/windows.rs"),
        ),
        (
            "ui/implementation/windows.rs",
            include_str!("../../../src/ui/implementation/windows.rs"),
        ),
        ("new_ui_add.rs", include_str!("../../../src/new_ui_add.rs")),
    ] {
        for bad in ["paused =", "toggle_pause", "open_game_menu"] {
            assert!(!src.contains(bad), "{name} has {bad:?}");
        }
    }
}

#[test]
fn every_window_says_which_button_opened_it() {
    let ids = [
        (
            ToolWindow::Reposition {
                names: vec![],
                selected: None,
            },
            "reposition",
        ),
        (
            ToolWindow::Time(TimeForm::new(&SimClock::default(), false)),
            "time",
        ),
        (
            ToolWindow::Weather {
                files: vec![],
                selected: None,
                real: false,
                real_label: String::new(),
                stations: vec![],
                station: None,
                station_before: None,
            },
            "weather",
        ),
        (ToolWindow::Controls, "controls"),
        (ToolWindow::Tour(TourForm::default()), "tour"),
        (ToolWindow::EndTour, "tour"),
        (ToolWindow::Destination(DestForm::default()), "destination"),
        (ToolWindow::Service, "service"),
        (ToolWindow::DriverMenu, "driver"),
        (ToolWindow::Profile, "driver"),
        (ToolWindow::Evaluation { trip: 0 }, "driver"),
        (
            ToolWindow::Driver {
                names: vec![],
                selected: None,
            },
            "driver",
        ),
    ];
    let bar: Vec<&str> = testing::ui::alt::buttons(&Default::default())
        .iter()
        .map(|b| b.id)
        .collect();
    for (w, id) in ids {
        assert_eq!(w.button(), id);
        assert!(bar.contains(&id), "{id} is not on the bar");
    }
}

use glam::Vec2;
use omsi_ui::alt_bar;
use omsi_ui::alt_bar::{Hit, COLUMNS};
use openomsi_game::testing::ui::alt::*;
use openomsi_game::testing::ui::menu::scale_for;
use winit::keyboard::KeyCode;

fn all_on() -> AltCtx {
    AltCtx {
        player: true,
        other_vehicle: true,
        at_station: true,
        placing: false,
        lan_client: false,
        server: false,
        timetable: true,
        on_tour: false,
        entry_points: true,
        controls: "alt-wheel",
        pumping: None,
        route_arrows: false,
    }
}

fn by_id<'a>(bs: &'a [OwnedButton], id: &str) -> &'a OwnedButton {
    bs.iter().find(|b| b.id == id).unwrap()
}

#[test]
fn every_button_has_a_unique_id_a_real_icon_and_a_reason_when_dim() {
    let bs = buttons(&AltCtx::default());
    let mut ids: Vec<_> = bs.iter().map(|b| b.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), bs.len());
    for b in &bs {
        if let OwnedGlyph::Icon(i) = b.glyph {
            assert!(omsi_ui::icons::svg(i).is_some(), "no icon {i}");
        }
        assert!(!b.title.is_empty());
        if let Some(why) = &b.disabled {
            assert!(!why.is_empty());
        }
    }
    // the original's fifteen, in its order: five by three
    let ids: Vec<&str> = bs.iter().map(|b| b.id).collect();
    assert_eq!(
        ids,
        vec![
            "add",
            "swap",
            "remove",
            "reposition",
            "ai",
            "tour",
            "destination",
            "accident",
            "repair",
            "service",
            "help",
            "time",
            "weather",
            "controls",
            "driver",
        ]
    );
    assert_eq!(bs.len(), COLUMNS * 3);
}

#[test]
fn what_the_game_can_do_is_enabled_and_the_rest_says_why() {
    let enabled_ids = |ctx: &AltCtx| -> Vec<&'static str> {
        buttons(ctx)
            .into_iter()
            .filter(|b| b.disabled.is_none())
            .map(|b| b.id)
            .collect()
    };
    assert_eq!(
        enabled_ids(&AltCtx::default()),
        vec!["add", "help", "time", "weather", "controls", "driver"]
    );
    assert_eq!(
        enabled_ids(&all_on()),
        vec![
            "add",
            "swap",
            "remove",
            "reposition",
            "tour",
            "destination",
            "repair",
            "service",
            "help",
            "time",
            "weather",
            "controls",
            "driver",
        ]
    );
    // one vehicle on the mouse at a time
    let placing = AltCtx {
        placing: true,
        ..all_on()
    };
    assert!(!enabled_ids(&placing).contains(&"add"));
    // a player of a LAN session: the host sets the clock and the weather and moves the
    // vehicles on the map
    let client = AltCtx {
        lan_client: true,
        ..all_on()
    };
    for id in ["reposition", "time", "weather"] {
        assert!(!enabled_ids(&client).contains(&id), "{id}");
    }
    // no entry points, no timetable, on a server: each says why
    let bs = buttons(&AltCtx {
        entry_points: false,
        timetable: false,
        ..all_on()
    });
    assert_eq!(
        by_id(&bs, "reposition").disabled.as_deref(),
        Some("This map has no entry points")
    );
    assert_eq!(
        by_id(&bs, "tour").disabled.as_deref(),
        Some("No timetable on this map")
    );
    let bs = buttons(&AltCtx {
        server: true,
        ..all_on()
    });
    assert_eq!(
        by_id(&bs, "tour").disabled.as_deref(),
        Some("Not on a server")
    );
    // the help marker switches the route arrows: always usable, drawn pressed while they are on
    assert_eq!(by_id(&bs, "help").disabled, None);
    assert!(!by_id(&bs, "help").on);
    let bs_on = buttons(&AltCtx {
        route_arrows: true,
        ..all_on()
    });
    assert!(by_id(&bs_on, "help").on);
    // what is not in the game yet stays dim whatever the state
    let bs = buttons(&all_on());
    for id in ["ai", "accident"] {
        assert_eq!(by_id(&bs, id).disabled.as_deref(), Some(NOT_YET), "{id}");
    }
}

#[test]
fn nothing_on_the_bar_sends_the_player_to_the_esc_menu() {
    let ctxs = [
        AltCtx::default(),
        all_on(),
        AltCtx {
            lan_client: true,
            server: true,
            on_tour: true,
            pumping: Some("wash"),
            ..all_on()
        },
    ];
    for ctx in ctxs {
        let mut bs = buttons(&ctx);
        wait_for(&mut bs, "weather");
        for b in buttons(&ctx).iter().chain(&bs) {
            for text in [Some(&b.title), b.disabled.as_ref()].into_iter().flatten() {
                assert!(!text.contains("Esc menu"), "{}: {text}", b.id);
                assert!(!text.contains("pauses"), "{}: {text}", b.id);
            }
        }
    }
}

#[test]
fn a_running_tour_shows_pressed_and_its_button_ends_it() {
    let tour = |ctx: &AltCtx| by_id(&buttons(ctx), "tour").clone();
    let idle = tour(&all_on());
    assert!(!idle.on);
    assert_eq!(idle.title, "Line and tour");
    let on = AltCtx {
        on_tour: true,
        ..all_on()
    };
    let running = tour(&on);
    assert!(running.on && running.disabled.is_none());
    assert_eq!(running.title, "Timetable");
    // (a tour running can be ended whatever else holds)
    let anyway = tour(&AltCtx {
        timetable: false,
        server: true,
        ..on
    });
    assert!(anyway.disabled.is_none());
    // the wash and refuel button shows pressed while the pump or the wash runs
    let bs = buttons(&AltCtx {
        pumping: Some("refuel"),
        ..all_on()
    });
    assert!(by_id(&bs, "service").on);
    assert!(!by_id(&buttons(&all_on()), "service").on);
    // the controls button shows what drives the vehicle
    let pad = buttons(&AltCtx {
        controls: "o-gamepad",
        ..all_on()
    });
    assert_eq!(by_id(&pad, "controls").glyph, OwnedGlyph::Icon("o-gamepad"));
}

#[test]
fn wash_and_refuel_works_at_a_petrol_station_only_and_holds_nothing_up() {
    let service = |ctx: &AltCtx| by_id(&buttons(ctx), "service").clone();
    assert!(service(&all_on()).disabled.is_none());
    let away = AltCtx {
        at_station: false,
        ..all_on()
    };
    assert_eq!(service(&away).disabled.as_deref(), Some(NO_STATION));
    // (running, it can be stopped wherever the bus is: driving off stops it anyway)
    let running = AltCtx {
        pumping: Some("wash"),
        ..away
    };
    assert!(service(&running).disabled.is_none());
    assert_eq!(
        service(&AltCtx::default()).disabled.as_deref(),
        Some("No vehicle to service")
    );
    // its menu open: the button shows pressed and the others stay usable
    let mut bs = buttons(&all_on());
    wait_for(&mut bs, FREE);
    assert!(by_id(&bs, FREE).on);
    assert_eq!(bs, {
        let mut plain = buttons(&all_on());
        plain.iter_mut().find(|b| b.id == FREE).unwrap().on = true;
        plain
    });
    assert_eq!(step_focus(&bs, None, Nav::Right), Some(0));
}

#[test]
fn while_a_window_is_open_the_other_buttons_wait() {
    let mut bs = buttons(&all_on());
    wait_for(&mut bs, "time");
    for b in &bs {
        assert!(b.disabled.is_some(), "{} should wait", b.id);
        assert_eq!(b.on, b.id == "time", "{}", b.id);
    }
    assert_eq!(by_id(&bs, "add").disabled.as_deref(), Some(WAIT));
    // a click on the bar finds the button but does nothing with it (see `alt_click`)
    let frame = AltFrame {
        buttons: bs,
        ..Default::default()
    };
    let size = (1672, 941);
    let at = alt_bar::cell_rect(origin(Vec2::new(1672.0, 941.0), frame.buttons.len()), 0).center();
    let hit = frame.hit(at, size).and_then(|i| frame.buttons.get(i));
    assert!(hit.is_some_and(|b| b.disabled.is_some()));
}

#[test]
fn a_tap_of_alt_shows_the_bar_and_a_second_hides_it_but_alt_with_a_key_does_not() {
    let mut s = AltState::default();
    s.alt_down();
    assert!(s.alt_up(true));
    assert!(s.open);
    // Alt+S (the quicksave) is not a tap
    s.alt_down();
    s.other_down();
    assert!(!s.alt_up(true));
    assert!(s.open);
    // nor is Alt with a click
    s.alt_down();
    s.other_down();
    assert!(!s.alt_up(true));
    s.focus = Some(3);
    s.alt_down();
    assert!(s.alt_up(true));
    assert!(!s.open);
    assert_eq!(s.focus, None);
    // an Alt release with no press seen (the window had lost the focus) does nothing
    assert!(!s.alt_up(true));
    assert!(!s.open);
    // a bar folded and hidden comes back unfolded
    s.alt_down();
    assert!(s.alt_up(true));
    s.folded = true;
    s.alt_down();
    assert!(s.alt_up(true));
    s.alt_down();
    assert!(s.alt_up(true));
    assert!(s.open && !s.folded);
}

#[test]
fn a_tap_with_the_esc_menu_open_ends_without_a_toggle_and_lost_input_clears_everything() {
    let mut s = AltState::default();
    // Alt down and up while the Esc menu is open: nothing is left pending
    s.alt_down();
    assert!(!s.alt_up(false));
    assert!(!s.open);
    // (the menu closed meanwhile: a release without a new press is not a tap)
    assert!(!s.alt_up(true));
    assert!(!s.open);
    // Alt+Tab: the release of Alt never comes, and what was pending must not toggle later
    s.open = true;
    s.alt_down();
    s.swallowed = true;
    s.focus = Some(2);
    s.taken.insert(KeyCode::ArrowDown);
    s.reset_input();
    assert!(!s.alt_up(true));
    assert!(s.open, "the bar stays as it was");
    assert!(!s.swallowed && s.focus.is_none() && s.taken.is_empty());
}

#[test]
fn the_keyboard_walks_the_usable_buttons_only() {
    let bs = buttons(&all_on());
    let id = |i: Option<usize>| bs[i.unwrap()].id;
    let at = |id: &str| bs.iter().position(|b| b.id == id);
    let first = step_focus(&bs, None, Nav::Right);
    assert_eq!(id(first), "add");
    let next = step_focus(&bs, first, Nav::Right);
    assert_eq!(id(next), "swap");
    // Left from the first goes round to the last
    assert_eq!(id(step_focus(&bs, first, Nav::Left)), "driver");
    // Down goes along the column to the next usable one, round the bar if need be
    assert_eq!(id(step_focus(&bs, at("service"), Nav::Down)), "driver");
    assert_eq!(id(step_focus(&bs, at("driver"), Nav::Up)), "service");
    assert_eq!(id(step_focus(&bs, at("repair"), Nav::Down)), "controls");
    assert_eq!(id(step_focus(&bs, at("controls"), Nav::Down)), "reposition");
    assert_eq!(id(step_focus(&bs, first, Nav::Up)), "help");
    // a few usable: Right goes round them, Down stays where its column has none
    let few = buttons(&AltCtx {
        lan_client: true,
        ..Default::default()
    });
    let at = |id: &str| few.iter().position(|b| b.id == id);
    assert_eq!(step_focus(&few, at("driver"), Nav::Right), at("add"));
    assert_eq!(step_focus(&few, at("add"), Nav::Right), at("help"));
    assert_eq!(step_focus(&few, at("controls"), Nav::Down), at("controls"));
    assert_eq!(step_focus(&few, None, Nav::Right), at("add"));
    // a window open: nothing on the bar takes the keyboard, and Enter on the button the
    // keyboard was left on does not open its window again
    let mut waiting = buttons(&all_on());
    assert_eq!(pressable(&waiting, at("time")), Some("time"));
    wait_for(&mut waiting, "time");
    assert_eq!(step_focus(&waiting, None, Nav::Right), None);
    assert_eq!(pressable(&waiting, at("time")), None);
    assert_eq!(pressable(&waiting, None), None);
}

#[test]
fn clicks_find_the_buttons_at_any_window_size() {
    let mut frame = AltFrame {
        buttons: buttons(&all_on()),
        ..Default::default()
    };
    for size in [(1280u32, 720u32), (1672, 941), (3840, 2160), (960, 540)] {
        let scale = scale_for(size.0, size.1);
        let pts = Vec2::new(size.0 as f32, size.1 as f32) / scale;
        let at = origin(pts, frame.buttons.len());
        for i in 0..frame.buttons.len() {
            // the centre of cell i, in window pixels
            let p = alt_bar::cell_rect(at, i).center();
            assert_eq!(frame.hit(p * scale, size), Some(i), "{size:?} button {i}");
            assert!(frame.over(p * scale, size));
        }
        assert!(!frame.over(Vec2::new(2.0, 2.0), size));
        // the panel sits inside the window
        let r = alt_bar::panel_rect(at, frame.buttons.len());
        assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= pts.x && r.y + r.h <= pts.y);
        // the arrow folds it; folded, only the tab in the corner is the bar's
        let arrow = alt_bar::arrow_rect(at, frame.buttons.len()).center();
        assert_eq!(
            frame.hit_any(arrow * scale, size),
            Some(Hit::Arrow),
            "{size:?}"
        );
        frame.folded = true;
        let mid = alt_bar::cell_rect(at, 7).center();
        assert!(!frame.over(mid * scale, size));
        let tab = alt_bar::tab_rect(corner(pts)).center();
        assert_eq!(frame.hit_any(tab * scale, size), Some(Hit::Arrow));
        frame.folded = false;
    }
}

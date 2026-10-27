/// F1 given to a door and the driver's view moved to 1 (#701): F1 is not the view any more.
#[test]
fn a_built_in_view_key_steps_aside_for_the_players_own() {
    use omsi_content::KeyBinding;
    let kb = |a: &str, k: i32| KeyBinding {
        action: a.into(),
        scan_code: k,
        modifier: 0,
    };
    let none = std::collections::HashSet::new();
    assert!(openomsi_game::testing::input::keyboard::key_left_free(
        Some(59),
        "view_set_driver",
        &none,
        &[]
    ));
    let own: std::collections::HashSet<i32> = [59].into();
    assert!(!openomsi_game::testing::input::keyboard::key_left_free(
        Some(59),
        "view_set_driver",
        &own,
        &[]
    ));
    assert!(!openomsi_game::testing::input::keyboard::key_left_free(
        Some(59),
        "view_set_driver",
        &none,
        &[kb("view_set_driver", 2)]
    ));
    assert!(openomsi_game::testing::input::keyboard::key_left_free(
        Some(59),
        "view_set_driver",
        &none,
        &[kb("view_set_passenger", 60)]
    ));
}

#[test]
fn a_route_number_takes_digits_letters_and_symbols() {
    use winit::keyboard::KeyCode;
    assert_eq!(
        openomsi_game::testing::input::keyboard::route_char(KeyCode::Digit5),
        Some('5')
    );
    assert_eq!(
        openomsi_game::testing::input::keyboard::route_char(KeyCode::Numpad0),
        Some('0')
    );
    assert_eq!(
        openomsi_game::testing::input::keyboard::route_char(KeyCode::KeyE),
        Some('E')
    );
    assert_eq!(
        openomsi_game::testing::input::keyboard::route_char(KeyCode::Minus),
        Some('-')
    );
    assert_eq!(
        openomsi_game::testing::input::keyboard::route_char(KeyCode::NumpadSubtract),
        Some('-')
    );
    assert_eq!(
        openomsi_game::testing::input::keyboard::route_char(KeyCode::NumpadAdd),
        Some('+')
    );
    assert_eq!(
        openomsi_game::testing::input::keyboard::route_char(KeyCode::Slash),
        Some('/')
    );
    assert_eq!(
        openomsi_game::testing::input::keyboard::route_char(KeyCode::Space),
        None
    );
}

#[test]
fn a_cursor_way_of_78_75_px_turns_by_the_field_of_view() {
    assert!(
        (78.75 * openomsi_game::testing::input::mouse::look_deg_per_px(60.0) - 60.0).abs() < 1e-4
    );
}

#[test]
fn chase_orbits_at_035_deg_px_with_stops_above_and_below() {
    // 100 px drag down-right: +35 yaw, -35 pitch.
    let (y, p) = openomsi_game::testing::input::mouse::chase_orbit_step(0.0, 0.0, 100.0, 100.0);
    assert!(
        (y - 35.0).abs() < 1e-4 && (p + 35.0).abs() < 1e-4,
        "{y} {p}"
    );
    // yaw wraps the full circle.
    assert!(
        (openomsi_game::testing::input::mouse::chase_orbit_step(350.0, 0.0, 100.0, 0.0).0 - 25.0)
            .abs()
            < 1e-3
    );
    // pitch never leaves the stops, whichever way it is dragged.
    assert_eq!(
        openomsi_game::testing::input::mouse::chase_orbit_step(0.0, 0.0, 0.0, -1000.0).1,
        25.0
    );
    assert_eq!(
        openomsi_game::testing::input::mouse::chase_orbit_step(0.0, 0.0, 0.0, 1000.0).1,
        -60.0
    );
}

#[test]
fn space_return_eases_home_like_the_viewpoint_switch() {
    // start: untouched; partway: well on the way (ease-out); end: exact and done.
    let (look, zoom, done) =
        openomsi_game::testing::input::mouse::reset_blend((30.0, -10.0), 0.5, 0.0);
    assert_eq!((look, zoom, done), ((30.0, -10.0), 0.5, false));
    let (look, zoom, done) =
        openomsi_game::testing::input::mouse::reset_blend((30.0, -10.0), 0.5, 0.27);
    assert!(look.0 > 3.0 && look.0 < 27.0 && zoom > 0.5 && zoom < 1.0 && !done);
    let (look, zoom, done) =
        openomsi_game::testing::input::mouse::reset_blend((30.0, -10.0), 0.5, 0.54);
    assert_eq!((look, zoom, done), ((0.0, 0.0), 1.0, true));
    assert!(openomsi_game::testing::input::mouse::reset_blend((30.0, -10.0), 0.5, 5.0).2);
}

#[test]
fn precision_zoom_follows_the_fov_curve_and_never_widens() {
    // a full 364 px drag down takes z 0 to 0.70: m = 1/(1+5.5*0.70).
    let m = openomsi_game::testing::input::mouse::precision_zoom_step(
        1.0,
        364.0,
        openomsi_game::testing::input::mouse::ZOOM_INTENT,
    );
    assert!((m - 1.0 / (1.0 + 5.5 * 0.70)).abs() < 1e-4, "{m}");
    // drag down zooms in, drag up undoes it, never past 1.0.
    let mid = openomsi_game::testing::input::mouse::precision_zoom_step(
        1.0,
        100.0,
        openomsi_game::testing::input::mouse::ZOOM_INTENT,
    );
    assert!(mid < 1.0 && mid > 0.45, "{mid}");
    assert!(
        (openomsi_game::testing::input::mouse::precision_zoom_step(
            mid,
            -100.0,
            openomsi_game::testing::input::mouse::ZOOM_INTENT
        ) - 1.0)
            .abs()
            < 1e-4
    );
    assert_eq!(
        openomsi_game::testing::input::mouse::precision_zoom_step(
            1.0,
            -50.0,
            openomsi_game::testing::input::mouse::ZOOM_INTENT
        ),
        1.0
    );
    // F1 runs the same curve 20% slower.
    let slow = openomsi_game::testing::input::mouse::precision_zoom_step(
        1.0,
        100.0,
        openomsi_game::testing::input::mouse::ZOOM_INTENT_F1,
    );
    assert!(slow > mid && slow < 1.0, "{slow} vs {mid}");
}

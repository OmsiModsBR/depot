use openomsi_game::testing::input::mouse::ease_look;

/// A cab's yaw (-180..180) eased across the back stays in its range: from 170 to -170 it
/// goes through 180, not out to 190 or round to 350.
#[test]
fn a_cab_yaw_stays_within_half_a_turn() {
    let mut shown = (170.0, 0.0);
    for _ in 0..30 {
        let (yaw, _) = ease_look(&mut shown, (-170.0, 0.0), 1.0 / 60.0, 100.0);
        assert!((-180.0..=180.0).contains(&yaw), "{yaw}");
        assert!(yaw >= 170.0 || yaw <= -170.0, "the short way round: {yaw}");
    }
}

/// The step is `1 - e^(-dt/tau)`: a machine drawing 600 frames a second glides exactly
/// as far in a second as one drawing 60 (a plain fraction of the way would smooth the
/// faster of the two harder, and the same setting would feel different on a faster PC).
#[test]
fn the_glide_does_not_depend_on_the_frame_rate() {
    let (from, to) = ((0.0, 0.0), (30.0, 10.0));
    let mut slow = from;
    for _ in 0..60 {
        ease_look(&mut slow, to, 1.0 / 60.0, 100.0);
    }
    let mut fast = from;
    for _ in 0..600 {
        ease_look(&mut fast, to, 1.0 / 600.0, 100.0);
    }
    assert!(
        (slow.0 - fast.0).abs() < 0.01 && (slow.1 - fast.1).abs() < 0.01,
        "{slow:?} against {fast:?}"
    );
    // a whole second at a time constant of 100 ms is very nearly there
    assert!(slow.0 > 29.9 && slow.1 > 9.9, "{slow:?}");
}

/// A head turned a degree past north eases a degree forward, not 359 back.
#[test]
fn the_glide_goes_the_short_way_round() {
    let mut shown = (359.0, 0.0);
    let there = ease_look(&mut shown, (1.0, 0.0), 0.05, 100.0);
    assert!((there.0 - 359.0).rem_euclid(360.0) < 1.0, "{there:?}");
}

/// Off, the view is drawn exactly where the head points, at once: what every machine
/// had before this was a setting.
#[test]
fn without_the_setting_the_view_is_where_the_head_points() {
    let mut shown = (0.0, 0.0);
    assert_eq!(
        ease_look(&mut shown, (30.0, 10.0), 1.0 / 60.0, 0.0),
        (30.0, 10.0)
    );
    assert_eq!(shown, (30.0, 10.0));
}

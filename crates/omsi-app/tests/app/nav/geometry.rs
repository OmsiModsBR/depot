use openomsi_game::testing::nav::geometry::turn_towards;
use std::f64::consts::PI;

#[test]
fn the_view_turns_the_short_way_and_lands_on_the_target() {
    // from 350 degrees to 10: forwards through north, not back through 340 degrees
    let (a, b) = (350f64.to_radians(), 10f64.to_radians());
    let step = turn_towards(a, b, 1.0 / 60.0);
    assert!(step > a, "{step}");
    // it eases in a few frames and then sits exactly on the target
    let mut p = 0.0;
    for _ in 0..600 {
        p = turn_towards(p, PI * 0.75, 1.0 / 60.0);
    }
    assert_eq!(p, PI * 0.75);
    // no time passing, no turning; a target already reached stays
    assert_eq!(turn_towards(1.0, 2.0, 0.0), 1.0);
    assert_eq!(turn_towards(0.5, 0.5, 0.016), 0.5);
    // a step never overshoots
    let s = turn_towards(0.0, 1.0, 10.0);
    assert!(s <= 1.0 && s > 0.99);
}

use glam::DVec2;
use omsi_sim::traffic::pool_density as uvg_density;
use openomsi_game::testing;
use openomsi_game::testing::traffic::player_reach_ahead;

/// Berlin-Spandau's `unsched_vehgroups.txt`: NormalCars 1, Trucks 0, Commercials 1,
/// Ambulance 1, GDRCars 0.
const SPANDAU: [i32; 5] = [1, 0, 1, 1, 0];

#[test]
fn a_bus_under_a_bridge_is_not_in_the_way_on_it() {
    use glam::DVec3;
    use testing::traffic::in_player_box;
    let (c, f, r) = (
        DVec3::new(0.0, 0.0, 32.0),
        DVec2::new(0.0, 1.0),
        DVec2::new(1.0, 0.0),
    );
    // the road through the bus's box, on its level and on a bridge 5.4 m above it
    assert!(in_player_box(
        DVec3::new(0.5, 3.0, 32.3),
        c,
        f,
        r,
        2.5,
        6.0,
        6.0
    ));
    assert!(!in_player_box(
        DVec3::new(0.5, 3.0, 37.4),
        c,
        f,
        r,
        2.5,
        6.0,
        6.0
    ));
    assert!(!in_player_box(
        DVec3::new(0.5, 3.0, 26.0),
        c,
        f,
        r,
        2.5,
        6.0,
        6.0
    ));
    // beside it
    assert!(!in_player_box(
        DVec3::new(3.5, 3.0, 32.0),
        c,
        f,
        r,
        2.5,
        6.0,
        6.0
    ));
}

#[test]
fn a_following_bus_is_no_bus_in_the_way() {
    let north = DVec2::new(0.0, 1.0);
    // a car ahead going the same way: only the bus itself counts
    assert_eq!(
        player_reach_ahead(6.0, 14.0, 1.5, north, DVec2::new(0.1, 3.0)),
        6.0
    );
    // a car crossing its way (or coming towards it): where the bus will be counts too
    assert_eq!(
        player_reach_ahead(6.0, 14.0, 1.5, north, DVec2::new(3.0, 0.0)),
        27.0
    );
    assert_eq!(
        player_reach_ahead(6.0, 14.0, 1.5, north, DVec2::new(0.0, -3.0)),
        27.0
    );
}

#[test]
fn a_group_off_by_default_drives_where_a_path_asks_for_it() {
    // a Falkensee path: no rule for the normal cars, the GDR cars asked for
    let rules = [(4u16, 1.0f32)];
    assert_eq!(uvg_density(&rules, &SPANDAU, 4), 1.0);
    assert_eq!(uvg_density(&rules, &SPANDAU, 0), 1.0);
    // and nowhere else
    assert_eq!(uvg_density(&[], &SPANDAU, 4), 0.0);
    assert_eq!(uvg_density(&[(0, 0.5)], &SPANDAU, 1), 0.0);
}

#[test]
fn a_default_follows_the_first_group_on_the_path() {
    // commercials (default 1) take the normal cars' density of the path
    assert_eq!(uvg_density(&[(0, 0.4)], &SPANDAU, 2), 0.4);
    assert_eq!(uvg_density(&[(0, 0.0)], &SPANDAU, 2), 0.0);
    assert_eq!(uvg_density(&[], &SPANDAU, 2), 1.0);
    // an own rule wins
    assert_eq!(uvg_density(&[(0, 0.4), (2, 2.0)], &SPANDAU, 2), 2.0);
}

#[test]
fn defaults_naming_each_other_end() {
    assert_eq!(uvg_density(&[], &[1, 3, 2], 1), 0.0);
}

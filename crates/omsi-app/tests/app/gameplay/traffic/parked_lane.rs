use openomsi_game::testing::traffic::*;

#[test]
fn a_parked_body_blocks_an_otherwise_empty_target_lane() {
    assert!(parked_lane_clear(&[], 50.0, 30.0, 70.0, 0.9));
    assert!(!parked_lane_clear(&[(55.0, 0.0)], 50.0, 30.0, 70.0, 0.9));
    // The centre is outside the inspected strip, but the parked car's body is not.
    assert!(!parked_lane_clear(&[(121.0, 0.0)], 50.0, 30.0, 70.0, 0.9));
    assert!(!parked_lane_clear(&[(19.0, 0.0)], 50.0, 30.0, 70.0, 0.9));
    assert!(parked_lane_clear(
        &[(123.0, 0.0), (17.0, 0.0)],
        50.0,
        30.0,
        70.0,
        0.9
    ));
}

#[test]
fn parked_clearance_accounts_for_vehicle_width_and_both_sides() {
    for lateral in [-2.1, 2.1] {
        // A car fits; a wider bus would overlap the parked body.
        assert!(parked_lane_clear(&[(50.0, lateral)], 50.0, 4.0, 10.0, 0.9));
        assert!(!parked_lane_clear(
            &[(50.0, lateral)],
            50.0,
            7.0,
            14.0,
            1.25
        ));
    }
    assert!(parked_lane_clear(&[(50.0, 2.7)], 50.0, 7.0, 14.0, 1.25));
}

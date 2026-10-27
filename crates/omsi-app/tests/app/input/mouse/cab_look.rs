use openomsi_game::testing::input::mouse::cab_look_yaw;

/// A passenger and the driver turn all the way round, as in OMSI (#909).
#[test]
fn the_head_turns_all_the_way_round() {
    let mut yaw = 0.0;
    for _ in 0..40 {
        yaw = cab_look_yaw(yaw + 10.0);
    }
    // 400 degrees turned: 40 past straight ahead, the short way
    assert!((yaw - 40.0).abs() < 1e-3, "{yaw}");
    assert!((cab_look_yaw(170.0 + 20.0) + 170.0).abs() < 1e-3);
    // (the driver looks back down the saloon: no stop at 140 degrees)
    assert!((cab_look_yaw(175.0) - 175.0).abs() < 1e-3);
    assert!((cab_look_yaw(-160.0) + 160.0).abs() < 1e-3);
}

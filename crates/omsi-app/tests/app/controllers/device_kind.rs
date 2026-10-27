#[test]
fn a_mapped_constant_force_wheel_is_not_a_gamepad() {
    assert!(openomsi_game::testing::controllers::mapped_device_is_gamepad(true, false));
    assert!(!openomsi_game::testing::controllers::mapped_device_is_gamepad(true, true));
    assert!(!openomsi_game::testing::controllers::mapped_device_is_gamepad(false, true));
}

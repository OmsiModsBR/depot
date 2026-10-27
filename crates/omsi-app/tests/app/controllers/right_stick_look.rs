#[test]
fn disabling_automatic_look_keeps_driving_controls_and_assigned_look() {
    for assigned in [[0.0, 0.0], [0.5, -0.25]] {
        let mut analog = openomsi_game::testing::controllers::Analog {
            steering: Some(0.3),
            stick: true,
            throttle: Some(0.8),
            brake: Some(0.2),
            clutch: Some(0.4),
            look: assigned,
        };
        analog.apply_default_gamepad_look(false, 1.0, -1.0);
        assert_eq!(analog.look, assigned);
        assert_eq!(analog.steering, Some(0.3));
        assert!(analog.stick);
        assert_eq!(analog.throttle, Some(0.8));
        assert_eq!(analog.brake, Some(0.2));
        assert_eq!(analog.clutch, Some(0.4));
    }
}

#[test]
fn enabled_automatic_look_keeps_dead_zone_direction_and_assigned_axes() {
    let mut analog = openomsi_game::testing::controllers::Analog::default();
    analog.apply_default_gamepad_look(true, 0.1, -0.1);
    assert_eq!(analog.look, [0.0, 0.0]);
    analog.apply_default_gamepad_look(true, 1.0, -1.0);
    assert_eq!(analog.look, [1.0, 1.0]);
    analog.look = [0.25, -0.5];
    analog.apply_default_gamepad_look(true, -1.0, 1.0);
    assert_eq!(analog.look, [0.25, -0.5]);
}

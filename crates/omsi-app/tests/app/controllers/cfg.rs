use omsi_content::input::gamectrler::DeviceCfg;
use omsi_content::input::gamectrler::Func;
#[test]
fn feedback_keeps_the_physical_position_inside_the_steering_deadzone() {
    for (axis, deadzone) in [(0.2, 0.3), (0.01, 0.02)] {
        let (steering, position) =
            openomsi_game::testing::controllers::wheel_steering(axis, false, 0, deadzone, 1.0);
        assert_eq!(steering, 0.0);
        assert_eq!(position, axis);
        let f = openomsi_game::testing::controllers::FfInput {
            on: true,
            kmh: 30.0,
            dt: 0.016,
            ..Default::default()
        };
        let mut t = 0.0;
        let force = openomsi_game::testing::controllers::wheel_force(
            &f, position, position, &mut t, 1.0, 0.0,
        );
        assert!(force < 0.0, "axis={axis}: {force}");
    }
}

#[test]
fn steering_range_and_reversal_preserve_the_feedback_position() {
    let (steering, position) =
        openomsi_game::testing::controllers::wheel_steering(0.5, false, 0, 0.1, 2.0);
    assert!((steering - 8.0 / 9.0).abs() < 1e-6);
    assert_eq!(position, 0.5);
    let (steering, position) =
        openomsi_game::testing::controllers::wheel_steering(0.5, true, 0, 0.1, 2.0);
    assert!((steering + 8.0 / 9.0).abs() < 1e-6);
    assert_eq!(position, -0.5);
    let (steering, position) =
        openomsi_game::testing::controllers::wheel_steering(0.5, false, 0, 0.0, 0.25);
    assert_eq!(steering, 0.125);
    assert_eq!(position, 0.5);
}

#[test]
fn axis_characteristics_change_bus_steering_but_not_feedback_position() {
    let (steering, position) =
        openomsi_game::testing::controllers::wheel_steering(0.5, false, 24, 0.0, 1.0);
    assert_eq!(steering, 0.25);
    assert_eq!(position, 0.5);
    let (steering, position) =
        openomsi_game::testing::controllers::wheel_steering(0.5, true, 24, 0.0, 1.0);
    assert_eq!(steering, -0.25);
    assert_eq!(position, -0.5);
    let (steering, position) =
        openomsi_game::testing::controllers::wheel_steering(0.4, false, 2, 0.0, 1.0);
    assert!((steering - 0.5).abs() < 1e-6);
    assert_eq!(position, 0.4);
}

#[test]
fn force_feedback_scales_are_saved_per_controller() {
    let devices = vec![
        DeviceCfg {
            name: "Wheel A".into(),
            ff_scale: Some((2.0, 0.5)),
            ..Default::default()
        },
        DeviceCfg {
            name: "Wheel B".into(),
            ff_scale: Some((0.75, 1.25)),
            ..Default::default()
        },
    ];
    let saved =
        omsi_content::input::gamectrler::parse(&omsi_content::input::gamectrler::to_text(&devices));
    assert_eq!(saved[0].ff_scale, Some((2.0, 0.5)));
    assert_eq!(saved[1].ff_scale, Some((0.75, 1.25)));
}

#[test]
fn force_feedback_direction_is_saved_and_applied_per_wheel() {
    let devices = vec![
        DeviceCfg {
            name: "Wheel A".into(),
            ff_invert: Some(true),
            ..Default::default()
        },
        DeviceCfg {
            name: "Wheel B".into(),
            ff_invert: Some(false),
            ..Default::default()
        },
        DeviceCfg {
            name: "Uncalibrated".into(),
            ..Default::default()
        },
    ];
    let saved =
        omsi_content::input::gamectrler::parse(&omsi_content::input::gamectrler::to_text(&devices));
    assert_eq!(saved[0].ff_invert, Some(true));
    assert_eq!(saved[1].ff_invert, Some(false));
    assert_eq!(saved[2].ff_invert, None);
    assert!(openomsi_game::testing::controllers::feedback_inverted(
        Some(&saved[0]),
        false,
        false
    ));
    assert!(!openomsi_game::testing::controllers::feedback_inverted(
        Some(&saved[1]),
        true,
        false
    ));
    assert!(openomsi_game::testing::controllers::feedback_inverted(
        Some(&saved[2]),
        true,
        false
    ));
    assert!(!openomsi_game::testing::controllers::feedback_inverted(
        Some(&saved[0]),
        false,
        true
    ));
    assert!(openomsi_game::testing::controllers::feedback_inverted(
        Some(&saved[1]),
        true,
        true
    ));
    let mut reversed = DeviceCfg::default();
    reversed.axes[0] = Some((Func::Steering, true));
    assert!(!openomsi_game::testing::controllers::calibrated_steering_reversed(Some(&reversed)));
    reversed.ff_invert = Some(false);
    assert!(openomsi_game::testing::controllers::calibrated_steering_reversed(Some(&reversed)));
}

#[test]
fn exact_wheel_configuration_beats_a_shorter_alias() {
    let devices = vec![
        DeviceCfg {
            name: "G920".into(),
            ff_scale: Some((0.0, 0.0)),
            ..Default::default()
        },
        DeviceCfg {
            name: "G920 Driving Force Racing Wheel for Xbox One".into(),
            ff_scale: Some((1.2, 0.6)),
            ..Default::default()
        },
    ];
    let matched = openomsi_game::testing::controllers::find_device_cfg(
        &devices,
        "G920 Driving Force Racing Wheel for Xbox One",
    )
    .unwrap();
    assert_eq!(matched.ff_scale, Some((1.2, 0.6)));
}

#[test]
fn a_reversed_steering_axis_reverses_its_motor_force_too() {
    let mut wheel = DeviceCfg::default();
    wheel.axes[0] = Some((Func::Steering, true));
    wheel.axes[1] = Some((Func::Throttle, true));
    assert!(openomsi_game::testing::controllers::force_axis_reversed(
        Some(&wheel),
        Some(0)
    ));
    assert!(!openomsi_game::testing::controllers::force_axis_reversed(
        Some(&wheel),
        Some(1)
    ));
    assert!(!openomsi_game::testing::controllers::force_axis_reversed(
        Some(&wheel),
        None
    ));
}

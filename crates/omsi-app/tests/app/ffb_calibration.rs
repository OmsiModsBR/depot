use openomsi_game::testing::ffb_calibration::*;

fn response(first: f32, second: f32) -> Option<Result<bool, &'static str>> {
    let mut c = Calibration::new(PULSE_FORCE);
    assert_eq!(c.update(0.0, Some(0.0)), None);
    assert_eq!(c.update(0.41, Some(0.0)), Some(PULSE_FORCE));
    c.update(0.67, Some(first));
    if c.result.is_some() {
        return c.result;
    }
    assert_eq!(c.update(1.08, Some(first)), Some(-PULSE_FORCE));
    c.update(1.34, Some(first + second));
    c.result
}

#[test]
fn both_polarities_are_detected() {
    assert_eq!(response(0.025, -0.025), Some(Ok(false)));
    assert_eq!(response(-0.025, 0.025), Some(Ok(true)));
}

#[test]
fn selected_strength_is_used_and_bounded() {
    for (requested, expected) in [(0.0, PULSE_FORCE), (0.35, 0.35), (1.0, MAX_PULSE_FORCE)] {
        let mut c = Calibration::new(requested);
        c.update(0.0, Some(0.0));
        assert_eq!(c.update(0.41, Some(0.0)), Some(expected));
        c.update(0.67, Some(0.025));
        assert_eq!(c.update(1.08, Some(0.025)), Some(-expected));
    }
}

#[test]
fn movement_during_the_pulse_is_kept_when_the_wheel_returns_before_the_last_sample() {
    for direction in [1.0, -1.0] {
        let mut c = Calibration::new(PULSE_FORCE);
        c.update(0.0, Some(0.0));
        assert_eq!(c.update(0.41, Some(0.0)), Some(PULSE_FORCE));
        c.update(0.55, Some(0.025 * direction));
        c.update(0.68, Some(0.0));
        assert_eq!(c.update(1.09, Some(0.0)), Some(-PULSE_FORCE));
        c.update(1.23, Some(-0.025 * direction));
        c.update(1.36, Some(0.0));
        assert_eq!(c.result, Some(Ok(direction < 0.0)));
    }
}

#[test]
fn a_pulse_with_significant_motion_in_both_directions_is_rejected() {
    let mut c = Calibration::new(PULSE_FORCE);
    c.update(0.0, Some(0.0));
    c.update(0.41, Some(0.0));
    c.update(0.5, Some(0.02));
    c.update(0.6, Some(-0.02));
    c.update(0.68, Some(0.0));
    assert!(c.result.unwrap().is_err());
}

#[test]
fn inconclusive_motion_is_rejected() {
    assert!(response(0.0, 0.0).unwrap().is_err());
    assert!(response(0.025, 0.025).unwrap().is_err());
    assert!(response(0.025, -0.001).unwrap().is_err());
}

#[test]
fn excessive_motion_and_stalled_frames_abort() {
    assert!(response(0.11, -0.11).unwrap().is_err());
    let mut c = Calibration::new(PULSE_FORCE);
    c.update(0.1, Some(0.0));
    c.update(0.8, Some(0.0));
    assert!(c.result.unwrap().is_err());
}

#[test]
fn missing_axis_noise_and_end_stops_do_not_start_a_pulse() {
    let mut missing = Calibration::new(PULSE_FORCE);
    for i in 0..25 {
        assert_eq!(missing.update(i as f32 / 10.0, None), None);
    }
    assert!(missing.result.unwrap().is_err());
    let mut noisy = Calibration::new(PULSE_FORCE);
    for i in 0..60 {
        assert_eq!(
            noisy.update(i as f32 / 10.0, Some(if i % 2 == 0 { 0.01 } else { -0.01 })),
            None
        );
    }
    assert!(noisy.result.unwrap().is_err());
    let mut end_stop = Calibration::new(PULSE_FORCE);
    assert_eq!(end_stop.update(0.0, Some(0.9)), None);
    assert!(end_stop.result.unwrap().is_err());
}

#[test]
fn disconnect_during_a_pulse_aborts_without_another_pulse() {
    let mut c = Calibration::new(PULSE_FORCE);
    c.update(0.0, Some(0.0));
    assert_eq!(c.update(0.41, Some(0.0)), Some(PULSE_FORCE));
    assert_eq!(c.update(0.5, None), None);
    assert!(c.result.unwrap().is_err());
    assert_eq!(c.update(0.6, Some(0.0)), None);
}

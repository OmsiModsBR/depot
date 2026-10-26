use openomsi_game::testing::player::{mouse_follow, mouse_pedal, mouse_steering};

/// The mouse's wheel eases after the cursor by default; with the smoothing off it is where
/// the cursor says the same frame, as in OMSI (#1092) - only the first second after
/// switching the mouse steering on still eases, whichever it is.
#[test]
fn the_wheel_follows_the_cursor_at_once_without_smoothing() {
    let dt = 1.0 / 60.0;
    let k = mouse_follow(0.0, dt, true);
    assert!(k > 0.7 && k < 0.8, "{k}");
    assert_eq!(mouse_follow(0.0, dt, false), 0.0);
    assert_eq!(mouse_pedal(0.3, 0.9, mouse_follow(0.0, dt, false)), 0.9);
    let steer = 0.9 + (0.3 - 0.9) * mouse_follow(0.0, dt, false);
    assert_eq!(steer, 0.9);
    for smooth in [true, false] {
        let k = mouse_follow(1.0, dt, smooth);
        assert!((k - 0.5f32.powf(dt)).abs() < 1e-6, "{smooth}: {k}");
    }
}

#[test]
fn the_mouse_pedal_reaches_the_floor() {
    for fps in [30.0f32, 60.0, 144.0] {
        let k = (-(1.0 / fps) / 0.06f32).exp();
        let (mut t, mut b) = (0.0, 1.0);
        for _ in 0..(fps as usize) {
            t = mouse_pedal(t, 1.0, k);
            b = mouse_pedal(b, 0.0, k);
        }
        // exactly: the kickdown compares the throttle with 1
        assert_eq!(t, 1.0, "{fps} fps");
        assert_eq!(b, 0.0, "{fps} fps");
    }
    // on the way it still eases
    let k = (-(1.0 / 60.0f32) / 0.06).exp();
    let t = mouse_pedal(0.0, 1.0, k);
    assert!(t > 0.2 && t < 0.3, "{t}");
}

#[test]
fn the_mouse_steers_less_the_faster_the_bus() {
    let w = 1600.0;
    // standing and slow: the whole width is the whole lock
    assert_eq!(mouse_steering(0.0, w, 0.0), -1.0);
    assert_eq!(mouse_steering(1600.0, w, 5.0), 1.0);
    assert_eq!(mouse_steering(800.0, w, 0.0), 0.0);
    assert!((mouse_steering(1200.0, w, 10.0) - 0.5).abs() < 1e-6);
    // faster: divided by the speed in tens of km/h
    assert!((mouse_steering(1200.0, w, 50.0) - 0.1).abs() < 1e-6);
    assert!((mouse_steering(1600.0, w, 100.0) - 0.1).abs() < 1e-6);
    // backwards like standing
    assert!((mouse_steering(1200.0, w, -20.0) - 0.5).abs() < 1e-6);
    // it moves smoothly: no step anywhere across the window
    let mut last = mouse_steering(0.0, w, 30.0);
    for px in 1..=1600 {
        let s = mouse_steering(px as f32, w, 30.0);
        assert!((s - last).abs() < 0.001, "step at {px}");
        last = s;
    }
}

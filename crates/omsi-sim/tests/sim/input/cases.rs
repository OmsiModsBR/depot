use omsi_sim::testing::input::*;

#[test]
fn linear_and_old_steering_as_omsi() {
    let mut a = KeyboardAxes {
        linear: true,
        old_steering: true,
        lock_curvature: 0.1,
        speed_kmh: 50.0,
        ..Default::default()
    };
    a.right_key = true;
    for _ in 0..100 {
        a.update(0.01);
    }
    // 0.05 1/m a second against a lock of 0.1 1/m: half the lock in a second, at any speed
    assert!((a.steering - 0.5).abs() < 1e-3, "{}", a.steering);
    a.right_key = false;
    for _ in 0..100 {
        a.update(0.01);
    }
    assert!((a.steering - 0.5).abs() < 1e-3, "old steering: it stays");
    a.old_steering = false;
    for _ in 0..50 {
        a.update(0.01);
    }
    assert!(
        (a.steering - 0.25).abs() < 0.02,
        "it comes back at the same pace: {}",
        a.steering
    );
}

/// A wheel let go comes back as the castor pulls it - a fifth of the lock a second
/// standing, about half rolling at 25 km/h - or stays with Old Steering, the keys' wheel
/// and the one on a phone's screen alike (#1090: that one was back in the middle within a
/// third of a second at any speed).
#[test]
fn a_wheel_let_go_comes_back_as_the_castor_pulls_it() {
    let after = |v: f32, old: bool, secs: f32| {
        let a = KeyboardAxes {
            lock_curvature: 0.1,
            speed_kmh: v,
            old_steering: old,
            ..Default::default()
        };
        let mut s = 1.0;
        for _ in 0..(secs * 100.0).round() as usize {
            s = a.let_go(s, 0.01);
        }
        s
    };
    assert!(
        (after(0.0, false, 1.0) - 0.8).abs() < 1e-3,
        "{}",
        after(0.0, false, 1.0)
    );
    assert!(
        (after(25.0, false, 1.0) - (1.0 - 0.8 / (1.0 + 25.0 / 45.0))).abs() < 1e-3,
        "{}",
        after(25.0, false, 1.0)
    );
    assert_eq!(after(25.0, false, 3.0), 0.0);
    assert_eq!(after(25.0, true, 3.0), 1.0);
    // (the keys' wheel let go goes the same way)
    let mut k = KeyboardAxes {
        lock_curvature: 0.1,
        speed_kmh: 25.0,
        steering: 1.0,
        ..Default::default()
    };
    for _ in 0..100 {
        k.update(0.01);
    }
    assert!(
        (k.steering - after(25.0, false, 1.0)).abs() < 1e-5,
        "{}",
        k.steering
    );
}

/// A speed that is no number (a bus whose physics went NaN) leaves the wheel coming
/// back as standing still; its clamp stopped the game (#1045).
#[test]
fn a_speed_that_is_no_number_does_not_stop_the_game() {
    let mut a = KeyboardAxes {
        lock_curvature: 0.1,
        speed_kmh: f32::NAN,
        steering: 0.5,
        ..Default::default()
    };
    for _ in 0..10 {
        a.update(0.01);
    }
    assert!(a.steering.is_finite() && a.steering < 0.5, "{}", a.steering);
}

/// `[redSteerSpd]` with the steady pace: OMSI's keys at speed.
#[test]
fn red_steer_spd_slows_the_keys_at_speed() {
    for (v, want) in [
        (50.0, 0.5 * 1.5 * (-0.1f32 * 50.0 / 3.6).exp()),
        (10.0, 0.5),
    ] {
        let mut a = KeyboardAxes {
            linear: true,
            red_steer_spd: true,
            lock_curvature: 0.1,
            speed_kmh: v,
            ..Default::default()
        };
        a.right_key = true;
        for _ in 0..100 {
            a.update(0.01);
        }
        assert!(
            (a.steering - want).abs() < 1e-3,
            "{v} km/h: {} against {want}",
            a.steering
        );
    }
    assert!((0.5 * 1.5 * (-0.1f32 * 50.0 / 3.6).exp() - 0.187).abs() < 1e-3);
}

/// OMSI's keyboard pedals: the brake stays until the throttle key, the throttle
/// goes up to 0.85 (1 with throttle_amplify) and comes back by itself.
#[test]
fn pedals_as_omsi_works_them_from_the_keys() {
    let mut a = KeyboardAxes {
        pedal_hold: true,
        ..Default::default()
    };
    a.brake_key = true;
    for _ in 0..30 {
        a.update(0.01);
    }
    assert!((a.brake - 0.3).abs() < 1e-3, "1 a second: {}", a.brake);
    a.brake_key = false;
    for _ in 0..100 {
        a.update(0.01);
    }
    assert!((a.brake - 0.3).abs() < 1e-3, "the brake stays: {}", a.brake);
    a.throttle_key = true;
    a.update(0.01);
    assert_eq!(a.brake, 0.0);
    for _ in 0..100 {
        a.update(0.01);
    }
    assert!(
        (a.throttle - 0.85).abs() < 1e-3,
        "up to 0.85: {}",
        a.throttle
    );
    a.amplify_key = true;
    for _ in 0..20 {
        a.update(0.01);
    }
    assert!((a.throttle - 1.0).abs() < 1e-3, "amplified: {}", a.throttle);
    a.amplify_key = false;
    a.throttle_key = false;
    for _ in 0..50 {
        a.update(0.01);
    }
    assert!(
        (a.throttle - 0.5).abs() < 1e-3,
        "falls at 1 a second: {}",
        a.throttle
    );
    // the other way: the brake comes off with its key
    let mut b = KeyboardAxes {
        pedal_hold: false,
        brake: 0.6,
        ..Default::default()
    };
    for _ in 0..10 {
        b.update(0.01);
    }
    assert!((b.brake - 0.3).abs() < 1e-3, "{}", b.brake);
}

/// The centring key brings the wheel back in a straight line at OMSI's pace, not at once.
#[test]
fn steering_neutral_brings_the_wheel_back_steadily() {
    let mut a = KeyboardAxes {
        old_steering: true,
        lock_curvature: 0.1,
        steering: 0.8,
        ..Default::default()
    };
    a.neutral_key = true;
    a.update(0.1);
    a.neutral_key = false;
    assert!((a.steering - 0.75).abs() < 1e-4, "{}", a.steering);
    for _ in 0..10 {
        a.update(0.1);
    }
    assert!((a.steering - 0.25).abs() < 1e-3, "{}", a.steering);
    for _ in 0..10 {
        a.update(0.1);
    }
    assert_eq!(a.steering, 0.0);
    // a steering key ends it
    a.steering = 0.5;
    a.right_key = true;
    a.update(0.01);
    a.right_key = false;
    a.update(0.5);
    assert!(a.steering > 0.5, "old steering stays: {}", a.steering);
}

/// The centring key is the quick way back, as in OMSI, where it moves the wheel at the
/// keys' own pace: with the keys turning faster at speed than OMSI's figure, it brought
/// the wheel back slower than the key had turned it, and slower than letting go (#851).
#[test]
fn steering_neutral_is_never_slower_than_the_keys_or_letting_go() {
    let step = 1.0 / 60.0;
    for v in [0.0, 15.0, 30.0, 50.0, 80.0] {
        let turned = |neutral: bool| {
            let mut a = KeyboardAxes {
                lock_curvature: 0.13,
                speed_kmh: v,
                ..Default::default()
            };
            a.right_key = true;
            for _ in 0..60 {
                a.update(step);
            }
            let from = a.steering;
            a.right_key = false;
            a.neutral_key = neutral;
            a.update(step);
            a.neutral_key = false;
            for _ in 0..29 {
                a.update(step);
            }
            (from, a.steering)
        };
        let (from, centred) = turned(true);
        let (_, let_go) = turned(false);
        assert!(
            centred <= let_go + 1e-6,
            "{v} km/h: the centring key left {centred}, letting go {let_go}"
        );
        // half a second back at least half of what a second of the key turned
        assert!(
            centred <= from * 0.5 + 1e-3,
            "{v} km/h: {from} turned in a second, {centred} left after half a second back"
        );
    }
    // with OMSI's steady keys it stays OMSI's pace
    let mut a = KeyboardAxes {
        linear: true,
        lock_curvature: 0.1,
        speed_kmh: 50.0,
        steering: 0.8,
        ..Default::default()
    };
    a.neutral_key = true;
    a.update(0.1);
    assert!((a.steering - 0.75).abs() < 1e-4, "{}", a.steering);
    // with Old Steering the wheel does not come back by itself: the keys' pace, slowed
    // at speed with Dynamic steering as OMSI's centring is
    let mut a = KeyboardAxes {
        old_steering: true,
        red_steer_spd: true,
        lock_curvature: 0.13,
        speed_kmh: 50.0,
        steering: 0.8,
        ..Default::default()
    };
    a.right_key = true;
    a.update(0.1);
    let key_pace = (a.steering - 0.8) / 0.1;
    a.right_key = false;
    a.steering = 0.8;
    a.neutral_key = true;
    a.update(0.1);
    let back = (0.8 - a.steering) / 0.1;
    assert!(
        (back - key_pace).abs() < 1e-3,
        "{back} against the keys' {key_pace}"
    );
}

/// Num 5 held with the steering key towards the middle comes back faster than either
/// alone, and stops in the middle (#1652).
#[test]
fn centring_with_the_key_towards_the_middle_is_faster() {
    let back = |left: bool, neutral: bool| {
        let mut a = KeyboardAxes {
            lock_curvature: 0.13,
            speed_kmh: 30.0,
            steering: 0.9,
            ..Default::default()
        };
        a.left_key = left;
        a.neutral_key = neutral;
        for _ in 0..6 {
            a.update(1.0 / 60.0);
        }
        0.9 - a.steering
    };
    let (both, key, centre) = (back(true, true), back(true, false), back(false, true));
    assert!(
        both > key * 1.3 && both > centre * 1.3,
        "both {both}, key {key}, centring {centre}"
    );
    let mut a = KeyboardAxes {
        lock_curvature: 0.13,
        steering: 0.05,
        ..Default::default()
    };
    a.left_key = true;
    a.neutral_key = true;
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert_eq!(a.steering, 0.0);
}

/// OMSI treats the two steering keys symmetrically: either one works alone,
/// both together hold the current wheel position, and releasing one immediately lets
/// the other continue steering.
#[test]
fn opposite_steering_keys_hold_then_resume_remaining_direction() {
    let mut a = KeyboardAxes {
        steering: 0.25,
        ..Default::default()
    };

    a.left_key = true;
    a.update(0.1);
    let after_left = a.steering;
    assert!(
        after_left < 0.25,
        "left alone must steer left: {after_left}"
    );

    a.right_key = true;
    a.update(0.2);
    assert!(
        (a.steering - after_left).abs() < 1e-6,
        "both keys must hold the wheel: {} -> {}",
        after_left,
        a.steering
    );

    a.left_key = false;
    a.update(0.1);
    let after_right_resume = a.steering;
    assert!(
        after_right_resume > after_left,
        "releasing left while right remains held must steer right: {after_left} -> {after_right_resume}"
    );

    a.left_key = true;
    a.update(0.2);
    assert!(
        (a.steering - after_right_resume).abs() < 1e-6,
        "both keys must hold symmetrically: {} -> {}",
        after_right_resume,
        a.steering
    );

    a.right_key = false;
    a.update(0.1);
    assert!(
        a.steering < after_right_resume,
        "releasing right while left remains held must steer left: {after_right_resume} -> {}",
        a.steering
    );
}

#[test]
fn the_clutch_goes_down_at_once_and_comes_up_slowly() {
    let mut a = KeyboardAxes::default();
    a.clutch_key = true;
    a.update(0.01);
    assert_eq!(a.clutch, 1.0);
    a.clutch_key = false;
    for _ in 0..100 {
        a.update(0.01);
    }
    assert!((a.clutch - 0.3).abs() < 1e-3, "{}", a.clutch);
}

/// A key turns the wheel at the pace it comes back to the middle with, only a tenth
/// faster: a few seconds from the middle to full lock standing, quicker while rolling.
/// Let go, it comes back at very nearly the speed it was turned with and settles in the
/// middle without swinging through it; standing still it still comes back, just slowly.
#[test]
fn steering_returns_like_a_spring() {
    let step = 1.0 / 60.0;
    // standing: full lock takes a few seconds, not the old 1.25 s
    let mut s = KeyboardAxes::default();
    s.set(EngineAction::SteeringRight, true);
    let mut t_lock = None;
    for i in 1..=300 {
        s.update(step);
        if s.steering >= 1.0 && t_lock.is_none() {
            t_lock = Some(i as f32 * step);
        }
    }
    let t_lock = t_lock.expect("never reached full lock");
    assert!(
        (2.5..=5.0).contains(&t_lock),
        "middle to full lock standing took {t_lock} s"
    );
    // at 30 km/h a second of the key is under half a lock, and slower still at 80
    let mut k = KeyboardAxes {
        speed_kmh: 30.0,
        ..Default::default()
    };
    k.set(EngineAction::SteeringLeft, true);
    for _ in 0..60 {
        k.update(step);
    }
    assert!(
        k.steering < -0.3 && k.steering > -0.6,
        "held left for a second at 30 km/h: {}",
        k.steering
    );
    let mut fast = KeyboardAxes {
        speed_kmh: 80.0,
        ..Default::default()
    };
    fast.set(EngineAction::SteeringLeft, true);
    for _ in 0..60 {
        fast.update(step);
    }
    assert!(
        fast.steering > k.steering,
        "turned faster at 80 km/h ({}) than at 30 ({})",
        fast.steering,
        k.steering
    );
    // let go at 30 km/h: back in about the time it took (the key's pace, a tenth slower
    // than the turn), never through the middle
    k.set(EngineAction::SteeringLeft, false);
    let from = k.steering;
    let mut t_back = None;
    for i in 1..=300 {
        let before = k.steering;
        k.update(step);
        assert!(
            k.steering <= 0.0,
            "swung through the middle: {}",
            k.steering
        );
        assert!(
            k.steering >= before,
            "moved away from the middle: {} -> {}",
            before,
            k.steering
        );
        if k.steering == 0.0 && t_back.is_none() {
            t_back = Some(i as f32 * step);
        }
    }
    let t_back = t_back.expect("never settled");
    assert!(
        (0.8..=1.8).contains(&t_back),
        "back in {t_back} s from {from} (turned for 1 s)"
    );
    // standing still it still comes back, but slowly: well off centre after a second,
    // not stuck
    let mut s = KeyboardAxes {
        steering: -0.8,
        speed_kmh: 0.0,
        ..Default::default()
    };
    for _ in 0..60 {
        s.update(step);
    }
    assert!(
        s.steering < -0.3,
        "returned too fast standing still: {}",
        s.steering
    );
    assert!(
        s.steering > -0.78,
        "hardly came back standing still: {}",
        s.steering
    );
}

/// Mouse steering switched off: the wheel keeps its angle (no self-centring, whatever the
/// speed) until a steering key is pressed; then the keyboard steering takes over.
#[test]
fn a_wheel_left_by_the_mouse_stays_until_a_steering_key() {
    let mut a = KeyboardAxes {
        speed_kmh: 30.0,
        steering: 0.6,
        mouse_hold: true,
        ..Default::default()
    };
    for _ in 0..500 {
        a.update(0.01);
    }
    assert!((a.steering - 0.6).abs() < 1e-6, "it stays: {}", a.steering);
    a.left_key = true;
    a.update(0.01);
    assert!(a.steering < 0.6 && !a.mouse_hold, "a key takes over");
    a.left_key = false;
    let before = a.steering;
    for _ in 0..100 {
        a.update(0.01);
    }
    assert!(
        a.steering < before,
        "keyboard behaviour: it comes back by itself"
    );
}

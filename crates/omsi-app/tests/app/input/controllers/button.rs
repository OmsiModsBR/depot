#[test]
fn a_wheel_with_buttons_past_the_table_counts_them_in_order() {
    let moza = openomsi_game::testing::controllers::key_bitmap_buttons(
        "ffffffff ffffffffffffffff ffff000000000000 0 0 0 0 ffff00000000 0 0 0 0",
    );
    assert_eq!(moza.len(), 128);
    if cfg!(target_os = "linux") {
        assert_eq!(
            openomsi_game::testing::controllers::button_index(&moza, 0x120),
            Some(0)
        );
        assert_eq!(
            openomsi_game::testing::controllers::button_index(&moza, 0x12f),
            Some(15)
        );
        assert_eq!(
            openomsi_game::testing::controllers::button_index(&moza, 0x270),
            Some(16)
        );
        assert_eq!(
            openomsi_game::testing::controllers::button_index(&moza, 0x2c0),
            Some(96)
        );
        let pad: Vec<u32> = vec![0x130, 0x131, 0x133, 0x134];
        assert_eq!(
            openomsi_game::testing::controllers::button_index(&pad, 0x133),
            None
        );
    }
}

#[test]
fn a_device_lists_as_many_buttons_as_its_highest_number() {
    let moza = openomsi_game::testing::controllers::key_bitmap_buttons(
        "ffffffff ffffffffffffffff ffff000000000000 0 0 0 0 ffff00000000 0 0 0 0",
    );
    assert_eq!(
        openomsi_game::testing::controllers::button_count(&moza),
        128
    );
    if cfg!(target_os = "linux") {
        assert_eq!(
            openomsi_game::testing::controllers::button_count(&[0x130, 0x131, 0x133, 0x134]),
            5
        );
    }
    assert_eq!(openomsi_game::testing::controllers::button_count(&[]), 0);
}

#[test]
fn buttons_count_as_directinput_does() {
    if cfg!(target_os = "macos") {
        assert_eq!(
            openomsi_game::testing::controllers::code_button(0x9_0001),
            Some(0)
        );
        assert_eq!(
            openomsi_game::testing::controllers::code_button(0x9_0010),
            Some(15)
        );
    }
    if cfg!(target_os = "linux") {
        assert_eq!(
            openomsi_game::testing::controllers::code_button(0x1_0120),
            Some(0)
        );
        assert_eq!(
            openomsi_game::testing::controllers::code_button(0x1_02c0),
            Some(16)
        );
    }
    if cfg!(windows) {
        assert_eq!(openomsi_game::testing::controllers::code_button(3), Some(3));
        // an analog trigger (WGI axis code) is no numbered button
        assert_eq!(
            openomsi_game::testing::controllers::code_button(0x1_0004),
            None
        );
        // the D-pad is the first hat, not dropped
        assert_eq!(
            openomsi_game::testing::controllers::code_button(u32::MAX - 3),
            Some(openomsi_game::testing::controllers::HAT_BUTTONS)
        );
        assert_eq!(
            openomsi_game::testing::controllers::code_button(u32::MAX),
            Some(openomsi_game::testing::controllers::HAT_BUTTONS + 3)
        );
    }
}

#[test]
fn the_wheel_is_pulled_to_the_middle_harder_at_speed() {
    let mut t = 0.0;
    let f = |kmh| openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh,
        dt: 0.016,
        ..Default::default()
    };
    let slow =
        openomsi_game::testing::controllers::wheel_force(&f(10.0), 0.5, 0.5, &mut t, 1.0, 1.0);
    let fast =
        openomsi_game::testing::controllers::wheel_force(&f(60.0), 0.5, 0.5, &mut t, 1.0, 1.0);
    assert!(slow < 0.0 && fast < slow, "{slow} {fast}");
    // Moving to the right at a standstill, it is held back by tyre scrub.
    let turning =
        openomsi_game::testing::controllers::wheel_force(&f(0.0), 0.0, -0.05, &mut t, 1.0, 1.0);
    assert!(turning < 0.0);
}

#[test]
fn a_parked_wheel_is_not_pulled_to_the_middle() {
    let mut t = 0.0;
    for kmh in [-0.5, -0.1, 0.0, 0.1, 0.5] {
        // Include residual lateral acceleration from the body's suspension.
        let f = openomsi_game::testing::controllers::FfInput {
            on: true,
            kmh,
            lateral_accel: 3.0,
            dt: 0.016,
            ..Default::default()
        };
        for x in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            let force =
                openomsi_game::testing::controllers::wheel_force(&f, x, x, &mut t, 1.0, 0.0);
            assert_eq!(force, 0.0, "kmh={kmh}, steering={x}");
        }
    }
}

#[test]
fn parking_drag_opposes_turning_in_both_directions() {
    let mut t = 0.0;
    let f = openomsi_game::testing::controllers::FfInput {
        on: true,
        dt: 0.016,
        ..Default::default()
    };
    for x in [-0.5, 0.5] {
        let right =
            openomsi_game::testing::controllers::wheel_force(&f, x, x - 0.01, &mut t, 1.0, 0.0);
        let left =
            openomsi_game::testing::controllers::wheel_force(&f, x, x + 0.01, &mut t, 1.0, 0.0);
        assert!(right < 0.0 && left > 0.0, "{right} {left}");
        assert!((right + left).abs() < 1e-6, "{right} {left}");
    }
}

#[test]
fn self_aligning_torque_builds_up_as_the_bus_rolls_in_either_direction() {
    let mut t = 0.0;
    let force = |kmh, t: &mut f32| {
        let f = openomsi_game::testing::controllers::FfInput {
            on: true,
            kmh,
            dt: 0.016,
            ..Default::default()
        };
        openomsi_game::testing::controllers::wheel_force(&f, 0.5, 0.5, t, 1.0, 0.0)
    };
    let mut previous = 0.0;
    for kmh in [0.51, 1.0, 2.0, 3.0, 4.0, 5.0] {
        let forward = force(kmh, &mut t);
        let reverse = force(-kmh, &mut t);
        assert!(forward < previous, "kmh={kmh}: {forward} >= {previous}");
        assert_eq!(forward, reverse);
        previous = forward;
    }
}

#[test]
fn steering_resistance_drops_as_the_bus_starts_rolling() {
    let mut t = 0.0;
    let f = |kmh| openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh,
        dt: 0.016,
        ..Default::default()
    };
    let parked =
        openomsi_game::testing::controllers::wheel_force(&f(0.0), 0.2, 0.15, &mut t, 1.0, 0.0);
    let moving =
        openomsi_game::testing::controllers::wheel_force(&f(30.0), 0.2, 0.15, &mut t, 1.0, 0.0);
    assert!(parked < moving && moving < 0.0, "{parked} {moving}");
}

#[test]
fn a_returning_wheel_is_not_stopped_by_parking_drag() {
    let mut t = 0.0;
    let f = openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh: 20.0,
        dt: 0.016,
        ..Default::default()
    };
    let right = openomsi_game::testing::controllers::wheel_force(&f, 0.5, 0.55, &mut t, 1.0, 0.0);
    let left = openomsi_game::testing::controllers::wheel_force(&f, -0.5, -0.55, &mut t, 1.0, 0.0);
    assert!(right < 0.0 && left > 0.0, "{right} {left}");
}

#[test]
fn a_ninety_degree_turn_has_return_torque_across_wheel_ranges() {
    let mut t = 0.0;
    let f = openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh: 30.0,
        dt: 0.016,
        ..Default::default()
    };
    for range in [900.0, 1080.0, 1800.0, 2880.0] {
        let x = 90.0 / (range * 0.5);
        let right = openomsi_game::testing::controllers::wheel_force(&f, x, x, &mut t, 1.0, 0.0);
        let left = openomsi_game::testing::controllers::wheel_force(&f, -x, -x, &mut t, 1.0, 0.0);
        assert!(right < -0.05, "range={range}: {right}");
        assert_eq!(right, -left);
    }
}

#[test]
fn small_remaining_angles_keep_returning_while_the_bus_rolls() {
    let mut t = 0.0;
    for kmh in [5.0, 20.0, 70.0] {
        let f = openomsi_game::testing::controllers::FfInput {
            on: true,
            kmh,
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        for x in [0.005, 0.01, 0.05] {
            // Already returning slowly: do not let steering drag cancel
            // the last few degrees of return on a wheel with friction.
            let right = openomsi_game::testing::controllers::wheel_force(
                &f,
                x,
                x + 0.001,
                &mut t,
                1.0,
                0.0,
            );
            let left = openomsi_game::testing::controllers::wheel_force(
                &f,
                -x,
                -x - 0.001,
                &mut t,
                1.0,
                0.0,
            );
            assert!(
                right < -0.02 && left > 0.02,
                "kmh={kmh}, x={x}: {right} {left}"
            );
            assert!((right + left).abs() < 1e-6);
        }
    }
}

#[test]
fn return_torque_fades_smoothly_at_the_physical_centre() {
    let mut t = 0.0;
    let f = openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh: 30.0,
        dt: 0.016,
        ..Default::default()
    };
    let mut previous: f32 = 1.0;
    for x in [0.2, 0.1, 0.01, 0.001, 0.0001, 0.00001, 0.0] {
        let force = openomsi_game::testing::controllers::wheel_force(&f, x, x, &mut t, 1.0, 0.0);
        assert!(force.abs() < previous, "x={x}: {force}");
        if x == 0.0 {
            assert_eq!(force, 0.0);
        } else {
            assert!(force < 0.0);
        }
        if x <= 0.0001 {
            assert!(force.abs() < 0.002, "x={x}: {force}");
        }
        previous = force.abs();
    }
}

#[test]
fn a_moving_wheel_is_damped_when_it_crosses_the_centre() {
    let mut t = 0.0;
    for kmh in [5.0, 20.0, 70.0] {
        let f = openomsi_game::testing::controllers::FfInput {
            on: true,
            kmh,
            dt: 0.02,
            ..Default::default()
        };
        // At the centre there is no spring torque. The remaining force must
        // slow a crossing wheel in either direction rather than accelerate it.
        let right =
            openomsi_game::testing::controllers::wheel_force(&f, 0.0, -0.01, &mut t, 1.0, 0.0);
        let left =
            openomsi_game::testing::controllers::wheel_force(&f, 0.0, 0.01, &mut t, 1.0, 0.0);
        assert!(right < -0.015 && left > 0.015, "kmh={kmh}: {right} {left}");
        assert!((right + left).abs() < 1e-6);
        assert_eq!(
            openomsi_game::testing::controllers::wheel_force(&f, 0.0, 0.0, &mut t, 1.0, 0.0),
            0.0
        );
        // Steering strength controls damping too, independently of vibration.
        assert_eq!(
            openomsi_game::testing::controllers::wheel_force(&f, 0.0, -0.01, &mut t, 0.0, 0.0),
            0.0
        );
    }
}

#[test]
fn the_same_wheel_velocity_gives_the_same_force_at_different_frame_rates() {
    let mut t = 0.0;
    for velocity in [-0.5, 0.5] {
        let mut reference: Option<f32> = None;
        for fps in [20.0, 30.0, 60.0, 144.0] {
            let dt = 1.0 / fps;
            let f = openomsi_game::testing::controllers::FfInput {
                on: true,
                kmh: 30.0,
                dt,
                ..Default::default()
            };
            let force = openomsi_game::testing::controllers::wheel_force(
                &f,
                0.05,
                0.05 - velocity * dt,
                &mut t,
                1.0,
                0.0,
            );
            if let Some(previous) = reference {
                assert!(
                    (force - previous).abs() < 1e-6,
                    "fps={fps}: {force} {previous}"
                );
            } else {
                reference = Some(force);
            }
        }
    }
}

#[test]
fn a_real_turn_adds_aligning_torque_but_a_parked_bus_does_not() {
    let mut t = 0.0;
    let mut f = openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh: 30.0,
        dt: 0.016,
        ..Default::default()
    };
    let straight = openomsi_game::testing::controllers::wheel_force(&f, 0.2, 0.2, &mut t, 1.0, 0.0);
    f.lateral_accel = 3.0;
    let right_turn =
        openomsi_game::testing::controllers::wheel_force(&f, 0.2, 0.2, &mut t, 1.0, 0.0);
    assert!(right_turn < straight, "{straight} {right_turn}");
    f.kmh = 0.0;
    let parked = openomsi_game::testing::controllers::wheel_force(&f, 0.2, 0.2, &mut t, 1.0, 0.0);
    f.lateral_accel = 0.0;
    let parked_without_accel =
        openomsi_game::testing::controllers::wheel_force(&f, 0.2, 0.2, &mut t, 1.0, 0.0);
    assert_eq!(parked, parked_without_accel);
}

#[test]
fn steering_assist_lightens_turning_out() {
    let mut t = 0.0;
    let f = openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh: 25.0,
        lateral_accel: 2.0,
        dt: 0.016,
        ..Default::default()
    };
    let held = openomsi_game::testing::controllers::wheel_force(&f, 0.5, 0.5, &mut t, 1.0, 0.0);
    let turning_out =
        openomsi_game::testing::controllers::wheel_force(&f, 0.5, 0.48, &mut t, 1.0, 0.0);
    let returning =
        openomsi_game::testing::controllers::wheel_force(&f, 0.5, 0.52, &mut t, 1.0, 0.0);
    // Assistance still lightens turning out. A moving return also includes
    // damping, so its net torque need not exceed the outward-turning torque.
    assert!(
        held < turning_out && turning_out < 0.0,
        "{held} {turning_out}"
    );
    assert!(returning < 0.0, "{returning}");
}

#[test]
fn centering_does_not_grow_linearly_to_full_lock() {
    let mut t = 0.0;
    let f = openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh: 30.0,
        dt: 0.016,
        ..Default::default()
    };
    let quarter =
        openomsi_game::testing::controllers::wheel_force(&f, 0.25, 0.25, &mut t, 1.0, 0.0);
    let full = openomsi_game::testing::controllers::wheel_force(&f, 1.0, 1.0, &mut t, 1.0, 0.0);
    assert!(full < quarter && full > 2.5 * quarter, "{quarter} {full}");
}

#[test]
fn wheel_bumps_need_motion_and_a_suspension_or_impact_event() {
    assert_eq!(
        openomsi_game::testing::controllers::bump_strength(0.0, 0.0, 20.0),
        0.0
    );
    assert_eq!(
        openomsi_game::testing::controllers::bump_strength(1.0, 0.0, 0.0),
        0.0
    );
    assert_eq!(
        openomsi_game::testing::controllers::bump_strength(0.008, 0.08, 20.0),
        0.0
    );
    assert!(openomsi_game::testing::controllers::bump_strength(0.015, 0.0, 20.0) < 0.05);
    assert!(openomsi_game::testing::controllers::bump_strength(0.05, 0.0, 20.0) > 0.4);
    assert!(openomsi_game::testing::controllers::bump_strength(0.1, 0.0, 20.0) > 0.9);
    assert!(openomsi_game::testing::controllers::bump_strength(0.0, 1.0, 20.0) > 0.5);
}

#[test]
fn settling_ignores_seams_and_keeps_slow_motion_as_it_was() {
    let dt = 1.0 / 60.0;
    let mut settled = Vec::new();
    // the first frame (or the first back in the bus) starts from the travel as it is
    assert_eq!(
        openomsi_game::testing::controllers::settle(&mut settled, &[0.09], dt),
        vec![0.0]
    );
    // a 1.5 cm seam for one step
    let seam = openomsi_game::testing::controllers::settle(&mut settled, &[0.105], dt)[0];
    assert!(openomsi_game::testing::controllers::bump_strength(seam, 0.0, 20.0) < 0.05);
    // a 6 cm step (a pothole's edge)
    let mut settled = vec![0.0];
    let step = openomsi_game::testing::controllers::settle(&mut settled, &[0.06], dt)[0];
    assert!(openomsi_game::testing::controllers::bump_strength(step, 0.0, 20.0) > 0.45);
    // brake dive at 0.3 m/s: about as strong as the old one-step rate made it
    let mut settled = vec![0.0];
    let mut travel = 0.0;
    let mut ripple = 0.0;
    for _ in 0..60 {
        travel += 0.3 * dt;
        ripple = openomsi_game::testing::controllers::settle(&mut settled, &[travel], dt)[0];
    }
    let old = (0.3 - 0.12) / 0.9;
    assert!(
        (openomsi_game::testing::controllers::bump_strength(ripple, 0.0, 20.0) - old).abs() < 0.05,
        "{ripple}"
    );
}

/// Where the wheel plays the shaking as its own periodic effect, the force set each
/// frame carries none of it.
#[test]
fn the_shaking_leaves_the_force_when_the_wheel_plays_it() {
    let f = openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh: 0.0,
        vib_amp: 1.0,
        vib_period: 2.5,
        dt: 0.016,
        ..Default::default()
    };
    let mut t = 0.0;
    let soft = openomsi_game::testing::controllers::wheel_force(&f, 0.0, 0.0, &mut t, 0.0, 1.0);
    assert!(soft.abs() > 0.01, "{soft}");
    t = 0.0;
    let periodic = openomsi_game::testing::controllers::wheel_force(
        &openomsi_game::testing::controllers::FfInput { vib_amp: 0.0, ..f },
        0.0,
        0.0,
        &mut t,
        0.0,
        1.0,
    );
    assert_eq!(periodic, 0.0);
}

#[test]
fn a_wheel_bump_uses_the_device_vibration_strength() {
    let mut t = 0.0;
    let f = openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh: 20.0,
        wheel_bump: 1.0,
        dt: 0.016,
        ..Default::default()
    };
    let off = openomsi_game::testing::controllers::wheel_force(&f, 0.0, 0.0, &mut t, 0.0, 0.0);
    t = 0.0;
    let on = openomsi_game::testing::controllers::wheel_force(&f, 0.0, 0.0, &mut t, 0.0, 1.0);
    assert_eq!(off, 0.0);
    assert!(on.abs() > 0.15, "{on}");
    assert!(on.abs() < 0.5, "{on}");
    let lighter = openomsi_game::testing::controllers::wheel_force(
        &openomsi_game::testing::controllers::FfInput {
            wheel_bump: 0.25,
            ..f
        },
        0.0,
        0.0,
        &mut t,
        0.0,
        1.0,
    );
    assert!(lighter.abs() > on.abs() * 0.45, "{lighter} {on}");
    t = 0.37;
    let at_impact =
        openomsi_game::testing::controllers::wheel_force(&f, 0.0, 0.0, &mut t, 0.0, 1.0);
    assert!((on - at_impact).abs() < 0.001, "{on} {at_impact}");
}

#[test]
fn active_turning_feels_firmer_at_road_speed_than_in_town() {
    let mut t = 0.0;
    let f = |kmh| openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh,
        dt: 0.016,
        ..Default::default()
    };
    let town =
        openomsi_game::testing::controllers::wheel_force(&f(10.0), 0.5, 0.48, &mut t, 1.0, 0.0);
    let road =
        openomsi_game::testing::controllers::wheel_force(&f(70.0), 0.5, 0.48, &mut t, 1.0, 0.0);
    assert!(town < 0.0 && road < town * 1.4, "{town} {road}");
}

#[test]
fn steering_force_scale_changes_the_constant_force() {
    let mut t = 0.0;
    let f = openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh: 30.0,
        dt: 0.016,
        ..Default::default()
    };
    let zero = openomsi_game::testing::controllers::wheel_force(&f, 0.4, 0.4, &mut t, 0.0, 0.0);
    let normal = openomsi_game::testing::controllers::wheel_force(&f, 0.4, 0.4, &mut t, 1.0, 0.0);
    assert_eq!(zero, 0.0);
    assert!(normal.abs() > 0.05, "{normal}");
}

/// The largest the trembling of the tarmac and the engine may become (of the wheel's
/// full lock) - a wheel's centring spring is several times this, and the scripts' own
/// shaking is 0.25, so the tremble stays felt as a buzz and not as a push.
const MICRO_MAX: f32 = 0.25;

/// Ten seconds of the road at `kmh`, as sampled by the game.
fn road(kmh: f32) -> Vec<f32> {
    let mut m = openomsi_game::testing::controllers::Micro::default();
    (0..600)
        .map(|_| m.sample(1.0 / 60.0, kmh, 0.0, 0.0, 0.0, 1.0, 0.0))
        .collect()
}

/// The most the signal ever lines up with itself, at any lag up to two seconds. Sines
/// line themselves up; noise cannot, so this is what tells a road carrying a weight
/// from a road rattling.
fn self_similarity(v: &[f32]) -> f32 {
    let rms = (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
    let mut best = 0.0f32;
    for lag in 4..120.min(v.len()) {
        let mut num = 0.0f32;
        for i in lag..v.len() {
            num += v[i] * v[i - lag];
        }
        best = best.max((num / ((v.len() - lag) as f32) / rms / rms).abs());
    }
    best
}

#[test]
fn the_road_carries_a_weight_rather_than_a_rattle() {
    // A wheel is told one force at a time, and a force that repeats itself tells the
    // driver there is something solid underneath the bus, while one that never does
    // tells them the surface is broken up into loose pieces - which is what the
    // complaint was: a road made of nothing but noise reads as random however fast it
    // is felt. Noise scores about 0.2 here at any speed and the ride about 0.8, so
    // the grain left under it cannot pull this down where it matters.
    for kmh in [15.0, 30.0, 50.0, 80.0] {
        let alike = self_similarity(&road(kmh));
        assert!(
            alike > 0.5,
            "the road at {kmh} km/h carries nothing solid: {alike:.2}"
        );
    }
}
#[test]
fn the_road_is_under_the_tyres_and_not_merely_slow() {
    // How often the road changes is what the tyre feels, and it has to outrun the
    // hands: the rim goes wherever it is told and the hands go with it, so a change a
    // few times a second is a push to be dragged along by rather than a texture. The
    // short layer is what keeps this true as the bus speeds up.
    let rate = |v: &[f32]| {
        let mut crossings = 0;
        for i in 1..v.len() {
            if (v[i] > 0.0) != (v[i - 1] > 0.0) {
                crossings += 1;
            }
        }
        crossings as f32 / (v.len() as f32 / 60.0) / 2.0
    };
    assert!(
        rate(&road(30.0)) > 6.0,
        "the road in town is too slow to feel: {}",
        rate(&road(30.0))
    );
    assert!(
        rate(&road(50.0)) > 10.0,
        "the road at 50 is too slow to feel: {}",
        rate(&road(50.0))
    );
    assert!(
        rate(&road(80.0)) > 13.0,
        "the road does not quicken with the speed: {}",
        rate(&road(80.0))
    );
}

#[test]
fn the_tremble_stays_silent_at_a_standstill_and_appears_with_the_speed() {
    // how much is felt is the average of the tremble, not its loudest frame: the
    // grain is noise, and its peaks fall where the pattern happens to be
    let felt = |m: &mut openomsi_game::testing::controllers::Micro, kmh: f32| {
        (0..240)
            .map(|_| m.sample(1.0 / 60.0, kmh, 0.0, 0.0, 0.0, 1.0, 0.0).abs())
            .sum::<f32>()
            / 240.0
    };
    let mut m = openomsi_game::testing::controllers::Micro::default();
    assert_eq!(
        felt(&mut m, 0.0),
        0.0,
        "a bus standing still feels no grain at all"
    );
    let crawling = felt(&mut m, 5.0);
    let town = felt(&mut m, 30.0);
    let road = felt(&mut m, 50.0);
    let fast = felt(&mut m, 80.0);
    assert!(crawling > 0.0 && town > crawling * 2.0, "{crawling} {town}");
    // the finer layer is let go above 40 Hz and the coarser one carries the road
    // there, so the strength holds from town speed up rather than falling away
    assert!(
        road > town * 0.9,
        "the grain should be at its strongest by 30 km/h: {town} {road}"
    );
    assert!(
        fast > road * 0.9,
        "the grain thins out at speed: {fast} against {road}"
    );
    assert!(fast < MICRO_MAX, "the grain grows too strong: {fast}");
}

#[test]
fn the_tremble_is_read_along_the_road_not_along_the_clock() {
    let mut m = openomsi_game::testing::controllers::Micro::default();
    // two seconds at 50 km/h over one stretch of tarmac, then the same stretch again
    let over = |m: &mut openomsi_game::testing::controllers::Micro| {
        (0..120)
            .map(|_| m.sample(1.0 / 60.0, 50.0, 0.0, 0.0, 0.0, 1.0, 0.0))
            .collect::<Vec<_>>()
    };
    let first = over(&mut m);
    assert!(first.iter().any(|v| v.abs() > 0.001));
    let start = m.roll;
    assert!(
        start > 20.0,
        "the distance the bus rolled was not kept: {start}"
    );
    m.roll = 0.0;
    assert_eq!(
        over(&mut m),
        first,
        "the same stretch of road shook differently"
    );
    // a long drive does not walk the pattern off into a constant
    m.roll = 2_000_000.0;
    assert!(
        over(&mut m).iter().any(|v| v.abs() > 0.001),
        "the grain died out after a long drive"
    );
}

#[test]
fn a_wet_or_snowy_road_hums_more_than_dry_asphalt() {
    let felt = |cond: f32| {
        let mut m = openomsi_game::testing::controllers::Micro::default();
        (0..120)
            .map(|_| m.sample(1.0 / 60.0, 50.0, cond, 0.0, 0.0, 1.0, 0.0).abs())
            .sum::<f32>()
            / 120.0
    };
    let (dry, wet, snow) = (felt(0.0), felt(1.0), felt(2.0));
    assert!(wet > dry && snow > wet, "{dry} {wet} {snow}");
}

#[test]
fn the_engine_is_felt_idling_and_grows_with_what_it_is_doing() {
    let mut m = openomsi_game::testing::controllers::Micro::default();
    let off = (0..60)
        .map(|_| m.sample(1.0 / 60.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0))
        .fold(0.0f32, f32::max);
    assert_eq!(
        off, 0.0,
        "an engine that is not running is felt through nothing"
    );
    let peak = |rpm: f32, load: f32| {
        let mut m = openomsi_game::testing::controllers::Micro::default();
        // two seconds, so the engine's speed is settled and its angle has turned
        (0..120)
            .map(|_| m.sample(1.0 / 60.0, 0.0, 0.0, rpm, load, 0.0, 1.0))
            .fold(0.0f32, f32::max)
    };
    let idling = peak(700.0, 0.0);
    let working = peak(700.0, 1.0);
    let pulling = peak(2200.0, 1.0);
    assert!(
        idling > 0.001,
        "a running engine at idle is not felt at all: {idling}"
    );
    assert!(
        working > idling,
        "a loaded engine is not felt harder than an idling one: {idling} {working}"
    );
    assert!(
        pulling > working,
        "a fast engine is not felt harder than a slow one: {working} {pulling}"
    );
    assert!(
        pulling < MICRO_MAX,
        "the engine shakes the wheel too hard: {pulling}"
    );
}

#[test]
fn the_engine_leads_on_the_crankshaft_and_stays_under_the_aliasing_limit() {
    // six seconds of an engine at 700 rpm, which is where the buzz is at its most
    // characteristic; the whole of it is a sum of sines, so projecting the signal on
    // each frequency that is in it says how the sound is shared out between them
    let dt = 1.0f32 / 60.0;
    let mut m = openomsi_game::testing::controllers::Micro::default();
    let sig: Vec<f32> = (0..360)
        .map(|_| m.sample(dt, 0.0, 0.0, 700.0, 0.5, 0.0, 1.0))
        .collect();
    let partial = |f: f32| {
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (i, &x) in sig.iter().enumerate() {
            let w = std::f32::consts::TAU * f * dt * i as f32;
            re += x * w.cos();
            im += x * w.sin();
        }
        (re * re + im * im).sqrt() / sig.len() as f32 * 2.0
    };
    // at 700 rpm the crankshaft turns at 11.7 Hz and a four-stroke four fires at
    // 23.3 Hz; the rumble under them is at 2.9 Hz. What leads is the crankshaft.
    let rumble = partial(700.0 / 240.0);
    let crank = partial(700.0 / 60.0);
    let firing = partial(700.0 / 30.0);
    assert!(
        crank > rumble,
        "the buzz leads on the low rumble instead of the engine: {rumble} {crank}"
    );
    assert!(
        crank > firing,
        "the firing pulses lead instead of the engine turning: {crank} {firing}"
    );
    // a wheel is told its force once a frame, so nothing the engine does may ask for
    // a frequency the frame cannot carry
    for f in [700.0 / 240.0, 700.0 / 60.0, 700.0 / 30.0] {
        assert!(
            f < 28.0,
            "the engine asks for {f} Hz, past what a wheel can be told"
        );
    }
    // and at the top of the rev range, where the crank and the pulses have both been
    // let go, what is left is the rumble and not silence
    let mut m = openomsi_game::testing::controllers::Micro::default();
    let top: Vec<f32> = (0..360)
        .map(|_| m.sample(dt, 0.0, 0.0, 4500.0, 1.0, 0.0, 1.0))
        .collect();
    let mut im = 0.0f32;
    for (i, &x) in top.iter().enumerate() {
        im += x * (std::f32::consts::TAU * (4500.0 / 240.0) * dt * i as f32).sin();
    }
    let mass = (im / top.len() as f32).abs();
    assert!(mass > 0.01, "a flat-out engine is felt as nothing: {mass}");
}

#[test]
fn the_tremble_is_turned_off_by_its_settings() {
    let run = |road: f32, engine: f32| {
        let mut m = openomsi_game::testing::controllers::Micro::default();
        (0..120)
            .map(|_| m.sample(1.0 / 60.0, 60.0, 0.0, 900.0, 0.5, road, engine))
            .fold(0.0f32, f32::max)
    };
    assert_eq!(run(0.0, 0.0), 0.0);
    assert!(run(1.0, 0.0) > 0.0 && run(0.0, 1.0) > 0.0);
    assert!(
        run(2.0, 2.0) > run(1.0, 1.0),
        "the settings do not make it stronger"
    );
    // a bigger engine buzz is not a road that got rougher
    let mut road_only = openomsi_game::testing::controllers::Micro::default();
    let mut both = openomsi_game::testing::controllers::Micro::default();
    for _ in 0..60 {
        road_only.sample(1.0 / 60.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0);
        both.sample(1.0 / 60.0, 0.0, 0.0, 900.0, 0.0, 1.0, 1.0);
    }
    assert_eq!(
        road_only.roll, both.roll,
        "the engine changed how the road is read"
    );
}

#[test]
fn the_tremble_comes_out_of_the_wheel_through_the_vibration_strength() {
    let mut t = 0.0;
    let f = openomsi_game::testing::controllers::FfInput {
        on: true,
        kmh: 50.0,
        micro: 0.08,
        dt: 0.016,
        ..Default::default()
    };
    assert_eq!(
        openomsi_game::testing::controllers::wheel_force(&f, 0.0, 0.0, &mut t, 1.0, 0.0),
        0.0,
        "it shook the wheel with the vibrations switched off"
    );
    let felt = openomsi_game::testing::controllers::wheel_force(&f, 0.0, 0.0, &mut t, 1.0, 1.0);
    assert!((felt - 0.08).abs() < 0.0001, "{felt}");
    assert_eq!(
        openomsi_game::testing::controllers::wheel_force(
            &openomsi_game::testing::controllers::FfInput { micro: 0.5, ..f },
            0.0,
            0.0,
            &mut t,
            1.0,
            1.0
        ),
        0.5
    );
}

#[test]
fn a_vibration_eases_away_once_it_stops() {
    assert_eq!(
        openomsi_game::testing::controllers::fade_gain(
            0.0,
            openomsi_game::testing::controllers::FF_FADE
        ),
        1.0
    );
    assert_eq!(
        openomsi_game::testing::controllers::fade_gain(
            openomsi_game::testing::controllers::FF_FADE * 0.5,
            openomsi_game::testing::controllers::FF_FADE
        ),
        0.5
    );
    assert_eq!(
        openomsi_game::testing::controllers::fade_gain(
            openomsi_game::testing::controllers::FF_FADE,
            openomsi_game::testing::controllers::FF_FADE
        ),
        0.0
    );
    assert_eq!(
        openomsi_game::testing::controllers::fade_gain(
            openomsi_game::testing::controllers::FF_FADE * 3.0,
            openomsi_game::testing::controllers::FF_FADE
        ),
        0.0
    );
    // it never jumps: no frame hands the wheel a step of its own
    let mut last = 1.0;
    for i in 0..40 {
        let g = openomsi_game::testing::controllers::fade_gain(
            i as f32 / 40.0 * openomsi_game::testing::controllers::FF_FADE * 1.2,
            openomsi_game::testing::controllers::FF_FADE,
        );
        assert!(g <= last && last - g < 0.15, "{g} after {last}");
        last = g;
    }
    // a fade of nothing is the old way: it stops where it stands
    assert_eq!(
        openomsi_game::testing::controllers::fade_gain(0.5, 0.0),
        1.0
    );
}

#[test]
fn the_settings_fade_is_how_long_a_jolt_takes_to_go() {
    // The jolt the physics hands over is not faded by a gain applied afterwards: by the
    // time such a gain would start, the jolt it was following is already spent. So the
    // jolt is held and eased on the settings' own time, and this is what says whether
    // the setting reaches the jolt at all.
    let dt = 1.0f32 / 60.0;
    let tail = |fade: f32| {
        let mut held = 0.6f32;
        let mut frames = 0;
        for _ in 0..600 {
            held = held * (-dt / (fade * 0.3)).exp();
            frames += 1;
            if held < 0.002 {
                break;
            }
        }
        frames
    };
    let fast = tail(0.05);
    let slow = tail(0.6);
    assert!(
        fast < slow * 3,
        "the setting barely changes the jolt: {fast} {slow}"
    );
    assert!(fast >= 2, "the jolt was cut off in a single frame: {fast}");
    assert!(slow <= 120, "a long fade never ends: {slow}");
}

#[test]
fn a_scripts_shaking_eases_away_instead_of_stopping_dead() {
    // The scripts write the amplitude afresh every frame, so on the frame they stop it
    // is already zero and fading that fades nothing: the wheel drops the shake in a
    // single step, which is the jolt the fade is here to avoid. What the fade needs is
    // the last amplitude the scripts reached the wheel with, so this is what says
    // whether it is still there once they have gone quiet.
    let dt = 1.0f32 / 60.0;
    let mut vib = openomsi_game::testing::controllers::ScriptVib::default();
    let (amp, period) = vib.step(
        true,
        0.8,
        12.0,
        openomsi_game::testing::controllers::FF_FADE,
        dt,
    );
    assert_eq!(
        amp, 0.8,
        "the shake the scripts asked for never reached the wheel"
    );
    assert_eq!(
        period, 12.0,
        "the period the scripts asked for was not passed on"
    );
    // the frame the scripts stop: kept, not cut off
    let (mut amp, period) = vib.step(
        true,
        0.0,
        0.0,
        openomsi_game::testing::controllers::FF_FADE,
        dt,
    );
    assert!(
        amp > 0.5,
        "the shake stopped dead the frame the scripts did: {amp}"
    );
    assert_eq!(
        period, 12.0,
        "the period went with the amplitude instead of being kept"
    );
    // and it eases away to nothing rather than to a stop
    let mut frames = 0;
    while amp > 0.0 && frames < 600 {
        amp = vib
            .step(
                true,
                0.0,
                0.0,
                openomsi_game::testing::controllers::FF_FADE,
                dt,
            )
            .0;
        frames += 1;
    }
    assert_eq!(amp, 0.0, "the shake never finished fading: {amp}");
    // the setting is what decides how long that takes
    let mut fast = openomsi_game::testing::controllers::ScriptVib::default();
    fast.step(true, 0.8, 12.0, 0.05, dt);
    let mut quick = 0;
    while fast.step(true, 0.0, 0.0, 0.05, dt).0 > 0.0 && quick < 600 {
        quick += 1;
    }
    assert!(
        quick < frames,
        "the setting barely changes the shake: {quick} {frames}"
    );
    // with the fade at zero it stops where it stands, as it always did
    let mut none = openomsi_game::testing::controllers::ScriptVib::default();
    none.step(true, 0.8, 12.0, 0.0, dt);
    assert_eq!(
        none.step(true, 0.0, 0.0, 0.0, dt).0,
        0.0,
        "the fade at zero still hangs on"
    );
    // and a bus whose force feedback is off is not shaking at all
    let mut off = openomsi_game::testing::controllers::ScriptVib::default();
    off.step(
        true,
        0.8,
        12.0,
        openomsi_game::testing::controllers::FF_FADE,
        dt,
    );
    assert_eq!(
        off.step(
            false,
            0.0,
            0.0,
            openomsi_game::testing::controllers::FF_FADE,
            dt
        )
        .0,
        0.0
    );
}

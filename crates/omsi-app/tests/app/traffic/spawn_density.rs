use openomsi_game::testing::traffic_density::*;

// The behaviour under test is the one written down in the parity registry
// (docs/parity/traffic-people-density.md): vehicles appear lane by lane, at a rate that is
// a product of independent factors, and the only global limit is the cap.

#[test]
fn the_road_factor_scales_the_rate_linearly() {
    let base = lane_rate(1.0, 1.0, 1.0, 1);
    for k in [0.5f32, 2.0, 3.0] {
        let r = lane_rate(1.0, 1.0, k, 1);
        assert!((r / base - k).abs() < 1e-5, "factor {k} gave {}", r / base);
    }
}

#[test]
fn a_curve_above_one_or_two_is_used_as_it_is() {
    let r = lane_rate(1.0, 3.5, 1.0, 1);
    assert!((r / lane_rate(1.0, 1.0, 1.0, 1) - 3.5).abs() < 1e-5);
}

#[test]
fn a_wide_road_splits_its_density_between_its_lanes() {
    let one = lane_rate(1.0, 1.0, 1.0, 1);
    let three = lane_rate(1.0, 1.0, 1.0, 3);
    assert!((three * 3.0 - one).abs() < 1e-5);
}

#[test]
fn no_density_means_no_vehicle_ever() {
    assert_eq!(lane_rate(0.0, 1.0, 1.0, 1), 0.0);
    assert_eq!(lane_rate(1.0, 0.0, 1.0, 1), 0.0);
    assert!(next_interval(0.0, 100.0, 0.5).is_none());
    assert_eq!(initial_count(0.0, 100.0, 0.5, 0.5), 0);
}

#[test]
fn a_tiny_rate_is_raised_to_the_floor_not_dropped() {
    assert_eq!(lane_rate(1.0, 0.001, 0.01, 1), MIN_RATE);
}

#[test]
fn the_interval_falls_with_the_rate_and_with_the_length() {
    let slow = mean_interval(1.0, 100.0).unwrap();
    assert!((mean_interval(2.0, 100.0).unwrap() * 2.0 - slow).abs() < 1e-3);
    assert!((mean_interval(1.0, 200.0).unwrap() * 2.0 - slow).abs() < 1e-3);
}

#[test]
fn the_interval_is_random_around_its_mean() {
    let mean = mean_interval(1.0, 100.0).unwrap();
    let (lo, hi) = (
        next_interval(1.0, 100.0, 0.0).unwrap(),
        next_interval(1.0, 100.0, 0.999).unwrap(),
    );
    assert!(lo < mean && mean < hi);
}

#[test]
fn the_first_fill_is_proportional_to_the_rate_and_the_length() {
    let mean = |rate: f32, len: f32| {
        let n = 2000;
        (0..n)
            .map(|i| {
                let u = (i as f32 + 0.5) / n as f32;
                initial_count(rate, len, u, (u * 7.0).fract()) as f32
            })
            .sum::<f32>()
            / n as f32
    };
    let one = mean(1.0, 400.0);
    assert!(one > 0.5, "a 400 m lane starts with {one} cars on average");
    assert!((mean(2.0, 400.0) / one - 2.0).abs() < 0.15);
    assert!((mean(1.0, 800.0) / one - 2.0).abs() < 0.15);
}

#[test]
fn a_clock_fires_once_per_step_and_never_catches_up() {
    let mut c = SpawnClock::new(10.0);
    assert!(!c.step(4.0));
    assert!(!c.step(4.0));
    assert!(c.step(4.0));
    // a long stall is one spawn, not three
    c.restart(10.0);
    assert!(c.step(35.0));
    // the lateness is dropped: the next wait starts whole
    c.restart(10.0);
    assert!(!c.step(1.0));
}

#[test]
fn a_clock_stands_still_in_the_pause() {
    let mut c = SpawnClock::new(1.0);
    assert!(!c.step(0.0));
    assert!(c.step(1.0));
}

#[test]
fn only_the_cap_limits_the_count() {
    assert_eq!(spawn_room(200, 0), 200);
    assert_eq!(spawn_room(200, 199), 1);
    assert_eq!(spawn_room(200, 200), 0);
    assert_eq!(spawn_room(200, 350), 0);
    assert_eq!(spawn_room(0, 0), 0);
}

#[test]
fn walkers_follow_the_people_curve_and_stop_at_half_the_pool() {
    assert_eq!(walker_target(14, 1.0, 1.0, 1000), 14);
    assert_eq!(walker_target(14, 1.0, 2.0, 1000), 28);
    // the curve is not clamped at 3
    assert_eq!(walker_target(14, 1.0, 4.0, 1000), 56);
    assert_eq!(walker_target(14, 1.0, 4.0, 40), 20);
    assert_eq!(walker_target(14, 1.0, 1.0, 0), 0);
    assert_eq!(walker_target(14, 1.0, -1.0, 1000), 0);
}

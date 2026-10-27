use omsi_sim::traffic::TrafficLightController;
use openomsi_game::testing::traffic::{
    mirror_light_tick, reset_light_runtime, set_mirror_light_clock,
};

fn program() -> TrafficLightController {
    TrafficLightController::from_program(
        vec![(vec![(0, 4.0), (6, 4.0)], Some(25.0))],
        Some(8.0),
        &[[0.0, 4.0, 1.0]],
        &[[0.0, 6.0, 1.0, 1.0]],
    )
}

#[test]
fn host_hold_survives_missing_local_requests() {
    let mut ctl = program();
    ctl.stops[0].if_request = false;
    ctl.start(4.0);
    set_mirror_light_clock(&mut ctl, 4.0, true);
    for _ in 0..60 {
        mirror_light_tick(&mut ctl, 0.1, 20_000.0);
    }
    assert_eq!(ctl.time, 4.0);
    assert!(ctl.held);
    assert_eq!(ctl.state(0), 6);
}

#[test]
fn unheld_host_clock_crosses_local_stop_and_jump_points() {
    let mut ctl = program();
    ctl.start(3.5);
    set_mirror_light_clock(&mut ctl, 3.5, false);
    mirror_light_tick(&mut ctl, 1.0, 20_000.0);
    assert_eq!(ctl.time, 4.5);
    assert_eq!(ctl.state(0), 6);
    mirror_light_tick(&mut ctl, 2.0, 20_000.0);
    assert_eq!(ctl.time, 6.5);
    assert!(!ctl.held);
}

#[test]
fn first_snapshot_is_not_replaced_by_the_day_time_seed() {
    let mut ctl = program();
    ctl.offset = 0.75;
    set_mirror_light_clock(&mut ctl, 6.5, false);
    mirror_light_tick(&mut ctl, 0.0, 20_000.0);
    assert_eq!(ctl.time, 6.5);
    assert_eq!(ctl.state(0), 6);
}

#[test]
fn a_new_crossing_uses_the_day_clock_until_its_first_snapshot() {
    let mut ctl = program();
    ctl.offset = 0.75;
    mirror_light_tick(&mut ctl, 0.0, 10.0);
    assert_eq!(ctl.time, 2.75);
    set_mirror_light_clock(&mut ctl, 6.5, true);
    mirror_light_tick(&mut ctl, 1.0, 10.0);
    assert_eq!(ctl.time, 6.5);
    assert!(ctl.held);
}

#[test]
fn host_seek_release_and_cycle_wrap_replace_interpolation() {
    let mut ctl = program();
    set_mirror_light_clock(&mut ctl, 7.75, false);
    mirror_light_tick(&mut ctl, 0.5, 0.0);
    assert_eq!(ctl.time, 0.25);
    assert_eq!(ctl.state(0), 0);
    set_mirror_light_clock(&mut ctl, 10.5, true);
    mirror_light_tick(&mut ctl, 2.0, 0.0);
    assert_eq!(ctl.time, 2.5);
    set_mirror_light_clock(&mut ctl, 1.5, false);
    mirror_light_tick(&mut ctl, 1.0, 0.0);
    assert_eq!(ctl.time, 2.5);
    assert!(!ctl.held);
    mirror_light_tick(&mut ctl, -1.0, 0.0);
    assert_eq!(ctl.time, 2.5);
}

#[test]
fn returning_to_local_simulation_discards_a_previously_passed_stop() {
    let mut ctl = program();
    ctl.offset = 0.75;
    ctl.start(3.25);
    ctl.request[0] = true;
    ctl.advance(0.0);
    assert!(!ctl.held);
    set_mirror_light_clock(&mut ctl, 4.0, false);
    reset_light_runtime(&mut ctl, 20_000.0);
    assert_eq!(ctl.lights, vec![vec![(0, 4.0), (6, 4.0)]]);
    assert_eq!(ctl.approach, vec![Some(25.0)]);
    assert_eq!(ctl.offset, 0.75);
    assert_eq!(ctl.stops.len(), 2);
    assert_eq!(ctl.request, vec![false]);
    ctl.advance(0.0);
    assert!(ctl.held);
    assert_eq!(ctl.time, 4.0);
}

#[test]
fn returning_to_local_simulation_discards_a_previous_backward_jump() {
    let mut ctl = program();
    ctl.start(5.5);
    ctl.advance(0.5);
    assert_eq!(ctl.time, 1.0);
    set_mirror_light_clock(&mut ctl, 6.0, false);
    reset_light_runtime(&mut ctl, 20_000.0);
    ctl.advance(0.0);
    assert_eq!(ctl.time, 1.0);
    assert!(!ctl.held);
}

#[test]
fn returning_to_local_simulation_rechecks_a_host_hold() {
    let mut ctl = program();
    ctl.stops[0].if_request = false;
    set_mirror_light_clock(&mut ctl, 4.0, true);
    reset_light_runtime(&mut ctl, 20_000.0);
    assert!(!ctl.held);
    ctl.advance(0.5);
    assert!(!ctl.held);
    assert_eq!(ctl.time, 4.5);
}

#[test]
fn a_new_crossing_keeps_its_first_seed_when_returning_to_local_simulation() {
    let mut ctl = program();
    ctl.offset = 0.75;
    reset_light_runtime(&mut ctl, 10.0);
    assert_eq!(ctl.time, 2.75);
    ctl.start(20_000.0);
    ctl.advance(0.5);
    assert_eq!(ctl.time, 3.25);
}

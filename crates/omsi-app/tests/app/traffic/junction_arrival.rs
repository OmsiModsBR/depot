use omsi_sim::traffic::AiState;
use openomsi_game::testing::traffic::{crossing_arrival, queued_exit_vehicle};

#[test]
fn stopped_queue_does_not_predict_a_restart() {
    let st = AiState::new(0, 0.0, 1);
    assert_eq!(crossing_arrival(&st, 10.0, false, false, true), f32::MAX);
}

#[test]
fn crawling_queue_is_measured_at_its_actual_speed() {
    let mut st = AiState::new(0, 0.0, 1);
    for speed in [0.2, 0.3, 0.4, 0.5, 0.8, 1.4] {
        st.speed = speed;
        assert!((crossing_arrival(&st, 10.0, false, false, true) - 10.0 / speed).abs() < 1e-5);
    }
}

#[test]
fn crawling_queue_allows_a_gap_but_nearby_traffic_still_counts() {
    let mut st = AiState::new(0, 0.0, 1);
    st.speed = 0.4;
    let gap = 7.5;
    assert!(crossing_arrival(&st, 10.0, false, false, true) > gap);
    assert!(crossing_arrival(&st, 1.0, false, false, true) < gap);
    // Once it can move freely again, account for it accelerating towards the crossing.
    assert!(crossing_arrival(&st, 10.0, true, false, false) < gap);
}

#[test]
fn queue_already_at_the_conflict_still_blocks() {
    let st = AiState::new(0, 0.0, 1);
    for distance in [-1.0, 0.0, 0.3] {
        assert_eq!(crossing_arrival(&st, distance, false, true, true), 0.0);
    }
}

#[test]
fn freely_starting_car_keeps_its_accelerating_prediction() {
    let st = AiState::new(0, 0.0, 1);
    let expected = (20.0 / st.accel).sqrt() + st.reaction;
    assert!((crossing_arrival(&st, 10.0, false, false, false) - expected).abs() < 1e-5);
    assert!((crossing_arrival(&st, 10.0, true, false, false) - expected).abs() < 1e-5);
}

#[test]
fn car_waiting_before_the_conflict_is_not_approaching() {
    let mut st = AiState::new(0, 0.0, 1);
    st.speed = 2.0;
    assert_eq!(crossing_arrival(&st, 10.0, false, true, false), f32::MAX);
    assert!(crossing_arrival(&st, 10.0, true, false, false) < 5.0);
    assert_eq!(crossing_arrival(&st, 10.0, false, false, false), 5.0);
}

#[test]
fn a_queue_on_a_short_lane_after_the_exit_keeps_the_junction_clear() {
    // Lanes 20 and 21 are two short pieces immediately beyond the crossing. The first
    // is empty; a car whose rear is 1 m into the second leaves only 3 m beyond the exit.
    let way = [(10, -20.0), (20, 5.0), (21, 7.0), (22, 10.0), (23, 40.0)];
    assert_eq!(
        queued_exit_vehicle(&way, (20, 5.0), 8.0, [(21, 1.0, 0.0)]),
        Some((3.0, 0.0, 21))
    );
}

#[test]
fn traffic_beyond_the_space_the_car_needs_does_not_close_the_exit() {
    let way = [(10, -20.0), (20, 5.0), (21, 7.0), (22, 10.0), (23, 40.0)];
    assert_eq!(
        queued_exit_vehicle(&way, (20, 5.0), 8.0, [(22, 5.0, 0.0), (23, 0.0, 0.0)]),
        None
    );
}

#[test]
fn the_nearest_vehicle_across_exit_pieces_wins() {
    let way = [(10, -20.0), (20, 5.0), (21, 7.0), (22, 10.0)];
    assert_eq!(
        queued_exit_vehicle(
            &way,
            (20, 5.0),
            10.0,
            [(21, 4.0, 1.0), (20, 5.0, 0.5), (22, -1.0, 0.0)]
        ),
        Some((4.0, 0.0, 22))
    );
}

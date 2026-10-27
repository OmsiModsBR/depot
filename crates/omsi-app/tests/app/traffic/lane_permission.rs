use glam::DVec3;
use omsi_sim::traffic::{AiState, LaneBuilder, LaneKind};
use openomsi_game::testing::traffic::*;

#[test]
fn lane_changes_follow_type_and_pool_permissions() {
    let mut lane = LaneBuilder::arc(DVec3::ZERO, 0.0, 20.0, 0.0, 0.0, LaneKind::Street, 3.0);
    let mut state = AiState::new(0, 0.0, 1);
    lane.no_cars = true;
    lane.rule_bus = true;
    assert!(!lane_open_to(&lane, &state));
    state.veh_type = 1;
    assert!(
        lane_open_to(&lane, &state),
        "taxis may use an explicitly open bus lane"
    );
    state.veh_type = 2;
    assert!(lane_open_to(&lane, &state));
    state.traffic_pool = Some((2, vec![1, 1, 1].into()));
    lane.group_density = vec![(2, 0.0)];
    assert!(!lane_open_to(&lane, &state));
    lane.group_density = vec![(2, 0.5)];
    lane.density = 0.0;
    assert!(
        lane_open_to(&lane, &state),
        "the specific pool rule matches route-planning semantics"
    );
    lane.group_density.clear();
    state.traffic_pool = Some((2, vec![1, 1, 0].into()));
    assert!(
        !lane_open_to(&lane, &state),
        "disabled default groups remain closed without a path override"
    );
}

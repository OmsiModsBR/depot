use glam::DVec3;
use omsi_sim::testing::traffic::*;
#[test]
fn independent_rules_and_default_references() {
    let mut lane = LaneBuilder::arc(DVec3::ZERO, 0.0, 100.0, 0.0, 0.0, LaneKind::Street, 3.0);
    lane.group_density = vec![(0, 1.0), (1, 0.1), (3, 0.001), (5, 0.0)];
    let defaults = [1, 0, 1, 0, 0, 0];
    assert_eq!(lane.pool_density(&defaults, 1), 0.1);
    assert_eq!(lane.pool_density(&defaults, 2), 1.0);
    assert_eq!(lane.pool_density(&defaults, 3), 0.001);
    assert_eq!(lane.pool_density(&defaults, 4), 0.0);
    assert_eq!(lane.pool_density(&defaults, 5), 0.0);
    lane.group_density.push((1, 0.5));
    assert_eq!(lane.pool_density(&defaults, 1), 0.5);
}

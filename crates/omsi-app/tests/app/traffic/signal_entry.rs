use glam::DVec3;
use omsi_sim::traffic::{LaneBuilder, LaneKey};
use omsi_sim::traffic::{LaneKind, Network};
use openomsi_game::testing::traffic::*;

fn lane(id: Option<i64>, light: Option<(usize, usize)>) -> omsi_sim::traffic::Lane {
    let mut lane = LaneBuilder::arc(DVec3::ZERO, 0.0, 20.0, 0.0, 0.0, LaneKind::Street, 3.0);
    lane.source = 2;
    lane.key = id.map(|id| LaneKey {
        tile: (0, 0),
        id,
        path: 0,
    });
    lane.traffic_light = light;
    lane
}

#[test]
fn a_signal_does_not_require_scenery_identity() {
    let mut net = Network {
        lanes: vec![lane(None, None), lane(None, Some((0, 1)))],
        ..Default::default()
    };
    let way = [(0, -5.0), (1, 15.0)];
    assert_eq!(entry_light(&net, &way, 1), Some((0, 1)));
    net.lanes[1].source = 1;
    net.lanes[1].key = Some(LaneKey {
        tile: (0, 0),
        id: 10,
        path: 0,
    });
    assert_eq!(entry_light(&net, &way, 1), Some((0, 1)));
}

#[test]
fn junction_interior_deduplication_keeps_the_next_junction_signal() {
    let net = Network {
        lanes: vec![
            lane(Some(10), Some((0, 0))),
            lane(Some(10), Some((0, 1))),
            lane(Some(11), Some((1, 0))),
        ],
        ..Default::default()
    };
    let way = [(0, -5.0), (1, 15.0), (2, 35.0)];
    assert_eq!(entry_light(&net, &way, 0), Some((0, 0)));
    assert_eq!(entry_light(&net, &way, 1), None);
    assert_eq!(entry_light(&net, &way, 2), Some((1, 0)));
}

use glam::DVec3;
use omsi_sim::testing::traffic::*;

fn straight(x: f64, y0: f64, y1: f64, id: i64, tile: (i32, i32)) -> Lane {
    let mut l = LaneBuilder::polyline(
        vec![DVec3::new(x, y0, 0.0), DVec3::new(x, y1, 0.0)],
        LaneKind::Street,
        3.0,
    );
    l.key = Some(LaneKey { tile, id, path: 0 });
    l.source = 1;
    l
}

#[test]
fn extend_links_like_link() {
    let all = vec![
        straight(0.0, 0.0, 100.0, 1, (0, 0)),
        straight(0.0, 100.0, 200.0, 2, (0, 0)),
        straight(0.0, 200.0, 300.0, 3, (0, 1)),
        straight(0.0, 300.0, 400.0, 4, (0, 1)),
    ];
    let mut whole = Network {
        lanes: all.clone(),
        ..Default::default()
    };
    whole.link(1.5);
    let mut grown = Network {
        lanes: all[..2].to_vec(),
        ..Default::default()
    };
    grown.link(1.5);
    let r = grown.extend(all[2..].to_vec(), 1.5);
    assert_eq!(r, 2..4);
    for i in 0..4 {
        assert_eq!(grown.lanes[i].next, whole.lanes[i].next, "lane {i}");
        assert_eq!(grown.prev[i], whole.prev[i], "lane {i}");
    }
    assert_eq!(grown.reach, whole.reach);
    assert_eq!(grown.crossings.len(), 4);
    assert_eq!(
        grown.find(
            LaneKey {
                tile: (0, 1),
                id: 3,
                path: 0
            },
            None
        ),
        Some(2)
    );
    assert_eq!(
        grown
            .nearest_lane(DVec3::new(0.5, 350.0, 0.0), LaneKind::Street)
            .map(|n| n.0),
        Some(3)
    );
}

#[test]
fn start_grid_tracks_added_lanes_in_map_order() {
    let first = vec![
        straight(0.0, 0.0, 100.0, 1, (0, 0)),
        straight(300.0, 0.0, 100.0, 2, (1, 0)),
    ];
    let mut net = Network {
        lanes: first,
        ..Default::default()
    };
    net.link(1.5);
    let near = |net: &Network| {
        net.lanes_starting_near(DVec3::ZERO, 60.0)
            .into_iter()
            .filter(|&i| net.lanes[i].start().truncate().length() < 60.0)
            .collect::<Vec<_>>()
    };
    assert_eq!(near(&net), vec![0]);
    net.extend(vec![straight(25.0, 0.0, 100.0, 3, (0, 1))], 1.5);
    assert_eq!(near(&net), vec![0, 2]);
}

#[test]
fn added_lanes_only_change_reach_of_their_predecessors() {
    let old = vec![
        straight(0.0, 0.0, 100.0, 1, (0, 0)),
        straight(0.0, 100.0, 200.0, 2, (0, 0)),
        straight(500.0, 0.0, 100.0, 3, (2, 0)),
    ];
    let extra = vec![straight(0.0, 200.0, 300.0, 4, (0, 1))];
    let mut grown = Network {
        lanes: old.clone(),
        ..Default::default()
    };
    grown.link(1.5);
    let distant_reach = grown.reach[2];
    grown.extend(extra.clone(), 1.5);
    let mut whole = Network {
        lanes: old.into_iter().chain(extra).collect(),
        ..Default::default()
    };
    whole.link(1.5);
    assert_eq!(grown.reach, whole.reach);
    assert_eq!(grown.reach[2], distant_reach);
}

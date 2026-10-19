use glam::DVec3;
use omsi_sim::testing::traffic::{Lane, LaneBuilder};
use omsi_sim::testing::traffic::{LaneKind, Network};

fn straight(a: (f64, f64), b: (f64, f64)) -> Lane {
    LaneBuilder::polyline(
        vec![DVec3::new(a.0, a.1, 0.0), DVec3::new(b.0, b.1, 0.0)],
        LaneKind::Street,
        3.0,
    )
}

/// A square block: the route goes north then east; the bus turned west by mistake at
/// the first corner and must be led round the block back onto the route.
#[test]
fn a_way_back_joins_the_route_ahead() {
    let lanes = vec![
        straight((0.0, 0.0), (0.0, 100.0)),         // 0 north (route)
        straight((0.0, 100.0), (100.0, 100.0)),     // 1 east (route)
        straight((100.0, 100.0), (200.0, 100.0)),   // 2 east (route)
        straight((0.0, 100.0), (-100.0, 100.0)),    // 3 west (the mistake)
        straight((-100.0, 100.0), (-100.0, 200.0)), // 4 north
        straight((-100.0, 200.0), (100.0, 200.0)),  // 5 east
        straight((100.0, 200.0), (100.0, 100.0)),   // 6 south, back to the route's corner
    ];
    let mut net = Network {
        lanes,
        ..Default::default()
    };
    net.link(1.5);
    // (square corners: joined by hand, `link` wants a junction's curves)
    for (a, n) in [
        (0, vec![1, 3]),
        (1, vec![2]),
        (3, vec![4]),
        (4, vec![5]),
        (5, vec![6]),
        (6, vec![2]),
    ] {
        net.lanes[a].next = n;
    }
    let route = [0usize, 1, 2];
    let (path, join) = net
        .shortest_path_to_any(DVec3::new(-40.0, 100.0, 0.0), 270.0, &route[1..], 6000.0)
        .expect("a way");
    assert_eq!(path.first(), Some(&3));
    assert!(path.contains(&6), "{path:?}");
    // round the block back to the route's second corner: it goes on east on lane 2
    assert_eq!(route[1..][join], 2, "{path:?} join {join}");
}

use glam::DVec3;
use omsi_sim::traffic::{Lane, LaneBuilder, LaneKind, Network};
use openomsi_game::testing::app_events::street_limit;

fn road(a: (f64, f64), b: (f64, f64), limit: Option<f32>) -> Lane {
    let mut l = LaneBuilder::polyline(
        vec![DVec3::new(a.0, a.1, 0.0), DVec3::new(b.0, b.1, 0.0)],
        LaneKind::Street,
        3.0,
    );
    if let Some(v) = limit {
        l.speed_limit_kmh = v;
        l.limit_from_map = true;
    }
    l
}

fn net(lanes: Vec<Lane>) -> Network {
    let mut n = Network {
        lanes,
        ..Default::default()
    };
    n.build_grid();
    n
}

#[test]
fn the_limit_is_the_one_of_the_street_the_vehicle_drives_along() {
    // a 30 zone running north, crossed by a 50 road running east, the vehicle at the
    // crossing and a little off the east road's centre line, heading north
    let n = net(vec![
        road((0.0, -50.0), (0.0, 50.0), Some(30.0)),
        road((-50.0, 1.0), (50.0, 1.0), Some(50.0)),
    ]);
    assert_eq!(street_limit(&n, DVec3::new(0.5, 0.0, 0.0), 0.0), Some(30.0));
    // heading east it is the other street's
    assert_eq!(
        street_limit(&n, DVec3::new(0.5, 0.0, 0.0), 90.0),
        Some(50.0)
    );
}

#[test]
fn a_street_without_a_limit_from_the_map_shows_none() {
    let n = net(vec![road((0.0, -50.0), (0.0, 50.0), None)]);
    assert_eq!(street_limit(&n, DVec3::new(0.0, 0.0, 0.0), 0.0), None);
}

#[test]
fn off_the_streets_it_shows_none_and_backing_up_it_still_reads() {
    let n = net(vec![road((0.0, -50.0), (0.0, 50.0), Some(30.0))]);
    assert_eq!(street_limit(&n, DVec3::new(40.0, 0.0, 0.0), 0.0), None);
    // heading south on the northbound lane: reversing along it
    assert_eq!(
        street_limit(&n, DVec3::new(0.0, 0.0, 0.0), 180.0),
        Some(30.0)
    );
}

#[test]
fn the_limit_is_given_to_five() {
    let n = net(vec![road((0.0, -50.0), (0.0, 50.0), Some(48.0))]);
    assert_eq!(street_limit(&n, DVec3::ZERO, 0.0), Some(50.0));
}

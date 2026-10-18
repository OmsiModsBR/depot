use glam::DVec2;
use omsi_sim::testing::collision::*;

fn square(x: f64, y: f64, heading_deg: f64) -> Obb {
    Obb {
        center: DVec2::new(x, y),
        half: DVec2::new(1.0, 1.0),
        heading: heading_deg.to_radians(),
        z0: 0.0,
        z1: 1.0,
        velocity: DVec2::ZERO,
        mass: 0.0,
        pole: None,
        id: -1,
    }
}

#[test]
fn separation_of_boxes() {
    // side by side, a metre apart; overlapping by half a metre
    assert!((square(0.0, 0.0, 0.0).separation(&square(3.0, 0.0, 0.0)) - 1.0).abs() < 1e-9);
    assert!((square(0.0, 0.0, 0.0).separation(&square(1.5, 0.0, 0.0)) + 0.5).abs() < 1e-9);
    // a box turned by 45° points a corner at the other: the gap is from that corner
    let s = square(0.0, 0.0, 0.0).separation(&square(3.0, 0.0, 45.0));
    assert!((s - (2.0 - 2f64.sqrt())).abs() < 1e-9, "{s}");
    assert!((s - square(3.0, 0.0, 45.0).separation(&square(0.0, 0.0, 0.0))).abs() < 1e-12);
}

#[test]
fn vehicle_box_from_its_origin() {
    // a car facing east whose origin is 1 m behind its middle
    let b = Obb::vehicle(DVec2::new(10.0, 5.0), 90.0, 3.0, 1.0, 0.9);
    assert!((b.center - DVec2::new(11.0, 5.0)).length() < 1e-9);
    assert!((b.half - DVec2::new(0.9, 2.0)).length() < 1e-9);
    // its front bumper is at x = 13
    let wall = Obb {
        center: DVec2::new(14.0, 5.0),
        half: DVec2::new(5.0, 0.5),
        heading: 90f64.to_radians(),
        z0: 0.0,
        z1: 1.0,
        velocity: DVec2::ZERO,
        mass: 0.0,
        pole: None,
        id: -1,
    };
    assert!((b.separation(&wall) - 0.5).abs() < 1e-9);
}

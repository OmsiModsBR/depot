use glam::{DVec2, DVec3};
use omsi_sim::testing::collision::*;

fn bus_at(y: f64) -> Obb {
    // 2.5 m wide, 12 m long, heading north, centred at (0, y)
    Obb::from_box(
        [2.5, 12.0, 3.0, 0.0, 0.0, 1.5],
        DVec3::new(0.0, y, 0.0),
        0.0,
    )
}

#[test]
fn a_post_ahead_pushes_the_bus_back_not_sideways() {
    let post = Obb::from_box(
        [0.1, 0.1, 3.0, 0.0, 0.0, 1.5],
        DVec3::new(0.3, 6.0, 0.0),
        0.0,
    );
    // the bumper is 0.25 m past the post's near face
    let bus = bus_at(0.2);
    let c = bus.contact(&post).expect("touching");
    assert!((c.normal - DVec2::new(0.0, -1.0)).length() < 1e-9, "{c:?}");
    assert!((c.depth - 0.25).abs() < 1e-9, "{c:?}");
    assert!((c.point - DVec2::new(0.3, 6.0)).length() < 1e-6, "{c:?}");
}

#[test]
fn contact_point_far_from_the_map_origin() {
    let base = DVec3::new(892248.2, 4196461.4, 33.2);
    let post = Obb::from_box(
        [0.1, 0.1, 3.0, 0.0, 0.0, 1.5],
        base + DVec3::new(0.3, 6.0, 0.0),
        0.0,
    );
    let bus = Obb::from_box(
        [2.5, 12.0, 3.0, 0.0, 0.0, 1.5],
        base + DVec3::new(0.0, 0.2, 0.0),
        0.0,
    );
    let c = bus.contact(&post).expect("touching");
    assert!(
        (c.point - DVec2::new(base.x + 0.3, base.y + 6.0)).length() < 1e-3,
        "{c:?}"
    );
}

#[test]
fn glancing_contact_with_a_turned_wall() {
    // a wall along the east side, turned 10° so it meets the bus's front right corner
    let wall = Obb::from_box(
        [1.0, 20.0, 3.0, 0.0, 0.0, 1.5],
        DVec3::new(2.0, 0.0, 0.0),
        -10.0,
    );
    let bus = bus_at(0.0);
    let c = bus.contact(&wall).expect("touching");
    // pushed out to the west, a little to the north (the wall's own normal)
    assert!(c.normal.x < -0.9 && c.depth > 0.0 && c.depth < 1.0, "{c:?}");
    // the overlap is at the front right of the bus
    assert!(c.point.x > 0.5 && c.point.y > 2.0, "{c:?}");
}

#[test]
fn no_contact_under_a_high_or_low_box() {
    let sign = Obb::from_box(
        [2.0, 2.0, 1.0, 0.0, 0.0, 4.0],
        DVec3::new(0.0, 0.0, 0.0),
        0.0,
    );
    let low = Obb::from_box(
        [2.0, 2.0, 0.2, 0.0, 0.0, 0.1],
        DVec3::new(0.0, 0.0, 0.0),
        0.0,
    );
    let bus = Obb::from_box([2.5, 12.0, 3.0, 0.0, 0.0, 1.8], DVec3::ZERO, 0.0);
    assert!(bus.contact(&sign).is_none() && bus.contact(&low).is_none());
    assert!(bus.overlaps(&bus_at(1.0)));
}

use glam::{DVec2, DVec3};
use omsi_sim::testing::collision::*;
use std::sync::Arc;

/// The twelve triangles of a closed box from `lo` to `hi`.
fn cube(lo: DVec3, hi: DVec3) -> Vec<[DVec3; 3]> {
    let c = |i: u32| {
        DVec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        )
    };
    let quads = [
        [0, 1, 3, 2],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
        [0, 2, 6, 4],
        [1, 3, 7, 5],
    ];
    quads
        .iter()
        .flat_map(|q| [[c(q[0]), c(q[1]), c(q[2])], [c(q[0]), c(q[2]), c(q[3])]])
        .collect()
}

fn bus(x: f64, y: f64, z: f64) -> Obb {
    Obb::from_box([2.5, 12.0, 2.9, 0.0, 0.0, 1.75], DVec3::new(x, y, z), 0.0)
}

#[test]
fn near_lists_each_part_once_in_the_order_met() {
    let mut seed = 7u64;
    let mut rnd = |a: f64, b: f64| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        a + (b - a) * ((seed >> 11) as f64 / (1u64 << 53) as f64)
    };
    let mut tris = Vec::new();
    for _ in 0..400 {
        let (x, y, w, d) = (
            rnd(-30.0, 30.0),
            rnd(-30.0, 30.0),
            rnd(0.2, 14.0),
            rnd(0.2, 14.0),
        );
        tris.extend(cube(
            DVec3::new(x, y, 0.0),
            DVec3::new(x + w, y + d, rnd(1.0, 6.0)),
        ));
    }
    let shape = MeshShape::from_triangles(tris.into_iter(), 0.3);
    for _ in 0..300 {
        let (c, r) = (
            DVec2::new(rnd(-40.0, 40.0), rnd(-40.0, 40.0)),
            rnd(0.5, 25.0),
        );
        let mut want: Vec<u32> = Vec::new();
        for y in ((c.y - r) / PART_CELL).floor() as i32..=((c.y + r) / PART_CELL).floor() as i32 {
            for x in ((c.x - r) / PART_CELL).floor() as i32..=((c.x + r) / PART_CELL).floor() as i32
            {
                for &i in shape
                    .grid()
                    .get(&(x, y))
                    .map(|v| v.as_slice())
                    .unwrap_or(&[])
                {
                    let p = &shape.parts[i as usize];
                    if (p.center - c).length() <= r + p.radius() && !want.contains(&i) {
                        want.push(i);
                    }
                }
            }
        }
        assert_eq!(shape.near(c, r), want);
    }
}

#[test]
fn two_houses_leave_the_street_between_them_open() {
    // two houses 10 m apart in one mesh: its extents cover the street between them
    let mut tris = cube(DVec3::new(-20.0, -10.0, 0.0), DVec3::new(-5.0, 10.0, 12.0));
    tris.extend(cube(
        DVec3::new(5.0, -10.0, 0.0),
        DVec3::new(20.0, 10.0, 12.0),
    ));
    let shape = Arc::new(MeshShape::from_triangles(tris.into_iter(), 0.3));
    let base = DVec3::new(892248.2, 4196461.4, 33.2);
    let mut w = CollisionWorld::default();
    w.add_mesh(MeshObstacle::new(shape, base, 30.0, 7));
    let turn = |x: f64, y: f64| {
        let h = 30f64.to_radians();
        DVec3::new(
            base.x + x * h.cos() + y * h.sin(),
            base.y - x * h.sin() + y * h.cos(),
            base.z,
        )
    };
    let street = turn(0.0, 0.0);
    let mut b = bus(street.x, street.y, street.z);
    b.heading = 30f64.to_radians();
    assert!(w.hit(&b).is_none(), "the street between the houses is open");
    let wall = turn(4.0, 0.0);
    let mut b = bus(wall.x, wall.y, wall.z);
    b.heading = 30f64.to_radians();
    let hit = w.hit(&b).expect("the bus's right side is in the house");
    assert_eq!(hit.id, 7);
    let c = b.contact(&hit).expect("touching");
    // pushed back to the left, out of the east house
    let left = DVec2::new(-(30f64.to_radians().cos()), 30f64.to_radians().sin());
    assert!(c.normal.dot(left) > 0.99 && c.depth < 0.5, "{c:?}");
}

#[test]
fn a_bridge_deck_passes_over_the_bus_and_carries_it() {
    // a deck 5.5..6.5 m up, 20 m wide across the road, with a railing along each edge
    let mut tris = cube(DVec3::new(-50.0, -10.0, 5.5), DVec3::new(50.0, 10.0, 6.5));
    tris.extend(cube(
        DVec3::new(-50.0, -10.0, 6.5),
        DVec3::new(50.0, -9.8, 7.5),
    ));
    tris.extend(cube(
        DVec3::new(-50.0, 9.8, 6.5),
        DVec3::new(50.0, 10.0, 7.5),
    ));
    let shape = Arc::new(MeshShape::from_triangles(tris.into_iter(), 0.3));
    let mut w = CollisionWorld::default();
    w.add_mesh(MeshObstacle::new(shape, DVec3::ZERO, 0.0, 1));
    // the road below runs across the deck
    let mut under = bus(0.0, 0.0, 0.0);
    under.heading = 90f64.to_radians();
    assert!(w.hit(&under).is_none());
    // a bus on the deck, its floor clear of the plate, between the railings
    let mut on = bus(0.0, 0.0, 6.5);
    on.heading = 90f64.to_radians();
    assert!(w.hit(&on).is_none());
    // and one driving into the railing
    let mut into = bus(0.0, 8.8, 6.5);
    into.heading = 90f64.to_radians();
    assert!(w.hit(&into).is_some());
}

#[test]
fn a_pitched_roof_is_not_a_block() {
    // a roof ridge 8 m up over eaves at 4 m, the house 10 m deep: a bus may not stand in
    // the house but may stand next to it where the roof's extents reach
    let ridge = [DVec3::new(0.0, -10.0, 8.0), DVec3::new(0.0, 10.0, 8.0)];
    let eave = [DVec3::new(5.0, -10.0, 4.0), DVec3::new(5.0, 10.0, 4.0)];
    let tris = vec![[ridge[0], ridge[1], eave[1]], [ridge[0], eave[1], eave[0]]];
    let shape = MeshShape::from_triangles(tris.into_iter(), 0.3);
    assert!(shape.parts.len() > 2);
    let mut w = CollisionWorld::default();
    w.add_mesh(MeshObstacle::new(Arc::new(shape), DVec3::ZERO, 0.0, 1));
    // a 3 m tall bus 1 m over the ground under the eave's edge
    let b = Obb::from_box(
        [2.5, 12.0, 3.0, 0.0, 0.0, 2.5],
        DVec3::new(4.5, 0.0, 0.0),
        0.0,
    );
    assert!(w.hit(&b).is_none());
}

#[test]
fn camera_ray_meets_a_mesh_wall() {
    let shape = Arc::new(MeshShape::from_triangles(
        cube(DVec3::new(-10.0, -10.0, 0.0), DVec3::new(10.0, 10.0, 12.0)).into_iter(),
        0.3,
    ));
    let mut w = CollisionWorld::default();
    w.add_mesh(MeshObstacle::new(
        shape,
        DVec3::new(100.0, 0.0, 0.0),
        0.0,
        1,
    ));
    assert!(w.ray_blocked(
        DVec3::new(50.0, 0.0, 2.0),
        DVec3::new(150.0, 0.0, 2.0),
        2.5,
        3.0
    ));
    assert!(!w.ray_blocked(
        DVec3::new(50.0, 0.0, 20.0),
        DVec3::new(150.0, 0.0, 20.0),
        2.5,
        3.0
    ));
    assert!(!w.ray_blocked(
        DVec3::new(50.0, 20.0, 2.0),
        DVec3::new(150.0, 20.0, 2.0),
        2.5,
        3.0
    ));
}

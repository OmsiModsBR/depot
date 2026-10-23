use glam::{DVec2, Vec3};
use omsi_geometry::util::angle_diff;
use omsi_sim::testing::people::crowd::*;

fn walker(x: f64, y: f64, want: DVec2) -> Walker {
    Walker {
        pos: DVec2::new(x, y),
        vel: want,
        radius: 0.25,
        want,
        give: 1.0,
        space: 0,
        fixed: false,
        ghost: false,
        corridor: None,
    }
}

#[test]
fn head_on_walkers_pass_without_touching() {
    let mut w = vec![
        walker(0.0, -5.0, DVec2::new(0.0, 1.3)),
        walker(0.05, 5.0, DVec2::new(0.0, -1.3)),
    ];
    let p = CrowdParams::default();
    let mut closest = f64::MAX;
    for _ in 0..300 {
        w[0].want = DVec2::new(0.0, 1.3);
        w[1].want = DVec2::new(0.0, -1.3);
        step(&mut w, &[], &p, 1.0 / 30.0);
        closest = closest.min((w[0].pos - w[1].pos).length());
    }
    assert!(closest >= 0.5 - 1e-6, "closest approach {closest}");
    // both got past each other and kept going
    assert!(
        w[0].pos.y > 3.0 && w[1].pos.y < -3.0,
        "{:?} {:?}",
        w[0].pos,
        w[1].pos
    );
    // and they did not swerve wildly
    assert!(w[0].pos.x.abs() < 1.5 && w[1].pos.x.abs() < 1.5);
}

#[test]
fn walker_goes_round_a_standing_person() {
    let mut w = vec![
        walker(0.0, -4.0, DVec2::new(0.0, 1.2)),
        Walker {
            give: 0.3,
            ..walker(0.0, 0.0, DVec2::ZERO)
        },
    ];
    let p = CrowdParams::default();
    let mut closest = f64::MAX;
    for _ in 0..300 {
        w[0].want = DVec2::new(0.0, 1.2);
        w[1].want = -w[1].pos * 1.5; // keeps to its spot
        step(&mut w, &[], &p, 1.0 / 30.0);
        closest = closest.min((w[0].pos - w[1].pos).length());
    }
    assert!(closest >= 0.5 - 1e-6);
    assert!(w[0].pos.y > 3.0);
    assert!(
        w[1].pos.length() < 0.4,
        "the standing person drifted to {:?}",
        w[1].pos
    );
}

#[test]
fn crowd_never_interpenetrates() {
    // twelve people walking to the same point from a ring
    let mut w: Vec<Walker> = (0..12)
        .map(|k| {
            let a = k as f64 / 12.0 * std::f64::consts::TAU;
            walker(a.cos() * 5.0, a.sin() * 5.0, DVec2::ZERO)
        })
        .collect();
    let p = CrowdParams::default();
    for _ in 0..600 {
        for x in w.iter_mut() {
            let d = -x.pos;
            x.want = if d.length() > 0.1 {
                d.normalize() * 1.2
            } else {
                DVec2::ZERO
            };
        }
        step(&mut w, &[], &p, 1.0 / 30.0);
    }
    for i in 0..w.len() {
        for j in i + 1..w.len() {
            let d = (w[i].pos - w[j].pos).length();
            assert!(d > 0.45, "{i} and {j} overlap: {d}");
        }
    }
}

#[test]
fn velocity_changes_smoothly() {
    // a walker told to reverse at once does not flip its velocity in one frame
    let mut w = vec![walker(0.0, 0.0, DVec2::new(0.0, 1.3))];
    let p = CrowdParams::default();
    w[0].want = DVec2::new(0.0, -1.3);
    let dt = 1.0 / 30.0;
    let before = w[0].vel;
    step(&mut w, &[], &p, dt);
    assert!(
        (w[0].vel - before).length() <= p.max_accel * dt + 1e-9,
        "{:?}",
        w[0].vel
    );
}

#[test]
fn nobody_walks_through_a_bus() {
    let bus = Block {
        center: DVec2::ZERO,
        half: DVec2::new(1.25, 6.0),
        heading: 0.0,
        vel: DVec2::ZERO,
    };
    let mut w = vec![walker(-4.0, 0.3, DVec2::new(1.3, 0.0))];
    let p = CrowdParams::default();
    for _ in 0..400 {
        let d = DVec2::new(4.0, 0.0) - w[0].pos;
        w[0].want = if d.length() > 0.1 {
            d.normalize() * 1.3
        } else {
            DVec2::ZERO
        };
        step(&mut w, &[bus], &p, 1.0 / 30.0);
        let (_, inside) = bus.closest(w[0].pos);
        assert!(!inside, "inside the bus at {:?}", w[0].pos);
    }
}

#[test]
fn ghosts_pass_through_each_other() {
    // two people pressed against each other in a doorway: as ghosts they get past
    let mut w = vec![
        Walker {
            ghost: true,
            ..walker(0.0, -0.2, DVec2::new(0.0, 1.0))
        },
        Walker {
            ghost: true,
            ..walker(0.0, 0.2, DVec2::new(0.0, -1.0))
        },
    ];
    let p = CrowdParams::cabin();
    for _ in 0..60 {
        w[0].want = DVec2::new(0.0, 1.0);
        w[1].want = DVec2::new(0.0, -1.0);
        step(&mut w, &[], &p, 1.0 / 30.0);
    }
    assert!(
        w[0].pos.y > 1.0 && w[1].pos.y < -1.0,
        "{:?} {:?}",
        w[0].pos,
        w[1].pos
    );
}

#[test]
fn corridor_clamp_keeps_people_in_the_aisle() {
    let p = clamp_to_corridor(
        DVec2::new(1.0, 2.0),
        DVec2::new(0.0, 0.0),
        DVec2::new(0.0, 5.0),
        0.3,
    );
    assert!((p - DVec2::new(0.3, 2.0)).length() < 1e-9, "{p:?}");
    let q = clamp_to_corridor(
        DVec2::new(0.1, 2.0),
        DVec2::new(0.0, 0.0),
        DVec2::new(0.0, 5.0),
        0.3,
    );
    assert!((q - DVec2::new(0.1, 2.0)).length() < 1e-9);
}

#[test]
fn cabin_route_follows_one_way_links() {
    // 0-1-2 two-way, 2->3 one-way, 3->0 one-way
    let pts = vec![
        Vec3::new(0.0, 0.0, 0.5),
        Vec3::new(0.0, -2.0, 0.5),
        Vec3::new(0.0, -4.0, 0.5),
        Vec3::new(0.0, -4.0, 2.5),
    ];
    let g = PathGraph::new(
        pts,
        &[(0, 1, false), (1, 2, false), (2, 3, true), (3, 0, true)],
    );
    assert_eq!(g.route_points(0, 3), Some(vec![0, 1, 2, 3]));
    assert_eq!(g.route_points(3, 2), Some(vec![3, 0, 1, 2]));
    let r = g.route(Vec3::new(0.0, 0.1, 0.5), Vec3::new(0.5, -2.0, 0.9));
    assert_eq!(r.last().copied(), Some(Vec3::new(0.5, -2.0, 0.9)));
    assert!(r.contains(&Vec3::new(0.0, -2.0, 0.5)));
    assert!((g.distance(0, 2) - 4.0).abs() < 1e-5);
    assert!(g.distance(0, 3) > 5.9 && g.distance(0, 3) < 6.1);
}

#[test]
fn heading_turns_smoothly_the_short_way() {
    let mut h = 350.0;
    for _ in 0..30 {
        h = turn_towards(h, 20.0, 180.0, 1.0 / 30.0);
    }
    assert!((h - 20.0).abs() < 3.0 || (h - 20.0).abs() > 357.0, "{h}");
    let h1 = turn_towards(0.0, 180.0, 90.0, 0.1);
    assert!((h1 - 9.0).abs() < 1e-6 || (h1 - 351.0).abs() < 1e-6, "{h1}");
    assert!((angle_diff(350.0, 10.0) - 20.0).abs() < 1e-9);
    assert!((heading_of(DVec2::new(1.0, 0.0)) - 90.0).abs() < 1e-9);
}

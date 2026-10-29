use glam::DVec3;
use glam::Vec3;
use openomsi_game::testing::output::camera::arm::*;

fn run(arm: &mut SpringArm, want: f32, free: f32, secs: f32) -> f32 {
    let mut l = 0.0;
    for _ in 0..(secs * 60.0) as usize {
        l = arm.update(want, free, DVec3::ZERO, 1.0 / 60.0);
    }
    l
}

#[test]
fn snaps_in_and_eases_out() {
    let mut arm = SpringArm::default();
    assert_eq!(run(&mut arm, 10.0, 10.0, 0.5), 10.0);
    // a wall at 4 m: in within a moment, not in one frame
    let first = arm.update(10.0, 4.0, DVec3::ZERO, 1.0 / 60.0);
    assert!(first > 4.0 && first < 10.0, "{first}");
    assert_eq!(run(&mut arm, 10.0, 4.0, 0.5), 4.0);
    // clear again: still in during the hold, then out gradually, never past the target
    let held = run(&mut arm, 10.0, 10.0, 0.2);
    assert!((held - 4.0).abs() < 1e-4, "{held}");
    let a = run(&mut arm, 10.0, 10.0, 0.4);
    assert!(a > 4.5 && a < 9.5, "{a}");
    let b = run(&mut arm, 10.0, 10.0, 2.0);
    assert!(b > 9.9 && b <= 10.0, "{b}");
    // and it moves out monotonically, without overshoot
    let mut arm = SpringArm::default();
    run(&mut arm, 10.0, 3.0, 0.5);
    let mut last = 3.0;
    for _ in 0..180 {
        let l = arm.update(10.0, 10.0, DVec3::ZERO, 1.0 / 60.0);
        assert!(l >= last && l <= 10.0);
        last = l;
    }
}

#[test]
fn a_flickering_gap_does_not_pump() {
    // the way is clear every other frame (an edge swept past): the arm stays in
    let mut arm = SpringArm::default();
    run(&mut arm, 10.0, 10.0, 0.2);
    let mut max = 0.0f32;
    for k in 0..120 {
        let free = if k % 2 == 0 { 5.0 } else { 10.0 };
        let l = arm.update(10.0, free, DVec3::ZERO, 1.0 / 60.0);
        if k > 40 {
            max = max.max(l);
        }
    }
    assert!(max <= 5.0 + 1e-4, "{max}");
}

#[test]
fn zoom_is_direct_and_a_jump_resets() {
    let mut arm = SpringArm::default();
    run(&mut arm, 10.0, 10.0, 0.2);
    assert_eq!(arm.update(12.0, 12.0, DVec3::ZERO, 1.0 / 60.0), 12.0);
    run(&mut arm, 12.0, 3.0, 0.2);
    // the bus was put 100 m away with the way clear: no easing out from 3 m
    assert_eq!(
        arm.update(12.0, 12.0, DVec3::new(100.0, 0.0, 0.0), 1.0 / 60.0),
        12.0
    );
}

#[test]
fn the_tree_finds_the_nearest_triangle() {
    // a wall of quads along y, every metre from 2 to 40, each 2 m wide and high
    let mut tris = Vec::new();
    for k in 0..39 {
        let y = 2.0 + k as f32;
        let (a, b, c, d) = (
            Vec3::new(-1.0, y, -1.0),
            Vec3::new(1.0, y, -1.0),
            Vec3::new(1.0, y, 1.0),
            Vec3::new(-1.0, y, 1.0),
        );
        tris.push([a, b, c]);
        tris.push([a, c, d]);
    }
    let bvh = TriBvh::build(tris.clone());
    assert!(bvh.nodes.len() > 1);
    for (o, dir) in [
        (Vec3::ZERO, Vec3::Y),
        (Vec3::new(0.3, 10.5, 0.2), Vec3::Y),
        (Vec3::new(0.5, 50.0, 0.0), -Vec3::Y),
        (
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.05, 1.0, 0.02).normalize(),
        ),
    ] {
        let brute = tris
            .iter()
            .filter_map(|t| ray_triangle(o, dir, t[0], t[1], t[2]))
            .fold(f32::MAX, f32::min);
        let found = bvh.ray(o, dir, 100.0).unwrap();
        assert!(
            (found - brute).abs() < 1e-5,
            "{o:?} {dir:?}: {found} vs {brute}"
        );
    }
    // nothing nearer than the limit, nothing off to the side
    assert_eq!(bvh.ray(Vec3::ZERO, Vec3::Y, 1.5), None);
    assert_eq!(bvh.ray(Vec3::new(3.0, 0.0, 0.0), Vec3::Y, 100.0), None);
}

#[test]
fn rays_meet_triangles_from_both_sides() {
    let (a, b, c) = (
        Vec3::new(-1.0, 5.0, -1.0),
        Vec3::new(1.0, 5.0, -1.0),
        Vec3::new(0.0, 5.0, 1.0),
    );
    let t = ray_triangle(Vec3::ZERO, Vec3::Y, a, b, c).unwrap();
    assert!((t - 5.0).abs() < 1e-5);
    assert!(ray_triangle(Vec3::new(0.0, 10.0, 0.0), -Vec3::Y, a, b, c).is_some());
    assert!(ray_triangle(Vec3::new(3.0, 0.0, 0.0), Vec3::Y, a, b, c).is_none());
    assert!(ray_triangle(Vec3::ZERO, -Vec3::Y, a, b, c).is_none());
    assert_eq!(
        slab(
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(2.0, -1.0, -1.0),
            Vec3::new(3.0, 1.0, 1.0)
        ),
        Some((2.0, 3.0))
    );
    assert_eq!(
        slab(
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(2.0, 1.5, -1.0),
            Vec3::new(3.0, 2.0, 1.0)
        ),
        None
    );
}

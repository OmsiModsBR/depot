use glam::DVec3;
use openomsi_game::testing::player::orbit_pivot;

#[test]
fn pivot_yaws_with_the_bus_and_ignores_body_attitude() {
    // heading 0: the centre passes through unrotated.
    let p = orbit_pivot(DVec3::new(10.0, 20.0, 5.0), 0.0, [0.0, 0.0, 1.2]);
    assert!((p.x - 10.0).abs() < 1e-6 && (p.y - 20.0).abs() < 1e-6);
    assert!((p.z - 6.2).abs() < 1e-6, "{p:?}");
    // heading 90: the forward offset swings sideways, height untouched
    // (a body_rotation pivot would also tilt it with pitch/bank).
    let q = orbit_pivot(DVec3::ZERO, 90.0, [1.0, 2.0, 1.2]);
    assert!(
        (q.x - 2.0).abs() < 1e-5 && (q.y + 1.0).abs() < 1e-5,
        "{q:?}"
    );
    assert!((q.z - 1.2).abs() < 1e-6, "{q:?}");
}

use glam::DVec3;
use omsi_sim::testing::collision::*;

#[test]
fn a_bus_drives_up_a_mesh_ramp() {
    // the Heerstraße bridge's ramp: 70 m long, rising 6.95 m, 3.6 m wide
    let (a, b, c, d) = (
        DVec3::new(-1.8, 0.0, 0.0),
        DVec3::new(1.8, 0.0, 0.0),
        DVec3::new(1.8, 70.0, 6.95),
        DVec3::new(-1.8, 70.0, 6.95),
    );
    let shape = MeshShape::from_triangles([[a, b, c], [a, c, d]].into_iter(), 0.3);
    let mut w = CollisionWorld::default();
    w.add_mesh(MeshObstacle::new(
        std::sync::Arc::new(shape),
        DVec3::ZERO,
        0.0,
        1,
    ));
    // a 12 m bus standing on the ramp (pitched with it), its floor 0.35 m over it
    for y in [8.0, 20.0, 40.0, 63.0] {
        let z = y * 6.95 / 70.0;
        let pitch = (6.95f64 / 70.0).atan();
        let mut bus = Obb::from_box(
            [2.5, 12.0, 2.85, 0.0, 0.0, 1.775],
            DVec3::new(0.0, y, z),
            0.0,
        );
        // the pitched box's lowest corner is at the rear: 6 m back, 0.35 m up
        bus.z0 = z - 6.0 * pitch.sin() + 0.35 * pitch.cos();
        // which is all the plan-view box knows: it is in the ramp ahead
        assert!(w.hit(&bus).is_some());
        let (up, fwd) = (
            DVec3::new(0.0, -pitch.sin(), pitch.cos()),
            DVec3::new(0.0, pitch.cos(), pitch.sin()),
        );
        let solid = Box3 {
            center: DVec3::new(0.0, y, z) + up * 1.775,
            axes: [DVec3::X, fwd, up],
            half: DVec3::new(1.25, 6.0, 1.425),
        };
        let faces = w.obstacles_near_solid(&bus, Some(&solid));
        assert!(faces.iter().all(|f| !f.overlaps(&bus)), "at {y}: {faces:?}");
    }
    // but a bus level with the foot of the ramp, 10 m up it, is in it
    let bus = Obb::from_box(
        [2.5, 12.0, 2.85, 0.0, 0.0, 1.775],
        DVec3::new(0.0, 10.0, 0.0),
        0.0,
    );
    assert!(w.hit(&bus).is_some());
}

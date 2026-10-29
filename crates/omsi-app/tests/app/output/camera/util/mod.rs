mod outside_fov;

use glam::{DVec3, Mat4};

/// A mirror's camera looks along the eye's ray reflected in its face; a static one looks
/// where it is pointed, wherever the eye is, turned by the player's offset.
#[test]
fn a_static_camera_does_not_follow_the_eye() {
    let mirror = omsi_vehicle::Camera {
        pos: [1.2, 5.0, 2.0],
        fov: 30.0,
        yaw: 180.0,
        ..Default::default()
    };
    let cctv = omsi_vehicle::Camera {
        fixed: true,
        yaw: 90.0,
        pitch: -40.0,
        ..mirror.clone()
    };
    let views = |c: &omsi_vehicle::Camera| {
        [DVec3::new(0.0, 4.0, 2.0), DVec3::new(-0.5, 3.0, 2.2)]
            .map(|eye| {
                openomsi_game::testing::output::camera::util::mirror_view_in(
                    DVec3::ZERO,
                    Mat4::IDENTITY,
                    c,
                    eye,
                    [0.0; 2],
                )
            })
            .map(|c| (c.yaw, c.pitch))
    };
    let [a, b] = views(&mirror);
    assert!((a.0 - b.0).abs() > 1.0, "{a:?} {b:?}");
    assert_eq!(views(&cctv), [(90.0, -40.0); 2]);
    let turned = openomsi_game::testing::output::camera::util::mirror_view_in(
        DVec3::ZERO,
        Mat4::IDENTITY,
        &cctv,
        DVec3::ZERO,
        [5.0, -60.0],
    );
    assert_eq!((turned.yaw, turned.pitch), (95.0, -89.0));
}

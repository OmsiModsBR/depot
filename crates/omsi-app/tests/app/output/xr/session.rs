use glam::{Mat4, Vec3};
use openomsi_game::testing::output::xr::session::*;
use openxr as xr;

#[test]
fn stereo_disparity_stays_stable_through_zoom_animation() {
    let left_eye = Vec3::new(-0.032, 0.0, 0.0);
    let right_eye = Vec3::new(0.032, 0.0, 0.0);
    let left = eye_projection(
        xr::Fovf {
            angle_left: -0.9,
            angle_right: 0.8,
            angle_up: 0.8,
            angle_down: -0.8,
        },
        0.05,
        1000.0,
    );
    let right = eye_projection(
        xr::Fovf {
            angle_left: -0.8,
            angle_right: 0.9,
            angle_up: 0.8,
            angle_down: -0.8,
        },
        0.05,
        1000.0,
    );
    let screen_x = |eye: Vec3, projection: Mat4, point: Vec3| {
        let clip = projection * (point - eye).extend(1.0);
        clip.x / clip.w
    };
    for depth in [0.8, 2.0, 10.0] {
        let point = Vec3::new(0.12, 0.0, -depth);
        let normal = screen_x(left_eye, left, point) - screen_x(right_eye, right, point);
        for zoom in [1.0, 1.1, 1.3, 1.45, 1.6] {
            let magnified = screen_x(
                zoom_eye_position(left_eye, Vec3::ZERO, zoom),
                zoom_projection(left, zoom),
                point,
            ) - screen_x(
                zoom_eye_position(right_eye, Vec3::ZERO, zoom),
                zoom_projection(right, zoom),
                point,
            );
            assert!(
                (magnified - normal).abs() < 1e-5,
                "depth {depth}, zoom {zoom}: {magnified} instead of {normal}"
            );
        }
    }
}

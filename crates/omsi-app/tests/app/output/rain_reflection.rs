//! The street the puddles mirror under "DirectX9 (D3D9Ex)" (spec 001, T016): drawn from the
//! window's camera mirrored under the road, into a small picture of fixed size, while the
//! road is wet and the option is on.

use glam::{DVec3, Vec4Swizzles};
use omsi_render::{Camera, Lighting};
use openomsi_game::testing::output::rain_reflection::*;

fn view() -> Camera {
    Camera {
        position: DVec3::new(120.0, -40.0, 3.5),
        yaw: 30.0,
        pitch: -12.0,
        roll: 0.0,
        fov_deg: 60.0,
        near: 0.3,
        far: 5000.0,
    }
}

fn wet(ground: Option<f64>) -> Lighting {
    Lighting {
        wetness: 0.8,
        puddle_ground: ground,
        shadows: true,
        ..Default::default()
    }
}

/// Where the camera puts a point (normalised device coordinates, x right and y up).
fn on_screen(cam: &Camera, p: DVec3) -> (f32, f32) {
    let origin = DVec3::new(100.0, -50.0, 0.0);
    let clip = cam.view_proj(16.0 / 9.0, origin) * (p - origin).as_vec3().extend(1.0);
    assert!(clip.w > 0.0, "{p} is behind the camera");
    let ndc = clip.xy() / clip.w;
    (ndc.x, ndc.y)
}

#[test]
fn the_mirrored_camera_sees_upside_down_what_the_eye_sees_in_the_water() {
    let ground = 1.25;
    let cam = view();
    let mirrored = mirrored_camera(&cam, ground);
    assert!((mirrored.position.z - (2.0 * ground - cam.position.z)).abs() < 1e-9);
    assert_eq!(mirrored.pitch, -cam.pitch);
    // a lamp post's top ahead, and its image under the water's surface
    let forward = cam.forward().as_dvec3();
    let top = cam.position + forward * 30.0 + DVec3::new(0.0, 0.0, 9.0);
    let image = DVec3::new(top.x, top.y, 2.0 * ground - top.z);
    let (x, y) = on_screen(&mirrored, top);
    let (ix, iy) = on_screen(&cam, image);
    assert!(
        (x - ix).abs() < 1e-4 && (y + iy).abs() < 1e-4,
        "{x},{y} vs {ix},{iy}"
    );
    // so a spot of the water shows, from the mirrored camera, what the eye sees reflected there
    let spot = cam.position + forward * 20.0;
    let spot = DVec3::new(spot.x, spot.y, ground);
    let (sx, sy) = on_screen(&mirrored, spot);
    let (ex, ey) = on_screen(&cam, spot);
    assert!((sx - ex).abs() < 1e-4 && (sy + ey).abs() < 1e-4);
}

#[test]
fn the_picture_is_wanted_on_a_wet_road_with_the_option_on_only() {
    let cam = view();
    assert!(rain_reflection_camera(Some(true), &wet(Some(0.0)), &cam).is_some());
    assert!(rain_reflection_camera(Some(false), &wet(Some(0.0)), &cam).is_some());
    // the option off (or not DirectX9), the height of the road not known
    assert!(rain_reflection_camera(None, &wet(Some(0.0)), &cam).is_none());
    assert!(rain_reflection_camera(Some(true), &wet(None), &cam).is_none());
    // a dry road, and one under snow
    let dry = Lighting {
        wetness: 0.0,
        ..wet(Some(0.0))
    };
    assert!(rain_reflection_camera(Some(true), &dry, &cam).is_none());
    let snowed = Lighting {
        snow: 1.0,
        ..wet(Some(0.0))
    };
    assert!(rain_reflection_camera(Some(true), &snowed, &cam).is_none());
}

#[test]
fn the_picture_leaves_out_the_ground_the_shadows_and_the_people_when_asked() {
    assert_eq!(RAIN_REFLECTION_SIZE, (128, 256));
    let window = wet(Some(0.0));
    for humans in [true, false] {
        let l = rain_reflection_lighting(&window, humans);
        assert!(l.ground_hidden && !l.shadows);
        assert_eq!(l.humans_hidden, !humans);
        assert!(l.min_obj_size >= window.min_obj_size);
    }
    assert!(!window.ground_hidden && !window.humans_hidden);
}

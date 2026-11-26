//! The wet road's inputs, worked out without a device (`omsi_render_vanilla::wet_road`).

use omsi_render_vanilla::scene::MATERIAL_VECTORS;
use omsi_render_vanilla::wet_road::{sheen, wetness, wets};
use omsi_render_vanilla::{DrawKind, SceneLight, SceneMaterial};

fn rain(wet: f32, snow: f32) -> SceneLight {
    SceneLight {
        ambient: [0.3, 0.3, 0.3, snow],
        weather: [wet, 0.0, 0.0, 0.0],
        ..Default::default()
    }
}

fn material(moisture: f32) -> [[f32; 4]; MATERIAL_VECTORS] {
    let mut u = [[0.0; 4]; MATERIAL_VECTORS];
    u[3][2] = moisture;
    u
}

#[test]
fn the_road_is_as_wet_as_the_weather_says_and_dry_under_snow() {
    assert_eq!(wetness(&rain(0.0, 0.0)), 0.0);
    assert_eq!(wetness(&rain(0.6, 0.0)), 0.6);
    assert_eq!(wetness(&rain(1.7, 0.0)), 1.0);
    // (snow covers the road: no water on it)
    assert_eq!(wetness(&rain(1.0, 1.0)), 0.0);
    assert_eq!(wetness(&rain(1.0, 0.25)), 0.75);
}

#[test]
fn the_sheen_is_half_the_light_from_above_and_a_third_of_the_suns() {
    let light = SceneLight {
        sky_color: [0.4, 0.5, 0.6, 1.0],
        sun_color: [1.0, 0.8, 0.6, 0.0],
        sun_dir: [0.0, 0.0, 1.0, 0.5],
        ..Default::default()
    };
    let s = sheen(&light);
    let want = [0.2 + 0.175, 0.25 + 0.14, 0.3 + 0.105];
    for c in 0..3 {
        assert!((s[c] - want[c]).abs() < 1e-6, "{s:?} against {want:?}");
    }
}

#[test]
fn only_the_colour_of_a_surface_with_moisture_gets_wet() {
    let wet = material(1.0);
    let dry = material(0.0);
    let m = |u| SceneMaterial {
        uniform: u,
        ..Default::default()
    };
    for kind in [DrawKind::Opaque, DrawKind::AlphaTest, DrawKind::Blend] {
        assert!(wets(kind, &m(&wet)), "{kind:?}");
        assert!(!wets(kind, &m(&dry)), "{kind:?}");
    }
    // the depth-only part, a painted ground layer and a pane without depth are left alone
    for kind in [
        DrawKind::SurfaceDepth,
        DrawKind::TerrainPaint,
        DrawKind::BlendNoWrite,
    ] {
        assert!(!wets(kind, &m(&wet)), "{kind:?}");
    }
}

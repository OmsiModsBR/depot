//! Vanilla+'s moonlight (`Lighting::moonlight`): how much the moon adds to the night's
//! ambient light, by its phase, its height, the daylight, the clouds and the graphics mode.

use glam::Vec3;
use omsi_render::Lighting;

/// A clear night with the moon `illum` lit at `altitude` degrees, in Vanilla+.
fn night(illum: f32, altitude: f32) -> Lighting {
    let h = altitude.to_radians();
    Lighting {
        moon_dir: Vec3::new(0.0, h.cos(), h.sin()),
        moon_illum: illum,
        night: 1.0,
        cloud_density: 0.0,
        classic: false,
        enhanced: false,
        ..Default::default()
    }
}

fn level(l: &Lighting) -> f32 {
    let m = l.moonlight();
    (m.x + m.y + m.z) / 3.0
}

#[test]
fn a_full_moon_high_in_a_clear_night_lights_the_ambient_with_a_cool_grey() {
    let m = night(1.0, 60.0).moonlight();
    assert!(m.z > m.x && m.x > 0.03, "{m}");
}

#[test]
fn the_phase_counts_much_more_than_the_share_of_the_disc_lit() {
    let full = level(&night(1.0, 60.0));
    let half = level(&night(0.5, 60.0));
    assert!(half > 0.0 && half < full * 0.15, "half {half} full {full}");
    assert_eq!(level(&night(0.0, 60.0)), 0.0);
}

#[test]
fn a_low_moon_gives_less_and_one_below_the_horizon_none() {
    let high = level(&night(1.0, 60.0));
    let low = level(&night(1.0, 5.0));
    assert!(low > 0.0 && low < high, "low {low} high {high}");
    assert_eq!(level(&night(1.0, -5.0)), 0.0);
}

#[test]
fn daylight_and_clouds_hold_it_back() {
    let clear = level(&night(1.0, 60.0));
    let day = Lighting {
        night: 0.0,
        ..night(1.0, 60.0)
    };
    assert_eq!(level(&day), 0.0);
    let overcast = Lighting {
        cloud_density: 1.0,
        ..night(1.0, 60.0)
    };
    let o = level(&overcast);
    assert!(o > 0.0 && o < clear * 0.25, "overcast {o} clear {clear}");
}

#[test]
fn vanilla_and_enhanced_add_no_moonlight() {
    let vanilla = Lighting {
        classic: true,
        ..night(1.0, 60.0)
    };
    assert_eq!(vanilla.moonlight(), Vec3::ZERO);
    let enhanced = Lighting {
        enhanced: true,
        ..night(1.0, 60.0)
    };
    assert_eq!(enhanced.moonlight(), Vec3::ZERO);
}

/// The middle of a white wall seen at night in Vanilla+, with the moon `illum` lit high in
/// the sky (the wall's brightness, mean of r, g, b).
fn wall_at_night(illum: f32) -> u32 {
    use glam::{DVec3, Mat4, Vec2};
    use omsi_geometry::MeshData;
    use omsi_render::{AlphaMode, Camera, RenderOptions, Renderer};
    const SIZE: u32 = 32;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let mut renderer = pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            ssao: false,
            shadow_size: 1024,
            fxaa: false,
            render_scale: 1.0,
            ..Default::default()
        },
    ))
    .expect("test renderer");
    let mut scene = renderer.new_scene();
    let (y, half) = (10.0, 20.0);
    let wall = renderer.add_mesh(
        &mut scene,
        &MeshData {
            positions: vec![
                Vec3::new(-half, y, -half),
                Vec3::new(half, y, -half),
                Vec3::new(half, y, half),
                Vec3::new(-half, y, half),
            ],
            normals: vec![-Vec3::Y; 4],
            uvs: vec![Vec2::ZERO; 4],
            ranges: vec![(0, 6, 0)],
            indices: vec![0, 1, 2, 0, 2, 3],
            one_sided: false,
        },
    );
    let white = renderer.add_material(
        &mut scene,
        None,
        AlphaMode::Opaque,
        [1.0, 1.0, 1.0, 1.0],
        false,
    );
    renderer.add_instance(&mut scene, wall, DVec3::ZERO, Mat4::IDENTITY, vec![white]);
    let camera = Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 40.0,
        near: 0.1,
        far: 100.0,
    };
    let lighting = Lighting {
        shadows: false,
        fog_density: 0.0,
        sun_dir: Vec3::new(0.0, -0.5, -0.8).normalize(),
        sun_intensity: 0.0,
        sun_color: Vec3::ZERO,
        secondary: Vec3::ZERO,
        ambient: Vec3::splat(0.05),
        ..night(illum, 60.0)
    };
    let rgba = renderer
        .render_to_image(&mut scene, SIZE, SIZE, &camera, &lighting)
        .unwrap();
    let at = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
    (rgba[at] as u32 + rgba[at + 1] as u32 + rgba[at + 2] as u32) / 3
}

#[test]
#[ignore = "needs a graphics adapter"]
fn a_full_moon_brightens_a_wall_at_night_and_a_new_moon_does_not() {
    let (new, full) = (wall_at_night(0.0), wall_at_night(1.0));
    println!("wall at night: new moon {new}, full moon {full}");
    assert!(full > new + 10, "new {new} full {full}");
}

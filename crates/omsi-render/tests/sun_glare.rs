//! The sun's glare in the classic pictures (`Lighting::sun_glare`): a lamp's glow and star
//! laid out towards the sun, its strength by the daylight, the horizon, the clouds and the
//! fog, and what it does to the picture.

use glam::{DVec3, Vec3};
use omsi_render::{Lighting, SUN_GLARE_BRIGHTNESS, SUN_GLARE_DISTANCE, SUN_GLARE_SIZE};

/// A unit vector at this azimuth (degrees, clockwise from north) and altitude (degrees).
fn towards(azimuth: f32, altitude: f32) -> Vec3 {
    let (a, h) = (azimuth.to_radians(), altitude.to_radians());
    Vec3::new(h.cos() * a.sin(), h.cos() * a.cos(), h.sin())
}

/// A clear day with the sun at `altitude` degrees in the north, the glare on, in Vanilla+.
fn day(altitude: f32) -> Lighting {
    Lighting {
        sun_dir: towards(0.0, altitude),
        sun_intensity: 1.0,
        sun_color: Vec3::ONE,
        night: 0.0,
        cloud_density: 0.0,
        fog_density: 0.0,
        sun_glow: true,
        classic: false,
        enhanced: false,
        ..Default::default()
    }
}

#[test]
fn a_high_sun_in_a_clear_sky_gives_the_full_glare() {
    let l = day(40.0);
    assert!((l.sun_glare_strength() - 1.0).abs() < 1e-6);
    let eye = DVec3::new(100.0, 200.0, 3.0);
    let g = l.sun_glare(eye, 4, 5);
    assert_eq!(g.len(), 2);
    let (glow, star) = (&g[0], &g[1]);
    // a lamp a few metres towards the sun, both turned to the viewer at the sky's depth
    let at = eye + l.sun_dir.as_dvec3() * SUN_GLARE_DISTANCE;
    assert!((glow.position - at).length() < 1e-5 && glow.position == star.position);
    assert!(glow.sky && star.sky && glow.rotating == 2 && star.rotating == 2);
    // the glow as wide as the lamp's size, the star 2.5 times that and growing
    assert!((glow.size - SUN_GLARE_SIZE * 0.5).abs() < 1e-6);
    assert!((star.size - SUN_GLARE_SIZE * 1.25).abs() < 1e-6 && star.flags & 8 != 0);
    assert_eq!((glow.texture, star.texture), (4, 5));
    assert!((glow.brightness - SUN_GLARE_BRIGHTNESS).abs() < 1e-6);
}

#[test]
fn the_glare_fades_with_the_horizon_the_night_the_clouds_and_the_fog() {
    let full = day(40.0).sun_glare_strength();
    let low = day(0.0).sun_glare_strength();
    assert!(low > 0.0 && low < full, "low {low}");
    assert_eq!(day(-5.0).sun_glare_strength(), 0.0);
    let night = Lighting {
        night: 1.0,
        ..day(40.0)
    };
    assert_eq!(night.sun_glare_strength(), 0.0);
    let cloudy = Lighting {
        cloud_density: 0.7,
        ..day(40.0)
    };
    let c = cloudy.sun_glare_strength();
    assert!(c > 0.0 && c < 0.7, "cloudy {c}");
    // a closed deck hides it even when the light still carries some sun
    let overcast = Lighting {
        cloud_density: 1.0,
        sun_intensity: 0.15,
        ..day(40.0)
    };
    assert_eq!(overcast.sun_glare_strength(), 0.0);
    let fog = Lighting {
        fog_density: 0.01,
        ..day(40.0)
    };
    assert_eq!(fog.sun_glare_strength(), 0.0);
    // a weaker glare is smaller as well as dimmer
    let g = day(0.0).sun_glare(DVec3::ZERO, 0, 0);
    assert!(g[0].size < SUN_GLARE_SIZE * 0.5 && g[0].brightness < SUN_GLARE_BRIGHTNESS);
}

#[test]
fn the_setting_off_draws_none_and_enhanced_one_procedural_sprite() {
    let off = Lighting {
        sun_glow: false,
        ..day(40.0)
    };
    assert!(off.sun_glare(DVec3::ZERO, 0, 0).is_empty());
    let enhanced = Lighting {
        enhanced: true,
        ..day(40.0)
    };
    let e = enhanced.sun_glare(DVec3::ZERO, 0, 0);
    assert_eq!(e.len(), 1);
    assert!(e[0].sky && e[0].procedural && e[0].flags & 8 == 0);
    // as wide as the classic star is drawn: 2.5 times the size, grown by its brightness by day
    let star = &vanilla_star();
    assert!(
        (e[0].size - star.size * star.brightness * 0.48).abs() < 1e-4,
        "{}",
        e[0].size
    );
    let vanilla = Lighting {
        classic: true,
        ..day(40.0)
    };
    assert_eq!(vanilla.sun_glare(DVec3::ZERO, 0, 0).len(), 2);
}

fn vanilla_star() -> omsi_render::Corona {
    let l = Lighting {
        classic: true,
        ..day(40.0)
    };
    l.sun_glare(DVec3::ZERO, 0, 0)[1]
}

/// Looking at the sun (20 degrees up in the north) over a dark sky, in Vanilla: the
/// brightness (mean of r, g, b) of the picture's middle and of a point `off` pixels to its
/// right, with the glare as asked and, when `wall`, a wall 50 m ahead hiding the sky.
fn looking_at_the_sun(glare: bool, wall: bool, off: u32) -> (u32, u32) {
    looking_at_the_sun_in(false, glare, wall, off)
}

/// [`looking_at_the_sun`], in Vanilla or (`enhanced`) in Enhanced graphics.
fn looking_at_the_sun_in(enhanced: bool, glare: bool, wall: bool, off: u32) -> (u32, u32) {
    use glam::{Mat4, Vec2};
    use omsi_geometry::MeshData;
    use omsi_render::{AlphaMode, Camera, RenderOptions, Renderer};
    use omsi_texture::Image;
    const SIZE: u32 = 64;
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
    let dark = renderer.add_texture(&mut scene, &Image::solid([8, 8, 16, 255]), false);
    renderer.set_sky_textures(&mut scene, [dark, dark, dark]);
    renderer.set_sun_glare(&mut scene, glare.then_some([0, 0]));
    if wall {
        let (y, half) = (50.0, 200.0);
        let mesh = renderer.add_mesh(
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
        let black = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [0.0, 0.0, 0.0, 1.0],
            true,
        );
        renderer.add_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![black]);
    }
    let camera = Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 20.0,
        roll: 0.0,
        fov_deg: 60.0,
        near: 0.1,
        far: 1000.0,
    };
    let lighting = Lighting {
        shadows: false,
        sky_weights: [0.0, 0.0, 1.0],
        classic: !enhanced,
        enhanced,
        ..day(20.0)
    };
    let rgba = renderer
        .render_to_image(&mut scene, SIZE, SIZE, &camera, &lighting)
        .unwrap();
    let at = |x: u32| {
        let i = ((SIZE / 2 * SIZE + x) * 4) as usize;
        (rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32) / 3
    };
    (at(SIZE / 2), at(SIZE / 2 + off))
}

#[test]
#[ignore = "needs a graphics adapter"]
fn looking_at_the_sun_the_glare_floods_the_picture_and_a_wall_hides_it() {
    let (bare, _) = looking_at_the_sun(false, false, 0);
    let (lit, aside) = looking_at_the_sun(true, false, 20);
    let (behind_wall, _) = looking_at_the_sun(true, true, 0);
    println!("sun glare: none {bare}, at the sun {lit}, 20 px aside {aside}, wall {behind_wall}");
    assert!(lit > 200, "at the sun {lit}");
    // wide: still bright a third of the picture away from the sun
    assert!(aside > bare + 60, "aside {aside} bare {bare}");
    assert!(behind_wall < 10, "behind the wall {behind_wall}");
}

#[test]
#[ignore = "needs a graphics adapter"]
fn in_enhanced_the_procedural_glare_brightens_round_the_sun() {
    // (the Enhanced sky is near white right by the sun: looked at some 23 degrees aside)
    let (_, without) = looking_at_the_sun_in(true, false, false, 24);
    let (_, with) = looking_at_the_sun_in(true, true, false, 24);
    let (_, wall) = looking_at_the_sun_in(true, false, true, 24);
    let (_, wall_glare) = looking_at_the_sun_in(true, true, true, 24);
    println!(
        "enhanced sun glare 24 px aside: sky {without} -> {with}, wall {wall} -> {wall_glare}"
    );
    assert!(with > without + 10, "sky {without} -> {with}");
    assert!(wall_glare <= wall + 1, "wall {wall} -> {wall_glare}");
}

/// What the glare costs: frames of an empty scene (only the sky) at 1280x720 looking at the
/// sun, without the glare (as before) and with it, in turns. Prints the milliseconds per
/// frame; it checks nothing.
#[test]
#[ignore = "needs a graphics adapter; a measurement"]
fn cost_of_the_sun_glare() {
    use omsi_render::{Camera, RenderOptions, Renderer};
    use omsi_texture::Image;
    const FRAMES: u32 = 40;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let mut r = pollster::block_on(Renderer::new_with(
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
    let mut scenes: Vec<_> = [false, true]
        .iter()
        .map(|&glare| {
            let mut s = r.new_scene();
            let dark = r.add_texture(&mut s, &Image::solid([8, 8, 16, 255]), false);
            r.set_sky_textures(&mut s, [dark, dark, dark]);
            r.set_sun_glare(&mut s, glare.then_some([0, 0]));
            s
        })
        .collect();
    let cam = Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 20.0,
        roll: 0.0,
        fov_deg: 60.0,
        near: 0.1,
        far: 1000.0,
    };
    let lighting = Lighting {
        shadows: false,
        sky_weights: [0.0, 0.0, 1.0],
        classic: true,
        ..day(20.0)
    };
    for s in scenes.iter_mut() {
        r.render_to_image(s, 1280, 720, &cam, &lighting).unwrap();
    }
    let mut totals = [0.0f64; 2];
    for _ in 0..FRAMES {
        for (k, s) in scenes.iter_mut().enumerate() {
            let t = std::time::Instant::now();
            r.render_to_image(s, 1280, 720, &cam, &lighting).unwrap();
            totals[k] += t.elapsed().as_secs_f64();
        }
    }
    for (k, name) in ["no glare (as before)", "glare, sun in sight"]
        .iter()
        .enumerate()
    {
        println!(
            "sun glare cost: {name:<22} {:.3} ms/frame",
            totals[k] * 1000.0 / FRAMES as f64
        );
    }
}

//! The classic sky's sun and moon (`Texture\sonne.bmp`, `Texture\mond.bmp`): where they are
//! drawn, the moon's phase from the sun's side, and what they cost the sky pass.
//!
//! Needs a graphics adapter (a software one is enough):
//!
//! ```text
//! cargo test -p omsi-render --release --test sky_bodies -- --ignored --nocapture
//! ```

use glam::{DVec3, Vec3};
use omsi_render::{Camera, Lighting, RenderOptions, Renderer, Scene};
use omsi_texture::Image;

/// A unit vector at this azimuth (degrees, clockwise from north) and altitude (degrees).
fn towards(azimuth: f32, altitude: f32) -> Vec3 {
    let (a, h) = (azimuth.to_radians(), altitude.to_radians());
    Vec3::new(h.cos() * a.sin(), h.cos() * a.cos(), h.sin())
}

fn renderer() -> Renderer {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    pollster::block_on(Renderer::new_with(
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
    .expect("test renderer")
}

/// A scene with a dark sky and, when asked, a white sun and a white moon.
fn sky(renderer: &Renderer, sun: bool, moon: bool) -> Scene {
    let mut scene = renderer.new_scene();
    let dark = renderer.add_texture(&mut scene, &Image::solid([8, 8, 16, 255]), false);
    let white = renderer.add_texture(&mut scene, &Image::solid([255, 255, 255, 255]), false);
    renderer.set_sky_textures(&mut scene, [dark, dark, dark]);
    renderer.set_sky_bodies(&mut scene, sun.then_some(white), moon.then_some(white));
    scene
}

/// Looking north, 30 degrees up, through a narrow field of view.
fn camera(fov_deg: f32) -> Camera {
    Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 30.0,
        roll: 0.0,
        fov_deg,
        near: 0.1,
        far: 1000.0,
    }
}

fn lighting(sun: Vec3, moon: Vec3) -> Lighting {
    Lighting {
        shadows: false,
        fog_density: 0.0,
        sun_dir: sun,
        moon_dir: moon,
        sky_weights: [0.0, 0.0, 1.0],
        ..Default::default()
    }
}

const SIZE: u32 = 128;

/// The brightness (mean of r, g, b) of the pixel `dx` to the right of the picture's middle.
fn brightness(rgba: &[u8], dx: i32) -> u32 {
    let x = (SIZE as i32 / 2 + dx) as u32;
    let at = ((SIZE / 2 * SIZE + x) * 4) as usize;
    (rgba[at] as u32 + rgba[at + 1] as u32 + rgba[at + 2] as u32) / 3
}

fn picture(renderer: &mut Renderer, scene: &mut Scene, sun: Vec3, moon: Vec3) -> Vec<u8> {
    renderer
        .render_to_image(scene, SIZE, SIZE, &camera(4.0), &lighting(sun, moon))
        .unwrap()
}

#[test]
#[ignore = "needs a graphics adapter"]
fn the_sun_is_drawn_where_it_stands_and_only_with_its_picture() {
    let mut r = renderer();
    let ahead = towards(0.0, 30.0);
    let away = towards(180.0, -30.0);
    let mut with = sky(&r, true, false);
    let lit = picture(&mut r, &mut with, ahead, away);
    let mut without = sky(&r, false, false);
    let bare = picture(&mut r, &mut without, ahead, away);
    assert!(brightness(&lit, 0) > 200, "sun {}", brightness(&lit, 0));
    assert!(brightness(&bare, 0) < 40, "no sun {}", brightness(&bare, 0));
    // the sun elsewhere leaves the sky as it was
    let elsewhere = picture(&mut r, &mut with, towards(90.0, 30.0), away);
    assert!(brightness(&elsewhere, 0) < 40);
}

#[test]
#[ignore = "needs a graphics adapter"]
fn the_moon_shows_the_phase_the_sun_gives_it() {
    let mut r = renderer();
    let moon = towards(0.0, 30.0);
    let mut scene = sky(&r, false, true);
    // full: the sun behind the eye
    let full = picture(&mut r, &mut scene, -moon, moon);
    assert!(brightness(&full, 0) > 150, "full {}", brightness(&full, 0));
    // new: the sun behind the moon (its picture left out), the face towards us dark
    let new = picture(&mut r, &mut scene, towards(0.0, 31.0), moon);
    assert!(brightness(&new, 0) < 40, "new {}", brightness(&new, 0));
    // a quarter: the sun to the east (the right of the picture) lights the right half only
    let quarter = picture(&mut r, &mut scene, towards(90.0, 0.0), moon);
    let (left, right) = (brightness(&quarter, -8), brightness(&quarter, 8));
    assert!(right > 100 && left < 40, "left {left} right {right}");
}

/// What the sun and the moon cost the sky pass: frames of an empty scene (only the sky) at
/// 1280x720, without the pictures (the sky as before), with them but out of sight, and with
/// both in sight. Prints the milliseconds per frame; it checks nothing.
#[test]
#[ignore = "needs a graphics adapter; a measurement"]
fn cost_of_the_sun_and_the_moon() {
    const FRAMES: u32 = 40;
    let mut r = renderer();
    let cam = Camera {
        pitch: 20.0,
        fov_deg: 60.0,
        ..camera(60.0)
    };
    let in_sight = lighting(towards(-8.0, 20.0), towards(8.0, 22.0));
    let out_of_sight = lighting(towards(180.0, 20.0), towards(170.0, 22.0));
    let cases: [(&str, bool, &Lighting); 3] = [
        ("no pictures (as before)", false, &in_sight),
        ("pictures, out of sight", true, &out_of_sight),
        ("pictures, both in sight", true, &in_sight),
    ];
    // warm up every case once, then take them in turns so that drift hits all alike
    let mut scenes: Vec<Scene> = cases.iter().map(|c| sky(&r, c.1, c.1)).collect();
    for (k, c) in cases.iter().enumerate() {
        r.render_to_image(&mut scenes[k], 1280, 720, &cam, c.2)
            .unwrap();
    }
    let mut totals = [0.0f64; 3];
    for _ in 0..FRAMES {
        for (k, c) in cases.iter().enumerate() {
            let t = std::time::Instant::now();
            r.render_to_image(&mut scenes[k], 1280, 720, &cam, c.2)
                .unwrap();
            totals[k] += t.elapsed().as_secs_f64();
        }
    }
    for (k, c) in cases.iter().enumerate() {
        println!(
            "sky bodies cost: {:<26} {:.3} ms/frame",
            c.0,
            totals[k] * 1000.0 / FRAMES as f64
        );
    }
}

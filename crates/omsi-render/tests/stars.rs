//! The night sky's stars (`helper\stars.dat` laid out as a star map): where the classic skies
//! draw them, how large and how bright, how the sky turns, and what they cost the sky pass.
//!
//! Needs a graphics adapter (a software one is enough):
//!
//! ```text
//! cargo test -p omsi-render --release --test stars -- --ignored --nocapture
//! ```

use glam::{DVec3, Vec3};
use omsi_content::stars::{StarCatalog, StarMap};
use omsi_render::{Camera, Lighting, RenderOptions, Renderer, Scene};
use omsi_texture::Image;

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

/// A scene with a black sky and the stars of `lines` (stars.dat's own lines).
fn sky(renderer: &Renderer, lines: &str) -> Scene {
    let mut scene = renderer.new_scene();
    let black = renderer.add_texture(&mut scene, &Image::solid([0, 0, 0, 255]), false);
    renderer.set_sky_textures(&mut scene, [black, black, black]);
    let map = StarMap::new(&StarCatalog::parse(lines), 256);
    let image = Image {
        width: map.width,
        height: map.height,
        rgba: map.rgba,
        has_alpha: true,
    };
    renderer.set_star_map(&mut scene, Some((&image, map.cells)));
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

const UNTURNED: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn night(frame: [[f32; 3]; 3], bright: f32, classic: bool) -> Lighting {
    Lighting {
        shadows: false,
        fog_density: 0.0,
        sun_dir: Vec3::new(0.0, -0.5, -0.866),
        sky_weights: [0.0, 0.0, 1.0],
        star_frame: frame,
        star_brightness: bright,
        classic,
        ..Default::default()
    }
}

const SIZE: u32 = 128;

/// The pixels brighter than black (their index and colour).
fn lit(rgba: &[u8]) -> Vec<(usize, [u8; 3])> {
    rgba.chunks_exact(4)
        .enumerate()
        .filter(|(_, p)| p[0] > 0 || p[1] > 0 || p[2] > 0)
        .map(|(k, p)| (k, [p[0], p[1], p[2]]))
        .collect()
}

/// With the frame unturned a star's right ascension and declination are its azimuth from
/// the east round to the north and its height: (90, 30) stands north, 30 degrees up - the
/// middle of the camera's view.
const AHEAD_A: &str = "90\t30\t1.0\tA\n";
const AHEAD_K: &str = "90\t30\t0.003981072\tK\n";

#[test]
#[ignore = "needs a graphics adapter"]
fn a_star_is_a_square_of_its_size_in_its_class_colour() {
    let mut r = renderer();
    for classic in [true, false] {
        // magnitude 0: two pixels a side, in the colour of class A (240, 244, 255)
        let mut scene = sky(&r, AHEAD_A);
        let px = r
            .render_to_image(
                &mut scene,
                SIZE,
                SIZE,
                &camera(10.0),
                &night(UNTURNED, 1.0, classic),
            )
            .unwrap();
        let l = lit(&px);
        assert_eq!(l.len(), 4, "classic {classic}: {l:?}");
        let [r0, g0, b0] = l[0].1;
        assert!(
            r0.abs_diff(240) <= 3 && g0.abs_diff(244) <= 3 && b0 >= 252,
            "{:?}",
            l[0]
        );
        let (x, y) = (l[0].0 as u32 % SIZE, l[0].0 as u32 / SIZE);
        assert!(
            x.abs_diff(SIZE / 2) <= 2 && y.abs_diff(SIZE / 2) <= 2,
            "{x} {y}"
        );
        // magnitude 6: one pixel, a quarter as bright (class K: 255, 228, 147)
        let mut scene = sky(&r, AHEAD_K);
        let px = r
            .render_to_image(
                &mut scene,
                SIZE,
                SIZE,
                &camera(10.0),
                &night(UNTURNED, 1.0, classic),
            )
            .unwrap();
        let l = lit(&px);
        assert_eq!(l.len(), 1, "classic {classic}: {l:?}");
        assert!(
            l[0].1[0].abs_diff(64) <= 3 && l[0].1[1].abs_diff(57) <= 3,
            "{:?}",
            l[0]
        );
    }
}

#[test]
#[ignore = "needs a graphics adapter"]
fn the_stars_fade_with_the_daylight_and_turn_with_the_sky() {
    let mut r = renderer();
    let mut scene = sky(&r, AHEAD_A);
    let mut draw = |frame, bright| {
        r.render_to_image(
            &mut scene,
            SIZE,
            SIZE,
            &camera(10.0),
            &night(frame, bright, true),
        )
        .unwrap()
    };
    let half = lit(&draw(UNTURNED, 0.5));
    assert!(half[0].1[0].abs_diff(120) <= 3, "{half:?}");
    assert!(lit(&draw(UNTURNED, 0.0)).is_empty());
    // the sky turned a little about the north-south line: the same star, moved across the
    // view (it stands 30 degrees up: half the turn's 2.9 degrees, some 18 pixels)
    let a = 0.05f32;
    let little = [
        [a.cos(), 0.0, a.sin()],
        [0.0, 1.0, 0.0],
        [-a.sin(), 0.0, a.cos()],
    ];
    let moved = lit(&draw(little, 1.0));
    assert_eq!(moved.len(), 4, "{moved:?}");
    assert!(
        (moved[0].0 as u32 % SIZE).abs_diff(SIZE / 2) > 12,
        "{moved:?}"
    );
}

#[test]
#[ignore = "needs a graphics adapter"]
fn without_a_star_map_the_sky_has_no_stars() {
    let mut r = renderer();
    let mut scene = r.new_scene();
    let black = r.add_texture(&mut scene, &Image::solid([0, 0, 0, 255]), false);
    r.set_sky_textures(&mut scene, [black, black, black]);
    let px = r
        .render_to_image(
            &mut scene,
            SIZE,
            SIZE,
            &camera(10.0),
            &night(UNTURNED, 1.0, true),
        )
        .unwrap();
    assert!(lit(&px).is_empty());
}

#[test]
#[ignore = "needs a graphics adapter"]
fn in_enhanced_a_bright_star_shows_in_a_dark_night() {
    let mut r = renderer();
    let enhanced = Lighting {
        classic: false,
        enhanced: true,
        ..night(UNTURNED, 1.0, false)
    };
    let mut with = sky(&r, "90\t30\t3.8\tA\n");
    let mut without = r.new_scene();
    let black = r.add_texture(&mut without, &Image::solid([0, 0, 0, 255]), false);
    r.set_sky_textures(&mut without, [black, black, black]);
    // the brightest pixel near the middle of the view
    let mut peak = |scene: &mut Scene| {
        let px = r
            .render_to_image(scene, SIZE, SIZE, &camera(10.0), &enhanced)
            .unwrap();
        let mut best = 0u32;
        for y in SIZE / 2 - 4..SIZE / 2 + 4 {
            for x in SIZE / 2 - 4..SIZE / 2 + 4 {
                let i = ((y * SIZE + x) * 4) as usize;
                best = best.max((px[i] as u32 + px[i + 1] as u32 + px[i + 2] as u32) / 3);
            }
        }
        best
    };
    let (star, bare) = (peak(&mut with), peak(&mut without));
    println!("enhanced: a bright star {star}, the sky without {bare}");
    assert!(star > bare + 40, "star {star} sky {bare}");
}

/// Some 5000 stars spread over the sky as the stock catalogue spreads its magnitudes (most
/// of them faint), from a fixed seed.
fn many_stars() -> String {
    let mut x = 0x2545_f491_u32;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x as f64 / u32::MAX as f64
    };
    let mut s = String::new();
    for _ in 0..5000 {
        let ra = next() * 360.0;
        let dec = (next() * 2.0 - 1.0).asin().to_degrees();
        let mag = 6.0 - 7.0 * next().powf(4.0);
        let flux = 10f64.powf(-0.4 * mag);
        s.push_str(&format!("{ra:.2}\t{dec:.2}\t{flux:.6}\tG\n"));
    }
    s
}

/// What the stars cost the sky pass: frames of an empty scene (only the sky) at 1280x720,
/// without a star map (the sky as before), with one by day (the stars out) and with one
/// at night, and the Enhanced sky at night without and with one. Prints the milliseconds
/// per frame; it checks nothing.
#[test]
#[ignore = "needs a graphics adapter; a measurement"]
fn cost_of_the_stars() {
    const FRAMES: u32 = 40;
    let mut r = renderer();
    let cam = Camera {
        pitch: 20.0,
        fov_deg: 60.0,
        ..camera(60.0)
    };
    let stars = many_stars();
    let bare = |r: &Renderer| {
        let mut s = r.new_scene();
        let black = r.add_texture(&mut s, &Image::solid([0, 0, 0, 255]), false);
        r.set_sky_textures(&mut s, [black, black, black]);
        s
    };
    let mut scenes = [
        bare(&r),
        sky(&r, &stars),
        sky(&r, &stars),
        bare(&r),
        sky(&r, &stars),
    ];
    let enhanced = Lighting {
        enhanced: true,
        ..night(UNTURNED, 1.0, false)
    };
    let lights = [
        night(UNTURNED, 1.0, false),
        night(UNTURNED, 0.0, false),
        night(UNTURNED, 1.0, false),
        enhanced.clone(),
        enhanced,
    ];
    let names = [
        "no star map (as before)",
        "star map, by day",
        "star map, at night",
        "Enhanced, no star map",
        "Enhanced, star map",
    ];
    for k in 0..5 {
        r.render_to_image(&mut scenes[k], 1280, 720, &cam, &lights[k])
            .unwrap();
    }
    let mut totals = [0.0f64; 5];
    for _ in 0..FRAMES {
        for k in 0..5 {
            let t = std::time::Instant::now();
            r.render_to_image(&mut scenes[k], 1280, 720, &cam, &lights[k])
                .unwrap();
            totals[k] += t.elapsed().as_secs_f64();
        }
    }
    for k in 0..5 {
        println!(
            "stars cost: {:<24} {:.3} ms/frame",
            names[k],
            totals[k] * 1000.0 / FRAMES as f64
        );
    }
}

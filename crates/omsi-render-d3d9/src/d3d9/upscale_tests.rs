//! Device tests of the render scale, skipped like the others without a usable device. They draw
//! off-screen: the scale works on whatever frame the device begins.

use super::tests::device;
use super::*;
use crate::render_scale::Upscale;
use crate::scene::{
    DrawKind, SceneBatch, SceneFrame, SceneMaterial, SceneTexture, TexFormat, MATERIAL_VECTORS,
};

const IDENTITY: [[f32; 4]; 4] = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

/// A quad over `x0..x1` (clip space) and the whole height, uv 0..1, facing the camera.
fn quad(x0: f32, x1: f32) -> (Vec<f32>, Vec<u32>) {
    let v = |x: f32, y: f32, u: f32, w: f32| [x, y, 0.5, 0., 0., 1., u, w];
    (
        [
            v(x0, -1., 0., 1.),
            v(x1, -1., 1., 1.),
            v(x1, 1., 1., 0.),
            v(x0, 1., 0., 0.),
        ]
        .concat(),
        vec![0, 1, 2, 0, 2, 3],
    )
}

/// A lit material whose colour shows as the texture: only ambient light, all of it taken.
fn uniform() -> [[f32; 4]; MATERIAL_VECTORS] {
    let mut u = [[0.0; 4]; MATERIAL_VECTORS];
    u[0] = [1.0; 4];
    u[9] = [1.0, 1.0, 1.0, 0.0];
    u
}

/// Puts up a quad over `x0..x1` of grey `level`, as mesh 0 with material 0 and texture 0.
fn set_up(dev: &mut D3d9Device, x0: f32, x1: f32, level: u8) {
    let (v, i) = quad(x0, x1);
    dev.set_scene_mesh(0, &v, &i).unwrap();
    let levels = vec![[level, level, level, 255].repeat(4)];
    dev.set_scene_texture(
        0,
        &SceneTexture {
            width: 2,
            height: 2,
            format: TexFormat::Rgba8,
            levels: &levels,
            make_mips: true,
        },
    )
    .unwrap();
}

/// Draws the quad put up by `set_up` over `clear` in a `w` x `h` frame and returns the pixels.
fn draw(dev: &mut D3d9Device, w: u32, h: u32, clear: f32) -> Vec<u8> {
    dev.begin_frame(&FrameDesc {
        width: w,
        height: h,
        clear_rgba: [clear, clear, clear, 1.0],
    })
    .unwrap();
    let batches = [SceneBatch {
        kind: DrawKind::Opaque,
        cull: false,
        surface: false,
        mesh: 0,
        first: 0,
        count: 6,
        material: 0,
        instances: 0..1,
    }];
    let materials = [uniform()];
    let frame = SceneFrame {
        view_proj: IDENTITY,
        light: crate::scene::SceneLight {
            cam_pos: [0.0, 0.0, -1.0, 0.0],
            ambient: [1.0, 1.0, 1.0, 0.0],
            sky_color: [0.0, 0.0, 0.0, 1.0],
            ..Default::default()
        },
        list: &[0],
        models: &[IDENTITY],
        params: &[[1.0, 1.0, 0.0, 0.0], [0.0; 4]],
        main: &batches,
        cab: &[],
        surface_bias: -24,
        lamps: Default::default(),
        coronas: Default::default(),
        sky: Default::default(),
        rain: Default::default(),
        smoke: Default::default(),
    };
    dev.draw_scene(&frame, |m| {
        materials.get(m).map(|u| SceneMaterial {
            texture: Some(0),
            uniform: u,
            ..Default::default()
        })
    })
    .unwrap();
    dev.end_frame().unwrap()
}

fn red(px: &[u8], w: u32, x: u32, y: u32) -> u8 {
    px[((y * w + x) * 4) as usize]
}

#[test]
fn no_scale_draws_the_same_picture_as_the_option_never_set() {
    let Some(mut dev) = device() else { return };
    set_up(&mut dev, -1.0, 0.0, 255);
    let base = draw(&mut dev, 16, 8, 0.0);
    for scale in [0.0, 1.0, f32::NAN, 7.0] {
        dev.set_render_scale(scale, Upscale::Fsr1);
        assert_eq!(dev.render_scale(), None, "{scale}");
        assert_eq!(draw(&mut dev, 16, 8, 0.0), base, "{scale}");
    }
    // a scale used and then taken off goes back to it, and lets go of its targets
    dev.set_render_scale(0.5, Upscale::Bilinear);
    assert_ne!(draw(&mut dev, 16, 8, 0.0), base);
    dev.set_render_scale(1.0, Upscale::Bilinear);
    assert!(dev.scale.targets.is_none());
    assert_eq!(draw(&mut dev, 16, 8, 0.0), base);
}

#[test]
fn bilinear_is_the_small_picture_stretched_with_the_cards_filter() {
    let Some(mut dev) = device() else { return };
    // an edge a third of the way across, so it does not fall on a texel border
    set_up(&mut dev, -1.0, -0.3, 255);
    let (w, h) = (32u32, 16u32);
    // the picture at the small size, drawn without the option
    let small = draw(&mut dev, w / 2, h / 2, 0.0);
    dev.set_render_scale(0.5, Upscale::Bilinear);
    let big = draw(&mut dev, w, h, 0.0);
    assert_eq!(big.len(), (w * h * 4) as usize);
    // the reference: bilinear sampling of the small picture at texel centres, edges held
    let (sw, sh) = (w / 2, h / 2);
    let mut worst = 0i32;
    for y in 0..h {
        for x in 0..w {
            let fx = ((x as f32 + 0.5) / 2.0 - 0.5).clamp(0.0, (sw - 1) as f32);
            let fy = ((y as f32 + 0.5) / 2.0 - 0.5).clamp(0.0, (sh - 1) as f32);
            let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
            let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
            let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
            let at = |x, y| red(&small, sw, x, y) as f32;
            let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
            let bottom = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
            let want = top * (1.0 - ty) + bottom * ty;
            worst = worst.max((red(&big, w, x, y) as f32 - want).abs().round() as i32);
        }
    }
    assert!(worst <= 3, "off by {worst} from the stretched small picture");
}

#[test]
fn flat_pictures_stay_flat_with_both_upscales() {
    let Some(mut dev) = device() else { return };
    for upscale in [Upscale::Bilinear, Upscale::Fsr1] {
        dev.set_render_scale(0.5, upscale);
        // all white, all black (the sharpening's 0 times 1/0), and a grey
        for (level, clear) in [(255u8, 0.0), (0, 0.0), (128, 0.0)] {
            set_up(&mut dev, -1.0, 1.0, level);
            let p = draw(&mut dev, 32, 16, clear);
            assert!(
                p.chunks_exact(4)
                    .all(|c| c[0].abs_diff(level) <= 1 && c[1].abs_diff(level) <= 1 && c[3] == 255),
                "{upscale:?} {level}: {:?}",
                &p[..8]
            );
        }
    }
}

#[test]
fn the_sharper_upscale_keeps_an_edge_tighter_than_bilinear_and_in_range() {
    let Some(mut dev) = device() else { return };
    // an edge at a third of the way across, white on black
    set_up(&mut dev, -1.0, -0.3, 255);
    let (w, h) = (64u32, 32u32);
    let row = |p: &[u8]| -> Vec<u8> { (0..w).map(|x| red(p, w, x, h / 2)).collect() };
    dev.set_render_scale(0.5, Upscale::Bilinear);
    let soft = row(&draw(&mut dev, w, h, 0.0));
    dev.set_render_scale(0.5, Upscale::Fsr1);
    let sharp = row(&draw(&mut dev, w, h, 0.0));
    // (the two are different passes: the sharper one really ran)
    assert!(dev.fsr_in_use());
    assert_ne!(sharp, soft);
    // the pixels that are neither black nor white: the edge's width
    let width = |r: &[u8]| r.iter().filter(|&&v| v > 12 && v < 243).count();
    eprintln!("soft {soft:?}\nsharp {sharp:?}");
    assert!(
        width(&sharp) <= width(&soft),
        "sharp {sharp:?}\nsoft {soft:?}"
    );
    // the edge stays where it was (the half-way point within a pixel of the bilinear one)
    let half = |r: &[u8]| r.iter().position(|&v| v < 128).unwrap();
    assert!(half(&sharp).abs_diff(half(&soft)) <= 1, "{sharp:?}\n{soft:?}");
    // white to black, with no wild ringing
    assert!(sharp[0] >= 240 && sharp[w as usize - 1] <= 12, "{sharp:?}");
}

#[test]
fn the_interface_is_drawn_at_the_pictures_size_over_the_scaled_scene() {
    let Some(mut dev) = device() else { return };
    set_up(&mut dev, -1.0, 1.0, 0);
    dev.set_render_scale(0.5, Upscale::Bilinear);
    // a one-pixel-wide white overlay line at the picture's own resolution
    let levels = vec![[255, 255, 255, 255].repeat(4)];
    dev.set_scene_texture(
        5,
        &SceneTexture {
            width: 2,
            height: 2,
            format: TexFormat::Rgba8,
            levels: &levels,
            make_mips: false,
        },
    )
    .unwrap();
    let (w, h) = (16u32, 8u32);
    dev.begin_frame(&FrameDesc {
        width: w,
        height: h,
        clear_rgba: [0.0, 0.0, 0.0, 1.0],
    })
    .unwrap();
    let batches = [SceneBatch {
        kind: DrawKind::Opaque,
        cull: false,
        surface: false,
        mesh: 0,
        first: 0,
        count: 6,
        material: 0,
        instances: 0..1,
    }];
    let materials = [uniform()];
    let frame = SceneFrame {
        view_proj: IDENTITY,
        light: crate::scene::SceneLight {
            cam_pos: [0.0, 0.0, -1.0, 0.0],
            ambient: [1.0, 1.0, 1.0, 0.0],
            sky_color: [0.0, 0.0, 0.0, 1.0],
            ..Default::default()
        },
        list: &[0],
        models: &[IDENTITY],
        params: &[[1.0, 1.0, 0.0, 0.0], [0.0; 4]],
        main: &batches,
        cab: &[],
        surface_bias: -24,
        lamps: Default::default(),
        coronas: Default::default(),
        sky: Default::default(),
        rain: Default::default(),
        smoke: Default::default(),
    };
    dev.draw_scene(&frame, |m| {
        materials.get(m).map(|u| SceneMaterial {
            texture: Some(0),
            uniform: u,
            ..Default::default()
        })
    })
    .unwrap();
    dev.draw_overlays(&[crate::ui::UiOverlay {
        texture: 5,
        rect: [3.0, 0.0, 4.0, 8.0],
        premultiplied: false,
        transposed: false,
    }])
    .unwrap();
    let p = dev.end_frame().unwrap();
    // exactly one pixel column is white, the next ones black: not smeared by the up-scaling
    assert_eq!(red(&p, w, 3, 4), 255, "{:?}", (0..w).map(|x| red(&p, w, x, 4)).collect::<Vec<_>>());
    assert_eq!(red(&p, w, 2, 4), 0);
    assert_eq!(red(&p, w, 4, 4), 0);
}

#[test]
fn the_targets_follow_the_picture_size_and_are_let_go_of() {
    let Some(mut dev) = device() else { return };
    set_up(&mut dev, -1.0, 1.0, 200);
    dev.set_render_scale(0.5, Upscale::Fsr1);
    for (w, h) in [(32, 16), (48, 24), (7, 5), (1, 1), (32, 16)] {
        let p = draw(&mut dev, w, h, 0.0);
        assert_eq!(p.len(), (w * h * 4) as usize);
        if dev.scene_size_for((w, h)).is_some() {
            assert_eq!(dev.scale.targets.as_ref().map(|t| t.native), Some((w, h)));
        }
    }
    dev.drop_scale_targets();
    assert!(dev.scale.targets.is_none());
    // and a scene after that makes them again
    draw(&mut dev, 32, 16, 0.0);
    assert!(dev.scale.targets.is_some());
}

//! Device tests of the scene layer, skipped like the others without a usable device.

use super::tests::device;
use super::*;
use crate::scene::{
    DrawKind, SceneBatch, SceneFrame, SceneLight, SceneMaterial, SceneTexture, TexFormat,
    MATERIAL_VECTORS,
};

const W: u32 = 8;
const H: u32 = 4;
const IDENTITY: [[f32; 4]; 4] = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

/// A quad over the whole target at clip depth `z` (identity camera and model), uv 0..1.
fn quad(z: f32) -> (Vec<f32>, Vec<u32>) {
    let v = |x: f32, y: f32, u: f32, w: f32| [x, y, z, 0., 0., 1., u, w];
    (
        [
            v(-1., -1., 0., 1.),
            v(1., -1., 1., 1.),
            v(1., 1., 1., 0.),
            v(-1., 1., 0., 0.),
        ]
        .concat(),
        vec![0, 1, 2, 0, 2, 3],
    )
}

fn rgba(dev: &mut D3d9Device, id: usize, px: [u8; 4]) {
    let levels = vec![px.repeat(4)];
    dev.set_scene_texture(
        id,
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

/// A lit material whose colour shows as the texture: only ambient light, all of it taken by
/// the material's ambient colour. `mode`: 0 opaque, 1 test, 2 blend.
fn uniform(mode: f32) -> [[f32; 4]; MATERIAL_VECTORS] {
    let mut u = [[0.0; 4]; MATERIAL_VECTORS];
    u[0] = [1.0; 4]; // colour
    u[1] = [mode, 0.0, 0.0, 0.0];
    u[9] = [1.0, 1.0, 1.0, 0.0]; // ambient colour
    u
}

fn light(fog: [f32; 4]) -> SceneLight {
    SceneLight {
        cam_pos: [0.0, 0.0, -1.0, 0.0],
        ambient: [1.0, 1.0, 1.0, 0.0],
        fog,
        sky_color: [0.0, 0.0, 0.0, 1.0],
        ..Default::default()
    }
}

fn batch(
    kind: DrawKind,
    mesh: usize,
    material: usize,
    instances: std::ops::Range<u32>,
) -> SceneBatch {
    SceneBatch {
        kind,
        cull: false,
        surface: false,
        mesh,
        first: 0,
        count: 6,
        material,
        instances,
    }
}

const VISIBLE: [f32; 4] = [1.0, 1.0, 0.0, 0.0];
const SECOND: [f32; 4] = [0.0; 4];

/// Draws `batches` over a black clear with one model per list entry and returns the pixels.
#[allow(clippy::too_many_arguments)]
fn draw(
    dev: &mut D3d9Device,
    batches: &[SceneBatch],
    models: &[[[f32; 4]; 4]],
    params: &[[f32; 4]],
    list: &[u32],
    light: SceneLight,
    materials: &[[[f32; 4]; MATERIAL_VECTORS]],
    texture: Option<usize>,
) -> Vec<u8> {
    dev.begin_frame(&FrameDesc {
        width: W,
        height: H,
        clear_rgba: [0.0, 0.0, 0.0, 1.0],
    })
    .unwrap();
    let frame = SceneFrame {
        view_proj: IDENTITY,
        light,
        list,
        models,
        params,
        main: batches,
        cab: &[],
        surface_bias: -24,
    };
    dev.draw_scene(&frame, |m| {
        materials.get(m).map(|u| SceneMaterial {
            texture,
            transmap: None,
            uniform: u,
        })
    })
    .unwrap();
    dev.end_frame().unwrap()
}

fn all(px: &[u8], want: [u8; 4], tol: u8) -> bool {
    px.chunks_exact(4)
        .all(|p| p.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= tol))
}

#[test]
fn the_scene_shows_the_texture_under_the_ambient_light() {
    let Some(mut dev) = device() else { return };
    let (v, i) = quad(0.5);
    dev.set_scene_mesh(3, &v, &i).unwrap();
    rgba(&mut dev, 5, [200, 100, 50, 255]);
    let px = draw(
        &mut dev,
        &[batch(DrawKind::Opaque, 3, 0, 0..1)],
        &[IDENTITY],
        &[VISIBLE, SECOND],
        &[0],
        light([0.0; 4]),
        &[uniform(0.0)],
        Some(5),
    );
    assert!(all(&px, [200, 100, 50, 255], 1), "{:?}", &px[..4]);
}

#[test]
fn without_light_a_lit_material_is_black_and_a_missing_texture_is_white() {
    let Some(mut dev) = device() else { return };
    let (v, i) = quad(0.5);
    dev.set_scene_mesh(0, &v, &i).unwrap();
    let mut dark = light([0.0; 4]);
    dark.ambient = [0.0; 4];
    let px = draw(
        &mut dev,
        &[batch(DrawKind::Opaque, 0, 0, 0..1)],
        &[IDENTITY],
        &[VISIBLE, SECOND],
        &[0],
        dark,
        &[uniform(0.0)],
        None,
    );
    assert!(all(&px, [0, 0, 0, 255], 0), "{:?}", &px[..4]);
    // an emissive white material on the missing (white) texture
    let mut glow = uniform(0.0);
    glow[4] = [1.0, 1.0, 1.0, 0.0];
    let px = draw(
        &mut dev,
        &[batch(DrawKind::Opaque, 0, 0, 0..1)],
        &[IDENTITY],
        &[VISIBLE, SECOND],
        &[0],
        dark,
        &[glow],
        Some(42),
    );
    assert!(all(&px, [255, 255, 255, 255], 0), "{:?}", &px[..4]);
}

#[test]
fn fog_covers_the_scene_with_its_colour() {
    let Some(mut dev) = device() else { return };
    let (v, i) = quad(0.5);
    dev.set_scene_mesh(0, &v, &i).unwrap();
    rgba(&mut dev, 0, [255, 255, 255, 255]);
    // the camera 1000 m away, a dense fog
    let mut l = light([0.25, 0.5, 0.75, 1.0]);
    l.cam_pos = [0.0, 0.0, -1000.0, 0.0];
    let px = draw(
        &mut dev,
        &[batch(DrawKind::Opaque, 0, 0, 0..1)],
        &[IDENTITY],
        &[VISIBLE, SECOND],
        &[0],
        l,
        &[uniform(0.0)],
        Some(0),
    );
    assert!(all(&px, [64, 128, 191, 255], 1), "{:?}", &px[..4]);
}

#[test]
fn alpha_tested_materials_cut_at_128() {
    let Some(mut dev) = device() else { return };
    let (v, i) = quad(0.5);
    dev.set_scene_mesh(0, &v, &i).unwrap();
    for (alpha, kept) in [(128u8, false), (129, true)] {
        rgba(&mut dev, 0, [255, 0, 0, alpha]);
        let px = draw(
            &mut dev,
            &[batch(DrawKind::AlphaTest, 0, 0, 0..1)],
            &[IDENTITY],
            &[VISIBLE, SECOND],
            &[0],
            light([0.0; 4]),
            &[uniform(1.0)],
            Some(0),
        );
        let want = if kept {
            [255, 0, 0, alpha]
        } else {
            [0, 0, 0, 255]
        };
        assert!(all(&px, want, 1), "alpha {alpha}: {:?}", &px[..4]);
    }
}

#[test]
fn depth_is_reversed_nearer_is_greater() {
    let Some(mut dev) = device() else { return };
    let (near_v, i) = quad(0.8);
    let (far_v, _) = quad(0.2);
    dev.set_scene_mesh(0, &near_v, &i).unwrap();
    dev.set_scene_mesh(1, &far_v, &i).unwrap();
    rgba(&mut dev, 0, [255, 0, 0, 255]);
    rgba(&mut dev, 1, [0, 0, 255, 255]);
    dev.begin_frame(&FrameDesc {
        width: W,
        height: H,
        clear_rgba: [0.0, 0.0, 0.0, 1.0],
    })
    .unwrap();
    let u = [uniform(0.0), uniform(0.0)];
    // the near quad first, then the far one: the far one must lose
    let batches = [
        batch(DrawKind::Opaque, 0, 0, 0..1),
        batch(DrawKind::Opaque, 1, 1, 0..1),
    ];
    let frame = SceneFrame {
        view_proj: IDENTITY,
        light: light([0.0; 4]),
        list: &[0],
        models: &[IDENTITY],
        params: &[VISIBLE, SECOND],
        main: &batches,
        cab: &[],
        surface_bias: -24,
    };
    dev.draw_scene(&frame, |m| {
        u.get(m).map(|uniform| SceneMaterial {
            texture: Some(m),
            transmap: None,
            uniform,
        })
    })
    .unwrap();
    let px = dev.end_frame().unwrap();
    assert!(all(&px, [255, 0, 0, 255], 1), "{:?}", &px[..4]);
}

#[test]
fn each_list_entry_is_an_instance_and_invisible_ones_are_not_drawn() {
    let Some(mut dev) = device() else { return };
    // a quad over the left half, moved right by its second model
    let v = |x: f32, y: f32| [x, y, 0.5, 0., 0., 1., 0., 0.];
    let verts = [v(-1., -1.), v(0., -1.), v(0., 1.), v(-1., 1.)].concat();
    dev.set_scene_mesh(0, &verts, &[0, 1, 2, 0, 2, 3]).unwrap();
    rgba(&mut dev, 0, [0, 255, 0, 255]);
    let mut moved = IDENTITY;
    moved[3] = [1.0, 0.0, 0.0, 1.0];
    let models = [IDENTITY, moved];
    let params = [VISIBLE, SECOND, VISIBLE, SECOND];
    let px = draw(
        &mut dev,
        &[batch(DrawKind::Opaque, 0, 0, 0..2)],
        &models,
        &params,
        &[0, 1],
        light([0.0; 4]),
        &[uniform(0.0)],
        Some(0),
    );
    assert!(all(&px, [0, 255, 0, 255], 1), "both halves: {:?}", px);
    // the second instance invisible: the right half stays clear
    let hidden = [VISIBLE, SECOND, [1.0, 0.0, 0.0, 0.0], SECOND];
    let px = draw(
        &mut dev,
        &[batch(DrawKind::Opaque, 0, 0, 0..2)],
        &models,
        &hidden,
        &[0, 1],
        light([0.0; 4]),
        &[uniform(0.0)],
        Some(0),
    );
    let row = &px[..(W * 4) as usize];
    assert!(all(&row[..16], [0, 255, 0, 255], 1), "left: {row:?}");
    assert!(all(&row[16..], [0, 0, 0, 255], 0), "right: {row:?}");
}

#[test]
fn compressed_textures_and_missing_meshes() {
    let Some(mut dev) = device() else { return };
    let (v, i) = quad(0.5);
    dev.set_scene_mesh(0, &v, &i).unwrap();
    // one DXT1 block, both end colours pure red (RGB565 0xF800), every texel colour 0
    let levels = vec![vec![0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0]];
    dev.set_scene_texture(
        7,
        &SceneTexture {
            width: 4,
            height: 4,
            format: TexFormat::Bc1,
            levels: &levels,
            make_mips: false,
        },
    )
    .unwrap();
    // a batch whose mesh is not up is skipped
    let px = draw(
        &mut dev,
        &[
            batch(DrawKind::Opaque, 9, 0, 0..1),
            batch(DrawKind::Opaque, 0, 0, 0..1),
        ],
        &[IDENTITY],
        &[VISIBLE, SECOND],
        &[0],
        light([0.0; 4]),
        &[uniform(0.0)],
        Some(7),
    );
    assert!(all(&px, [255, 0, 0, 255], 1), "{:?}", &px[..4]);
    dev.free_scene_mesh(0);
    assert!(!dev.has_scene_mesh(0));
    dev.move_scene_texture(7, 8);
    assert!(dev.has_scene_texture(8) && !dev.has_scene_texture(7));
}

#[test]
fn a_frame_pointing_outside_its_data_is_refused() {
    let Some(mut dev) = device() else { return };
    let (v, i) = quad(0.5);
    dev.set_scene_mesh(0, &v, &i).unwrap();
    dev.begin_frame(&FrameDesc {
        width: W,
        height: H,
        clear_rgba: [0.0; 4],
    })
    .unwrap();
    let u = uniform(0.0);
    let mut b = batch(DrawKind::Opaque, 0, 0, 0..1);
    b.count = 9;
    let frame = SceneFrame {
        view_proj: IDENTITY,
        light: light([0.0; 4]),
        list: &[0],
        models: &[IDENTITY],
        params: &[VISIBLE, SECOND],
        main: std::slice::from_ref(&b),
        cab: &[],
        surface_bias: 0,
    };
    let material = |_| {
        Some(SceneMaterial {
            texture: None,
            transmap: None,
            uniform: &u,
        })
    };
    assert!(matches!(
        dev.draw_scene(&frame, material),
        Err(BackendError::InvalidArgument(_))
    ));
    // the frame is still usable afterwards
    dev.end_frame().unwrap();
}

//! Device tests of the scene layer, skipped like the others without a usable device.

use super::tests::device;
use super::*;
use crate::scene::{
    env_factor, DrawKind, SceneBatch, SceneFrame, SceneLamps, SceneLight, SceneMaterial,
    SceneTexture, TexAddress, TexFormat, BLOB_SURFACE_CODE, MATERIAL_VECTORS,
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
        lamps: Default::default(),
    };
    dev.draw_scene(&frame, |m| {
        materials.get(m).map(|u| SceneMaterial {
            texture,
            uniform: u,
            ..Default::default()
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
        lamps: Default::default(),
    };
    dev.draw_scene(&frame, |m| {
        u.get(m).map(|uniform| SceneMaterial {
            texture: Some(m),
            uniform,
            ..Default::default()
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
    dev.clear_scene();
    assert!(!dev.has_scene_texture(8));
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
        lamps: Default::default(),
    };
    let material = |_| {
        Some(SceneMaterial {
            uniform: &u,
            ..Default::default()
        })
    };
    assert!(matches!(
        dev.draw_scene(&frame, material),
        Err(BackendError::InvalidArgument(_))
    ));
    // the frame is still usable afterwards
    dev.end_frame().unwrap();
}

#[test]
fn unlit_materials_take_the_texture_times_their_colour_in_linear_light() {
    let Some(mut dev) = device() else { return };
    let (v, i) = quad(0.5);
    dev.set_scene_mesh(0, &v, &i).unwrap();
    rgba(&mut dev, 0, [200, 200, 200, 255]);
    let mut u = uniform(0.0);
    u[0] = [0.5, 0.5, 0.5, 1.0];
    u[1][1] = 1.0; // unlit
    let mut dark = light([0.0; 4]);
    dark.ambient = [0.0; 4];
    let px = draw(
        &mut dev,
        &[batch(DrawKind::Opaque, 0, 0, 0..1)],
        &[IDENTITY],
        &[VISIBLE, SECOND],
        &[0],
        dark,
        &[u],
        Some(0),
    );
    let decode = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let encode = |c: f32| {
        if c <= 0.0031308 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    };
    let want = (encode(decode(200.0 / 255.0) * 0.5) * 255.0).round() as u8;
    assert!(
        all(&px, [want, want, want, 255], 2),
        "{want}: {:?}",
        &px[..4]
    );
}

#[test]
fn culling_drops_the_triangles_counter_clockwise_on_screen() {
    let Some(mut dev) = device() else { return };
    let (v, ccw) = quad(0.5);
    // the quad's indices run counter-clockwise as seen; reversed they run clockwise
    let cw: Vec<u32> = ccw.chunks(3).flat_map(|t| [t[0], t[2], t[1]]).collect();
    dev.set_scene_mesh(0, &v, &ccw).unwrap();
    dev.set_scene_mesh(1, &v, &cw).unwrap();
    rgba(&mut dev, 0, [255, 255, 255, 255]);
    for (mesh, shown) in [(0, false), (1, true)] {
        let mut b = batch(DrawKind::Opaque, mesh, 0, 0..1);
        b.cull = true;
        let px = draw(
            &mut dev,
            &[b],
            &[IDENTITY],
            &[VISIBLE, SECOND],
            &[0],
            light([0.0; 4]),
            &[uniform(0.0)],
            Some(0),
        );
        let want = if shown { [255; 4] } else { [0, 0, 0, 255] };
        assert!(all(&px, want, 1), "mesh {mesh}: {:?}", &px[..4]);
    }
}

#[test]
fn depth_only_draws_hide_what_is_behind_without_colour() {
    let Some(mut dev) = device() else { return };
    let (near_v, i) = quad(0.8);
    let (far_v, _) = quad(0.2);
    dev.set_scene_mesh(0, &near_v, &i).unwrap();
    dev.set_scene_mesh(1, &far_v, &i).unwrap();
    rgba(&mut dev, 0, [255, 255, 255, 255]);
    let px = draw(
        &mut dev,
        &[
            batch(DrawKind::SurfaceDepth, 0, 0, 0..1),
            batch(DrawKind::Opaque, 1, 0, 0..1),
        ],
        &[IDENTITY],
        &[VISIBLE, SECOND],
        &[0],
        light([0.0; 4]),
        &[uniform(0.0)],
        Some(0),
    );
    assert!(all(&px, [0, 0, 0, 255], 0), "{:?}", &px[..4]);
}

#[test]
fn a_device_opened_again_gets_the_scene_back() {
    let Some(mut dev) = device() else { return };
    let (v, i) = quad(0.5);
    dev.set_scene_mesh(4, &v, &i).unwrap();
    dev.set_scene_mesh(6, &v, &i).unwrap();
    dev.move_scene_mesh(6, 2);
    dev.free_scene_mesh(4);
    rgba(&mut dev, 1, [10, 200, 30, 255]);
    dev.recover().unwrap();
    assert!(dev.has_scene_mesh(2) && !dev.has_scene_mesh(4) && !dev.has_scene_mesh(6));
    assert!(dev.has_scene_texture(1));
    let px = draw(
        &mut dev,
        &[batch(DrawKind::Opaque, 2, 0, 0..1)],
        &[IDENTITY],
        &[VISIBLE, SECOND],
        &[0],
        light([0.0; 4]),
        &[uniform(0.0)],
        Some(1),
    );
    assert!(all(&px, [10, 200, 30, 255], 1), "{:?}", &px[..4]);
    dev.clear_scene();
    dev.recover().unwrap();
    assert!(!dev.has_scene_mesh(2) && !dev.has_scene_texture(1));
}

/// Draws one opaque quad of mesh 0 with `material`, its instance's second vector `second`.
fn draw_one(
    dev: &mut D3d9Device,
    material: SceneMaterial,
    second: [f32; 4],
    light: SceneLight,
) -> Vec<u8> {
    draw_lamps(dev, material, second, light, SceneLamps::default())
}

/// As [`draw_one`], under `lamps`.
fn draw_lamps(
    dev: &mut D3d9Device,
    material: SceneMaterial,
    second: [f32; 4],
    light: SceneLight,
    lamps: SceneLamps,
) -> Vec<u8> {
    let (v, i) = quad(0.5);
    dev.set_scene_mesh(0, &v, &i).unwrap();
    dev.begin_frame(&FrameDesc {
        width: W,
        height: H,
        clear_rgba: [0.0, 0.0, 0.0, 1.0],
    })
    .unwrap();
    let frame = SceneFrame {
        view_proj: IDENTITY,
        light,
        list: &[0],
        models: &[IDENTITY],
        params: &[VISIBLE, second],
        main: &[batch(DrawKind::Opaque, 0, 0, 0..1)],
        cab: &[],
        surface_bias: -24,
        lamps,
    };
    dev.draw_scene(&frame, |_| Some(material)).unwrap();
    dev.end_frame().unwrap()
}

fn dark() -> SceneLight {
    let mut l = light([0.0; 4]);
    l.ambient = [0.0; 4];
    l
}

#[test]
fn night_maps_show_by_night_or_always_when_switched() {
    let Some(mut dev) = device() else { return };
    rgba(&mut dev, 1, [40, 120, 220, 255]);
    let mut u = uniform(0.0);
    u[2][3] = 1.0; // has a night map
    let mut night = dark();
    night.sun_color[3] = 1.0;
    fn m(u: &[[f32; 4]; MATERIAL_VECTORS]) -> SceneMaterial<'_> {
        SceneMaterial {
            uniform: u,
            nightmap: Some(1),
            ..Default::default()
        }
    }
    // (black under no light: what shows is the night map alone)
    let lit = [0.0, 1.0, 0.0, 0.0];
    let px = draw_one(&mut dev, m(&u), lit, night);
    assert!(all(&px, [40, 120, 220, 255], 2), "night: {:?}", &px[..4]);
    let px = draw_one(&mut dev, m(&u), lit, dark());
    assert!(all(&px, [0, 0, 0, 255], 1), "day: {:?}", &px[..4]);
    // an instance whose lights are off
    let px = draw_one(&mut dev, m(&u), SECOND, night);
    assert!(all(&px, [0, 0, 0, 255], 1), "off: {:?}", &px[..4]);
    u[2][3] = 2.0; // switched: by day too
    let px = draw_one(&mut dev, m(&u), SECOND, dark());
    assert!(all(&px, [40, 120, 220, 255], 2), "switched: {:?}", &px[..4]);
}

#[test]
fn light_maps_light_the_texture_as_strong_as_the_instance_says() {
    let Some(mut dev) = device() else { return };
    rgba(&mut dev, 0, [255, 255, 255, 255]);
    rgba(&mut dev, 1, [100, 150, 200, 255]);
    let mut u = uniform(0.0);
    u[3][0] = 1.0; // has a light map
    let m = SceneMaterial {
        texture: Some(0),
        lightmap: Some(1),
        uniform: &u,
        ..Default::default()
    };
    let px = draw_one(&mut dev, m, [1.0, 0.0, 0.0, 0.0], dark());
    assert!(all(&px, [100, 150, 200, 255], 2), "{:?}", &px[..4]);
    let px = draw_one(&mut dev, m, SECOND, dark());
    assert!(all(&px, [0, 0, 0, 255], 1), "none: {:?}", &px[..4]);
}

#[test]
fn sphere_maps_mix_in_by_their_factor_or_their_mask() {
    let Some(mut dev) = device() else { return };
    rgba(&mut dev, 0, [255, 0, 0, 255]);
    rgba(&mut dev, 1, [0, 0, 255, 255]);
    let mut u = uniform(0.0);
    u[3][1] = 0.5; // the reflection factor
    let mut m = SceneMaterial {
        texture: Some(0),
        envmap: Some(1),
        uniform: &u,
        ..Default::default()
    };
    // the factor x 255, truncated: 127 of 255 of the reflection
    let px = draw_one(&mut dev, m, SECOND, light([0.0; 4]));
    assert!(all(&px, [128, 0, 127, 255], 2), "factor: {:?}", &px[..4]);
    // a sphere map that is not there reflects nothing
    m.envmap = Some(9);
    let px = draw_one(&mut dev, m, SECOND, light([0.0; 4]));
    assert!(all(&px, [255, 0, 0, 255], 1), "missing: {:?}", &px[..4]);
    // masked: the mask's alpha times the sun's light
    let mut masked = u;
    masked[3][3] = 1.0;
    let mut sun = light([0.0; 4]);
    sun.sun_color = [1.0, 1.0, 1.0, 0.0];
    for (alpha, want) in [(0u8, [255, 0, 0, 255]), (255, [0, 0, 255, 255])] {
        rgba(&mut dev, 2, [0, 0, 0, alpha]);
        let m = SceneMaterial {
            texture: Some(0),
            envmap: Some(1),
            env_mask: Some(2),
            uniform: &masked,
            ..Default::default()
        };
        let px = draw_one(&mut dev, m, SECOND, sun);
        assert!(all(&px, want, 2), "mask {alpha}: {:?}", &px[..4]);
    }
}

#[test]
fn the_specular_highlight_is_added_to_the_lit_colour() {
    let Some(mut dev) = device() else { return };
    rgba(&mut dev, 0, [0, 0, 0, 255]);
    let mut u = uniform(0.0);
    u[5] = [1.0, 1.0, 1.0, 8.0];
    // the camera straight in front of the face, the light from above on it
    let mut l = dark();
    l.cam_pos = [0.0, 0.0, 10.0, 0.0];
    l.sky_color = [1.0, 1.0, 1.0, 1.0];
    let m = SceneMaterial {
        texture: Some(0),
        uniform: &u,
        ..Default::default()
    };
    let px = draw_one(&mut dev, m, SECOND, l);
    assert!(all(&px, [255, 255, 255, 255], 8), "{:?}", &px[..4]);
    u[5][3] = 0.0; // no highlight
    let m = SceneMaterial {
        texture: Some(0),
        uniform: &u,
        ..Default::default()
    };
    let px = draw_one(&mut dev, m, SECOND, l);
    assert!(all(&px, [0, 0, 0, 255], 1), "none: {:?}", &px[..4]);
}

#[test]
fn texture_addressing_follows_the_material() {
    let Some(mut dev) = device() else { return };
    // left column red, right column blue
    let row = [[255, 0, 0, 255], [0, 0, 255, 255]].concat();
    let levels = vec![[row.clone(), row].concat()];
    dev.set_scene_texture(
        0,
        &SceneTexture {
            width: 2,
            height: 2,
            format: TexFormat::Rgba8,
            levels: &levels,
            make_mips: false,
        },
    )
    .unwrap();
    let plain = uniform(0.0);
    // [matl_texadress_mirroronce]: the uniform's flags say so as well (the shader mirrors)
    let mut once = uniform(0.0);
    once[8][1] = 2.0;
    let draw = |dev: &mut D3d9Device, address, u: &[[f32; 4]; MATERIAL_VECTORS], shift: f32| {
        let shifted = [1.0, 1.0, shift, 0.0];
        let (v, i) = quad(0.5);
        dev.set_scene_mesh(0, &v, &i).unwrap();
        dev.begin_frame(&FrameDesc {
            width: W,
            height: H,
            clear_rgba: [0.0, 0.0, 0.0, 1.0],
        })
        .unwrap();
        let frame = SceneFrame {
            view_proj: IDENTITY,
            light: light([0.0; 4]),
            list: &[0],
            models: &[IDENTITY],
            params: &[shifted, SECOND],
            main: &[batch(DrawKind::Opaque, 0, 0, 0..1)],
            cab: &[],
            surface_bias: -24,
            lamps: Default::default(),
        };
        dev.draw_scene(&frame, |_| {
            Some(SceneMaterial {
                texture: Some(0),
                address,
                uniform: u,
                ..Default::default()
            })
        })
        .unwrap();
        dev.end_frame().unwrap()
    };
    // the texture moved by 3/4: u runs 0.75..1.75
    let px = draw(&mut dev, TexAddress::Clamp, &plain, 0.75);
    assert!(all(&px, [0, 0, 255, 255], 2), "clamped: {px:?}");
    // repeated, the middle of the quad reads the next repeat's left column
    let px = draw(&mut dev, TexAddress::Wrap, &plain, 0.75);
    let at = (4 * 4) as usize;
    assert!(px[at] > px[at + 2], "wrapped: {:?}", &px[at..at + 4]);
    // moved back by 3/4: u runs -0.75..0.25; the left edge reads the right column mirrored
    // once, and clamped it reads the left one
    let px = draw(&mut dev, TexAddress::MirrorOnce, &once, -0.75);
    assert!(px[2] > px[0], "mirrored once: {:?}", &px[..4]);
    let px = draw(&mut dev, TexAddress::Clamp, &plain, -0.75);
    assert!(px[0] > px[2], "clamped below 0: {:?}", &px[..4]);
}

/// A light above the middle of the quad: `pos` xyz and radius, white, then `dir`, `extra`.
fn lamp(pos: [f32; 4], dir: [f32; 4], extra: [f32; 4]) -> [[f32; 4]; 4] {
    [pos, [1.0, 1.0, 1.0, 1.0], dir, extra]
}

const POINT: [f32; 4] = [0.0, 0.0, -1.0, -2.0];
/// One cell round the quad, two places.
const ONE_CELL: [f32; 4] = [-10.0, -10.0, 25.0, 1.0];

fn white_lit(u: &[[f32; 4]; MATERIAL_VECTORS]) -> SceneMaterial<'_> {
    SceneMaterial {
        texture: Some(0),
        uniform: u,
        ..Default::default()
    }
}

#[test]
fn the_maps_lamps_light_what_takes_them() {
    let Some(mut dev) = device() else { return };
    rgba(&mut dev, 0, [255, 255, 255, 255]);
    let lights = lamp([0.0, 0.0, 1.5, 100.0], POINT, [0.0; 4]);
    let lamps = SceneLamps {
        lights: &lights,
        grid: &[0, u32::MAX],
        place: ONE_CELL,
        ..Default::default()
    };
    let u = uniform(0.0);
    let px = draw_lamps(&mut dev, white_lit(&u), SECOND, dark(), lamps);
    assert!(all(&px, [255, 255, 255, 255], 1), "lit: {:?}", &px[..4]);
    // [nomaplighting]
    let mut no_map = uniform(0.0);
    no_map[1][1] = 0.25;
    let px = draw_lamps(&mut dev, white_lit(&no_map), SECOND, dark(), lamps);
    assert!(all(&px, [0, 0, 0, 255], 1), "nomaplighting: {:?}", &px[..4]);
    // a vehicle's light lights it all the same
    let headlight = lamp([0.0, 0.0, 1.5, 100.0], [1.0, 0.0, 0.0, -2.0], [0.0; 4]);
    let px = draw_lamps(
        &mut dev,
        white_lit(&no_map),
        SECOND,
        dark(),
        SceneLamps {
            lights: &headlight,
            ..lamps
        },
    );
    assert!(all(&px, [255, 255, 255, 255], 1), "vehicle: {:?}", &px[..4]);
    // the lamp out of the grid's cell: no light
    let px = draw_lamps(
        &mut dev,
        white_lit(&u),
        SECOND,
        dark(),
        SceneLamps {
            grid: &[u32::MAX, u32::MAX],
            ..lamps
        },
    );
    assert!(
        all(&px, [0, 0, 0, 255], 1),
        "not in the cell: {:?}",
        &px[..4]
    );
    // out of reach
    let far = lamp([0.0, 0.0, 1.5, 0.5], POINT, [0.0; 4]);
    let px = draw_lamps(
        &mut dev,
        white_lit(&u),
        SECOND,
        dark(),
        SceneLamps {
            lights: &far,
            ..lamps
        },
    );
    assert!(all(&px, [0, 0, 0, 255], 1), "out of reach: {:?}", &px[..4]);
}

#[test]
fn spots_light_only_inside_their_cone() {
    let Some(mut dev) = device() else { return };
    rgba(&mut dev, 0, [255, 255, 255, 255]);
    let u = uniform(0.0);
    for (down, lit) in [(true, true), (false, false)] {
        let z = if down { -1.0 } else { 1.0 };
        let lights = lamp(
            [0.0, 0.0, 1.5, 100.0],
            [0.0, 0.0, z, 0.5],
            [0.9, 0.0, 0.0, 0.0],
        );
        let px = draw_lamps(
            &mut dev,
            white_lit(&u),
            SECOND,
            dark(),
            SceneLamps {
                lights: &lights,
                grid: &[0, u32::MAX],
                place: ONE_CELL,
                ..Default::default()
            },
        );
        // the middle of the quad, straight under the light
        let at = ((2 * W + 4) * 4) as usize;
        let want = if lit { 255 } else { 0 };
        assert!(
            px[at].abs_diff(want) <= 1,
            "down {down}: {:?}",
            &px[at..at + 4]
        );
    }
}

#[test]
fn interior_lamps_light_the_meshes_that_name_them() {
    let Some(mut dev) = device() else { return };
    rgba(&mut dev, 0, [255, 255, 255, 255]);
    let u = uniform(0.0);
    // one lamp, core 1 m, reaching 10 m, 0.7 m over the quad's middle: full light there
    let lights = lamp([0.0, 0.0, 1.2, 0.0], POINT, [0.0, 1.0, 0.0, 10.0]);
    let lamps = SceneLamps {
        lights: &lights,
        ..Default::default()
    };
    let px = draw_lamps(&mut dev, white_lit(&u), [0.0, 0.0, 1.0, 0.0], dark(), lamps);
    let at = ((2 * W + 4) * 4) as usize;
    assert!(px[at] >= 250, "middle: {:?}", &px[at..at + 4]);
    // an instance that names none
    let px = draw_lamps(&mut dev, white_lit(&u), SECOND, dark(), lamps);
    assert!(all(&px, [0, 0, 0, 255], 1), "none: {:?}", &px[..4]);
    // a plain brightness (below 1), no light needed
    let px = draw_one(&mut dev, white_lit(&u), [0.0, 0.0, 0.5, 0.0], dark());
    assert!(
        all(&px, [128, 122, 107, 255], 2),
        "brightness: {:?}",
        &px[..4]
    );
}

#[test]
fn reflections_take_the_lamps_light_and_spill_their_factor() {
    let Some(mut dev) = device() else { return };
    rgba(&mut dev, 0, [255, 0, 0, 255]);
    rgba(&mut dev, 1, [0, 0, 255, 255]);
    rgba(&mut dev, 2, [0, 0, 0, 255]);
    let mut masked = uniform(0.0);
    masked[3][1] = 0.5;
    masked[3][3] = 1.0;
    let m = SceneMaterial {
        texture: Some(0),
        envmap: Some(1),
        env_mask: Some(2),
        uniform: &masked,
        ..Default::default()
    };
    // without the sun the mask reflects by the lamps' light alone
    let lights = lamp([0.0, 0.0, 1.5, 100.0], POINT, [0.0; 4]);
    let px = draw_lamps(
        &mut dev,
        m,
        SECOND,
        light([0.0; 4]),
        SceneLamps {
            lights: &lights,
            grid: &[0, u32::MAX],
            place: ONE_CELL,
            ..Default::default()
        },
    );
    assert!(all(&px, [0, 0, 255, 255], 2), "lamp lit: {:?}", &px[..4]);
    let px = draw_one(&mut dev, m, SECOND, light([0.0; 4]));
    assert!(all(&px, [255, 0, 0, 255], 2), "dark: {:?}", &px[..4]);
    // a factor of 10 spills into its neighbours as the packed colour does
    let mut strong = uniform(0.0);
    strong[3][1] = 10.0;
    let k = env_factor(10.0, [1.0; 3]);
    let want = [
        ((1.0 - k[0]) * 255.0).round() as u8,
        0,
        (k[2] * 255.0).round() as u8,
        255,
    ];
    let m = SceneMaterial {
        texture: Some(0),
        envmap: Some(1),
        uniform: &strong,
        ..Default::default()
    };
    let px = draw_one(&mut dev, m, SECOND, light([0.0; 4]));
    assert!(all(&px, want, 2), "{want:?}: {:?}", &px[..4]);
}

#[test]
fn tile_light_maps_light_their_objects_by_night() {
    let Some(mut dev) = device() else { return };
    rgba(&mut dev, 0, [255, 255, 255, 255]);
    // an atlas of 2 by 2 tiles of one pixel: the top left one white, the top right red
    assert!(!dev.has_tile_light_map());
    dev.set_tile_light_map(2, 1, (0, 0), &[255, 255, 255, 255])
        .unwrap();
    dev.set_tile_light_map(2, 1, (1, 0), &[255, 0, 0, 255])
        .unwrap();
    assert!(dev.has_tile_light_map());
    // (outside the atlas, or not one tile of pixels: refused)
    assert!(dev.set_tile_light_map(2, 1, (2, 0), &[0; 4]).is_err());
    assert!(dev.set_tile_light_map(2, 1, (0, 1), &[0; 8]).is_err());
    let mut night = dark();
    night.sun_color[3] = 1.0;
    // the atlas lies over the quad, 2 m a side
    let over = SceneLamps {
        tile_map: [-1.0, -1.0, 2.0, 1.0],
        ..Default::default()
    };
    // ([LightMapMapping]: params.y 0.35)
    let mut mapped = uniform(0.0);
    mapped[1][1] = 0.35;
    let check = |px: &[u8], what: &str| {
        let at = |x: u32, y: u32| ((y * W + x) * 4) as usize;
        assert!(
            px[at(0, 0)..at(0, 0) + 3].iter().all(|&c| c >= 253),
            "{what} top left: {:?}",
            &px[at(0, 0)..at(0, 0) + 4]
        );
        let r = &px[at(W - 1, 0)..at(W - 1, 0) + 4];
        assert!(
            r[0] >= 253 && r[1] <= 2 && r[2] <= 2,
            "{what} top right: {r:?}"
        );
        let b = &px[at(0, H - 1)..at(0, H - 1) + 4];
        assert!(b[..3].iter().all(|&c| c <= 2), "{what} bottom: {b:?}");
    };
    let px = draw_lamps(&mut dev, white_lit(&mapped), SECOND, night, over);
    check(&px, "night");
    // by day, on what does not take it, and away from the loaded square: nothing
    let px = draw_lamps(&mut dev, white_lit(&mapped), SECOND, dark(), over);
    assert!(all(&px, [0, 0, 0, 255], 1), "day: {:?}", &px[..4]);
    let plain = uniform(0.0);
    let px = draw_lamps(&mut dev, white_lit(&plain), SECOND, night, over);
    assert!(all(&px, [0, 0, 0, 255], 1), "plain: {:?}", &px[..4]);
    let away = SceneLamps {
        tile_map: [5.0, 5.0, 2.0, 1.0],
        ..Default::default()
    };
    let px = draw_lamps(&mut dev, white_lit(&mapped), SECOND, night, away);
    assert!(all(&px, [0, 0, 0, 255], 1), "away: {:?}", &px[..4]);
    // a device opened again gets the atlas back; a new scene drops it
    dev.recover().unwrap();
    let px = draw_lamps(&mut dev, white_lit(&mapped), SECOND, night, over);
    check(&px, "recovered");
    dev.clear_scene();
    assert!(!dev.has_tile_light_map());
    let px = draw_lamps(&mut dev, white_lit(&mapped), SECOND, night, over);
    assert!(all(&px, [0, 0, 0, 255], 1), "cleared: {:?}", &px[..4]);
}

/// A frame of one opaque quad textured with scene texture `texture`, under ambient light.
fn quad_frame<'a>(
    batches: &'a [SceneBatch],
    models: &'a [[[f32; 4]; 4]],
    params: &'a [[f32; 4]],
    list: &'a [u32],
) -> SceneFrame<'a> {
    SceneFrame {
        view_proj: IDENTITY,
        light: light([0.0; 4]),
        list,
        models,
        params,
        main: batches,
        cab: &[],
        surface_bias: -24,
        lamps: Default::default(),
    }
}

#[test]
fn a_scene_drawn_into_a_texture_is_what_a_later_material_shows() {
    let Some(mut dev) = device() else { return };
    let (v, i) = quad(0.5);
    dev.set_scene_mesh(3, &v, &i).unwrap();
    rgba(&mut dev, 5, [200, 100, 50, 255]);
    let batches = [batch(DrawKind::Opaque, 3, 0, 0..1)];
    let models = [IDENTITY];
    let params = [VISIBLE, SECOND];
    let list = [0u32];
    let materials = [uniform(0.0)];
    let into = |dev: &mut D3d9Device| {
        let frame = quad_frame(&batches, &models, &params, &list);
        dev.draw_scene_into(9, (4, 4), [0.0, 0.0, 0.0, 1.0], &frame, |m| {
            materials.get(m).map(|u| SceneMaterial {
                texture: Some(5),
                uniform: u,
                ..Default::default()
            })
        })
    };
    // outside any frame, then inside one (the window's picture follows each time)
    into(&mut dev).unwrap();
    let shown = draw(
        &mut dev,
        &batches,
        &models,
        &params,
        &list,
        light([0.0; 4]),
        &materials,
        Some(9),
    );
    assert!(all(&shown, [200, 100, 50, 255], 2), "{:?}", &shown[..4]);

    dev.begin_frame(&FrameDesc {
        width: W,
        height: H,
        clear_rgba: [0.0, 0.0, 1.0, 1.0],
    })
    .unwrap();
    // a texture of another size is made again
    rgba(&mut dev, 5, [10, 220, 30, 255]);
    let frame = quad_frame(&batches, &models, &params, &list);
    dev.draw_scene_into(9, (2, 2), [0.0, 0.0, 0.0, 1.0], &frame, |m| {
        materials.get(m).map(|u| SceneMaterial {
            texture: Some(5),
            uniform: u,
            ..Default::default()
        })
    })
    .unwrap();
    dev.draw_scene(&frame, |m| {
        materials.get(m).map(|u| SceneMaterial {
            texture: Some(9),
            uniform: u,
            ..Default::default()
        })
    })
    .unwrap();
    let px = dev.end_frame().unwrap();
    assert!(all(&px, [10, 220, 30, 255], 2), "{:?}", &px[..4]);
}

#[test]
fn a_scene_target_of_no_size_is_refused() {
    let Some(mut dev) = device() else { return };
    let frame = quad_frame(&[], &[], &[], &[]);
    let r = dev.draw_scene_into(0, (0, 4), [0.0; 4], &frame, |_| None);
    assert!(matches!(r, Err(BackendError::InvalidArgument(_))));
}

/// Draws a white quad and a black one over it with texture 1 (`format`, `level`) on a blending
/// material, the black one with surface code `code`; returns the first pixel.
fn black_over_white(dev: &mut D3d9Device, format: TexFormat, level: Vec<u8>, code: f32) -> [u8; 4] {
    let levels = vec![level];
    dev.set_scene_texture(
        1,
        &SceneTexture {
            width: 4,
            height: 4,
            format,
            levels: &levels,
            make_mips: false,
        },
    )
    .unwrap();
    dev.begin_frame(&FrameDesc {
        width: W,
        height: H,
        clear_rgba: [0.0, 0.0, 0.0, 1.0],
    })
    .unwrap();
    let u = [uniform(0.0), uniform(2.0)];
    let mut over = batch(DrawKind::BlendNoWrite, 1, 1, 1..2);
    over.surface = true;
    let batches = [batch(DrawKind::Opaque, 0, 0, 0..1), over];
    let frame = SceneFrame {
        view_proj: IDENTITY,
        light: light([0.0; 4]),
        list: &[0, 1],
        models: &[IDENTITY, IDENTITY],
        params: &[VISIBLE, SECOND, VISIBLE, [0.0, 0.0, 0.0, code]],
        main: &batches,
        cab: &[],
        surface_bias: -24,
        lamps: Default::default(),
    };
    dev.draw_scene(&frame, |m| {
        u.get(m).map(|uniform| SceneMaterial {
            texture: Some(m),
            uniform,
            ..Default::default()
        })
    })
    .unwrap();
    dev.end_frame().unwrap()[..4].try_into().unwrap()
}

#[test]
fn a_shadow_blob_shows_by_its_texture_alpha_and_not_as_a_solid_box() {
    let Some(mut dev) = device() else { return };
    let (far_v, i) = quad(0.2);
    let (near_v, _) = quad(0.8);
    dev.set_scene_mesh(0, &far_v, &i).unwrap();
    dev.set_scene_mesh(1, &near_v, &i).unwrap();
    rgba(&mut dev, 0, [255, 255, 255, 255]);
    let white = [255, 255, 255, 255];
    let black = [0, 0, 0, 255];
    // clear black texels in each format the pictures come in: the road shows through (DXT1's
    // transparent texel is index 3 with the end colours in order)
    let bc1_clear = vec![0u8, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF];
    let clear = [
        (TexFormat::Rgba8, [0u8, 0, 0, 0].repeat(16)),
        (TexFormat::Bc1, bc1_clear),
        (TexFormat::Bc2, vec![0u8; 16]),
        (TexFormat::Bc3, vec![0u8; 16]),
    ];
    for (format, level) in clear {
        let px = black_over_white(&mut dev, format, level, BLOB_SURFACE_CODE);
        assert!(all(&px, white, 3), "{format:?}: {px:?}");
    }
    // a picture without alpha is a solid black box: left out for a blob, drawn for anything else
    let bc2_solid = [vec![0xFFu8; 8], vec![0u8; 8]].concat();
    let bc3_solid = [vec![0xFFu8, 0xFF, 0, 0, 0, 0, 0, 0], vec![0u8; 8]].concat();
    let solid = [
        (TexFormat::Rgba8, [0u8, 0, 0, 255].repeat(16)),
        (TexFormat::Bc1, vec![0u8; 8]),
        (TexFormat::Bc2, bc2_solid),
        (TexFormat::Bc3, bc3_solid),
    ];
    for (format, level) in solid {
        let px = black_over_white(&mut dev, format, level.clone(), BLOB_SURFACE_CODE);
        assert!(all(&px, white, 3), "{format:?} blob: {px:?}");
        let px = black_over_white(&mut dev, format, level, 1.0);
        assert!(all(&px, black, 3), "{format:?} surface: {px:?}");
    }
}

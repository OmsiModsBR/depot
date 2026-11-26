use omsi_render_vanilla::backend::VERTEX_FLOATS;
use omsi_render_vanilla::testing::domain::scene::*;
use omsi_render_vanilla::BackendError;
use std::ops::Range;

#[test]
fn a_texture_too_large_is_brought_within_the_limit() {
    // a chain leaves its top levels out
    let chain: Vec<Vec<u8>> = (0..mip_count(8, 4))
        .map(|i| {
            let (w, h) = level_size(8, 4, i);
            vec![i as u8; TexFormat::Bc1.level_bytes(w, h)]
        })
        .collect();
    let mut t = SceneTexture {
        width: 8,
        height: 4,
        format: TexFormat::Bc1,
        levels: &chain,
        make_mips: false,
    };
    assert!(t.fitted(8).is_none());
    let (w, h, levels) = t.fitted(2).unwrap();
    assert_eq!((w, h, levels.len()), (2, 1, 2));
    assert_eq!(levels[0][0], 2);
    // a single compressed level cannot be made smaller
    t.levels = &chain[..1];
    assert!(t.fitted(4).is_none());
    // a single RGBA image is halved: white and black columns to grey (in linear light)
    let image = vec![[[255, 255, 255, 255], [0, 0, 0, 255]].concat().repeat(2)];
    let t = SceneTexture {
        width: 2,
        height: 2,
        format: TexFormat::Rgba8,
        levels: &image,
        make_mips: true,
    };
    let (w, h, levels) = t.fitted(1).unwrap();
    assert_eq!((w, h), (1, 1));
    assert_eq!(levels, vec![vec![188, 188, 188, 255]]);
}

#[test]
fn level_sizes_follow_the_format() {
    assert_eq!(TexFormat::Rgba8.level_bytes(3, 2), 24);
    assert_eq!(TexFormat::Bc1.level_bytes(8, 8), 32);
    assert_eq!(TexFormat::Bc1.level_bytes(1, 1), 8);
    assert_eq!(TexFormat::Bc3.level_bytes(6, 4), 32);
    assert_eq!(TexFormat::Bc3.rows(8, 8), (32, 2));
    assert_eq!(TexFormat::Rgba8.rows(3, 2), (12, 2));
    assert_eq!(mip_count(1, 1), 1);
    assert_eq!(mip_count(256, 64), 9);
    assert_eq!(level_size(256, 64, 7), (2, 1));
}

#[test]
fn textures_are_checked_level_by_level() {
    let levels = vec![vec![0u8; 64], vec![0u8; 16], vec![0u8; 4]];
    let t = SceneTexture {
        width: 4,
        height: 4,
        format: TexFormat::Rgba8,
        levels: &levels,
        make_mips: false,
    };
    t.validate(2048, true).unwrap();
    let short = vec![vec![0u8; 64], vec![0u8; 15]];
    let bad = SceneTexture {
        levels: &short,
        ..t
    };
    assert!(matches!(
        bad.validate(2048, true),
        Err(BackendError::InvalidArgument(_))
    ));
    let one = vec![vec![0u8; 3 * 4 * 4]];
    let npot = SceneTexture {
        width: 3,
        levels: &one,
        ..t
    };
    assert!(matches!(
        npot.validate(2048, false),
        Err(BackendError::Unsupported(_))
    ));
    npot.validate(2048, true).unwrap();
}

#[test]
fn the_mip_chain_averages_down_to_one_texel() {
    // 2x2: black, white, white, black -> one mid grey in linear light (half the light is
    // 188 as stored, not 128); then nothing more.
    let rgba = [
        [0, 0, 0, 255],
        [255, 255, 255, 255],
        [255, 255, 255, 255],
        [0, 0, 0, 255],
    ]
    .concat();
    let chain = mip_chain(2, 2, &rgba);
    assert_eq!(chain, vec![vec![188, 188, 188, 255]]);
    // 3x1 -> 1x1 averages the first two texels (the odd one is left at this level).
    let row = [[0, 0, 0, 0], [100, 100, 100, 100], [200, 200, 200, 200]].concat();
    let chain = mip_chain(3, 1, &row);
    assert_eq!(chain.len(), 1);
    // (the colour is weighted by alpha: the transparent texel's black does not count)
    assert_eq!(chain[0], vec![100, 100, 100, 50]);
    // the chain of 8x2 has 8x2 -> 4x1 -> 2x1 -> 1x1 below level 0
    let chain = mip_chain(8, 2, &[7u8; 8 * 2 * 4]);
    let sizes: Vec<usize> = chain.iter().map(|l| l.len() / 4).collect();
    assert_eq!(sizes, vec![4, 2, 1]);
    assert!(chain.iter().flatten().all(|&c| c == 7));
}

#[test]
fn the_mip_chain_keeps_transparent_colour_out_of_the_silhouette() {
    // a green leaf texel beside three blue fully transparent ones stays green
    let rgba = [
        [0, 200, 0, 255],
        [0, 0, 255, 0],
        [0, 0, 255, 0],
        [0, 0, 255, 0],
    ]
    .concat();
    assert_eq!(mip_chain(2, 2, &rgba), vec![vec![0, 200, 0, 64]]);
    // all four transparent: the plain average, so a mask in alpha does not blacken colour
    let rgba = [[40, 80, 120, 0]; 4].concat();
    assert_eq!(mip_chain(2, 2, &rgba), vec![vec![40, 80, 120, 0]]);
}

#[test]
fn meshes_are_checked() {
    let v = vec![0.0f32; 3 * VERTEX_FLOATS];
    validate_mesh(&v, &[0, 1, 2], false).unwrap();
    assert!(validate_mesh(&v, &[0, 1, 3], false).is_err());
    assert!(validate_mesh(&v, &[0, 1], false).is_err());
    assert!(validate_mesh(&v[..7], &[0, 0, 0], false).is_err());
}

fn batch(instances: Range<u32>) -> SceneBatch {
    SceneBatch {
        kind: DrawKind::Opaque,
        cull: false,
        surface: false,
        mesh: 0,
        first: 0,
        count: 3,
        material: 0,
        instances,
    }
}

#[test]
fn the_reflection_factor_spills_as_a_packed_colour() {
    let k = env_factor(0.5, [1.0, 0.5, 0.0]);
    assert_eq!(k, [127.0 / 255.0, 63.0 / 255.0, 0.0]);
    // a factor of 10 at full light: 2550 a channel, OR-ed into its neighbours
    let k = env_factor(10.0, [1.0; 3]);
    let p = (0xffu32 << 24) | (2550 << 16) | (2550 << 8) | 2550;
    assert_eq!(
        k,
        [(p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff].map(|c| c as f32 / 255.0)
    );
}

#[test]
fn a_texture_has_alpha_when_a_texel_is_not_opaque() {
    let tex = |format, level: Vec<u8>| {
        let levels = [level];
        SceneTexture {
            width: 4,
            height: 4,
            format,
            levels: &levels,
            make_mips: false,
        }
        .alpha_in_use()
    };
    let px = |a: u8| [[0, 0, 0, 255].repeat(15), vec![0, 0, 0, a]].concat();
    assert!(!tex(TexFormat::Rgba8, px(255)));
    assert!(tex(TexFormat::Rgba8, px(254)));
    // DXT1: black all over, opaque; the same with one texel on the transparent index 3
    let solid = [0u8, 0, 0, 0, 0, 0, 0, 0];
    assert!(!tex(TexFormat::Bc1, solid.to_vec()));
    assert!(tex(TexFormat::Bc1, vec![0, 0, 0, 0, 0, 0, 0, 0xC0]));
    // (index 3 in four-colour mode is a colour)
    assert!(!tex(TexFormat::Bc1, vec![1, 0, 0, 0, 0, 0, 0, 0xC0]));
    assert!(!tex(TexFormat::Bc2, [vec![0xFF; 8], vec![0; 8]].concat()));
    assert!(tex(TexFormat::Bc2, [vec![0xFE; 8], vec![0; 8]].concat()));
    // DXT5: both end alphas 255, every index in use giving 255 (index 7 of the six-step
    // mode is 255 as well); one end below 255 and index 0 or 1 used is clear
    assert!(!tex(
        TexFormat::Bc3,
        [vec![255, 255, 0, 0, 0, 0, 0, 0], vec![0; 8]].concat()
    ));
    assert!(tex(
        TexFormat::Bc3,
        [vec![0, 255, 0, 0, 0, 0, 0, 0], vec![0; 8]].concat()
    ));
}

#[test]
fn instance_records_follow_the_list() {
    let models = [[[1.0, 0.0, 0.0, 0.0]; 4], [[2.0, 0.0, 0.0, 0.0]; 4]];
    let params = [[10.0; 4], [11.0; 4], [20.0; 4], [21.0; 4]];
    let main = [batch(0..2)];
    let frame = SceneFrame {
        view_proj: [[0.0; 4]; 4],
        light: SceneLight::default(),
        list: &[1, 0],
        models: &models,
        params: &params,
        main: &main,
        cab: &[],
        surface_bias: -24,
        lamps: SceneLamps::default(),
        coronas: SceneCoronas::default(),
        rain: Default::default(),
        sky: Default::default(),
        smoke: SceneSmoke::default(),
        rain_reflection: None,
    };
    frame.validate().unwrap();
    let mut out = vec![0.0; 2 * INSTANCE_FLOATS];
    frame.write_instance_records(&mut out);
    assert_eq!(out[0], 2.0);
    assert_eq!(out[16], 20.0);
    assert_eq!(out[20], 21.0);
    assert_eq!(out[INSTANCE_FLOATS], 1.0);
    assert_eq!(out[INSTANCE_FLOATS + 16], 10.0);

    let outside = [batch(1..3)];
    assert!(SceneFrame {
        main: &outside,
        ..frame
    }
    .validate()
    .is_err());
    assert!(SceneFrame {
        list: &[2],
        main: &[],
        ..frame
    }
    .validate()
    .is_err());
}

#[test]
fn a_corona_run_must_lie_in_the_sprite_list() {
    let sprites = [[0.0; CORONA_FLOATS]; 3];
    let ok = SceneCoronas {
        sprites: &sprites,
        runs: &[(0, 0, 2, false), (4, 2, 1, true)],
    };
    assert!(ok.validate().is_ok());
    let bad = SceneCoronas {
        sprites: &sprites,
        runs: &[(0, 2, 2, false)],
    };
    assert!(bad.validate().is_err());
}

#[test]
fn lights_are_packed_for_the_shader() {
    let spot = [
        [1.0, 2.0, 3.0, 16.0],
        [1.0, 0.5, 0.25, 2.0],
        [0.0, 0.0, -1.0, 0.7],
        [0.9, 0.5, 0.0, 4.0],
    ];
    assert_eq!(
        pack_light(&spot),
        [
            [1.0, 2.0, 3.0, 0.0],
            [2.0, 1.0, 0.5, 1.0 / 16.0],
            [0.0, 0.0, -1.0, 0.7],
            [0.9, 0.25, 4.0, 16.0],
        ]
    );
    // a vehicle's point light lights every surface; one passed by (radius 0) nothing,
    // an interior lamp without intensity reaches nothing
    let point = [
        [0.0; 4],
        [1.0, 1.0, 1.0, 0.0],
        [1.0, 0.0, 0.0, -2.0],
        [0.0, 0.0, 0.0, 3.0],
    ];
    let packed = pack_light(&point);
    assert_eq!(packed[0][3], 1.0);
    assert_eq!((packed[1][3], packed[3][2], packed[3][3]), (0.0, 0.0, 0.0));
    assert!((packed[3][1] - 1e-4).abs() < 1e-9, "{}", packed[3][1]);
}

#[test]
fn borders_and_env_bits_are_unpacked_for_the_shader() {
    assert_eq!(
        border_rgba([0.0, 1.0, (0x40_80_ff) as f32, 0.5]),
        [64.0 / 255.0, 128.0 / 255.0, 1.0, 0.5]
    );
    assert_eq!(border_rgba([0.0, 0.0, 12345.0, 1.0]), [0.0; 4]);
    assert_eq!(border_rgba([0.0, 2.0, 12345.0, 1.0]), [0.0; 4]);
    assert_eq!(env_bits(0.0), [0.0, 0.0]);
    assert_eq!(env_bits(1.0), [1.0, 0.0]);
    assert_eq!(env_bits(2.0), [0.0, 1.0]);
    assert_eq!(env_bits(3.0), [1.0, 1.0]);
}

#[test]
fn a_cell_cap_keeps_the_first_map_lamps_and_every_vehicle_light() {
    let map = [[0.0; 4], [0.0; 4], [0.0, 0.0, 0.0, -2.0], [0.0; 4]];
    // (a headlight is a spot: a direction and the cosine of its outer cone)
    let car = [[0.0; 4], [0.0; 4], [0.0, 0.0, -1.0, 0.5], [0.0; 4]];
    let old_car = [[0.0; 4], [0.0; 4], [1.0, 0.0, 0.0, -2.0], [0.0; 4]];
    // lights 0, 1, 2 map lamps, 3 a vehicle's, 4 a map lamp; one cell of 6 places
    let lights: Vec<[f32; 4]> = [map, map, map, car, map, old_car].concat();
    let grid = [0, 1, 2, 3, 4, 5, u32::MAX];
    let mut lamps = SceneLamps {
        lights: &lights,
        grid: &grid,
        place: [0.0, 0.0, 1.0, 1.0],
        ..Default::default()
    };
    let mut out = Vec::new();
    lamps.grid_texels(6, &mut out);
    assert_eq!(out, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, -1.0]);
    lamps.cell_cap = 2;
    lamps.grid_texels(6, &mut out);
    assert_eq!(out, [0.0, 1.0, 3.0, 5.0, -1.0, -1.0, -1.0]);
    // (lights past the first `kept` are gone, as without a cap)
    lamps.grid_texels(2, &mut out);
    assert_eq!(out, [0.0, 1.0, -1.0, -1.0, -1.0, -1.0, -1.0]);
}

#[test]
fn lamp_grids_are_checked_and_read_as_indices() {
    let lights = [[0.0; 4]; 8];
    let grid = [0, 1, u32::MAX, 5, u32::MAX, u32::MAX, 1, 0];
    let lamps = SceneLamps {
        lights: &lights,
        grid: &grid,
        place: [0.0, 0.0, 25.0, 2.0],
        ..Default::default()
    };
    lamps.validate().unwrap();
    assert_eq!(lamps.count(), 2);
    assert_eq!(lamps.grid_shape(), Some((2, 2)));
    let mut out = Vec::new();
    lamps.grid_texels(2, &mut out);
    assert_eq!(out, [0.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 0.0]);
    assert!(SceneLamps {
        grid: &grid[..7],
        ..lamps
    }
    .validate()
    .is_err());
    assert!(SceneLamps {
        lights: &lights[..7],
        ..lamps
    }
    .validate()
    .is_err());
    // no grid: no map lamps, the interior lamps still
    let none = SceneLamps {
        place: [0.0; 4],
        ..lamps
    };
    assert_eq!(none.grid_shape(), None);
    none.validate().unwrap();
}

#[test]
fn kinds_decide_blend_and_depth_write() {
    assert!(!DrawKind::Opaque.blends() && DrawKind::Opaque.writes_depth());
    assert!(DrawKind::Blend.blends() && DrawKind::Blend.writes_depth());
    assert!(DrawKind::BlendNoWrite.blends() && !DrawKind::BlendNoWrite.writes_depth());
    assert!(DrawKind::TerrainPaint.blends() && !DrawKind::TerrainPaint.writes_depth());
    assert!(!DrawKind::SurfaceDepth.writes_color());
}

#[test]
fn the_surface_bias_pulls_towards_the_eye_in_reversed_depth() {
    let (constant, slope) = surface_depth_bias(-24);
    assert!(constant > 0.0 && slope > 0.0);
    assert_eq!(constant, 24.0 / 16_777_216.0);
    assert_eq!(surface_depth_bias(0), (0.0, 0.0));
}

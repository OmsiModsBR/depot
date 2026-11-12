//! A `[matl_bumpmap]` on an envmapped material in the Enhanced picture moves the
//! reflection; it does not turn the surface rough. (The specular antialiasing read the bump
//! map's noise as curvature, and the envmap of a bump-mapped body all but vanished.)

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{AlphaMode, Camera, Lighting, MaterialExtra, RenderOptions, Renderer};
use omsi_texture::Image;

const SIZE: u32 = 32;

/// The roughness the Enhanced pass gives an envmapped board (`OMSI_DEBUG_ENHANCED=17`
/// shows it in the red channel), with a noisy bump map or without.
fn roughness(renderer: &mut Renderer, bump: bool) -> f32 {
    let mut scene = renderer.new_scene();
    let grey = renderer.add_texture(&mut scene, &Image::solid([128, 128, 128, 255]), false);
    // a height map of noise, at a factor of 1 (the panel lines of some bodies' maps)
    let mut seed = 12345u32;
    let noise: Vec<u8> = (0..64 * 64)
        .flat_map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let v = (seed >> 24) as u8;
            [v, v, v, 255]
        })
        .collect();
    let height = Image {
        width: 64,
        height: 64,
        rgba: noise,
        has_alpha: false,
    };
    let bump_map =
        renderer.add_texture_data(&mut scene, &omsi_texture::gpu::prepare_bump(&height, false));
    let paint = renderer.add_material_extra(
        &mut scene,
        None,
        AlphaMode::Opaque,
        [0.5, 0.5, 0.5, 1.0],
        false,
        None,
        None,
        None,
        Some((grey, 1.0)),
        [0.0; 3],
        MaterialExtra {
            bump: bump.then_some((bump_map, 1.0)),
            ..Default::default()
        },
    );
    let board = MeshData {
        positions: vec![
            Vec3::new(-1.0, 3.0, -1.0),
            Vec3::new(1.0, 3.0, -1.0),
            Vec3::new(1.0, 3.0, 1.0),
            Vec3::new(-1.0, 3.0, 1.0),
        ],
        normals: vec![-Vec3::Y; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
    };
    let mesh = renderer.add_mesh(&mut scene, &board);
    renderer.add_surface_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![paint]);
    let camera = Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 20.0,
        near: 0.1,
        far: 100.0,
    };
    let lighting = Lighting {
        enhanced: true,
        shadows: false,
        fog_density: 0.0,
        ..Default::default()
    };
    let img = renderer
        .render_to_image(&mut scene, SIZE, SIZE, &camera, &lighting)
        .unwrap();
    let c = SIZE / 2;
    let reds: Vec<f32> = (c - 4..c + 4)
        .flat_map(|y| (c - 4..c + 4).map(move |x| (x, y)))
        .map(|(x, y)| img[((y * SIZE + x) * 4) as usize] as f32)
        .collect();
    reds.iter().sum::<f32>() / reds.len() as f32
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_bump_map_keeps_an_envmapped_surface_smooth() {
    std::env::set_var("OMSI_DEBUG_ENHANCED", "17");
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let mut renderer = pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            ssao: false,
            fxaa: false,
            render_scale: 1.0,
            ..Default::default()
        },
    ))
    .expect("test renderer");
    let smooth = roughness(&mut renderer, false);
    let bumped = roughness(&mut renderer, true);
    eprintln!("roughness on the screen: {smooth} without the bump map, {bumped} with it");
    assert!(
        smooth < 120.0,
        "the envmapped board is not smooth to begin with: {smooth}"
    );
    assert!(
        (bumped - smooth).abs() < 12.0,
        "the bump map makes the surface rough: {smooth} without, {bumped} with"
    );
}

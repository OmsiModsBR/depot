//! A vehicle's envmapped face that the cab classification counts as cab (a face up under
//! the roof's top: the shoulder rounding into a bus's roof, the cap over its destination
//! display) reflects in the Enhanced picture as it does anywhere else, as the vanilla
//! picture lays its sphere map by the material alone. Its reflection was swapped for the
//! cab's grey light and its envmap photo switched off: it faded wherever it faced the sky.

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{
    AlphaMode, Camera, Lighting, MaterialExtra, RenderOptions, Renderer, ShadingPath,
};
use omsi_texture::Image;

const SIZE: u32 = 32;

/// An envmap photo laid out as OMSI's are (the street at the top, the sky at the bottom),
/// its sky half white or black.
fn photo(sky_white: bool) -> Image {
    let rgba = (0..64u32)
        .flat_map(|y| {
            let v = if (y >= 32) == sky_white { 255u8 } else { 0 };
            (0..64).flat_map(move |_| [v, v, v, 255])
        })
        .collect();
    Image {
        width: 64,
        height: 64,
        rgba,
        has_alpha: false,
    }
}

/// The picture of a black chrome patch of a vehicle, 0.5 m under its roof and facing up
/// (cab by the classification), mirroring the sky; mean green over its centre.
fn reflection(renderer: &mut Renderer, sky_white: bool) -> f32 {
    let mut scene = renderer.new_scene();
    let sky = renderer.add_texture(&mut scene, &Image::solid([90, 140, 220, 255]), false);
    renderer.set_sky_textures(&mut scene, [sky, sky, sky]);
    let black = renderer.add_texture(&mut scene, &Image::solid([0, 0, 0, 255]), false);
    let env = renderer.add_texture(&mut scene, &photo(sky_white), false);
    let mask = renderer.add_texture(&mut scene, &Image::solid([255, 255, 255, 255]), false);
    let chrome = renderer.add_material_extra(
        &mut scene,
        Some(black),
        AlphaMode::Opaque,
        [0.5, 0.5, 0.5, 1.0],
        false,
        None,
        None,
        None,
        Some((env, 1.0)),
        [0.0; 3],
        MaterialExtra {
            env_mask: Some(mask),
            ..Default::default()
        },
    );
    let n = Vec3::new(0.0, -0.5, 0.87).normalize();
    let t1 = Vec3::X;
    let t2 = n.cross(Vec3::X).normalize();
    let c = Vec3::new(0.0, 3.0, 2.9);
    let patch = MeshData {
        positions: vec![c - t1 - t2, c + t1 - t2, c + t1 + t2, c - t1 + t2],
        normals: vec![n; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
    };
    let mesh = renderer.add_mesh(&mut scene, &patch);
    let part = renderer.add_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![chrome]);
    renderer.set_roof(&mut scene, part, Some(3.4));
    let camera = Camera {
        position: DVec3::new(0.0, 0.0, 2.9),
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 20.0,
        near: 0.1,
        far: 100.0,
    };
    let lighting = Lighting {
        path: ShadingPath::Hdr,
        shadows: false,
        fog_density: 0.0,
        ..Default::default()
    };
    let img = renderer
        .render_to_image(&mut scene, SIZE, SIZE, &camera, &lighting)
        .unwrap();
    let c = SIZE / 2;
    let greens: Vec<f32> = (c - 4..c + 4)
        .flat_map(|y| (c - 4..c + 4).map(move |x| (x, y)))
        .map(|(x, y)| img[((y * SIZE + x) * 4 + 1) as usize] as f32)
        .collect();
    greens.iter().sum::<f32>() / greens.len() as f32
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_face_up_under_the_roof_mirrors_the_envmap() {
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
    let white = reflection(&mut renderer, true);
    let black = reflection(&mut renderer, false);
    eprintln!("reflection under the roof: {white} with a white sky in the photo, {black} with a black one");
    assert!(
        white > black * 1.3 + 5.0,
        "the envmap photo does not show under the roof: {white} white, {black} black"
    );
}

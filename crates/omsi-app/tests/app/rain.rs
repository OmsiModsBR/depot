/// The snow on the glass goes up with its mip levels, and it is white throughout (a
/// level made smaller stays white, only less opaque).
#[test]
fn snow_on_glass_has_mip_levels_and_stays_white() {
    let img = openomsi_game::testing::rain::snow_on_glass(std::path::Path::new("/nonexistent"));
    assert!(img.rgba.chunks_exact(4).all(|p| p[..3] == [255, 255, 255]));
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::NOOP;
    descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
    let instance = wgpu::Instance::new(descriptor);
    let renderer = pollster::block_on(omsi_render::Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        omsi_render::RenderOptions {
            msaa: 1,
            shadow_size: 1024,
            ..Default::default()
        },
    ))
    .expect("noop renderer");
    let mut scene = renderer.new_scene();
    let id = openomsi_game::testing::rain::add_snow_on_glass(
        &renderer,
        &mut scene,
        std::path::Path::new("/nonexistent"),
    );
    assert_eq!(renderer.texture_levels(&scene, id), Some((512, 512, 10)));
}

#[test]
fn snow_on_glass_is_mostly_clear_and_thickest_at_the_rim() {
    // no content root here: the procedural specks
    let img = openomsi_game::testing::rain::snow_on_glass(std::path::Path::new("/nonexistent"));
    assert_eq!((img.width, img.height), (512, 512));
    let alpha_of = |x: usize, y: usize| img.rgba[(y * 512 + x) * 4 + 3] as u32;
    let (mut middle, mut rim, mut n_middle, mut n_rim) = (0u32, 0u32, 0u32, 0u32);
    for y in 0..512 {
        for x in 0..512 {
            let d = ((x as f32 - 255.5).abs()).max((y as f32 - 255.5).abs()) / 255.5;
            if d < 0.35 {
                middle += alpha_of(x, y);
                n_middle += 1;
            } else if d > 0.8 {
                rim += alpha_of(x, y);
                n_rim += 1;
            }
        }
    }
    let mean = img.rgba.chunks_exact(4).map(|p| p[3] as u32).sum::<u32>() / (512 * 512);
    assert!(mean < 60, "the glass would be opaque (mean alpha {mean})");
    let (a, b) = (rim as f32 / n_rim as f32, middle as f32 / n_middle as f32);
    assert!(
        a > b * 1.3,
        "the rim should carry more snow than the middle ({a:.1} vs {b:.1})"
    );
}

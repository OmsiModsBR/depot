//! Tests moved out of `src/renderer/prepare.rs`.

use omsi_render::testing::internals::*;

fn noop_renderer() -> Renderer {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::NOOP;
    descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
    let instance = wgpu::Instance::new(descriptor);
    pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            shadow_size: 1024,
            ..Default::default()
        },
    ))
    .expect("noop renderer")
}

#[test]
fn a_fallback_keeps_what_the_game_uploaded() {
    let img = omsi_texture::Image {
        width: 2,
        height: 2,
        rgba: vec![255; 16],
        has_alpha: true,
    };
    let mut r = noop_renderer();
    let mut scene = r.new_scene();
    r.set_corona_texture(3, &img);
    r.set_light_map_tile((1, 2), Some(&img));
    r.set_light_map_place(100.0, 200.0, 300.0);
    r.set_dynamic_scale(0.7);
    r.set_env_heading(Some(12.0));
    r.shadow_blobs = !r.shadow_blobs;
    r.blend_by_origin = true;
    let blobs = r.shadow_blobs;
    r.fall_back_to_single_sample(&mut scene);
    assert!(r
        .sprites
        .corona_textures
        .get(3)
        .is_some_and(|t| t.is_some()));
    assert_eq!(r.light_map.lm_place.get(), (100.0, 200.0, 300.0));
    assert_eq!(r.dynamic_scale(), 0.7);
    assert_eq!(r.env_heading.get(), Some(12.0));
    assert_eq!(r.shadow_blobs, blobs);
    assert!(r.blend_by_origin);
    assert!(scene.dirty && scene.camera_bind_group.is_none());
}

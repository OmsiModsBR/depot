//! A renderer is built with the mode it was asked for and no other (FR-026): the Enhanced
//! pipelines exist only in an Enhanced renderer, Vanilla+'s reflection pass only in a Vanilla+ one.

use omsi_render::testing::{modes_built, ModesBuilt};
use omsi_render::{RenderOptions, Renderer, ShadingPath};

fn renderer(path: ShadingPath) -> Renderer {
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
            path,
            ..Default::default()
        },
    ))
    .expect("test renderer")
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_vanilla_plus_renderer_builds_no_enhanced_pipeline() {
    let r = renderer(ShadingPath::Linear);
    assert_eq!(
        modes_built(&r),
        ModesBuilt {
            enhanced: false,
            vanilla_plus_reflections: true
        }
    );
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn an_enhanced_renderer_builds_no_vanilla_plus_reflection_pass() {
    let r = renderer(ShadingPath::Hdr);
    assert_eq!(
        modes_built(&r),
        ModesBuilt {
            enhanced: true,
            vanilla_plus_reflections: false
        }
    );
}

//! Making everything the renderer draws with, once at start (`Renderer::build`): the
//! shaders and layouts, then each pass's pipelines through its own builder.

use super::options::RenderOptions;
use super::state::Renderer;
use crate::assets::{AssetStore, TexAddressing};
use crate::effects::puddles;
use crate::gpu::{
    basic_pipelines, fault_is, sixteen_texture_units, target_2d, FaultHook, GpuContext, Samplers,
    GL_BACKEND,
};
use crate::interface::{ModeResources, RenderMode};
use crate::lighting::{LightMapAtlas, ShadingPath};
use crate::modes::{
    enhanced::{Enhanced, EnhancedResources},
    vanilla_plus::VanillaPlus,
};
use crate::output::{Diagnostics, Presenter};
use crate::passes::{
    ao_pipelines, post_pipelines, prepass_pipelines, snow_layout, CoronaBuilder, Exposure,
    PathBuilders, SceneBuilder, ShadowResources, SkyBuilder, SnowBuilder, SpriteResources,
};
use crate::scene::{CameraUniform, CullHistory, Layouts};
use crate::shaders::scene_shader_source;
use crate::sky::{EnhancedUniform, SkyResources};
use crate::targets::TargetCache;

/// The render mode of a graphics path: the one function that names the modes. They keep no
/// state of their own yet, so they are shared, and the path may change from frame to frame.
pub(crate) fn mode_for(path: ShadingPath) -> &'static dyn RenderMode {
    match path {
        ShadingPath::Hdr => &Enhanced,
        ShadingPath::Linear => &VanillaPlus,
    }
}

impl Renderer {
    /// The pipelines, samplers and fixed textures of a renderer whose options are settled.
    pub(crate) fn build(
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter_name: String,
        format: wgpu::TextureFormat,
        options: RenderOptions,
    ) -> Renderer {
        let (msaa, shadow_size) = (options.msaa, options.shadow_size);
        let gpu = GpuContext::open(&device, format, msaa);
        if fault_is(FaultHook::Build) && msaa > 1 {
            // test hook for the fallback in `new_with`: a sample count no device takes
            let _ = target_2d(
                &device,
                "invalid",
                wgpu::Extent3d {
                    width: 4,
                    height: 4,
                    depth_or_array_layers: 1,
                },
                3,
                format,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
        }
        // One module for both paths: the enhanced fragment shader shares the vertex shader,
        // which the depth prepass relies on to the last bit (see `VsOut::clip`).
        log::info!("renderer: compiling the scene shaders");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("omsi"),
            source: wgpu::ShaderSource::Wgsl(
                scene_shader_source(GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed)).into(),
            ),
        });
        let layouts = Layouts::new(&device);
        let light_map = LightMapAtlas::new(&device);
        let snow_layout = snow_layout(&device);
        let sprites = SpriteResources::new(&device, &queue, &snow_layout);
        let scene_builder = SceneBuilder::new(&device, &shader, &layouts);
        let vertex_layout = scene_builder.vertex_layout().clone();
        let shadows = ShadowResources::new(
            &device,
            shadow_size,
            &shader,
            &layouts.material_layout,
            &vertex_layout,
        );
        let hdr_format = wgpu::TextureFormat::Rgba16Float;
        let samplers = Samplers::new(&device, options.anisotropy);
        let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let assets = AssetStore::new(&device, &queue);
        let builders = PathBuilders {
            scene: scene_builder,
            corona: CoronaBuilder::new(&device, &layouts, &sprites.corona_layout, msaa),
            snow: SnowBuilder::new(&device, &layouts, &snow_layout, msaa),
            sky: SkyBuilder::new(&device, &layouts, msaa),
            msaa,
            basic: basic_pipelines(),
        };
        let pass = builders.vanilla(format);
        // the enhanced path: its own lighting in all three
        let leave_out_enhanced = options.no_enhanced
            && (cfg!(target_os = "android")
                || adapter_name.to_ascii_lowercase().contains("opengl")
                || GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed));
        // (its textures do not fit OpenGL's units here, see `sixteen_texture_units`)
        if sixteen_texture_units() && !options.no_enhanced {
            log::warn!("renderer: the enhanced graphics take more textures than OpenGL has units for on {adapter_name}; drawing vanilla+");
        }
        let leave_out_enhanced = leave_out_enhanced || sixteen_texture_units();
        let gl = GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed);
        // what only the enhanced mode draws with is its own
        let enhanced: Option<Box<dyn ModeResources>> = (!leave_out_enhanced).then(|| {
            Box::new(EnhancedResources::build(
                &builders,
                &device,
                &layouts.camera_layout,
                format,
                hdr_format,
                gl,
            )) as Box<dyn ModeResources>
        });
        let reflection_pass = (!leave_out_enhanced
            && !GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed))
        .then(|| builders.reflections(hdr_format));
        let sky_shader = &builders.sky.module;
        let presenter = Presenter::new(&device, format, msaa);
        let gl = GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed);
        let ssao = ao_pipelines(&device, gl);
        let prepass = prepass_pipelines(
            &device,
            &layouts.camera_layout,
            &layouts.material_layout,
            msaa,
            &shader,
            &vertex_layout,
        );
        let post = post_pipelines(&device, format, hdr_format);
        let adaptation = Exposure::new(&device, &post, &assets.white_texture, hdr_format);
        // --- enhanced lighting: the uniform, the sky table, the reflection probe
        let enh_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("enhanced lighting"),
            size: std::mem::size_of::<EnhancedUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sky = SkyResources::new(
            &device,
            &queue,
            &layouts.sky_layout,
            sky_shader,
            &camera_buf,
            &enh_buf,
            &samplers.lin_sampler,
            hdr_format,
        );
        let diagnostics = Diagnostics::new(&device);
        // GLES cannot reliably read the depth buffer (the same limitation as SSAO).
        let puddles = (!gl && !leave_out_enhanced).then(|| {
            puddles::Pipelines::new(
                &device,
                &shader,
                &layouts.camera_layout,
                &layouts.material_layout,
            )
        });
        Renderer {
            gpu,
            presenter,
            culling: CullHistory::new(),
            targets: TargetCache::new(),
            dynamic_scale: std::cell::Cell::new(1.0),
            shadows,
            assets,
            samplers,
            address_next: std::cell::Cell::new(TexAddressing::Wrap),
            light_map_next: std::cell::Cell::new(false),
            light_map,
            puddles,
            post,
            adaptation,
            diagnostics,
            enh_buf,
            sky,
            texture_aspect: None,
            rain_reflection: None,
            last_frame: None,
            instant_exposure: false,
            // (OMSI_RENDER_CLOCK=<s>: the animations' clock starts that far on - offscreen
            // stills of moving things, the windy trees, at different moments)
            started: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs_f64(
                    crate::gpu::debug()
                        .render_clock
                        .as_deref()
                        .and_then(|v| v.trim().parse::<f64>().ok())
                        .filter(|s| s.is_finite() && *s >= 0.0)
                        .unwrap_or(0.0),
                ))
                .unwrap_or_else(std::time::Instant::now),
            tree_gust_drift: std::cell::Cell::new((glam::DVec2::ZERO, 0.0)),
            ssao,
            layouts,
            prepass,
            device,
            queue,
            adapter_name,
            pass,
            enhanced,
            reflection_pass,
            sprites,
            camera_buf,
            shadow_blobs: options.shadow_blobs,
            options,
            env_heading: Default::default(),
            blend_by_origin: false,
            stats: Default::default(),
            overlay_hook: None,
            asset_mirror: Default::default(),
            plan_sink: None,
            counts: Default::default(),
            scratch: Default::default(),
        }
    }
}

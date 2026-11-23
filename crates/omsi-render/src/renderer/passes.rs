//! The passes on the renderer: the main pass's pipelines for a picture, and the main,
//! glass and post passes given the parts they draw with.

use crate::assets::GpuTexture;
use crate::gpu::target_2d;
use crate::interface::PipeSet;
use crate::lighting::Lighting;
use crate::passes::{
    encode_enhanced_post, encode_glass, encode_main_pass, Batch, FrameFeatures, MainParts,
    PassPipelines, PostParts,
};
use crate::renderer::Renderer;
use crate::scene::{Scene, TextureId};

impl Renderer {
    pub(crate) fn main_pass(&self, set: PipeSet) -> &PassPipelines {
        match set {
            PipeSet::Own => self.own_pipelines(),
            PipeSet::Reflections => self.reflection_pass.as_ref().unwrap(),
            PipeSet::Plain => &self.pass,
        }
    }

    /// The pipelines in the enhanced mode's own shading.
    fn own_pipelines(&self) -> &PassPipelines {
        self.enhanced
            .as_ref()
            .and_then(|m| m.own_pipelines())
            .expect("the enhanced mode's pipelines")
    }

    /// The main pass: the scene over the sky, with its depth from the prepass where it can
    /// share it, split over several encoders when the bundles allow (the parts are returned).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_main_pass(
        &mut self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        lighting: &Lighting,
        scene_view: &wgpu::TextureView,
        main_batches: &[Batch],
        cab_batches: &[Batch],
        prepass_batches: &[Batch],
        main_bundles: &[wgpu::RenderBundle],
        overlays: &[(TextureId, [f32; 4])],
        width: u32,
        height: u32,
        features: &FrameFeatures,
        pipe_set: PipeSet,
        has_presurface: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) -> Vec<wgpu::CommandEncoder> {
        // (the fields by name: `self.targets` is borrowed mutably below)
        let pipes = match pipe_set {
            PipeSet::Own => self
                .enhanced
                .as_ref()
                .and_then(|m| m.own_pipelines())
                .expect("the enhanced mode's pipelines"),
            PipeSet::Reflections => self.reflection_pass.as_ref().unwrap(),
            PipeSet::Plain => &self.pass,
        };
        encode_main_pass(
            MainParts {
                device: &self.device,
                queue: &self.queue,
                format: self.gpu.format,
                msaa: self.options.msaa,
                targets: &mut self.targets,
                prepass: &self.prepass,
                pipes,
                sky_mirror_pipeline: self.enhanced.as_ref().and_then(|m| m.sky_mirror_pipeline()),
                sky: &self.sky,
                sprites: &self.sprites,
                overlay_pipeline: &self.presenter.overlay_pipeline,
                seconds: self.started.elapsed().as_secs_f32(),
                smoke: self.options.smoke,
                snowfall: self.options.snowfall,
                coronas: self.options.coronas,
            },
            scene,
            encoder,
            lighting,
            scene_view,
            main_batches,
            cab_batches,
            prepass_batches,
            main_bundles,
            overlays,
            width,
            height,
            features,
            has_presurface,
            tset,
            timed,
        )
    }

    /// The rain films on the glass: a copy of the clean picture behind them, then the panes'
    /// drops drawn over it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_glass(
        &mut self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        scene_view: &wgpu::TextureView,
        rain_batches: &[Batch],
        width: u32,
        height: u32,
        features: &FrameFeatures,
        pipe_set: PipeSet,
        puddles_on: bool,
    ) {
        encode_glass(
            &self.targets,
            &self.main_pass(pipe_set).rain_pipelines,
            scene,
            encoder,
            scene_view,
            rain_batches,
            width,
            height,
            features,
            puddles_on,
        );
    }

    /// The enhanced path's post passes: glow, metering, adaptation, tone curve and FXAA.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_enhanced_post(
        &mut self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        lighting: &Lighting,
        scene_view: &wgpu::TextureView,
        overlays: &[(TextureId, [f32; 4])],
        width: u32,
        height: u32,
        dt: f32,
        features: &FrameFeatures,
        puddles_on: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
        // (a device lost since this frame began took the meter's buffer as well)
        let lost = self.device_lost().is_some();
        encode_enhanced_post(
            PostParts {
                queue: &self.queue,
                post: &self.post,
                targets: &self.targets,
                adaptation: &mut self.adaptation,
                sky: &self.sky,
                msaa: self.options.msaa,
                fxaa: self.options.fxaa,
                instant_exposure: self.instant_exposure,
                exposure_log: self
                    .diagnostics
                    .exposure_log
                    .as_mut()
                    .filter(|_| features.with_overlays && !lost),
                overlay_pipeline_1x: &self.presenter.overlay_pipeline_1x,
            },
            scene,
            encoder,
            lighting,
            scene_view,
            overlays,
            width,
            height,
            dt,
            features,
            puddles_on,
            tset,
            timed,
        );
    }

    /// The scene's slot for the picture behind the glass (black until a frame is drawn).
    pub(crate) fn glass_slot(&self, scene: &mut Scene) -> TextureId {
        if let Some(id) = scene.glass_slot {
            return id;
        }
        scene.textures.push(GpuTexture::showing(
            self.assets.black_texture.texture.clone(),
            self.assets.black_texture.view.clone(),
            (1, 1),
        ));
        let id = scene.textures.len() - 1;
        scene.glass_slot = Some(id);
        id
    }

    /// Refraction reads a copy made before drawing films, avoiding rain/wiper feedback.
    pub(crate) fn prepare_glass_behind(
        &mut self,
        scene: &mut Scene,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) {
        let Some(id) = scene.glass_slot else { return };
        if self.targets.glass_picture.as_ref().is_none_or(|v| {
            v.texture().width() != width
                || v.texture().height() != height
                || v.texture().format() != format
        }) {
            let tex = target_2d(
                &self.device,
                "picture behind glass",
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                1,
                format,
                wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            self.targets.glass_picture = Some(tex.create_view(&Default::default()));
        }
        let view = self.targets.glass_picture.as_ref().unwrap();
        if scene.textures[id].view != *view {
            scene.textures[id] =
                GpuTexture::showing(view.texture().clone(), view.clone(), (width, height));
            self.rebind_textures(scene, &[id]);
        }
    }
}

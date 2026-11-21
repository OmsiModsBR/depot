//! The main pass's pipelines for each shading path (`PassPipelines`): vanilla+ in the swap
//! chain's format, the enhanced path and the vanilla reflections in high range. Each pass's
//! own pipelines are built in its module; here they are put together by path.

use super::corona::{additive, alpha_blend, screen, CoronaBuilder};
use super::main::PassPipelines;
use super::scene_pipelines::SceneBuilder;
use super::sky::SkyBuilder;
use super::snow::SnowBuilder;

/// The builders of the passes that draw in the main pass, and what decides which pipelines
/// are made.
pub(crate) struct PathBuilders<'a> {
    pub(crate) scene: SceneBuilder<'a>,
    pub(crate) corona: CoronaBuilder<'a>,
    pub(crate) snow: SnowBuilder<'a>,
    pub(crate) sky: SkyBuilder<'a>,
    /// The scene's sample count (the rain's films are single-sampled).
    pub(crate) msaa: u32,
    /// Leave the snowfall out (`basic_pipelines`).
    pub(crate) basic: bool,
}

impl PathBuilders<'_> {
    #[allow(clippy::too_many_arguments)]
    fn path(
        &self,
        format: wgpu::TextureFormat,
        scene_fs: &str,
        corona_fs: &str,
        corona_blend: wgpu::BlendState,
        smoke_fs: &str,
        snow_fs: &str,
        sky_fs: &str,
    ) -> PassPipelines {
        PassPipelines {
            pipelines: self.scene.pipelines(format, scene_fs, self.msaa),
            rain_pipelines: self.scene.pipelines(format, scene_fs, 1),
            corona_pipeline: self.corona.pipeline(format, corona_fs, corona_blend),
            smoke_pipeline: self.corona.pipeline(format, smoke_fs, alpha_blend()),
            snow_pipeline: (!self.basic).then(|| self.snow.pipeline(format, snow_fs)),
            sky_pipeline: self.sky.pipeline(format, sky_fs),
        }
    }

    /// Vanilla+, drawn in the swap chain's format.
    pub(crate) fn vanilla(&self, format: wgpu::TextureFormat) -> PassPipelines {
        self.path(
            format,
            "fs_main",
            "fs_main",
            screen(),
            "fs_smoke",
            "fs_snow",
            "fs_main",
        )
    }

    /// The enhanced path, drawn in high range.
    pub(crate) fn enhanced(&self, hdr: wgpu::TextureFormat) -> PassPipelines {
        self.path(
            hdr,
            "fs_enhanced",
            "fs_enhanced",
            additive(),
            "fs_smoke_enhanced",
            "fs_snow_enhanced",
            "fs_enhanced",
        )
    }

    /// The vanilla scene drawn in high range, for the reflections of the enhanced path.
    pub(crate) fn reflections(&self, hdr: wgpu::TextureFormat) -> PassPipelines {
        self.path(
            hdr,
            "fs_vanilla_reflections",
            "fs_main",
            screen(),
            "fs_smoke",
            "fs_snow",
            "fs_main",
        )
    }

    /// The sky seen in a mirror, by the enhanced path.
    pub(crate) fn sky_mirror(&self, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        self.sky.pipeline(format, "fs_enhanced_mirror")
    }
}

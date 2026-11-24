//! The main pass's pipelines of a shading path (`PassPipelines`), put together from each
//! pass's own builder; the fragment entry points a path takes are the path's to name (the
//! plain ones here, Enhanced's in its mode).

use super::corona::{alpha_blend, screen, CoronaBuilder};
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

/// The fragment entry points of one shading path, for each pass that draws in the main pass.
pub(crate) struct PathShaders<'a> {
    pub(crate) scene: &'a str,
    pub(crate) corona: &'a str,
    pub(crate) corona_blend: wgpu::BlendState,
    pub(crate) smoke: &'a str,
    pub(crate) snow: &'a str,
    pub(crate) sky: &'a str,
}

impl PathBuilders<'_> {
    /// The main pass's pipelines of a path, drawn in `format`.
    pub(crate) fn path(&self, format: wgpu::TextureFormat, fs: &PathShaders) -> PassPipelines {
        PassPipelines {
            pipelines: self.scene.pipelines(format, fs.scene, self.msaa),
            rain_pipelines: self.scene.pipelines(format, fs.scene, 1),
            corona_pipeline: self.corona.pipeline(format, fs.corona, fs.corona_blend),
            smoke_pipeline: self.corona.pipeline(format, fs.smoke, alpha_blend()),
            snow_pipeline: (!self.basic).then(|| self.snow.pipeline(format, fs.snow)),
            sky_pipeline: self.sky.pipeline(format, fs.sky),
        }
    }

    /// The pipelines of a path's sky, drawn alone (the mirrors' sky of the enhanced mode).
    pub(crate) fn sky_only(&self, format: wgpu::TextureFormat, fs: &str) -> wgpu::RenderPipeline {
        self.sky.pipeline(format, fs)
    }

    /// Vanilla+, drawn in the swap chain's format.
    pub(crate) fn vanilla(&self, format: wgpu::TextureFormat) -> PassPipelines {
        self.path(
            format,
            &PathShaders {
                scene: "fs_main",
                corona: "fs_main",
                corona_blend: screen(),
                smoke: "fs_smoke",
                snow: "fs_snow",
                sky: "fs_main",
            },
        )
    }

    /// The vanilla scene drawn in high range, for the puddles' reflections.
    pub(crate) fn reflections(&self, hdr: wgpu::TextureFormat) -> PassPipelines {
        self.path(
            hdr,
            &PathShaders {
                scene: "fs_vanilla_reflections",
                corona: "fs_main",
                corona_blend: screen(),
                smoke: "fs_smoke",
                snow: "fs_snow",
                sky: "fs_main",
            },
        )
    }
}

//! The main pass's pipelines of a shading path (`PassPipelines`), put together from each
//! pass's own builder; the fragment entry points a path takes are the path's to name (the
//! plain ones here, Enhanced's in its mode).

use super::corona::{alpha_blend, CoronaBuilder};
use super::main::PassPipelines;
use super::scene_pipelines::SceneBuilder;
use super::sky::SkyBuilder;
use super::snow::SnowBuilder;

/// The builders of the passes that draw in the main pass, and what decides which pipelines
/// are made.
pub struct PathBuilders<'a> {
    pub scene: SceneBuilder<'a>,
    pub corona: CoronaBuilder<'a>,
    pub snow: SnowBuilder<'a>,
    pub sky: SkyBuilder<'a>,
    /// The scene's sample count (the rain's films are single-sampled).
    pub msaa: u32,
    /// Leave the snowfall out (`basic_pipelines`).
    pub basic: bool,
}

/// The fragment entry points of one shading path, for each pass that draws in the main pass.
pub struct PathShaders<'a> {
    pub scene: &'a str,
    pub corona: &'a str,
    pub corona_blend: wgpu::BlendState,
    pub smoke: &'a str,
    pub snow: &'a str,
    pub sky: &'a str,
}

impl PathBuilders<'_> {
    /// The main pass's pipelines of a path, drawn in `format`.
    pub fn path(&self, format: wgpu::TextureFormat, fs: &PathShaders) -> PassPipelines {
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
    pub fn sky_only(&self, format: wgpu::TextureFormat, fs: &str) -> wgpu::RenderPipeline {
        self.sky.pipeline(format, fs)
    }
}

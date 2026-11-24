//! Enhanced's pipelines: which fragment entry points of the shared passes it draws with.

use crate::passes::{additive, PassPipelines, PathBuilders, PathShaders};

/// The main pass, drawn in high range with Enhanced's own shading.
pub(super) fn main_pass(builders: &PathBuilders<'_>, hdr: wgpu::TextureFormat) -> PassPipelines {
    builders.path(
        hdr,
        &PathShaders {
            scene: "fs_enhanced",
            corona: "fs_enhanced",
            corona_blend: additive(),
            smoke: "fs_smoke_enhanced",
            snow: "fs_snow_enhanced",
            sky: "fs_enhanced",
        },
    )
}

/// The sky seen in a mirror, tone-mapped into the plain pass's format.
pub(super) fn sky_mirror(
    builders: &PathBuilders<'_>,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    builders.sky_only(format, "fs_enhanced_mirror")
}

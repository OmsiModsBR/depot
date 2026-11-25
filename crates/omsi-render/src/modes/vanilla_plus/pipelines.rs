//! Vanilla+'s pipelines: which fragment entry points of the shared passes it draws with.

use crate::passes::{screen, PassPipelines, PathBuilders, PathShaders};

/// The plain shading, drawn in the swap chain's format.
pub(super) fn plain(builders: &PathBuilders<'_>, format: wgpu::TextureFormat) -> PassPipelines {
    builders.path(
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

/// The plain scene drawn in high range, for the puddles' reflections.
pub(super) fn reflections(builders: &PathBuilders<'_>, hdr: wgpu::TextureFormat) -> PassPipelines {
    builders.path(
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

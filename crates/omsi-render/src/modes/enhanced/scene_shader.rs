//! Enhanced's scene fragment shader, as the mode hands it to the scene module.

use crate::gpu::basic_pipelines;

/// The WGSL of the Enhanced shading (`fs_enhanced` and its helpers), for the scene module.
pub(crate) fn scene_text() -> String {
    let src = include_str!("shaders/enhanced.wgsl").to_string();
    if basic_pipelines() {
        lean_scene(src)
    } else {
        src
    }
}

/// The scene shader without the parts 0.2.0 added to every pixel's lighting that a phone's
/// shader compiler gives up on (`basic_pipelines`): the street lamps' shadow maps read in
/// the loop over the lamps, and the sun's soft shadow taken a second and a third time for
/// the moon and a debug view. Adreno 740/830 (Galaxy S23-S25) failed the scene pipelines
/// with these inlined into the enhanced fragment shader, and every frame stopped on an
/// invalid 'omsi' pipeline (#1633, #1663, #1708).
pub(crate) fn lean_scene(src: String) -> String {
    let out = src
        .replace("irr = irr * lamp_shadow_at(li, p, n, thin);", "")
        .replace("ms = sun_shadow_soft(in.world, n, thin);", "ms = 1.0;")
        .replace(
            "let sm = sun_shadow_soft(in.world, n, thin);",
            "let sm = 1.0;",
        );
    debug_assert_eq!(out.matches("sun_shadow_soft(in.world").count(), 1);
    debug_assert!(!out.contains("* lamp_shadow_at("));
    out
}

/// The WGSL of the Enhanced sky (`fs_enhanced`, `fs_enhanced_mirror` and the night sky), for the
/// sky module.
pub(crate) fn sky_text() -> &'static str {
    include_str!("shaders/sky.wgsl")
}

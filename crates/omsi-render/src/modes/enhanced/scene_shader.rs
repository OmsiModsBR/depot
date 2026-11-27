//! Enhanced's scene fragment shader, as the mode hands it to the scene module.

use crate::gpu::{basic_pipelines, cloud_shade_fits};

/// The WGSL of the Enhanced shading (`fs_enhanced` and its helpers), for the scene module.
pub fn scene_text() -> String {
    let src = include_str!("shaders/enhanced.wgsl").to_string();
    let src = if cloud_shade_fits() {
        src
    } else {
        without_cloud_shade(src)
    };
    if basic_pipelines() {
        lean_scene(src)
    } else {
        src
    }
}

/// The scene shader on a device with no texture slot left for the clouds' shade map: the sun
/// is dimmed by the clouds over the camera instead (the renderer puts that into `sun_open`),
/// and the map reads as all light.
pub fn without_cloud_shade(src: String) -> String {
    let out = src
        .replace(
            "@group(0) @binding(21) var t_cloud_shade: texture_2d<f32>;",
            "",
        )
        .replace(
            "return textureSampleLevel(t_cloud_shade, s_lin, uv, 0.0).r;",
            "return 1.0;",
        );
    debug_assert!(!out.contains("t_cloud_shade"));
    out
}

/// The scene shader without the parts 0.2.0 added to every pixel's lighting that a phone's
/// shader compiler gives up on (`basic_pipelines`): the street lamps' shadow maps read in
/// the loop over the lamps, and the sun's soft shadow taken a second and a third time for
/// the moon and a debug view. Adreno 740/830 (Galaxy S23-S25) failed the scene pipelines
/// with these inlined into the enhanced fragment shader, and every frame stopped on an
/// invalid 'omsi' pipeline (#1633, #1663, #1708).
pub fn lean_scene(src: String) -> String {
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
pub fn sky_text() -> &'static str {
    include_str!("shaders/sky.wgsl")
}

/// The WGSL of the Enhanced coronas and smoke (`fs_enhanced`, `fs_smoke_enhanced`), for the
/// corona module.
pub fn corona_text() -> String {
    let src = include_str!("shaders/corona.wgsl").to_string();
    // (the basic set lights rain by the sky alone: the loop over the lamps is one more
    // thing a phone's compiler can fail on, see `lean_scene`)
    if basic_pipelines() {
        src.replace(
            "let l = precip_light(in.wpos.xyz, to_eye, in.wpos.w > 1.5);",
            "let l = sh_irradiance(vec3<f32>(0.0, 0.0, 1.0)) / PI;",
        )
    } else {
        src
    }
}

/// The WGSL of Enhanced's snowfall (`fs_snow_enhanced`), for the snow module.
pub fn snow_text() -> &'static str {
    include_str!("shaders/snow.wgsl")
}

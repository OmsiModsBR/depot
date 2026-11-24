//! The shaders' sources as the device gets them: the scene shader with the per-draw
//! arrays as storage buffers or textures, and the sky, snow, fog lamps and corona shaders.
//! The WGSL files sit beside this module, one folder per domain that draws with them;
//! `common/` holds the pieces several shaders include.

use crate::gpu::{array_path, basic_pipelines, ArrayPath, ARRAY_TEX_WIDTH};

/// A full-screen pass's shader with the shared triangle (`common/fullscreen.wgsl`) ahead
/// of it.
pub(crate) fn with_fullscreen(src: &str) -> String {
    [include_str!("common/fullscreen.wgsl"), src].join("\n")
}

/// The post passes' shader (`post/post.wgsl`), with the sRGB curves of `common/colour.wgsl`.
pub(crate) fn post_shader_source() -> String {
    with_fullscreen(
        &[
            include_str!("common/colour.wgsl"),
            include_str!("post/post.wgsl"),
        ]
        .join("\n"),
    )
}

/// The scene shader: the core's vertex stage and base shading, and the mode's own fragment
/// shader, in one module (the depth prepass relies on one vertex shader for both).
///
/// On OpenGL a texture has one sampler (GLSL's combined sampler2D), so there the tile
/// masks are read through `s_diffuse` at a UV clamped half a texel inside the tile, which
/// is what `s_tile`'s clamp to edge gives; reading `t_trans`/`t_night` through both
/// samplers fails the whole module ("Conflicting samplers").
pub(crate) fn scene_shader_source(gl: bool, mode: &str) -> String {
    arrays_as_textures(&scene_shader_text(gl, mode), array_path())
}

/// The scene module with its arrays read as `path` has them (see `ArrayPath`): each
/// storage array the device cannot read becomes a texture and its `name[i]` a function
/// that loads texel `i`; without storage at all the point lights are none.
pub(crate) fn arrays_as_textures(src: &str, path: ArrayPath) -> String {
    if path == ArrayPath::Storage {
        return src.to_string();
    }
    let w = ARRAY_TEX_WIDTH;
    let mut out = src.to_string();
    let mut swap = |decl: &str, with: String, name: &str, call: &str| {
        assert!(out.contains(decl), "scene shader: {decl} not found");
        out = out.replace(decl, &with);
        out = indexing_as_calls(&out, name, call);
    };
    let load = |name: &str, ty: &str, pick: &str| {
        format!(
            "var {name}_tex: texture_2d<{ty}>;\nfn {name}_at(i: u32) -> {} {{ return textureLoad({name}_tex, vec2<u32>(i % {w}u, i / {w}u), 0){pick}; }}",
            if pick.is_empty() { format!("vec4<{ty}>") } else { ty.to_string() }
        )
    };
    swap(
        "var<storage, read> models: array<vec4<f32>>;",
        load("models", "f32", ""),
        "models",
        "models_at",
    );
    swap(
        "var<storage, read> inst_params: array<vec4<f32>>;",
        load("inst_params", "f32", ""),
        "inst_params",
        "inst_params_at",
    );
    swap(
        "var<storage, read> draw_list: array<u32>;",
        load("draw_list", "u32", ".x"),
        "draw_list",
        "draw_list_at",
    );
    if path == ArrayPath::NoStorage {
        swap(
            "@group(0) @binding(3) var<storage, read> lights: array<PointLight>;",
            "fn lights_at(i: u32) -> PointLight { return PointLight(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0)); }".to_string(),
            "lights",
            "lights_at",
        );
        swap(
            "@group(0) @binding(4) var<storage, read> grid: array<u32>;",
            "fn grid_at(i: u32) -> u32 { return 0xffffffffu; }".to_string(),
            "grid",
            "grid_at",
        );
    }
    out
}

/// `name[expr]` (the whole word `name`) turned into `call(expr)`.
pub(crate) fn indexing_as_calls(src: &str, name: &str, call: &str) -> String {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let pat = format!("{name}[");
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(at) = rest.find(&pat) {
        let before = rest[..at].chars().next_back();
        out.push_str(&rest[..at]);
        if before.is_some_and(word) {
            out.push_str(&pat);
            rest = &rest[at + pat.len()..];
            continue;
        }
        // the matching bracket
        let inner = &rest[at + pat.len()..];
        let mut depth = 1;
        let end = inner
            .char_indices()
            .find(|&(_, c)| {
                depth += match c {
                    '[' => 1,
                    ']' => -1,
                    _ => 0,
                };
                depth == 0
            })
            .map(|(i, _)| i)
            .expect("unbalanced brackets in the scene shader");
        out.push_str(call);
        out.push('(');
        out.push_str(&indexing_as_calls(&inner[..end], name, call));
        out.push(')');
        rest = &inner[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The scene module's text: the core's sources, then `mode`, the fragment shader the mode
/// draws the scene with (empty for a mode with none of its own).
pub(crate) fn scene_shader_text(gl: bool, mode: &str) -> String {
    let src = [
        include_str!("common/camera.wgsl"),
        include_str!("common/math.wgsl"),
        include_str!("common/colour.wgsl"),
        include_str!("scene/shader.wgsl"),
        include_str!("common/enhanced_common.wgsl"),
        include_str!("common/puddle_common.wgsl"),
        include_str!("common/lamp_air.wgsl"),
        mode,
    ]
    .join("\n");
    if !gl {
        return src;
    }
    let clamped = |t: &str| {
        format!(
            "textureSample({t}, s_diffuse, clamp(uv, 0.5 / vec2<f32>(textureDimensions({t})), \
             vec2<f32>(1.0) - 0.5 / vec2<f32>(textureDimensions({t}))))"
        )
    };
    let out = src
        .replace("textureSample(t_trans, s_tile, uv)", &clamped("t_trans"))
        .replace("textureSample(t_night, s_tile, uv)", &clamped("t_night"));
    debug_assert!(!out.contains("s_tile, uv)"));
    out
}

/// How far the cloud field reaches before it repeats (m): a cumulus is then half a kilometre
/// to two across. Written into the sky shaders by `sky_shader_source`.
pub(crate) const CLOUD_FIELD_TILE: f32 = 14000.0;

/// The sky module: the dome of the plain path, the clouds' and the stars' pieces and the
/// reflection probe every mode has, then `mode`, the mode's own sky fragment shaders (empty for
/// a mode with none).
pub(crate) fn sky_shader_source(mode: &str) -> String {
    let cloud_tile = format!("const CLOUD_FIELD_TILE: f32 = {CLOUD_FIELD_TILE:?};");
    [
        cloud_tile.as_str(),
        include_str!("common/camera.wgsl"),
        include_str!("common/math.wgsl"),
        include_str!("common/fullscreen.wgsl"),
        include_str!("common/colour.wgsl"),
        include_str!("sky/sky.wgsl"),
        include_str!("sky/stars.wgsl"),
        include_str!("common/enhanced_common.wgsl"),
        include_str!("sky/sky_light.wgsl"),
        mode,
    ]
    .join("\n")
}

/// The snowfall (snow.wgsl), with the lamps the enhanced picture lights it by (none
/// without storage buffers).
pub(crate) fn snow_shader_source() -> String {
    let src = [
        include_str!("common/camera.wgsl"),
        include_str!("common/math.wgsl"),
        include_str!("sky/snow.wgsl"),
        include_str!("common/enhanced_common.wgsl"),
        FOG_LAMP_LIGHTS,
        include_str!("common/lamp_air.wgsl"),
    ]
    .join("\n");
    if array_path() == ArrayPath::NoStorage {
        let src = src
            .replace("@group(0) @binding(3) var<storage, read> lights: array<PointLight>;", "fn lights_at(i: u32) -> PointLight { return PointLight(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0)); }")
            .replace("@group(0) @binding(4) var<storage, read> grid: array<u32>;", "fn grid_at(i: u32) -> u32 { return 0xffffffffu; }");
        return indexing_as_calls(
            &indexing_as_calls(&src, "lights", "lights_at"),
            "grid",
            "grid_at",
        );
    }
    src
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SnowUniform {
    pub(crate) wind: [f32; 4],
    pub(crate) fall: [f32; 4],
}

/// The snowfall's flakes for a snowfall of strength `s` (0..1): how many each layer of
/// snow.wgsl holds (the near one at the density of a heavy fall, some 190 a cubic metre,
/// scaled down for a lighter one) and their mean diameter (m) - a light fall single
/// crystals of a millimetre, a heavy one aggregates of several.
pub(crate) fn snowfall_flakes(s: f32) -> ([u32; 3], f32) {
    let k = 0.08 + 0.92 * s.clamp(0.0, 1.0);
    (
        [
            (70000.0 * k) as u32,
            (55000.0 * k) as u32,
            (25000.0 * k) as u32,
        ],
        0.0006 + 0.0034 * s * s,
    )
}

/// A full-screen pass that reads the scene's lamps and the sky's light (`own` is its
/// fragment shader): the camera, math and full-screen pieces ahead of it, the sky's uniform,
/// the lamps' bindings and the air's light after it. The lamps are none without storage
/// buffers, so the pass is not built there.
pub(crate) fn lamp_pass_source(own: &str) -> String {
    [
        include_str!("common/camera.wgsl"),
        include_str!("common/math.wgsl"),
        include_str!("common/fullscreen.wgsl"),
        own,
        include_str!("common/enhanced_common.wgsl"),
        FOG_LAMP_LIGHTS,
        include_str!("common/lamp_air.wgsl"),
    ]
    .join("\n")
}

/// The scene's point lights and their grid as the fog pass reads them (`lamp_air.wgsl`):
/// the scene shader's bindings (the types are in `common/camera.wgsl`).
pub(crate) const FOG_LAMP_LIGHTS: &str =
    "@group(0) @binding(3) var<storage, read> lights: array<PointLight>;
@group(0) @binding(4) var<storage, read> grid: array<u32>;
";

/// The light coronas (both paths).
pub(crate) fn corona_shader_source() -> String {
    let src = [
        include_str!("common/camera.wgsl"),
        include_str!("common/math.wgsl"),
        include_str!("common/colour.wgsl"),
        include_str!("lights/corona.wgsl"),
        include_str!("common/enhanced_common.wgsl"),
        FOG_LAMP_LIGHTS,
        include_str!("common/lamp_air.wgsl"),
    ]
    .join("\n");
    // (the basic set lights rain by the sky alone: the loop over the lamps is one more
    // thing a phone's compiler can fail on, see `lean_scene`)
    let src = if basic_pipelines() {
        src.replace(
            "let l = precip_light(in.wpos.xyz, to_eye, in.wpos.w > 1.5);",
            "let l = sh_irradiance(vec3<f32>(0.0, 0.0, 1.0)) / PI;",
        )
    } else {
        src
    };
    // (without storage buffers the precipitation has no lamps to be lit by)
    if array_path() == ArrayPath::NoStorage {
        let src = src
            .replace("@group(0) @binding(3) var<storage, read> lights: array<PointLight>;", "fn lights_at(i: u32) -> PointLight { return PointLight(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0)); }")
            .replace("@group(0) @binding(4) var<storage, read> grid: array<u32>;", "fn grid_at(i: u32) -> u32 { return 0xffffffffu; }");
        return indexing_as_calls(
            &indexing_as_calls(&src, "lights", "lights_at"),
            "grid",
            "grid_at",
        );
    }
    src
}

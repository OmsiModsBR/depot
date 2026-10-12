//! The shaders' sources as the device gets them: the scene shader with the per-draw
//! arrays as storage buffers or textures, and the sky, snow, fog lamps and corona shaders.

use super::*;

/// The scene shader: the vanilla path and the enhanced fragment shader in one module.
///
/// On OpenGL a texture has one sampler (GLSL's combined sampler2D), so there the tile
/// masks are read through `s_diffuse` at a UV clamped half a texel inside the tile, which
/// is what `s_tile`'s clamp to edge gives; reading `t_trans`/`t_night` through both
/// samplers fails the whole module ("Conflicting samplers").
pub(crate) fn scene_shader_source(gl: bool) -> String {
    arrays_as_textures(&scene_shader_text(gl), array_path())
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

pub(crate) fn scene_shader_text(gl: bool) -> String {
    let src = [
        include_str!("colour.wgsl"),
        include_str!("shader.wgsl"),
        include_str!("enhanced_common.wgsl"),
        include_str!("puddle_common.wgsl"),
        include_str!("lamp_air.wgsl"),
        include_str!("enhanced.wgsl"),
    ]
    .join("\n");
    // Enhanced+: the enhanced pass writes the reflections' surfaces as well (`GBUF_FORMAT`)
    let src = if rt_gbuf() {
        src.replace("//RT ", "")
    } else {
        src
    };
    let src = if basic_pipelines() {
        lean_scene(src)
    } else {
        src
    };
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

/// The enhanced clouds' noise textures (clouds.rs), made once: the shape map (2-D RGBA8)
/// and the detail volume (3-D R8), both with their mip chains, and a repeating sampler.
pub(crate) fn cloud_noise_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::TextureView, wgpu::Sampler, Vec<u8>) {
    let t0 = std::time::Instant::now();
    let (shape, detail) = std::thread::scope(|s| {
        let a = s.spawn(clouds::shape_map);
        let b = s.spawn(clouds::detail_volume);
        (
            a.join().expect("cloud shape"),
            b.join().expect("cloud detail"),
        )
    });
    let make = |label: &str,
                size: u32,
                dim: wgpu::TextureDimension,
                format: wgpu::TextureFormat,
                bpp: u32,
                levels: &[Vec<u8>]| {
        let depth = if dim == wgpu::TextureDimension::D3 {
            size
        } else {
            1
        };
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: depth,
            },
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: dim,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (m, data) in levels.iter().enumerate() {
            let e = (size >> m).max(1);
            let d = if dim == wgpu::TextureDimension::D3 {
                e
            } else {
                1
            };
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: m as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(e * bpp),
                    rows_per_image: Some(e),
                },
                wgpu::Extent3d {
                    width: e,
                    height: e,
                    depth_or_array_layers: d,
                },
            );
        }
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    };
    let shape_view = make(
        "cloud shape",
        clouds::SHAPE_SIZE,
        wgpu::TextureDimension::D2,
        wgpu::TextureFormat::Rgba8Unorm,
        4,
        &shape,
    );
    let detail_view = make(
        "cloud detail",
        clouds::DETAIL_SIZE,
        wgpu::TextureDimension::D3,
        wgpu::TextureFormat::R8Unorm,
        1,
        &detail,
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("cloud noise"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });
    log::info!("cloud noise made in {:.2} s", t0.elapsed().as_secs_f32());
    // (the shape map's second level kept for the clouds' shadow, `cloud_sun_transmittance`)
    let cpu = shape.get(1).cloned().unwrap_or_default();
    (shape_view, detail_view, sampler, cpu)
}

/// The sky dome (both paths) and the enhanced reflection probe.
pub(crate) fn sky_shader_source() -> String {
    [
        include_str!("colour.wgsl"),
        include_str!("sky.wgsl"),
        include_str!("enhanced_common.wgsl"),
        include_str!("sky_enhanced.wgsl"),
    ]
    .join("\n")
}

/// The snowfall (snow.wgsl), with the lamps the enhanced picture lights it by (none
/// without storage buffers).
pub(crate) fn snow_shader_source() -> String {
    let src = [
        include_str!("snow.wgsl"),
        include_str!("enhanced_common.wgsl"),
        FOG_LAMP_LIGHTS,
        include_str!("lamp_air.wgsl"),
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

/// Enhanced: the lamps' light in the fog over the drawn picture (`fog_lamps.wgsl`; not
/// without storage buffers, where there are no lamps).
pub(crate) fn fog_lamps_shader_source() -> String {
    [
        include_str!("fog_lamps.wgsl"),
        include_str!("enhanced_common.wgsl"),
        FOG_LAMP_LIGHTS,
        include_str!("lamp_air.wgsl"),
    ]
    .join("\n")
}

/// The scene's point lights and their grid as the fog pass reads them (`lamp_air.wgsl`):
/// the scene shader's declarations.
pub(crate) const FOG_LAMP_LIGHTS: &str = "struct PointLight {
    pos: vec4<f32>,
    color: vec4<f32>,
    dir: vec4<f32>,
    extra: vec4<f32>,
};
@group(0) @binding(3) var<storage, read> lights: array<PointLight>;
@group(0) @binding(4) var<storage, read> grid: array<u32>;
const CELL_CAP: u32 = 32u;
";

/// The light coronas (both paths).
pub(crate) fn corona_shader_source() -> String {
    let src = [
        include_str!("corona.wgsl"),
        include_str!("enhanced_common.wgsl"),
        FOG_LAMP_LIGHTS,
        include_str!("lamp_air.wgsl"),
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

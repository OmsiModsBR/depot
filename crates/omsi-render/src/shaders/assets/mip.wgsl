// Mipmap generation: alpha-weight RGB so bright RGB stored in transparent texels does not
// bleed into tree/fence silhouettes when the texture switches to a smaller mip. Where all
// four texels have alpha 0 the plain average is kept: on opaque materials the alpha channel
// is often only the envmap mask, and 0 there (lamps, wheels) must not turn the colour black.
@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;
struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    var out: VsOut;
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dim = textureDimensions(t_src);
    let centre = in.uv * vec2<f32>(dim);
    let base = vec2<i32>(floor(centre - vec2<f32>(1.0)));
    var rgb = vec3<f32>(0.0);
    var plain = vec3<f32>(0.0);
    var alpha = 0.0;
    for (var y = 0; y < 2; y = y + 1) {
        for (var x = 0; x < 2; x = x + 1) {
            let p = clamp(base + vec2<i32>(x, y), vec2<i32>(0), vec2<i32>(dim) - vec2<i32>(1));
            let q = textureLoad(t_src, p, 0);
            rgb = rgb + q.rgb * q.a;
            plain = plain + q.rgb;
            alpha = alpha + q.a;
        }
    }
    return vec4<f32>(select(plain * 0.25, rgb / alpha, alpha > 1e-5), alpha * 0.25);
}

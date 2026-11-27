// The clouds' shade map, second step (sky/cloud_shade.rs): a Gaussian blur along one axis of
// the transmittance the depth pass drew, so that a cloud's shadow has the soft edge its
// height gives it (the sun's disc seen from the ground, and the light the cloud scatters
// beside its own shadow). The pass runs twice, along x and then along y.

struct Blur {
    // xy the uv step of one texel along the axis, z the Gaussian's sigma (texels), w the
    // taps on each side of the centre
    v: vec4<f32>,
};
@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;
@group(0) @binding(2) var<uniform> blur: Blur;

@vertex
fn vs_blur(@builtin(vertex_index) i: u32) -> FullscreenVertex {
    return fullscreen_vertex(i);
}

@fragment
fn fs_blur(in: FullscreenVertex) -> @location(0) vec4<f32> {
    let taps = i32(blur.v.w);
    let k = 1.0 / (2.0 * blur.v.z * blur.v.z);
    var sum = 0.0;
    var total = 0.0;
    for (var i = -taps; i <= taps; i = i + 1) {
        let w = exp(-f32(i * i) * k);
        sum = sum + w * textureSampleLevel(t_src, s_src, in.uv + blur.v.xy * f32(i), 0.0).r;
        total = total + w;
    }
    return vec4<f32>(sum / total, 0.0, 0.0, 1.0);
}

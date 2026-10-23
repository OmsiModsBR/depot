// One triangle over the whole screen, from the vertex index alone (drawn as 0..3 with no
// vertex buffer): its corners in clip space, and the uv at each (0,0 at the top left).
struct FullscreenVertex {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn fullscreen_vertex(i: u32) -> FullscreenVertex {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    return FullscreenVertex(vec4<f32>(x, y, 0.0, 1.0), vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5));
}

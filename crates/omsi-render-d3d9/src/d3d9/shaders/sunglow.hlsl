// The sun's glow (Shader Model 3.0): a bright glow added around the sun's place in the sky,
// as a camera-facing square far away on the sun's side, drawn after the scene with the
// sky's depth (the far plane), so whatever stands in front of the sun (buildings, hills,
// the vehicle the camera is in) hides it where it covers it and glass lets it through.
//
// Only the glow's size and fall-off are decided here; how much of it shows (the sun's
// strength, the weather, the fog, the horizon) is worked out per frame by the backend and
// arrives as the colour.

// ---- vertex stage ----

float4x4 view_proj : register(c0); // column-major, one column per register
float4 cam_pos : register(c4);     // xyz the camera, relative to the render origin
// as the scene's: Direct3D 9's pixel centres are half a pixel off the default renderer's
float4 pixel_offset : register(c5);
float4 cam_right : register(c6);
float4 cam_up : register(c7);
// xyz the direction towards the sun, w the distance the square is put at
float4 sun : register(c8);
// x the square's half side in metres at that distance
float4 size : register(c9);

struct VsOut {
    float4 clip : POSITION;
    float2 uv : TEXCOORD0;
};

VsOut vs_main(float2 corner : POSITION) {
    VsOut o;
    float3 world = cam_pos.xyz + sun.xyz * sun.w
        + (cam_right.xyz * corner.x + cam_up.xyz * corner.y) * size.x;
    o.clip = mul(view_proj, float4(world, 1.0));
    // pushed to the far plane (0 with reversed depth), where the sky is
    o.clip.z = o.clip.w * 0.000001;
    o.clip.xy += pixel_offset.xy * o.clip.w;
    o.uv = corner;
    return o;
}

// ---- pixel stage ----

// rgb the glow's colour at full strength, already worked out for the frame
float4 glow : register(c200);

float4 ps_main(float2 uv : TEXCOORD0) : COLOR0 {
    float r = length(uv);
    // a hot core and a wide soft halo, nothing at the square's edge
    float k = 0.8 * exp(-r * r * 14.0) + 0.35 * exp(-r * 3.5);
    k *= saturate(1.0 - r);
    return float4(glow.rgb * k, 1.0);
}

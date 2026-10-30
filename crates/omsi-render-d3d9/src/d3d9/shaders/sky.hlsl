// The sky dome (Shader Model 3.0): the picture behind the scene, as the default renderer's
// vanilla sky draws it, without the clouds. The dome rides with the camera; u is the azimuth
// relative to the sun and v the elevation (0 at the zenith, 1 at the horizon) into the three
// pictures of envir.cfg's [sky_textures] (day, twilight, night), blended by the weights the
// time of day gives. The fog covers the horizon and, in a thick fog, the whole sky, so the
// far geometry fades into it without a seam.
//
// The pictures are blended in linear light and the fog is mixed in the stored (gamma-encoded)
// space, as the default renderer does; the result is written as it is.

// ---- vertex stage ----

float4x4 view_proj : register(c0); // column-major, one column per register
float4 cam_pos : register(c4);     // xyz the camera, relative to the render origin
// as the scene's: Direct3D 9's pixel centres are half a pixel off the default renderer's
float4 pixel_offset : register(c5);

struct VsOut {
    float4 clip : POSITION;
    float3 dir : TEXCOORD0;
};

VsOut vs_main(float3 pos : POSITION) {
    VsOut o;
    // the dome rides with the camera; pushed to the far plane (0 with reversed depth)
    float3 world = cam_pos.xyz + pos * 4000.0;
    o.clip = mul(view_proj, float4(world, 1.0));
    o.clip.z = o.clip.w * 0.000001;
    o.clip.xy += pixel_offset.xy * o.clip.w;
    o.dir = pos;
    return o;
}

// ---- pixel stage ----

sampler2D t_day : register(s0);
sampler2D t_twilight : register(s1);
sampler2D t_night : register(s2);

// x the sun's azimuth (radians, clockwise from north), yzw the weights of the day, twilight
// and night pictures
float4 sky : register(c200);
// rgb the fog's colour, w its density
float4 fog : register(c201);

float3 srgb_encode(float3 c) {
    c = saturate(c);
    float3 hi = 1.055 * exp2(log2(c) * (1.0 / 2.4)) - 0.055;
    return c <= 0.0031308 ? c * 12.92 : hi;
}

float3 srgb_decode(float3 c) {
    c = saturate(c);
    float3 hi = exp2(log2((c + 0.055) / 1.055) * 2.4);
    return c <= 0.04045 ? c / 12.92 : hi;
}

float4 ps_main(float3 dir : TEXCOORD0) : COLOR0 {
    float3 d = normalize(dir);
    float az = atan2(d.x, d.y);
    float u = frac((az - sky.x) / 6.2831853 + 0.5);
    float elev = asin(clamp(d.z, -1.0, 1.0));
    // the horizon row at the bottom; below the horizon the horizon's colour stays
    float v = clamp(1.0 - elev / 1.5707963, 0.0, 0.995);
    float4 uv = float4(u, v, 0.0, 0.0);
    float3 c = srgb_decode(tex2Dlod(t_day, uv).rgb) * sky.y
        + srgb_decode(tex2Dlod(t_twilight, uv).rgb) * sky.z * 0.8
        + srgb_decode(tex2Dlod(t_night, uv).rgb) * sky.w * 0.6;
    // fog swallows the horizon, and a thick fog (a few hundred metres of sight) the whole sky
    float horizon = saturate(1.0 - elev / 0.12) * saturate(fog.w * 1500.0);
    float whole = saturate(fog.w * 150.0 - 0.15) * clamp(1.0 - elev / 1.2, 0.35, 1.0);
    float f = max(horizon, whole);
    return float4(lerp(srgb_encode(c), fog.rgb, f), 1.0);
}

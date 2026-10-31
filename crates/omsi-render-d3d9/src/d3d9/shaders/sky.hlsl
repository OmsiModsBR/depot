// The sky dome (Shader Model 3.0): the picture behind the scene, as the default renderer's
// vanilla sky draws it. The dome rides with the camera; u is the azimuth relative to the sun
// and v the elevation (0 at the zenith, 1 at the horizon) into the three pictures of
// envir.cfg's [sky_textures] (day, twilight, night), blended by the weights the time of day
// gives. Over them come the stars (night only, a few points that stay where they are in the
// world) and the weather's one layer of clouds (the cloud type's own picture, laid flat at
// the weather's base height and moved by the wind). The fog covers the horizon and, in a
// thick fog, the whole sky, so the far geometry fades into it without a seam.
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
sampler2D t_clouds : register(s3); // white, the clouds in its alpha

// x the sun's azimuth (radians, clockwise from north), yzw the weights of the day, twilight
// and night pictures
float4 sky : register(c200);
// rgb the fog's colour, w its density
float4 fog : register(c201);
// the light the clouds are lit by (the scene's): xyz towards the sun, then the sun's, the
// ambient and the sky's colour
float4 sun_dir : register(c202);
float4 sun_color : register(c203);
float4 ambient : register(c204);
float4 sky_color : register(c205);
// x the cloud cover (0: no clouds), yz the drift with the wind in tiles, w the layer's base
// height in metres
float4 clouds : register(c206);

// One tile of the cloud picture, in metres: the wind's drift (`clouds.yz`) is counted in them.
static const float CLOUD_TILE = 2500.0;
// Cells across a face of the cube the stars are laid out on (about 0.3 degrees each), and the
// share of cells that hold one.
static const float STAR_CELLS = 190.0;
static const float STAR_SHARE = 0.012;

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

float hash(float3 p) {
    p = frac(p * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return frac((p.x + p.y) * p.z);
}

// The stars' light towards d (0 .. 1): the sphere is cut into the six faces of a cube, each
// into cells; a share of the cells holds a point of its own brightness, drawn about a pixel
// wide. They stay where they are in the world (no turning with the hours).
float stars(float3 d, float pixel) {
    float3 a = abs(d);
    float m = max(a.x, max(a.y, a.z));
    float face = (a.x == m) ? ((d.x > 0.0) ? 0.0 : 1.0) : ((a.y == m) ? ((d.y > 0.0) ? 2.0 : 3.0) : ((d.z > 0.0) ? 4.0 : 5.0));
    float2 uv = (a.x == m) ? d.yz : ((a.y == m) ? d.xz : d.xy);
    uv = uv / m * (STAR_CELLS * 0.5);
    float2 cell = floor(uv);
    float2 at = uv - cell;
    float3 id = float3(cell, face);
    float2 spot = float2(hash(id + 17.0), hash(id + 41.0)) * 0.6 + 0.2;
    // a pixel in cell units: the angle it spans, scaled by the face's stretch towards its edges
    float2 face_uv = uv / (STAR_CELLS * 0.5);
    float size = max(pixel * 0.5 * STAR_CELLS * (1.0 + dot(face_uv, face_uv)), 0.05);
    float brightness = hash(id + 73.0);
    float present = (hash(id + 5.0) < STAR_SHARE) ? 1.0 : 0.0;
    float k = saturate(1.0 - length(at - spot) / size);
    return present * k * k * (0.35 + 0.65 * brightness * brightness);
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
    // the sky's own colour, which the clouds are lit from (the stars are behind them)
    float3 sky_c = c;
    // the stars: only at night (fading in with the dusk), above the horizon
    float pixel = length(float2(length(ddx(d)), length(ddy(d))));
    float night = smoothstep(0.35, 1.0, sky.w);
    c += float3(0.85, 0.9, 1.0) * stars(d, pixel) * night * saturate(elev * 8.0);
    // the cloud layer, flat at its base height: the ground point under the view ray, in
    // tiles, the wind's drift added; none below the horizon, fading into it above
    float t = clouds.w / max(d.z, 0.02);
    float2 ground = d.xy * (t / CLOUD_TILE) + clouds.yz;
    float shape = tex2D(t_clouds, ground).a;
    float closed = smoothstep(0.8, 1.0, clouds.x);
    float cover = shape * saturate((d.z - 0.01) * 7.0) * ((clouds.x > 0.001) ? 1.0 : 0.0);
    // lit as the scene is: white by day, warm at sunrise, at night hardly lighter than the
    // night sky; the closed deck greys over
    float sun_up = saturate(sun_dir.z * 4.0 + 0.3);
    float3 lit = sun_color.rgb * 1.1 * sun_up + ambient.rgb * lerp(0.5, 1.5, sun_up) + sky_color.rgb * 0.3;
    float3 cloud_col = max(min(lit, 0.97), sky_c * 1.12) * (1.0 - 0.3 * closed);
    c = lerp(c, cloud_col, cover);
    // fog swallows the horizon, and a thick fog (a few hundred metres of sight) the whole sky
    float horizon = saturate(1.0 - elev / 0.12) * saturate(fog.w * 1500.0);
    float whole = saturate(fog.w * 150.0 - 0.15) * clamp(1.0 - elev / 1.2, 0.35, 1.0);
    float f = max(horizon, whole);
    return float4(lerp(srgb_encode(c), fog.rgb, f), 1.0);
}

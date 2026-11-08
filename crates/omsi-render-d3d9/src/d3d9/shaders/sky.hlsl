// The sky dome (Shader Model 3.0): the picture behind the scene, as the default renderer's
// vanilla sky draws it. The dome rides with the camera; u is the azimuth relative to the sun
// and v the elevation (0 at the zenith, 1 at the horizon) into the three pictures of
// envir.cfg's [sky_textures] (day, twilight, night), blended by the weights the time of day
// gives. Over them come the stars (helper\stars.dat, turning with the hours, as bright as the
// night lets them) and the weather's one layer of clouds (the cloud type's own picture, laid flat at
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
sampler2D t_sun : register(s4);    // Texture\sonne.bmp
sampler2D t_moon : register(s5);   // Texture\mond.bmp
sampler2D t_stars : register(s6);  // the star map (omsi_content::StarMap), read texel by texel

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
// (x not used) y 1 draws the sun's picture, z the moon's
float4 options : register(c207);
// xyz towards the moon
float4 moon_dir : register(c208);
// the rows of the rotation from a world direction into the stars' frame; w of the first how
// bright the stars are (0 none), w of the second the star map's cells across a face (0: no
// stars: no map, or the Stars option off)
float4 stars_x : register(c209);
float4 stars_y : register(c210);
float4 stars_z : register(c211);

// The sun's picture is a square whose half side is this tangent of the angle from its middle,
// and the moon a ball of this tangent of its angular radius: the sizes of the stock
// helper\sonne.x square and helper\mond.x ball, set 3 units from the eye.
static const float SUN_HALF = 0.02677;
static const float MOON_RADIUS = 0.01563;

// One tile of the cloud picture, in metres: the wind's drift (`clouds.yz`) is counted in them.
static const float CLOUD_TILE = 2500.0;
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

// The stars: the star map holds the sphere of the stars' own frame cut into the six faces of
// a cube, each face into cells of up to two stars (two layers of cells, one under the other),
// a texel a star: its place in the cell (rg), its size (b: 0 none, else 1 + 254 x size / 8;
// the size is 4 x the square root of its brightness) and its class (a). The same layout and
// reading as the default renderer's sky (stars.wgsl).

// The colour of a class number (omsi_content::StarClass::rgb, by its code): the first class's
// colour plus each step up to the code (a few vector operations rather than a chain of tests:
// the pixel shader's instruction slots are few).
float3 star_class_rgb(float code) {
    float4 lo = step(float4(1.5, 2.5, 3.5, 4.5), code);
    float3 hi = step(float3(5.5, 6.5, 7.5), code);
    float3 rgb = float3(209.0, 198.0, 255.0)
        + mul(lo, float4x3(6.0, 7.0, 0.0, 9.0, 12.0, 0.0, 16.0, 27.0, 0.0, 14.0, 11.0, -30.0))
        + mul(hi, float3x3(1.0, -3.0, -40.0, 0.0, -24.0, -38.0, 0.0, -30.0, 0.0));
    return rgb / 255.0;
}

float3 star_frame(float3 d) {
    return float3(dot(stars_x.xyz, d), dot(stars_y.xyz, d), dot(stars_z.xyz, d));
}

// The face of the cube e falls on (0..5, as omsi_content::stars::face_of finds it) and its
// axes: fu and fv across it, fn out through its middle. A direction w on the face lies at
// (fu.w, fv.w) / fn.w on it (-1..1 each), and the place (x, y) points along fn + fu x + fv y.
float star_face(float3 e, out float3 fu, out float3 fv, out float3 fn) {
    float3 a = abs(e);
    bool on_x = a.x >= a.y && a.x >= a.z;
    bool on_y = !on_x && a.y >= a.z;
    float along = on_x ? e.x : (on_y ? e.y : e.z);
    float s = along > 0.0 ? 1.0 : -1.0;
    fu = on_x ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0);
    fv = (on_x || on_y) ? float3(0.0, 0.0, 1.0) : float3(0.0, 1.0, 0.0);
    fn = (on_x ? float3(1.0, 0.0, 0.0) : (on_y ? float3(0.0, 1.0, 0.0) : float3(0.0, 0.0, 1.0))) * s;
    return (on_x ? 0.0 : (on_y ? 2.0 : 4.0)) + (s > 0.0 ? 0.0 : 1.0);
}

// The place of direction w on the face of axes fu, fv, fn.
float2 star_face_uv(float3 fu, float3 fv, float3 fn, float3 w) {
    return float2(dot(fu, w), dot(fv, w)) / dot(fn, w);
}

// The stars at this pixel (colour as stored, 0..1 a channel): each a square of points of its
// size (at least 1 pixel, at most 2) in its class's colour, as bright as its size up to 1. e
// the view ray in the stars' frame (unit), ex and ey how it changes a pixel across and a
// pixel down. The pixel reads the cell it looks into and, only where a star next door could
// reach it, the neighbouring ones on its nearer sides; the second layer only under a full
// first one.
float3 stars(float3 e, float3 ex, float3 ey) {
    float3 col = 0.0;
    float n = stars_y.w;
    float xx = dot(ex, ex);
    float xy = dot(ex, ey);
    float yy = dot(ey, ey);
    float det = max(xx * yy - xy * xy, 1e-30);
    float3 fu;
    float3 fv;
    float3 fn;
    float face = star_face(e, fu, fv, fn);
    float2 uv = star_face_uv(fu, fv, fn, e);
    float2 g = (uv * 0.5 + 0.5) * n;
    float2 own = floor(g);
    float2 at = g - own;
    // how many cells a pixel spans at most along either of the face's axes: a pixel's step
    // (radians) at most sqrt(2) x the longer one along an axis, a radian 1 + |uv|^2 of the
    // face's place there at most (the place's -1..1 n cells)
    float span = 0.71 * n * (1.0 + dot(uv, uv)) * sqrt(max(xx, yy));
    float2 to_uv = 1.0 / float2(6.0 * n, 2.0 * n);
    // (a star reaches a pixel from a cell next door only when the pixel lies within `span`
    // of that side; `span` is well under half a cell, so one side an axis at most)
    float2 side = at < 0.5 ? -1.0 : 1.0;
    float2 near = min(at, 1.0 - at) < span ? 1.0 : 0.0;
    // (the cells in turn: own, across, up, diagonal)
    float2 step = 0.0;
    [loop] for (int k = 0; k < 4; k++) {
        float2 cell = own + step * side;
        [branch] if (all(step <= near) && all(cell >= 0.0) && all(cell < n)) {
            [loop] for (int layer = 0; layer < 2; layer++) {
                float2 texel = float2(face * n + cell.x, layer * n + cell.y) + 0.5;
                float4 t = tex2Dlod(t_stars, float4(texel * to_uv, 0.0, 0.0));
                // (a point-sampled 8-bit texel: byte / 255 to well under a byte's step, so
                // the bytes are read without rounding)
                float code = t.b * 255.0;
                // (the second layer holds a star only under a full first one)
                if (code < 0.5) break;
                float2 place = (cell + (t.rg * 255.0 + 0.5) / 256.0) / n * 2.0 - 1.0;
                // where the star stands from the pixel's middle, in pixels (towards the star
                // w, not of unit length; a cell about the pixel's own is always well in front)
                float3 w = fn + fu * place.x + fv * place.y;
                float3 delta = w / dot(w, e) - e;
                float p = dot(ex, delta);
                float q = dot(ey, delta);
                float2 off = float2(yy * p - xy * q, xx * q - xy * p) / det;
                float s = (code - 1.0) / 254.0 * 8.0;
                float half_side = clamp(s, 1.0, 2.0) * 0.5;
                if (abs(off.x) < half_side && abs(off.y) < half_side) {
                    col += star_class_rgb(t.a * 255.0) * min(s, 1.0);
                }
            }
        }
        step = step.x > 0.5 ? float2(0.0, 1.0) : float2(1.0, step.y);
    }
    return min(col, 1.0);
}

// The sun and the moon towards d (linear light): the sun's picture where the sun stands, the
// moon's on a ball whose side towards the sun is lit (its phase). Neither shows below the
// horizon.
float3 sky_bodies(float3 d) {
    float3 col = 0.0;
    float3 up = float3(0.0, 0.0, 1.0);
    float3 sd = normalize(sun_dir.xyz);
    float ks = dot(d, sd);
    [branch] if (options.y > 0.5 && ks > 0.99 && d.z > -0.005) {
        float3 right = normalize(cross(sd, up) + float3(1e-5, 0.0, 0.0));
        float3 top = cross(right, sd);
        float2 q = float2(dot(d, right), dot(d, top)) / (ks * SUN_HALF);
        if (max(abs(q.x), abs(q.y)) < 1.0) {
            float4 uv = float4(0.5 + 0.5 * q.x, 0.5 - 0.5 * q.y, 0.0, 0.0);
            col += srgb_decode(tex2Dlod(t_sun, uv).rgb);
        }
    }
    float3 md = normalize(moon_dir.xyz);
    float km = dot(d, md);
    [branch] if (options.z > 0.5 && km > 0.999 && d.z > -0.005) {
        float3 right = normalize(cross(md, up) + float3(1e-5, 0.0, 0.0));
        float3 top = cross(right, md);
        float2 q = float2(dot(d, right), dot(d, top)) / (km * MOON_RADIUS);
        float q2 = dot(q, q);
        if (q2 < 1.0) {
            float3 n = right * q.x + top * q.y - md * sqrt(1.0 - q2);
            float lit = max(dot(n, sd), 0.0);
            float4 uv = float4(0.5 + 0.5 * q.x, 0.5 - 0.5 * q.y, 0.0, 0.0);
            float gain = lerp(0.45, 1.0, sky.w);
            col += srgb_decode(tex2Dlod(t_moon, uv).rgb) * lit * gain;
        }
    }
    return col;
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
    // the stars, as bright as the night lets them, fading out towards the horizon (with no
    // map, the Stars option off or a sky too light for them the lookup is skipped: the
    // condition is a constant)
    float3 e = star_frame(d);
    float3 e_dx = ddx(e);
    float3 e_dy = ddy(e);
    [branch] if (stars_y.w > 0.5 && stars_x.w > 0.0 && d.z > 0.0) {
        c += srgb_decode(stars(e, e_dx, e_dy) * stars_x.w) * saturate(elev * 8.0);
    }
    // the cloud layer, flat at its base height: the ground point under the view ray, in
    // tiles, the wind's drift added; none below the horizon, fading into it above. With no
    // cover (clear weather, or the Cloud picture option off) the picture, its lighting and
    // the sample are skipped: the condition is a constant, and the result is the same as
    // a cover of 0
    float t = clouds.w / max(d.z, 0.02);
    float2 ground = d.xy * (t / CLOUD_TILE) + clouds.yz;
    // (the gradients out here: a sample inside a branch has no derivatives of its own)
    float2 ground_dx = ddx(ground);
    float2 ground_dy = ddy(ground);
    // the sun and the moon, behind the clouds
    float3 bodies = sky_bodies(d);
    float hidden = 0.0;
    [branch] if (clouds.x > 0.001) {
        float shape = tex2Dgrad(t_clouds, ground, ground_dx, ground_dy).a;
        float closed = smoothstep(0.8, 1.0, clouds.x);
        float cover = shape * saturate((d.z - 0.01) * 7.0);
        // lit as the scene is: white by day, warm at sunrise, at night hardly lighter than the
        // night sky; the closed deck greys over
        float sun_up = saturate(sun_dir.z * 4.0 + 0.3);
        float3 lit = sun_color.rgb * 1.1 * sun_up + ambient.rgb * lerp(0.5, 1.5, sun_up) + sky_color.rgb * 0.3;
        float3 cloud_col = max(min(lit, 0.97), sky_c * 1.12) * (1.0 - 0.3 * closed);
        c = lerp(c, cloud_col, cover);
        hidden = cover;
    }
    c += bodies * (1.0 - hidden);
    // fog swallows the horizon, and a thick fog (a few hundred metres of sight) the whole sky
    float horizon = saturate(1.0 - elev / 0.12) * saturate(fog.w * 1500.0);
    float whole = saturate(fog.w * 150.0 - 0.15) * clamp(1.0 - elev / 1.2, 0.35, 1.0);
    float f = max(horizon, whole);
    return float4(lerp(srgb_encode(c), fog.rgb, f), 1.0);
}

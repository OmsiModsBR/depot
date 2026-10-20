// The interface, as `omsi-ui`'s shader draws it (Shader Model 3.0): shapes, text and icons in
// pixels, ribbons and markers in a 3D world, each draw with its layer's camera, layout box,
// rounded clip and opacity; and the HUD's pictures over the scene, as quads in pixels.
//
// Colour: where the device blends in linear light (it converts to sRGB after the blend), the
// pictures are read as sRGB, the vertex colour is made linear here and the target is written
// sRGB, as in the default renderer. Elsewhere all of it is worked on gamma-encoded, which
// differs only where alpha is partial (edges, fades).

// ---- vertex stage ----

float4x4 view_proj : register(c0);  // column-major, one column per register
float4 layout_box : register(c4);   // x, y, w, h: pixel vertices are laid out in it
float4 vs_params : register(c5);    // z metres per pixel per unit depth, w 1 = linear light
// xy: Direct3D 9 puts pixel centres on integer coordinates; the default renderer's are
// half a pixel off them, so the picture is moved by (-1/width, 1/height) of the viewport
float4 pixel_offset : register(c6);

struct VsIn {
    float3 pos : POSITION;
    float2 ext : TEXCOORD0;
    float2 width : TEXCOORD1;
    float2 uv : TEXCOORD2;
    float4 color : TEXCOORD3;  // sRGB, straight alpha
    float2 mode : TEXCOORD4;   // x 0 pixels, 1 world; y 1 = times the texture
};

struct VsOut {
    float4 clip : POSITION;
    float2 uv : TEXCOORD0;
    float4 color : TEXCOORD1;
    float tex : TEXCOORD2;
};

float3 to_linear(float3 c) {
    float3 lo = c / 12.92;
    float3 hi = pow((c + 0.055) / 1.055, 2.4);
    return lerp(hi, lo, step(c, 0.04045));
}

VsOut vs_main(VsIn v) {
    VsOut o;
    if (v.mode.x < 0.5) {
        // pixels of the target, whatever the viewport
        float2 p = v.pos.xy + v.ext * v.width.x;
        float2 q = (p - layout_box.xy) / layout_box.zw;
        o.clip = float4(q.x * 2.0 - 1.0, 1.0 - q.y * 2.0, 0.0, 1.0);
    } else {
        float4 c = mul(view_proj, float4(v.pos, 1.0));
        // the larger of the width in metres and in pixels at this depth
        float mpp = max(c.w, 0.01) * vs_params.z;
        float w = max(v.width.x, v.width.y * mpp);
        float3 wp = v.pos + float3(v.ext * w, 0.0);
        o.clip = mul(view_proj, float4(wp, 1.0));
        o.clip.z = 0.5 * o.clip.w;
    }
    o.clip.xy += pixel_offset.xy * o.clip.w;
    o.uv = v.uv;
    float3 rgb = v.color.rgb;
    if (vs_params.w > 0.5) {
        rgb = to_linear(rgb);
    }
    o.color = float4(rgb, v.color.a);
    o.tex = v.mode.y;
    return o;
}

// ---- pixel stage ----

float4 clip_box : register(c0);   // x0, y0, x1, y1 in target pixels
float4 ps_params : register(c1);  // x corner radius, y opacity
float4 ps_flags : register(c2);   // x 1 = the picture holds premultiplied alpha
sampler2D img : register(s0);

struct PsIn {
    float2 uv : TEXCOORD0;
    float4 color : TEXCOORD1;
    float tex : TEXCOORD2;
};

float rounded_mask(float2 p) {
    float r = ps_params.x;
    float2 lo = clip_box.xy;
    float2 hi = clip_box.zw;
    float2 c = (lo + hi) * 0.5;
    float2 half_size = (hi - lo) * 0.5;
    float2 q = abs(p - c) - half_size + r;
    float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
    return saturate(0.5 - d);
}

float4 ps_main(PsIn i, float2 vpos : VPOS) : COLOR0 {
    // (sampled for every pixel: a sample in a branch has no derivatives)
    float4 t = tex2D(img, i.uv);
    // VPOS is the pixel's corner; the default renderer's position is its centre
    float k = rounded_mask(vpos + 0.5) * ps_params.y;
    if (ps_flags.x > 0.5) {
        return t * k;
    }
    float4 c = i.color;
    if (i.tex > 0.5) {
        c = float4(c.rgb * t.rgb, c.a * t.a);
    }
    float a = c.a * k;
    return float4(c.rgb * a, a);
}

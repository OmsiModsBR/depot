// The scene, as the default renderer's scene shader draws its vanilla picture (Shader Model
// 3.0). Same inputs: a vertex is position, normal and uv; each draw brings its model matrix
// and two parameter vectors as instance data; the material is its uniform's 11 vectors and
// the light is the camera uniform's.
//
// Colour is worked on as it is stored, gamma-encoded: the texture is sampled without
// conversion, multiplied by the vertex light (clamped at 1) and written as it is, with the fog
// mixed in the same space, which is how the vanilla picture is defined.
//
// Not yet here (later phases): shadows and ambient occlusion (taken as 1), the map's lamps
// and the vehicles' interior lamps, the specular highlight, night, light and environment
// maps, bump maps, detail texturing, swaying trees, rain and snow, and the fog's way through
// the player's vehicle.

// ---- vertex stage ----

float4x4 view_proj : register(c0); // column-major, one column per register
float4 vs_cam_pos : register(c4);  // xyz the camera
// xy: Direct3D 9 puts pixel centres on integer coordinates; the default renderer's are
// half a pixel off them, so the picture is moved by (-1/width, 1/height) in clip space
float4 pixel_offset : register(c5);

struct VsIn {
    float3 pos : POSITION;
    float3 normal : NORMAL;
    float2 uv : TEXCOORD0;
    // instance data: the model matrix's columns, then its parameters
    float4 m0 : TEXCOORD1;
    float4 m1 : TEXCOORD2;
    float4 m2 : TEXCOORD3;
    float4 m3 : TEXCOORD4;
    float4 params : TEXCOORD5;  // x alpha multiplier, y visible, zw uv offset
    float4 params2 : TEXCOORD6; // w the surface code
};

struct VsOut {
    float4 clip : POSITION;
    float3 world : TEXCOORD0;
    float3 normal : TEXCOORD1;
    float2 uv : TEXCOORD2;
    float4 params : TEXCOORD3;
    float4 params2 : TEXCOORD4;
};

// A normal of no length (a mesh saved with zero normals) is upright instead of NaN.
float3 safe_normal(float3 v) {
    float d = dot(v, v);
    if (d > 1e-20) {
        return v * rsqrt(d);
    }
    return float3(0.0, 0.0, 1.0);
}

VsOut vs_main(VsIn i) {
    VsOut o;
    float4 wp = i.m0 * i.pos.x + i.m1 * i.pos.y + i.m2 * i.pos.z + i.m3;
    // Road surfaces are pulled towards the eye along the line of sight - the picture does
    // not move, only the depth - so they win over the flush ground beside them: the same
    // share of the way at every vertex, a little more for the objects lying on them (code
    // 1.25) and for a vehicle's shadow blob (2). Code 0.9 shades as a surface without the
    // pull; painted ground (0.75) stays put.
    float surf = i.params2.w;
    float3 cp = wp.xyz;
    if (surf > 0.9) {
        float3 to = wp.xyz - vs_cam_pos.xyz;
        float d = length(to);
        float decal = 0.0;
        if (surf > 1.5) {
            decal = 0.05 + 0.002 * d;
        } else if (surf > 1.1) {
            decal = 0.01 + 0.001 * d;
        }
        float pull = min(0.003 * d + decal, d * 0.3);
        cp = wp.xyz - to / max(d, 1e-3) * pull;
    }
    o.clip = mul(view_proj, float4(cp, 1.0));
    o.clip.xy += pixel_offset.xy * o.clip.w;
    o.world = wp.xyz;
    o.normal = safe_normal((i.m0 * i.normal.x + i.m1 * i.normal.y + i.m2 * i.normal.z).xyz);
    o.uv = i.uv + i.params.zw;
    o.params = i.params;
    o.params2 = i.params2;
    if (i.params.y < 0.5) {
        // invisible: the triangle is collapsed outside the depth range
        o.clip = float4(0.0, 0.0, 2.0, 1.0);
    }
    return o;
}

// ---- pixel stage ----

sampler2D t_diffuse : register(s0);
sampler2D t_trans : register(s1);

// the material's uniform
float4 m_color : register(c0);
float4 m_params : register(c1);  // x alpha mode (0 opaque, 1 test, 2 blend), y unlit, z has transmap, w transmap alpha
float4 m_extra : register(c2);   // x terrain, y detail repeats, z ground repeats, w has nightmap
float4 m_params2 : register(c3);
float4 m_emissive : register(c4);
float4 m_specular : register(c5);
float4 m_bump : register(c6);
float4 m_pbr : register(c7);
float4 m_flags : register(c8);   // y texture addressing (1 border, 2 mirror once), z border rgb, w border alpha
float4 m_ambient : register(c9);
float4 m_sway : register(c10);
// the light
float4 cam_pos : register(c11);
float4 sun_dir : register(c12);
float4 sun_color : register(c13);
float4 ambient : register(c14);
float4 fog : register(c15);
float4 sky_color : register(c16);
// x the alpha cut, y 1 when this draw cuts, z 1 for the depth-only parts that are fully opaque
float4 draw_flags : register(c17);

float3 srgb_encode(float3 c) {
    c = saturate(c);
    float3 lo = c * 12.92;
    float3 hi = 1.055 * pow(c, 1.0 / 2.4) - 0.055;
    return float3(c.r <= 0.0031308 ? lo.r : hi.r,
                  c.g <= 0.0031308 ? lo.g : hi.g,
                  c.b <= 0.0031308 ? lo.b : hi.b);
}

float3 srgb_decode(float3 c) {
    c = saturate(c);
    float3 lo = c / 12.92;
    float3 hi = pow((c + 0.055) / 1.055, 2.4);
    return float3(c.r <= 0.04045 ? lo.r : hi.r,
                  c.g <= 0.04045 ? lo.g : hi.g,
                  c.b <= 0.04045 ? lo.b : hi.b);
}

// [matl_texadress_border]: outside 0..1 the material's border colour
float4 diffuse_border(float4 tex, float2 uv) {
    if (m_flags.y > 0.5 && m_flags.y < 1.5 && (any(uv < 0.0) || any(uv > 1.0))) {
        float p = floor(m_flags.z + 0.5);
        float r = floor(p / 65536.0);
        float g = floor((p - r * 65536.0) / 256.0);
        float b = p - r * 65536.0 - g * 256.0;
        return float4(r / 255.0, g / 255.0, b / 255.0, m_flags.w);
    }
    return tex;
}

// [matl_texadress_mirroronce]: mirrored once about 0
float2 tex_address(float2 uv) {
    return m_flags.y > 1.5 ? abs(uv) : uv;
}

float4 ps_main(VsOut i) : COLOR0 {
    float2 duv = tex_address(i.uv);
    if (m_extra.x > 0.5) {
        // terrain: uv is tile space; the ground texture repeats extra.z times per tile
        duv = i.uv * m_extra.z;
    }
    // the transmap and the other maps stay where they are under a scrolling texture
    float2 buv = tex_address(i.uv - i.params.zw);
    // (both sampled before any branch: a sample's gradients need every pixel of the quad)
    float4 tex = diffuse_border(tex2D(t_diffuse, duv), duv);
    float4 tm = tex2D(t_trans, buv);
    if (draw_flags.z > 0.5) {
        // depth only: what the colour pass will cover completely
        clip(tex.a * m_color.a * i.params.x - 0.999);
        return float4(0.0, 0.0, 0.0, 0.0);
    }
    if (m_params.z > 0.5) {
        // [matl_transmap]: alpha from its own map (a map without an alpha channel is opaque)
        tex.a = m_params.w > 0.5 ? tm.a : 1.0;
        if (m_extra.x > 0.5 && m_params.x > 1.5) {
            // a painted ground layer: only the coverage ramp is sharpened
            tex.a = smoothstep(0.32, 0.68, tex.a);
        }
    }
    float mode = m_params.x;
    if (draw_flags.y > 0.5 && mode > 0.5 && mode < 1.5) {
        clip(tex.a - draw_flags.x);
    }
    float3 n = safe_normal(i.normal);
    float ndl = max(dot(n, sun_dir.xyz), 0.0);
    // three lights: the sun (A), the light from above (B) and the ambient light (C)
    float from_above = 0.5 + 0.5 * n.z;
    float shadow = 1.0;
    float ao = 1.0;
    float3 light = sun_color.rgb * sun_dir.w * ndl * shadow
        + (sky_color.rgb * from_above * (0.6 + 0.4 * shadow) + ambient.xyz) * ao;
    // the material's diffuse colour takes the sun and the light from above, its ambient
    // colour the ambient light
    float3 ambient_light = ambient.xyz * ao;
    float3 mat_light = m_color.rgb * (light - ambient_light) + m_ambient.rgb * ambient_light;
    float3 encoded;
    if (m_params.y > 0.5) {
        // unlit: the texture times the material's colour and its emissive colour
        encoded = srgb_encode(srgb_decode(tex.rgb) * (m_color.rgb + m_emissive.rgb));
    } else {
        float3 v = saturate(m_emissive.rgb + mat_light);
        encoded = tex.rgb * v;
    }
    float f = saturate(1.0 - exp(-distance(i.world, cam_pos.xyz) * fog.w));
    float3 rgb = lerp(encoded, fog.xyz, f);
    float a = tex.a * m_color.a;
    if (mode < 0.5) {
        a = 1.0;
    }
    return float4(rgb, a * i.params.x);
}

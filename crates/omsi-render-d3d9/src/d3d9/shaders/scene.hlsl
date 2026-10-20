// The scene, as the default renderer's scene shader draws its vanilla picture (Shader Model
// 3.0). Same inputs: a vertex is position, normal and uv; each draw brings its model matrix
// and two parameter vectors as instance data; the material is its uniform's 11 vectors and
// the light is the camera uniform's.
//
// Colour is worked on as it is stored, gamma-encoded: the texture is sampled without
// conversion, multiplied by the vertex light (clamped at 1) and written as it is, with the fog
// mixed in the same space, which is how the vanilla picture is defined. What the default
// renderer adds in linear light (the highlight, the night map) is added in linear light here.
//
// The material's maps: the light map (and a terrain's detail texture), the night map (and a
// terrain's night map), the sphere map of [matl_envmap] with its mask and its bump map, and
// the D3D material's specular highlight, lit at the vertex.
//
// The lamps: the map's lamps and the vehicles' lights from the grid of cells round the
// camera, and the vehicles' interior lamps on the meshes they light, read from two float
// textures (the light list, four texels a light, and the grid's light indices).
//
// Not yet here (later phases): shadows and ambient occlusion (taken as 1), the tile light
// maps (the splines' and the ground's lamps baked into the map), the detail noise, swaying
// trees, rain and snow (and the wet sheen on a reflecting surface), and the fog's way
// through the player's vehicle.

// ---- vertex stage ----

float4x4 view_proj : register(c0); // column-major, one column per register
float4 vs_cam_pos : register(c4);  // xyz the camera
// xy: Direct3D 9 puts pixel centres on integer coordinates; the default renderer's are
// half a pixel off them, so the picture is moved by (-1/width, 1/height) in clip space
float4 pixel_offset : register(c5);
float4 vs_sun_dir : register(c6);
float4 vs_sun_color : register(c7);
float4 vs_sky_color : register(c8);
float4 vs_cam_right : register(c9);
float4 vs_cam_up : register(c10);
// the material's specular colour (w its power, 0 = none) and its params (y unlit)
float4 vs_specular : register(c11);
float4 vs_params : register(c12);
// x the lights in the light list
float4 vs_lamps : register(c13);

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
    // the highlight from the sun and from the light above, and where the sphere map is read
    float3 spec_sun : TEXCOORD5;
    float3 spec_sky : TEXCOORD6;
    float2 env_uv : TEXCOORD7;
    // the instance's interior lamps (`interior_lamps`): x the first in the list, y how many,
    // z a plain brightness instead (-1: the lamps)
    float3 lamps : TEXCOORD8;
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
    // the interior lamp code, worked out once per vertex: the first of the instance's lamps
    // times 64 plus how many (no more than the list holds), or below 1 a plain brightness
    float code = floor(i.params2.z + 0.5);
    float first = floor(code / 64.0);
    float count = clamp(min(code - first * 64.0, vs_lamps.x - first), 0.0, 63.0);
    o.lamps = i.params2.z < 1.0 ? float3(0.0, 0.0, saturate(i.params2.z)) : float3(first, count, -1.0);
    // Direct3D's specular term at the vertex: the sun (light A) and the light from straight
    // above (light B), each (N.H)^power with the local viewer's half vector where the light
    // falls on the face, times the material's specular colour, at most 1
    float3 n = o.normal;
    float3 v = normalize(vs_cam_pos.xyz - wp.xyz);
    float3 s0 = float3(0.0, 0.0, 0.0);
    float3 s1 = float3(0.0, 0.0, 0.0);
    if (vs_specular.w > 0.0 && vs_params.y < 0.5) {
        float p = vs_specular.w;
        float3 l = vs_sun_dir.xyz;
        if (dot(n, l) > 0.0) {
            s0 = vs_sun_color.rgb * vs_sun_dir.w * pow(max(dot(n, normalize(v + l)), 0.0), p);
        }
        if (n.z > 0.0) {
            s1 = vs_sky_color.rgb * pow(max(dot(n, normalize(v + float3(0.0, 0.0, 1.0))), 0.0), p);
        }
        float3 total = s0 + s1;
        float3 k = min(1.0, total) / max(total, 1e-6);
        s0 = min(1.0, s0 * k * vs_specular.rgb);
        s1 = min(1.0, s1 * k * vs_specular.rgb);
    }
    o.spec_sun = s0;
    o.spec_sky = s1;
    // Direct3D's sphere map (D3DTSS_TCI_SPHEREMAP) from the reflection in camera space:
    // u = Rx/m + 0.5, v = Ry/m + 0.5 with m = 2|R - (0, 0, 1)|
    float3 r = reflect(-v, n);
    float3 right = vs_cam_right.xyz;
    float3 up = vs_cam_up.xyz;
    float3 rc = float3(dot(r, right), dot(r, up), dot(r, cross(up, right)));
    float m = 2.0 * length(rc - float3(0.0, 0.0, 1.0));
    o.env_uv = rc.xy / max(m, 1e-6) + 0.5;
    if (i.params.y < 0.5) {
        // invisible: the triangle is collapsed outside the depth range
        o.clip = float4(0.0, 0.0, 2.0, 1.0);
    }
    return o;
}

// ---- pixel stage ----

sampler2D t_diffuse : register(s0);
sampler2D t_trans : register(s1);
sampler2D t_night : register(s2);
sampler2D t_light : register(s3);  // a terrain's detail texture on a terrain material
sampler2D t_env : register(s4);
sampler2D t_envmask : register(s5);
sampler2D t_bump : register(s6);
// the lights, four texels a row, and the grid's light indices (-1 none), point-sampled
sampler2D t_lights : register(s7);
sampler2D t_grid : register(s8);
// x OR y of two bytes (x across, y down), point-sampled
sampler2D t_or : register(s10);

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
// xy one texel of the bump map
float4 bump_texel : register(c18);
// x 1 when the sphere map is there, y 1 when it has a mask, z 1 when a transmap is declared
float4 env_on : register(c19);
// the grid: cell = floor(world xy * x + yz) (`bind_lamps`), w the cells per side
float4 light_grid : register(c20);
// x 1 / the light texture's rows, y 1 / the grid texture's width, z 1 / its height, w the
// places per cell (0: no grid)
float4 lamp_tex : register(c21);
// x the lights in the light texture
float4 lamp_count : register(c22);
// x the places per cell of the grid, as a loop count (0: no grid)
int4 lamp_places : register(i0);
// [matl_texadress_border]: the border colour (rgb, alpha), unpacked from the flags
float4 border : register(c24);

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

// [matl_texadress_border]: outside 0..1 the material's border colour
float4 diffuse_border(float4 tex, float2 uv) {
    float2 off = abs(uv - 0.5);
    if (m_flags.y > 0.5 && m_flags.y < 1.5 && max(off.x, off.y) > 0.5) {
        return border;
    }
    return tex;
}

// [matl_texadress_mirroronce]: mirrored once about 0
float2 tex_address(float2 uv) {
    return m_flags.y > 1.5 ? abs(uv) : uv;
}

// Vector `k` of light `li`, as the device packs them (`scene::pack_light`): xyz the
// position, w 1 for a point light that lights every surface; rgb the colour times the
// intensity, w 1 / the radius; the spot's direction and the cosine of its outer cone (below
// -1.5 a point light); the cosine of the inner cone, the core radius squared, the full-light
// radius squared and the reach of an interior lamp squared.
float4 light_vec(float li, float k) {
    return tex2Dlod(t_lights, float4((k + 0.5) * 0.25, (li + 0.5) * lamp_tex.x, 0.0, 0.0));
}

// The lights the grid cell of `p` names. A light is named in every cell its range touches,
// so the point's own cell holds every light that reaches it. Full within an eighth of the
// range, then falling with the square of the distance to nothing at the range, three and
// three quarters strong (the default renderer's lamp pools); `map_k` how much of the map's
// lamps the surface takes, a vehicle's own point lights lighting every surface.
float3 point_lights(float3 p, float3 n, float map_k) {
    float3 sum = float3(0.0, 0.0, 0.0);
    // the cell, which must be on the grid (no grid: none of side 0, see `bind_lamps`)
    float2 c = floor(p.xy * light_grid.x + light_grid.yz);
    float2 off = abs(c + 0.5 - 0.5 * light_grid.w);
    if (max(off.x, off.y) > 0.5 * light_grid.w - 0.25) {
        return sum;
    }
    float row = (c.y + 0.5) * lamp_tex.z;
    float base = c.x * lamp_tex.w;
    for (int j = 0; j < lamp_places.x; j++) {
        float li = tex2Dlod(t_grid, float4((base + j + 0.5) * lamp_tex.y, row, 0.0, 0.0)).r;
        if (li < 0.0) {
            break;
        }
        float4 pos = light_vec(li, 0.0);
        float4 color = light_vec(li, 1.0);
        float4 dir = light_vec(li, 2.0);
        float4 extra = light_vec(li, 3.0);
        float3 d = pos.xyz - p;
        float dist2 = dot(d, d);
        float dist = sqrt(dist2);
        // (nothing at the range and beyond: no test needed)
        float att = min(1.0, extra.z / max(dist2, 0.01)) * saturate(1.0 - dist * color.w);
        float3 to = d / max(dist, 0.01);
        float ndl = max(dot(n, to), 0.15);
        // a spot: full inside the inner cone, nothing outside the outer one
        float spot = smoothstep(dir.w, max(extra.x, dir.w + 1e-3), dot(-to, dir.xyz));
        float k = dir.w >= -1.5 ? spot : max(pos.w, map_k);
        sum += color.rgb * (att * 3.75 * ndl * k);
    }
    return sum;
}

// [interiorlight]: a vehicle's saloon lamps on a mesh that names them (`lamps`, from the
// instance's code: the first and how many, or a plain brightness, a passenger standing in a
// lit bus). Each lamp is Direct3D's point light, 1 / (d / core)^2 within its reach, times
// N.L, and the sum saturates as a vertex colour does.
float3 interior_lamps(float3 p, float3 n, float3 lamps) {
    if (lamps.z >= 0.0) {
        return float3(1.0, 0.96, 0.84) * lamps.z;
    }
    float3 sum = float3(0.0, 0.0, 0.0);
    [loop] for (int i = 0; i < 63; i++) {
        if (i >= lamps.y) {
            break;
        }
        float li = lamps.x + i;
        float4 pos = light_vec(li, 0.0);
        float4 color = light_vec(li, 1.0);
        float4 extra = light_vec(li, 3.0);
        float3 d = pos.xyz - p;
        float dist2 = max(dot(d, d), 1e-4);
        float reach = dist2 < extra.w ? 1.0 : 0.0;
        sum += color.rgb * (extra.y / dist2 * max(dot(n, d * rsqrt(dist2)), 0.0) * reach);
    }
    return min(sum, 1.0);
}

// x OR y for whole numbers 0..255, from the table of them (`or_table`)
float or_byte(float x, float y) {
    return tex2Dlod(t_or, float4((x + 0.5) / 256.0, (y + 0.5) / 256.0, 0.0, 0.0)).r * 255.0;
}

// The reflection's weight of a material without a mask: the factor x 255 x its light (each
// channel at most 1), truncated, put together as one packed colour without saturation, so
// that a factor over 1 spills into the next channel (`scene::env_factor`).
float3 texture_factor(float factor, float3 light) {
    float3 m = floor(saturate(light) * max(factor, 0.0) * 255.0);
    // each channel's bytes: the lowest, the next and (blue's) the third (all whole and not
    // negative: the remainders need no fmod)
    float3 above = floor(m / 256.0);
    float3 low = m - above * 256.0;
    float3 next = above - floor(above / 256.0) * 256.0;
    float third = floor(above.b / 256.0);
    third -= floor(third / 256.0) * 256.0;
    // red takes green's second byte and blue's third, green blue's second (nothing to take
    // when every channel is below 256)
    float r = or_byte(or_byte(low.r, next.g), third);
    return float3(r, or_byte(low.g, next.b), low.b) / 255.0;
}

float4 ps_main(VsOut i) : COLOR0 {
    float2 duv = tex_address(i.uv);
    bool terrain = m_extra.x > 0.5;
    if (terrain) {
        // terrain: uv is tile space; the ground texture repeats extra.z times per tile
        duv = i.uv * m_extra.z;
    }
    // the transmap and the other maps stay where they are under a scrolling texture
    float2 buv = tex_address(i.uv - i.params.zw);
    // a terrain's night map lies upside down in its tile; its detail texture repeats
    float2 nuv = terrain ? float2(i.uv.x, 1.0 - i.uv.y) : buv;
    float2 luv = terrain ? i.uv * m_extra.y : buv;
    // (every map sampled before any branch: a sample's gradients need every pixel of the quad)
    float4 tex = diffuse_border(tex2D(t_diffuse, duv), duv);
    float diffuse_a = tex.a;
    float4 tm = tex2D(t_trans, buv);
    float4 nm = tex2D(t_night, nuv);
    float4 lm = tex2D(t_light, luv);
    float4 mask = tex2D(t_envmask, duv);
    // [matl_bumpmap]: the slope of the height map (alpha) moves the sphere-map lookup, a
    // byte of height difference counting as a signed-byte step
    float h = tex2D(t_bump, duv).a;
    float hx = tex2D(t_bump, duv + float2(bump_texel.x, 0.0)).a;
    float hy = tex2D(t_bump, duv + float2(0.0, bump_texel.y)).a;
    float2 env_uv = i.env_uv;
    if (m_bump.y > 0.5) {
        env_uv += clamp(float2(h - hx, h - hy) * 2.0, -1.0, 1.0) * m_bump.x;
    }
    float4 env = tex2D(t_env, env_uv);
    if (draw_flags.z > 0.5) {
        // depth only: what the colour pass will cover completely
        clip(tex.a * m_color.a * i.params.x - 0.999);
        return float4(0.0, 0.0, 0.0, 0.0);
    }
    if (terrain && m_extra.y > 0.0) {
        // the detail texture, multiplied in linear light
        tex.rgb = srgb_encode(srgb_decode(tex.rgb) * srgb_decode(lm.rgb));
    }
    if (m_params.z > 0.5) {
        // [matl_transmap]: alpha from its own map (a map without an alpha channel is opaque)
        tex.a = m_params.w > 0.5 ? tm.a : 1.0;
        if (terrain && m_params.x > 1.5) {
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
    // the map's lamps: not on a [nomaplighting] material (params.y 0.25), on a
    // [LightMapMapping] one (0.35, lit by the tile light map alone) or on a tree's leaves
    // (0.15)
    float map_k = (m_params.y > 0.1 && m_params.y < 0.45) ? 0.0 : 1.0;
    float3 lamp_light = point_lights(i.world, n, map_k);
    float3 light = sun_color.rgb * sun_dir.w * ndl * shadow
        + (sky_color.rgb * from_above * (0.6 + 0.4 * shadow) + ambient.xyz) * ao + lamp_light;
    // the material's diffuse colour takes the sun and the light from above, its ambient
    // colour the ambient light
    float3 ambient_light = ambient.xyz * ao;
    float3 mat_light = m_color.rgb * (light - ambient_light) + m_ambient.rgb * ambient_light;
    bool terrain_night = m_params.y < 0.5 && terrain && m_extra.w > 0.5 && m_extra.w < 1.5;
    // what is drawn, as stored (`base`), and what goes on it in linear light: the unlit look
    // takes its colours there, the specular highlight and the night map are added there
    // (one decode and one encode for all of them)
    bool unlit = m_params.y > 0.5;
    float3 base = tex.rgb;
    float3 scale = m_color.rgb + m_emissive.rgb;
    float3 spec = float3(0.0, 0.0, 0.0);
    if (!unlit) {
        float3 v = saturate(m_emissive.rgb + mat_light
            + m_color.rgb * interior_lamps(i.world, n, i.lamps));
        if (m_params2.x > 0.5 && !terrain) {
            // [matl_lightmap], as strong as the instance says, over what is lit
            v = v + lm.rgb * saturate(i.params2.x) * (1.0 - v);
        }
        if (terrain_night) {
            v = min(v + nm.rgb * sun_color.w * saturate(i.params2.y), 1.0);
        }
        base = tex.rgb * v;
        scale = float3(1.0, 1.0, 1.0);
        spec = i.spec_sun * shadow + i.spec_sky;
    }
    // [matl_nightmap]: by night, or always for one something else switches
    bool night_map = m_extra.w > 0.5 && !terrain_night;
    bool switched = m_extra.w > 1.5;
    float night = (switched ? 1.0 : sun_color.w * saturate(i.params2.y)) * (night_map ? 1.0 : 0.0);
    float3 encoded = base;
    if (unlit || any(spec > 0.0) || night > 0.0) {
        float3 lin = saturate(srgb_decode(base) * scale + spec) + srgb_decode(nm.rgb) * night;
        encoded = srgb_encode(lin);
    }
    if (m_params2.y > 0.0) {
        // [matl_envmap]: mixed in as stored, by the mask's alpha (or the diffuse alpha where
        // a transmap is declared) times the light, else by the factor times the ambient
        // light and that light; the light is the mean of the lamps' and the sun's, at most 1
        bool has_mask = env_on.y > 0.5;
        bool declared = env_on.z > 0.5;
        float g = saturate(dot(lamp_light, 1.0 / 3.0) + dot(sun_color.rgb, 1.0 / 3.0));
        float3 kk;
        if (has_mask || declared) {
            kk = (has_mask ? mask.a : diffuse_a) * g;
        } else {
            kk = texture_factor(m_params2.y, ambient.rgb + g);
        }
        encoded = lerp(encoded, env.rgb, kk * env_on.x);
    }
    float f = saturate(1.0 - exp(-distance(i.world, cam_pos.xyz) * fog.w));
    float3 rgb = lerp(encoded, fog.xyz, f);
    float a = tex.a * m_color.a;
    if (mode < 0.5) {
        a = 1.0;
    }
    return float4(rgb, a * i.params.x);
}

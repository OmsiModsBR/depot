// Light coronas (Shader Model 3.0): the camera-facing sprites a light shows round its lamp
// (headlights, tail and brake lights, indicators, beacons, street lamps), the cone a light
// draws in fog and the halo round a light in fog. A port of the default renderer's corona
// shader (`corona.wgsl`, vanilla picture), over the same sprite records.
//
// A sprite is one instance of a unit quad: its record is five vectors (position and size;
// colour and brightness; direction and the outer cone's cosine; the up axis and the rotating
// mode; the inner cone's cosine, the z offset, the parameter bits and the kind). The kind is
// 0 for a glow, 1 for the cone in fog, 2 for the halo. (Smoke has its own pass, not here.)
//
// The picture is added over the scene with `src + dst * (1 - src)`, which the device sets up.

// ---- vertex stage ----

float4x4 view_proj : register(c0); // column-major, one column per register
float4 cam_pos : register(c4);     // xyz the camera
// xy: Direct3D 9 puts pixel centres on integer coordinates; the default renderer's are
// half a pixel off them, so the picture is moved by (-1/width, 1/height) in clip space
float4 pixel_offset : register(c5);
float4 cam_right : register(c100);
float4 cam_up : register(c101);
float4 fog : register(c102); // w the fog's density

struct VsIn {
    float2 corner : TEXCOORD0;
    float4 pos_size : TEXCOORD1; // xyz relative to the render origin, w the world radius
    float4 color : TEXCOORD2;    // rgb, a the brightness (0..2)
    float4 dir : TEXCOORD3;      // xyz facing direction (0 = omni), w the outer cone's cosine
    float4 up : TEXCOORD4;       // xyz up / rotation axis, w the rotating mode
    float4 extra : TEXCOORD5;    // x the inner cone's cosine, y the z offset, z bits, w kind
};

struct VsOut {
    float4 clip : POSITION;
    float2 uv : TEXCOORD0;
    float4 color : TEXCOORD1;
    // x 1 for a light's cone in fog, yz the cone's inner and outer half angles (radians)
    float4 beam : TEXCOORD2;
};

VsOut vs_main(VsIn i) {
    VsOut o;
    // (a sprite that is not drawn lies beyond the far plane)
    o.clip = float4(0.0, 0.0, 2.0, 1.0);
    o.uv = float2(0.0, 0.0);
    o.color = float4(0.0, 0.0, 0.0, 0.0);
    o.beam = float4(0.0, 0.0, 0.0, 0.0);

    float3 pos = i.pos_size.xyz;
    float size = i.pos_size.w;
    float2 c = i.corner;
    float3 to_cam = cam_pos.xyz - pos;
    float dist = length(to_cam);
    float3 view_dir = to_cam / max(dist, 0.001);

    if (i.extra.w > 2.5) {
        return o;
    }

    if (i.extra.w > 1.5) {
        // the halo round a light in fog: licht.bmp turned to the viewer, pulled towards
        // them by its size, seen from in front of the light
        float k = i.color.a;
        if (length(i.dir.xyz) > 0.5) {
            float3 axis = normalize(i.dir.xyz);
            float inner = i.extra.x;
            float outer = i.dir.w;
            float ang = acos(clamp(dot(axis, view_dir), -1.0, 1.0));
            float from_side = (ang > inner) ? 1.0 : 0.0;
            if (outer > inner + 0.0001) {
                from_side = clamp((ang - inner) / (outer - inner), 0.0, 1.0);
            }
            float side = min(from_side, clamp(3.0 - ang, 0.0, 1.0));
            float front = (1.0 - side) * clamp(1.0 - (ang - 2.0) * 0.5, 0.5, 1.0);
            k = k * front * front * ((ang < 1.5705) ? 1.0 : 0.0);
        }
        float3 fwd = normalize(cross(cam_up.xyz, cam_right.xyz));
        float on_screen = 2.0 * dot(fwd, -view_dir) - 1.0;
        float far_k = clamp(1.0 - dist / max(i.extra.y, 1.0), 0.0, 1.0);
        k = k * max(on_screen, 0.0) * far_k;
        float3 right = normalize(cross(float3(0.0, 0.0, 1.0), view_dir));
        float3 upv = cross(view_dir, right);
        float3 centre = pos + view_dir * min(size, dist * 0.9);
        o.clip = mul(view_proj, float4(centre + (right * c.x + upv * c.y) * size, 1.0));
        o.uv = c * 0.5 + 0.5;
        o.color = float4(i.color.rgb, k);
        if (k <= 0.001 || on_screen <= 0.01) {
            o.clip = float4(0.0, 0.0, 2.0, 1.0);
        }
    } else if (i.extra.w > 0.5) {
        // a light's cone in the fog: a flat fan from the lamp, `size` in radius, spreading
        // to the outer half angle either side of the light's axis and turned about that
        // axis to face the eye; the pixel stage cuts the fan out of this square
        float3 axis = normalize(i.dir.xyz);
        float inner = i.extra.x;
        float outer = i.dir.w;
        float3 side = cross(view_dir, axis);
        if (length(side) < 0.001) {
            side = cross(float3(0.0, 0.0, 1.0), axis);
        }
        side = normalize(side);
        float ang = acos(clamp(dot(axis, view_dir), -1.0, 1.0));
        float from_side = (ang > inner) ? 1.0 : 0.0;
        if (outer > inner + 0.0001) {
            from_side = clamp((ang - inner) / (outer - inner), 0.0, 1.0);
        }
        float behind = clamp(3.0 - ang, 0.0, 1.0);
        float3 fwd = normalize(cross(cam_up.xyz, cam_right.xyz));
        float on_screen = 2.0 * dot(fwd, -view_dir) - 1.0;
        float far_k = clamp(1.0 - dist / max(i.extra.y, 1.0), 0.0, 1.0);
        float k = i.color.a * min(from_side, behind) * max(on_screen, 0.0) * far_k;
        o.clip = mul(view_proj, float4(pos + (axis * c.x + side * c.y) * size, 1.0));
        o.uv = c;
        o.color = float4(i.color.rgb, k);
        o.beam = float4(1.0, inner, outer, 0.0);
        if (k <= 0.001 || on_screen <= 0.01) {
            o.clip = float4(0.0, 0.0, 2.0, 1.0);
        }
    } else {
        // a glow (or a raindrop's streak: a cone of -2, a snowflake's: -3)
        bool streak = i.dir.w < -1.5 && i.dir.w > -2.5;
        bool directed = length(i.dir.xyz) > 0.5 && !streak;
        float brightness = i.color.a;
        if (directed) {
            // seen inside its outer cone, in full inside the inner one, linearly between
            float ang = acos(clamp(dot(normalize(i.dir.xyz), view_dir), -1.0, 1.0));
            float outer = acos(clamp(i.dir.w, -1.0, 1.0));
            float inner = (i.extra.x >= -1.0) ? acos(clamp(i.extra.x, -1.0, 1.0)) : 0.0;
            brightness = brightness * clamp((outer - ang) / max(outer - inner, 0.0001), 0.0, 1.0);
        }
        // the light that reaches the eye through the fog: exp(-density x distance)
        if (!streak) {
            brightness = brightness * exp(-dist * max(fog.w, 0.0));
        }
        // a star grows with the light's strength (parameter bit 8); every sprite's strength
        // stops at 1
        bool star = frac(floor(i.extra.z + 0.5) / 16.0) >= 0.5 && !streak;
        float grow = star ? brightness : 1.0;
        if (!streak) {
            brightness = min(brightness, 1.0);
        }
        // the sprite's axes: flat and facing its direction (rotating 0), turned to the
        // viewer about its up axis (1), or turned to the viewer entirely (2)
        int mode = (int)(i.up.w + 0.5);
        float3 right = normalize(cross(float3(0.0, 0.0, 1.0), view_dir));
        float3 upv = cross(view_dir, right);
        float3 axis = float3(0.0, 0.0, 1.0);
        if (length(i.up.xyz) > 0.5) {
            axis = normalize(i.up.xyz);
        }
        if (!streak && mode == 0 && directed) {
            float3 n = normalize(i.dir.xyz);
            float3 u = axis - n * dot(axis, n);
            if (length(u) < 0.01) {
                u = float3(0.0, 0.0, 1.0) - n * n.z;
            }
            upv = normalize(u);
            right = cross(upv, n);
        } else if (!streak && mode == 1) {
            float3 r = cross(axis, view_dir);
            if (length(r) > 0.01) {
                right = normalize(r);
                upv = axis;
            }
        }
        // a minimum size on the screen in the distance; the 0.9 is the original's sprite
        // drawn a little inside the diameter its size names
        float radius = size;
        float2 stretch = float2(1.0, 1.0);
        float3 up_axis = upv;
        float3 pulled = float3(0.0, 0.0, 0.0);
        if (!streak) {
            radius = max(size * grow, dist * 0.002) * 0.9;
        } else {
            stretch = float2(0.06, 4.0);
            up_axis = float3(0.0, 0.0, 1.0);
        }
        if (!streak) {
            // moved towards the viewer by its z offset, so that a lamp inside its housing
            // shows (without one: half its size, at most half a metre)
            float pull = (i.extra.y >= 0.0) ? i.extra.y : min(radius * 0.5, 0.5);
            pulled = view_dir * min(pull, dist * 0.9);
        }
        float3 wp = pos + (right * c.x * stretch.x + up_axis * c.y * stretch.y) * radius + pulled;
        o.clip = mul(view_proj, float4(wp, 1.0));
        o.uv = c * 0.5 + 0.5;
        o.color = float4(i.color.rgb, brightness);
        // (not `<= 0.001`: a NaN brightness is not smaller)
        if (!(brightness > 0.001) || brightness > 1.0e30) {
            o.clip = float4(0.0, 0.0, 2.0, 1.0);
        }
    }
    o.clip.xy += pixel_offset.xy * o.clip.w;
    return o;
}

// ---- pixel stage ----

sampler2D picture : register(s0);

float4 ps_main(VsOut i) : COLOR {
    // read the right way up: the sprite's v runs up the screen, the picture's down
    float3 t = tex2D(picture, float2(i.uv.x, 1.0 - i.uv.y)).rgb;
    // the cone's fan: its apex has uv (0, 1) and a rim vertex (sin a, 1 - cos a), a being
    // 0.05 inside the inner cone and rising to 0.9 pi/2 + 0.05 at the outer edge
    float r = length(i.uv);
    float phi = abs(atan2(i.uv.y, i.uv.x));
    float span = max(i.beam.z - i.beam.y, 0.0001);
    float a = 0.05 + 0.9 * 1.5708 * clamp((phi - i.beam.y) / span, 0.0, 1.0);
    float2 fan_uv = clamp(float2(0.0, 1.0) + min(r, 1.0) * float2(sin(a), -cos(a)), 0.0, 1.0);
    float tb = tex2D(picture, fan_uv).r;
    float inside = (r <= 1.0 && phi <= i.beam.z) ? 1.0 : 0.0;
    float3 shape = (i.beam.x > 0.5) ? float3(tb * inside, tb * inside, tb * inside) : t;
    return float4(i.color.rgb * shape * i.color.a, 1.0);
}

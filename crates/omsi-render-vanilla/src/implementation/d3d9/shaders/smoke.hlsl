// Smoke and particles (Shader Model 3.0): the camera-facing, alpha-blended squares a `[smoke]`
// system or a `[particle_emitter]` makes (exhaust, chimneys, fireworks, wheel smoke). A port
// of the smoke branch of the default renderer's corona shader (`corona.wgsl`, vanilla
// picture), over the same sprite records.
//
// A particle is one instance of a unit quad: its record is five vectors (position and size;
// colour and alpha; the cosine and sine of its spin; the ground it fades into, whether it has
// one, the z offset and how high the fade reaches). It is a pass of its own, so the scene's
// pixel shader keeps its instruction slots.

// ---- vertex stage ----

float4x4 view_proj : register(c0); // column-major, one column per register
float4 cam_pos : register(c4);     // xyz the camera
// xy: Direct3D 9 puts pixel centres on integer coordinates; the default renderer's are
// half a pixel off them, so the picture is moved by (-1/width, 1/height) in clip space
float4 pixel_offset : register(c5);
float4 cam_right : register(c100);

struct VsIn {
    float2 corner : TEXCOORD0;
    float4 pos_size : TEXCOORD1; // xyz relative to the render origin, w the half width
    float4 color : TEXCOORD2;    // rgb, a the particle's alpha
    float4 dir : TEXCOORD3;      // x the cosine and y the sine of the spin
    float4 up : TEXCOORD4;       // x the ground's z, y 1 when it has one, z the z offset, w the fade
    float4 extra : TEXCOORD5;    // (the kind, 3, is the default renderer's)
};

struct VsOut {
    float4 clip : POSITION;
    float2 uv : TEXCOORD0;
    float4 color : TEXCOORD1;
    // x how high this point is over the ground it fades into (m), y how high the fade
    // reaches (0: not faded)
    float2 ground : TEXCOORD2;
};

VsOut vs_main(VsIn i) {
    VsOut o;
    // (a particle that is not drawn lies beyond the far plane)
    o.clip = float4(0.0, 0.0, 2.0, 1.0);
    o.uv = float2(0.0, 0.0);
    o.color = float4(0.0, 0.0, 0.0, 0.0);
    o.ground = float2(0.0, 0.0);

    float3 pos = i.pos_size.xyz;
    float2 c = i.corner;
    float3 to_cam = cam_pos.xyz - pos;
    float dist = length(to_cam);
    float3 view_dir = to_cam / max(dist, 0.001);
    // facing the viewer, turned about the line of sight by the particle's own angle
    float3 side = cross(float3(0.0, 0.0, 1.0), view_dir);
    if (length(side) < 0.0001) {
        // (seen from straight above or below)
        side = cam_right.xyz;
    }
    float3 right0 = normalize(side);
    float3 up0 = cross(view_dir, right0);
    float3 right = right0 * i.dir.x + up0 * i.dir.y;
    float3 upv = up0 * i.dir.x - right0 * i.dir.y;
    // (drawn at 0.9 of its size, never smaller than a couple of pixels in the distance)
    float size = max(i.pos_size.w, dist * 0.002) * 0.9;
    float3 wp = pos + (right * c.x + upv * c.y) * size;
    o.clip = mul(view_proj, float4(wp, 1.0));
    if (i.up.z > 0.0) {
        // moved towards the eye in depth only: its place and size on the screen stay
        float4 nearer = mul(view_proj, float4(wp + view_dir * min(i.up.z, dist * 0.9), 1.0));
        o.clip.z = nearer.z / nearer.w * o.clip.w;
    }
    o.uv = c * 0.5 + 0.5;
    float a = min(i.color.a, 1.0);
    o.color = float4(i.color.rgb, a);
    // a particle that knows its ground fades out into it over its lowest part, instead of
    // being cut off by it in a straight line
    if (i.up.y > 0.5) {
        o.ground = float2(wp.z - i.up.x, i.up.w);
    }
    // (not `<= 0.001`: a NaN alpha is not smaller)
    if (!(a > 0.001) || a > 1.0e30) {
        o.clip = float4(0.0, 0.0, 2.0, 1.0);
    }
    o.clip.xy += pixel_offset.xy * o.clip.w;
    return o;
}

// ---- pixel stage ----

sampler2D picture : register(s0);
// rgb the scene's light on the particle: the ambient plus 0.6 of the sun's, at most 1.2
float4 light : register(c200);

float4 ps_main(VsOut i) : COLOR {
    // read the right way up: the sprite's v runs up the screen, the picture's down
    float4 t = tex2D(picture, float2(i.uv.x, 1.0 - i.uv.y));
    float over_ground = (i.ground.y > 0.0) ? smoothstep(0.0, i.ground.y, i.ground.x) : 1.0;
    return float4(i.color.rgb * t.rgb * light.rgb, saturate(t.a * i.color.a * over_ground));
}

// The wet road: a pass of its own over the surfaces whose texture carries [moisture] or
// [puddles], drawn with the scene's vertex shader (the same meshes, instances and depth)
// after the scene's batches, so the scene's pixel shader keeps its instruction slots.
//
// Under rain such a surface darkens and mirrors the sky a little, most where it is looked
// along (a Fresnel sheen), and pools of standing water spread over it as it soaks, which
// mirror the sky almost fully at grazing angles. It is the default renderer's vanilla wet
// road, worked on the stored (gamma-encoded) colour: the pass blends ONE, INVSRCALPHA, so
// the picture becomes the scene's colour times its darkening, mixed towards the sheen by
// the weight. The fog takes the effect away with the distance, as it covers the road.

sampler2D t_diffuse : register(s0);

// rgb the sheen's colour (as stored), w how wet the surfaces are (none under snow)
float4 sheen : register(c0);
// xyz the camera, w the fog's density
float4 eye : register(c1);
// xy the render origin's map position modulo the pattern's period (metres)
float4 origin : register(c2);
// x the alpha mode (0 opaque, 1 cut, 2 blended), y the alpha cut, z 1 on a terrain
float4 batch : register(c3);

struct PsIn {
    float3 world : TEXCOORD0;
    float3 normal : TEXCOORD1;
    float2 uv : TEXCOORD2;
    float4 params : TEXCOORD3;  // x alpha multiplier
    float4 params2 : TEXCOORD4; // w the surface code
};

// The pools' pattern repeats every this many metres (the default renderer's PATTERN_PERIOD).
static const float PERIOD = 1000.0;
// A road wet through has standing water on about this share of it.
static const float SPREAD = 0.37;

// A value in 0..1 for a lattice cell, the cell taken modulo the lattice's cells per period.
float hash_cell(float2 c, float cells) {
    float2 w = c - cells * floor(c / cells);
    float3 p = frac(w.xyx * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return frac((p.x + p.y) * p.z);
}

// Value noise of `freq` cells a metre, repeating every PERIOD metres.
float noise(float2 p, float freq, float2 offset) {
    float2 q = p * freq + offset;
    float cells = round(PERIOD * freq);
    float2 i = floor(q);
    float2 f = q - i;
    float2 u = f * f * (3.0 - 2.0 * f);
    float a = hash_cell(i, cells);
    float b = hash_cell(i + float2(1.0, 0.0), cells);
    float c = hash_cell(i + float2(0.0, 1.0), cells);
    float d = hash_cell(i + float2(1.0, 1.0), cells);
    return lerp(lerp(a, b, u.x), lerp(c, d, u.x), u.y);
}

// How much of the spot is standing water: the pools spread from the pattern's lowest spots
// as the road soaks, on what faces up only.
float puddle_coverage(float2 xy, float3 n, float wet) {
    float pn = noise(xy, 0.35, float2(17.3, -9.1)) * 0.6
        + noise(xy, 1.6, float2(-4.0, 8.0)) * 0.3
        + noise(xy, 5.0, float2(2.7, 11.3)) * 0.1;
    float threshold = 1.0 - wet * SPREAD;
    return smoothstep(threshold - 0.02, threshold + 0.02, pn) * smoothstep(0.75, 0.95, n.z);
}

float4 ps_main(PsIn i) : COLOR0 {
    // (sampled before any branch: a sample's gradients need every pixel of the quad)
    float alpha = tex2D(t_diffuse, i.uv).a;
    float3 n = normalize(i.normal);
    float coverage = 1.0;
    if (batch.x > 0.5 && batch.x < 1.5) {
        clip(alpha - batch.y);
    } else if (batch.x > 1.5) {
        coverage = saturate(alpha * i.params.x);
    }
    // a vehicle's part under its roof stays dry (the ground and the roads never are under one)
    float surface = i.params2.w;
    float outside = 1.0;
    if (batch.z < 0.5 && surface < -500.0) {
        float roof = -surface - 5000.0;
        outside = (i.world.z < roof - 0.3 && n.z > 0.5) ? 0.0 : 1.0;
    }
    float wet = sheen.w * outside;
    float3 v = normalize(i.world - eye.xyz);
    float facing = saturate(-dot(v, n));
    float fresnel = pow(1.0 - facing, 4.0);
    float puddle = puddle_coverage(i.world.xy + origin.xy, n, wet);
    float water = 0.02 + 0.98 * pow(1.0 - facing, 5.0);
    float weight = clamp(lerp(fresnel * wet * 0.85, water * wet, puddle), 0.0, 0.9);
    // the darkening is a quarter of the light at most, made in linear light: on the stored
    // colour that is about that factor to the power 1/2.2
    float dark = pow(lerp(1.0, 0.75, wet), 1.0 / 2.2);
    float keep = exp(-distance(i.world, eye.xyz) * eye.w) * coverage;
    float w = weight * keep;
    float d = 1.0 - (1.0 - dark) * keep;
    return float4(sheen.rgb * w, 1.0 - (1.0 - w) * d);
}

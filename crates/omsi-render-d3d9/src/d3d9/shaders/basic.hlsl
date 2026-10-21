// Basic textured pass of the legacy renderer (Shader Model 3.0).
//
// Alpha test and blending are not done here: they are Direct3D 9 render states applied after
// the pixel shader, so this shader only produces the colour and its alpha.

float4x4 mvp : register(c0); // column-major, one column per register

struct VsIn {
    float3 pos : POSITION;
    float3 normal : NORMAL;
    float2 uv : TEXCOORD0;
};

struct VsOut {
    float4 pos : POSITION;
    float2 uv : TEXCOORD0;
};

VsOut vs_main(VsIn i) {
    VsOut o;
    o.pos = mul(mvp, float4(i.pos, 1.0));
    o.uv = i.uv;
    return o;
}

sampler2D base_tex : register(s0);
float4 tint : register(c0);

float4 ps_main(VsOut i) : COLOR0 {
    return tex2D(base_tex, i.uv) * tint;
}

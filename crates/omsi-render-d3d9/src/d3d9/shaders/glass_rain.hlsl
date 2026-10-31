// Raindrops on glass: a pass of its own over the panes of materials with a [matl_raindropmap],
// drawn with the scene's vertex shader (the same meshes, instances and depth) after the
// scene's batches, so the scene's pixel shader keeps its instruction slots.
//
// The drops are a tiling picture (red the drop's shape, green when it forms, blue how it
// catches the light). How far the drops have formed along the pane's width is kept by the
// device in 32 places (`glass_rain::GlassRain`); a drop shows once that reaches the moment
// it forms, so the rain builds the drops up one by one and the wiper's stroke takes them
// away. The raindrop map says where on the pane drops can gather.

sampler2D t_map : register(s0);
sampler2D t_drops : register(s1);

// how far the drops have formed at 32 places across the pane's width, four to a vector
float4 wet[8] : register(c0);
// x the picture's repeats across the pane, y the drops' strongest opacity
float4 look : register(c8);
// rgb the light the drops catch
float4 tint : register(c9);

struct PsIn {
    float2 uv : TEXCOORD2;
    float4 params : TEXCOORD3;  // x alpha multiplier
};

// The places' values joined with straight lines between them.
float developed(float u) {
    float x = saturate(u) * 31.0;
    float w = 0.0;
    [unroll]
    for (int k = 0; k < 8; k++) {
        float4 at = float4(4.0 * k, 4.0 * k + 1.0, 4.0 * k + 2.0, 4.0 * k + 3.0);
        w += dot(wet[k], saturate(1.0 - abs(x - at)));
    }
    return w;
}

float4 ps_main(PsIn i) : COLOR {
    float4 map = tex2D(t_map, i.uv);
    float area = map.r * map.a;
    float4 drop = tex2D(t_drops, i.uv * look.x);
    float formed = saturate((developed(i.uv.x) - drop.g) * 8.0);
    float a = drop.r * formed * area * look.y * i.params.x;
    return float4(tint.rgb * lerp(0.4, 1.0, drop.b), a);
}

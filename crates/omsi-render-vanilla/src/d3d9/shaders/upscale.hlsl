// The render scale's up-scaling passes (Shader Model 3.0): the 3D picture, drawn smaller than
// the window, brought up to the window's size before the interface is drawn over it.
//
//   ps_bilinear  one read through the card's bilinear filter.
//   ps_easu      edge-adaptive upsampling (FSR 1's "EASU"), twelve point reads of the small
//                picture, written to a window-sized picture of its own.
//   ps_rcas      the sharpening pass (FSR 1's "RCAS") over that picture, five point reads,
//                written to the window's picture.
//
// ps_easu and ps_rcas are adapted from AMD's FidelityFX Super Resolution 1 (v1.0.2, commit
// a21ffb8f6c13233ba336352bdff293894c706575, ffx-fsr/ffx_fsr1.h), which is MIT licensed:
//
//   Copyright (c) 2021 Advanced Micro Devices, Inc. All rights reserved.
//   Permission is hereby granted, free of charge, to any person obtaining a copy of this
//   software and associated documentation files (the "Software"), to deal in the Software
//   without restriction, including without limitation the rights to use, copy, modify, merge,
//   publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons
//   to whom the Software is furnished to do so, subject to the following conditions:
//   The above copyright notice and this permission notice shall be included in all copies or
//   substantial portions of the Software.
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
//   INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
//   PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
//   FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//   OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//   DEALINGS IN THE SOFTWARE.
//
// What was changed for Shader Model 3.0: the reference reads four texels at once ("gather4"),
// which this shader model has not; the twelve texels are read one by one, at their centres.
// The reference's bit-trick approximations of 1/x and 1/sqrt(x) are the exact rcp() and
// rsqrt(), and the 16-bit paths are dropped. The algorithm is otherwise the reference's.

// ---- vertex stage ----

// xy: half a pixel of the target the pass draws into (Direct3D 9's pixel centres are half a
// pixel off), so that a texel is read where its pixel is
float4 half_pixel : register(c250);

struct VsOut {
    float4 clip : POSITION;
    float2 uv : TEXCOORD0;
};

VsOut vs_main(float2 corner : POSITION) {
    VsOut o;
    o.clip = float4(corner + half_pixel.xy, 0.0, 1.0);
    o.uv = float2(corner.x * 0.5 + 0.5, 0.5 - corner.y * 0.5);
    return o;
}

// ---- pixel stage ----

sampler2D src : register(s0);

// x, y the size of the picture being written, z, w one over the size of the picture read
float4 sizes : register(c220);
// the mapping of an output pixel to the small picture (FsrEasuCon's first constant)
float4 mapping : register(c221);
// x the sharpening pass's strength factor
float4 rcas : register(c222);

float4 ps_bilinear(float2 uv : TEXCOORD0) : COLOR0 {
    return float4(tex2Dlod(src, float4(uv, 0.0, 0.0)).rgb, 1.0);
}

// one texel of the small picture, by its integer position
float3 texel(float2 p) {
    return tex2Dlod(src, float4((p + 0.5) * sizes.zw, 0.0, 0.0)).rgb;
}

// the picture's "luma times 2", the reference's cheapest
float lum(float3 c) {
    return c.b * 0.5 + (c.r * 0.5 + c.g);
}

// the reference's FsrEasuTapF: one tap's weight into the accumulators
void easu_tap(inout float3 aC, inout float aW, float2 off, float2 dir, float2 len, float lob,
    float clp, float3 c) {
    float2 v;
    v.x = (off.x * dir.x) + (off.y * dir.y);
    v.y = (off.x * (-dir.y)) + (off.y * dir.x);
    v *= len;
    float d2 = min(v.x * v.x + v.y * v.y, clp);
    float wB = 0.4 * d2 - 1.0;
    float wA = lob * d2 - 1.0;
    wB *= wB;
    wA *= wA;
    wB = 1.5625 * wB - 0.5625;
    float w = wB * wA;
    aC += c * w;
    aW += w;
}

// the reference's FsrEasuSetF: one of the four bilinear corners' gradient into dir and len
void easu_set(inout float2 dir, inout float len, float w, float lA, float lB, float lC,
    float lD, float lE) {
    // (the reference's 1/x is an approximation that stays finite at zero; rcp() does not, so
    // the argument is held off zero: a flat area then has a zero gradient times a huge number)
    float lenX = max(abs(lD - lC), abs(lC - lB));
    lenX = rcp(max(lenX, 1e-30));
    float dirX = lD - lB;
    dir.x += dirX * w;
    lenX = saturate(abs(dirX) * lenX);
    lenX *= lenX;
    len += lenX * w;
    float lenY = max(abs(lE - lC), abs(lC - lA));
    lenY = rcp(max(lenY, 1e-30));
    float dirY = lE - lA;
    dir.y += dirY * w;
    lenY = saturate(abs(dirY) * lenY);
    lenY *= lenY;
    len += lenY * w;
}

float4 ps_easu(float2 uv : TEXCOORD0) : COLOR0 {
    // the output pixel's integer position, and where it falls in the small picture
    float2 ip = floor(uv * sizes.xy);
    float2 pp = ip * mapping.xy + mapping.zw;
    float2 fp = floor(pp);
    pp -= fp;

    // the twelve-tap kernel, by the reference's letters
    //      b c
    //    e f g h
    //    i j k l
    //      n o
    float3 b = texel(fp + float2(0.0, -1.0));
    float3 c = texel(fp + float2(1.0, -1.0));
    float3 e = texel(fp + float2(-1.0, 0.0));
    float3 f = texel(fp);
    float3 g = texel(fp + float2(1.0, 0.0));
    float3 h = texel(fp + float2(2.0, 0.0));
    float3 i = texel(fp + float2(-1.0, 1.0));
    float3 j = texel(fp + float2(0.0, 1.0));
    float3 k = texel(fp + float2(1.0, 1.0));
    float3 l = texel(fp + float2(2.0, 1.0));
    float3 n = texel(fp + float2(0.0, 2.0));
    float3 o = texel(fp + float2(1.0, 2.0));

    float bL = lum(b), cL = lum(c), eL = lum(e), fL = lum(f), gL = lum(g), hL = lum(h);
    float iL = lum(i), jL = lum(j), kL = lum(k), lL = lum(l), nL = lum(n), oL = lum(o);

    // direction and length of the edge, accumulated over the four bilinear corners
    float2 dir = float2(0.0, 0.0);
    float len = 0.0;
    easu_set(dir, len, (1.0 - pp.x) * (1.0 - pp.y), bL, eL, fL, gL, jL);
    easu_set(dir, len, pp.x * (1.0 - pp.y), cL, fL, gL, hL, kL);
    easu_set(dir, len, (1.0 - pp.x) * pp.y, fL, iL, jL, kL, nL);
    easu_set(dir, len, pp.x * pp.y, gL, jL, kL, lL, oL);

    // normalise, and clean up close to zero
    float2 dir2 = dir * dir;
    float dirR = dir2.x + dir2.y;
    bool zro = dirR < (1.0 / 32768.0);
    dirR = rsqrt(dirR);
    dirR = zro ? 1.0 : dirR;
    dir.x = zro ? 1.0 : dir.x;
    dir *= dirR;
    // from {0 to 2} to {0 to 1}, and shaped with a square
    len = len * 0.5;
    len *= len;
    // stretch the kernel: 1.0 along the axes, to sqrt(2.0) on the diagonal
    float stretch = (dir.x * dir.x + dir.y * dir.y) * rcp(max(abs(dir.x), abs(dir.y)));
    float2 len2 = float2(1.0 + (stretch - 1.0) * len, 1.0 - 0.5 * len);
    // the window shifts with the amount of edge
    float lob = 0.5 + ((1.0 / 4.0 - 0.04) - 0.5) * len;
    float clp = rcp(lob);

    // the min and max of the four nearest texels, to dering with
    float3 min4 = min(min(min(f, g), j), k);
    float3 max4 = max(max(max(f, g), j), k);

    float3 aC = float3(0.0, 0.0, 0.0);
    float aW = 0.0;
    easu_tap(aC, aW, float2(0.0, -1.0) - pp, dir, len2, lob, clp, b);
    easu_tap(aC, aW, float2(1.0, -1.0) - pp, dir, len2, lob, clp, c);
    easu_tap(aC, aW, float2(-1.0, 1.0) - pp, dir, len2, lob, clp, i);
    easu_tap(aC, aW, float2(0.0, 1.0) - pp, dir, len2, lob, clp, j);
    easu_tap(aC, aW, float2(0.0, 0.0) - pp, dir, len2, lob, clp, f);
    easu_tap(aC, aW, float2(-1.0, 0.0) - pp, dir, len2, lob, clp, e);
    easu_tap(aC, aW, float2(1.0, 1.0) - pp, dir, len2, lob, clp, k);
    easu_tap(aC, aW, float2(2.0, 1.0) - pp, dir, len2, lob, clp, l);
    easu_tap(aC, aW, float2(2.0, 0.0) - pp, dir, len2, lob, clp, h);
    easu_tap(aC, aW, float2(1.0, 0.0) - pp, dir, len2, lob, clp, g);
    easu_tap(aC, aW, float2(1.0, 2.0) - pp, dir, len2, lob, clp, o);
    easu_tap(aC, aW, float2(0.0, 2.0) - pp, dir, len2, lob, clp, n);

    // normalise and dering
    return float4(min(max4, max(min4, aC * rcp(aW))), 1.0);
}

// the reference's RCAS limit: the point at which sharpening turns unnatural
static const float RCAS_LIMIT = 0.25 - (1.0 / 16.0);

float4 ps_rcas(float2 uv : TEXCOORD0) : COLOR0 {
    // a 3x3 neighbourhood's cross
    //      b
    //    d e f
    //      h
    float3 b = tex2Dlod(src, float4(uv + float2(0.0, -sizes.w), 0.0, 0.0)).rgb;
    float3 d = tex2Dlod(src, float4(uv + float2(-sizes.z, 0.0), 0.0, 0.0)).rgb;
    float3 e = tex2Dlod(src, float4(uv, 0.0, 0.0)).rgb;
    float3 f = tex2Dlod(src, float4(uv + float2(sizes.z, 0.0), 0.0, 0.0)).rgb;
    float3 h = tex2Dlod(src, float4(uv + float2(0.0, sizes.w), 0.0, 0.0)).rgb;

    // the ring's min and max
    float3 mn4 = min(min(min(b, d), f), h);
    float3 mx4 = max(max(max(b, d), f), h);
    // the limiters (a hair is added to the divisors, which are zero on pure black and pure
    // white, where the reference's 0 times 1/0 would be NaN)
    float3 hitMin = min(mn4, e) * rcp(4.0 * mx4 + 1e-6);
    float3 hitMax = (1.0 - max(mx4, e)) * rcp(4.0 * mn4 - 4.0 - 1e-6);
    float3 lobe3 = max(-hitMin, hitMax);
    float lobe = max(-RCAS_LIMIT, min(max(max(lobe3.r, lobe3.g), lobe3.b), 0.0)) * rcas.x;
    // resolve
    float rcpL = rcp(4.0 * lobe + 1.0);
    return float4((lobe * b + lobe * d + lobe * h + lobe * f + e) * rcpL, 1.0);
}

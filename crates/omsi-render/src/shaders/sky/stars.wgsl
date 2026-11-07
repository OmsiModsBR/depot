// The night sky's stars (`helper\stars.dat`), read from the star map (omsi_content::StarMap):
// the sphere of the stars' own frame cut into the six faces of a cube, each face into cells
// of up to two stars (two layers of cells, one under the other), a texel a star: its place
// in the cell (rg), its size (b: 0 none, else 1 + 254 x size / 8; the size is 4 x the square
// root of its brightness) and its class (a). The sky turns with the hours: the camera's
// `stars` rows take a world direction into that frame. A pixel reads the cell it looks into
// and, only where a star next door could reach it, the neighbouring cells on its nearer
// sides; the second layer only under a full first one - most cells hold no star, so most
// pixels read one texel.

@group(1) @binding(11) var t_stars: texture_2d<f32>;

// The colour of a class number (omsi_content::StarClass::rgb, by its code).
fn star_class_rgb(code: i32) -> vec3<f32> {
    switch code {
        case 1: { return vec3<f32>(209.0, 198.0, 255.0) / 255.0; }
        case 2: { return vec3<f32>(215.0, 205.0, 255.0) / 255.0; }
        case 3: { return vec3<f32>(224.0, 217.0, 255.0) / 255.0; }
        case 4: { return vec3<f32>(240.0, 244.0, 255.0) / 255.0; }
        case 5: { return vec3<f32>(254.0, 255.0, 225.0) / 255.0; }
        case 6: { return vec3<f32>(255.0, 252.0, 185.0) / 255.0; }
        case 7: { return vec3<f32>(255.0, 228.0, 147.0) / 255.0; }
        default: { return vec3<f32>(255.0, 198.0, 147.0) / 255.0; }
    }
}

// The face of the cube direction e falls on (0..5), as omsi_content::stars::face_of finds it.
fn star_face(e: vec3<f32>) -> i32 {
    let a = abs(e);
    if (a.x >= a.y && a.x >= a.z) {
        return select(1, 0, e.x > 0.0);
    }
    if (a.y >= a.z) {
        return select(3, 2, e.y > 0.0);
    }
    return select(5, 4, e.z > 0.0);
}

// The place of direction v on face `face` (-1..1 each on the face itself).
fn star_face_uv(face: i32, v: vec3<f32>) -> vec2<f32> {
    if (face < 2) {
        return v.yz / abs(v.x);
    }
    if (face < 4) {
        return v.xz / abs(v.y);
    }
    return v.xy / abs(v.z);
}

// The direction of a place on a face (the way back from `star_face_uv`).
fn star_face_dir(face: i32, uv: vec2<f32>) -> vec3<f32> {
    let s = select(-1.0, 1.0, face % 2 == 0);
    if (face < 2) {
        return normalize(vec3<f32>(s, uv.x, uv.y));
    }
    if (face < 4) {
        return normalize(vec3<f32>(uv.x, s, uv.y));
    }
    return normalize(vec3<f32>(uv.x, uv.y, s));
}

// Whether the game has given the sky a star map to draw the stars from.
fn stars_on() -> bool {
    return camera.stars[1].w > 0.5;
}

// The world direction d in the stars' frame.
fn star_frame(d: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(camera.stars[0].xyz, d), dot(camera.stars[1].xyz, d), dot(camera.stars[2].xyz, d));
}

// The stars at the pixel looking along d (world, unit), each weighed by `star_weight`: dx and
// dy how d changes a pixel across and a pixel down (dpdx, dpdy: taken by the caller, where
// every pixel of the quad still runs), `reach` how far from the pixel's middle a star still
// counts (pixels), `mode` and `limit` what `star_weight` makes of each.
fn stars_light(d: vec3<f32>, dx: vec3<f32>, dy: vec3<f32>, reach: f32, mode: i32, limit: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    let n = camera.stars[1].w;
    let ni = i32(n);
    let e = star_frame(d);
    let ex = star_frame(dx);
    let ey = star_frame(dy);
    let face = star_face(e);
    let uv = star_face_uv(face, e);
    let g = (uv * 0.5 + 0.5) * n;
    let own = floor(g);
    let at = g - own;
    // how many cells `reach` pixels span on each side, along the face's two axes
    let gx = (star_face_uv(face, e + ex) - uv) * (0.5 * n);
    let gy = (star_face_uv(face, e + ey) - uv) * (0.5 * n);
    let span = (abs(gx) + abs(gy)) * reach;
    // (the steps a pixel makes, to tell where a star stands from the pixel in pixels)
    let xx = dot(ex, ex);
    let xy = dot(ex, ey);
    let yy = dot(ey, ey);
    let det = xx * yy - xy * xy;
    if (det <= 0.0) {
        return col;
    }
    // (a star reaches a pixel from a cell next door only when the pixel lies within `span`
    // of that side; `span` is well under half a cell, so one side an axis at most)
    let side = select(vec2<f32>(1.0), vec2<f32>(-1.0), at < vec2<f32>(0.5));
    let near = min(at, vec2<f32>(1.0) - at) < span;
    for (var k = 0; k < 4; k = k + 1) {
        let step = vec2<f32>(f32(k & 1), f32(k >> 1));
        if ((step.x > 0.0 && !near.x) || (step.y > 0.0 && !near.y)) {
            continue;
        }
        let cell = own + step * side;
        if (any(cell < vec2<f32>(0.0)) || any(cell >= vec2<f32>(n))) {
            continue;
        }
        for (var layer = 0; layer < 2; layer = layer + 1) {
            let t = textureLoad(t_stars, vec2<i32>(face * ni + i32(cell.x), layer * ni + i32(cell.y)), 0);
            if (t.b < 0.5 / 255.0) {
                // (the second layer holds a star only under a full first one)
                break;
            }
            let place = (cell + (floor(t.rg * 255.0 + 0.5) + 0.5) / 256.0) / n * 2.0 - 1.0;
            let sd = star_face_dir(face, place);
            let c = dot(sd, e);
            if (c < 0.999) {
                continue;
            }
            // where the star stands from the pixel's middle, in pixels: its step off the view
            // ray (on the plane across it) told in the steps a pixel makes
            let delta = sd / c - e;
            let p = dot(ex, delta);
            let q = dot(ey, delta);
            let off = vec2<f32>(yy * p - xy * q, xx * q - xy * p) / det;
            let size = (floor(t.b * 255.0 + 0.5) - 1.0) / 254.0 * 8.0;
            col = col + star_class_rgb(i32(floor(t.a * 255.0 + 0.5))) * star_weight(off, size, mode, limit);
        }
    }
    return col;
}

// How much of a star of `size` standing `off` pixels from the pixel's middle the pixel takes.
// Mode 0 (the classic skies): a square of points of the size (at least 1 pixel, at most 2),
// as bright as the size up to 1. Mode 1 (Enhanced): a soft point about a pixel across, its
// brightness (0..1) by its rank among the stars, shown only above `limit` (the faintest
// star the sky behind lets through).
fn star_weight(off: vec2<f32>, size: f32, mode: i32, limit: f32) -> f32 {
    if (mode == 0) {
        let half = clamp(size, 1.0, 2.0) * 0.5;
        return select(0.0, min(size, 1.0), abs(off.x) < half && abs(off.y) < half);
    }
    // (the share of the catalogue brighter than this star: the stars about three times as
    // many a magnitude fainter, all of them by magnitude 6 - size 4 is magnitude 0; the
    // brightness follows the share as the sky's limit expects it, the top tenth over half)
    let brighter = min(0.001 * pow(max(size, 1e-3) / 4.0, -2.5), 1.0);
    let b = pow(1.0 - brighter, 4.0);
    return (0.04 + 1.4 * b) * exp(-dot(off, off) / 0.72) * smoothstep(limit, limit + 0.15, b);
}

// The classic skies' stars at this pixel (colour as stored, 0..1 a channel), all as bright
// as the night lets them (`stars[0].w`); none below the horizon.
fn classic_stars(d: vec3<f32>, dx: vec3<f32>, dy: vec3<f32>) -> vec3<f32> {
    let bright = camera.stars[0].w;
    if (!stars_on() || bright <= 0.0 || d.z <= 0.0) {
        return vec3<f32>(0.0);
    }
    return min(stars_light(d, dx, dy, 1.0, 0, 0.0), vec3<f32>(1.0)) * bright;
}

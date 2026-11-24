// The sky as the sky cube holds it (drawn a face a frame by `fs_sky_cube`, divided by the
// table scale): rgb, and how much cloud covers the direction.
@group(0) @binding(17) var t_sky_cube: texture_cube<f32>;

@fragment
fn fs_enhanced(in: VsOut) -> @location(0) vec4<f32> {
    return enhanced_sky(in);
}

fn enhanced_sky(in: VsOut) -> vec4<f32> {
    let d = normalize(in.dir);
    let pre = enh.exposure.x;
    // the cube is drawn from its own eye (lib.rs Probe::cube_eye): look the clouds' base up
    // from there, so the sky does not slide with a camera that moved since
    var ld = d;
    let tb = cloud_shell(d, CLOUD_BOTTOM);
    if (tb > 0.0 && camera.cam_pos.z < CLOUD_BOTTOM) {
        ld = normalize(d * min(tb, CLOUD_MAX_DIST) - enh.eye.xyz);
    }
    let cube = textureSampleLevel(t_sky_cube, s_lin, vec3<f32>(ld.x, ld.z, ld.y), 0.0);
    var col = cube.rgb * enh.ground.w;
    var sun_px = vec3<f32>(0.0);
    // the sun: a limb-darkened disc as bright as its irradiance spread over its size,
    // behind whatever cloud there is
    let sd = normalize(camera.sun_dir.xyz);
    let r = enh.sun.w;
    let cosang = dot(d, sd);
    if (cosang > cos(r * 1.5) && d.z > -0.02) {
        let x = clamp(acos(clamp(cosang, -1.0, 1.0)) / r, 0.0, 1.5);
        let disc = 1.0 - smoothstep(0.85, 1.1, x);
        let limb = 1.0 - 0.6 * (1.0 - sqrt(max(1.0 - min(x, 1.0) * min(x, 1.0), 0.0)));
        let cover = cube.a;
        let h0 = camera.cam_pos.z - enh.fog.z;
        let t = air_of(d, 30000.0, h0, h0 + 30000.0 * max(d.z, 0.0), 0.0).a;
        let l = enh.sun_disc.rgb / (PI * r * r) * disc * limb * (1.0 - cover) * enh.lights.w * t;
        sun_px = l;
    }
    col = col + night_sky(d, cube.a, dpdx(d), dpdy(d));
    // the dome is drawn pre-exposed; the disc only a little over white: the eye's glare round
    // it is drawn from its light (fog_lamps.wgsl `sun_glare`), and from the disc's pixels
    // as well the glow would have counted it twice
    return vec4<f32>(min(col * pre, vec3<f32>(4000.0)) + min(sun_px * pre, vec3<f32>(24.0)), 1.0);
}

// The mirrors of an Enhanced picture are drawn with the plain shading (lib.rs, the cost of
// the enhanced shader in a small picture), but their sky is this one, tone-mapped here
// with the window's curve (post.wgsl `natural_tone`): the plain envir.cfg sky showed a blue
// dusk in the mirrors over the window's grey one, or the other way round (#1754, #1449).
// The meter's correction is left out (by day a few tenths of a stop at most).
fn mirror_tone(color: vec3<f32>, contrast: f32) -> vec3<f32> {
    let knee = 0.66;
    let x = max(color, vec3<f32>(0.0));
    let y = 0.18 * pow(x / 0.18 + vec3<f32>(1e-7), vec3<f32>(contrast));
    let peak = max(y.r, max(y.g, y.b));
    if (peak <= knee) {
        return y;
    }
    let d = 1.0 - knee;
    let shoulder = vec3<f32>(knee) + d * (vec3<f32>(1.0) - exp(-(y - vec3<f32>(knee)) / d));
    let soft = select(y, shoulder, y > vec3<f32>(knee));
    let np = knee + d * (1.0 - exp(-(peak - knee) / d));
    let o = mix(y * (np / peak), soft, 0.4);
    let g = 1.0 - 1.0 / (0.3 * (peak - np) + 1.0);
    return mix(o, vec3<f32>(np), g);
}

@fragment
fn fs_enhanced_mirror(in: VsOut) -> @location(0) vec4<f32> {
    let hdr = enhanced_sky(in).rgb;
    return vec4<f32>(clamp(mirror_tone(hdr, max(enh.debug.w, 1.0)), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// The moon and the stars towards d (radiance), behind what cloud covers it. The moon is a
// lit sphere: its lit side faces the sun, its phase is what the sun's direction makes of
// it, and its grey seas darken parts of the disc. The stars are the star map's (stars.wgsl),
// turning with the sky, each a point about a pixel across in its class's colour; they show
// only where the sky behind them is dark - a lit city's glow and the haze leave the
// brightest few. dx and dy: how d changes a pixel across and a pixel down.
fn night_sky(d: vec3<f32>, cover: f32, dx: vec3<f32>, dy: vec3<f32>) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    if (d.z < -0.01) {
        return col;
    }
    let h0 = camera.cam_pos.z - enh.fog.z;
    let through = air_of(d, 30000.0, h0, h0 + 30000.0 * max(d.z, 0.0), 0.0).a * (1.0 - cover);
    if (through < 0.01) {
        return col;
    }
    let m = enh.moon.xyz;
    let r = enh.moon.w;
    let cm = dot(d, m);
    if (cm > cos(r * 1.3) && max(enh.moon_disc.r, enh.moon_disc.g) > 0.0) {
        // the point on the moon's sphere seen in direction d
        let right = normalize(cross(m, vec3<f32>(0.0, 0.0, 1.0)) + vec3<f32>(1e-5, 0.0, 0.0));
        let up = cross(right, m);
        let q = vec2<f32>(dot(d, right), dot(d, up)) / r;
        let q2 = dot(q, q);
        let rim = 1.0 - smoothstep(0.92, 1.05, sqrt(q2));
        let nz = sqrt(max(1.0 - q2, 0.0));
        let n = normalize(right * q.x + up * q.y - m * nz);
        let sun_side = smoothstep(-0.04, 0.08, dot(n, normalize(camera.sun_dir.xyz)));
        // the seas: a few broad dark patches fixed on the face
        let sea = smoothstep(0.45, 0.7, cloud_fbm(q * 1.7 + vec2<f32>(3.1, 7.7)));
        let albedo = 1.0 - 0.38 * sea;
        // (as bright as the lit part's share of the irradiance spread over the disc; the
        // dark part keeps a trace of earthshine)
        let lit_share = max(0.5 + 0.5 * dot(-m, normalize(camera.sun_dir.xyz)), 0.03);
        let l = enh.moon_disc.rgb / (PI * r * r * lit_share) * (sun_side + 0.004) * albedo * rim;
        col = col + l * through;
    }
    // the stars
    let vis = enh.moon_disc.w;
    if (vis > 0.0 && d.z > 0.0 && stars_on()) {
        // the faintest that shows against the sky behind it (in the picture's own units): a
        // dark country sky shows thousands, a city's glow the few dozen brightest, a
        // twilight none
        let sky_l = dot(sky_table(d), vec3<f32>(0.2126, 0.7152, 0.0722)) * enh.exposure.x;
        let limit = clamp(sky_l * 90.0 + (1.0 - vis), 0.0, 1.2);
        let dark = 1.0 - smoothstep(0.03, 0.1, sky_l);
        if (dark > 0.0) {
            let stars = stars_light(d, dx, dy, 1.5, 1, limit);
            col = col + stars * dark * through / max(enh.exposure.x, 1e-6) * smoothstep(0.0, 0.15, d.z);
        }
    }
    return col;
}


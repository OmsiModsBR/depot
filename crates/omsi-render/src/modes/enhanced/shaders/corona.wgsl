// Enhanced graphics: the corona in the pre-exposed high-range picture - the glare of a
// lamp is drawn relative to what the eye is adapted to, brighter than white so that the
// glow filter picks it up; precipitation streaks keep the vanilla level.
@fragment
fn fs_enhanced(in: CoronaOut) -> @location(0) vec4<f32> {
    if (in.wpos.w > 0.5) {
        let to_eye = normalize(camera.cam_pos.xyz - in.wpos.xyz);
        let l = precip_light(in.wpos.xyz, to_eye, in.wpos.w > 1.5);
        return vec4<f32>(in.color.rgb * corona_shape(in) * in.color.a * l * enh.exposure.x, 1.0);
    }
    if (in.kind < -2.5) {
        return vec4<f32>(in.color.rgb * sun_glare_shape(in.uv) * in.color.a * enh.exposure.w, 1.0);
    }
    // (a fog cone is lit fog, not glare: it keeps the scene's level too)
    let scale = select(enh.exposure.w, enh.exposure.y, in.kind > 0.5);
    var shape = corona_shape(in);
    // (taken here, in uniform control flow: how much of the sprite one pixel spans)
    let r = length(in.uv - vec2<f32>(0.5)) * 2.0;
    let px = fwidth(r);
    if (in.kind < 0.5 && in.beam < 0.5) {
        // A lamp seen through clear air is a small, very bright core with a faint ring of
        // glare round it, not a lit disc as wide as its sprite (OMSI's glow bitmaps are
        // flat discs with soft rims): a narrow core bright enough for the glow pass to
        // spread it the way a lens does, a short falloff of glare, and a dim trace of the
        // bitmap (which keeps a star's rays and a streak's shape). Mist, fog and rain
        // scatter the light round the lamp, and there the full halo comes back.
        let wet = clamp(enh.fog.x * 400.0 + enh.weather.z * 0.7, 0.0, 1.0);
        // (the core never narrower than a pixel and a half: a far lamp stays a point of
        // light, as the sprite's distance floor keeps it on the screen)
        let lit = textureSampleLevel(t_corona, s_corona, vec2<f32>(0.5, 0.5), 0.0).rgb;
        // ... and never wider than half the sprite, and inside its disc: on a far lamp's
        // sprite of a few pixels the pixel-wide core had grown past the sprite's edge, and
        // lit by the bitmap's centre colour it filled the whole square to its corners
        let w = clamp(px * 1.5, 0.1, 0.5);
        let disc = clamp((1.0 - r) / max(px, 1e-3), 0.0, 1.0);
        let core = (exp(-r * r / (w * w)) * 3.0 + exp(-r * 9.0) * 0.35) * disc;
        // The bitmap itself is cut to the disc as well. On a sprite a few pixels wide it is
        // read from one of its last mip levels, where its round glow is averaged over the
        // whole square: a flat value up to the corners, which the lamp's brightness took
        // past white - every far lamp a solid little square.
        shape = shape * disc;
        shape = mix(core * max(lit, shape) + shape * 0.22, shape, wet);
    }
    // (the light's colour in the picture's linear terms: see `light_colour`)
    return vec4<f32>(light_colour(in.color.rgb) * shape * in.color.a * scale, 1.0);
}

@fragment
fn fs_smoke_enhanced(in: CoronaOut) -> @location(0) vec4<f32> {
    let c = smoke_color(in);
    return vec4<f32>(c.rgb * enh.exposure.y, c.a);
}

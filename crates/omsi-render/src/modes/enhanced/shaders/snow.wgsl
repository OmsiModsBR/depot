// the enhanced picture: what the flake scatters of the sky's, the sun's and the lamps'
// light, at the albedo of snow
@fragment
fn fs_snow_enhanced(in: SnowOut) -> @location(0) vec4<f32> {
    let a = flake_alpha(in);
    let to_eye = normalize(camera.cam_pos.xyz - in.pos);
    // (a flake hangs under the sky: its light from above as much as from all round)
    let l = (precip_light(in.pos, to_eye, true) + sh_irradiance(vec3<f32>(0.0, 0.0, 1.0)) / PI) * 0.5 * 0.85 * enh.exposure.x;
    return vec4<f32>(min(l, vec3<f32>(60.0)) * a, a);
}

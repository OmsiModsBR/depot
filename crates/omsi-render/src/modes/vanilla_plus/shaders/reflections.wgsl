// Vanilla+: the plain shading drawn into the high-range pass for the puddles' reflections - the
// picture, and the screen mask with the weight of the wet puddles (`EnhancedOut`, the pass's two
// targets, in common/puddle_common.wgsl).
@fragment
fn fs_vanilla_reflections(in: FsIn) -> EnhancedOut {
    var weight = 0.0;
    let c = shade_vanilla(in, &weight, camera.cam_pos.xyz);
    let coverage = select(c.a, 0.0, in.params2.w > 1.5);
    var out: EnhancedOut;
    out.color = c;
    out.mask = vec4<f32>(0.0, weight * 0.49, weight, coverage);
    return out;
}

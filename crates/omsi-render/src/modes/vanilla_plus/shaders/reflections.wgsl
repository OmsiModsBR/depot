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

// The wet roads' reflected view of the local vehicle, shaded plainly: one reflected camera for
// the whole vehicle, its transparent panes included.
@fragment
fn fs_puddle_vehicle(input: FsIn) -> @location(0) vec4<f32> {
    let plane = vehicle_reflection.plane;
    let height = dot(plane.xyz, input.world) - plane.w;
    if (height < 0.0 || material.emissive.w > 1.5) { discard; }
    let eye = camera.cam_pos.xyz - 2.0 * plane.xyz * (dot(plane.xyz, camera.cam_pos.xyz) - plane.w);
    var unused = 0.0;
    return shade_vanilla(input, &unused, eye);
}

// What shows through the closed chassis in that view.
@fragment
fn fs_puddle_chassis() -> @location(0) vec4<f32> {
    return vec4<f32>(camera.ambient.rgb * 0.025, 1.0);
}

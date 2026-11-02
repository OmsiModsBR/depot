//! #324: DirectX 12's shader compiler overflowed its stack on the sky and clouds shader.
//! The cloud march's loops (56 steps, 6 light steps, 3 octaves) must not have trip counts
//! it can see through: it unrolls those into every entry point. Only the sky table's
//! four-step loop may keep a literal bound.

/// The sky shader as the HLSL wgpu's Direct3D 12 backend hands the shader compiler
/// (shader model 5.1 is the FXC path; 6.5 the DXC one).
fn sky_hlsl(model: naga::back::hlsl::ShaderModel) -> String {
    let module = naga::front::wgsl::parse_str(&omsi_render::testing::sky_shader_source()).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap();
    let opts = naga::back::hlsl::Options {
        shader_model: model,
        ..Default::default()
    };
    let mut out = String::new();
    let popts = Default::default();
    naga::back::hlsl::Writer::new(&mut out, &opts, &popts)
        .write(&module, &info, None)
        .unwrap();
    out
}

#[test]
fn cloud_march_loops_have_no_constant_trip_count() {
    for model in [
        naga::back::hlsl::ShaderModel::V5_1,
        naga::back::hlsl::ShaderModel::V6_5,
    ] {
        let hlsl = sky_hlsl(model);
        let mut runtime = 0;
        for line in hlsl.lines().map(str::trim) {
            // a loop test: `if ((_eN < int(K))) {` (a literal bound) or `if ((_eN < _eM)) {`
            let Some(rest) = line.strip_prefix("if ((_e") else {
                continue;
            };
            let Some((_, rhs)) = rest.split_once(" < ") else {
                continue;
            };
            if let Some(k) = rhs
                .strip_prefix("int(")
                .and_then(|r| r.strip_suffix("))) {"))
            {
                let k: i32 = k.parse().unwrap();
                assert!(k <= 4, "literal loop bound {k} in the sky shader: {line}");
            } else if rhs.starts_with("_e") && rhs.ends_with(")) {") {
                runtime += 1;
            }
        }
        assert!(
            runtime >= 3,
            "the cloud march's loop tests were not found ({runtime})"
        );
    }
}

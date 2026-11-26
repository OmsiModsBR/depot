use omsi_render_vanilla::testing::d3d9::shader::*;

#[test]
fn basic_shaders_compile_for_sm3() {
    let vs = compile(BASIC, "basic.hlsl", "vs_main", Stage::Vertex).unwrap();
    let ps = compile(BASIC, "basic.hlsl", "ps_main", Stage::Pixel).unwrap();
    // The first token of SM3 bytecode is the version: 0xFFFE0300 (vs), 0xFFFF0300 (ps).
    assert_eq!(vs[0], 0xFFFE_0300);
    assert_eq!(ps[0], 0xFFFF_0300);
}

#[test]
fn scene_shaders_compile_for_sm3() {
    let vs = compile(SCENE, "scene.hlsl", "vs_main", Stage::Vertex).unwrap();
    let ps = compile(SCENE, "scene.hlsl", "ps_main", Stage::Pixel).unwrap();
    assert_eq!(vs[0], 0xFFFE_0300);
    assert_eq!(ps[0], 0xFFFF_0300);
}

#[test]
fn the_scene_pixel_shader_fits_every_sm3_card() {
    // (512 slots is what every Shader Model 3.0 card gives a pixel shader; the shader
    // keeps a tenth of it free, for the compiler's count being approximate and for what
    // is added next)
    let ps = compile(SCENE, "scene.hlsl", "ps_main", Stage::Pixel).unwrap();
    let slots = reported_slots(&ps).unwrap();
    println!("scene pixel shader: {slots} instruction slots");
    assert!(slots <= 460, "the scene pixel shader takes {slots} slots");
    // the device's guard counts the bytecode itself (`caps::instruction_slots`): it
    // must not stray far from the compiler's count
    let counted = omsi_render_vanilla::caps::instruction_slots(&ps);
    assert!(
        counted.abs_diff(slots) <= slots / 10,
        "counted {counted}, the compiler {slots}"
    );
}

#[test]
fn corona_shaders_compile_for_sm3() {
    let vs = compile(CORONA, "corona.hlsl", "vs_main", Stage::Vertex).unwrap();
    let ps = compile(CORONA, "corona.hlsl", "ps_main", Stage::Pixel).unwrap();
    assert_eq!(vs[0], 0xFFFE_0300);
    assert_eq!(ps[0], 0xFFFF_0300);
    // (a separate pass: its pixel shader has its own 512 slots, with room to spare)
    let slots = reported_slots(&ps).unwrap();
    println!("corona pixel shader: {slots} instruction slots");
    assert!(slots <= 460, "the corona pixel shader takes {slots} slots");
}

#[test]
fn wet_road_pixel_shader_compiles_for_sm3_in_few_slots() {
    let ps = compile(WET_ROAD, "wet_road.hlsl", "ps_main", Stage::Pixel).unwrap();
    assert_eq!(ps[0], 0xFFFF_0300);
    // (a pass of its own: its pixel shader has its own 512 slots, with room to spare; the
    // scene's pixel shader keeps its slots)
    let slots = reported_slots(&ps).unwrap();
    println!("wet road pixel shader: {slots} instruction slots");
    assert!(
        slots <= 460,
        "the wet road pixel shader takes {slots} slots"
    );
}

#[test]
fn glass_rain_pixel_shader_compiles_for_sm3_in_few_slots() {
    let ps = compile(GLASS_RAIN, "glass_rain.hlsl", "ps_main", Stage::Pixel).unwrap();
    assert_eq!(ps[0], 0xFFFF_0300);
    // (a pass of its own: the scene's pixel shader keeps its slots)
    let slots = reported_slots(&ps).unwrap();
    println!("glass rain pixel shader: {slots} instruction slots");
    assert!(
        slots <= 128,
        "the glass rain pixel shader takes {slots} slots"
    );
}

#[test]
fn sky_shaders_compile_for_sm3() {
    let vs = compile(SKY, "sky.hlsl", "vs_main", Stage::Vertex).unwrap();
    let ps = compile(SKY, "sky.hlsl", "ps_main", Stage::Pixel).unwrap();
    assert_eq!(vs[0], 0xFFFE_0300);
    assert_eq!(ps[0], 0xFFFF_0300);
    // (a separate pass: its pixel shader has its own 512 slots, with room to spare)
    let slots = reported_slots(&ps).unwrap();
    println!("sky pixel shader: {slots} instruction slots");
    assert!(slots <= 460, "the sky pixel shader takes {slots} slots");
}

#[test]
fn smoke_shaders_compile_for_sm3() {
    let vs = compile(SMOKE, "smoke.hlsl", "vs_main", Stage::Vertex).unwrap();
    let ps = compile(SMOKE, "smoke.hlsl", "ps_main", Stage::Pixel).unwrap();
    assert_eq!(vs[0], 0xFFFE_0300);
    assert_eq!(ps[0], 0xFFFF_0300);
    // (a separate pass: its pixel shader has its own 512 slots, with room to spare)
    let slots = reported_slots(&ps).unwrap();
    println!("smoke pixel shader: {slots} instruction slots");
    assert!(slots <= 460, "the smoke pixel shader takes {slots} slots");
}

#[test]
fn upscale_shaders_compile_for_sm3_and_fit() {
    let vs = compile(UPSCALE, "upscale.hlsl", "vs_main", Stage::Vertex).unwrap();
    assert_eq!(vs[0], 0xFFFE_0300);
    for entry in ["ps_bilinear", "ps_easu", "ps_rcas"] {
        let ps = compile(UPSCALE, "upscale.hlsl", entry, Stage::Pixel).unwrap();
        assert_eq!(ps[0], 0xFFFF_0300);
        // (passes of their own: each pixel shader has its 512 slots, with room to spare)
        let slots = reported_slots(&ps).unwrap();
        println!("upscale {entry}: {slots} instruction slots");
        assert!(slots <= 460, "{entry} takes {slots} slots");
        let counted = omsi_render_vanilla::caps::instruction_slots(&ps);
        assert!(
            counted.abs_diff(slots) <= slots / 10 + 2,
            "{entry}: counted {counted}, the compiler {slots}"
        );
    }
}

#[test]
fn ui_shaders_compile_for_sm3() {
    let vs = compile(UI, "ui.hlsl", "vs_main", Stage::Vertex).unwrap();
    let ps = compile(UI, "ui.hlsl", "ps_main", Stage::Pixel).unwrap();
    assert_eq!(vs[0], 0xFFFE_0300);
    assert_eq!(ps[0], 0xFFFF_0300);
}

#[test]
fn compiler_errors_are_reported() {
    let err = compile(
        "float4 ps_main() : COLOR0 { return nope; }",
        "bad.hlsl",
        "ps_main",
        Stage::Pixel,
    )
    .unwrap_err();
    assert!(err.to_string().contains("nope"), "{err}");
}

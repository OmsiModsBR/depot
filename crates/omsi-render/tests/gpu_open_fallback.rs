//! Tests moved out of `src/gpu/open.rs`.

use omsi_render::testing::internals::gpu::open::*;

#[test]
fn a_remembered_fallback_is_read_back() {
    let e = parse_fallbacks("Adreno (TM) 830 (Gl)|1|1\nNVIDIA | odd (Vulkan)|4|0\nbroken\n");
    assert_eq!(
        e,
        vec![
            ("Adreno (TM) 830 (Gl)".to_string(), 1, true),
            ("NVIDIA | odd (Vulkan)".to_string(), 4, false)
        ]
    );
}

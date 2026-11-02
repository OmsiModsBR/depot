//! What the tests in `tests/` need from inside the crate (feature `test-support`).

/// The sky and clouds shader as one WGSL module (all its entry points).
pub fn sky_shader_source() -> String {
    crate::shaders::sky_shader_source()
}

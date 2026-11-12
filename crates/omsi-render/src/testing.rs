//! What the tests in `tests/` need from inside the crate (feature `test-support`).

/// The sky and clouds shader as one WGSL module (all its entry points).
pub fn sky_shader_source() -> String {
    crate::shaders::sky_shader_source()
}

/// A light's colour as the Enhanced passes take it (`lighting::linear_light_colour`).
pub fn linear_light_colour(c: [f32; 3]) -> [f32; 3] {
    crate::lighting::linear_light_colour(c)
}

//! What the tests in `tests/` need from inside the crate (feature `test-support`).

/// The sky and clouds shader as one WGSL module (all its entry points).
pub fn sky_shader_source() -> String {
    crate::shaders::sky_shader_source()
}

/// A light's colour as the Enhanced passes take it (`lighting::linear_light_colour`).
pub fn linear_light_colour(c: [f32; 3]) -> [f32; 3] {
    crate::lighting::linear_light_colour(c)
}

pub use crate::renderer::lists::{shadow_caster, ShadowCaster};

/// What a picture draws (`FrameFeatures::decide`), for tests without a graphics adapter.
pub use crate::interface::{ModeShading, ModeView, PipeSet};
pub use crate::passes::FrameFeatures;
pub use crate::renderer::{BuiltPasses, FrameInputs, FrameSwitches, ViewKind};

/// What the render mode of `path` answers for a frame (`interface::RenderMode`), for the tests
/// that need no graphics adapter.
pub mod modes {
    use super::{FrameFeatures, ModeShading, ModeView, PipeSet};
    use crate::interface::RenderMode;
    use crate::lighting::{PointLight, ShadingPath};

    fn mode(path: ShadingPath) -> &'static dyn RenderMode {
        crate::renderer::mode_for(path)
    }

    pub fn shading(path: ShadingPath, view: &ModeView) -> ModeShading {
        mode(path).shading(view)
    }

    pub fn pipe_set(path: ShadingPath, features: &FrameFeatures) -> PipeSet {
        mode(path).pipe_set(features)
    }

    pub fn draws_light(path: ShadingPath, l: &PointLight, features: &FrameFeatures) -> bool {
        mode(path).draws_light(l, features)
    }

    /// The radius a mode's light record gives the shader for `l` (0 for a lamp the plain
    /// shader reads as no light).
    pub fn light_radius(path: ShadingPath, l: &PointLight, features: &FrameFeatures) -> f32 {
        mode(path).light_record(l, glam::Vec3::ZERO, features).pos[3]
    }

    pub fn lamp_shadows(path: ShadingPath, features: &FrameFeatures) -> bool {
        mode(path).lamp_shadows(features)
    }
}

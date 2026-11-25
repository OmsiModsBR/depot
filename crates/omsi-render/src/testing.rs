//! What the tests in `tests/` need from inside the crate (feature `test-support`).

/// The sky and clouds shader as one WGSL module (all its entry points).
pub fn sky_shader_source() -> String {
    crate::shaders::sky_shader_source(crate::renderer::sky_own_text(
        crate::lighting::ShadingPath::Hdr,
    ))
}

/// A light's colour as the Enhanced passes take it (its light record's colour).
pub fn linear_light_colour(c: [f32; 3]) -> [f32; 3] {
    let lamp = crate::lighting::PointLight {
        color: c,
        intensity: 1.0,
        ..Default::default()
    };
    let g = crate::renderer::mode_for(crate::lighting::ShadingPath::Hdr).light_record(
        &lamp,
        glam::Vec3::ZERO,
        &FrameFeatures::decide(
            crate::lighting::ShadingPath::Hdr,
            &crate::renderer::RenderOptions::default(),
            &FrameInputs {
                view: ViewKind::Window,
                lead_view: true,
                full_size: (1280, 720),
                picture_size: (1280, 720),
                rain: 0.0,
                wetness: 0.0,
                snow: 0.0,
                has_glass_slot: false,
                has_puddle_material: false,
                built: BuiltPasses {
                    own_mode: true,
                    reflections: true,
                    puddles: true,
                    ssao: true,
                },
                switches: FrameSwitches::default(),
            },
        ),
    );
    [g.color[0], g.color[1], g.color[2]]
}

/// The sphere (centre, radius) a mesh is culled and sorted by.
pub fn mesh_bounds(data: &omsi_geometry::MeshData) -> (glam::Vec3, f32) {
    crate::assets::mesh_bounds(data)
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

    /// How far a picture sees through a fog of `density` (1/m) for a camera of the range `far`
    /// (m), and whether the mode's fog is its own (`FogCull`).
    pub fn fog_cull(
        path: ShadingPath,
        features: &FrameFeatures,
        density: f32,
        far: f32,
    ) -> (f32, bool) {
        let lighting = crate::lighting::Lighting {
            fog_density: density,
            ..Default::default()
        };
        let camera = crate::scene::Camera {
            position: glam::DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_deg: 60.0,
            near: 0.1,
            far,
        };
        let cull = mode(path).fog_cull(features, &lighting, &camera);
        (cull.far, cull.own)
    }
}

//! What a legacy device can do, reduced to the facts the renderer's feature profile needs.

/// Pixel/vertex shader generation of the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShaderModel {
    /// No programmable pipeline (fixed function only).
    None,
    Sm2,
    Sm3,
}

impl ShaderModel {
    /// Builds the model from the `major` of a D3D9 shader version (`D3DPS_VERSION`).
    pub fn from_major(major: u8) -> Self {
        match major {
            0 | 1 => Self::None,
            2 => Self::Sm2,
            _ => Self::Sm3,
        }
    }
}

/// Device facts. Plain data so it can be built and tested on any platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caps {
    pub adapter_name: String,
    pub shader_model: ShaderModel,
    pub max_texture_size: u32,
    /// Textures may have any size (not only powers of two).
    pub non_pow2_textures: bool,
    /// Indices can be 32 bit.
    pub index32: bool,
    /// Simultaneous render targets (1 means no MRT).
    pub max_render_targets: u32,
}

impl Caps {
    /// The minimum the planned "Vanilla only" profile needs: programmable shaders and
    /// textures of at least 2048. Declares nothing about *supported* hardware; that is a
    /// product decision.
    pub fn meets_profile_floor(&self) -> bool {
        self.shader_model >= ShaderModel::Sm2 && self.max_texture_size >= 2048
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(sm: ShaderModel, tex: u32) -> Caps {
        Caps {
            adapter_name: "test".into(),
            shader_model: sm,
            max_texture_size: tex,
            non_pow2_textures: false,
            index32: false,
            max_render_targets: 1,
        }
    }

    #[test]
    fn shader_model_from_major() {
        assert_eq!(ShaderModel::from_major(1), ShaderModel::None);
        assert_eq!(ShaderModel::from_major(2), ShaderModel::Sm2);
        assert_eq!(ShaderModel::from_major(3), ShaderModel::Sm3);
    }

    #[test]
    fn profile_floor() {
        assert!(caps(ShaderModel::Sm2, 2048).meets_profile_floor());
        assert!(!caps(ShaderModel::None, 4096).meets_profile_floor());
        assert!(!caps(ShaderModel::Sm3, 1024).meets_profile_floor());
    }
}

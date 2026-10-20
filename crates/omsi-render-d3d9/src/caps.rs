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
    /// The renderer's floor (maintainers' decision, 2026-10-19): Shader Model 3.0 in both
    /// stages and textures of at least 2048. Declares nothing about *supported* hardware;
    /// that is a separate product decision.
    pub fn meets_floor(&self) -> bool {
        self.shader_model >= ShaderModel::Sm3 && self.max_texture_size >= 2048
    }
}

/// The instruction slots every Shader Model 3.0 device gives a pixel shader at least
/// (`D3DCAPS9::MaxPixelShader30InstructionSlots`, which goes up to 32768).
pub const PS30_MIN_SLOTS: u32 = 512;

/// How many instruction slots compiled Shader Model 2 or 3 bytecode takes: declarations,
/// constant definitions and comments take none, the macro instructions as many as the
/// Shader Model 3.0 pixel shader tables give them (`slots_of`), every other one one.
pub fn instruction_slots(code: &[u32]) -> u32 {
    const END: u32 = 0x0000_FFFF;
    const COMMENT: u32 = 0xFFFE;
    // dcl, def, defi, defb, nop
    const FREE: [u32; 5] = [0x1F, 0x51, 0x30, 0x2F, 0x00];
    let mut slots = 0;
    // (the first token is the version)
    let mut i = 1;
    while let Some(&token) = code.get(i) {
        if token == END {
            break;
        }
        let opcode = token & 0xFFFF;
        let length = if opcode == COMMENT {
            (token >> 16) & 0x7FFF
        } else {
            if !FREE.contains(&opcode) {
                slots += slots_of(opcode);
            }
            (token >> 24) & 0x0F
        };
        i += 1 + length as usize;
    }
    slots
}

/// The slots of an instruction that takes more than one in a ps_3_0 shader.
fn slots_of(opcode: u32) -> u32 {
    match opcode {
        // sincos
        0x25 => 8,
        // m4x4, m3x4
        0x14 | 0x16 => 4,
        // pow, nrm, lit, m4x3, m3x3
        0x20 | 0x24 | 0x10 | 0x15 | 0x17 => 3,
        // lrp, crs, dsx, dsy, m3x2
        0x12 | 0x21 | 0x5B | 0x5C | 0x18 => 2,
        _ => 1,
    }
}

/// What a device says its pixel shaders may take, as far as it can be trusted: below the
/// floor every SM3 card gives (or none at all) is taken for a wrong cap, and the floor used.
pub fn ps30_slots(reported: u32) -> u32 {
    reported.max(PS30_MIN_SLOTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_slots_leave_out_declarations_and_comments() {
        let code = [
            0xFFFF_0300, // ps_3_0
            0x0002_FFFE, // a comment of two tokens
            0x1234_5678,
            0x9ABC_DEF0,
            0x0500_0051, // def c0, 4 values
            0xA00F_0000,
            0,
            0,
            0,
            0,
            0x0200_001F, // dcl
            0x8000_0000,
            0x900F_0000,
            0x0300_0042, // texld r0, v0, s0
            0x800F_0000,
            0x90E4_0000,
            0xA0E4_0800,
            0x0200_0001, // mov oC0, r0
            0x800F_0800,
            0x80E4_0000,
            0x0000_FFFF,
            0x0100_0001, // after the end: not read
        ];
        assert_eq!(instruction_slots(&code), 2);
        assert_eq!(instruction_slots(&[0xFFFF_0300]), 0);
        // (cut short: counted as far as it goes)
        assert_eq!(instruction_slots(&code[..15]), 1);
        // pow r0, r1.x, c0.x and sincos r0.xy, r1.x: 3 and 8 slots
        let macros = [
            0xFFFF_0300,
            0x0300_0020,
            0x800F_0000,
            0x8000_0001,
            0xA000_0000,
            0x0200_0025,
            0x8003_0000,
            0x8000_0001,
            0x0000_FFFF,
        ];
        assert_eq!(instruction_slots(&macros), 11);
    }

    #[test]
    fn a_reported_slot_count_below_the_floor_is_not_trusted() {
        assert_eq!(ps30_slots(0), 512);
        assert_eq!(ps30_slots(96), 512);
        assert_eq!(ps30_slots(4096), 4096);
    }

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
        assert!(caps(ShaderModel::Sm3, 2048).meets_floor());
        assert!(!caps(ShaderModel::Sm2, 4096).meets_floor());
        assert!(!caps(ShaderModel::Sm3, 1024).meets_floor());
    }
}

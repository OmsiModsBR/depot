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

/// About how many instruction slots compiled Shader Model 2 or 3 bytecode takes:
/// declarations, constant definitions and comments take none, the macro instructions, the
/// flow control and the explicit-level samples as many as a ps_3_0 shader gives them
/// (`slots_of`), every other one one. A test holds it within a tenth of the compiler's own
/// count.
pub fn instruction_slots(code: &[u32]) -> u32 {
    const END: u32 = 0x0000_FFFF;
    const COMMENT: u32 = 0xFFFE;
    // dcl, def, defi, defb, label
    const FREE: [u32; 5] = [0x1F, 0x51, 0x30, 0x2F, 0x1E];
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
        // lrp, crs, dsx, dsy, m3x2, texkill
        0x12 | 0x21 | 0x5B | 0x5C | 0x18 | 0x41 => 2,
        // flow control: loop, rep, if, if_comp, break_comp, breakp, callnz; texldd
        0x1B | 0x26 | 0x28 | 0x29 | 0x2D | 0x60 | 0x1A | 0x5D => 3,
        // endloop, endrep, call; texldl
        0x1D | 0x27 | 0x19 | 0x5F => 2,
        _ => 1,
    }
}

/// What a device says its pixel shaders may take, as far as it can be trusted: below the
/// floor every SM3 card gives (or none at all) is taken for a wrong cap, and the floor used.
pub fn ps30_slots(reported: u32) -> u32 {
    reported.max(PS30_MIN_SLOTS)
}

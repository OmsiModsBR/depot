use omsi_render_vanilla::testing::domain::caps::*;

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
    // texkill r0 (2 slots), nop (1), label l0 (none)
    let others = [
        0xFFFF_0300,
        0x0100_0041,
        0x800F_0000,
        0x0000_0000,
        0x0100_001E,
        0xA0E0_0000,
        0x0000_FFFF,
    ];
    assert_eq!(instruction_slots(&others), 3);
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

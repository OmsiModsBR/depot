use omsi_render_vanilla::backend::AlphaMode;
use omsi_render_vanilla::ShaderModel;
use omsi_render_vanilla::*;

fn backend() -> NullBackend {
    NullBackend::new(Caps {
        adapter_name: "null".into(),
        shader_model: ShaderModel::Sm3,
        max_texture_size: 2048,
        non_pow2_textures: true,
        index32: false,
        max_render_targets: 1,
    })
}

fn tri() -> MeshDesc {
    MeshDesc {
        vertices: vec![0.0; 24],
        indices: vec![0, 1, 2],
    }
}

#[test]
fn frame_roundtrip_returns_clear_color() {
    let mut b = backend();
    b.begin_frame(&FrameDesc {
        width: 2,
        height: 1,
        clear_rgba: [1.0, 0.0, 0.0, 1.0],
    })
    .unwrap();
    let px = b.end_frame().unwrap();
    assert_eq!(px, vec![255, 0, 0, 255, 255, 0, 0, 255]);
}

#[test]
fn draw_validates_handles_and_frame() {
    let mut b = backend();
    let m = b.create_mesh(&tri()).unwrap();
    let batch = DrawBatch {
        mesh: m,
        texture: None,
        alpha: AlphaMode::Opaque,
        mvp: [0.0; 16],
        tint: [1.0; 4],
    };
    assert!(b.draw(&batch).is_err(), "draw outside a frame");
    b.begin_frame(&FrameDesc {
        width: 1,
        height: 1,
        clear_rgba: [0.0; 4],
    })
    .unwrap();
    b.draw(&batch).unwrap();
    let bad = DrawBatch {
        mesh: MeshHandle { index: 9, ..m },
        ..batch
    };
    assert!(b.draw(&bad).is_err());
    assert_eq!(b.batches_drawn, 1);
}

#[test]
fn rejects_bad_resources() {
    let mut b = backend();
    assert!(b
        .create_texture(&TextureDesc {
            width: 2,
            height: 2,
            rgba: vec![0; 3]
        })
        .is_err());
    assert!(b
        .create_texture(&TextureDesc {
            width: 4096,
            height: 1,
            rgba: vec![0; 4096 * 4]
        })
        .is_err());
    assert!(b
        .create_mesh(&MeshDesc {
            vertices: vec![0.0; 24],
            indices: vec![0, 1, 7]
        })
        .is_err());
}

#[test]
fn device_loss_blocks_until_recovered() {
    let mut b = backend();
    b.lost = true;
    assert!(matches!(
        b.create_mesh(&tri()),
        Err(BackendError::DeviceLost)
    ));
    b.recover().unwrap();
    assert!(b.create_mesh(&tri()).is_ok());
}

#[test]
fn handles_from_before_recovery_are_rejected() {
    let mut b = backend();
    let old = b.create_mesh(&tri()).unwrap();
    b.recover().unwrap();
    let new = b.create_mesh(&tri()).unwrap();
    assert_eq!(old.index, new.index, "same slot, different generation");
    b.begin_frame(&FrameDesc {
        width: 1,
        height: 1,
        clear_rgba: [0.0; 4],
    })
    .unwrap();
    let batch = |mesh| DrawBatch {
        mesh,
        texture: None,
        alpha: AlphaMode::Opaque,
        mvp: [0.0; 16],
        tint: [1.0; 4],
    };
    assert!(b.draw(&batch(old)).is_err());
    b.draw(&batch(new)).unwrap();
}

#[test]
fn frame_size_is_checked() {
    let mut b = backend();
    assert!(b
        .begin_frame(&FrameDesc {
            width: 0,
            height: 1,
            clear_rgba: [0.0; 4]
        })
        .is_err());
    assert!(b
        .begin_frame(&FrameDesc {
            width: 4096,
            height: 1,
            clear_rgba: [0.0; 4]
        })
        .is_err());
}

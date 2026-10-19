//! Device tests. They need a Direct3D 9Ex driver with Shader Model 3.0; machines without one
//! (CI runners may have none) skip them, which proves nothing about the device path.

use super::*;

const W: u32 = 8;
const H: u32 = 4;
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

fn device() -> Option<D3d9Device> {
    match D3d9Device::open() {
        Ok(dev) => {
            // Printed so a test log shows which adapter (the default one) was exercised.
            eprintln!("D3D9Ex adapter: {:?}", dev.caps());
            Some(dev)
        }
        Err(e) => {
            eprintln!("no usable Direct3D 9Ex device here ({e}); skipping");
            None
        }
    }
}

/// A quad covering the whole target at depth `z`, in clip space (identity transform).
fn quad(z: f32) -> MeshDesc {
    let v = |x: f32, y: f32, u: f32, w: f32| [x, y, z, 0., 0., -1., u, w];
    MeshDesc {
        vertices: [
            v(-1., -1., 0., 1.),
            v(1., -1., 1., 1.),
            v(1., 1., 1., 0.),
            v(-1., 1., 0., 0.),
        ]
        .concat(),
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

fn solid(rgba: [u8; 4]) -> TextureDesc {
    TextureDesc {
        width: 2,
        height: 2,
        rgba: rgba.repeat(4),
    }
}

fn frame(clear: [f32; 4]) -> FrameDesc {
    FrameDesc {
        width: W,
        height: H,
        clear_rgba: clear,
    }
}

/// Draws one full-screen quad over black and returns the pixels.
fn draw_one(dev: &mut D3d9Device, tex: [u8; 4], alpha: AlphaMode, tint: [f32; 4]) -> Vec<u8> {
    let mesh = dev.create_mesh(&quad(0.5)).unwrap();
    let texture = dev.create_texture(&solid(tex)).unwrap();
    dev.begin_frame(&frame([0., 0., 0., 1.])).unwrap();
    dev.draw(&DrawBatch {
        mesh,
        texture: Some(texture),
        alpha,
        mvp: IDENTITY,
        tint,
    })
    .unwrap();
    dev.end_frame().unwrap()
}

fn all_near(px: &[u8], want: [u8; 4], tol: u8) -> bool {
    px.chunks_exact(4)
        .all(|p| p.iter().zip(want).all(|(&a, b)| a.abs_diff(b) <= tol))
}

#[test]
fn clear_and_read_back() {
    let Some(mut dev) = device() else { return };
    dev.status().expect("fresh device is usable");
    dev.begin_frame(&frame([1., 0., 0., 1.])).unwrap();
    let px = dev.end_frame().unwrap();
    assert_eq!(px.len(), (W * H * 4) as usize);
    assert!(all_near(&px, [255, 0, 0, 255], 0));
}

#[test]
fn opaque_quad_shows_its_texture_times_tint() {
    let Some(mut dev) = device() else { return };
    let px = draw_one(
        &mut dev,
        [0, 255, 0, 255],
        AlphaMode::Opaque,
        [1., 1., 1., 1.],
    );
    assert!(all_near(&px, [0, 255, 0, 255], 1), "{:?}", &px[..4]);
    let px = draw_one(
        &mut dev,
        [255, 255, 255, 255],
        AlphaMode::Opaque,
        [0., 0., 1., 1.],
    );
    assert!(all_near(&px, [0, 0, 255, 255], 1), "{:?}", &px[..4]);
}

#[test]
fn untextured_batch_uses_white() {
    let Some(mut dev) = device() else { return };
    let mesh = dev.create_mesh(&quad(0.5)).unwrap();
    dev.begin_frame(&frame([0., 0., 0., 1.])).unwrap();
    let batch = DrawBatch {
        mesh,
        texture: None,
        alpha: AlphaMode::Opaque,
        mvp: IDENTITY,
        tint: [1., 0., 1., 1.],
    };
    dev.draw(&batch).unwrap();
    assert!(all_near(&dev.end_frame().unwrap(), [255, 0, 255, 255], 1));
}

#[test]
fn alpha_test_keeps_only_pixels_at_or_above_the_reference() {
    let Some(mut dev) = device() else { return };
    let below = (ALPHA_TEST_REF - 1) as u8;
    let px = draw_one(&mut dev, [255, 0, 0, below], AlphaMode::Test, [1.; 4]);
    assert!(
        all_near(&px, [0, 0, 0, 255], 0),
        "below the reference is discarded: {:?}",
        &px[..4]
    );
    let px = draw_one(
        &mut dev,
        [255, 0, 0, ALPHA_TEST_REF as u8],
        AlphaMode::Test,
        [1.; 4],
    );
    assert!(
        all_near(&px, [255, 0, 0, ALPHA_TEST_REF as u8], 1),
        "at the reference is kept: {:?}",
        &px[..4]
    );
}

#[test]
fn blend_mixes_with_what_is_behind() {
    let Some(mut dev) = device() else { return };
    let px = draw_one(&mut dev, [255, 0, 0, 128], AlphaMode::Blend, [1.; 4]);
    // 255 * 128/255 over black.
    assert!(
        px.chunks_exact(4)
            .all(|p| p[0].abs_diff(128) <= 2 && p[1] == 0 && p[2] == 0),
        "{:?}",
        &px[..4]
    );
}

#[test]
fn nearer_quad_wins_the_depth_test() {
    let Some(mut dev) = device() else { return };
    let near = dev.create_mesh(&quad(0.2)).unwrap();
    let far = dev.create_mesh(&quad(0.8)).unwrap();
    let red = dev.create_texture(&solid([255, 0, 0, 255])).unwrap();
    let blue = dev.create_texture(&solid([0, 0, 255, 255])).unwrap();
    dev.begin_frame(&frame([0., 0., 0., 1.])).unwrap();
    let batch = |mesh, texture| DrawBatch {
        mesh,
        texture: Some(texture),
        alpha: AlphaMode::Opaque,
        mvp: IDENTITY,
        tint: [1.; 4],
    };
    dev.draw(&batch(near, red)).unwrap();
    dev.draw(&batch(far, blue)).unwrap();
    assert!(all_near(&dev.end_frame().unwrap(), [255, 0, 0, 255], 1));
}

#[test]
fn bad_handles_and_frames_are_rejected() {
    let Some(mut dev) = device() else { return };
    let mesh = dev.create_mesh(&quad(0.5)).unwrap();
    let batch = DrawBatch {
        mesh,
        texture: None,
        alpha: AlphaMode::Opaque,
        mvp: IDENTITY,
        tint: [1.; 4],
    };
    assert!(dev.draw(&batch).is_err(), "draw outside a frame");
    dev.begin_frame(&frame([0.; 4])).unwrap();
    assert!(
        dev.begin_frame(&frame([0.; 4])).is_err(),
        "frame already begun"
    );
    assert!(dev
        .draw(&DrawBatch {
            mesh: MeshHandle(99),
            ..batch.clone()
        })
        .is_err());
    assert!(dev
        .draw(&DrawBatch {
            texture: Some(TextureHandle(99)),
            ..batch
        })
        .is_err());
    dev.end_frame().unwrap();
}

#[test]
fn transform_is_column_major() {
    let Some(mut dev) = device() else { return };
    let mesh = dev.create_mesh(&quad(0.5)).unwrap();
    // Half width, moved right by half the screen: x' = 0.5 x + 0.5 covers the right half.
    // Transposed, the translation would land in w and the picture would differ.
    let mvp = [
        0.5, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0.5, 0., 0., 1.,
    ];
    dev.begin_frame(&frame([0., 0., 0., 1.])).unwrap();
    dev.draw(&DrawBatch {
        mesh,
        texture: None,
        alpha: AlphaMode::Opaque,
        mvp,
        tint: [1., 0., 0., 1.],
    })
    .unwrap();
    let px = dev.end_frame().unwrap();
    for y in 0..H as usize {
        for x in 0..W as usize {
            let p = &px[(y * W as usize + x) * 4..][..4];
            // Column 4 sits on the edge; the rasterizer's tie rule decides it, so skip it.
            match x {
                0..=2 => assert_eq!(p, [0, 0, 0, 255], "left half at {x},{y}"),
                5..=7 => assert_eq!(p, [255, 0, 0, 255], "right half at {x},{y}"),
                _ => {}
            }
        }
    }
}

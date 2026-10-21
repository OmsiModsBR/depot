//! Device tests of the interface layer, skipped like the others without a usable device.

use super::tests::device;
use super::*;
use crate::scene::{SceneTexture, TexFormat};
use crate::ui::{UiDraw, UiFrame, UiLayer, UiOverlay, UI_VERTEX_FLOATS};

const W: u32 = 8;
const H: u32 = 4;

/// Two triangles in pixels over (x0, y0)..(x1, y1), `color` times the picture.
fn rect(x0: f32, y0: f32, x1: f32, y1: f32, color: [f32; 4]) -> Vec<f32> {
    let v = |x: f32, y: f32, u: f32, w: f32| {
        let mut p = [0.0; UI_VERTEX_FLOATS];
        p[0] = x;
        p[1] = y;
        p[7] = u;
        p[8] = w;
        p[9..13].copy_from_slice(&color);
        p[14] = 1.0;
        p
    };
    [
        v(x0, y0, 0., 0.),
        v(x1, y0, 1., 0.),
        v(x1, y1, 1., 1.),
        v(x0, y0, 0., 0.),
        v(x1, y1, 1., 1.),
        v(x0, y1, 0., 1.),
    ]
    .concat()
}

fn white_atlas(dev: &mut D3d9Device, owner: usize) {
    dev.ui_atlas(owner, 4, &[255; 64], [0, 0, 4, 4]).unwrap();
}

fn frame(dev: &mut D3d9Device, draw: impl FnOnce(&mut D3d9Device)) -> Vec<u8> {
    dev.begin_frame(&FrameDesc {
        width: W,
        height: H,
        clear_rgba: [0.0, 0.0, 0.0, 1.0],
    })
    .unwrap();
    draw(dev);
    dev.end_frame().unwrap()
}

fn px(p: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * W + x) * 4) as usize;
    [p[at], p[at + 1], p[at + 2], p[at + 3]]
}

fn near(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(b).take(3).all(|(x, y)| x.abs_diff(y) <= tol)
}

fn full() -> UiLayer {
    UiLayer::flat([0.0, 0.0, W as f32, H as f32], 0.0, 1.0)
}

fn one_draw(texture: usize) -> [UiDraw; 1] {
    [UiDraw {
        buffer: 0,
        range: 0..6,
        layer: 0,
        texture,
    }]
}

#[test]
fn the_interface_draws_its_colours_over_the_frame() {
    let Some(mut dev) = device() else { return };
    white_atlas(&mut dev, 1);
    dev.ui_vertices(1, 0, &rect(0., 0., 8., 4., [0.5, 0.25, 1.0, 1.0]))
        .unwrap();
    let p = frame(&mut dev, |d| {
        d.draw_ui(
            1,
            &UiFrame {
                size: (W, H),
                clear: None,
                layers: &[full()],
                draws: &one_draw(0),
            },
        )
        .unwrap()
    });
    // (as stored: B, G, R, A in memory read back as R, G, B, A)
    assert!(
        near(px(&p, 3, 2), [128, 64, 255, 255], 2),
        "{:?}",
        px(&p, 3, 2)
    );
}

#[test]
fn layers_clip_and_fade() {
    let Some(mut dev) = device() else { return };
    white_atlas(&mut dev, 1);
    dev.ui_vertices(1, 0, &rect(0., 0., 8., 4., [1.0; 4]))
        .unwrap();
    let half = UiLayer::flat([0.0, 0.0, 4.0, 4.0], 0.0, 0.5);
    let p = frame(&mut dev, |d| {
        d.draw_ui(
            1,
            &UiFrame {
                size: (W, H),
                clear: None,
                layers: &[half],
                draws: &one_draw(0),
            },
        )
        .unwrap()
    });
    // half of white over black: 0.5 in linear light where the device blends so, else 0.5
    // as stored
    let grey = if dev.scene_caps.linear_ui { 188 } else { 128 };
    assert!(near(px(&p, 1, 1), [grey; 4], 3), "{:?}", px(&p, 1, 1));
    assert!(near(px(&p, 6, 1), [0; 4], 0), "{:?}", px(&p, 6, 1));
}

#[test]
fn pictures_are_drawn_and_missing_ones_are_not() {
    let Some(mut dev) = device() else { return };
    white_atlas(&mut dev, 1);
    dev.ui_image(1, 2, 2, 2, &[10, 200, 30, 255].repeat(4))
        .unwrap();
    let mut v = rect(0., 0., 4., 4., [1.0; 4]);
    v.extend(rect(4., 0., 8., 4., [1.0; 4]));
    dev.ui_vertices(1, 0, &v).unwrap();
    let draws = [
        UiDraw {
            buffer: 0,
            range: 0..6,
            layer: 0,
            texture: 2,
        },
        UiDraw {
            buffer: 0,
            range: 6..12,
            layer: 0,
            texture: 7,
        },
    ];
    let p = frame(&mut dev, |d| {
        d.draw_ui(
            1,
            &UiFrame {
                size: (W, H),
                clear: None,
                layers: &[full()],
                draws: &draws,
            },
        )
        .unwrap()
    });
    assert!(
        near(px(&p, 1, 1), [10, 200, 30, 255], 2),
        "{:?}",
        px(&p, 1, 1)
    );
    assert!(near(px(&p, 6, 1), [0; 4], 0), "{:?}", px(&p, 6, 1));
    dev.ui_free(1, 2);
    assert!(!dev.has_ui_texture(1, 2) && dev.has_ui_texture(1, 0));
    dev.ui_gone(1);
    assert!(!dev.has_ui_texture(1, 0));
}

#[test]
fn a_draw_outside_its_buffer_or_a_frame_is_refused() {
    let Some(mut dev) = device() else { return };
    white_atlas(&mut dev, 1);
    dev.ui_vertices(1, 0, &rect(0., 0., 8., 4., [1.0; 4]))
        .unwrap();
    let ui = |range| UiDraw {
        buffer: 0,
        range,
        layer: 0,
        texture: 0,
    };
    let layers = [full()];
    let outside = [ui(0..12)];
    let f = UiFrame {
        size: (W, H),
        clear: None,
        layers: &layers,
        draws: &outside,
    };
    assert!(dev.draw_ui(1, &f).is_err());
    frame(&mut dev, |d| assert!(d.draw_ui(1, &f).is_err()));
    assert!(dev.ui_vertices(1, 0, &[0.0; 7]).is_err());
    assert!(dev.ui_atlas(1, 4, &[0; 64], [2, 0, 4, 1]).is_err());
}

#[test]
fn overlays_show_scene_pictures_straight_or_premultiplied() {
    let Some(mut dev) = device() else { return };
    let levels = vec![[0, 0, 255, 128].repeat(4)];
    let texture = SceneTexture {
        width: 2,
        height: 2,
        format: TexFormat::Rgba8,
        levels: &levels,
        make_mips: false,
    };
    dev.set_scene_texture(3, &texture).unwrap();
    let p = frame(&mut dev, |d| {
        d.draw_overlays(&[
            UiOverlay {
                texture: 3,
                rect: [0.0, 0.0, 4.0, 4.0],
                premultiplied: false,
                transposed: false,
            },
            UiOverlay {
                texture: 3,
                rect: [4.0, 0.0, 8.0, 4.0],
                premultiplied: true,
                transposed: true,
            },
            UiOverlay {
                texture: 99,
                rect: [0.0, 0.0, 8.0, 4.0],
                premultiplied: false,
                transposed: false,
            },
        ])
        .unwrap()
    });
    // straight: blue at half alpha over black; premultiplied: all of the blue
    let half = if dev.scene_caps.linear_ui { 188 } else { 128 };
    assert!(
        near(px(&p, 1, 1), [0, 0, half, 255], 3),
        "{:?}",
        px(&p, 1, 1)
    );
    assert!(
        near(px(&p, 6, 1), [0, 0, 255, 255], 2),
        "{:?}",
        px(&p, 6, 1)
    );
}

#[test]
fn a_device_opened_again_gets_the_interface_back() {
    let Some(mut dev) = device() else { return };
    white_atlas(&mut dev, 1);
    dev.ui_image(1, 2, 2, 2, &[255; 16]).unwrap();
    dev.ui_vertices(1, 0, &rect(0., 0., 8., 4., [1.0, 0.0, 0.0, 1.0]))
        .unwrap();
    dev.recover().unwrap();
    assert!(dev.has_ui_texture(1, 0) && dev.has_ui_texture(1, 2));
    let p = frame(&mut dev, |d| {
        d.draw_ui(
            1,
            &UiFrame {
                size: (W, H),
                clear: None,
                layers: &[full()],
                draws: &one_draw(2),
            },
        )
        .unwrap()
    });
    assert!(
        near(px(&p, 2, 2), [255, 0, 0, 255], 1),
        "{:?}",
        px(&p, 2, 2)
    );
}

#[test]
fn the_interface_draws_into_scene_textures_shown_as_overlays() {
    let Some(mut dev) = device() else { return };
    white_atlas(&mut dev, 1);
    // green over the left half of a 4 by 4 target, the rest left transparent
    dev.ui_vertices(1, 0, &rect(0., 0., 2., 4., [0.0, 1.0, 0.0, 1.0]))
        .unwrap();
    let into = |dev: &mut D3d9Device, size: u32| {
        dev.draw_ui_into(
            1,
            5,
            &UiFrame {
                size: (size, size),
                clear: Some([0.0; 4]),
                layers: &[UiLayer::flat(
                    [0.0, 0.0, size as f32, size as f32],
                    0.0,
                    1.0,
                )],
                draws: &one_draw(0),
            },
        )
    };
    let shown = |dev: &mut D3d9Device, draw_inside: bool| {
        dev.begin_frame(&FrameDesc {
            width: W,
            height: H,
            clear_rgba: [1.0, 0.0, 0.0, 1.0],
        })
        .unwrap();
        if draw_inside {
            into(dev, 4).unwrap();
        }
        dev.draw_overlays(&[UiOverlay {
            texture: 5,
            rect: [0.0, 0.0, W as f32, H as f32],
            premultiplied: true,
            transposed: false,
        }])
        .unwrap();
        dev.end_frame().unwrap()
    };
    // drawn outside a frame (as the minimap is, before the scene), then shown
    into(&mut dev, 4).unwrap();
    assert!(dev.has_scene_texture(5));
    let p = shown(&mut dev, false);
    assert!(
        near(px(&p, 1, 1), [0, 255, 0, 255], 2),
        "{:?}",
        px(&p, 1, 1)
    );
    assert!(
        near(px(&p, 6, 2), [255, 0, 0, 255], 2),
        "{:?}",
        px(&p, 6, 2)
    );
    // drawn inside one: the frame goes on into the window's picture
    let p = shown(&mut dev, true);
    assert!(
        near(px(&p, 1, 1), [0, 255, 0, 255], 2),
        "{:?}",
        px(&p, 1, 1)
    );
    assert!(
        near(px(&p, 6, 2), [255, 0, 0, 255], 2),
        "{:?}",
        px(&p, 6, 2)
    );
    // a new size makes it again; none at all is refused
    into(&mut dev, 2).unwrap();
    let p = shown(&mut dev, false);
    assert!(
        near(px(&p, 1, 1), [0, 255, 0, 255], 2),
        "{:?}",
        px(&p, 1, 1)
    );
    assert!(into(&mut dev, 0).is_err());
    // freed with the renderer's texture; a device opened again has it once drawn again
    dev.free_scene_texture(5);
    let p = shown(&mut dev, false);
    assert!(
        near(px(&p, 1, 1), [255, 0, 0, 255], 2),
        "{:?}",
        px(&p, 1, 1)
    );
    into(&mut dev, 4).unwrap();
    dev.recover().unwrap();
    assert!(!dev.has_scene_texture(5));
    into(&mut dev, 4).unwrap();
    let p = shown(&mut dev, false);
    assert!(
        near(px(&p, 1, 1), [0, 255, 0, 255], 2),
        "{:?}",
        px(&p, 1, 1)
    );
}

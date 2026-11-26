use omsi_render_vanilla::testing::domain::ui::*;

#[test]
fn a_flat_layer_covers_the_target_and_lays_out_in_its_own_box() {
    let mut l = UiLayer::flat([0.0, 0.0, 100.0, 50.0], 0.0, 1.0);
    l.viewport = [0.0, 0.0, 800.0, 450.0];
    assert_eq!(l.viewport_in(1600, 900), [0, 0, 1600, 900]);
    assert_eq!(l.layout_box(1600, 900), [0.0, 0.0, 800.0, 450.0]);
    let world = UiLayer {
        px_scale: 0.01,
        viewport: [100.0, 50.0, 4000.0, 300.0],
        ..l
    };
    assert_eq!(world.viewport_in(1600, 900), [100, 50, 1500, 300]);
}

#[test]
fn the_scissor_is_the_clip_on_whole_pixels_inside_the_target() {
    let l = UiLayer::flat([-5.0, 10.2, 99.5, 2000.0], 0.0, 1.0);
    assert_eq!(l.scissor_in(100, 100), Some([0, 10, 100, 100]));
    assert_eq!(
        UiLayer::flat([5.0, 5.0, 5.0, 9.0], 0.0, 1.0).scissor_in(10, 10),
        None
    );
}

#[test]
fn overlays_snap_and_turn_their_pictures() {
    assert_eq!(snap_rect([0.4, 0.5, 0.6, 10.49]), [0.0, 1.0, 1.0, 10.0]);
    let v = overlay_vertices(&UiOverlay {
        texture: 1,
        rect: [10.0, 20.0, 30.0, 60.0],
        premultiplied: false,
        transposed: true,
    });
    // the second corner: right, top; on its side its uv is (0, 1)
    assert_eq!(&v[1][..2], &[30.0, 20.0]);
    assert_eq!(&v[1][7..9], &[0.0, 1.0]);
    assert_eq!(v[1][14], 1.0);
}

#[test]
fn atlas_regions_are_checked() {
    assert!(atlas_region_fits(4, 64, [0, 0, 4, 4]));
    assert!(!atlas_region_fits(4, 64, [1, 0, 4, 4]));
    assert!(!atlas_region_fits(4, 60, [0, 0, 1, 1]));
    assert!(!atlas_region_fits(4, 64, [u32::MAX, 0, 2, 1]));
}

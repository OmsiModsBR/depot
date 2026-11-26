//! Tests moved out of `src/output/destination.rs`.

use omsi_render::testing::internals::*;

#[test]
fn only_the_window_picture_gets_the_overlay_hook() {
    let panel = Output::TriplePanel {
        projection: Mat4::IDENTITY,
        later: false,
    };
    assert!(Output::Window.is_window());
    assert!(!panel.is_window());
    // (the panels draw the HUD and overlays too, which is why `flags` alone is not enough)
    assert!(panel.flags().0);
    assert!(!Output::XrEye {
        projection: Mat4::IDENTITY,
        second: false
    }
    .is_window());
    assert!(!Output::Texture {
        exclude: TextureId::default()
    }
    .is_window());
}

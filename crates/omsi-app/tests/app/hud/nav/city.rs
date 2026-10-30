use omsi_ui::Rect;
use openomsi_game::testing::hud::nav::city::*;
use openomsi_game::testing::hud::nav::draw::rects_overlap;

/// The compass' clickable rect, as `draw` registers it.
fn compass_rect(w: f32, h: f32, s: f32, open: bool) -> Rect {
    let (c, r) = (compass_centre(w, h, s, open), 24.0 * s);
    Rect::new(c.x - r, c.y - r, 2.0 * r, 2.0 * r + 16.0 * s)
}

#[test]
fn the_compass_keeps_its_place_while_the_lines_panel_is_closed() {
    let (w, h, s) = (1280.0, 720.0, 1.0);
    let c = compass_centre(w, h, s, false);
    assert_eq!((c.x, c.y), (w - 36.0 - 24.0, h - 62.0 - 24.0));
}

#[test]
fn the_compass_never_sits_under_the_lines_panel() {
    for (w, h) in [
        (1920.0, 1080.0),
        (1280.0, 720.0),
        (1024.0, 600.0),
        (640.0, 480.0),
        (400.0, 300.0),
    ] {
        for s in [0.75, 1.0, 1.5, 2.0] {
            let (x0, pw) = lines_panel_span(w, s);
            let panel = Rect::new(x0, 44.0 * s, pw, h - 44.0 * s - 12.0 * s);
            let compass = compass_rect(w, h, s, true);
            assert!(!rects_overlap(&compass, &panel), "{w}x{h} at {s}");
            assert!(compass.right() <= x0, "{w}x{h} at {s}");
        }
    }
}

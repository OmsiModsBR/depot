use glam::Vec3;
use openomsi_game::testing::output::mirror_hud::*;
use winit::keyboard::KeyCode;

fn editing(panels: Vec<Panel>) -> MirrorHud {
    MirrorHud {
        enabled: true,
        edit: true,
        panels,
        ..Default::default()
    }
}

#[test]
fn a_panel_keeps_its_place_inside_the_window() {
    let mut m = editing(vec![Panel {
        cam: 0,
        x: 0.9,
        y: 0.9,
        h: 0.3,
        aspect: 0.5,
    }]);
    assert!(m.wheel(
        5.0,
        false,
        (0.9 * 1600.0 + 5.0, 0.9 * 900.0 + 5.0),
        (1600.0, 900.0)
    ));
    let r = MirrorHud::rect(&m.panels[0], 1600.0, 900.0);
    assert!(r[2] <= 1600.5 && r[3] <= 900.5, "{r:?}");
}

#[test]
fn dragging_moves_the_grabbed_corner_with_the_cursor() {
    let mut m = editing(vec![Panel {
        cam: 0,
        x: 0.1,
        y: 0.1,
        h: 0.3,
        aspect: 1.0,
    }]);
    let size = (1000.0, 1000.0);
    assert!(m.press(true, (150.0, 150.0), size));
    assert!(m.moved((350.0, 250.0), size));
    let q = m.panels[0];
    assert!(
        (q.x - 0.3).abs() < 1e-4 && (q.y - 0.2).abs() < 1e-4,
        "{q:?}"
    );
    // letting go ends the drag; the cursor then moves nothing
    assert!(m.press(false, (350.0, 250.0), size));
    assert!(!m.moved((900.0, 900.0), size));
}

#[test]
fn the_mouse_does_nothing_to_the_panels_outside_the_editor() {
    let mut m = editing(vec![Panel {
        cam: 0,
        x: 0.1,
        y: 0.1,
        h: 0.3,
        aspect: 1.0,
    }]);
    m.edit = false;
    let size = (1000.0, 1000.0);
    assert!(
        !m.press(true, (150.0, 150.0), size),
        "a click is left to the cab"
    );
    assert!(
        !m.wheel(1.0, false, (150.0, 150.0), size),
        "the wheel is left to the zoom"
    );
    assert!(!m.dragging());
}

#[test]
fn the_arrows_are_the_editors_only_while_it_is_on() {
    let mut m = editing(vec![Panel {
        cam: 3,
        x: 0.1,
        y: 0.1,
        h: 0.3,
        aspect: 1.0,
    }]);
    let size = (1000.0, 1000.0);
    assert!(m.arrow(KeyCode::ArrowLeft, true), "a press is taken");
    assert_eq!(
        m.turning(),
        Some([true, false, false, false, false, false, false, false])
    );
    assert_eq!(m.cam_under((150.0, 150.0), size), Some(3));
    assert_eq!(m.cam_under((900.0, 900.0), size), None);
    assert!(
        !m.arrow(KeyCode::ArrowLeft, false),
        "a release goes on to the game"
    );
    assert_eq!(m.turning(), None);
    m.edit = false;
    assert!(
        !m.arrow(KeyCode::ArrowRight, true),
        "outside the editor the key is the game's"
    );
    assert_eq!(m.turning(), None);
    assert!(!m.arrow(KeyCode::KeyA, true));
    m.edit = true;
    assert!(
        m.arrow(KeyCode::PageUp, true),
        "Page Up shifts the mirror forward"
    );
    assert_eq!(
        m.turning(),
        Some([false, false, false, false, true, false, false, false])
    );
    assert!(
        m.arrow(KeyCode::Equal, true),
        "plus widens the field of view"
    );
    assert_eq!(m.turning().map(|a| a[7]), Some(true));
}

#[test]
fn only_a_click_on_a_panel_is_taken() {
    let mut m = editing(vec![Panel {
        cam: 0,
        x: 0.1,
        y: 0.1,
        h: 0.3,
        aspect: 1.0,
    }]);
    assert!(!m.press(true, (900.0, 900.0), (1000.0, 1000.0)));
    assert!(!m.dragging());
}

#[test]
fn the_panel_picked_up_comes_to_the_top() {
    let mut m = editing(vec![
        Panel {
            cam: 0,
            x: 0.1,
            y: 0.1,
            h: 0.3,
            aspect: 1.0,
        },
        Panel {
            cam: 1,
            x: 0.5,
            y: 0.5,
            h: 0.3,
            aspect: 1.0,
        },
    ]);
    assert!(m.press(true, (150.0, 150.0), (1000.0, 1000.0)));
    assert_eq!(m.panels.last().map(|q| q.cam), Some(0));
}

#[test]
fn the_size_keys_change_the_panel_under_the_cursor() {
    let mut m = editing(vec![Panel {
        cam: 0,
        x: 0.1,
        y: 0.1,
        h: 0.3,
        aspect: 1.0,
    }]);
    let at = (0.1 * 1600.0 + 5.0, 0.1 * 900.0 + 5.0);
    assert!(
        m.size_key(KeyCode::BracketRight, at, (1600.0, 900.0)),
        "] widens"
    );
    assert!(m.panels[0].aspect > 1.0 && m.panels[0].h == 0.3);
    assert!(
        m.size_key(KeyCode::Quote, at, (1600.0, 900.0)),
        "' makes it taller"
    );
    assert!(m.panels[0].h > 0.3);
    assert!(m.size_key(KeyCode::Semicolon, at, (1600.0, 900.0)));
    assert!(m.size_key(KeyCode::BracketLeft, at, (1600.0, 900.0)));
    assert!(
        (m.panels[0].h - 0.3).abs() < 0.01 && (m.panels[0].aspect - 1.0).abs() < 0.01,
        "a step each way ends about where it began"
    );
    assert!(!m.size_key(KeyCode::KeyA, at, (1600.0, 900.0)));
    let mut off = MirrorHud {
        enabled: true,
        panels: m.panels.clone(),
        ..Default::default()
    };
    assert!(
        !off.size_key(KeyCode::BracketRight, at, (1600.0, 900.0)),
        "outside the editor the key is the game's"
    );
}

#[test]
fn a_picture_is_laid_out_as_its_glass_shows_it_to_the_driver() {
    let (eye, centre) = (Vec3::ZERO, Vec3::new(0.0, 5.0, 0.0));
    let turn = |du, dv| Turn::of(eye, centre, du, dv);
    // seen from the front: right is +x, up is +z
    assert_eq!(
        turn(Vec3::X, -Vec3::Z),
        Turn {
            transposed: false,
            x_reversed: false,
            y_reversed: false
        }
    );
    // the usual side mirror: the picture runs to the left
    assert_eq!(turn(-Vec3::X, -Vec3::Z), Turn::default());
    // v up the glass: the picture upside down
    assert_eq!(
        turn(Vec3::X, Vec3::Z),
        Turn {
            transposed: false,
            x_reversed: false,
            y_reversed: true
        }
    );
    // on its side: u up the glass, v across it
    assert_eq!(
        turn(Vec3::Z, Vec3::X),
        Turn {
            transposed: true,
            x_reversed: false,
            y_reversed: true
        }
    );
    assert_eq!(
        turn(-Vec3::Z, -Vec3::X),
        Turn {
            transposed: true,
            x_reversed: true,
            y_reversed: false
        }
    );
}

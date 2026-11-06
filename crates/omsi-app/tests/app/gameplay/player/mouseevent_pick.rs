use openomsi_game::testing::player::{
    best_mouseevent_hit, best_unblocked_hit, drawn_from, MOUSEEVENT_PICK_DEPTH_BAND,
};

#[test]
fn a_switch_behind_a_panel_is_not_picked() {
    // the ray meets a panel 0.3 m away before the switch 0.5 m away
    assert_eq!(best_unblocked_hit(vec![(0.5, 0.0, 7, 0)], |_| 0.3), None);
}

#[test]
fn a_switch_on_its_panel_is_picked() {
    // the panel the switch is mounted on lies just behind it, or flush with it
    assert_eq!(best_unblocked_hit(vec![(0.5, 0.0, 7, 0)], |_| 0.5), Some(7));
    assert_eq!(
        best_unblocked_hit(vec![(0.5, 0.0, 7, 0)], |_| 0.52),
        Some(7)
    );
    assert_eq!(
        best_unblocked_hit(vec![(0.5, 0.0, 7, 0)], |_| f32::INFINITY),
        Some(7)
    );
}

#[test]
fn a_hidden_switch_gives_way_to_one_seen_by_another_ray() {
    // ray 0 (nearer switch) is blocked by a window, ray 1 reaches a switch farther off
    let hits = vec![(0.4, 0.0, 1, 0), (0.6, 0.0, 2, 1)];
    let mut traced = Vec::new();
    let pick = best_unblocked_hit(hits, |r| {
        traced.push(r);
        if r == 0 {
            0.2
        } else {
            f32::INFINITY
        }
    });
    assert_eq!(pick, Some(2));
    assert_eq!(traced, vec![0, 1]);
}

#[test]
fn viewpoint_bits_decide_what_each_camera_sees() {
    assert!(drawn_from(0, true) && drawn_from(0, false));
    assert!(drawn_from(1, false) && !drawn_from(1, true));
    assert!(drawn_from(2, true) && !drawn_from(2, false));
    assert!(drawn_from(3, true) && drawn_from(3, false));
}

#[test]
fn aachen_ibox_d11_wins_when_d10_is_closer_but_off_aim() {
    // Realistic AC O530 digit row: D10 (backspace) ~12 mm closer along the ray than
    // D11 (digit 0), but the cursor aims at D11's centre (lateral ≈ 0).
    let d10 = 0usize;
    let d11 = 1usize;
    let d12 = 2usize;
    let hits = [
        (0.488, 0.012_f32.powi(2), d10),
        (0.500, 0.001_f32.powi(2), d11),
        (0.512, 0.015_f32.powi(2), d12),
    ];
    assert!(
        (hits[1].0 - hits[0].0) > 0.002,
        "fixture must exceed the old 2 mm tie window"
    );
    assert!((hits[1].0 - hits[0].0) < MOUSEEVENT_PICK_DEPTH_BAND);
    assert_eq!(best_mouseevent_hit(&hits), Some(d11));
}

#[test]
fn clearly_nearer_surface_still_wins_outside_the_band() {
    let hits = [(0.40, 0.02_f32.powi(2), 0usize), (0.50, 0.0, 1usize)];
    assert_eq!(best_mouseevent_hit(&hits), Some(0));
}

#[test]
fn empty_hits_yield_none() {
    assert_eq!(best_mouseevent_hit(&[]), None);
}

use openomsi_game::testing::player::HeldRepeat;

/// Page Up held three frames turns the roller blind on each of them: once by the press,
/// twice from the list; let go, no more (#1098, #1145).
#[test]
fn a_held_key_is_told_every_frame_after_its_press_until_let_go() {
    let mut r = HeldRepeat::default();
    assert!(r.due().is_empty());
    r.press(201, vec!["bus_rollband_up".into()]);
    assert!(r.due().is_empty(), "the press itself was this frame's call");
    assert_eq!(r.due(), vec!["bus_rollband_up".to_string()]);
    assert_eq!(r.due(), vec!["bus_rollband_up".to_string()]);
    r.release(201);
    assert!(r.due().is_empty());
    // a key without a held action is none of it
    r.press(63, Vec::new());
    r.due();
    assert!(r.due().is_empty());
}

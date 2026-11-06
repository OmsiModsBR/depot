use openomsi_game::testing::player::indicator_toggle_action;

#[test]
fn repeated_presses_switch_each_side_on_then_off() {
    for (side, on) in [(1, "blinker_left_set"), (2, "blinker_right_set")] {
        let mut state = 0;
        assert_eq!(indicator_toggle_action(&mut state, None, side), on);
        assert_eq!(
            indicator_toggle_action(&mut state, None, side),
            "blinker_off"
        );
        assert_eq!(state, 0);
    }
}

#[test]
fn changing_side_and_automatic_cancellation_use_the_current_lever() {
    let mut state = 1;
    assert_eq!(
        indicator_toggle_action(&mut state, Some(1), 2),
        "blinker_right_set"
    );
    assert_eq!(
        indicator_toggle_action(&mut state, Some(0), 2),
        "blinker_right_set"
    );
    assert_eq!(
        indicator_toggle_action(&mut state, Some(2), 2),
        "blinker_off"
    );
}

#[test]
fn hazards_keep_their_dedicated_toggle_trigger() {
    let mut state = 0;
    assert_eq!(
        indicator_toggle_action(&mut state, Some(0), 3),
        "blinker_warn_toggle"
    );
    assert_eq!(state, 3);
    assert_eq!(
        indicator_toggle_action(&mut state, Some(3), 3),
        "blinker_warn_toggle"
    );
    assert_eq!(state, 0);
}

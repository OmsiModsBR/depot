#[test]
fn a_dpad_direction_is_numbered_as_the_first_hat() {
    assert_eq!(
        openomsi_game::testing::mac_game_controller::button_index(0),
        0
    );
    assert_eq!(
        openomsi_game::testing::mac_game_controller::button_index(11),
        11
    );
    assert_eq!(
        openomsi_game::testing::mac_game_controller::button_index(12),
        openomsi_game::testing::controllers::HAT_BUTTONS
    );
    assert_eq!(
        openomsi_game::testing::mac_game_controller::button_index(15),
        openomsi_game::testing::controllers::HAT_BUTTONS + 3
    );
}

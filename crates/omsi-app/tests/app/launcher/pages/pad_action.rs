/// Every game action a button can be given is one the game carries out from a
/// controller (app_events: `view_look_*` / `voice_radio` while held, the gears by name,
/// the rest through `is_game_action`, the doors through `Player::action`) - Space's
/// reset of every view among them (#1167).
#[test]
fn a_button_can_reset_every_view() {
    assert!(openomsi_game::testing::launcher::pages::PAD_GAME_ACTIONS
        .contains(&"view_reset_all_directions"));
    assert!(openomsi_game::testing::launcher::pages::PAD_GAME_ACTIONS.contains(&"voice_radio"));
    assert_eq!(
        openomsi_game::testing::launcher::pages::known_action("voice_radio").as_deref(),
        Some("Multiplayer: bus radio (hold)")
    );
    for a in openomsi_game::testing::launcher::pages::PAD_GAME_ACTIONS {
        let held = a.starts_with("view_look_") || a == "voice_radio";
        let handled = held
            || openomsi_game::testing::actions::is_game_action(a)
            || a.starts_with("gear_")
            || openomsi_game::testing::player::door_action(a).is_some();
        assert!(handled, "{a}");
    }
    // held pad actions are game actions so they never reach the bus script
    assert!(openomsi_game::testing::actions::is_game_action(
        "voice_radio"
    ));
    assert!(openomsi_game::testing::actions::is_game_action(
        "view_look_left"
    ));
}

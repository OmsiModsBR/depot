#[test]
fn axes_are_counted() {
    assert_eq!(openomsi_game::testing::evdev_buttons::bit_count("0"), 0);
    assert_eq!(openomsi_game::testing::evdev_buttons::bit_count("3007b"), 8);
    assert_eq!(openomsi_game::testing::evdev_buttons::bit_count("1 3"), 3);
}

#[test]
fn keyboards_and_mice_are_told_from_a_gear_shifter() {
    // the Aerosoft gear shifter, a gaming mouse, a keyboard's media keys
    assert!(!openomsi_game::testing::evdev_buttons::has_keyboard_keys(
        "3f 0 0 0 0 0 0 ffff00000000 0 0 0 0"
    ));
    assert!(!openomsi_game::testing::evdev_buttons::has_keyboard_keys(
        "ffffffff0000 0 0 0 0"
    ));
    assert!(openomsi_game::testing::evdev_buttons::has_keyboard_keys("3f00733fff 0 0 483ffff17aff32d bfd4444600000000 1 130ff38b17d000 677bfad9415fed 19ed68000004400 10000002"));
}

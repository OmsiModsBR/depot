#[test]
fn layouts_match_the_kernel() {
    assert_eq!(
        std::mem::size_of::<openomsi_game::testing::evdev_ff::FfEffect>(),
        48
    );
    assert_eq!(
        std::mem::size_of::<openomsi_game::testing::evdev_ff::InputEvent>(),
        24
    );
}

#[test]
fn ff_bits() {
    assert!(openomsi_game::testing::evdev_ff::has_bit(
        "11fff0000 0",
        0x52
    ));
    assert!(!openomsi_game::testing::evdev_ff::has_bit(
        "11fff0000 0",
        0x61
    ));
    assert!(!openomsi_game::testing::evdev_ff::has_bit("0", 0x52));
}

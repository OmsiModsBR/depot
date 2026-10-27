#[test]
fn the_window_size_is_read_from_the_settings() {
    assert_eq!(
        openomsi_game::testing::settings::resolution_of("msaa=4\nresolution=1280x800\n"),
        Some((1280, 800))
    );
    assert_eq!(
        openomsi_game::testing::settings::resolution_of("resolution=auto\n"),
        None
    );
    assert_eq!(
        openomsi_game::testing::settings::resolution_of("window_size=1920*1080\n"),
        Some((1920, 1080))
    );
    assert_eq!(
        openomsi_game::testing::settings::resolution_of("vsync=1\n"),
        None
    );
}

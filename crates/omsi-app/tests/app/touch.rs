use openomsi_game::testing::settings::Settings;
use openomsi_game::testing::touch::touch_lock_angle;

/// The information bar keeps to the gap between the buttons along the top (it lay under
/// them, its ends hidden, #1164), or goes under them where the gap is narrow.
#[test]
fn the_information_bar_keeps_clear_of_the_buttons_along_the_top() {
    assert_eq!(
        openomsi_game::testing::touch::info_room(1280.0, 14.0, 278.0, 914.0, 56.0, 1.0),
        [286.0, 906.0, 14.0]
    );
    assert_eq!(
        openomsi_game::testing::touch::info_room(720.0, 14.0, 278.0, 470.0, 56.0, 1.0),
        [14.0, 706.0, 64.0]
    );
}

#[test]
fn the_screen_wheel_turns_as_the_wheel_settings_say() {
    let deg = |s: &Settings| touch_lock_angle(s).to_degrees().round();
    // "Full lock at: OMSI": the wheel's whole rotation is the lock, half of it each way
    assert_eq!(
        deg(&Settings {
            wheel_range: 900.0,
            wheel_lock: 0.0,
            ..Default::default()
        }),
        450.0
    );
    assert_eq!(
        deg(&Settings {
            wheel_range: 1800.0,
            wheel_lock: 0.0,
            ..Default::default()
        }),
        900.0
    );
    // a lock set of its own wins
    assert_eq!(
        deg(&Settings {
            wheel_range: 900.0,
            wheel_lock: 540.0,
            ..Default::default()
        }),
        270.0
    );
}

use openomsi_game::testing::input::mouse::confines_cursor;

/// The cursor is held inside the window only while the mouse steers and nothing else has
/// the mouse: the window in the background or the game menu let it go.
#[test]
fn the_cursor_is_held_in_the_window_only_while_the_mouse_steers() {
    assert!(confines_cursor(true, false, false));
    assert!(!confines_cursor(false, false, false));
    assert!(!confines_cursor(true, true, false), "focus lost");
    assert!(!confines_cursor(true, false, true), "menu open");
}

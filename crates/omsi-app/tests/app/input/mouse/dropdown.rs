/// A drop-down's thumb dragged down its track scrolls the list to its end (#794).
#[test]
fn a_dropdowns_thumb_dragged_scrolls_it() {
    // 40 entries, 8 shown, a 300 px track with a 60 px thumb
    let track = [0.0, 100.0, 4.0, 400.0];
    assert_eq!(
        openomsi_game::testing::input::mouse::dropdown_top_at(100.0, track, 60.0, 40, 8),
        0
    );
    assert_eq!(
        openomsi_game::testing::input::mouse::dropdown_top_at(340.0, track, 60.0, 40, 8),
        32
    );
    assert_eq!(
        openomsi_game::testing::input::mouse::dropdown_top_at(220.0, track, 60.0, 40, 8),
        16
    );
    // past the ends it stays at them
    assert_eq!(
        openomsi_game::testing::input::mouse::dropdown_top_at(-50.0, track, 60.0, 40, 8),
        0
    );
    assert_eq!(
        openomsi_game::testing::input::mouse::dropdown_top_at(900.0, track, 60.0, 40, 8),
        32
    );
    // a list that fits never scrolls
    assert_eq!(
        openomsi_game::testing::input::mouse::dropdown_top_at(300.0, track, 60.0, 5, 8),
        0
    );
}

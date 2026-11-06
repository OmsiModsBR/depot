use openomsi_game::testing::player::auto_shift_gear as g;

#[test]
fn the_automated_manual_shifts_by_the_engine_speed() {
    // pulling away: first gear in from neutral, never reverse
    assert_eq!(g(0, 5, 1.0, 0.5, 0.0, 0.0), 1);
    assert_eq!(g(0, 5, 1.0, 0.0, 0.0, 0.0), 0);
    assert_eq!(g(-1, 5, 3.0, 1.0, 0.0, 5.0), -1);
    // up early with a light foot, late with a heavy one
    assert_eq!(g(2, 5, 2.8, 0.3, 0.0, 30.0), 3);
    assert_eq!(g(2, 5, 2.8, 1.0, 0.0, 30.0), 2);
    assert_eq!(g(2, 5, 3.7, 1.0, 0.0, 30.0), 3);
    // never past the top gear, down near the idle, not below first
    assert_eq!(g(5, 5, 4.0, 1.0, 0.0, 90.0), 5);
    assert_eq!(g(3, 5, 1.2, 0.0, 0.5, 20.0), 2);
    assert_eq!(g(1, 5, 1.0, 0.0, 1.0, 2.0), 1);
    assert_eq!(g(4, 5, 1.0, 0.0, 1.0, 3.0), 1);
}

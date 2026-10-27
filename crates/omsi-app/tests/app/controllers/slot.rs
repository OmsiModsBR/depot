use openomsi_game::testing::controllers::*;

#[test]
fn a_missing_axis_does_not_shift_the_rest() {
    // X and Rz only (a wheel with one pedal axis): Rz stays slot 5
    let axes = if cfg!(target_os = "macos") {
        vec![(0x1_0030, 0.5), (0x1_0035, -1.0)]
    } else {
        vec![(0x3_0000, 0.5), (0x3_0005, -1.0)]
    };
    let got = di_slots(&axes);
    assert!(got.contains(&(0, 0.5)), "{got:?}");
    if !cfg!(windows) {
        assert!(got.contains(&(5, -1.0)), "{got:?}");
    }
}

#[test]
fn two_devices_merge_by_the_larger_movement() {
    let mut s = None;
    set(&mut s, 0.0);
    set(&mut s, 0.4);
    set(&mut s, -0.1);
    assert_eq!(s, Some(0.4));
}

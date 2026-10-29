use openomsi_game::testing::output::camera::util::auto_outside_fov;

#[test]
fn ultrawide_keeps_the_sideways_view_of_16_9() {
    assert_eq!(auto_outside_fov(16.0 / 9.0), 60.0);
    assert_eq!(auto_outside_fov(4.0 / 3.0), 60.0);
    let wide = |fov: f32, a: f32| (fov.to_radians() * 0.5).tan() * a;
    let uw = auto_outside_fov(21.0 / 9.0);
    assert!(uw < 60.0);
    assert!((wide(uw, 21.0 / 9.0) - wide(60.0, 16.0 / 9.0)).abs() < 1e-4);
    let superwide = auto_outside_fov(32.0 / 9.0);
    assert!(superwide < uw);
    assert!((wide(superwide, 32.0 / 9.0) - wide(60.0, 16.0 / 9.0)).abs() < 1e-4);
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0] {
        assert_eq!(auto_outside_fov(invalid), 60.0);
    }
}

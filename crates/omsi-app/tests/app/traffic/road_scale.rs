use openomsi_game::testing::traffic::road_scale;

#[test]
fn path_density_scales_the_street_target() {
    assert!((road_scale(&[1.0; 100]) - 1.0).abs() < 1e-6);
    assert!((road_scale(&[0.2; 100]) - 0.2).abs() < 1e-6);
    assert_eq!(road_scale(&[0.0; 100]), 0.0);
    assert!((road_scale(&[1.0; 500]) - 2.0).abs() < 1e-6);
}

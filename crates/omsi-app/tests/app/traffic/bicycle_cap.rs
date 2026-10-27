use openomsi_game::testing::traffic::is_bicycle;

#[test]
fn a_light_vehicle_is_no_bicycle_without_the_bicycle_script() {
    // a motorcycle (0.125 t) and other light vehicles follow the road's limit
    assert!(!is_bicycle(0.125, false));
    assert!(!is_bicycle(0.2, false));
    assert!(!is_bicycle(0.0, false));
    assert!(!is_bicycle(0.95, false));
}

#[test]
fn the_stock_weight_or_a_light_vehicle_with_vmax_is_a_bicycle() {
    assert!(is_bicycle(0.3, false));
    assert!(is_bicycle(0.3, true));
    assert!(is_bicycle(0.125, true));
}

#[test]
fn a_heavy_vehicle_with_a_vmax_variable_is_no_bicycle() {
    // a car or a lorry that names a variable `vmax` keeps to the road's limit
    assert!(!is_bicycle(0.95, true));
    assert!(!is_bicycle(12.0, true));
    assert!(!is_bicycle(0.0, true));
}

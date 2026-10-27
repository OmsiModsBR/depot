use openomsi_game::testing::admin::weather_file_ok;

#[test]
fn only_a_weather_file() {
    assert!(weather_file_ok("Weather/#CAVOK.owt"));
    assert!(weather_file_ok("weather\\Bodennebel.OWT"));
    assert!(!weather_file_ok("Weather/../server.cfg"));
    assert!(!weather_file_ok("Weather/sub/x.owt"));
    assert!(!weather_file_ok("maps/x.owt"));
    assert!(!weather_file_ok("Weather/x.cfg"));
}

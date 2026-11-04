use clap::Parser;
use openomsi_game::testing::core::cli::Args;
use openomsi_game::testing::weather_setup::*;

#[test]
fn the_snow_of_a_chosen_weather_is_read_without_loading_it() {
    let mut args = Args::parse_from(["depot"]);
    args.weather = Some("custom:t=-5;snow=1;snowroad=1".into());
    assert_eq!(weather_snow(&args), (true, true));
    args.weather = Some("custom:t=10;snow=0".into());
    assert_eq!(weather_snow(&args), (false, false));
    // a file that is not there: no snow, and no weather taken up
    args.weather = Some("Weather/does-not-exist.owt".into());
    assert_eq!(weather_snow(&args), (false, false));
}

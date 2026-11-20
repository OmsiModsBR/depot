//! The `clouds` setting changed while the game runs (spec 001, FR-020e): the cloud cover of
//! the next picture is the one a game started with that choice has, whatever the choice was
//! before.

use omsi_content::weather::Weather;
use openomsi_game::testing::weather_setup::*;

fn weather(kind: &str) -> Weather {
    Weather {
        fog: (50000.0, 1.0),
        clouds: (kind.into(), 100.0),
        precip: vec![0.0; 5],
        ..Default::default()
    }
}

fn density(w: &Weather) -> f32 {
    let clock = omsi_sim::SimClock::default();
    let daylight = omsi_sim::Daylight::compute(&clock, None);
    weather_lighting(
        &daylight,
        w,
        [0.0; 2],
        0.0,
        false,
        omsi_cfg::GraphicsMode::VanillaPlus,
    )
    .cloud_density
}

/// One test, since the switches are the process's.
#[test]
fn the_cloud_cover_follows_the_setting_in_both_directions() {
    let cumulus = weather("Cumulus 2");
    set_clouds(true, true);
    let fresh_on = density(&cumulus);
    assert!(fresh_on > 0.0);
    set_clouds(false, true);
    let fresh_off = density(&cumulus);
    assert_eq!(fresh_off, 0.0);
    // on -> off -> on: the weather is untouched, the cover is the fresh one
    assert_eq!(density(&cumulus), fresh_off);
    set_clouds(true, true);
    assert_eq!(density(&cumulus), fresh_on);
    // a clear sky stays clear
    assert_eq!(density(&weather("-1")), 0.0);
    // the fair-weather clouds of an unchosen default weather do not depend on the setting
    for clouds in [true, false] {
        set_clouds(clouds, true);
        let mut w = weather("-1");
        with_fair_weather_clouds(&mut w, true);
        assert_eq!(w.clouds.0, "Cumulus 1", "clouds {clouds}");
        let mut chosen = weather("-1");
        with_fair_weather_clouds(&mut chosen, false);
        assert_eq!(chosen.clouds.0, "-1");
    }
    set_clouds(false, false);
    let mut w = weather("-1");
    with_fair_weather_clouds(&mut w, true);
    assert_eq!(w.clouds.0, "-1", "no fair-weather clouds in Vanilla");
    set_clouds(true, true);
}

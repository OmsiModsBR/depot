use glam::Vec3;
use omsi_sim::testing::clock::SimClock;
use omsi_sim::testing::daylight::*;

fn clock_at(hours: f64) -> SimClock {
    SimClock {
        day_of_year: 182,
        time: hours * 3600.0,
        ..Default::default()
    }
}

#[test]
fn the_sun_follows_the_maps_time_zone_and_summer_time() {
    let mut spandau = SunPlace::default();
    spandau.dst.push((19890326, 19890924, 2.0, 3.0, 1.0));
    // 21 June 1989: summer time, the sun due south at about 13:07 local
    let mut c = SimClock {
        year: 1989,
        day_of_year: 172,
        time: 13.12 * 3600.0,
        ..Default::default()
    };
    let (alt, az) = sun_position(&c, &spandau);
    assert!(
        (az - 180.0).abs() < 3.0 && (alt - 61.0).abs() < 1.5,
        "summer noon: alt {alt:.1} az {az:.1}"
    );
    // 21 December: standard time, south at about 12:07
    c.day_of_year = 355;
    c.time = 12.12 * 3600.0;
    let (alt, az) = sun_position(&c, &spandau);
    assert!(
        (az - 180.0).abs() < 3.0 && (alt - 14.0).abs() < 1.5,
        "winter noon: alt {alt:.1} az {az:.1}"
    );
    // without summer time the June sun would be an hour further on
    c.day_of_year = 172;
    c.time = 13.12 * 3600.0;
    let (_, az) = sun_position(&c, &SunPlace::default());
    assert!(az > 195.0, "no DST: az {az:.1}");
}

#[test]
fn the_moon_rises_and_waxes_on_time() {
    // full moon of 20 January 2000 04:40 UTC: nearly all lit, opposite the sun, high
    // in a Berlin winter night
    let berlin = SunPlace {
        latitude: 52.5,
        longitude: 13.4,
        timezone: 0.0,
        dst: Vec::new(),
    };
    let c = SimClock {
        year: 2000,
        day_of_year: 20,
        time: 4.67 * 3600.0,
        ..Default::default()
    };
    let (alt, _, illum) = moon_position(&c, &berlin);
    assert!(illum > 0.98, "illum {illum}");
    assert!(alt > 10.0, "alt {alt}");
    // new moon of 6 January 2000 18:14 UTC: dark
    let c = SimClock {
        year: 2000,
        day_of_year: 6,
        time: 18.23 * 3600.0,
        ..Default::default()
    };
    assert!(moon_position(&c, &berlin).2 < 0.02);
    // first quarter of 14 January 2000 13:34 UTC: half lit, in the south-east to south
    // in the afternoon
    let c = SimClock {
        year: 2000,
        day_of_year: 14,
        time: 13.57 * 3600.0,
        ..Default::default()
    };
    let (alt, az, illum) = moon_position(&c, &berlin);
    assert!((illum - 0.5).abs() < 0.08, "illum {illum}");
    assert!(
        (90.0..200.0).contains(&az) && alt > 0.0,
        "alt {alt} az {az}"
    );
}

#[test]
fn envir_brightness_is_light_a_plus_the_light_map_under_the_lamps() {
    // a December midnight: no light A, so only the light map's light counts
    let night = Daylight::compute(
        &SimClock {
            day_of_year: 355,
            time: 0.0,
            ..Default::default()
        },
        None,
    );
    assert!(night.lamps_on);
    assert_eq!(night.envir_brightness(None), 0.0);
    let lit = night.envir_brightness(Some(Vec3::new(0.6, 0.5, 0.4)));
    assert!((lit - 0.5).abs() < 1e-5, "{lit}");
    // noon: light A alone is (nearly) 1, and the light map is off by day
    let noon = Daylight::compute(&clock_at(12.0), None);
    assert!(noon.envir_brightness(None) > 0.95);
    assert_eq!(
        noon.envir_brightness(Some(Vec3::ONE)),
        noon.envir_brightness(None)
    );
}

#[test]
fn stock_envir_leaves_the_enhanced_sky_untinted() {
    let own = default_envir_values();
    for h in [3.0, 6.0, 12.0, 19.0, 23.0] {
        assert_eq!(
            Daylight::compute(&clock_at(h), None).envir_tint,
            [Vec3::ONE; 3],
            "{h} h"
        );
        assert_eq!(
            Daylight::compute(&clock_at(h), Some(&own)).envir_tint,
            [Vec3::ONE; 3],
            "{h} h"
        );
    }
}

#[test]
fn a_warmer_envir_tints_the_sun() {
    let mut warm = default_envir_values();
    // the midday sun as warm as a late afternoon's
    warm.light_color_a[4] = [255.0, 230.0, 170.0];
    warm.light_color_a[3] = [255.0, 230.0, 160.0];
    let d = Daylight::compute(&clock_at(12.0), Some(&warm));
    let a = d.envir_tint[0];
    assert!((a.x - 1.0).abs() < 1e-3 && a.y < 0.95 && a.z < 0.7, "{a:?}");
    assert_eq!(d.envir_tint[1], Vec3::ONE);
    // by night the stock sun is black: nothing to compare with, no tint
    assert_eq!(
        Daylight::compute(&clock_at(1.0), Some(&warm)).envir_tint[0],
        Vec3::ONE
    );
}

//! The weather choice's launcher side (`omsi_launcher_lib::weather`): a weather typed by hand
//! (`custom:`), the physical model's choice and the airports of the METAR list.

use omsi_launcher_lib::weather::*;

#[test]
fn custom_weather_reads_back_what_it_wrote() {
    let w = CustomWeather {
        visibility_m: 1200.0,
        wind_dir: 270.0,
        wind_speed: 6.5,
        temp_c: -3.0,
        humidity: 88.0,
        cloud: 4,
        precip: 2,
        snow_cover: true,
        ..CustomWeather::default()
    };
    let text = w.encode();
    assert!(text.starts_with("custom:"));
    assert_eq!(custom_weather(Some(&text)), Some(w));
}

#[test]
fn custom_weather_is_kept_within_its_bounds() {
    let mut w = CustomWeather {
        visibility_m: 10.0,
        wind_dir: 400.0,
        cloud: 99,
        precip: 7,
        ..CustomWeather::default()
    };
    w.normalize();
    assert_eq!(w.visibility_m, 50.0);
    assert_eq!(w.wind_dir, 40.0);
    assert_eq!(w.cloud, CUSTOM_CLOUDS.len() - 1);
    assert_eq!(w.precip, CUSTOM_PRECIP.len() as i32 - 1);
}

#[test]
fn only_a_custom_choice_is_a_custom_weather() {
    assert_eq!(custom_weather(None), None);
    assert_eq!(custom_weather(Some("natural")), None);
    assert_eq!(custom_weather(Some("Weather/fog.owt")), None);
    assert_eq!(
        custom_weather(Some("CUSTOM:t=20")).map(|w| w.temp_c),
        Some(20.0)
    );
}

#[test]
fn natural_is_the_model_and_so_is_no_choice() {
    assert!(is_natural(None));
    assert!(is_natural(Some(" Natural ")));
    assert!(!is_natural(Some("custom:t=20")));
    assert!(!is_natural(Some("metar:EDDB")));
}

#[test]
fn the_metar_list_always_offers_berlin_brandenburg() {
    let dir = std::env::temp_dir().join(format!("depot-weather-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("Weather")).unwrap();
    std::fs::write(
        dir.join("Weather/ICAO.txt"),
        "lowg - Graz\nEDDH - Hamburg\nnot a line\nED1 - too short\nEDDH - again\n",
    )
    .unwrap();
    let list = metar_airports(&dir);
    let codes: Vec<&str> = list.iter().map(|a| a.0.as_str()).collect();
    assert_eq!(codes, ["EDDB", "EDDH", "LOWG"]);
    assert_eq!(list[2].1, "lowg - Graz");
    let _ = std::fs::remove_dir_all(&dir);
}

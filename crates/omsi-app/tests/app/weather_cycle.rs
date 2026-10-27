use omsi_content::weather::Weather;
use openomsi_game::testing::weather_cycle::*;

fn w(name: &str, cover: f32, rain: f32, fog: f32, temp: f32, snow: bool) -> Weather {
    Weather {
        name: name.into(),
        fog: (fog, 0.0),
        temp: (temp, 0.0),
        clouds: (
            if cover > 0.5 {
                "Overcast 1".into()
            } else if cover > 0.2 {
                "Cumulus 2".into()
            } else {
                "-1".into()
            },
            300.0,
        ),
        precip: vec![rain, 0.0, 0.0, 0.0, 0.0],
        snow,
        ..Default::default()
    }
}

#[test]
fn a_blend_passes_through_the_middle_and_switches_the_clouds_halfway() {
    let (a, b) = (
        w("clear", 0.1, 0.0, 50000.0, 20.0, false),
        w("rain", 0.9, 1.0, 2000.0, 12.0, false),
    );
    let mut bl = Blend::new(a, b, 100.0);
    let (x, changed, done) = bl.step(25.0);
    assert!(!changed && !done && (x.temp.0 - 18.0).abs() < 1e-3 && x.clouds.0 == "-1");
    let (x, changed, _) = bl.step(30.0);
    assert!(changed && x.clouds.0 == "Overcast 1" && x.fog.0 < 50000.0 && x.fog.0 > 2000.0);
    let (x, _, done) = bl.step(60.0);
    assert!(done && x.name == "rain" && (x.precip[0] - 1.0).abs() < 1e-6);
}

#[test]
fn the_next_weather_suits_the_month_and_is_near() {
    let all = vec![
        (
            "Weather/a.owt".to_string(),
            w("clear", 0.1, 0.0, 50000.0, 20.0, false),
        ),
        (
            "Weather/b.owt".to_string(),
            w("cloudy", 0.4, 0.0, 30000.0, 18.0, false),
        ),
        (
            "Weather/c.owt".to_string(),
            w("snow", 1.0, 1.0, 800.0, -5.0, true),
        ),
    ];
    // in July there is no snow; from clear, cloudy is the only near one left
    for r in [0.0, 0.3, 0.7, 0.99] {
        assert_eq!(
            pick(&all, &all[0].1, "Weather/a.owt", 7, r).as_deref(),
            Some("Weather/b.owt")
        );
    }
    // in January snow may come
    assert!((0..20)
        .map(|i| pick(&all, &all[1].1, "Weather/b.owt", 1, i as f32 / 20.0))
        .any(|p| p.as_deref() == Some("Weather/c.owt")));
}
#[test]
fn the_clouds_drift_smoothly_while_the_wind_blends() {
    let mut a = w("calm", 0.4, 0.0, 50000.0, 20.0, false);
    a.wind = (0.0, 1.0);
    let mut b = w("storm", 0.4, 0.0, 50000.0, 20.0, false);
    b.wind = (90.0, 20.0);
    let mut bl = Blend::new(a, b, 240.0);
    // late in the year, where the old time-based drift jumped by whole tiles a frame
    let mut d = openomsi_game::testing::weather_setup::cloud_drift_at(&bl.from, 3.0e7);
    for _ in 0..600 {
        let (x, _, _) = bl.step(1.0 / 2.0);
        let before = d;
        openomsi_game::testing::weather_setup::cloud_drift_step(&mut d, &x, 1.0 / 2.0);
        for i in 0..2 {
            let step = (d[i] - before[i]).abs();
            assert!(step.min(1.0 - step) < 0.01, "jump {step}");
        }
    }
}

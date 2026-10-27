mod report;

use openomsi_game::testing::weather_model::*;

fn place() -> omsi_sim::daylight::SunPlace {
    omsi_sim::daylight::SunPlace::default()
}

fn run(year: i32, doy: i32, days: i32, mut f: impl FnMut(&WeatherModel, f64)) {
    let clock = omsi_sim::SimClock {
        year,
        day_of_year: doy,
        time: 0.0,
        ..Default::default()
    };
    let mut m = WeatherModel::new(&clock, &place());
    for k in 0..(days * 24 * 2) {
        m.step(year, &place(), 0.5);
        f(&m, k as f64 * 0.5);
    }
}

#[test]
fn days_warm_up_and_nights_cool_down() {
    // over a summer month: afternoons warmer than the mornings by several degrees on
    // average, and the temperature within what a Berlin summer has
    let (mut morning, mut afternoon, mut n) = (0.0, 0.0, 0);
    run(2026, 180, 30, |m, h| {
        let hod = (h + 72.0).rem_euclid(24.0);
        assert!((-2.0..40.0).contains(&m.temp), "{} at {h}", m.temp);
        if (5.5..6.0).contains(&hod) {
            morning += m.temp;
            n += 1;
        }
        if (15.0..15.5).contains(&hod) {
            afternoon += m.temp;
        }
    });
    let range = (afternoon - morning) / n as f32;
    assert!((4.0..16.0).contains(&range), "diurnal range {range}");
}

#[test]
fn the_year_has_grey_days_rain_fog_and_clear_evenings() {
    let mut grey_winter = 0;
    let mut grey_summer = 0;
    let mut rain_hours = 0;
    let mut fog_mornings = 0;
    let mut cumulus_afternoons = 0;
    let mut clear_evenings = 0;
    run(2026, 1, 45, |m, h| {
        let hod = (h + 72.0).rem_euclid(24.0);
        if (13.0..13.5).contains(&hod) && m.deck > 0.85 {
            grey_winter += 1;
        }
        if m.precip > 0.1 {
            rain_hours += 1;
        }
    });
    run(2026, 170, 45, |m, h| {
        let hod = (h + 72.0).rem_euclid(24.0);
        if (13.0..13.5).contains(&hod) && m.deck > 0.85 {
            grey_summer += 1;
        }
        if (15.0..15.5).contains(&hod) && m.cumulus > 0.25 && m.deck < 0.5 {
            cumulus_afternoons += 1;
        }
        if (20.0..20.5).contains(&hod) && m.cumulus < 0.15 && m.deck < 0.3 {
            clear_evenings += 1;
        }
    });
    run(2026, 280, 45, |m, h| {
        let hod = (h + 72.0).rem_euclid(24.0);
        if (6.5..7.0).contains(&hod) && m.fog > 0.4 {
            fog_mornings += 1;
        }
    });
    assert!(
        grey_winter > grey_summer + 5,
        "grey days: winter {grey_winter}, summer {grey_summer}"
    );
    assert!(
        (15..300).contains(&rain_hours),
        "rain {rain_hours} half-hours in 45 winter days"
    );
    assert!(
        cumulus_afternoons >= 8,
        "cumulus afternoons {cumulus_afternoons}"
    );
    assert!(clear_evenings >= 5, "clear evenings {clear_evenings}");
    assert!(fog_mornings >= 1, "foggy autumn mornings {fog_mornings}");
}

#[test]
fn rain_washes_the_air_and_a_high_fills_it_with_haze() {
    // the haze right after rain against the haze before it
    let mut after_rain = Vec::new();
    let mut dry_long = Vec::new();
    let mut since = 1000.0;
    run(2026, 120, 90, |m, _| {
        if m.precip > 0.2 {
            since = 0.0;
        } else {
            since += 0.5;
        }
        if (6.0..12.0).contains(&since) {
            after_rain.push(m.aerosol);
        }
        if since > 96.0 {
            dry_long.push(m.aerosol);
        }
    });
    let avg = |v: &Vec<f32>| v.iter().sum::<f32>() / v.len().max(1) as f32;
    assert!(!after_rain.is_empty() && !dry_long.is_empty());
    assert!(
        avg(&dry_long) > avg(&after_rain) * 1.3,
        "{} vs {}",
        avg(&dry_long),
        avg(&after_rain)
    );
}

#[test]
fn the_state_changes_smoothly() {
    let mut last: Option<WeatherModel> = None;
    run(2026, 200, 10, |m, _| {
        if let Some(l) = &last {
            assert!((m.temp - l.temp).abs() < 2.5, "{} -> {}", l.temp, m.temp);
            assert!((m.deck - l.deck).abs() < 0.4 && (m.cumulus - l.cumulus).abs() < 0.5);
        }
        last = Some(m.clone());
    });
}

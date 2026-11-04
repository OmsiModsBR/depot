use omsi_content::weather::psychro::dew_point_q as dew_point;
use openomsi_game::testing::weather_model::*;

/// `cargo test -p omsi-app weather_report -- --ignored --nocapture`: day by day.
#[test]
#[ignore]
fn weather_report() {
    let place = omsi_sim::daylight::SunPlace::default();
    for (doy0, label) in [
        (1, "winter"),
        (100, "spring"),
        (180, "summer"),
        (280, "autumn"),
    ] {
        println!("== {label}");
        let clock = omsi_sim::SimClock {
            year: 2026,
            day_of_year: doy0,
            time: 0.0,
            ..Default::default()
        };
        let mut m = WeatherModel::new(&clock, &place);
        let mut stats = [0.0f32; 6];
        let mut snow_days = 0;
        for day in 0..30 {
            let mut line = format!("d{day:2}");
            for hour in 0..24 {
                m.step(2026, &place, 1.0);
                if hour == 14 {
                    stats[3] = m.temp;
                    stats[4] = m.mixed;
                    stats[5] = 125.0 * (m.temp - dew_point(m.q)).max(0.0);
                }
                if hour % 3 == 2 {
                    let c = if m.precip > 0.1 && m.temp < 1.5 {
                        '*'
                    } else if m.precip > 0.1 {
                        'R'
                    } else if m.fog > 0.4 {
                        'F'
                    } else if m.deck > 0.85 {
                        '#'
                    } else if m.deck > 0.5 {
                        '='
                    } else if m.cumulus > 0.45 {
                        'C'
                    } else if m.cumulus > 0.15 {
                        'c'
                    } else if m.veil > 0.5 {
                        '~'
                    } else {
                        '.'
                    };
                    line.push(c);
                }
                stats[0] += (m.precip > 0.1) as i32 as f32;
                stats[1] += (m.deck > 0.85) as i32 as f32;
                stats[2] += (m.fog > 0.4) as i32 as f32;
                stats[3 + 0] += 0.0;
                if hour == 12 && m.snow_cover > 0.5 {
                    snow_days += 1;
                }
            }
            let w = m.weather();
            println!("{line}  14h T {:4.1} mix {:4.0} lcl {:4.0} | 23h T {:5.1} rh {:3.0}% vis {:6.0} haze {:.2} mix {:4.0} rh_up {:.2} p {:6.1}", stats[3], stats[4], stats[5], m.temp, m.humidity() * 100.0, w.fog.0, m.sky().haze, m.mixed, m.rh_up, w.pressure);
            let _ = &w;
        }
        println!(
            "rain {:.0}% grey {:.0}% fog {:.0}% snow-covered days {snow_days}",
            stats[0] / 7.2,
            stats[1] / 7.2,
            stats[2] / 7.2
        );
    }
}

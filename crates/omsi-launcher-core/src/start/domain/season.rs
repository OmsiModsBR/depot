//! The seasons of a start: the weather that fits a season.

use super::super::interface::output::WeatherInfo;

/// Weather presets that make sense in the season (no summer shower in the snow).
pub fn weather_fits(season: &str, w: &WeatherInfo) -> bool {
    let rain = w.precip.starts_with("rain");
    match season {
        "winter" => w.snow || if rain { w.temp <= 10.0 } else { w.temp <= 16.0 },
        s => {
            if w.snow || w.temp <= 0.0 {
                return false;
            }
            s != "summer" || w.temp >= 8.0
        }
    }
}

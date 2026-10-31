//! The seasons of a start: the month a season moves the date into, and the weather that fits
//! a season.

use super::super::interface::output::WeatherInfo;

/// The month (`MM`) a start moves into when its season is chosen: none for "auto".
pub fn season_month(season: &str) -> Option<&'static str> {
    match season {
        "spring" => Some("04"),
        "summer" => Some("07"),
        "autumn" => Some("10"),
        "winter" => Some("01"),
        _ => None,
    }
}

/// `date` (`YYYY-MM-DD`) moved into `month` (`04`, `07`, `10` or `01`, a season's), the day
/// kept within the month: 31 March into spring is 30 April.
pub fn date_in_month(date: &str, month: &str) -> String {
    let yy = date.get(0..4).unwrap_or("1989");
    let last = if month == "04" { 30 } else { 31 };
    let dd = date
        .get(8..10)
        .and_then(|d| d.parse::<u32>().ok())
        .unwrap_or(15)
        .clamp(1, last);
    format!("{yy}-{month}-{dd:02}")
}

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

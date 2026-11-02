//! A weather typed by hand on the Drive page (`custom:` choice), its fields and its text.

use super::super::interface::output::{absolute_humidity, relative_humidity, Weather};

pub const CUSTOM_CLOUDS: [&str; 5] = [
    "No clouds",
    "Cumulus 1",
    "Cumulus 2",
    "Cumulus 3",
    "Overcast 1",
];
pub const CUSTOM_PRECIP: [&str; 3] = ["None", "Rain", "Snow"];

#[derive(Debug, Clone, PartialEq)]
pub struct CustomWeather {
    pub visibility_m: f32,
    pub brightness: f32,
    pub wind_dir: f32,
    pub wind_speed: f32,
    pub temp_c: f32,
    pub humidity: f32,
    pub pressure: f32,
    pub cloud: usize,
    pub cloud_base_m: f32,
    pub precip: i32,
    pub precip_intensity: f32,
    pub road_wetness: f32,
    pub snow_cover: bool,
    pub snow_on_road: bool,
}
impl Default for CustomWeather {
    fn default() -> Self {
        Self {
            visibility_m: 50_000.0,
            brightness: 1.0,
            wind_dir: 0.0,
            wind_speed: 0.0,
            temp_c: 15.0,
            humidity: 51.0,
            pressure: 1013.0,
            cloud: 0,
            cloud_base_m: 50.0,
            precip: 0,
            precip_intensity: 32.0,
            road_wetness: 0.0,
            snow_cover: false,
            snow_on_road: false,
        }
    }
}
impl CustomWeather {
    pub fn normalize(&mut self) {
        self.visibility_m = self.visibility_m.clamp(50.0, 50_000.0);
        self.brightness = self.brightness.clamp(0.0, 1.5);
        self.wind_dir = self.wind_dir.rem_euclid(360.0);
        self.wind_speed = self.wind_speed.clamp(0.0, 50.0);
        self.temp_c = self.temp_c.clamp(-40.0, 50.0);
        self.humidity = self.humidity.clamp(0.0, 100.0);
        self.pressure = self.pressure.clamp(900.0, 1100.0);
        self.cloud = self.cloud.min(CUSTOM_CLOUDS.len() - 1);
        self.cloud_base_m = self.cloud_base_m.clamp(50.0, 5000.0);
        self.precip = self.precip.clamp(0, 2);
        self.precip_intensity = self.precip_intensity.clamp(0.0, 255.0);
        self.road_wetness = self.road_wetness.clamp(0.0, 1.0);
    }
    pub fn parse(text: &str) -> Option<Self> {
        let body = text
            .strip_prefix("custom:")
            .or_else(|| text.strip_prefix("CUSTOM:"))?;
        let mut c = Self::default();
        for part in body.split(';') {
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };
            let n = value.trim().parse::<f32>().ok();
            match key.trim().to_ascii_lowercase().as_str() {
                "vis" => {
                    if let Some(v) = n {
                        c.visibility_m = v
                    }
                }
                "br" => {
                    if let Some(v) = n {
                        c.brightness = v
                    }
                }
                "wd" => {
                    if let Some(v) = n {
                        c.wind_dir = v
                    }
                }
                "ws" => {
                    if let Some(v) = n {
                        c.wind_speed = v
                    }
                }
                "t" => {
                    if let Some(v) = n {
                        c.temp_c = v
                    }
                }
                "rh" => {
                    if let Some(v) = n {
                        c.humidity = v
                    }
                }
                "p" => {
                    if let Some(v) = n {
                        c.pressure = v
                    }
                }
                "c" => {
                    if let Some(v) = n {
                        c.cloud = v.round().max(0.0) as usize
                    }
                }
                "cb" => {
                    if let Some(v) = n {
                        c.cloud_base_m = v
                    }
                }
                "pt" => {
                    if let Some(v) = n {
                        c.precip = v.round() as i32
                    }
                }
                "pi" => {
                    if let Some(v) = n {
                        c.precip_intensity = v
                    }
                }
                "wet" => {
                    if let Some(v) = n {
                        c.road_wetness = v
                    }
                }
                "snow" => {
                    if let Some(v) = n {
                        c.snow_cover = v >= 0.5
                    }
                }
                "snowroad" => {
                    if let Some(v) = n {
                        c.snow_on_road = v >= 0.5
                    }
                }
                _ => {}
            }
        }
        c.normalize();
        Some(c)
    }
    pub fn encode(&self) -> String {
        let mut c = self.clone();
        c.normalize();
        format!("custom:vis={:.0};br={:.2};wd={:.0};ws={:.1};t={:.1};rh={:.0};p={:.0};c={};cb={:.0};pt={};pi={:.0};wet={:.2};snow={};snowroad={}",
            c.visibility_m,c.brightness,c.wind_dir,c.wind_speed,c.temp_c,c.humidity,c.pressure,c.cloud,c.cloud_base_m,
            c.precip,c.precip_intensity,c.road_wetness,c.snow_cover as u8,c.snow_on_road as u8)
    }
    pub fn from_weather(w: &Weather, brightness: f32, wetness: f32) -> Self {
        let kind = w.clouds.0.trim().to_ascii_lowercase();
        let cloud = if kind.starts_with("cumulus 1") {
            1
        } else if kind.starts_with("cumulus 2") {
            2
        } else if kind.starts_with("cumulus 3") {
            3
        } else if kind.starts_with("overcast") {
            4
        } else {
            0
        };
        let mut c = Self {
            visibility_m: w.fog.0,
            brightness,
            wind_dir: w.wind.0,
            wind_speed: w.wind.1,
            temp_c: w.temp.0,
            humidity: relative_humidity(w.temp.0, w.temp.1),
            pressure: if w.pressure > 0.0 { w.pressure } else { 1013.0 },
            cloud,
            cloud_base_m: w.clouds.1.max(50.0),
            precip: w.precip.first().copied().unwrap_or(0.0).round() as i32,
            precip_intensity: w.precip.get(1).copied().unwrap_or(32.0),
            road_wetness: wetness,
            snow_cover: w.snow,
            snow_on_road: w.snow_on_road,
        };
        c.normalize();
        c
    }
    pub fn to_weather(&self) -> Weather {
        let mut c = self.clone();
        c.normalize();
        let cloud = match c.cloud {
            1 => "Cumulus 1",
            2 => "Cumulus 2",
            3 => "Cumulus 3",
            4 => "Overcast 1",
            _ => "-1",
        };
        Weather {
            path: std::path::PathBuf::from(c.encode()),
            name: "Custom weather".into(),
            description: "User-defined weather".into(),
            fog: (c.visibility_m, 1.0),
            wind: (c.wind_dir, c.wind_speed),
            temp: (c.temp_c, absolute_humidity(c.temp_c, c.humidity)),
            pressure: c.pressure,
            clouds: (cloud.into(), c.cloud_base_m),
            precip: vec![c.precip as f32, c.precip_intensity, 0.0, 0.0, 0.0],
            ground_wet: [c.road_wetness * 255.0, 0.0, 0.0],
            snow: c.snow_cover,
            snow_on_road: c.snow_on_road,
        }
    }
}
pub fn custom_weather(text: Option<&str>) -> Option<CustomWeather> {
    text.and_then(CustomWeather::parse)
}

/// Whether a weather choice means the game's own physical model: `natural`, or none at all.
pub fn is_natural(choice: Option<&str>) -> bool {
    choice.is_none_or(|w| w.trim().eq_ignore_ascii_case("natural"))
}

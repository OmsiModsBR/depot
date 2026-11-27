//! The start menu's state: the choices (map, vehicle, time, traffic, passengers,
//! schedule, situation, weather, day), the lines it draws and the keys that move
//! through them. Reading the folders is in `implementation/menu.rs`.

//! Start menu: map, vehicle, time, traffic, passengers, schedule - keyboard driven,
//! drawn as HUD text before the world is loaded.

pub struct Menu {
    pub maps: Vec<(String, String)>,
    pub vehicles: Vec<(String, String)>,
    /// (manufacturer, type) of each of `vehicles`, as OMSI's [friendlyname] gives them.
    pub vehicle_meta: Vec<(String, String)>,
    pub map: usize,
    pub vehicle: usize,
    pub hour: i32,
    pub traffic: usize,
    pub passengers: bool,
    pub schedule: bool,
    pub row: usize,
    pub start: bool,
    /// Situations (name, file); index 0 = none.
    pub situations: Vec<(String, String)>,
    pub situation: usize,
    /// Weather files (name, path); index 0 = the map's default.
    pub weathers: Vec<(String, String)>,
    pub weather: usize,
    /// Day of the year, so winter can be chosen without the command line.
    pub day: i32,
}

impl Menu {
    pub fn lines(&self) -> Vec<String> {
        let mark = |r: usize| if self.row == r { ">" } else { " " };
        vec![
            "Project Depot".to_string(),
            String::new(),
            format!(
                "{} Map:        {}",
                mark(0),
                self.maps.get(self.map).map(|m| m.0.as_str()).unwrap_or("-")
            ),
            format!(
                "{} Vehicle:    {}",
                mark(1),
                self.vehicles
                    .get(self.vehicle)
                    .map(|v| v.0.as_str())
                    .unwrap_or("-")
            ),
            format!("{} Time:       {:02}:00", mark(2), self.hour),
            format!("{} Traffic:    {} vehicles", mark(3), self.traffic),
            format!(
                "{} Passengers: {}",
                mark(4),
                if self.passengers { "on" } else { "off" }
            ),
            format!(
                "{} Timetable:  {}",
                mark(5),
                if self.schedule { "on" } else { "off" }
            ),
            format!(
                "{} Weather:    {}",
                mark(6),
                self.weathers
                    .get(self.weather)
                    .map(|w| w.0.as_str())
                    .unwrap_or("-")
            ),
            format!(
                "{} Date:       {}  ({})",
                mark(7),
                self.date_text(),
                self.season_text()
            ),
            format!(
                "{} Situation:  {}",
                mark(8),
                self.situations
                    .get(self.situation)
                    .map(|s| s.0.as_str())
                    .unwrap_or("-")
            ),
            String::new(),
            "Arrow keys choose, Enter starts, Esc quits".to_string(),
        ]
    }

    /// The chosen day of the year as a date in the map's default year.
    pub fn date_text(&self) -> String {
        let months = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut d = self.day.max(1);
        for (i, m) in months.iter().enumerate() {
            if d <= *m {
                return format!("{d:02}.{:02}.", i + 1);
            }
            d -= m;
        }
        "31.12.".to_string()
    }

    pub fn season_text(&self) -> &'static str {
        match self.day {
            1..=59 => "winter",
            60..=120 => "spring",
            121..=243 => "summer",
            244..=304 => "autumn",
            _ => "winter",
        }
    }

    /// Handle a key (winit key code name); returns true when consumed.
    pub fn key(&mut self, code: winit::keyboard::KeyCode) {
        use winit::keyboard::KeyCode as K;
        let dir: i32 = match code {
            K::ArrowLeft => -1,
            K::ArrowRight => 1,
            _ => 0,
        };
        match code {
            K::ArrowUp => self.row = (self.row + 8) % 9,
            K::ArrowDown => self.row = (self.row + 1) % 9,
            K::Enter | K::NumpadEnter => self.start = true,
            _ => {}
        }
        if dir != 0 {
            let step = |v: usize, n: usize| -> usize {
                if n == 0 {
                    0
                } else {
                    ((v as i32 + dir).rem_euclid(n as i32)) as usize
                }
            };
            match self.row {
                0 => self.map = step(self.map, self.maps.len()),
                1 => self.vehicle = step(self.vehicle, self.vehicles.len()),
                2 => self.hour = (self.hour + dir).rem_euclid(24),
                3 => self.traffic = (self.traffic as i32 + dir * 10).clamp(0, 200) as usize,
                4 => self.passengers = !self.passengers,
                5 => self.schedule = !self.schedule,
                6 => self.weather = step(self.weather, self.weathers.len()),
                7 => self.day = (self.day + dir * 15).rem_euclid(365).max(1),
                8 => self.situation = step(self.situation, self.situations.len()),
                _ => {}
            }
        }
    }
}

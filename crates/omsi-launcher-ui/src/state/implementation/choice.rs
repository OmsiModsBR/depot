//! The next start's choices: the bus, the map, the line and tour, the season and the trip.

use super::super::domain::state::*;
use super::super::interface::output::core;
use super::text::*;

impl State {
    // --- helpers for the pages -------------------------------------------------------------

    pub fn bus(&self) -> Option<&core::VehicleInfo> {
        self.vehicles.iter().find(|v| v.file == self.choice.bus)
    }
    pub fn map(&self) -> Option<&core::MapInfo> {
        self.maps.iter().find(|m| m.file == self.choice.map)
    }
    pub fn line(&self) -> Option<&core::LineInfo> {
        let l = self.choice.line.as_ref()?;
        self.lines.iter().find(|x| &x.name == l)
    }
    pub fn tour(&self) -> Option<&core::TourInfo> {
        let t = self.choice.tour.as_ref()?;
        self.line()?.tours.iter().find(|x| &x.number == t)
    }

    /// The depot file for the chosen bus on the chosen map (`core::start::default_hof`).
    pub fn default_hof(&self) -> String {
        core::start::default_hof(&self.config.root, &self.choice, self.map(), self.bus())
    }

    /// The bus "Use for the next start" was clicked for on Vehicles.
    pub fn use_bus(&mut self, u: crate::vehicles::UseBus) {
        self.select_bus(&u.file);
        self.choice
            .use_bus(u.file, u.paint, u.hof, u.number, u.plate);
        if !self.choice.hof_manual {
            self.choice.hof = self.default_hof();
        }
        self.touched();
    }

    /// No bus for the next start (the Start card's Clear).
    pub fn clear_bus(&mut self) {
        self.choice.clear_bus();
        self.choice.hof = self.default_hof();
        self.touched();
    }

    pub fn select_bus(&mut self, file: &str) {
        if self.choice.bus == file {
            return;
        }
        self.choice.bus = file.to_string();
        self.choice.paint.clear();
        self.choice.number.clear();
        // (a hand-picked depot file stays when the new bus has one of that name)
        let keep = self.choice.hof_manual
            && self.bus().is_some_and(|v| {
                v.hofs
                    .iter()
                    .any(|h| h.eq_ignore_ascii_case(&self.choice.hof))
            });
        if !keep {
            self.choice.hof_manual = false;
            self.choice.hof = self.default_hof();
        }
        self.touched();
    }

    pub fn select_map(&mut self, file: &str) {
        if self.choice.map == file {
            return;
        }
        self.choice.map = file.to_string();
        self.choice.entry = -1;
        self.choice.line = None;
        self.choice.tour = None;
        self.choice.hof = self.default_hof();
        self.lines.clear();
        self.load_lines();
        self.touched();
    }

    /// The season `season` ("auto": by the date) chosen for the start: the date moves into it
    /// (its timetable days with it), and a weather preset that does not fit it any more goes.
    pub fn set_season(&mut self, season: &str) {
        self.choice.season = season.to_string();
        if let Some(month) = core::start::season_month(season) {
            let date = self.choice.date.clone();
            self.choice.date = date_in_month(&date, month);
            self.load_lines();
        }
        let w = self.choice.weather.clone();
        if let Some(wi) = self.weathers.iter().find(|x| x.file == w).cloned() {
            if !self.weather_fits(&wi) {
                self.choice.weather.clear();
            }
        }
        self.touched();
    }

    /// The season the chosen date (or the override) means.
    pub fn season(&self) -> &str {
        self.choice.season()
    }

    /// Weather presets that make sense in the season (no summer shower in the snow).
    pub fn weather_fits(&self, w: &core::WeatherInfo) -> bool {
        core::start::weather_fits(self.season(), w)
    }

    /// The trip of the tour a start at the chosen time begins with: the next to leave (one
    /// that left a minute or two ago still counts), else the tour's last.
    pub fn first_trip(&self) -> Option<usize> {
        self.choice.first_trip(self.tour()?, self.real_time())
    }

    pub fn picked_trip(&self) -> Option<usize> {
        self.choice.picked_trip(self.real_time())
    }

    /// The start time follows the computer's clock (`use_real_time`).
    pub fn real_time(&self) -> bool {
        self.settings
            .get("use_real_time")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Start the tour at trip `index` (leaving at `departure`): at its departure, or at the
    /// real time, which then stays and the bus waits for the trip.
    pub fn pick_trip(&mut self, index: usize, departure: f64) {
        if self.choice.pick_trip(index, departure, self.real_time()) {
            self.touched();
        }
    }
}

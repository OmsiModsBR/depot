//! The start the launcher prepares: the choices of the Drive and Maps pages as they are
//! remembered between launches.

use serde::{Deserialize, Serialize};

/// The duty as it is remembered between launches (`~/.depot/launcher-duty.json`).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Choice {
    pub bus: String,
    pub paint: String,
    /// The number plate the player typed for the bus (empty: as the content says).
    pub plate: String,
    /// The fleet number picked from the bus's `[number]` list (empty: its first).
    pub number: String,
    pub hof: String,
    /// The depot file was chosen by hand (else it follows the map and the date).
    pub hof_manual: bool,
    pub map: String,
    /// The start: the entry point's place in the map's list, or -1 = automatic (nearest to
    /// the duty's first stop by road).
    pub entry: i32,
    pub line: Option<String>,
    pub tour: Option<String>,
    pub free: bool,
    /// Minutes of the day.
    pub time: i32,
    pub start_trip: Option<(String, String, usize, i32)>,
    pub date: String,
    /// "auto", spring, summer, autumn, winter.
    pub season: String,
    pub weather: String,
    pub traffic: f32,
    pub passengers: bool,
    pub schedule: bool,
    pub autostart: bool,
    /// Start on foot (no bus of one's own until one is placed).
    pub on_foot: bool,
    /// off, host, join
    pub lan_mode: String,
    pub lan_addr: String,
    /// 2: `entry` may be -1 (automatic); older files had a fixed entry point there.
    pub version: u32,
    /// The kind of session the Maps page starts: "new", "saved" (a situation the game saved)
    /// or "without_bus" (on foot at the start point).
    pub session: String,
}

impl Default for Choice {
    fn default() -> Self {
        Choice {
            bus: String::new(),
            paint: String::new(),
            plate: String::new(),
            number: String::new(),
            hof: String::new(),
            hof_manual: false,
            map: String::new(),
            entry: -1,
            line: None,
            tour: None,
            free: false,
            time: 9 * 60,
            start_trip: None,
            date: "1989-05-30".into(),
            season: "auto".into(),
            weather: String::new(),
            traffic: 30.0,
            passengers: true,
            schedule: true,
            autostart: false,
            on_foot: false,
            lan_mode: "off".into(),
            lan_addr: String::new(),
            version: 3,
            session: "new".into(),
        }
    }
}

impl Choice {
    /// The bus "Use for the next start" was clicked for, with what its card held: its paint,
    /// a depot file picked by hand (none: the automatic one), a fleet number and a plate.
    pub fn use_bus(
        &mut self,
        file: String,
        paint: String,
        hof: Option<String>,
        number: Option<String>,
        plate: String,
    ) {
        self.bus = file;
        self.paint = paint;
        self.hof_manual = hof.is_some();
        if let Some(hof) = hof {
            self.hof = hof;
        }
        self.number = number.unwrap_or_default();
        self.plate = plate;
    }

    /// No bus picked for the next start: the whole preselection goes, no depot file, number
    /// or plate picked for it stays behind for the game (the depot file goes back to the
    /// automatic one, which the caller sets).
    pub fn clear_bus(&mut self) {
        self.bus.clear();
        self.paint.clear();
        self.number.clear();
        self.plate.clear();
        self.hof_manual = false;
    }

    /// A duty read back from the file, as a new launcher starts with it (`mobile`: the
    /// launcher of a phone, whose Drive page still holds the bus).
    pub fn fresh(mut c: Choice, mobile: bool) -> Choice {
        // A server is joined on purpose, in the launcher's own session (Multiplayer page): a
        // join left in the file made every later start connect to it, with no sign of it on
        // the Drive page ("Leave Server" is only there for a server joined since the launch).
        if c.lan_mode == "join" {
            c.lan_mode = "off".into();
        }
        if c.version < 2 {
            // the start was always the map's first entry point: now it is automatic
            c.entry = -1;
            c.version = 2;
        }
        if c.version < 3 {
            // the Drive page always held a bus; on a computer the bus of a start is now only one
            // picked on Vehicles ("Use this bus"), so the old one is no preselection
            if !mobile {
                c.bus.clear();
                c.paint.clear();
                c.number.clear();
                c.plate.clear();
                c.hof_manual = false;
            }
            c.version = 3;
        }
        // (older launchers took a vehicle line of a broken ailists.cfg for the map's depot)
        if c.hof.to_ascii_lowercase().contains(".bus")
            || c.hof.to_ascii_lowercase().contains(".ovh")
        {
            c.hof.clear();
        }
        c
    }

    /// The trip picked to start the tour with (its index), while the line, the tour and the
    /// time are still the ones it was picked for. At the real time (`real_time`) the clock
    /// moves on and the trip picked stays: the bus waits for it.
    pub fn picked_trip(&self, real_time: bool) -> Option<usize> {
        let (line, tour, index, time) = self.start_trip.as_ref()?;
        (self.line.as_ref() == Some(line)
            && self.tour.as_ref() == Some(tour)
            && (*time == self.time || real_time))
            .then_some(*index)
    }

    /// Start the tour at trip `index` (leaving at `departure`, s of the day): at its
    /// departure, or at the real time (`real_time`), which then stays and the bus waits for
    /// the trip. False without a line and a tour.
    pub fn pick_trip(&mut self, index: usize, departure: f64, real_time: bool) -> bool {
        let (Some(line), Some(tour)) = (self.line.clone(), self.tour.clone()) else {
            return false;
        };
        if !real_time {
            self.time = (departure / 60.0).floor() as i32;
        }
        self.start_trip = Some((line, tour, index, self.time));
        true
    }

    /// The season the chosen date (or the override) means.
    pub fn season(&self) -> &str {
        if self.season != "auto" {
            return &self.season;
        }
        let m: u32 = self
            .date
            .get(5..7)
            .and_then(|x| x.parse().ok())
            .unwrap_or(5);
        match m {
            12 | 1 | 2 => "winter",
            3..=5 => "spring",
            6..=8 => "summer",
            _ => "autumn",
        }
    }
}

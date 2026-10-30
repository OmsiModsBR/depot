//! The duty a start hands the game, and the trip of the tour it begins with.

use super::super::domain::choice::Choice;
use super::super::interface::output::{Duty, TourInfo};

impl Choice {
    /// The duty for the game: `hof` is the depot file to use (a hand-picked one, or the
    /// automatic one, see `default_hof`), `trip` the trip picked to start the tour with
    /// (`Choice::picked_trip`) and `profile` the player's profile (empty: none).
    pub fn to_duty(&self, hof: String, trip: Option<usize>, profile: &str) -> Duty {
        let c = self;
        // (multiplayer is disabled: a duty never hosts or joins)
        let lan = "off".to_string();
        // Joining: the host's map, as its status gives it (a code's host is asked when the
        // code is typed). The game takes it from the host's welcome too, but only when that
        // comes before the map is loaded: through a tunnel it came later, the game started
        // on the map chosen here, and the players never met ("the host drives on X10 Berlin,
        // you on Berlin-Spandau"). A map not installed here comes with the host's mods.
        let host_map: Option<String> = None;
        Duty {
            map: host_map.unwrap_or_else(|| c.map.clone()),
            bus: c.bus.clone(),
            paint: Some(c.paint.clone()).filter(|p| !p.is_empty()),
            plate: Some(c.plate.clone()).filter(|p| !p.trim().is_empty()),
            number: Some(c.number.clone()).filter(|n| !n.trim().is_empty()),
            hof: Some(hof).filter(|p| !p.is_empty()),
            entry: Some(c.entry),
            line: if c.free { None } else { c.line.clone() },
            tour: if c.free { None } else { c.tour.clone() },
            trip: if c.free {
                None
            } else {
                trip.map(|i| i.to_string())
            },
            whole_tour: !c.free && trip.is_some(),
            time: format!("{:02}:{:02}", c.time / 60, c.time % 60),
            date: Some(c.date.clone()),
            weather: Some(c.weather.clone()).filter(|w| !w.is_empty()),
            traffic: Some(c.traffic.round() as u32),
            passengers: Some(c.passengers),
            schedule: Some(c.schedule),
            autostart: Some(c.autostart),
            on_foot: Some(c.on_foot),
            profile: Some(profile.to_string()).filter(|p| !p.is_empty()),
            lan: Some(lan),
            lan_name: None,
            season: Some(c.season.clone()).filter(|s| s != "auto"),
            tutorial: None,
            situation: None,
            monitor_at: None,
        }
    }

    /// The trip of `tour` a start at the chosen time begins with: the one picked (see
    /// `Choice::picked_trip`), else the next to leave (one that left a minute or two ago
    /// still counts), else the tour's last. Its place in the tour's trips.
    pub fn first_trip(&self, tour: &TourInfo, real_time: bool) -> Option<usize> {
        if let Some(i) = self.picked_trip(real_time) {
            if let Some(k) = tour.trips.iter().position(|x| x.index == i) {
                return Some(k);
            }
        }
        trip_index_at(tour, self.time as f64 * 60.0)
    }
}

/// The trip a tour starts with at `now` (s of the day), shared by the route preview and the
/// launch choice.
pub fn trip_index_at(tour: &TourInfo, now: f64) -> Option<usize> {
    tour.trips
        .iter()
        .position(|x| x.departure >= now - 120.0)
        .or(if tour.trips.is_empty() {
            None
        } else {
            Some(tour.trips.len() - 1)
        })
}

/// `d` as a start without a bus has it: on foot at the start point, the bus, its paint, number
/// and plate left out, and the depot file the map and the date give (not one picked for a bus).
pub fn without_bus(d: &mut Duty, default_hof: Option<String>) {
    d.bus.clear();
    d.paint = None;
    d.number = None;
    d.plate = None;
    d.hof = default_hof.filter(|h| !h.is_empty());
    d.on_foot = Some(true);
}

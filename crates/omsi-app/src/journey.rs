//! The journey's timetable (#800, #1061): the stops of the duty's trips with their planned
//! and actual arrival and departure and how far off those were, as the logbooks of the
//! virtual bus companies keep them. It is written to the content folder's `Journeys` every
//! time the bus reaches or leaves a stop, so it is there however the session ends.

use crate::career::{EARLY_DEPARTURE, LATE_ARRIVAL};
use crate::schedule::{PlannedTrip, PlayerDuty};
use omsi_ui::ingame::{ReportRow, StopState};
use std::path::PathBuf;

/// One stop of a trip in the log.
struct Row {
    trip: usize,
    stop: usize,
    name: String,
    /// Planned arrival and departure, and the actual ones (s of the duty's day).
    plan: (f64, f64),
    arrived: Option<f64>,
    left: Option<f64>,
    /// The trip begins here (no arrival to tell) / ends here (no departure).
    first: bool,
    last: bool,
    /// The driver skipped it on purpose: no mark, neither missed nor late.
    skipped: bool,
}

/// A trip's report: its title, a row per stop, and the totals as (label, value).
pub(crate) type TripLines = (String, Vec<ReportRow>, Vec<(&'static str, String)>);

impl Row {
    /// Whether the bus has been at this stop.
    fn reached(&self) -> bool {
        self.arrived.is_some() || self.left.is_some()
    }

    /// Whether the bus arrived late here, and whether it left early (as the career counts).
    fn judged(&self) -> (bool, bool) {
        let late = !self.first && self.arrived.is_some_and(|a| (a - self.plan.0).round() > LATE_ARRIVAL);
        let early = !self.last && self.left.is_some_and(|l| (l - self.plan.1).round() < EARLY_DEPARTURE);
        (late, early)
    }
}

pub(crate) struct Journey {
    /// The duty it is the log of: line, tour and where its trips begin in the tour.
    key: (String, String, usize),
    /// The lines above the trips (driver, map, bus, date; line and tour).
    head: String,
    /// Each trip's title.
    trips: Vec<String>,
    rows: Vec<Row>,
    dir: PathBuf,
    /// The file, chosen when there is first something to write.
    path: Option<PathBuf>,
}

impl Journey {
    /// The log of the duty `key` (line, tour, where its `trips` begin in the tour), nothing
    /// driven yet; `head` comes first in it.
    fn new(key: (String, String, usize), trips_planned: &[PlannedTrip], head: &str, dir: PathBuf) -> Journey {
        let mut trips = Vec::new();
        let mut rows = Vec::new();
        for (t, trip) in trips_planned.iter().enumerate() {
            let line = if trip.line.trim().is_empty() { key.0.trim() } else { trip.line.trim() };
            trips.push(format!("Trip {}  ·  {} to {}  ·  {}", key.2 + t + 1, line, trip.terminus.trim(), hms(trip.departure)));
            let n = trip.stops.len();
            // (the stations a depot run passes are not stops)
            for (k, s) in trip.stops.iter().enumerate().filter(|(_, s)| s.stops) {
                rows.push(Row { trip: t, stop: k, name: s.name.trim().to_string(), plan: (s.arr, s.dep), arrived: None, left: None, first: k == 0, last: k + 1 == n, skipped: false });
            }
        }
        let head = format!("{}\nLine {}, tour {}", head.trim_end(), key.0.trim(), key.1.trim());
        Journey { key, head, trips, rows, dir, path: None }
    }

    /// The bus arrived at stop `at` (trip, stop) `late` s after its time (negative: early).
    /// True when that is news.
    fn arrived(&mut self, at: (usize, usize), late: f64) -> bool {
        match self.row(at) {
            Some(r) if r.arrived.is_none() => {
                r.arrived = Some(r.plan.0 + late);
                // (skipped, then served after all - the IBIS sent back: served)
                r.skipped = false;
                true
            }
            _ => false,
        }
    }

    /// The driver skipped stop `at` (trip, stop). True when that is news.
    fn skipped(&mut self, at: (usize, usize)) -> bool {
        match self.row(at) {
            Some(r) if !r.skipped && !r.reached() => {
                r.skipped = true;
                true
            }
            _ => false,
        }
    }

    /// How stop `at` (trip, stop) went, in a few words for the timetable window ("served, +1
    /// min", "skipped"); None while the bus has not been there.
    pub(crate) fn how(&self, at: (usize, usize)) -> Option<String> {
        let r = self.rows.iter().find(|r| (r.trip, r.stop) == at)?;
        if r.skipped && !r.reached() {
            return Some("skipped".into());
        }
        // (the first stop is about its departure, every other about its arrival)
        let (real, plan) = if r.first { (r.left?, r.plan.1) } else { (r.arrived.or(r.left)?, r.plan.0) };
        let m = ((real - plan) / 60.0).round() as i64;
        Some(match m {
            0 => "served, on time".into(),
            m if m > 0 => format!("served, +{m} min"),
            m => format!("served, {m} min"),
        })
    }

    /// The bus left stop `at`, having arrived and left `late` s after its times.
    fn left(&mut self, at: (usize, usize), late: (f64, f64)) -> bool {
        let Some(r) = self.row(at) else { return false };
        r.arrived.get_or_insert(r.plan.0 + late.0);
        r.left = Some(r.plan.1 + late.1);
        r.skipped = false;
        true
    }

    /// What the end-of-trip report shows of trip `t` (its place in the duty): its title, a
    /// row per stop and the totals, as the log counts them.
    pub(crate) fn report(&self, t: usize) -> Option<TripLines> {
        let title = self.trips.get(t)?.clone();
        let rows: Vec<&Row> = self.rows.iter().filter(|r| r.trip == t).collect();
        // (a duty taken up or resumed in the middle of the trip: the stops before are not the
        // driver's)
        let from = rows.iter().position(|r| r.reached()).unwrap_or(rows.len());
        let (mut served, mut late, mut early) = (0, 0, 0);
        let skipped = rows.iter().skip(from).filter(|r| r.skipped && !r.reached()).count();
        let mut out = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            let (is_late, is_early) = r.judged();
            // (the first stop is about its departure, and a stop left early; every other
            // about its arrival)
            let by_departure = r.first || (is_early && !is_late);
            let [plan, real, diff] = if by_departure { times(r.plan.1, r.left) } else { times(r.plan.0, r.arrived) };
            let state = if r.reached() {
                served += 1;
                late += is_late as usize;
                early += is_early as usize;
                match (is_late, is_early) {
                    (true, true) => StopState::LateLeftEarly,
                    (true, false) => StopState::Late,
                    (false, true) => StopState::LeftEarly,
                    _ => StopState::OnTime,
                }
            } else if r.skipped {
                StopState::Skipped
            } else if i > from && rows[i + 1..].iter().any(|x| x.reached()) {
                StopState::Missed
            } else {
                StopState::None
            };
            out.push(ReportRow { stop: r.name.clone(), scheduled: plan, actual: real, deviation: diff, state });
        }
        let totals = vec![
            // (a stop skipped on purpose is not one the trip had to serve)
            ("Stops served", format!("{served} of {}", rows.len() - from - skipped)),
            ("Late arrivals", late.to_string()),
            ("Early departures", early.to_string()),
        ];
        Some((title, out, totals))
    }

    fn row(&mut self, (trip, stop): (usize, usize)) -> Option<&mut Row> {
        self.rows.iter_mut().find(|r| r.trip == trip && r.stop == stop)
    }

    /// The log as it is written.
    fn text(&self) -> String {
        let rule = "-".repeat(108);
        let mut out = format!("Project Depot journey log\n{}\n", self.head);
        let (mut served, mut late, mut early) = (0, 0, 0);
        for (t, title) in self.trips.iter().enumerate() {
            let rows: Vec<&Row> = self.rows.iter().filter(|r| r.trip == t).collect();
            // (a trip is in it once the bus has reached one of its stops: a tour has dozens)
            let Some(from) = rows.iter().position(|r| r.reached()) else {
                continue;
            };
            out.push_str(&format!("\n{title}\n{rule}\n{:<30}  {:>9} {:>9} {:>10}   {:>9} {:>9} {:>10}   Status\n{rule}\n", "Stop", "Arr. plan", "Arr. real", "Diff", "Dep. plan", "Dep. real", "Diff"));
            for (i, r) in rows.iter().enumerate() {
                let arr = if r.first { [String::new(), String::new(), String::new()] } else { times(r.plan.0, r.arrived) };
                let dep = if r.last { [String::new(), String::new(), String::new()] } else { times(r.plan.1, r.left) };
                let (is_late, is_early) = r.judged();
                let status = if r.reached() {
                    served += 1;
                    late += is_late as usize;
                    early += is_early as usize;
                    match (is_late, is_early) {
                        (true, true) => "late, left early",
                        (true, false) => "late",
                        (false, true) => "left early",
                        _ => "OK",
                    }
                } else if r.skipped {
                    "skipped"
                } else if i > from && rows[i + 1..].iter().any(|x| x.reached()) {
                    "missed"
                } else {
                    ""
                };
                let name: String = r.name.chars().take(30).collect();
                out.push_str(format!("{name:<30}  {:>9} {:>9} {:>10}   {:>9} {:>9} {:>10}   {status}", arr[0], arr[1], arr[2], dep[0], dep[1], dep[2]).trim_end());
                out.push('\n');
            }
        }
        out.push_str(&format!("{rule}\nStops served: {served}, late: {late}, left early: {early}\n"));
        out
    }

    /// Write the log (its file is named by the real date and time it began, and the line and tour).
    fn write(&mut self) {
        if self.path.is_none() {
            let (y, mo, d, h, mi) = omsi_launcher_lib::local_now().unwrap_or((1970, 1, 1, 0, 0));
            let clean = |s: &str| s.trim().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect::<String>();
            let stem = format!("{y:04}-{mo:02}-{d:02} {h:02}-{mi:02} line {} tour {}", clean(&self.key.0), clean(&self.key.1));
            if let Err(e) = std::fs::create_dir_all(&self.dir) {
                log::warn!("journey log: {}: {e}", self.dir.display());
                return;
            }
            // (another session begun in the same minute keeps its own)
            let path = (1..100).map(|n| self.dir.join(if n == 1 { format!("{stem}.txt") } else { format!("{stem} ({n}).txt") })).find(|p| !p.exists());
            let Some(path) = path else { return };
            log::info!("journey log: {}", path.display());
            self.path = Some(path);
        }
        if let Some(p) = self.path.as_ref() {
            if let Err(e) = std::fs::write(p, self.text()) {
                log::warn!("journey log: {}: {e}", p.display());
            }
        }
    }
}

/// Keep `slot` the log of `duty` and note in it what the duty's update did, written at once
/// when that changed it: the bus left the stop it was due at before the update, `due`,
/// `served` seconds late (arrival, departure: what `PlayerDuty::update` returned), and
/// arrived at the one it is due at now (`PlayerDuty::arrived`). `head` (driver, map, bus,
/// date) is asked for when a log begins.
pub(crate) fn note(slot: &mut Option<Journey>, duty: &PlayerDuty, due: (usize, usize), served: Option<(f64, f64)>, root: &std::path::Path, head: impl FnOnce() -> String) {
    // (every frame: the key is compared, not made)
    if slot.as_ref().is_none_or(|j| (j.key.0.as_str(), j.key.1.as_str(), j.key.2) != (duty.line.as_str(), duty.tour.as_str(), duty.first_trip)) {
        let dir = crate::startup::content_dir().unwrap_or_else(|| root.to_path_buf()).join("Journeys");
        *slot = Some(Journey::new((duty.line.clone(), duty.tour.clone(), duty.first_trip), &duty.trips, &head(), dir));
    }
    let Some(j) = slot.as_mut() else { return };
    let left = served.is_some_and(|s| j.left(due, s));
    let arrived = duty.arrived().is_some_and(|late| j.arrived((duty.trip_index, duty.next_stop), late));
    let mut skipped = false;
    for at in duty.skipped_stops() {
        skipped |= j.skipped(*at);
    }
    if left || arrived || skipped {
        j.write();
    }
}

/// The log's first line: the driver, the map, the bus and the game's date.
pub(crate) fn head(career: &crate::career::Career, map: &str, bus: &omsi_sim::VehicleInstance, clock: &omsi_sim::SimClock) -> String {
    let driver = career.driver.as_ref().map(|d| d.name.clone()).or_else(|| career.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned()));
    let (day, month) = clock.day_month();
    let bus = format!("{} {}", bus.ty.def.manufacturer.trim(), bus.ty.def.type_name.trim());
    let mut parts: Vec<String> = driver.map(|d| format!("Driver {}", d.trim())).into_iter().collect();
    parts.extend([map.trim().to_string(), bus.trim().to_string(), format!("{:04}-{month:02}-{day:02}", clock.year)]);
    parts.join("  ·  ")
}

/// A time of day as HH:MM:SS (a duty's times may run past midnight or before it).
fn hms(t: f64) -> String {
    let s = t.round().rem_euclid(86400.0) as i64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// Planned, actual and the difference (+ late, − early), blank while it has not happened.
fn times(plan: f64, real: Option<f64>) -> [String; 3] {
    match real {
        Some(r) => {
            let d = (r - plan).round() as i64;
            let a = d.abs();
            [hms(plan), hms(r), format!("{}{:02}:{:02}:{:02}", if d < 0 { "-" } else { "+" }, a / 3600, a / 60 % 60, a % 60)]
        }
        None => [hms(plan), String::new(), String::new()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::PlannedStop;

    fn trip(stops: &[(&str, f64)]) -> PlannedTrip {
        PlannedTrip {
            name: "5_A".into(),
            line: "5".into(),
            terminus: "Muzealna".into(),
            departure: stops[0].1,
            end: stops[stops.len() - 1].1,
            stops: stops.iter().map(|(n, t)| PlannedStop { object_id: 0, name: n.to_string(), arr: *t, dep: *t, position: None, dir: Default::default(), stops: true }).collect(),
        }
    }

    /// A trip driven stop by stop: the first stop tells its departure only, the last its
    /// arrival only, a stop driven past is missed, a late arrival says so - and the file
    /// has it all at every stop.
    #[test]
    fn the_log_has_every_stop_with_its_times() {
        let t0 = 8.0 * 3600.0;
        let trips = [trip(&[("Metro Mlociny", t0), ("Zajezdnia", t0 + 300.0), ("Prozy", t0 + 600.0), ("Muzealna", t0 + 900.0)]), trip(&[("Muzealna", t0 + 1200.0), ("Metro Mlociny", t0 + 2100.0)])];
        let dir = std::env::temp_dir().join(format!("omsi-journey-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut j = Journey::new(("5".into(), "1".into(), 0), &trips, "Driver Test · Grundorf · MAN SD77 · 1989-05-30", dir.clone());
        assert!(j.arrived((0, 0), 20.0));
        assert!(!j.arrived((0, 0), 30.0), "the first arrival counts");
        assert!(j.left((0, 0), (20.0, 57.0)));
        j.write();
        let path = j.path.clone().expect("written");
        assert!(std::fs::read_to_string(&path).unwrap().contains("08:00:57"));
        // four minutes late at the second stop, past the third, early at the last
        assert!(j.arrived((0, 1), 240.0));
        assert!(j.left((0, 1), (240.0, 270.0)));
        assert!(j.arrived((0, 3), -30.0));
        j.write();
        let text = std::fs::read_to_string(&path).unwrap();
        let line = |name: &str| text.lines().find(|l| l.starts_with(name)).unwrap_or_else(|| panic!("no {name} in\n{text}")).to_string();
        assert!(text.contains("Line 5, tour 1") && text.contains("Driver Test") && text.contains("Trip 1  ·  5 to Muzealna  ·  08:00:00"), "{text}");
        assert!(line("Metro Mlociny").contains("08:00:00  08:00:57  +00:00:57"), "{text}");
        assert!(line("Metro Mlociny").ends_with("OK"), "{text}");
        assert!(line("Zajezdnia").contains("08:05:00  08:09:00  +00:04:00"), "{text}");
        assert!(line("Zajezdnia").contains("08:05:00  08:09:30  +00:04:30"), "{text}");
        assert!(line("Zajezdnia").ends_with("late"), "{text}");
        assert!(line("Prozy").ends_with("missed"), "{text}");
        assert!(line("Muzealna").contains("08:15:00  08:14:30  -00:00:30"), "{text}");
        assert!(!text.contains("Trip 2"), "a trip not begun is not in it yet: {text}");
        assert!(text.contains("Stops served: 3, late: 1, left early: 0"), "{text}");
        // the end-of-trip report shows the same
        let (title, rows, totals) = j.report(0).expect("trip 1");
        assert_eq!(title, "Trip 1  ·  5 to Muzealna  ·  08:00:00");
        let states: Vec<StopState> = rows.iter().map(|r| r.state).collect();
        assert_eq!(states, vec![StopState::OnTime, StopState::Late, StopState::Missed, StopState::OnTime]);
        assert_eq!((rows[0].scheduled.as_str(), rows[0].actual.as_str()), ("08:00:00", "08:00:57"));
        assert_eq!(rows[3].deviation, "-00:00:30");
        assert_eq!(totals[0], ("Stops served", "3 of 4".to_string()));
        assert_eq!(totals[1], ("Late arrivals", "1".to_string()));
        assert!(j.report(5).is_none());
        let _ = std::fs::remove_dir_all(&dir);
        // a duty taken up at its second trip: the first is not in it
        let mut j = Journey::new(("5".into(), "1".into(), 0), &trips, "Grundorf", dir.clone());
        assert!(j.arrived((1, 0), -60.0));
        let text = j.text();
        assert!(text.contains("Trip 2  ·  5 to Muzealna  ·  08:20:00") && !text.contains("Trip 1"), "{text}");
        // taken up (or resumed) at the third stop: the first two are not missed
        let mut j = Journey::new(("5".into(), "1".into(), 0), &trips, "Grundorf", dir.clone());
        assert!(j.arrived((0, 2), 0.0));
        assert!(j.arrived((0, 3), 0.0));
        let (_, rows, totals) = j.report(0).expect("trip 1");
        let states: Vec<StopState> = rows.iter().map(|r| r.state).collect();
        assert_eq!(states, vec![StopState::None, StopState::None, StopState::OnTime, StopState::OnTime]);
        assert_eq!(totals[0], ("Stops served", "2 of 2".to_string()));
        assert!(!j.text().contains("missed"), "{}", j.text());
    }

    /// A stop the driver skipped has no mark: not missed, not counted against the trip.
    #[test]
    fn a_skipped_stop_is_neither_missed_nor_counted() {
        let t0 = 8.0 * 3600.0;
        let trips = [trip(&[("Metro Mlociny", t0), ("Zajezdnia", t0 + 300.0), ("Prozy", t0 + 600.0), ("Muzealna", t0 + 900.0)])];
        let mut j = Journey::new(("5".into(), "1".into(), 0), &trips, "Grundorf", std::env::temp_dir());
        assert!(j.left((0, 0), (0.0, 60.0)));
        assert!(j.skipped((0, 1)));
        assert!(!j.skipped((0, 1)), "told once");
        assert!(j.arrived((0, 3), 120.0));
        let (_, rows, totals) = j.report(0).expect("trip 1");
        let states: Vec<StopState> = rows.iter().map(|r| r.state).collect();
        // (Prozy, passed without a word, is missed as before)
        assert_eq!(states, vec![StopState::OnTime, StopState::Skipped, StopState::Missed, StopState::OnTime]);
        assert_eq!(totals[0], ("Stops served", "2 of 3".to_string()));
        assert!(j.text().lines().any(|l| l.starts_with("Zajezdnia") && l.ends_with("skipped")), "{}", j.text());
        assert_eq!(j.how((0, 1)).as_deref(), Some("skipped"));
        assert_eq!(j.how((0, 0)).as_deref(), Some("served, +1 min"));
        assert_eq!(j.how((0, 3)).as_deref(), Some("served, +2 min"));
        assert_eq!(j.how((0, 2)), None);
        // a stop the bus has been at cannot be skipped after the fact
        assert!(!j.skipped((0, 0)));
    }

    /// A stop skipped and then served after all (the IBIS sent back to it) counts as served,
    /// once.
    #[test]
    fn a_skipped_stop_served_after_all_is_served() {
        let t0 = 8.0 * 3600.0;
        let trips = [trip(&[("Metro Mlociny", t0), ("Zajezdnia", t0 + 300.0), ("Muzealna", t0 + 900.0)])];
        let mut j = Journey::new(("5".into(), "1".into(), 0), &trips, "Grundorf", std::env::temp_dir());
        assert!(j.left((0, 0), (0.0, 0.0)));
        assert!(j.skipped((0, 1)));
        assert!(j.arrived((0, 1), 60.0));
        assert!(j.arrived((0, 2), 60.0));
        let (_, rows, totals) = j.report(0).expect("trip 1");
        let states: Vec<StopState> = rows.iter().map(|r| r.state).collect();
        assert_eq!(states, vec![StopState::OnTime, StopState::OnTime, StopState::OnTime]);
        assert_eq!(totals[0], ("Stops served", "3 of 3".to_string()));
        assert_eq!(j.how((0, 1)).as_deref(), Some("served, +1 min"));
        assert!(!j.text().contains("skipped"), "{}", j.text());
    }

    /// A stop reached late and left before its departure time says both.
    #[test]
    fn a_late_arrival_that_leaves_early_says_both() {
        let t0 = 8.0 * 3600.0;
        let trips = [trip(&[("Metro Mlociny", t0), ("Zajezdnia", t0 + 300.0), ("Muzealna", t0 + 900.0)])];
        let mut j = Journey::new(("5".into(), "1".into(), 0), &trips, "Grundorf", std::env::temp_dir());
        // (a stop with a layover: due at 08:05, leaving at 08:10)
        j.rows[1].plan.1 = t0 + 600.0;
        assert!(j.left((0, 0), (0.0, 0.0)));
        assert!(j.left((0, 1), (240.0, -180.0)));
        let (_, rows, totals) = j.report(0).expect("trip 1");
        assert_eq!(rows[1].state, StopState::LateLeftEarly);
        assert_eq!(rows[1].deviation, "+00:04:00");
        assert!(j.text().lines().any(|l| l.starts_with("Zajezdnia") && l.ends_with("late, left early")));
        assert_eq!(totals[1], ("Late arrivals", "1".to_string()));
        assert_eq!(totals[2], ("Early departures", "1".to_string()));
    }
}

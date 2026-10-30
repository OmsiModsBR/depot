//! The journey's timetable (#800, #1061): the stops of the duty's trips with their planned
//! and actual arrival and departure and how far off those were, as the logbooks of the
//! virtual bus companies keep them. It is written to the content folder's `Journeys` every
//! time the bus reaches or leaves a stop, so it is there however the session ends.

use crate::career::{EARLY_DEPARTURE, LATE_ARRIVAL};
use crate::schedule::{PlannedTrip, PlayerDuty};
use omsi_ui::ingame::{ReportRow, StopState};
use std::path::PathBuf;

/// One stop of a trip in the log.
pub struct Row {
    trip: usize,
    stop: usize,
    name: String,
    /// Planned arrival and departure, and the actual ones (s of the duty's day).
    pub plan: (f64, f64),
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
        let late = !self.first
            && self
                .arrived
                .is_some_and(|a| (a - self.plan.0).round() > LATE_ARRIVAL);
        let early = !self.last
            && self
                .left
                .is_some_and(|l| (l - self.plan.1).round() < EARLY_DEPARTURE);
        (late, early)
    }
}

/// A trip's title (the log file keeps it in English, the report shows it translated).
const TRIP_TITLE: &str = "Trip %{n}  ·  %{line} to %{to}  ·  %{at}";

fn trip_title(text: &str, (n, line, to, at): &(usize, String, String, String)) -> String {
    text.replace("%{n}", &n.to_string())
        .replace("%{line}", line)
        .replace("%{to}", to)
        .replace("%{at}", at)
}

pub struct Journey {
    /// The duty it is the log of: line, tour and where its trips begin in the tour.
    key: (String, String, usize),
    /// The lines above the trips (driver, map, bus, date; line and tour).
    head: String,
    /// Each trip's number in the tour, line, terminus and departure (its title).
    trips: Vec<(usize, String, String, String)>,
    pub rows: Vec<Row>,
    dir: PathBuf,
    /// The file, chosen when there is first something to write.
    pub path: Option<PathBuf>,
}

impl Journey {
    /// The log of the duty `key` (line, tour, where its `trips` begin in the tour), nothing
    /// driven yet; `head` comes first in it.
    pub fn new(
        key: (String, String, usize),
        trips_planned: &[PlannedTrip],
        head: &str,
        dir: PathBuf,
    ) -> Journey {
        let mut trips = Vec::new();
        let mut rows = Vec::new();
        for (t, trip) in trips_planned.iter().enumerate() {
            let line = if trip.line.trim().is_empty() {
                key.0.trim()
            } else {
                trip.line.trim()
            };
            trips.push((
                key.2 + t + 1,
                line.to_string(),
                trip.terminus.trim().to_string(),
                hms(trip.departure),
            ));
            let n = trip.stops.len();
            // (the stations a depot run passes are not stops)
            for (k, s) in trip.stops.iter().enumerate().filter(|(_, s)| s.stops) {
                rows.push(Row {
                    trip: t,
                    stop: k,
                    name: s.name.trim().to_string(),
                    plan: (s.arr, s.dep),
                    arrived: None,
                    left: None,
                    first: k == 0,
                    last: k + 1 == n,
                    skipped: false,
                });
            }
        }
        let head = format!(
            "{}\nLine {}, tour {}",
            head.trim_end(),
            key.0.trim(),
            key.1.trim()
        );
        Journey {
            key,
            head,
            trips,
            rows,
            dir,
            path: None,
        }
    }

    /// The bus arrived at stop `at` (trip, stop) `late` s after its time (negative: early).
    /// True when that is news.
    pub fn arrived(&mut self, at: (usize, usize), late: f64) -> bool {
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
    pub fn skipped(&mut self, at: (usize, usize)) -> bool {
        match self.row(at) {
            Some(r) if !r.skipped && !r.reached() => {
                r.skipped = true;
                true
            }
            _ => false,
        }
    }

    /// Whether the bus stopped at stop `at` (trip, stop).
    pub fn served(&self, at: (usize, usize)) -> bool {
        self.rows
            .iter()
            .any(|r| (r.trip, r.stop) == at && r.reached())
    }

    /// How stop `at` (trip, stop) went, in a few words for the timetable window ("served, +1
    /// min", "skipped"); None while the bus has not been there.
    pub fn how(&self, at: (usize, usize)) -> Option<String> {
        let r = self.rows.iter().find(|r| (r.trip, r.stop) == at)?;
        if r.skipped && !r.reached() {
            return Some("skipped".into());
        }
        // (the first stop is about its departure, every other about its arrival)
        let (real, plan) = if r.first {
            (r.left?, r.plan.1)
        } else {
            (r.arrived.or(r.left)?, r.plan.0)
        };
        let m = ((real - plan) / 60.0).round() as i64;
        Some(match m {
            0 => "served, on time".into(),
            m if m > 0 => omsi_ui::tr("served, %{m} min").replace("%{m}", &format!("+{m}")),
            m => omsi_ui::tr("served, %{m} min").replace("%{m}", &m.to_string()),
        })
    }

    /// The bus left stop `at`, having arrived and left `late` s after its times.
    pub fn left(&mut self, at: (usize, usize), late: (f64, f64)) -> bool {
        let Some(r) = self.row(at) else { return false };
        r.arrived.get_or_insert(r.plan.0 + late.0);
        r.left = Some(r.plan.1 + late.1);
        r.skipped = false;
        true
    }

    /// What the end-of-trip report shows of trip `t` (its place in the duty): its title, a
    /// row per stop and the totals, as the log counts them.
    pub fn report(&self, t: usize) -> Option<TripLines> {
        let title = trip_title(&omsi_ui::tr(TRIP_TITLE), self.trips.get(t)?);
        let rows: Vec<&Row> = self.rows.iter().filter(|r| r.trip == t).collect();
        // (a duty taken up or resumed in the middle of the trip: the stops before are not the
        // driver's)
        let from = rows.iter().position(|r| r.reached()).unwrap_or(rows.len());
        let (mut served, mut late, mut early) = (0, 0, 0);
        let skipped = rows
            .iter()
            .skip(from)
            .filter(|r| r.skipped && !r.reached())
            .count();
        let mut out = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            let (is_late, is_early) = r.judged();
            // (the first stop is about its departure, and a stop left early; every other
            // about its arrival)
            let by_departure = r.first || (is_early && !is_late);
            let [plan, real, diff] = if by_departure {
                times(r.plan.1, r.left)
            } else {
                times(r.plan.0, r.arrived)
            };
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
            out.push(ReportRow {
                stop: r.name.clone(),
                scheduled: plan,
                actual: real,
                deviation: diff,
                state,
            });
        }
        let totals = vec![
            // (a stop skipped on purpose is not one the trip had to serve)
            (
                "Stops served",
                omsi_ui::tr("%{served} of %{total}")
                    .replace("%{served}", &served.to_string())
                    .replace("%{total}", &(rows.len() - from - skipped).to_string()),
            ),
            ("Late arrivals", late.to_string()),
            ("Early departures", early.to_string()),
        ];
        Some((title, out, totals))
    }

    fn row(&mut self, (trip, stop): (usize, usize)) -> Option<&mut Row> {
        self.rows
            .iter_mut()
            .find(|r| r.trip == trip && r.stop == stop)
    }

    /// The log as it is written.
    pub fn text(&self) -> String {
        let rule = "-".repeat(108);
        let mut out = format!("Project Depot journey log\n{}\n", self.head);
        let (mut served, mut late, mut early) = (0, 0, 0);
        for (t, trip) in self.trips.iter().enumerate() {
            let title = trip_title(TRIP_TITLE, trip);
            let rows: Vec<&Row> = self.rows.iter().filter(|r| r.trip == t).collect();
            // (a trip is in it once the bus has reached one of its stops: a tour has dozens)
            let Some(from) = rows.iter().position(|r| r.reached()) else {
                continue;
            };
            out.push_str(&format!("\n{title}\n{rule}\n{:<30}  {:>9} {:>9} {:>10}   {:>9} {:>9} {:>10}   Status\n{rule}\n", "Stop", "Arr. plan", "Arr. real", "Diff", "Dep. plan", "Dep. real", "Diff"));
            for (i, r) in rows.iter().enumerate() {
                let arr = if r.first {
                    [String::new(), String::new(), String::new()]
                } else {
                    times(r.plan.0, r.arrived)
                };
                let dep = if r.last {
                    [String::new(), String::new(), String::new()]
                } else {
                    times(r.plan.1, r.left)
                };
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
                out.push_str(
                    format!(
                        "{name:<30}  {:>9} {:>9} {:>10}   {:>9} {:>9} {:>10}   {status}",
                        arr[0], arr[1], arr[2], dep[0], dep[1], dep[2]
                    )
                    .trim_end(),
                );
                out.push('\n');
            }
        }
        out.push_str(&format!(
            "{rule}\nStops served: {served}, late: {late}, left early: {early}\n"
        ));
        out
    }

    /// Write the log (its file is named by the real date and time it began, and the line and tour).
    pub fn write(&mut self) {
        if self.path.is_none() {
            let (y, mo, d, h, mi) = omsi_launcher_lib::local_now().unwrap_or((1970, 1, 1, 0, 0));
            let clean = |s: &str| {
                s.trim()
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect::<String>()
            };
            let stem = format!(
                "{y:04}-{mo:02}-{d:02} {h:02}-{mi:02} line {} tour {}",
                clean(&self.key.0),
                clean(&self.key.1)
            );
            if let Err(e) = std::fs::create_dir_all(&self.dir) {
                log::warn!("journey log: {}: {e}", self.dir.display());
                return;
            }
            // (another session begun in the same minute keeps its own)
            let path = (1..100)
                .map(|n| {
                    self.dir.join(if n == 1 {
                        format!("{stem}.txt")
                    } else {
                        format!("{stem} ({n}).txt")
                    })
                })
                .find(|p| !p.exists());
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
pub(crate) fn note(
    slot: &mut Option<Journey>,
    duty: &PlayerDuty,
    due: (usize, usize),
    served: Option<(f64, f64)>,
    root: &std::path::Path,
    head: impl FnOnce() -> String,
) {
    // (every frame: the key is compared, not made)
    if slot.as_ref().is_none_or(|j| {
        (j.key.0.as_str(), j.key.1.as_str(), j.key.2)
            != (duty.line.as_str(), duty.tour.as_str(), duty.first_trip)
    }) {
        let dir = crate::core::startup::content_dir()
            .unwrap_or_else(|| root.to_path_buf())
            .join("Journeys");
        *slot = Some(Journey::new(
            (duty.line.clone(), duty.tour.clone(), duty.first_trip),
            &duty.trips,
            &head(),
            dir,
        ));
    }
    let Some(j) = slot.as_mut() else { return };
    let left = served.is_some_and(|s| j.left(due, s));
    let arrived = duty
        .arrived()
        .is_some_and(|late| j.arrived((duty.trip_index, duty.next_stop), late));
    let mut skipped = false;
    for at in duty.skipped_stops() {
        skipped |= j.skipped(*at);
    }
    if left || arrived || skipped {
        j.write();
    }
}

/// The log's first line: the driver, the map, the bus and the game's date.
pub(crate) fn head(
    career: &crate::career::Career,
    map: &str,
    bus: &omsi_sim::VehicleInstance,
    clock: &omsi_sim::SimClock,
) -> String {
    let driver = career.driver.as_ref().map(|d| d.name.clone()).or_else(|| {
        career
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
    });
    let (day, month) = clock.day_month();
    let bus = format!(
        "{} {}",
        bus.ty.def.manufacturer.trim(),
        bus.ty.def.type_name.trim()
    );
    let mut parts: Vec<String> = driver
        .map(|d| format!("Driver {}", d.trim()))
        .into_iter()
        .collect();
    parts.extend([
        map.trim().to_string(),
        bus.trim().to_string(),
        format!("{:04}-{month:02}-{day:02}", clock.year),
    ]);
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
            [
                hms(plan),
                hms(r),
                format!(
                    "{}{:02}:{:02}:{:02}",
                    if d < 0 { "-" } else { "+" },
                    a / 3600,
                    a / 60 % 60,
                    a % 60
                ),
            ]
        }
        None => [hms(plan), String::new(), String::new()],
    }
}

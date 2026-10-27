//! The map's own timetable for the lines panel: for each bus line, its departures per direction
//! and day, and when a departure's bus is at each stop. Nothing is kept in the package: the
//! tours (`TTData/*.ttl`) say when each trip leaves and `schedule::TripTimes` says when it is at
//! each station, the same figures the AI buses of the game run by.

use crate::schedule::{trip_stations, TripTimes};
use omsi_timetable::TimetableData;
use std::collections::HashMap;
use std::path::Path;

/// The timetable of one date, read away from the frame: `get` never waits. The first call for a date starts a
/// thread that reads everything and sends it back; the frames after that look for the answer. A thread that dies
/// leaves an empty timetable for its date (so it is not tried again every frame); an answer for a date no longer
/// asked for is dropped and the current date is asked again.
pub struct TimesCache {
    load: std::sync::Arc<dyn Fn(i32) -> LineTimes + Send + Sync>,
    state: std::sync::Mutex<CacheState>,
}

#[derive(Default)]
struct CacheState {
    done: Option<(i32, std::sync::Arc<LineTimes>)>,
    pending: Option<(i32, std::sync::mpsc::Receiver<LineTimes>)>,
}

impl TimesCache {
    pub fn new(load: impl Fn(i32) -> LineTimes + Send + Sync + 'static) -> TimesCache {
        TimesCache {
            load: std::sync::Arc::new(load),
            state: Default::default(),
        }
    }

    /// What has been read for `date`, without starting a read (None: not read yet, or for another date).
    pub fn peek(&self, date: i32) -> Option<std::sync::Arc<LineTimes>> {
        let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.done
            .as_ref()
            .filter(|(d, _)| *d == date)
            .map(|(_, t)| t.clone())
    }

    /// The timetable of `date`; None while it is being read.
    pub fn get(&self, date: i32) -> Option<std::sync::Arc<LineTimes>> {
        use std::sync::mpsc::TryRecvError;
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((d, t)) = &st.done {
            if *d == date {
                return Some(t.clone());
            }
        }
        if let Some((asked, rx)) = &st.pending {
            let asked = *asked;
            match rx.try_recv() {
                Ok(t) => {
                    st.pending = None;
                    if asked == date {
                        let t = std::sync::Arc::new(t);
                        st.done = Some((date, t.clone()));
                        return Some(t);
                    }
                }
                // (still reading: for a date that is over, it finishes first and the current one is asked then)
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    st.pending = None;
                    if asked == date {
                        let t = std::sync::Arc::new(LineTimes::default());
                        st.done = Some((date, t.clone()));
                        return Some(t);
                    }
                }
            }
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let load = self.load.clone();
        std::thread::spawn(move || {
            let _ = tx.send(load(date));
        });
        st.pending = Some((date, rx));
        None
    }
}

/// A tour's mask when the file says nothing: every weekday, holidays and both school periods.
pub const ALL_DAYS: i32 = 1023;

/// The mask bit a tour needs to run on `date` (yyyymmdd), the one the AI buses use; both bits while the date is
/// not known (nothing is left out).
pub fn period_bit(calendar: &omsi_map::Calendar, date: i32) -> i32 {
    if date <= 0 {
        (1 << 8) | (1 << 9)
    } else {
        crate::schedule::school_period_bit(calendar, date)
    }
}

/// A departure: seconds after midnight, the weekdays it runs on (bits 0-6, Monday first), the
/// trip it runs and the trip's profile.
pub struct Dep {
    pub t: f32,
    pub mask: i32,
    pub trip: u16,
    pub profile: u8,
}

/// Which departure is picked: its time (s) and, when it is known, its trip and profile, since two trips (or
/// one trip with two profiles, whose stop times differ) can leave at the same time. A time alone (typed by hand) means the first departure at that time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DepKey {
    pub t: u32,
    pub trip: Option<u16>,
    pub profile: Option<u8>,
}

impl Dep {
    pub fn key(&self) -> DepKey {
        DepKey {
            t: self.t as u32,
            trip: Some(self.trip),
            profile: Some(self.profile),
        }
    }

    /// Whether this is the departure `pick` names.
    pub fn is(&self, pick: &DepKey) -> bool {
        self.t as u32 == pick.t
            && pick.trip.is_none_or(|trip| trip == self.trip)
            && pick.profile.is_none_or(|p| p == self.profile)
    }
}

/// A trip's stations (id and name) and when its bus is at each, per profile.
struct Run {
    stops: Vec<(i64, String)>,
    /// Whether the stop is listed: a type-1 trip's stations with no dwell time are pass-through points (the
    /// map's `... Ig` records beside a stop), except the first and the last.
    listed: Vec<bool>,
    times: Vec<TripTimes>,
}

/// The departures towards one terminus.
pub struct Dir {
    pub to: String,
    /// The name of the last stop (the destination board may say something else).
    pub end: String,
    pub deps: Vec<Dep>,
}

/// One line's timetable: at most its two busiest directions.
#[derive(Default)]
pub struct Sheet {
    pub dirs: Vec<Dir>,
    runs: Vec<Run>,
}

/// A stop of a departure: the map object, the name, the time of day (s) and whether the bus stops.
pub struct StopTime<'a> {
    pub id: i64,
    pub name: &'a str,
    pub at: f64,
    pub stops: bool,
}

#[derive(Default)]
pub struct LineTimes(HashMap<String, Sheet>);

impl LineTimes {
    pub fn load(map_dir: &Path, chrono: &[std::path::PathBuf], date: i32) -> LineTimes {
        let began = std::time::Instant::now();
        let data = TimetableData::load_with_chrono(
            map_dir,
            chrono,
            &omsi_map::chrono_deactivated_lines(chrono),
        );
        let calendar = omsi_map::Calendar::load(&map_dir.join("Holidays.txt")).unwrap_or_default();
        let period = period_bit(&calendar, date);
        let mut links: HashMap<(i64, i64), f64> = HashMap::new();
        for l in &data.stn_links {
            links.entry((l.from_id, l.to_id)).or_insert(l.length);
        }
        let link = |a: i64, b: i64| links.get(&(a, b)).copied();
        let mut out: HashMap<String, Sheet> = HashMap::new();
        // (line, trip) -> the run's place in its sheet
        let mut made: HashMap<(String, usize), u16> = HashMap::new();
        let mut by_dir: HashMap<String, HashMap<String, Vec<Dep>>> = HashMap::new();
        let mut ends: HashMap<(String, String), String> = HashMap::new();
        for line in &data.lines {
            for tour in &line.tours {
                let mask = tour.extra.trim().parse::<i32>().unwrap_or(ALL_DAYS);
                // (as the AI buses do: a tour of the other school period does not run on this date)
                if mask & period == 0 {
                    continue;
                }
                for tt in &tour.trips {
                    let Some(ti) = data
                        .trips
                        .iter()
                        .position(|x| x.name.eq_ignore_ascii_case(&tt.trip))
                    else {
                        continue;
                    };
                    let trip = &data.trips[ti];
                    // (a train's trips carry no line: its file names it)
                    let number = if trip.line.trim().is_empty() {
                        line.name.trim()
                    } else {
                        trip.line.trim()
                    }
                    .to_string();
                    let key = number.to_lowercase();
                    let sheet = out.entry(key.clone()).or_default();
                    let run = *made.entry((key.clone(), ti)).or_insert_with(|| {
                        let ids = trip_stations(trip);
                        let name = |k: usize| {
                            let id = ids[k];
                            trip.stations_legacy
                                .iter()
                                .find(|s| {
                                    s.first().and_then(|v| v.trim().parse::<i64>().ok()) == Some(id)
                                })
                                .and_then(|s| s.get(2).cloned())
                                .or_else(|| {
                                    data.bus_stops
                                        .iter()
                                        .find(|b| b.object_id == id)
                                        .map(|b| b.name.clone())
                                })
                                .unwrap_or_else(|| id.to_string())
                        };
                        let stops = (0..ids.len())
                            .map(|k| (ids[k], strip_number(&name(k), &number)))
                            .collect();
                        let dwell = |k: usize| {
                            if trip.stations.is_empty() {
                                trip.stations_legacy
                                    .get(k)
                                    .and_then(|s| s.get(7))
                                    .and_then(|v| v.trim().parse::<f64>().ok())
                            } else {
                                None
                            }
                        };
                        let listed = (0..ids.len())
                            .map(|k| {
                                k == 0 || k + 1 == ids.len() || dwell(k).is_none_or(|d| d > 0.0)
                            })
                            .collect();
                        let times = if trip.profiles.is_empty() {
                            vec![TripTimes::new(&ids, None, &link)]
                        } else {
                            trip.profiles
                                .iter()
                                .map(|p| TripTimes::new(&ids, Some(p), &link))
                                .collect()
                        };
                        sheet.runs.push(Run {
                            stops,
                            listed,
                            times,
                        });
                        (sheet.runs.len() - 1) as u16
                    });
                    let profile = usize::try_from(tt.profile)
                        .unwrap_or(0)
                        .min(sheet.runs[run as usize].times.len() - 1)
                        as u8;
                    let to = strip_number(trip.terminus.trim(), &number);
                    ends.entry((key.clone(), to.clone())).or_insert_with(|| {
                        sheet.runs[run as usize]
                            .stops
                            .last()
                            .map(|s| s.1.clone())
                            .unwrap_or_default()
                    });
                    by_dir
                        .entry(key)
                        .or_default()
                        .entry(to)
                        .or_default()
                        .push(Dep {
                            t: tt.departure * 60.0,
                            mask,
                            trip: run,
                            profile,
                        });
                }
            }
        }
        for (key, dirs) in by_dir {
            let mut dirs: Vec<Dir> = dirs
                .into_iter()
                .map(|(to, mut deps)| {
                    deps.sort_by(|a, b| a.t.total_cmp(&b.t));
                    Dir {
                        end: ends.remove(&(key.clone(), to.clone())).unwrap_or_default(),
                        to,
                        deps,
                    }
                })
                .collect();
            dirs.sort_by(|a, b| {
                b.deps
                    .len()
                    .cmp(&a.deps.len())
                    .then_with(|| a.to.cmp(&b.to))
            });
            dirs.truncate(2);
            if let Some(s) = out.get_mut(&key) {
                s.dirs = dirs;
            }
        }
        out.retain(|_, s| !s.dirs.is_empty());
        log::info!(
            "timetable for the lines panel: {} lines in {:?}",
            out.len(),
            began.elapsed()
        );
        LineTimes(out)
    }

    pub fn line(&self, number: &str) -> Option<&Sheet> {
        self.0.get(&number.trim().to_lowercase())
    }
}

/// A letter without its accent.
fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        _ => c,
    }
}

/// An IBIS-style name ("110Terminal Biblia") without the line number in front.
fn strip_number(name: &str, line: &str) -> String {
    match name.strip_prefix(line) {
        Some(rest) if !line.is_empty() && !rest.starts_with(|c: char| c.is_ascii_digit()) => {
            rest.trim_start().to_string()
        }
        _ => name.to_string(),
    }
}

impl Sheet {
    /// The directions in the order of the line's routes: route 1 goes towards the end its itinerary
    /// names last ("A - B": B), route 2 the other way.
    pub fn order(&self, itinerary: &str) -> Vec<usize> {
        let words = |s: &str| -> Vec<String> {
            s.to_lowercase()
                .chars()
                .map(fold)
                .collect::<String>()
                .split_whitespace()
                .map(str::to_string)
                .collect()
        };
        let end = words(itinerary.rsplit(['–', '-']).next().unwrap_or(""));
        let score = |d: &Dir| {
            [&d.to, &d.end]
                .iter()
                .map(|n| {
                    let w = words(n);
                    w.iter().filter(|x| end.contains(x)).count() as f32 / w.len().max(1) as f32
                })
                .fold(0.0, f32::max)
        };
        let mut o: Vec<usize> = (0..self.dirs.len()).collect();
        o.sort_by(|&a, &b| score(&self.dirs[b]).total_cmp(&score(&self.dirs[a])));
        o
    }

    /// Direction `k`'s departures on weekday `day` (0 = Monday).
    pub fn departures(&self, k: usize, day: usize) -> Vec<&Dep> {
        self.dirs.get(k).map_or_else(Vec::new, |d| {
            d.deps.iter().filter(|x| x.mask & (1 << day) != 0).collect()
        })
    }

    /// Where a departure's bus is when: the first stop leaves at its time, the last arrives.
    pub fn stops(&self, d: &Dep) -> Vec<StopTime<'_>> {
        let run = &self.runs[d.trip as usize];
        let t = &run.times[d.profile as usize];
        let last = run.stops.len().saturating_sub(1);
        run.stops
            .iter()
            .enumerate()
            .filter_map(|(k, (id, name))| {
                let (arr, dep) = *t.stations.get(k)?;
                Some(StopTime {
                    id: *id,
                    name,
                    at: d.t as f64 + if k == last { arr } else { dep },
                    stops: run.listed[k] && t.stops.get(k).copied().unwrap_or(true),
                })
            })
            .collect()
    }
}

/// `HH:MM` of a time of day in seconds, rounded to the minute (past 24:00 it wraps).
pub fn clock(secs: f64) -> String {
    let m = (secs / 60.0).round() as i64;
    format!(
        "{:02}:{:02}",
        m.div_euclid(60).rem_euclid(24),
        m.rem_euclid(60)
    )
}

//! What a map's lines run on a date, as the Timetable page's Map and Schematic tabs show it:
//! the lines with their trips and stops (`omsi_launcher_lib::list_lines`, read in the
//! background), and what the page looks up in them every frame worked out once when they
//! arrive - which lines call at a stop, what leaves from it and when, the trip each direction
//! is drawn from, and the roads that trip drives (`omsi_launcher_lib::trip_paths`: the map
//! draws a line on its streets or not at all). Stops are told apart by name (any case): the
//! two sides of a street are one stop here, as a timetable's stop list has them.

use omsi_launcher_lib as core;
use std::collections::HashMap;

/// A stop of the plan, by name.
#[derive(Debug, Default)]
pub struct StopEntry {
    /// The name as the timetable writes it (the first one met).
    pub name: String,
    /// Its map objects (one per side of the street, platform...), the first met first.
    pub ids: Vec<i64>,
    /// The lines that call here (by their place in [`Plan::lines`]), each with where it goes
    /// from here.
    pub serving: Vec<(usize, Vec<String>)>,
    /// What leaves from here on the tours that run that day: (time, line, where it goes), by
    /// time.
    pub departures: Vec<(f64, usize, String)>,
}

/// A way a line runs: where it goes, and the trip drawn for it as (tour, trip).
type Way = (String, Option<(usize, usize)>);

#[derive(Debug, Default)]
pub struct Plan {
    /// The lines, by name (any case).
    pub lines: Vec<core::LineInfo>,
    /// Per line: the ways it runs (its termini, the line's own first), and for each the trip
    /// drawn for it as (tour, trip) - the one with the most stops.
    dirs: Vec<Vec<Way>>,
    stops: Vec<StopEntry>,
    by_name: HashMap<String, usize>,
    /// Where each trip (by name, lower case) starts.
    trip_from: HashMap<String, String>,
    /// The roads of the trips the directions are drawn from (by name, lower case).
    paths: HashMap<String, core::TripPath>,
}

/// The stops a trip calls at to let people on: not the stations it passes, nor the last one
/// (where it ends: nothing leaves from there).
fn calls(trip: &core::TripInfo) -> impl Iterator<Item = &core::StopInfo> {
    let n = trip.stops.len().saturating_sub(1);
    trip.stops.iter().take(n).filter(|s| s.stops)
}

impl Plan {
    pub fn new(mut lines: Vec<core::LineInfo>) -> Plan {
        lines.sort_by_key(|x| x.name.to_lowercase());
        let mut plan = Plan::default();
        for (k, line) in lines.iter().enumerate() {
            // the ways it runs: its termini, in the order the line lists them
            let mut ways: Vec<String> = line.termini.clone();
            for t in line.tours.iter().flat_map(|t| t.trips.iter()) {
                if !t.terminus.is_empty() && !ways.contains(&t.terminus) {
                    ways.push(t.terminus.clone());
                }
            }
            let dirs = ways
                .into_iter()
                .map(|to| {
                    let mut best: Option<((usize, usize), usize)> = None;
                    for (a, tour) in line.tours.iter().enumerate() {
                        for (b, trip) in tour.trips.iter().enumerate() {
                            if trip.terminus == to && best.is_none_or(|x| trip.stops.len() > x.1) {
                                best = Some(((a, b), trip.stops.len()));
                            }
                        }
                    }
                    (to, best.map(|x| x.0))
                })
                .collect();
            plan.dirs.push(dirs);
            for tour in &line.tours {
                for trip in &tour.trips {
                    plan.trip_from
                        .entry(trip.name.to_lowercase())
                        .or_insert_with(|| trip.from.clone());
                    for st in calls(trip) {
                        let i = plan.entry(&st.name, st.id);
                        let e = &mut plan.stops[i];
                        match e.serving.iter_mut().find(|x| x.0 == k) {
                            Some((_, to)) if !to.contains(&trip.terminus) => {
                                to.push(trip.terminus.clone())
                            }
                            Some(_) => {}
                            None => e.serving.push((k, vec![trip.terminus.clone()])),
                        }
                        if tour.runs {
                            e.departures.push((st.dep, k, trip.terminus.clone()));
                        }
                    }
                    // (a stop where trips only end is still a stop of the map)
                    for st in trip.stops.iter().filter(|s| s.stops) {
                        plan.entry(&st.name, st.id);
                    }
                }
            }
        }
        for e in &mut plan.stops {
            e.serving.sort_by_key(|x| x.0);
            e.departures.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        plan.lines = lines;
        plan
    }

    /// The stop called `name`, made when it is new; its place.
    fn entry(&mut self, name: &str, id: i64) -> usize {
        let key = name.trim().to_lowercase();
        let i = match self.by_name.get(&key) {
            Some(i) => *i,
            None => {
                self.stops.push(StopEntry {
                    name: name.trim().to_string(),
                    ..Default::default()
                });
                self.by_name.insert(key, self.stops.len() - 1);
                self.stops.len() - 1
            }
        };
        if !self.stops[i].ids.contains(&id) {
            self.stops[i].ids.push(id);
        }
        i
    }

    /// The line called `name` (any case): its place.
    pub fn line(&self, name: &str) -> Option<usize> {
        self.lines
            .iter()
            .position(|x| x.name.eq_ignore_ascii_case(name))
    }

    /// The ways line `k` runs: where it goes.
    pub fn directions(&self, k: usize) -> Vec<&str> {
        self.dirs
            .get(k)
            .map(|d| d.iter().map(|x| x.0.as_str()).collect())
            .unwrap_or_default()
    }

    /// The trip drawn for line `k` going its `dir`-th way (the ways go round).
    pub fn route(&self, k: usize, dir: usize) -> Option<&core::TripInfo> {
        let dirs = self.dirs.get(k)?;
        let (a, b) = dirs.get(dir % dirs.len().max(1))?.1?;
        self.lines.get(k)?.tours.get(a)?.trips.get(b)
    }

    /// Every stop's entry.
    pub fn stops(&self) -> &[StopEntry] {
        &self.stops
    }

    pub fn stop(&self, name: &str) -> Option<&StopEntry> {
        self.by_name
            .get(&name.trim().to_lowercase())
            .map(|i| &self.stops[*i])
    }

    /// The lines that call at `stop`, each with where it goes from there.
    pub fn serving(&self, stop: &str) -> &[(usize, Vec<String>)] {
        self.stop(stop)
            .map(|e| e.serving.as_slice())
            .unwrap_or_default()
    }

    /// What leaves from `stop` at `from` (seconds of the day) or later, of line `only` when
    /// one is given: the first `n`.
    pub fn departures(
        &self,
        stop: &str,
        from: f64,
        n: usize,
        only: Option<usize>,
    ) -> Vec<&(f64, usize, String)> {
        let Some(e) = self.stop(stop) else {
            return Vec::new();
        };
        let at = e.departures.partition_point(|d| d.0 < from);
        e.departures[at..]
            .iter()
            .filter(|d| only.is_none_or(|k| d.1 == k))
            .take(n)
            .collect()
    }

    /// The stops whose name holds `q` (any case), the ones it begins first; at most `n`.
    pub fn find(&self, q: &str, n: usize) -> Vec<&str> {
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<(bool, &str)> = self
            .stops
            .iter()
            .filter_map(|e| {
                let low = e.name.to_lowercase();
                low.contains(&q)
                    .then_some((!low.starts_with(&q), e.name.as_str()))
            })
            .collect();
        hits.sort();
        hits.into_iter().take(n).map(|x| x.1).collect()
    }

    /// The trips the directions are drawn from, each once.
    pub fn drawn_trips(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for k in 0..self.lines.len() {
            for dir in 0..self.dirs[k].len() {
                if let Some(t) = self
                    .route(k, dir)
                    .filter(|t| !out.iter().any(|x| x.eq_ignore_ascii_case(&t.name)))
                {
                    out.push(t.name.clone());
                }
            }
        }
        out
    }

    /// The roads trip `name` drives, when they were read.
    pub fn path(&self, name: &str) -> Option<&core::TripPath> {
        self.paths.get(&name.to_lowercase())
    }

    /// The roads of the trips `names` (as [`Plan::drawn_trips`] gave them), in that order.
    pub fn set_paths(&mut self, names: &[String], paths: Vec<core::TripPath>) {
        self.paths = names.iter().map(|n| n.to_lowercase()).zip(paths).collect();
    }

    /// Where trip `name` starts (its first stop).
    pub fn trip_from(&self, name: &str) -> Option<&str> {
        self.trip_from.get(&name.to_lowercase()).map(String::as_str)
    }
}

//! The duty the lists start and end: lines, tours and their trips, and the driver whose run it is.

use super::vehicles::bus_cmp;
use crate::core::app::App;
use crate::hud::domain::lists::ListKind;

/// A switch row, if the setting `id` is one.
/// The duty given up (the game menu's "End the duty", Free drive in the list of lines): the
/// bus is unscheduled, and the `GetTT*` callbacks answer ""/0/-1 again as in OMSI. "End
/// the duty" dropped the duty alone, and the bus's own displays went on with the old trip -
/// a paper sign with its line in the window, the IBIS's stop list (#1317).
pub(crate) fn end_duty(app: &mut App) {
    app.duty = None;
    app.duty_changed();
    if let Some(p) = app.player.as_mut() {
        clear_timetable(&mut p.vehicle);
    }
    app.service_msg = Some(("Free drive: no duty".into(), 4.0));
}

/// A vehicle's timetable for its scripts emptied (see `end_duty`).
pub(crate) fn clear_timetable(v: &mut omsi_sim::VehicleInstance) {
    let h = &mut v.host;
    h.tt_line.clear();
    h.tt_stops.clear();
    h.tt_stop_ids.clear();
    h.tt_busstop_index = -1;
    h.tt_terminus_index = -1;
    h.tt_delay = 0.0;
    h.schedule_active = 0.0;
    v.set_var("schedule_active", 0.0);
}

/// The time (seconds of the day) a tour starts: its earliest trip's departure.
pub(crate) fn tour_start(tour: &omsi_timetable::Tour) -> Option<f64> {
    tour.trips
        .iter()
        .map(|t| t.departure as f64 * 60.0)
        .fold(None, |a: Option<f64>, d| Some(a.map_or(d, |x| x.min(d))))
}

/// Whether a tour is listed now: it runs on this day and is current at `now` (seconds of the
/// day) - under way, or leaving within half an hour; one that has finished is not.
pub(in crate::hud) fn tour_listed(
    sch: &crate::schedule::Schedule,
    line: &str,
    tour: &omsi_timetable::Tour,
    now: f64,
) -> bool {
    if !sch.tour_available(tour) {
        return false;
    }
    let start = tour_start(tour).unwrap_or(0.0);
    let end = sch
        .tour_stops(line, &tour.number)
        .iter()
        .map(|s| s.3)
        .fold(start, f64::max);
    // (a night tour's times go on past 24:00: the early hours of the next day count too)
    [now, now + 86400.0]
        .iter()
        .any(|n| *n >= start - 1800.0 && *n <= end + 60.0)
}

/// The line number a line's trips carry (`.ttp` line), else the name of the line's file.
pub(in crate::hud) fn line_sign(
    schedule: Option<&crate::schedule::Schedule>,
    line: &omsi_timetable::Line,
) -> String {
    let sign = schedule.and_then(|sch| {
        line.tours
            .iter()
            .flat_map(|t| t.trips.iter())
            .find_map(|tt| {
                let t = sch
                    .data
                    .trips
                    .iter()
                    .find(|x| x.name.eq_ignore_ascii_case(&tt.trip))?;
                Some(t.line.trim().to_string()).filter(|n| !n.is_empty())
            })
    });
    sign.unwrap_or_else(|| line.name.clone())
}

/// A line's tours in alphabetical order of their numbers (numbers inside them as numbers:
/// "2" before "10"; equal numbers by the time they start).
pub(in crate::hud) fn sorted_tours(line: &omsi_timetable::Line) -> Vec<&omsi_timetable::Tour> {
    let mut tours: Vec<&omsi_timetable::Tour> = line.tours.iter().collect();
    tours.sort_by(|a, b| {
        bus_cmp(a.number.trim(), b.number.trim()).then_with(|| {
            let (ta, tb) = (
                tour_start(a).unwrap_or(f64::MAX),
                tour_start(b).unwrap_or(f64::MAX),
            );
            ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    tours
}

/// The name of trip number `k` of a tour, counted as `Schedule::tour_stops` does (trips the
/// timetable does not know are left out).
pub(in crate::hud) fn tour_trip_name(
    sch: &crate::schedule::Schedule,
    tour: &omsi_timetable::Tour,
    k: usize,
) -> Option<String> {
    tour.trips
        .iter()
        .filter(|tt| {
            sch.data
                .trips
                .iter()
                .any(|x| x.name.eq_ignore_ascii_case(&tt.trip))
        })
        .nth(k)
        .map(|tt| tt.trip.clone())
}

/// Numbers compared as numbers where they are ("5" before "13", "N30" after "M49").
pub fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| {
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        (
            digits.parse::<u64>().unwrap_or(u64::MAX),
            s.to_ascii_lowercase(),
        )
    };
    key(a).cmp(&key(b))
}

/// The personnel files there are (content folder and OMSI 2's `Drivers`), by name.
pub(crate) fn driver_names(app: &App) -> Vec<String> {
    let mut names: Vec<String> = omsi_cfg::read_dir_merged("Drivers")
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("odr")))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .collect();
    let _ = app;
    names.sort_by_key(|n| n.to_ascii_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

/// Go on with another driver: this run so far into the old personnel file, the rest into
/// the new one.
pub(crate) fn switch_driver(app: &mut App, name: &str) {
    if app.career.path.is_some() {
        if let Err(e) = app.career.save() {
            log::warn!("writing the personnel file: {e}");
        }
    }
    let rel = format!("Drivers/{name}.odr");
    let mut next = crate::career::Career::load(&app.args.root, &rel);
    // (the distance and the clock of the run go on; the counters start with the new file)
    next.seconds = app.career.seconds;
    app.career = next;
    app.args.driver = Some(rel);
    app.service_msg = Some((
        omsi_ui::tr("Driver: %{name}").replace("%{name}", &name),
        3.0,
    ));
}

/// Take on line `line`, tour `tour` from now: the duty, and the IBIS typed for it.
/// The tour on row `k` of the open list of tours: (line, tour).
pub(crate) fn tour_at(app: &App, k: usize) -> Option<(String, String)> {
    let action = app.admin_list.as_ref()?.get(k)?.1.strip_prefix("tour ")?;
    let (line, tour) = action.split_once('\u{1}')?;
    Some((line.to_string(), tour.to_string()))
}

/// The time (seconds of the day) tour `tour` of line `line` starts.
pub(crate) fn tour_start_of(app: &App, line: &str, tour: &str) -> f64 {
    app.schedule
        .as_ref()
        .and_then(|s| s.data.lines.iter().find(|l| l.name == line))
        .and_then(|l| l.tours.iter().find(|t| t.number == tour))
        .and_then(tour_start)
        .unwrap_or(0.0)
}

/// For the tour on row `k`: how many stops its chosen trip has, the stop chosen to start
/// from, the trip chosen and how many trips the tour has.
pub(crate) fn tour_choice(app: &App, k: usize) -> Option<(usize, usize, usize, usize)> {
    let (line, tour) = tour_at(app, k)?;
    let sch = app.schedule.as_ref()?;
    let trips = sch.tour_trip_count(&line, &tour);
    let (stop, trip) = match app.list_kind.as_ref() {
        Some(ListKind::Tours(_, Some(p))) if p.0 == tour => (p.1, p.2),
        _ => (0, sch.tour_trip_now(&line, &tour, app.clock.time)),
    };
    let trip = trip.min(trips.saturating_sub(1));
    let n = sch.tour_trip_stops(&line, &tour, trip).len();
    (n > 0).then(|| (n, stop.min(n - 1), trip, trips))
}

/// Start the tour at stop number `chosen` of trip number `trip` of the tour (the trip chosen
/// by its time): the duty goes on from that stop, the bus stays where it is.
pub(crate) fn start_duty_at(app: &mut App, line: &str, tour: &str, trip: usize, chosen: usize) {
    let now = app.clock.time;
    let at = tour_start_of(app, line, tour);
    let Some((k, j)) = app.schedule.as_ref().and_then(|s| {
        s.tour_trip_stops(line, tour, trip)
            .get(chosen)
            .map(|x| (x.0, x.1))
    }) else {
        return start_duty(app, line, tour);
    };
    let (Some(w), Some(sch)) = (app.world.clone(), app.schedule.as_mut()) else {
        return;
    };
    let mut d = match sch.player_duty(&w, line, tour, at, None, false) {
        Ok(d) => d,
        Err(e) => {
            app.service_msg = Some((
                omsi_ui::tr("No duty: %{error}").replace("%{error}", &e.to_string()),
                8.0,
            ));
            return;
        }
    };
    // (no teleport: the stop chosen is the one the bus drives to next)
    d.start_at_here(k, j);
    if let Some(p) = app.player.as_mut() {
        d.update(&mut p.vehicle, now);
        let (trip, stop) = d.trip_for_ibis();
        p.set_duty_destination(trip, stop);
        if let Some(w) = app.world.as_ref() {
            let mut fonts = w.fonts.lock();
            if let Err(e) = crate::schedule_paper::update_vehicle(&mut p.vehicle, &d, &mut fonts) {
                log::warn!("driver timetable paper: {e:#}");
            }
        }
    }
    app.args.line = Some(line.to_string());
    app.args.tour = Some(tour.to_string());
    app.duty_changed();
    app.duty = Some(d);
    app.service_msg = Some((format!("Line {line}, tour {}", tour.trim()), 4.0));
}

pub(in crate::hud) fn start_duty(app: &mut App, line: &str, tour: &str) {
    let (Some(w), Some(sch)) = (app.world.clone(), app.schedule.as_mut()) else {
        return;
    };
    let now = app.clock.time;
    match sch.player_duty(&w, line, tour, now, None, false) {
        Ok(mut d) => {
            if let Some(p) = app.player.as_mut() {
                d.update(&mut p.vehicle, now);
                let (trip, stop) = d.trip_for_ibis();
                p.set_duty_destination(trip, stop);
                let mut fonts = w.fonts.lock();
                if let Err(e) =
                    crate::schedule_paper::update_vehicle(&mut p.vehicle, &d, &mut fonts)
                {
                    log::warn!("driver timetable paper: {e:#}");
                }
            }
            app.args.line = Some(line.to_string());
            app.args.tour = Some(tour.to_string());
            app.duty_changed();
            app.duty = Some(d);
            app.service_msg = Some((format!("Line {line}, tour {}", tour.trim()), 4.0));
        }
        Err(e) => {
            app.service_msg = Some((
                omsi_ui::tr("No duty: %{error}").replace("%{error}", &e.to_string()),
                8.0,
            ))
        }
    }
}

//! The vehicle as a page sees it: `window.omsi.vehicle`.
//!
//! OMSI buses name their script variables as they please (`door_0`, `Fahrertuer_Rechts`,
//! `elec_busbar_main` ...), so a page that wants "is the engine running?" would have to
//! know every bus. This module turns the variables into one **normalised snapshot** with
//! fixed names, using the same conventions the game itself uses (the engine check of the
//! start-up helper, the LAN pose, the door logic of the passengers). Anything a bus does not
//! have shows up as `null` (numbers) or `false` (switches), never as a missing property, so
//! a page can read `omsi.vehicle.engine.rpm` without guarding.
//!
//! Variables the snapshot does not cover are still reachable: `omsi.vars.num`,
//! `omsi.vars.str` and `omsi.getVar(name)` give every variable of the bus's own variable list.
//!
//! The snapshot is a tree of [`ApiValue`]s that any [`crate::htmltex::HtmlRenderer`] backend
//! can turn into its own objects. Numbers are rounded to a sensible step so that a value that
//! merely jitters in the last digit does not redraw the page every frame.
//!
//! # Layout (API version 1)
//!
//! | path | meaning |
//! | --- | --- |
//! | `api` | version of this layout (1) |
//! | `info.number`, `.ident`, `.yard`, `.route`, `.nextStop` | text variables of the bus |
//! | `motion.speedKmh`, `.heading`, `.pitch`, `.bank`, `.steeringDeg`, `.x`, `.y`, `.z`, `.odometerKm` | movement and place |
//! | `engine.running`, `.rpm`, `.throttle`, `.brake`, `.clutch`, `.gear`, `.tankContent` | drive train and pedals |
//! | `electrics.on`, `.busbarMain`, `.busbarAvailable`, `.failure` | on-board network |
//! | `battery.on` | battery switch (`null` when the bus has none) |
//! | `doors.count`, `.anyOpen`, `.list[i].number`, `.open`, `.isOpen` | door leaves, `list[0]` is door 1 |
//! | `passengers.onboard`, `.entries[i]`, `.exits[i]` (`number`, `open`, `requested`) | boarding state |
//! | `lights.headlights` (0-3), `.brake`, `.reverse`, `.fog`, `.indicator` (0-3), `.indicatorLeft`, `.indicatorRight`, `.hazard`, `.interior` | lamps |
//! | `brakes.parking`, `.stop`, `.kneeling` | brake and kneeling switches |
//! | `wipers.running` | windscreen wipers |
//! | `cabin.temperature` | cabin air, °C |
//! | `condition.dirt`, `.crashes`, `.lastImpactKJ`, `.streetCondition` | wear and road |
//! | `train.trailers` | coupled vehicles behind this one |
//! | `route.active` | the bus has a timetable (a duty) |
//! | `route.line`, `.destination` | line text and the text of the destination sign |
//! | `route.current` | the stop the bus is at or heading for: `index`, `name`, `arrival`, `departure` (`HH:MM`), `arrivalSec`, `departureSec` (s after midnight) |
//! | `route.terminus` | the last stop, same fields |
//! | `route.stops[i]` | `index`, `name`, `arrival`, `departure`, `arrivalSec`, `departureSec`, `served`, `current` |
//! | `route.nextIndex`, `.delaySec`, `.source` | next stop, delay (s), `timetable` / `ibis` / `none` |
//! | `route.ibis` | the IBIS's own numbers: `line`, `suffix`, `routeIndex`, `terminusIndex`, `terminusCode` |
//!
//! `window.omsi.time` (`hour`, `minute`, `second`, `asString`), `window.omsi.date` (`day`,
//! `month`, `year`, `asString`) and `window.omsi.locale` come from [`environment`].
//!
//! `window.omsi.depot` (see [`depot`]) lists what the depot file offers: `lines[]` (each with
//! its `routes[]`), `routes[]`, `destinations[]`. The page acts on it with
//! `omsi.setRoute(index)`, `omsi.setLine(text)` and `omsi.setDestination(index)`; with a
//! timetable `omsi.setNextStop(index)` skips to that stop.

mod snapshot;

pub use snapshot::{snapshot, Inputs};

use crate::vehicle::VehicleInstance;
use crate::SimClock;
use omsi_vehicle::hof::Hof;
use std::sync::RwLock;

pub use crate::htmlengine::ApiValue;

static LOCALE: RwLock<String> = RwLock::new(String::new());

/// Set the interface language pages see as `omsi.locale` (ISO 639-1, e.g. `de`; empty = `en`).
pub fn set_locale(iso: &str) {
    *LOCALE.write().unwrap() = iso.trim().to_ascii_lowercase();
}

/// The current `omsi.locale`.
pub fn locale() -> String {
    let l = LOCALE.read().unwrap();
    if l.is_empty() {
        "en".to_string()
    } else {
        l.clone()
    }
}

/// A time of day (seconds, `tod`) on the clock's date as a timestamp: seconds since 1970-01-01
/// 00:00:00 of the simulation's calendar, without a time zone.
pub fn timestamp(clock: &SimClock, tod: f64) -> f64 {
    let leaps_before = |y: i64| (y - 1) / 4 - (y - 1) / 100 + (y - 1) / 400;
    let y = clock.year as i64;
    let days = (y - 1970) * 365 + leaps_before(y) - leaps_before(1970)
        + (clock.day_of_year.max(1) - 1) as i64;
    (days * 86_400) as f64 + tod
}

/// `window.omsi.time`, `.date` and `.locale`: the simulation clock and the interface language.
/// `asString` of the time is `HH:MM:SS`; of the date `DD.MM.YYYY` (`MM/DD/YYYY` for `en`).
pub fn environment(clock: &SimClock, locale: &str) -> ApiValue {
    let secs: i64 = if clock.time.is_finite() {
        clock.time.floor() as i64
    } else {
        0
    };
    let t = secs.rem_euclid(86_400);
    let (h, m, s) = (t / 3600, (t % 3600) / 60, t % 60);
    let (d, mo) = clock.day_month();
    let y = clock.year;
    let date = if locale == "en" {
        format!("{mo:02}/{d:02}/{y:04}")
    } else {
        format!("{d:02}.{mo:02}.{y:04}")
    };
    map(vec![
        (
            "time",
            map(vec![
                ("hour", ApiValue::Num(h as f64)),
                ("minute", ApiValue::Num(m as f64)),
                ("second", ApiValue::Num(s as f64)),
                ("asString", ApiValue::Str(format!("{h:02}:{m:02}:{s:02}"))),
            ]),
        ),
        (
            "date",
            map(vec![
                ("day", ApiValue::Num(d as f64)),
                ("month", ApiValue::Num(mo as f64)),
                ("year", ApiValue::Num(y as f64)),
                ("asString", ApiValue::Str(date)),
            ]),
        ),
        ("locale", ApiValue::Str(locale.to_string())),
        (
            "timestamp",
            ApiValue::Num(timestamp(clock, secs.rem_euclid(86_400) as f64)),
        ),
    ])
}

/// `window.omsi.departures`: per stop key the departures as `{ line, destination, time }`, `time`
/// being a timestamp on the scale of `omsi.timestamp` (see [`timestamp`]).
pub fn departures(
    by_key: &std::collections::HashMap<String, Vec<(String, String, f64)>>,
) -> ApiValue {
    let mut keys: Vec<&String> = by_key.keys().collect();
    keys.sort();
    ApiValue::Map(
        keys.into_iter()
            .map(|k| {
                let list = by_key[k]
                    .iter()
                    .map(|(line, destination, time)| {
                        map(vec![
                            ("line", ApiValue::Str(line.clone())),
                            ("destination", ApiValue::Str(destination.clone())),
                            ("time", ApiValue::Num(time.round())),
                        ])
                    })
                    .collect();
                (k.clone(), ApiValue::List(list))
            })
            .collect(),
    )
}

/// `x` rounded to `dp` decimals, as the number a page prints without float noise
/// (`12.3`, not `12.300000190734863`).
fn round_dp(x: f64, dp: i32) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    let p = 10f64.powi(dp);
    (x * p).round() / p
}

fn num(x: f32, dp: i32) -> ApiValue {
    ApiValue::Num(round_dp(x as f64, dp))
}

fn opt(x: Option<f32>, dp: i32) -> ApiValue {
    x.map_or(ApiValue::Null, |v| num(v, dp))
}

fn map(items: Vec<(&str, ApiValue)>) -> ApiValue {
    ApiValue::Map(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// `HH:MM` of a time in seconds since midnight (wraps at 24 h).
fn hhmm(sec: f64) -> String {
    let s = if sec.is_finite() {
        sec.round() as i64
    } else {
        0
    };
    let s = s.rem_euclid(86_400);
    format!("{:02}:{:02}", s / 3600, (s % 3600) / 60)
}

/// The name a depot file gives a stop of a route list (its first display string), else the
/// ident without its `#` suffix.
fn stop_name(h: &Hof, ident: &str) -> String {
    let id = ident.split('#').next().unwrap_or("").trim();
    h.bus_stops
        .iter()
        .find(|b| b.ident.trim().eq_ignore_ascii_case(id))
        .and_then(|b| b.strings.first())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| id.to_string())
}

/// Everything [`route`] is made from.
pub struct RouteInputs<'a> {
    pub var: &'a dyn Fn(&str) -> Option<f32>,
    pub text: &'a dyn Fn(&str) -> String,
    pub hof: Option<&'a Hof>,
    /// The timetable's line, its delay (s) and stops (name, arrival, departure in s after
    /// midnight), and the index of the stop the bus is at or heading for.
    pub line: &'a str,
    pub delay_s: f32,
    pub stops: &'a [(String, f32, f32)],
    pub next: i32,
}

fn stop_value(
    k: usize,
    name: &str,
    times: Option<(f64, f64)>,
    served: bool,
    current: bool,
) -> ApiValue {
    let (arr, dep) = match times {
        Some((a, d)) => (Some(a), Some(d)),
        None => (None, None),
    };
    let clock = |x: Option<f64>| x.map_or(ApiValue::Null, |x| ApiValue::Str(hhmm(x)));
    let secs = |x: Option<f64>| x.map_or(ApiValue::Null, |x| ApiValue::Num(x.round()));
    map(vec![
        ("index", ApiValue::Num(k as f64)),
        ("name", ApiValue::Str(name.trim().to_string())),
        ("arrival", clock(arr)),
        ("departure", clock(dep)),
        ("arrivalSec", secs(arr)),
        ("departureSec", secs(dep)),
        ("served", ApiValue::Bool(served)),
        ("current", ApiValue::Bool(current)),
    ])
}

/// `omsi.vehicle.route`: line, destination, the stops with their planned times, the stop
/// the bus is at and the last one. Without a timetable the stops are those of the IBIS's
/// route (without times), when the bus has one.
pub fn route(i: &RouteInputs) -> ApiValue {
    let var = i.var;
    let n = i.stops.len();
    let active = n > 0;
    let next = if active {
        i.next.clamp(0, n as i32 - 1) as usize
    } else {
        0
    };
    let ibis_line = var("IBIS_LinieKurs");
    let route_index = var("IBIS_RouteIndex")
        .filter(|r| *r >= 0.0)
        .map(|r| r.round() as usize);

    let (stops, source): (Vec<ApiValue>, &str) = if active {
        let list = i
            .stops
            .iter()
            .enumerate()
            .map(|(k, (name, arr, dep))| {
                stop_value(
                    k,
                    name,
                    Some((*arr as f64, *dep as f64)),
                    k < next,
                    k == next,
                )
            })
            .collect();
        (list, "timetable")
    } else if let (Some(h), Some(r)) = (i.hof, route_index) {
        let idents = h
            .info_busstop_lists
            .get(r)
            .map(|l| l.as_slice())
            .unwrap_or(&[]);
        let list = idents
            .iter()
            .enumerate()
            .map(|(k, id)| stop_value(k, &stop_name(h, id), None, false, false))
            .collect();
        (list, "ibis")
    } else {
        (Vec::new(), "none")
    };

    let current = if active {
        stops.get(next).cloned().unwrap_or(ApiValue::Null)
    } else {
        let name = (i.text)("act_busstop");
        if name.trim().is_empty() {
            ApiValue::Null
        } else {
            stop_value(0, &name, None, false, true).with("index", ApiValue::Null)
        }
    };
    let terminus = stops.last().cloned().unwrap_or(ApiValue::Null);

    let line = if !i.line.trim().is_empty() {
        i.line.trim().to_string()
    } else {
        (i.text)("IBIS_Complex_Line").trim().to_string()
    };
    let line = if line.is_empty() {
        ibis_line
            .filter(|n| *n > 0.5)
            .map(|n| format!("{}", n.round() as i64))
            .unwrap_or_default()
    } else {
        line
    };

    map(vec![
        ("active", ApiValue::Bool(active)),
        ("source", ApiValue::Str(source.to_string())),
        ("line", ApiValue::Str(line)),
        (
            "destination",
            ApiValue::Str((i.text)("IBIS_terminus_name").trim().to_string()),
        ),
        ("delaySec", num(i.delay_s, 0)),
        (
            "nextIndex",
            if active {
                ApiValue::Num(next as f64)
            } else {
                ApiValue::Null
            },
        ),
        ("current", current),
        ("terminus", terminus),
        ("stops", ApiValue::List(stops)),
        (
            "ibis",
            map(vec![
                ("line", opt(ibis_line, 0)),
                ("suffix", opt(var("IBIS_Linie_Suffix"), 0)),
                ("routeIndex", opt(var("IBIS_RouteIndex"), 0)),
                ("terminusIndex", opt(var("IBIS_TerminusIndex"), 0)),
                ("terminusCode", opt(var("IBIS_TerminusCode"), 0)),
            ]),
        ),
    ])
}

/// `omsi.depot`: what the bus's depot file offers a driver, for `omsi.setRoute(index)`
/// and `omsi.setDestination(index)`.
///
/// * `routes[i]`: `index`, `code` (the IBIS route code), `name`, `line`, `terminusCode`,
///   `destinationIndex` (-1: none), `destination` (the sign's text), `first`, `last`,
///   `stops[]` (names).
/// * `lines[]`: `line` and its `routes[]` (the same objects), in the depot file's order.
/// * `destinations[i]`: `index`, `code`, `id`, `name`.
pub fn depot(h: &Hof) -> ApiValue {
    let first_string = |ti: Option<usize>| {
        ti.and_then(|t| h.termini.get(t))
            .map(|t| t.display_name())
            .unwrap_or_default()
    };
    let mut routes: Vec<ApiValue> = Vec::new();
    let mut lines: Vec<(String, Vec<ApiValue>)> = Vec::new();
    for (i, t) in h.info_trips.iter().enumerate() {
        let terminus_code = omsi_cfg::parse_i32(&t.route);
        let ti = h.termini.iter().position(|x| x.code == terminus_code);
        let names: Vec<String> = h
            .info_busstop_lists
            .get(i)
            .map(|l| l.iter().map(|s| stop_name(h, s)).collect())
            .unwrap_or_default();
        let code = omsi_cfg::parse_i32(&t.code);
        // `code` = line x 100 + route (the depot file's own rule); many depot files leave the
        // line column as a placeholder ("XXX"), so the line then comes from the code
        let raw_line = t.line.trim();
        let placeholder = raw_line.is_empty() || raw_line.chars().all(|c| c == 'x' || c == 'X');
        let line = if placeholder && code >= 100 {
            (code / 100).to_string()
        } else {
            raw_line.to_string()
        };
        let r = map(vec![
            ("index", ApiValue::Num(i as f64)),
            ("code", ApiValue::Num(code as f64)),
            ("name", ApiValue::Str(t.name.trim().to_string())),
            ("line", ApiValue::Str(line.clone())),
            ("terminusCode", ApiValue::Num(terminus_code as f64)),
            (
                "destinationIndex",
                ApiValue::Num(ti.map_or(-1.0, |x| x as f64)),
            ),
            ("destination", ApiValue::Str(first_string(ti))),
            (
                "first",
                ApiValue::Str(names.first().cloned().unwrap_or_default()),
            ),
            (
                "last",
                ApiValue::Str(names.last().cloned().unwrap_or_default()),
            ),
            (
                "stops",
                ApiValue::List(names.into_iter().map(ApiValue::Str).collect()),
            ),
        ]);
        match lines
            .iter_mut()
            .find(|(l, _)| l.eq_ignore_ascii_case(&line))
        {
            Some((_, v)) => v.push(r.clone()),
            None => lines.push((line, vec![r.clone()])),
        }
        routes.push(r);
    }
    let destinations: Vec<ApiValue> = h
        .termini
        .iter()
        .enumerate()
        .map(|(i, t)| {
            map(vec![
                ("index", ApiValue::Num(i as f64)),
                ("code", ApiValue::Num(t.code as f64)),
                ("id", ApiValue::Str(t.texture_id.clone())),
                ("name", ApiValue::Str(t.menu_name())),
            ])
        })
        .collect();
    log::info!(
        "omsi.depot '{}': {} trip(s), {} line(s), {} destination(s), {} busstop(s), {} busstop list(s)",
        h.name.trim(),
        h.info_trips.len(),
        lines.len(),
        h.termini.len(),
        h.bus_stops.len(),
        h.info_busstop_lists.len()
    );
    if h.info_trips.is_empty() {
        log::warn!(
            "omsi.depot '{}': no [infosystem_trip] entries parsed, omsi.depot.routes stays empty",
            h.name.trim()
        );
    }
    for (i, t) in h.info_trips.iter().enumerate().take(40) {
        log::info!(
            "omsi.depot route #{i}: code={:?} name={:?} route={:?} line(file)={:?}",
            t.code,
            t.name,
            t.route,
            t.line
        );
    }
    map(vec![
        ("name", ApiValue::Str(h.name.clone())),
        (
            "lines",
            ApiValue::List(
                lines
                    .into_iter()
                    .map(|(line, rs)| {
                        map(vec![
                            ("line", ApiValue::Str(line)),
                            ("routes", ApiValue::List(rs)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("routes", ApiValue::List(routes)),
        ("destinations", ApiValue::List(destinations)),
    ])
}

impl VehicleInstance {
    /// The normalised state of this vehicle for its HTML textures (see the module docs).
    pub fn html_env_snapshot(&self) -> ApiValue {
        environment(&self.host.clock, &locale())
    }

    /// The normalised state of this vehicle for its HTML textures (see the module docs).
    pub fn html_api_snapshot(&self) -> ApiValue {
        let var = |n: &str| self.var(n);
        let text = |n: &str| self.str_var(n);
        let mut api = snapshot(&Inputs {
            var: &var,
            text: &text,
            speed_kmh: self.motion().velocity_kmh(),
            steer_deg: self.motion().steer_deg,
            heading: self.heading,
            pitch: self.pitch,
            bank: self.bank,
            position: (self.position.x, self.position.y, self.position.z),
            engine_running: crate::startup::engine_running(self),
            interior_light: self.interior_light(),
            crashes: self.crashes,
            last_impact_j: self.last_impact,
            dirt: self.dirt,
            trailers: self.trailers.len(),
        });
        let route = route(&RouteInputs {
            var: &var,
            text: &text,
            hof: self.host.hof.as_deref(),
            line: &self.host.tt_line,
            delay_s: self.host.tt_delay,
            stops: &self.host.tt_stops,
            next: self.host.tt_busstop_index,
        });
        if let ApiValue::Map(m) = &mut api {
            m.push(("route".to_string(), route));
        }
        api
    }
}

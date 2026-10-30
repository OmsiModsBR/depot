//! The chooser's lists: their lines, what an open list tells the interface besides them, and doing a line.

use super::duty::{
    driver_names, end_duty, line_sign, natural, sorted_tours, start_duty_at, switch_driver,
    tour_listed, tour_start, tour_trip_name,
};
use super::keyboard::keyboard_pages;
use super::pages::pages_of;
use super::route::{destination_line, line_shown, route_numbers, set_route_by_hand};
use super::rows::{opens, row};
use super::store::remember_bus_seat;
use super::values::option_do;
use super::vehicles::{
    bus_cmp, fleet_numbers, hof_label, place_depots, place_liveries, place_types, place_vehicles,
};
use crate::core::app::App;
use crate::hud::domain::lists::{ListKind, Move};

pub(crate) fn items(app: &App, kind: &ListKind) -> Vec<(String, String)> {
    let tr = |t: &str| omsi_ui::tr(t).into_owned();
    let mut out: Vec<(String, String)> = Vec::new();
    match kind {
        ListKind::Admin => return crate::admin::items(app),
        ListKind::Options(_) | ListKind::Vehicle(_) | ListKind::World(_) => {
            let Some((mut pages, tab)) = pages_of(app, kind) else {
                return out;
            };
            if pages.is_empty() {
                return vec![(
                    row("Nothing to set here", 'i', "", "", None),
                    "noop".to_string(),
                )];
            }
            return pages.swap_remove(tab).1;
        }
        ListKind::ControllerDevices(_)
        | ListKind::Controller(..)
        | ListKind::ControllerAxis(..)
        | ListKind::ControllerButtons(_)
        | ListKind::ControllerButtonSettings(..)
        | ListKind::ControllerButton(..)
        | ListKind::ControllerCapture(_) => return crate::game_controller_menu::items(app, kind),
        ListKind::Controls => {
            return vec![
                opens(
                    "Keyboard",
                    "Change driving, vehicle, game and camera key bindings",
                    "keyboard",
                ),
                opens(
                    "Game controllers",
                    "Configure wheels, pedals, gamepads and force feedback",
                    "controllers",
                ),
            ];
        }
        ListKind::Keyboard(tab) => {
            return keyboard_pages(app).swap_remove((*tab).min(1)).1;
        }
        ListKind::Events => {
            let names = crate::describe::names(&app.args.root, &app.settings.language);
            let mut events = names.events();
            if let Some(p) = app.player.as_ref() {
                for action in p.vehicle.ty.program.trigger_names() {
                    if !events.iter().any(|(a, _)| a.eq_ignore_ascii_case(&action)) {
                        events.push((action.clone(), names.control(&action)));
                    }
                }
            }
            events.sort_by(|a, b| {
                a.1.to_ascii_lowercase()
                    .cmp(&b.1.to_ascii_lowercase())
                    .then_with(|| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()))
            });
            for (action, label) in events {
                out.push((
                    format!("{label}  ·  KY_{action}"),
                    format!("key_event {action}"),
                ));
            }
            if out.is_empty() {
                out.push(("No vehicle events were found".into(), "back".into()));
            }
            return out;
        }
        ListKind::Lines => {
            if let Some(sch) = app.schedule.as_ref() {
                let mut lines: Vec<&omsi_timetable::Line> = sch
                    .data
                    .lines
                    .iter()
                    .filter(|l| {
                        l.user_allowed
                            && l.tours
                                .iter()
                                .any(|t| tour_listed(sch, &l.name, t, app.clock.time))
                    })
                    .collect();
                lines.sort_by(|a, b| natural(&a.name, &b.name));
                for l in lines {
                    out.push((
                        format!(
                            "{} {}  ({} {})",
                            tr("Line"),
                            l.name,
                            l.tours
                                .iter()
                                .filter(|t| tour_listed(sch, &l.name, t, app.clock.time))
                                .count(),
                            tr("tours")
                        ),
                        format!("line {}", l.name),
                    ));
                }
            }
            if out.is_empty() {
                out.push((tr("No timetable on this map"), "back".into()));
            }
        }
        ListKind::Tours(line, _) => {
            if let Some(l) = app
                .schedule
                .as_ref()
                .and_then(|s| s.data.lines.iter().find(|l| l.name == *line))
            {
                for t in sorted_tours(l).into_iter().filter(|t| {
                    app.schedule
                        .as_ref()
                        .is_some_and(|s| tour_listed(s, line, t, app.clock.time))
                }) {
                    // (the tours in order of the time they start)
                    out.push((
                        format!("{} {}", tr("Tour"), t.number.trim()),
                        format!("tour {}\u{1}{}", line, t.number),
                    ));
                }
            }
        }
        ListKind::Drivers => {
            for name in driver_names(app) {
                let mark = if app
                    .career
                    .path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(&name))
                {
                    format!("  {}", tr("(now)"))
                } else {
                    String::new()
                };
                out.push((format!("{name}{mark}"), format!("driver {name}")));
            }
        }
        ListKind::Destinations => {
            if let Some(p) = app.player.as_ref().filter(|p| p.vehicle.host.hof.is_some()) {
                let now = line_shown(&p.vehicle).unwrap_or_else(|| "-".into());
                out.push((format!("{}: {now}...", tr("Route number")), "routes".into()));
            }
            if let Some(hof) = app.player.as_ref().and_then(|p| p.vehicle.host.hof.clone()) {
                let mut termini: Vec<(String, i32, usize)> = hof
                    .termini
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (t.menu_name(), t.code, i))
                    .collect();
                // (alphabetical, by name; picked by its row: several may share a name or a code)
                termini.sort_by_key(|(name, ..)| name.trim().to_lowercase());
                for (name, code, i) in termini {
                    out.push((format!("{:>3}  {}", code, name.trim()), format!("dest {i}")));
                }
            }
            if out.is_empty() {
                out.push((
                    tr("This bus has no depot file (.hof) with destinations"),
                    "back".into(),
                ));
            }
        }
        ListKind::RouteNumbers => {
            // any route number, typed as on OMSI's own field (#836): the scripts that read
            // it (a bus that switches its functions by route number) take what is typed
            match app.menu_edit.as_ref() {
                Some(t) => out.push((
                    format!(
                        "{}: {t}_  ({})",
                        tr("Route number"),
                        tr("Enter sets it, Esc cancels")
                    ),
                    "route_type".into(),
                )),
                None => out.push((
                    format!("{}...", tr("Type a route number")),
                    "route_type".into(),
                )),
            }
            for l in route_numbers(app) {
                out.push((format!("{} {l}", tr("Route")), format!("route {l}")));
            }
            if out.len() == 1 {
                out.push((
                    tr("No route numbers in the depot file or the timetable"),
                    "back".into(),
                ));
            }
        }
        ListKind::Hofs => {
            if let Some(p) = app.player.as_ref() {
                let now = p.vehicle.host.hof.as_ref().map(|h| h.path.clone());
                let mut files: Vec<(String, std::path::PathBuf)> =
                    omsi_vehicle::hof::depot_files(p.vehicle.ty.def.dir())
                        .into_iter()
                        .map(|f| (hof_label(&f), f))
                        .collect();
                // (alphabetical)
                files.sort_by_key(|(label, _)| label.to_lowercase());
                for (label, f) in files {
                    let mark = if now.as_ref() == Some(&f) {
                        format!("  {}", tr("(now)"))
                    } else {
                        String::new()
                    };
                    out.push((
                        format!("{label}{mark}"),
                        format!("hof {}", f.to_string_lossy()),
                    ));
                }
            }
            if out.is_empty() {
                out.push((tr("This bus has no depot files (.hof)"), "back".into()));
            }
        }
        ListKind::Spots => {
            if let Some(w) = app.world.as_ref() {
                // (the map's entry points, as the launcher's "Start at" lists them)
                for (i, e) in w.global.entry_points.iter().enumerate() {
                    let label = if e.name.trim().is_empty() {
                        format!("{} {}", tr("entry"), e.index + 1)
                    } else {
                        e.name.trim().to_string()
                    };
                    out.push((label, format!("spot {i}")));
                }
            }
            if out.is_empty() {
                out.push((tr("This map has no entry points"), "back".into()));
            }
        }
        ListKind::PlaceMaker => {
            let all = place_vehicles(app, &tr("Unknown manufacturer"));
            let mut groups: Vec<(String, String, Vec<&(String, String, String, String)>)> =
                Vec::new();
            for v in &all {
                match groups.iter_mut().find(|g| g.0 == v.0) {
                    Some(g) => g.2.push(v),
                    None => groups.push((v.0.clone(), v.1.clone(), vec![v])),
                }
            }
            groups.sort_by(|a, b| bus_cmp(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
            for (key, name, vs) in groups {
                if vs.len() == 1 {
                    // (a manufacturer with one type: that type at once)
                    out.push((
                        format!("{name}  ·  {}", vs[0].2),
                        format!("bus {}", vs[0].3),
                    ));
                } else {
                    out.push((
                        format!("{name}  ({} {})", vs.len(), tr("models")),
                        format!("maker {key}"),
                    ));
                }
            }
        }
        ListKind::PlaceType(key) => {
            let all = place_vehicles(app, &tr("Unknown manufacturer"));
            let types: Vec<(String, String)> = all
                .iter()
                .filter(|v| v.0 == *key)
                .map(|v| (v.2.clone(), v.3.clone()))
                .collect();
            for (label, path) in place_types(types) {
                out.push((label, format!("bus {path}")));
            }
        }
        ListKind::PlaceLivery(bus) => {
            out.push((tr("Random livery"), "livery ".into()));
            for n in place_liveries(app, bus) {
                out.push((n.clone(), format!("livery {n}")));
            }
        }
        ListKind::PlaceHof(bus, _) => {
            out.push((tr("The map's depot file"), "placehof ".into()));
            for (label, name) in place_depots(app, bus) {
                out.push((label, format!("placehof {name}")));
            }
        }
        ListKind::Numbers => {
            if let Some(p) = app.player.as_ref() {
                let mut numbers = fleet_numbers(&p.vehicle);
                // (in order of the numbers)
                numbers.sort_by(|a, b| natural(&a.0, &b.0));
                for (n, reg) in numbers {
                    out.push((
                        if reg.is_empty() {
                            n.clone()
                        } else {
                            format!("{n}  ({reg})")
                        },
                        format!("number {n}\u{1}{reg}"),
                    ));
                }
            }
            if out.is_empty() {
                out.push((tr("This bus has no list of fleet numbers"), "back".into()));
            }
        }
    }
    out.push((tr("Back"), "back".into()));
    out
}

/// A tour of line `line` as the line's preview shows it: "Tour 3  ›  Terminus" (where the
/// trip it has from the game's time `now` on goes) and the time that trip leaves.
pub(crate) fn tour_preview(
    sch: &crate::schedule::Schedule,
    line: &str,
    t: &omsi_timetable::Tour,
    now: f64,
) -> (String, String) {
    let tr = |t: &str| omsi_ui::tr(t).into_owned();
    let hm = |m: f32| format!("{:02}:{:02}", (m / 60.0) as i32 % 24, (m % 60.0) as i32);
    let terminus = |name: &str| {
        sch.data
            .trips
            .iter()
            .find(|x| x.name.eq_ignore_ascii_case(name))
            .map(|x| x.terminus.trim().to_string())
            .unwrap_or_default()
    };
    // (the trip and the time the tour has from the game's time on)
    let next = sch.tour_stops_from(line, &t.number, now).first().cloned();
    let end = match next.as_ref() {
        Some(n) => tour_trip_name(sch, t, n.0)
            .map(|name| terminus(&name))
            .unwrap_or_default(),
        None => t
            .trips
            .first()
            .map(|tt| terminus(&tt.trip))
            .unwrap_or_default(),
    };
    let what = if end.is_empty() {
        format!("{} {}", tr("Tour"), t.number.trim())
    } else {
        format!("{} {}  ›  {}", tr("Tour"), t.number.trim(), end)
    };
    let when = match next {
        Some(n) => hm((n.3 / 60.0) as f32),
        None => t
            .trips
            .first()
            .map(|tt| hm(tt.departure))
            .unwrap_or_default(),
    };
    (what, when)
}

/// What the open list tells the interface besides its lines: the layout it wants, its title
/// and the small line above it, and - for lines and tours - the timetable of the chosen one.
pub(crate) fn menu_extras(
    kind: Option<&ListKind>,
    list: Option<&[(String, String)]>,
    sel: Option<usize>,
    schedule: Option<&crate::schedule::Schedule>,
    now: f64,
) -> (
    crate::hud::MenuKind,
    Option<(String, String)>,
    Option<crate::hud::Preview>,
) {
    use crate::hud::{MenuKind, Preview};
    let Some(sel) = sel else {
        return (MenuKind::Game, None, None);
    };
    let tr = |t: &str| omsi_ui::tr(t).into_owned();
    // (the menu's own labels, translated, without their dots)
    let title = |t: &str| {
        tr(t)
            .trim_end_matches("...")
            .trim_end_matches('…')
            .trim_end()
            .to_string()
    };
    let head = |t: &str| Some((title(t), String::new()));
    let hm = |m: f32| format!("{:02}:{:02}", (m / 60.0) as i32 % 24, (m % 60.0) as i32);
    // a trip's line and terminus
    let trip_of = |name: &str| -> (String, String) {
        schedule
            .and_then(|s| {
                s.data
                    .trips
                    .iter()
                    .find(|x| x.name.eq_ignore_ascii_case(name))
            })
            .map(|x| (x.line.trim().to_string(), x.terminus.trim().to_string()))
            .unwrap_or_default()
    };
    let action = list
        .and_then(|l| l.get(sel))
        .map(|x| x.1.as_str())
        .unwrap_or("");
    // (no kind: the vehicle chooser)
    let Some(kind) = kind else {
        return (MenuKind::List, head("Place a vehicle..."), None);
    };
    match kind {
        ListKind::Options(_) => (MenuKind::Options, head("Options..."), None),
        ListKind::Vehicle(_) => (MenuKind::Options, head("Vehicle options..."), None),
        ListKind::World(_) => (MenuKind::Options, head("World options..."), None),
        ListKind::Controls => (MenuKind::Options, head("Controls..."), None),
        ListKind::Keyboard(_) => (MenuKind::Options, head("Keyboard"), None),
        ListKind::ControllerDevices(_) => (MenuKind::Options, head("Game controllers"), None),
        ListKind::Controller(name, _) => {
            (MenuKind::Options, Some((name.clone(), String::new())), None)
        }
        ListKind::ControllerAxis(name, a) => (
            MenuKind::Options,
            Some((
                omsi_ui::tr("%{device} · Axis %{n}")
                    .replace("%{device}", name)
                    .replace("%{n}", &(a + 1).to_string()),
                String::new(),
            )),
            None,
        ),
        ListKind::ControllerButtons(name) => (
            MenuKind::List,
            Some(("Choose a button".into(), name.clone())),
            None,
        ),
        ListKind::ControllerButtonSettings(name, b) => (
            MenuKind::Options,
            Some((crate::game_controller_menu::button_label(*b), name.clone())),
            None,
        ),
        ListKind::ControllerButton(_, b) => (
            MenuKind::List,
            Some((
                format!(
                    "{}: choose an action",
                    crate::game_controller_menu::button_label(*b)
                ),
                String::new(),
            )),
            None,
        ),
        ListKind::ControllerCapture(_) => (MenuKind::List, head("Assign a physical button"), None),
        ListKind::Events => (MenuKind::List, Some((tr("Add event"), String::new())), None),
        ListKind::Lines => {
            let preview = action.strip_prefix("line ").and_then(|name| {
                let line = schedule?.data.lines.iter().find(|l| l.name == name)?;
                let sch = schedule?;
                let rows = sorted_tours(line)
                    .into_iter()
                    .filter(|t| tour_listed(sch, &line.name, t, now))
                    .map(|t| tour_preview(sch, &line.name, t, now))
                    .collect();
                Some(Preview {
                    title: format!("{} {}", tr("Line"), line.name),
                    meta: format!(
                            "{} {}",
                            line.tours
                                .iter()
                                .filter(|t| schedule
                                    .is_some_and(|s| tour_listed(s, &line.name, t, now)))
                                .count(),
                            tr("tours")
                        ),
                    rows,
                    chosen: None,
                    button: None,
                    time: None,
                })
            });
            (MenuKind::Lines, head("Line and tour..."), preview)
        }
        ListKind::Tours(line_name, pick) => {
            // (the line number of the trip of a tour shown: a tour may run trips of several lines)
            let tour_line = |ln: &str, num: &str| -> Option<String> {
                let sch = schedule?;
                let line = sch.data.lines.iter().find(|l| l.name == ln)?;
                let tour = line.tours.iter().find(|t| t.number == num)?;
                let n_trips = sch.tour_trip_count(ln, num);
                let trip = pick
                    .as_ref()
                    .filter(|p| p.0 == num)
                    .map(|p| p.2)
                    .unwrap_or_else(|| sch.tour_trip_now(ln, num, now))
                    .min(n_trips.saturating_sub(1));
                let stops = sch.tour_trip_stops(ln, num, trip);
                stops
                    .first()
                    .and_then(|s| tour_trip_name(sch, tour, s.0))
                    .map(|n| trip_of(&n).0)
                    .filter(|l| !l.is_empty())
            };
            let preview = action
                .strip_prefix("tour ")
                .and_then(|rest| rest.split_once('\u{1}'))
                .and_then(|(ln, num)| {
                    let sch = schedule?;
                    let line = sch.data.lines.iter().find(|l| l.name == ln)?;
                    let tour = line.tours.iter().find(|t| t.number == num)?;
                    let n_trips = sch.tour_trip_count(ln, num);
                    let trip = pick
                        .as_ref()
                        .filter(|p| p.0 == num)
                        .map(|p| p.2)
                        .unwrap_or_else(|| sch.tour_trip_now(ln, num, now))
                        .min(n_trips.saturating_sub(1));
                    let stops = sch.tour_trip_stops(ln, num, trip);
                    let at = stops
                        .first()
                        .map(|s| s.3)
                        .unwrap_or_else(|| tour_start(tour).unwrap_or(0.0));
                    let chosen = pick
                        .as_ref()
                        .filter(|p| p.0 == num)
                        .map(|p| p.1)
                        .unwrap_or(0)
                        .min(stops.len().saturating_sub(1));
                    let rows = stops
                        .iter()
                        .map(|s| (s.2.trim().to_string(), hm((s.3 / 60.0) as f32)))
                        .collect();
                    let trip_line = tour_line(ln, num).unwrap_or_else(|| line_sign(schedule, line));
                    Some(Preview {
                        title: format!("{} {}", tr("Tour"), num.trim()),
                        meta: format!(
                            "{} {}  ·  {} {}/{}  ·  {}",
                            tr("Line"),
                            trip_line,
                            tr("Trip"),
                            trip + 1,
                            n_trips.max(1),
                            tr("Choose the stop to start from")
                        ),
                        rows,
                        chosen: Some(chosen),
                        button: Some(tr("Start trip")),
                        // (the time of the trip: between the arrows that step through the tour's trips)
                        time: Some(hm((at / 60.0) as f32)),
                    })
                });
            let chosen_line = action
                .strip_prefix("tour ")
                .and_then(|rest| rest.split_once('\u{1}'))
                .and_then(|(ln, num)| tour_line(ln, num));
            let sign = chosen_line
                .or_else(|| {
                    schedule
                        .and_then(|s| s.data.lines.iter().find(|l| l.name == *line_name))
                        .map(|l| line_sign(schedule, l))
                })
                .unwrap_or_else(|| line_name.clone());
            (
                MenuKind::Tours,
                Some((
                    title("Line and tour..."),
                    format!("{} {}", tr("Line"), sign),
                )),
                preview,
            )
        }
        ListKind::Drivers => (MenuKind::List, head("Driver..."), None),
        ListKind::Numbers => (MenuKind::List, head("Fleet number..."), None),
        ListKind::Destinations => (MenuKind::List, head("Destination display..."), None),
        ListKind::RouteNumbers => (
            MenuKind::List,
            Some((tr("Route number"), String::new())),
            None,
        ),
        ListKind::Hofs => (MenuKind::List, head("Depot file (HOF)..."), None),
        ListKind::Spots => (MenuKind::List, head("Teleport to a start point..."), None),
        ListKind::PlaceMaker
        | ListKind::PlaceType(_)
        | ListKind::PlaceLivery(_)
        | ListKind::PlaceHof(..) => (MenuKind::List, head("Place a vehicle..."), None),
        ListKind::Admin => (
            MenuKind::List,
            Some((tr("Administration"), String::new())),
            None,
        ),
    }
}

/// Do a line of the list; returns the list to show next (None: back to the menu).
pub(crate) fn run(app: &mut App, kind: &ListKind, action: &str) -> Option<ListKind> {
    run_move(app, kind, action, Move::Next)
}

/// Do a line of the list, a setting changed as `mv` says (Enter and a click on a line are
/// `Move::Next`; the arrows and a click on a slider or a stepper the others).
pub(crate) fn run_move(app: &mut App, kind: &ListKind, action: &str, mv: Move) -> Option<ListKind> {
    if crate::game_controller_menu::is_controller_list(Some(kind)) {
        return crate::game_controller_menu::run(app, kind, action, mv);
    }
    if action == "back" {
        return match kind {
            ListKind::Tours(..) => Some(ListKind::Lines),
            ListKind::Events => Some(ListKind::Keyboard(0)),
            ListKind::Keyboard(_) => Some(ListKind::Controls),
            ListKind::PlaceType(_) | ListKind::PlaceLivery(_) | ListKind::PlaceHof(..) => {
                Some(ListKind::PlaceMaker)
            }
            _ => None,
        };
    }
    let (verb, arg) = action.split_once(' ').unwrap_or((action, ""));
    match kind {
        ListKind::ControllerDevices(_)
        | ListKind::Controller(..)
        | ListKind::ControllerAxis(..)
        | ListKind::ControllerButtons(_)
        | ListKind::ControllerButtonSettings(..)
        | ListKind::ControllerButton(..)
        | ListKind::ControllerCapture(_) => unreachable!("controller lists handled above"),
        ListKind::Admin => {
            crate::admin::run(app, action);
            Some(ListKind::Admin)
        }
        ListKind::Options(_) | ListKind::World(_) => {
            if verb == "noop" || option_do(app, verb, arg, mv) {
                return Some(kind.clone());
            }
            // (the arrows and a click on a stepper only change values: no buttons)
            let step = matches!(mv, Move::Next);
            match verb {
                "vr_nav_edit" if step && app.vr_active() && app.player.is_some() => {
                    app.start_vr_nav_edit();
                    return None;
                }
                "vr_nav_reset" if step && app.vr_active() && app.player.is_some() => {
                    app.vr_nav_adjust("reset", 1.0);
                    LIST_DIRTY.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                // (the preset, the clouds and the precipitation are picked from a drop-down: `App::chooser_pick`)
                "weather" | "cloudkind" | "precipkind" | "metar_src" | "sel" | "preset"
                | "gfxprofile" | "reset" => {}
                "metar_icao_edit" if step => {
                    if app.menu_edit_icao {
                        app.apply_icao_edit();
                    } else {
                        app.start_icao_edit();
                    }
                }
                // the exact time: Enter starts typing it, and sets it when typed
                "time_edit" if step => {
                    if app.menu_edit.is_some() {
                        app.apply_time_edit();
                    } else if app.is_lan_client() {
                        app.service_msg =
                            Some(("In a LAN session the host sets the clock".into(), 3.0));
                    } else if app.real_time_locked() {
                        app.service_msg = Some((
                            "The time cannot be changed while the real-time sync is on".into(),
                            3.0,
                        ));
                    } else {
                        app.menu_edit = Some(String::new());
                    }
                }
                "seat_reset" if step => {
                    // (this bus's views as its .bus file has them again)
                    app.settings.seat = [0.0; 3];
                    app.settings.seat_pitch_deg = 0.0;
                    remember_bus_seat(app);
                }
                "clock_ontime" if step => {
                    if app.is_lan_client() {
                        app.service_msg =
                            Some(("In a LAN session the host sets the clock".into(), 3.0));
                    } else if let Some(d) = app
                        .player
                        .as_ref()
                        .map(|p| p.vehicle.host.tt_delay as f64)
                        .filter(|d| d.abs() >= 1.0)
                    {
                        // (the delay as it is now, not as the button was drawn: a second click
                        // would otherwise move the clock by the old amount again)
                        app.shift_clock(-d);
                        if let Some(p) = app.player.as_mut() {
                            p.vehicle.host.tt_delay = 0.0;
                        }
                    }
                }
                "clock_set" | "clock_shift" if step => {
                    if app.is_lan_client() {
                        app.service_msg =
                            Some(("In a LAN session the host sets the clock".into(), 3.0));
                    } else if let Ok(secs) = arg.trim().parse::<f64>() {
                        let by = if verb == "clock_set" {
                            secs - app.clock.time
                        } else {
                            secs
                        };
                        app.shift_clock(by);
                    }
                }
                "traffic_clear" if step => {
                    crate::traffic_link::clear_ai_traffic(app);
                }
                other if step => {
                    app.page_action(other);
                    return None;
                }
                _ => {}
            }
            Some(kind.clone())
        }
        ListKind::Vehicle(_) => {
            if matches!(mv, Move::Next) {
                app.page_action(verb);
                return None;
            }
            Some(kind.clone())
        }
        ListKind::Controls => {
            if !matches!(mv, Move::Next) {
                return Some(kind.clone());
            }
            match verb {
                "keyboard" => Some(ListKind::Keyboard(0)),
                "controllers" => Some(ListKind::ControllerDevices(0)),
                _ => Some(kind.clone()),
            }
        }
        ListKind::Keyboard(_) => {
            if !matches!(mv, Move::Next) {
                return Some(kind.clone());
            }
            match verb {
                "key_events" => Some(ListKind::Events),
                "keybind" => {
                    let mut p = arg.split_whitespace();
                    let game = p.next() == Some("g");
                    if let Some(index) = p.next().and_then(|n| n.parse::<usize>().ok()) {
                        app.begin_key_capture(game, index);
                    }
                    Some(kind.clone())
                }
                _ => Some(kind.clone()),
            }
        }
        ListKind::Events => {
            if matches!(mv, Move::Next) && verb == "key_event" {
                app.add_key_event(arg);
                Some(ListKind::Keyboard(0))
            } else {
                Some(ListKind::Events)
            }
        }
        ListKind::Lines => match verb {
            "line" => Some(ListKind::Tours(arg.to_string(), None)),
            "free" => {
                end_duty(app);
                None
            }
            _ => None,
        },
        ListKind::Tours(_, pick) => {
            if let Some((line, tour)) = arg.split_once('\u{1}') {
                let chosen = pick
                    .as_ref()
                    .filter(|p| p.0 == tour)
                    .map(|p| p.1)
                    .unwrap_or(0);
                let trip = pick
                    .as_ref()
                    .filter(|p| p.0 == tour)
                    .map(|p| p.2)
                    .unwrap_or_else(|| {
                        app.schedule
                            .as_ref()
                            .map(|s| s.tour_trip_now(line, tour, app.clock.time))
                            .unwrap_or(0)
                    });
                start_duty_at(app, line, tour, trip, chosen);
            }
            None
        }
        ListKind::Drivers => {
            switch_driver(app, arg);
            Some(ListKind::Drivers)
        }
        ListKind::Hofs => {
            if let Some(p) = app.player.as_mut() {
                match omsi_vehicle::Hof::load(std::path::Path::new(arg)) {
                    Ok(h) => {
                        let name = h.name.clone();
                        p.vehicle.host.hof = Some(std::sync::Arc::new(h));
                        app.service_msg = Some((format!("Depot file: {}", name.trim()), 3.0));
                    }
                    Err(e) => app.service_msg = Some((format!("Depot file: {e}"), 4.0)),
                }
            }
            None
        }
        ListKind::Spots => {
            if app.is_lan_client() {
                app.service_msg = Some((
                    "In a LAN session only the host moves vehicles on the map".into(),
                    4.0,
                ));
                return None;
            }
            let found = app.world.clone().and_then(|w| {
                let ep = arg
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| w.global.entry_points.get(i))?;
                // (the entry points of tiles that are not loaded come from the map index)
                w.index();
                w.entry_point_place(ep).map(|(pos, rot)| (pos, rot[0]))
            });
            match found {
                Some((pos, heading)) => {
                    crate::teleport::teleport(app, pos, heading);
                    app.service_msg = Some(("The bus stands at the start point".into(), 3.0));
                }
                None => app.service_msg = Some(("That start point is not in the map".into(), 3.0)),
            }
            None
        }
        ListKind::PlaceMaker | ListKind::PlaceType(_) if verb == "maker" => {
            Some(ListKind::PlaceType(arg.to_string()))
        }
        ListKind::PlaceMaker | ListKind::PlaceType(_) => {
            Some(ListKind::PlaceLivery(arg.to_string()))
        }
        ListKind::PlaceLivery(bus) => Some(ListKind::PlaceHof(bus.clone(), arg.to_string())),
        ListKind::PlaceHof(bus, paint) => {
            let (bus, paint, hof) = (bus.clone(), paint.clone(), arg.trim().to_string());
            app.close_game_menu();
            app.place_vehicle(
                &bus,
                Some(paint).filter(|p| !p.is_empty()),
                Some(hof).filter(|h| !h.is_empty()),
            );
            None
        }
        ListKind::Destinations if verb == "routes" => Some(ListKind::RouteNumbers),
        ListKind::RouteNumbers if verb == "route_type" => {
            // the first press starts typing, the next one (Enter) sets what is typed
            match app.menu_edit.take() {
                Some(t) => {
                    set_route_by_hand(app, &t);
                    None
                }
                None => {
                    app.menu_edit = Some(String::new());
                    Some(ListKind::RouteNumbers)
                }
            }
        }
        ListKind::RouteNumbers => {
            set_route_by_hand(app, arg);
            None
        }
        ListKind::Destinations => {
            if let Some(p) = app.player.as_mut() {
                let hof = p.vehicle.host.hof.clone();
                let ti: usize = arg.trim().parse().unwrap_or(usize::MAX);
                if let Some((hof, t)) = hof.as_ref().and_then(|h| h.termini.get(ti).map(|t| (h, t)))
                {
                    // (the line on the IBIS stays; only the destination changes)
                    let line = destination_line(&p.vehicle);
                    let name = t.menu_name();
                    p.set_destination_by_hand(hof, &line, ti);
                    log::info!(
                        "destination display set by hand: {} {} (terminus code now {:?})",
                        t.code,
                        name.trim(),
                        p.vehicle.var("IBIS_TerminusCode")
                    );
                    app.service_msg = Some((format!("Destination: {}", name.trim()), 3.0));
                }
            }
            None
        }
        ListKind::Numbers => {
            if let (Some((n, reg)), Some(p)) = (arg.split_once('\u{1}'), app.player.as_mut()) {
                let v = &mut p.vehicle;
                if let Some(i) = v.ty.program.str_var("number") {
                    v.state.str_vars[i as usize] = n.to_string();
                }
                if !reg.is_empty() {
                    if let Some(i) = v.ty.program.str_var("ident") {
                        v.state.str_vars[i as usize] = reg.to_string();
                    }
                }
                app.service_msg = Some((format!("Fleet number {n}"), 3.0));
            }
            None
        }
    }
}

/// Set by `option_do` when a value really changed (the open list is out of date then).
pub(crate) static LIST_DIRTY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

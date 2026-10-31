//! The next start the launcher prepares (`omsi_launcher_lib::start`): the choices remembered
//! between launches, the seasons, the duty for the game and the trip it begins with.

use omsi_launcher_lib::start::*;
use omsi_launcher_lib::{Duty, TourInfo, TripInfo, WeatherInfo};

/// `launcher-duty.json` from before the number plate field: the missing key falls back to
/// the default (no plate), and a typed plate survives a round trip.
#[test]
fn a_remembered_join_is_not_resumed_at_launch() {
    let saved: Choice =
        serde_json::from_str(r#"{"lan_mode":"join","lan_addr":"main.example.org"}"#).unwrap();
    assert_eq!(saved.lan_mode, "join");
    assert_eq!(Choice::fresh(saved, false).lan_mode, "off");
    let host: Choice = serde_json::from_str(r#"{"lan_mode":"host"}"#).unwrap();
    assert_eq!(Choice::fresh(host, false).lan_mode, "host");
}

/// A choice the Drive page left (it always held a bus) is no preselection on a computer:
/// only a bus picked on Vehicles is.
#[test]
fn the_drive_pages_bus_is_not_a_preselection() {
    let old: Choice = serde_json::from_str(r#"{"bus":"Vehicles/x.bus","paint":"Red","number":"12","plate":"AB","hof":"H","hof_manual":true,"version":2}"#).unwrap();
    let c = Choice::fresh(old, false);
    assert_eq!(
        (
            c.bus.as_str(),
            c.paint.as_str(),
            c.number.as_str(),
            c.plate.as_str(),
            c.hof_manual
        ),
        ("", "", "", "", false)
    );
    assert_eq!(c.version, 3);
    // (one picked since stays)
    let mut now = Choice::default();
    now.bus = "Vehicles/y.bus".into();
    assert_eq!(Choice::fresh(now, false).bus, "Vehicles/y.bus");
}

/// "Use for the next start" puts the card's choices in the next start, a depot file
/// picked by hand among them; Clear leaves nothing of it, the hand-picked depot file
/// included, so the duty carries the automatic one.
#[test]
fn a_used_bus_and_its_clear() {
    let mut c = Choice::default();
    c.hof = "Automatic".into();
    c.use_bus(
        "Vehicles/A/a.bus".into(),
        "Red".into(),
        Some("Picked".into()),
        Some("3201".into()),
        "B-V 1".into(),
    );
    assert_eq!(
        (
            c.bus.as_str(),
            c.paint.as_str(),
            c.hof.as_str(),
            c.number.as_str(),
            c.plate.as_str()
        ),
        ("Vehicles/A/a.bus", "Red", "Picked", "3201", "B-V 1")
    );
    assert!(c.hof_manual);
    c.clear_bus();
    assert_eq!(
        (
            c.bus.as_str(),
            c.paint.as_str(),
            c.number.as_str(),
            c.plate.as_str()
        ),
        ("", "", "", "")
    );
    assert!(!c.hof_manual);
    // (automatic depot file and no number: nothing picked by hand)
    c.use_bus(
        "Vehicles/A/a.bus".into(),
        String::new(),
        None,
        None,
        String::new(),
    );
    assert!(!c.hof_manual);
    assert_eq!(c.number, "");
}

#[test]
fn an_old_duty_file_loads_and_a_typed_plate_is_kept() {
    let old: Choice =
        serde_json::from_str(r#"{"bus":"Vehicles/x.bus","map":"maps/x/global.cfg"}"#).unwrap();
    assert_eq!(old.plate, "");
    let mut c = Choice::default();
    c.plate = "B-AB 1234".into();
    let back: Choice = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
    assert_eq!(back.plate, "B-AB 1234");
}

#[test]
fn a_season_keeps_the_day_within_its_month() {
    assert_eq!(date_in_month("2026-03-31", "04"), "2026-04-30");
    assert_eq!(date_in_month("2026-03-31", "07"), "2026-07-31");
    assert_eq!(date_in_month("2026-11-05", "01"), "2026-01-05");
}

/// A start without a bus leaves the bus, its paint, number and plate and a depot file picked
/// for it out of the duty, on foot - the choice itself keeps them for the next start.
#[test]
fn a_start_without_a_bus_leaves_the_bus_out_of_the_duty_only() {
    let mut d = Duty {
        bus: "Vehicles/A/a.bus".into(),
        paint: Some("Red".into()),
        number: Some("3201".into()),
        plate: Some("B-V 1".into()),
        hof: Some("Picked".into()),
        on_foot: Some(false),
        ..Default::default()
    };
    without_bus(&mut d, Some("Map's own".into()));
    assert!(d.bus.is_empty());
    assert_eq!((&d.paint, &d.number, &d.plate), (&None, &None, &None));
    assert_eq!(d.hof.as_deref(), Some("Map's own"));
    assert_eq!(d.on_foot, Some(true));
    // (the arguments of a duty without a bus: omsi-launcher-core's a_start_without_a_bus_names_none)
}

fn trip(index: usize, departure: f64) -> TripInfo {
    TripInfo {
        name: format!("trip {index}"),
        index,
        line: "137".into(),
        from: "A".into(),
        terminus: "B".into(),
        departure,
        arrival: departure + 1800.0,
        stops: Vec::new(),
        km: 10.0,
    }
}

fn tour(trips: Vec<TripInfo>) -> TourInfo {
    TourInfo {
        number: "1".into(),
        ai_group: String::new(),
        first: trips.first().map(|t| t.departure).unwrap_or(0.0),
        last: trips.last().map(|t| t.departure).unwrap_or(0.0),
        days: "daily".into(),
        runs: true,
        next_run: None,
        trips,
    }
}

/// A tour starts with the next trip to leave (one that left two minutes ago still counts),
/// and after the last one with the last.
#[test]
fn a_tour_starts_with_the_next_trip_to_leave() {
    let t = tour(vec![trip(1, 8.0 * 3600.0), trip(2, 9.0 * 3600.0)]);
    assert_eq!(trip_index_at(&t, 7.0 * 3600.0), Some(0));
    assert_eq!(trip_index_at(&t, 8.0 * 3600.0 + 60.0), Some(0));
    assert_eq!(trip_index_at(&t, 8.0 * 3600.0 + 300.0), Some(1));
    assert_eq!(trip_index_at(&t, 23.0 * 3600.0), Some(1));
    assert_eq!(trip_index_at(&tour(Vec::new()), 0.0), None);
}

/// A trip picked holds while the line, the tour and the time stay; at the real time the clock
/// moves on and the trip still holds. Picking moves the start to its departure, except at
/// the real time.
#[test]
fn a_picked_trip_holds_until_the_line_tour_or_time_change() {
    let mut c = Choice {
        line: Some("137".into()),
        tour: Some("1".into()),
        time: 7 * 60,
        ..Default::default()
    };
    assert!(c.pick_trip(2, 9.0 * 3600.0 + 30.0, false));
    assert_eq!(c.time, 9 * 60);
    assert_eq!(c.picked_trip(false), Some(2));
    let t = tour(vec![trip(1, 8.0 * 3600.0), trip(2, 9.0 * 3600.0)]);
    assert_eq!(c.first_trip(&t, false), Some(1));
    c.time = 10 * 60;
    assert_eq!(c.picked_trip(false), None);
    assert_eq!(c.picked_trip(true), Some(2));
    c.tour = Some("2".into());
    assert_eq!(c.picked_trip(true), None);
    // (at the real time the time stays where the clock has it)
    let mut r = Choice {
        line: Some("137".into()),
        tour: Some("1".into()),
        time: 7 * 60,
        ..Default::default()
    };
    assert!(r.pick_trip(1, 8.0 * 3600.0, true));
    assert_eq!(r.time, 7 * 60);
    // (no line or tour: nothing to pick)
    assert!(!Choice::default().pick_trip(1, 0.0, false));
}

/// The duty carries the choices; a free drive has no line, tour or trip, and the season
/// "auto" is no override.
#[test]
fn the_duty_carries_the_choices() {
    let mut c = Choice {
        map: "maps/x/global.cfg".into(),
        bus: "Vehicles/A/a.bus".into(),
        paint: "Red".into(),
        line: Some("137".into()),
        tour: Some("1".into()),
        time: 9 * 60 + 5,
        ..Default::default()
    };
    let d: Duty = c.to_duty("Spandau.hof".into(), Some(2), "Zane");
    assert_eq!(d.time, "09:05");
    assert_eq!(d.hof.as_deref(), Some("Spandau.hof"));
    assert_eq!(
        (d.line.as_deref(), d.tour.as_deref()),
        (Some("137"), Some("1"))
    );
    assert_eq!(d.trip.as_deref(), Some("2"));
    assert!(d.whole_tour);
    assert_eq!(d.profile.as_deref(), Some("Zane"));
    assert_eq!(d.paint.as_deref(), Some("Red"));
    assert_eq!((d.plate.clone(), d.season.clone()), (None, None));
    assert_eq!(d.lan.as_deref(), Some("off"));
    c.free = true;
    c.season = "winter".into();
    let d = c.to_duty(String::new(), Some(2), "");
    assert_eq!(
        (d.line, d.tour, d.trip, d.hof, d.profile),
        (None, None, None, None, None)
    );
    assert!(!d.whole_tour);
    assert_eq!(d.season.as_deref(), Some("winter"));
}

/// The season by the date, unless one is chosen; a chosen season moves the date into its
/// month.
#[test]
fn the_season_follows_the_date_unless_chosen() {
    let mut c = Choice {
        date: "2026-12-24".into(),
        ..Default::default()
    };
    assert_eq!(c.season(), "winter");
    c.date = "2026-04-01".into();
    assert_eq!(c.season(), "spring");
    c.date = "2026-08-31".into();
    assert_eq!(c.season(), "summer");
    c.date = "2026-10-10".into();
    assert_eq!(c.season(), "autumn");
    c.season = "winter".into();
    assert_eq!(c.season(), "winter");
    assert_eq!(season_month("autumn"), Some("10"));
    assert_eq!(season_month("auto"), None);
}

fn weather(temp: f32, precip: &str, snow: bool) -> WeatherInfo {
    WeatherInfo {
        name: String::new(),
        file: String::new(),
        description: String::new(),
        fog_m: 0.0,
        temp,
        clouds: String::new(),
        precip: precip.into(),
        snow,
        installed: true,
    }
}

/// No summer shower in the snow: a winter takes snow or the cold, the other seasons no snow
/// and no frost, a summer no cold day.
#[test]
fn a_weather_fits_its_season() {
    assert!(weather_fits("winter", &weather(-5.0, "snow", true)));
    assert!(weather_fits("winter", &weather(8.0, "rain", false)));
    assert!(!weather_fits("winter", &weather(12.0, "rain", false)));
    assert!(!weather_fits("spring", &weather(-1.0, "", false)));
    assert!(!weather_fits("autumn", &weather(5.0, "snow", true)));
    assert!(weather_fits("autumn", &weather(5.0, "rain", false)));
    assert!(!weather_fits("summer", &weather(5.0, "", false)));
    assert!(weather_fits("summer", &weather(25.0, "", false)));
}

/// The starred buses are kept by file, in lower case and with '/'.
#[test]
fn the_starred_buses_are_kept_by_file() {
    let f = parse_favourites("Vehicles\\MAN\\a.bus\n\n  vehicles/b.bus \n");
    assert!(f.contains("vehicles/man/a.bus"));
    assert!(f.contains("vehicles/b.bus"));
    assert_eq!(f.len(), 2);
    assert_eq!(favourite_key("Vehicles\\X.bus"), "vehicles/x.bus");
}

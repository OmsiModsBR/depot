use omsi_launcher_lib as core;
use openomsi_game::testing::launcher::timetable_plan::*;

/// A trip of `stops` leaving at `dep` (seconds), two minutes from stop to stop; a stop
/// named with a leading `~` is passed.
fn trip(name: &str, dep: f64, stops: &[&str]) -> core::TripInfo {
    let stops: Vec<core::StopInfo> = stops
        .iter()
        .enumerate()
        .map(|(i, s)| core::StopInfo {
            id: i as i64,
            name: s.trim_start_matches('~').to_string(),
            arr: dep + i as f64 * 120.0,
            dep: dep + i as f64 * 120.0,
            stops: !s.starts_with('~'),
        })
        .collect();
    core::TripInfo {
        name: name.into(),
        index: 1,
        line: String::new(),
        from: stops[0].name.clone(),
        terminus: stops.last().unwrap().name.clone(),
        departure: dep,
        arrival: stops.last().unwrap().arr,
        stops,
        km: 0.0,
    }
}

fn line(name: &str, runs: bool, trips: Vec<core::TripInfo>) -> core::LineInfo {
    let tour = core::TourInfo {
        number: "1".into(),
        ai_group: "Busses".into(),
        first: 0.0,
        last: 0.0,
        days: "daily".into(),
        runs,
        next_run: None,
        trips,
    };
    core::LineInfo {
        name: name.into(),
        user_allowed: true,
        termini: Vec::new(),
        tours: vec![tour],
    }
}

fn town() -> Plan {
    Plan::new(vec![
        line(
            "12",
            true,
            vec![
                trip("out", 3600.0, &["North", "Market", "Hall", "East"]),
                trip("back", 4200.0, &["East", "Hall", "North"]),
                trip("short", 4800.0, &["North", "Market"]),
            ],
        ),
        line(
            "7",
            true,
            vec![trip("a", 3700.0, &["Station", "Hall", "Uni"])],
        ),
        line(
            "N3",
            false,
            vec![trip("n", 3650.0, &["North", "Hall", "Park"])],
        ),
        line(
            "X1",
            true,
            vec![trip("x", 3500.0, &["North", "~Market", "~Hall", "Uni"])],
        ),
    ])
}

#[test]
fn a_direction_is_drawn_from_its_longest_trip() {
    let p = town();
    let k = p.line("12").unwrap();
    assert_eq!(p.directions(k), vec!["East", "North", "Market"]);
    assert_eq!(p.route(k, 0).unwrap().name, "out");
    assert_eq!(p.route(k, 1).unwrap().name, "back");
    // (a short working ends elsewhere: a third direction; and the directions go round)
    assert_eq!(p.route(k, 2).unwrap().name, "short");
    assert_eq!(p.route(k, 4).unwrap().name, "back");
    let empty = Plan::new(vec![line("x", true, Vec::new())]);
    assert!(empty.route(0, 0).is_none());
    assert_eq!(p.trip_from("BACK"), Some("East"));
    // (each drawn trip once, every line's)
    assert_eq!(p.drawn_trips(), vec!["out", "back", "short", "a", "n", "x"]);
}

#[test]
fn departures_are_of_running_tours_and_not_from_the_last_stop_or_a_passed_one() {
    let p = town();
    let (l12, l7) = (p.line("12").unwrap(), p.line("7").unwrap());
    let d = p.departures("hall", 0.0, 10, None);
    // 12 out at 3840, 7 at 3820, 12 back at 4320; N3 does not run that day, X1 passes
    assert_eq!(
        d.iter().map(|x| (x.0 as i64, x.1)).collect::<Vec<_>>(),
        vec![(3820, l7), (3840, l12), (4320, l12)]
    );
    assert_eq!(d[0].2, "Uni");
    // nothing leaves from where the trip ends, and the earlier ones are gone
    assert!(p
        .departures("East", 4000.0, 10, None)
        .iter()
        .all(|x| x.2 == "North"));
    assert_eq!(p.departures("Hall", 4000.0, 10, None).len(), 1);
    assert_eq!(p.departures("Hall", 0.0, 2, None).len(), 2);
    assert_eq!(p.departures("Hall", 0.0, 10, Some(l7)).len(), 1);
    assert!(p.departures("Nowhere", 0.0, 10, None).is_empty());
}

#[test]
fn a_stop_is_served_by_the_lines_that_call_there() {
    let p = town();
    let names = |s: &[(usize, Vec<String>)]| {
        s.iter()
            .map(|x| p.lines[x.0].name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(p.serving("Hall")), vec!["12", "7", "N3"]);
    let l12 = p
        .serving("Hall")
        .iter()
        .find(|x| p.lines[x.0].name == "12")
        .unwrap();
    assert_eq!(l12.1, vec!["East".to_string(), "North".to_string()]);
    // (a line that only ends there, or passes it, does not serve it)
    assert!(p.serving("Uni").is_empty());
    assert_eq!(names(p.serving("market")), vec!["12"]);
    // every stop is known by name, for the search, the ends of the lines too
    assert!(p.stop("uni").is_some() && p.stop("Park").is_some());
    assert_eq!(
        p.find("a", 10),
        vec!["East", "Hall", "Market", "Park", "Station"]
    );
    assert_eq!(p.find("ma", 10), vec!["Market"]);
    assert!(p.find("  ", 10).is_empty());
}

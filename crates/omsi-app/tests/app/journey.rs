use omsi_ui::ingame::StopState;
use openomsi_game::testing::journey::*;
use openomsi_game::testing::schedule::{PlannedStop, PlannedTrip};

fn trip(stops: &[(&str, f64)]) -> PlannedTrip {
    PlannedTrip {
        name: "5_A".into(),
        line: "5".into(),
        terminus: "Muzealna".into(),
        departure: stops[0].1,
        end: stops[stops.len() - 1].1,
        stops: stops
            .iter()
            .map(|(n, t)| PlannedStop {
                object_id: 0,
                name: n.to_string(),
                arr: *t,
                dep: *t,
                position: None,
                dir: Default::default(),
                stops: true,
            })
            .collect(),
    }
}

/// A trip driven stop by stop: the first stop tells its departure only, the last its
/// arrival only, a stop driven past is missed, a late arrival says so - and the file
/// has it all at every stop.
#[test]
fn the_log_has_every_stop_with_its_times() {
    let t0 = 8.0 * 3600.0;
    let trips = [
        trip(&[
            ("Metro Mlociny", t0),
            ("Zajezdnia", t0 + 300.0),
            ("Prozy", t0 + 600.0),
            ("Muzealna", t0 + 900.0),
        ]),
        trip(&[("Muzealna", t0 + 1200.0), ("Metro Mlociny", t0 + 2100.0)]),
    ];
    let dir = std::env::temp_dir().join(format!("omsi-journey-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut j = Journey::new(
        ("5".into(), "1".into(), 0),
        &trips,
        "Driver Test · Grundorf · MAN SD77 · 1989-05-30",
        dir.clone(),
    );
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
    let line = |name: &str| {
        text.lines()
            .find(|l| l.starts_with(name))
            .unwrap_or_else(|| panic!("no {name} in\n{text}"))
            .to_string()
    };
    assert!(
        text.contains("Line 5, tour 1")
            && text.contains("Driver Test")
            && text.contains("Trip 1  ·  5 to Muzealna  ·  08:00:00"),
        "{text}"
    );
    assert!(
        line("Metro Mlociny").contains("08:00:00  08:00:57  +00:00:57"),
        "{text}"
    );
    assert!(line("Metro Mlociny").ends_with("OK"), "{text}");
    assert!(
        line("Zajezdnia").contains("08:05:00  08:09:00  +00:04:00"),
        "{text}"
    );
    assert!(
        line("Zajezdnia").contains("08:05:00  08:09:30  +00:04:30"),
        "{text}"
    );
    assert!(line("Zajezdnia").ends_with("late"), "{text}");
    assert!(line("Prozy").ends_with("missed"), "{text}");
    assert!(
        line("Muzealna").contains("08:15:00  08:14:30  -00:00:30"),
        "{text}"
    );
    assert!(
        !text.contains("Trip 2"),
        "a trip not begun is not in it yet: {text}"
    );
    assert!(
        text.contains("Stops served: 3, late: 1, left early: 0"),
        "{text}"
    );
    // the end-of-trip report shows the same
    let (title, rows, totals) = j.report(0).expect("trip 1");
    assert_eq!(title, "Trip 1  ·  5 to Muzealna  ·  08:00:00");
    let states: Vec<StopState> = rows.iter().map(|r| r.state).collect();
    assert_eq!(
        states,
        vec![
            StopState::OnTime,
            StopState::Late,
            StopState::Missed,
            StopState::OnTime
        ]
    );
    assert_eq!(
        (rows[0].scheduled.as_str(), rows[0].actual.as_str()),
        ("08:00:00", "08:00:57")
    );
    assert_eq!(rows[3].deviation, "-00:00:30");
    assert_eq!(totals[0], ("Stops served", "3 of 4".to_string()));
    assert_eq!(totals[1], ("Late arrivals", "1".to_string()));
    assert!(j.report(5).is_none());
    let _ = std::fs::remove_dir_all(&dir);
    // a duty taken up at its second trip: the first is not in it
    let mut j = Journey::new(("5".into(), "1".into(), 0), &trips, "Grundorf", dir.clone());
    assert!(j.arrived((1, 0), -60.0));
    let text = j.text();
    assert!(
        text.contains("Trip 2  ·  5 to Muzealna  ·  08:20:00") && !text.contains("Trip 1"),
        "{text}"
    );
    // taken up (or resumed) at the third stop: the first two are not missed
    let mut j = Journey::new(("5".into(), "1".into(), 0), &trips, "Grundorf", dir.clone());
    assert!(j.arrived((0, 2), 0.0));
    assert!(j.arrived((0, 3), 0.0));
    let (_, rows, totals) = j.report(0).expect("trip 1");
    let states: Vec<StopState> = rows.iter().map(|r| r.state).collect();
    assert_eq!(
        states,
        vec![
            StopState::None,
            StopState::None,
            StopState::OnTime,
            StopState::OnTime
        ]
    );
    assert_eq!(totals[0], ("Stops served", "2 of 2".to_string()));
    assert!(!j.text().contains("missed"), "{}", j.text());
}

/// A stop the driver skipped has no mark: not missed, not counted against the trip.
#[test]
fn a_skipped_stop_is_neither_missed_nor_counted() {
    let t0 = 8.0 * 3600.0;
    let trips = [trip(&[
        ("Metro Mlociny", t0),
        ("Zajezdnia", t0 + 300.0),
        ("Prozy", t0 + 600.0),
        ("Muzealna", t0 + 900.0),
    ])];
    let mut j = Journey::new(
        ("5".into(), "1".into(), 0),
        &trips,
        "Grundorf",
        std::env::temp_dir(),
    );
    assert!(j.left((0, 0), (0.0, 60.0)));
    assert!(j.skipped((0, 1)));
    assert!(!j.skipped((0, 1)), "told once");
    assert!(j.arrived((0, 3), 120.0));
    let (_, rows, totals) = j.report(0).expect("trip 1");
    let states: Vec<StopState> = rows.iter().map(|r| r.state).collect();
    // (Prozy, passed without a word, is missed as before)
    assert_eq!(
        states,
        vec![
            StopState::OnTime,
            StopState::Skipped,
            StopState::Missed,
            StopState::OnTime
        ]
    );
    assert_eq!(totals[0], ("Stops served", "2 of 3".to_string()));
    assert!(
        j.text()
            .lines()
            .any(|l| l.starts_with("Zajezdnia") && l.ends_with("skipped")),
        "{}",
        j.text()
    );
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
    let trips = [trip(&[
        ("Metro Mlociny", t0),
        ("Zajezdnia", t0 + 300.0),
        ("Muzealna", t0 + 900.0),
    ])];
    let mut j = Journey::new(
        ("5".into(), "1".into(), 0),
        &trips,
        "Grundorf",
        std::env::temp_dir(),
    );
    assert!(j.left((0, 0), (0.0, 0.0)));
    assert!(j.skipped((0, 1)));
    assert!(!j.served((0, 1)), "skipped: not served yet");
    assert!(j.arrived((0, 1), 60.0));
    assert!(j.served((0, 1)));
    assert!(j.arrived((0, 2), 60.0));
    let (_, rows, totals) = j.report(0).expect("trip 1");
    let states: Vec<StopState> = rows.iter().map(|r| r.state).collect();
    assert_eq!(
        states,
        vec![StopState::OnTime, StopState::OnTime, StopState::OnTime]
    );
    assert_eq!(totals[0], ("Stops served", "3 of 3".to_string()));
    assert_eq!(j.how((0, 1)).as_deref(), Some("served, +1 min"));
    assert!(!j.text().contains("skipped"), "{}", j.text());
}

/// A stop reached late and left before its departure time says both.
#[test]
fn a_late_arrival_that_leaves_early_says_both() {
    let t0 = 8.0 * 3600.0;
    let trips = [trip(&[
        ("Metro Mlociny", t0),
        ("Zajezdnia", t0 + 300.0),
        ("Muzealna", t0 + 900.0),
    ])];
    let mut j = Journey::new(
        ("5".into(), "1".into(), 0),
        &trips,
        "Grundorf",
        std::env::temp_dir(),
    );
    // (a stop with a layover: due at 08:05, leaving at 08:10)
    j.rows[1].plan.1 = t0 + 600.0;
    assert!(j.left((0, 0), (0.0, 0.0)));
    assert!(j.left((0, 1), (240.0, -180.0)));
    let (_, rows, totals) = j.report(0).expect("trip 1");
    assert_eq!(rows[1].state, StopState::LateLeftEarly);
    assert_eq!(rows[1].deviation, "+00:04:00");
    assert!(j
        .text()
        .lines()
        .any(|l| l.starts_with("Zajezdnia") && l.ends_with("late, left early")));
    assert_eq!(totals[1], ("Late arrivals", "1".to_string()));
    assert_eq!(totals[2], ("Early departures", "1".to_string()));
}

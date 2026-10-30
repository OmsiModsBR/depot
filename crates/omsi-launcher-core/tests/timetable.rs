//! The timetable editor's data side (`omsi_launcher_lib::timetable`): the departures as typed,
//! the tours' numbers and repeats, and where a changed line is saved and how the map's own
//! timetable comes back.

use omsi_launcher_lib::timetable::*;
use omsi_timetable::{Line, Tour, TourTrip};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A folder of its own under the system's temporary folder, empty.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("depot-tt-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn tour(number: &str, departures: &[f32]) -> Tour {
    Tour {
        number: number.into(),
        ai_group: "Busses".into(),
        extra: String::new(),
        trips: departures
            .iter()
            .enumerate()
            .map(|(k, d)| TourTrip {
                trip: format!("t{k}"),
                profile: 0,
                departure: *d,
            })
            .collect(),
    }
}

/// A map folder `maps/<name>` under `root` whose `TTData` holds the lines `names` (each with
/// one tour leaving at 6:00), and those lines as the editor holds them.
fn map_with_lines(root: &Path, name: &str, names: &[&str]) -> (PathBuf, Vec<Line>) {
    let map = root.join("maps").join(name);
    let tt = map.join("TTData");
    std::fs::create_dir_all(&tt).unwrap();
    let lines = names
        .iter()
        .map(|n| {
            let line = Line {
                path: tt.join(format!("{n}.ttl")),
                name: n.to_string(),
                user_allowed: true,
                priority: 0,
                tours: vec![tour("1", &[360.0])],
            };
            line.save(&line.path).unwrap();
            line
        })
        .collect();
    (map, lines)
}

fn dirty(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

#[test]
fn a_departure_is_read_and_written_as_hours_and_minutes() {
    assert_eq!(parse_time("4:07"), Some(247.0));
    assert_eq!(parse_time("24:10"), Some(1450.0));
    assert_eq!(parse_time("4:7:30"), Some(247.5));
    assert_eq!(parse_time("4.07"), Some(247.0));
    assert_eq!(parse_time("4"), None);
    assert_eq!(parse_time("4:61"), None);
    assert_eq!(parse_time("48:00"), None);
    assert_eq!(fmt_time(247.0), "4:07");
    assert_eq!(fmt_time(247.5), "4:07:30");
}

#[test]
fn a_new_tour_is_numbered_past_the_highest() {
    let t = |n: &str| Tour {
        number: n.into(),
        ..Default::default()
    };
    assert_eq!(next_number(&[t("1"), t("7"), t("x")]), "8");
    assert_eq!(next_number(&[]), "1");
}

#[test]
fn a_tour_every_twenty_minutes_until_eight() {
    let base = tour("1", &[6.0 * 60.0, 6.5 * 60.0]);
    let mut tours = vec![base.clone()];
    let made = repeat_tour(&mut tours, &base, 20.0, 8.0 * 60.0);
    // 6:20, 6:40 … 8:00
    assert_eq!(made, 6);
    assert_eq!(tours.last().unwrap().trips[0].departure, 8.0 * 60.0);
    assert_eq!(tours.last().unwrap().trips[1].departure, 8.5 * 60.0);
    assert_eq!(tours.last().unwrap().number, "7");
}

/// A map unpacked in the content folder is saved in place, its file kept once as `.orig`;
/// the reset puts the `.orig` back.
#[test]
fn a_map_in_the_content_folder_is_saved_in_place_and_put_back() {
    let content = scratch("inplace");
    let (map, mut lines) = map_with_lines(&content, "Town", &["1", "2"]);
    let before = std::fs::read_to_string(&lines[0].path).unwrap();
    lines[0].tours.push(tour("2", &[420.0]));
    let (saved, err) = save_lines(&mut lines, &dirty(&["1"]), &map, Some(&content));
    assert_eq!((saved, err), (1, None));
    let file = map.join("TTData").join("1.ttl");
    assert_eq!(lines[0].path, file);
    assert_eq!(
        std::fs::read_to_string(map.join("TTData").join("1.ttl.orig")).unwrap(),
        before
    );
    assert_ne!(std::fs::read_to_string(&file).unwrap(), before);
    // (a line not changed is not written, and keeps no backup)
    assert!(!map.join("TTData").join("2.ttl.orig").exists());
    // (a second save keeps the first backup: the map's own file)
    lines[0].tours.push(tour("3", &[480.0]));
    save_lines(&mut lines, &dirty(&["1"]), &map, Some(&content));
    assert_eq!(
        std::fs::read_to_string(map.join("TTData").join("1.ttl.orig")).unwrap(),
        before
    );

    assert_eq!(
        reset_timetable(Some(&content), &map, "Town"),
        Ok(Reset::Restored(1))
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    assert_eq!(
        reset_timetable(Some(&content), &map, "Town"),
        Ok(Reset::Unchanged)
    );
    let _ = std::fs::remove_dir_all(&content);
}

/// A map outside the content folder is saved in the content folder's copy of its whole
/// `TTData`, marked as the launcher's; the reset deletes that copy and leaves the map alone.
#[test]
fn a_map_elsewhere_is_saved_in_a_marked_copy_that_the_reset_removes() {
    let root = scratch("copy");
    let content = root.join("content");
    std::fs::create_dir_all(&content).unwrap();
    let (map, mut lines) = map_with_lines(&root.join("game"), "Town", &["1", "2"]);
    let own = std::fs::read_to_string(&lines[0].path).unwrap();
    lines[0].tours.push(tour("2", &[420.0]));
    let (saved, err) = save_lines(&mut lines, &dirty(&["1"]), &map, Some(&content));
    assert_eq!((saved, err), (1, None));
    let copy = content.join("maps").join("Town").join("TTData");
    assert_eq!(lines[0].path, copy.join("1.ttl"));
    assert!(copy.join(COPY_MARK).is_file());
    // (the line not changed is copied too: the game reads the one folder)
    assert!(copy.join("2.ttl").is_file());
    assert_eq!(
        std::fs::read_to_string(map.join("TTData").join("1.ttl")).unwrap(),
        own
    );

    assert_eq!(
        reset_timetable(Some(&content), &map, "Town"),
        Ok(Reset::CopyRemoved)
    );
    assert!(!copy.exists());
    assert!(map.join("TTData").join("1.ttl").is_file());
    let _ = std::fs::remove_dir_all(&root);
}

/// The trips of a saved tour are put in the order they leave.
#[test]
fn a_saved_tour_has_its_trips_in_departure_order() {
    let content = scratch("order");
    let (map, mut lines) = map_with_lines(&content, "Town", &["1"]);
    lines[0].tours = vec![tour("1", &[420.0, 360.0, 390.0])];
    save_lines(&mut lines, &dirty(&["1"]), &map, Some(&content));
    let departures: Vec<f32> = lines[0].tours[0]
        .trips
        .iter()
        .map(|t| t.departure)
        .collect();
    assert_eq!(departures, [360.0, 390.0, 420.0]);
    let _ = std::fs::remove_dir_all(&content);
}

/// Without a content folder nothing is saved, and each line says why.
#[test]
fn without_a_content_folder_nothing_is_saved() {
    let root = scratch("nocontent");
    let (map, mut lines) = map_with_lines(&root, "Town", &["1"]);
    let (saved, err) = save_lines(&mut lines, &dirty(&["1"]), &map, None);
    assert_eq!(saved, 0);
    assert_eq!(err.as_deref(), Some("line 1: no content folder"));
    assert!(reset_timetable(None, &map, "Town").is_err());
    let _ = std::fs::remove_dir_all(&root);
}

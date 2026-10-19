//! #45: a map whose log says "1 lines, 1 trips, 1 tracks". Synthetic `TTData` only (hand-written
//! tiny files in a temporary folder); the reporter's real map is not used or committed.

use omsi_timetable::TimetableData;
use std::path::{Path, PathBuf};

fn map_dir(tag: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!("depot-t45-{tag}-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn write(dir: &Path, file: &str, text: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(file), text).unwrap();
}

const TRIP: &str =
    "[trip]\r\n\r\nTerminus\r\n12\r\n\r\n[station_typ2]\r\n1\r\n[station_typ2]\r\n2\r\n";
const TRACK: &str = "[track_entry]\r\n1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n";
const LINE: &str = "[userallowed]\r\n[priority]\r\n1\r\n[newtour]\r\n1\r\nBusses\r\n799\r\n[addtrip]\r\nT1\r\n0\r\n300\r\n";

/// Many files give many entries: nothing collapses to one.
#[test]
fn each_file_of_ttdata_is_one_entry() {
    let map = map_dir("many");
    let tt = map.join("TTData");
    for i in 1..=5 {
        write(&tt, &format!("T{i}.ttp"), TRIP);
        write(&tt, &format!("R{i}.ttr"), TRACK);
        write(&tt, &format!("{i}.ttl"), LINE);
    }
    let d = TimetableData::load(&map);
    assert_eq!((d.trips.len(), d.tracks.len(), d.lines.len()), (5, 5, 5));
    assert!(d.errors.is_empty(), "{:?}", d.errors);
    let _ = std::fs::remove_dir_all(&map);
}

/// Names with spaces, dots, accents and a different case of the extension are all read.
#[test]
fn awkward_file_names_and_extension_case_are_read() {
    let map = map_dir("names");
    let tt = map.join("TTData");
    write(&tt, "Linia 12.1 A.ttp", TRIP);
    write(&tt, "Łódź-Główna.TTP", TRIP);
    write(&tt, "Track one.TTR", TRACK);
    write(&tt, "12A.TTL", LINE);
    write(&tt, "12.1.ttl", LINE);
    let d = TimetableData::load(&map);
    assert_eq!((d.trips.len(), d.tracks.len(), d.lines.len()), (2, 1, 2));
    let _ = std::fs::remove_dir_all(&map);
}

/// The folder is found when the map ships it as `ttdata` / `TTDATA` (a case-sensitive file system
/// is where a map made on Windows would lose its timetable).
#[test]
fn ttdata_folder_is_found_in_any_letter_case() {
    for dir in ["ttdata", "TTDATA", "TtData"] {
        let map = map_dir("case");
        let tt = map.join(dir);
        write(&tt, "T1.ttp", TRIP);
        write(&tt, "1.ttl", LINE);
        let d = TimetableData::load(&map);
        assert_eq!((d.trips.len(), d.lines.len()), (1, 1), "folder {dir}");
        let _ = std::fs::remove_dir_all(&map);
    }
}

/// A file the loader cannot read is reported with its path, and the others still load.
#[test]
fn an_unreadable_file_is_named_in_the_errors_and_does_not_hide_the_rest() {
    let map = map_dir("bad");
    let tt = map.join("TTData");
    write(&tt, "Good.ttp", TRIP);
    write(&tt, "Good.ttl", LINE);
    // a directory with a timetable extension cannot be read as a file
    std::fs::create_dir_all(tt.join("Broken.ttp")).unwrap();
    let d = TimetableData::load(&map);
    assert_eq!((d.trips.len(), d.lines.len()), (1, 1));
    assert_eq!(d.errors.len(), 1, "{:?}", d.errors);
    assert!(
        d.errors[0].contains("Broken"),
        "the error names the file: {:?}",
        d.errors
    );
    let _ = std::fs::remove_dir_all(&map);
}

/// A map with the stop and link tables but nothing else gives empty trips/lines/tracks and no
/// error: the case the log line "0 lines" would show. Documents the silent part of #45.
#[test]
fn a_ttdata_folder_without_trips_gives_no_error_and_no_trips() {
    let map = map_dir("empty");
    let tt = map.join("TTData");
    write(&tt, "Busstops.cfg", "");
    let d = TimetableData::load(&map);
    assert!(d.trips.is_empty() && d.lines.is_empty() && d.tracks.is_empty());
    assert!(d.errors.is_empty());
    let _ = std::fs::remove_dir_all(&map);
}

/// Hypothesis from #45 (not confirmed): a trip whose text the parser cannot make sense of loads
/// as an empty trip without any message, so a damaged file is invisible in the log. Ignored until
/// the real data says which failure class applies; the wanted behavior is a log line naming the
/// file (the issue's item 3), without refusing valid maps.
#[test]
#[ignore = "hypothesis for #45: needs the reporter's TTData to know the failure class"]
fn a_trip_without_stations_is_reported() {
    let map = map_dir("nostations");
    let tt = map.join("TTData");
    write(&tt, "Empty.ttp", "[trip]\r\n\r\nX\r\n1\r\n");
    let d = TimetableData::load(&map);
    assert_eq!(d.trips.len(), 1);
    assert!(
        d.errors.iter().any(|e| e.contains("Empty")),
        "a trip with no station is named in the errors: {:?}",
        d.errors
    );
    let _ = std::fs::remove_dir_all(&map);
}

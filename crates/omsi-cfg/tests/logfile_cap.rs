//! The size cap of the log files: a log stops at the cap, and its last line says that it was
//! cut (the cap is injected small here; the files' own is 30 MB).

use omsi_cfg::logfile::{truncation_marker, Capped, CappedFile, LOG_CAP_BYTES};
use std::io::Write;

fn temp_log(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("omsi-cfg-logfile-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("a.log")
}

#[test]
fn the_files_cap_is_30_mb_and_the_marker_says_so() {
    assert_eq!(LOG_CAP_BYTES, 30 * 1024 * 1024);
    assert_eq!(
        truncation_marker(LOG_CAP_BYTES),
        "log truncated at 30 MB: the lines after this one were not written\n"
    );
    assert!(truncation_marker(300).starts_with("log truncated at 300 bytes"));
}

#[test]
fn a_log_under_the_cap_is_written_whole() {
    let mut log = Capped::new(Vec::new(), 300);
    log.write_all(b"one\n").unwrap();
    log.write_all(b"two\n").unwrap();
    assert_eq!(log.into_inner(), b"one\ntwo\n");
}

#[test]
fn the_line_that_does_not_fit_becomes_the_marker_and_the_rest_is_dropped() {
    let cap = 300;
    let marker = truncation_marker(cap);
    let line = format!("{}\n", "x".repeat(99));
    let mut log = Capped::new(Vec::new(), cap);
    for _ in 0..50 {
        // (a dropped line is no error: the program goes on)
        log.write_all(line.as_bytes()).unwrap();
    }
    let text = String::from_utf8(log.into_inner()).unwrap();
    assert!(text.len() as u64 <= cap, "{} bytes", text.len());
    assert!(text.ends_with(&marker), "{text}");
    assert_eq!(text.matches("log truncated").count(), 1);
    // whole lines only: two fit beside the marker's room, none is cut
    assert_eq!(text, format!("{line}{line}{marker}"));
}

#[test]
fn the_marker_has_room_even_when_a_line_ends_exactly_at_the_cap() {
    let cap = 300;
    let marker = truncation_marker(cap);
    // a line that would fill the file to the last byte: the marker's room is not given away
    let exact = format!("{}\n", "y".repeat(cap as usize - 1));
    let mut log = Capped::new(Vec::new(), cap);
    log.write_all(exact.as_bytes()).unwrap();
    log.write_all(b"an error after it\n").unwrap();
    assert_eq!(String::from_utf8(log.into_inner()).unwrap(), marker);
}

#[test]
fn a_small_line_after_the_marker_is_dropped_too() {
    let cap = 300;
    let mut log = Capped::new(Vec::new(), cap);
    log.write_all("z".repeat(400).as_bytes()).unwrap();
    log.write_all(b"small\n").unwrap();
    assert_eq!(
        String::from_utf8(log.into_inner()).unwrap(),
        truncation_marker(cap)
    );
}

#[test]
fn a_file_two_programs_append_to_stays_within_the_cap() {
    let path = temp_log("shared");
    // (the slack a shared file keeps free is 1/64 of the cap: 100 bytes here, two lines)
    let cap = 6400;
    let line = format!("{}\n", "w".repeat(49));
    let writers: Vec<_> = (0..2)
        .map(|_| {
            let (path, line) = (path.clone(), line.clone());
            std::thread::spawn(move || {
                let mut log = CappedFile::open(&path, cap).unwrap();
                for _ in 0..200 {
                    log.write_all(line.as_bytes()).unwrap();
                }
            })
        })
        .collect();
    for w in writers {
        w.join().unwrap();
    }
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.len() as u64 <= cap, "{} bytes", text.len());
    assert!(text.contains("log truncated at 6400 bytes"), "{text}");
    // every line is whole: a log line or the marker
    for l in text.lines() {
        assert!(
            l == line.trim_end() || l.starts_with("log truncated"),
            "{l}"
        );
    }
}

#[test]
fn a_capped_file_adds_to_what_is_there() {
    let path = temp_log("append");
    std::fs::write(&path, "before\n").unwrap();
    let mut log = CappedFile::open(&path, 6400).unwrap();
    log.write_all(b"after\n").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "before\nafter\n");
}

#[test]
fn a_shared_file_takes_no_line_longer_than_its_slack() {
    let path = temp_log("long");
    let cap = 6400;
    let mut log = CappedFile::open(&path, cap).unwrap();
    log.write_all(b"short\n").unwrap();
    log.write_all("v".repeat(101).as_bytes()).unwrap();
    log.write_all(b"after\n").unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        format!("short\n{}", truncation_marker(cap))
    );
}

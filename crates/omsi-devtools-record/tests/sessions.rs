//! The record files of a folder: one per session, named by its start, the last five kept.

use omsi_devtools_proto::{Hello, ToLauncher, PROTOCOL_VERSION};
use omsi_devtools_record::{prune, records_in, start_in, SESSIONS_KEPT};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("depot-run-record-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn hello() -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        build: "test".into(),
    }
}

/// 22 November 2026, 10:15:00 UTC, and `minutes` after it.
fn at(minutes: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_795_342_500 + minutes * 60)
}

fn names(dir: &std::path::Path) -> Vec<String> {
    records_in(dir)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn a_session_gets_a_file_named_by_its_start_with_the_header_in_it() {
    let dir = temp_dir("one");
    let (record, path) = start_in(&dir, &hello(), at(0)).unwrap();
    drop(record.finish().unwrap());
    assert_eq!(path, dir.join("run-20261122-101500.jsonl"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("{\"t\":\"run\","), "{text}");
    assert!(
        text.contains("\"started\":\"2026-11-22T10:15:00Z\""),
        "{text}"
    );
    assert_eq!(text.lines().count(), 1);
}

#[test]
fn the_last_five_sessions_are_kept_and_the_oldest_goes_when_a_new_one_starts() {
    let dir = temp_dir("five");
    for minute in 0..SESSIONS_KEPT as u64 {
        let (record, _) = start_in(&dir, &hello(), at(minute)).unwrap();
        drop(record.finish().unwrap());
    }
    assert_eq!(names(&dir).len(), 5);
    assert_eq!(names(&dir)[0], "run-20261122-101500.jsonl");
    // the sixth session: the first one's record goes, at its start
    let (mut sixth, path) = start_in(&dir, &hello(), at(5)).unwrap();
    assert_eq!(
        names(&dir),
        [
            "run-20261122-101600.jsonl",
            "run-20261122-101700.jsonl",
            "run-20261122-101800.jsonl",
            "run-20261122-101900.jsonl",
            "run-20261122-102000.jsonl",
        ]
    );
    // nothing is removed while a record is written
    for _ in 0..50 {
        sixth
            .message(&ToLauncher::Rejected { reason: "x".into() })
            .unwrap();
    }
    drop(sixth.finish().unwrap());
    assert_eq!(names(&dir).len(), 5);
    assert_eq!(path, dir.join("run-20261122-102000.jsonl"));
}

#[test]
fn two_sessions_started_in_the_same_second_get_two_files_in_order() {
    let dir = temp_dir("same");
    let (a, first) = start_in(&dir, &hello(), at(0)).unwrap();
    let (b, second) = start_in(&dir, &hello(), at(0)).unwrap();
    drop((a.finish().unwrap(), b.finish().unwrap()));
    assert_eq!(first, dir.join("run-20261122-101500.jsonl"));
    assert_eq!(second, dir.join("run-20261122-101500-2.jsonl"));
    assert_eq!(records_in(&dir), [first, second]);
}

/// More sessions in one second than are kept: the ones left are always the last five, in
/// the order they started (a number set free by a removal is not used again).
#[test]
fn many_sessions_in_the_same_second_keep_the_last_five_in_order() {
    let dir = temp_dir("same-many");
    let mut last = PathBuf::new();
    for _ in 0..12 {
        let (record, path) = start_in(&dir, &hello(), at(0)).unwrap();
        drop(record.finish().unwrap());
        last = path;
    }
    assert_eq!(last, dir.join("run-20261122-101500-12.jsonl"));
    assert_eq!(
        names(&dir),
        [8, 9, 10, 11, 12].map(|n| format!("run-20261122-101500-{n}.jsonl"))
    );
}

#[test]
fn only_record_files_are_counted_and_removed() {
    let dir = temp_dir("others");
    std::fs::create_dir_all(&dir).unwrap();
    for other in ["game.log", "launcher.log", "run-notes.txt", "settings.cfg"] {
        std::fs::write(dir.join(other), "kept").unwrap();
    }
    for minute in 0..8 {
        let (record, _) = start_in(&dir, &hello(), at(minute)).unwrap();
        drop(record.finish().unwrap());
    }
    assert_eq!(names(&dir).len(), 5);
    prune(&dir, 0);
    assert!(names(&dir).is_empty());
    for other in ["game.log", "launcher.log", "run-notes.txt", "settings.cfg"] {
        assert_eq!(std::fs::read_to_string(dir.join(other)).unwrap(), "kept");
    }
}

#[test]
fn a_date_at_the_end_of_february_of_a_leap_year_is_right() {
    let dir = temp_dir("leap");
    // 29 February 2028, 23:59:59 UTC
    let leap = UNIX_EPOCH + Duration::from_secs(1_835_481_599);
    let (record, path) = start_in(&dir, &hello(), leap).unwrap();
    drop(record.finish().unwrap());
    assert_eq!(path, dir.join("run-20280229-235959.jsonl"));
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("\"started\":\"2028-02-29T23:59:59Z\""));
}

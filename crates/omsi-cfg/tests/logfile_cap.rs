//! The size cap of the log files: a log stops at the cap, and its last line says that it was
//! cut (the cap is injected small here; the files' own is 30 MB).

use omsi_cfg::logfile::{
    is_last_word, truncation_marker, Capped, CappedFile, LAST_WORDS, LOG_CAP_BYTES,
};
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
        "log truncated at 30 MB: of the lines after this one, only errors and the end of the game were written\n"
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
    // (1/64 of the cap is kept for errors: 100 bytes of 6400)
    let cap = 6400;
    let marker = truncation_marker(cap);
    let line = format!("{}\n", "x".repeat(99));
    let mut log = Capped::new(Vec::new(), cap);
    for _ in 0..200 {
        // (a dropped line is no error: the program goes on)
        log.write_all(line.as_bytes()).unwrap();
    }
    let text = String::from_utf8(log.into_inner()).unwrap();
    assert!(text.len() as u64 <= cap, "{} bytes", text.len());
    assert!(text.ends_with(&marker), "{text}");
    assert_eq!(text.matches("log truncated").count(), 1);
    // whole lines only, up to the room kept for the marker and the errors: none is cut
    let lines = (cap as usize - 100 - marker.len()) / line.len();
    assert_eq!(text, format!("{}{marker}", line.repeat(lines)));
}

/// What the launcher looks for at the end of `game.log` to tell a crash is still there in
/// a log that was cut: errors, the panic and the end of the game are written after the
/// marker, ordinary lines are not.
#[test]
fn errors_a_panic_and_the_end_of_the_game_are_written_after_the_marker() {
    let cap = 64_000;
    let marker = truncation_marker(cap);
    let mut log = Capped::new(Vec::new(), cap);
    let line = format!("[t INFO  game] {}\n", "x".repeat(84));
    for _ in 0..1000 {
        log.write_all(line.as_bytes()).unwrap();
    }
    let error = "[t ERROR game] a texture could not be read\n";
    let panic = "[t ERROR game] the game stopped on an error (build x): panicked at a.rs:1:1:\n    index out of bounds\n   0: std::backtrace\n";
    let lost = "[t WARN  render] the graphics device was lost (Unknown)\n";
    let end = "[t INFO  game] game ends\n";
    log.write_all(error.as_bytes()).unwrap();
    log.write_all(line.as_bytes()).unwrap();
    log.write_all(panic.as_bytes()).unwrap();
    log.write_all(lost.as_bytes()).unwrap();
    log.write_all(b"[t DEBUG game] loaded a file\n").unwrap();
    log.write_all(end.as_bytes()).unwrap();
    let text = String::from_utf8(log.into_inner()).unwrap();
    assert!(text.len() as u64 <= cap, "{} bytes", text.len());
    assert!(
        text.ends_with(&format!("{line}{marker}{error}{panic}{lost}{end}")),
        "{}",
        &text[text.len() - 600..]
    );
}

#[test]
fn the_room_for_errors_ends_at_the_cap() {
    let cap = 6400;
    let mut log = Capped::new(Vec::new(), cap);
    log.write_all("x".repeat(7000).as_bytes()).unwrap();
    let error = "[t ERROR game] again\n";
    for _ in 0..1000 {
        log.write_all(error.as_bytes()).unwrap();
    }
    let text = String::from_utf8(log.into_inner()).unwrap();
    assert!(text.len() as u64 <= cap, "{} bytes", text.len());
    // (the marker, then whole error lines up to the cap)
    let marker = truncation_marker(cap);
    let errors = (cap as usize - marker.len()) / error.len();
    assert_eq!(text, format!("{marker}{}", error.repeat(errors)));
}

#[test]
fn every_last_word_is_one_and_an_ordinary_line_is_not() {
    for word in LAST_WORDS {
        assert!(
            is_last_word(format!("[t x] a{word}b\n").as_bytes()),
            "{word}"
        );
    }
    assert!(!is_last_word(b"[t INFO  game] loaded a file\n"));
    assert!(!is_last_word(b"[t WARN  game] an error in a script\n"));
    assert!(!is_last_word(b""));
}

#[test]
fn a_shared_file_takes_an_error_after_its_marker() {
    let path = temp_log("shared-error");
    let cap = 64_000;
    let mut log = CappedFile::open(&path, cap).unwrap();
    let line = format!("[t INFO  launcher] {}\n", "w".repeat(80));
    for _ in 0..1000 {
        log.write_all(line.as_bytes()).unwrap();
    }
    let error = "[t ERROR launcher] the install failed\n";
    log.write_all(error.as_bytes()).unwrap();
    log.write_all(line.as_bytes()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.len() as u64 <= cap, "{} bytes", text.len());
    assert!(
        text.ends_with(&format!("{line}{}{error}", truncation_marker(cap))),
        "{}",
        &text[text.len() - 400..]
    );
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

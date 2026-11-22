//! `launcher.log` is written by the launcher and the preview helper at the same time: each
//! line says who wrote it, and none breaks into another.

use omsi_launcher_lib::{append_log_line, launcher_log_line};

fn temp_log(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("depot-launcher-log-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("launcher.log")
}

#[test]
fn a_line_names_the_program_that_wrote_it() {
    assert_eq!(
        launcher_log_line(12, "launcher", "starting the game"),
        "12 [launcher] starting the game\n"
    );
    assert_eq!(
        launcher_log_line(12, "preview-helper", "no picture"),
        "12 [preview-helper] no picture\n"
    );
}

#[test]
fn the_lines_of_the_helper_end_up_in_the_launcher_file() {
    let log = temp_log("helper");
    append_log_line(&log, "launcher", "one");
    append_log_line(&log, "preview-helper", "two");
    let text = std::fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("[launcher] one"), "{text}");
    assert!(lines[1].ends_with("[preview-helper] two"), "{text}");
}

#[test]
fn two_writers_at_once_lose_no_line_and_break_none() {
    let log = temp_log("both");
    let long = "x".repeat(300);
    let writers: Vec<_> = ["launcher", "preview-helper"]
        .into_iter()
        .map(|source| {
            let (log, long) = (log.clone(), long.clone());
            std::thread::spawn(move || {
                for n in 0..200 {
                    append_log_line(&log, source, &format!("{n} {long}"));
                }
            })
        })
        .collect();
    for w in writers {
        w.join().unwrap();
    }
    let text = std::fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 400);
    for l in &lines {
        assert!(
            l.contains("[launcher] ") != l.contains("[preview-helper] "),
            "a line broke: {l}"
        );
        assert!(l.ends_with(&long), "a line broke: {l}");
    }
    for source in ["launcher", "preview-helper"] {
        let own = lines.iter().filter(|l| l.contains(&format!("[{source}] "))).count();
        assert_eq!(own, 200, "{source}");
    }
}

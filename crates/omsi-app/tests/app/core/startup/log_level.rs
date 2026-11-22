use omsi_cfg::logfile::LogLevel;
use openomsi_game::testing::core::startup::{log_level_arg, log_setup};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|a| a.to_string()).collect()
}

/// Whether the logger set up with `filter` writes a line of `level` said by `target`.
fn writes(filter: &str, level: log::Level, target: &str) -> bool {
    use log::Log;
    let logger = env_logger::Builder::new().parse_filters(filter).build();
    logger.enabled(&log::Metadata::builder().level(level).target(target).build())
}

#[test]
fn the_level_is_read_off_the_command_line_before_it_is_parsed() {
    assert_eq!(log_level_arg(&args(&["depot"])), None);
    assert_eq!(
        log_level_arg(&args(&["depot", "--log-level", "debug", "--no-menu"])),
        Some("debug")
    );
    assert_eq!(
        log_level_arg(&args(&["depot", "--log-level=warn"])),
        Some("warn")
    );
    // the last one counts, and a parameter with no value changes nothing
    assert_eq!(
        log_level_arg(&args(&[
            "depot",
            "--log-level",
            "warn",
            "--log-level=error"
        ])),
        Some("error")
    );
    assert_eq!(
        log_level_arg(&args(&["depot", "--log-level=warn", "--log-level"])),
        Some("warn")
    );
}

#[test]
fn each_level_shows_its_lines_and_hides_the_lines_below_it() {
    use log::Level::{Debug, Error, Info, Warn};
    let game = "omsi_sim::vehicle";
    let expected = [
        (LogLevel::Error, [true, false, false, false]),
        (LogLevel::Warn, [true, true, false, false]),
        (LogLevel::Info, [true, true, true, false]),
        (LogLevel::Debug, [true, true, true, true]),
    ];
    for (level, shown) in expected {
        let setup = log_setup(Some(level.name()), None);
        assert_eq!(setup.warning, None);
        for (line, shown) in [Error, Warn, Info, Debug].into_iter().zip(shown) {
            assert_eq!(
                writes(&setup.filter, line, game),
                shown,
                "{} line at level {}",
                line,
                level.name()
            );
        }
    }
}

#[test]
fn debug_leaves_the_graphics_libraries_at_info() {
    let setup = log_setup(Some("debug"), None);
    assert!(writes(
        &setup.filter,
        log::Level::Debug,
        "openomsi_game::frame"
    ));
    assert!(writes(&setup.filter, log::Level::Info, "wgpu_core::device"));
    assert!(!writes(
        &setup.filter,
        log::Level::Debug,
        "wgpu_core::device"
    ));
}

#[test]
fn no_level_asked_for_is_info_and_says_nothing() {
    let setup = log_setup(None, None);
    assert_eq!(setup.filter, LogLevel::Info.filter());
    assert_eq!((setup.warning, setup.note), (None, None));
    // (an empty RUST_LOG says nothing either)
    assert_eq!(log_setup(None, Some("  ")).filter, "info");
}

#[test]
fn a_level_that_is_none_of_the_four_is_info_with_a_warning() {
    let setup = log_setup(Some("loud"), None);
    assert_eq!(setup.filter, LogLevel::Info.filter());
    let warning = setup.warning.expect("a warning");
    assert!(warning.contains("\"loud\""), "{warning}");
    assert!(warning.contains("using info"), "{warning}");
    assert!(writes(&setup.filter, log::Level::Warn, "openomsi_game"));
}

#[test]
fn rust_log_wins_over_the_level_and_the_log_says_so() {
    let setup = log_setup(Some("error"), Some("warn,omsi_script=debug"));
    assert_eq!(setup.filter, "warn,omsi_script=debug");
    let warning = setup.warning.expect("a warning");
    assert!(
        warning.contains("RUST_LOG=warn,omsi_script=debug"),
        "{warning}"
    );
    assert!(warning.contains("--log-level error"), "{warning}");
    // (alone, it is the filter and nothing is said)
    let alone = log_setup(None, Some("debug"));
    assert_eq!((alone.filter.as_str(), alone.warning), ("debug", None));
}

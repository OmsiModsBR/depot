//! The four levels of the log: how each reads, is written, and what filter the logger gets.

use omsi_cfg::logfile::{LogLevel, LOG_CHANNELS};

#[test]
fn the_four_levels_read_in_any_letter_case() {
    assert_eq!(LogLevel::parse("error"), Some(LogLevel::Error));
    assert_eq!(LogLevel::parse("WARN"), Some(LogLevel::Warn));
    assert_eq!(LogLevel::parse("warning"), Some(LogLevel::Warn));
    assert_eq!(LogLevel::parse(" Info "), Some(LogLevel::Info));
    assert_eq!(LogLevel::parse("Debug"), Some(LogLevel::Debug));
}

#[test]
fn anything_else_is_no_level() {
    for text in ["", "trace", "verbose", "3", "off", "info,omsi_sim=debug"] {
        assert_eq!(LogLevel::parse(text), None, "{text:?}");
    }
}

#[test]
fn the_default_is_info_and_every_name_reads_back() {
    assert_eq!(LogLevel::default(), LogLevel::Info);
    for level in LogLevel::ALL {
        assert_eq!(LogLevel::parse(level.name()), Some(level));
    }
}

#[test]
fn debug_is_for_the_games_own_crates_only() {
    assert_eq!(LogLevel::Error.filter(), "error");
    assert_eq!(LogLevel::Warn.filter(), "warn");
    assert_eq!(LogLevel::Info.filter(), "info");
    let debug = LogLevel::Debug.filter();
    assert!(debug.starts_with("info,"), "{debug}");
    for target in LOG_CHANNELS.iter().flat_map(|(_, targets)| targets.iter()) {
        assert!(debug.contains(&format!(",{target}=debug")), "{target}");
    }
    assert!(!debug.contains("wgpu"), "{debug}");
}

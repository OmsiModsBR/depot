//! The channels of the log: their table, how a request for them reads, and the filter.

use omsi_cfg::logfile::{channel_names, ChannelLevels, LogLevel, LOG_CHANNELS};

#[test]
fn the_channels_are_the_nine_of_the_discussion() {
    assert_eq!(
        channel_names(),
        "script, content, audio, physics, ai, network, render, input, app"
    );
}

#[test]
fn no_target_is_in_two_channels() {
    let mut targets: Vec<&str> = LOG_CHANNELS
        .iter()
        .flat_map(|(_, targets)| targets.iter().copied())
        .collect();
    let all = targets.len();
    targets.sort_unstable();
    targets.dedup();
    assert_eq!(targets.len(), all);
}

/// A module given to a channel is taken out of its crate's: the crate itself must then be
/// some channel's too, or the rest of it would have no level.
#[test]
fn every_module_of_a_channel_has_its_crate_in_a_channel() {
    let targets: Vec<&str> = LOG_CHANNELS
        .iter()
        .flat_map(|(_, targets)| targets.iter().copied())
        .collect();
    for target in &targets {
        if let Some((krate, _)) = target.split_once("::") {
            assert!(targets.contains(&krate), "{target}");
        }
    }
}

#[test]
fn a_request_reads_whatever_its_spaces_and_letter_case() {
    let own = ChannelLevels::parse(" Physics = DEBUG ,,render=warn,");
    assert!(own.rejected.is_empty(), "{:?}", own.rejected);
    assert_eq!(
        own.levels,
        vec![("physics", LogLevel::Debug), ("render", LogLevel::Warn)]
    );
    assert_eq!(own.level_of("physics"), Some(LogLevel::Debug));
    assert_eq!(own.level_of("script"), None);
    assert_eq!(ChannelLevels::parse(""), ChannelLevels::default());
}

#[test]
fn a_channel_named_twice_keeps_the_last_level() {
    let own = ChannelLevels::parse("ai=debug,ai=error");
    assert_eq!(own.levels, vec![("ai", LogLevel::Error)]);
}

#[test]
fn what_is_not_understood_is_reported_and_the_rest_counts() {
    let own = ChannelLevels::parse("engine=debug,script=loud,audio,input=debug");
    assert_eq!(own.levels, vec![("input", LogLevel::Debug)]);
    assert_eq!(own.rejected.len(), 3);
}

#[test]
fn the_filter_gives_every_target_its_channels_level_or_the_global_one() {
    let own = ChannelLevels::parse("script=debug,render=error");
    let filter = own.filter(LogLevel::Warn);
    assert!(filter.starts_with("warn,"), "{filter}");
    assert!(filter.contains(",omsi_script=debug"), "{filter}");
    assert!(filter.contains(",omsi_render=error"), "{filter}");
    assert!(filter.contains(",omsi_render_vanilla=error"), "{filter}");
    assert!(filter.contains(",omsi_audio=warn"), "{filter}");
    // (what is no channel's, the graphics libraries, never goes past info)
    assert!(own.filter(LogLevel::Debug).starts_with("info,"));
    assert!(own.filter(LogLevel::Error).starts_with("error,"));
}

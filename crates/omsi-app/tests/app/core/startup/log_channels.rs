use openomsi_game::testing::core::startup::log_setup;

/// Whether the logger set up with `filter` writes a line of `level` said by `target`.
fn writes(filter: &str, level: log::Level, target: &str) -> bool {
    use log::Log;
    let logger = env_logger::Builder::new().parse_filters(filter).build();
    logger.enabled(&log::Metadata::builder().level(level).target(target).build())
}

#[test]
fn a_channel_at_debug_writes_its_debug_lines_and_the_others_stay_at_the_level() {
    use log::Level::{Debug, Info};
    let setup = log_setup(None, Some("physics=debug"), None);
    assert!(setup.warnings.is_empty(), "{:?}", setup.warnings);
    assert_eq!(setup.note.as_deref(), Some("log level: physics=debug"));
    assert!(writes(
        &setup.filter,
        Debug,
        "omsi_sim::physics::implementation::step"
    ));
    assert!(writes(&setup.filter, Debug, "omsi_sim::vehicle::motion"));
    // the rest of the same crate is another channel (`ai`, `app`) and stays at info
    assert!(!writes(&setup.filter, Debug, "omsi_sim::traffic::network"));
    assert!(!writes(&setup.filter, Debug, "omsi_sim::host::callbacks"));
    assert!(writes(&setup.filter, Info, "omsi_sim::host::callbacks"));
    assert!(!writes(&setup.filter, Debug, "omsi_script::vm"));
    assert!(!writes(&setup.filter, Debug, "wgpu_core::device"));
}

#[test]
fn a_channel_can_be_quieter_than_the_level() {
    use log::Level::{Debug, Info, Warn};
    let setup = log_setup(Some("debug"), Some("render=warn, ai=info"), None);
    assert_eq!(
        setup.note.as_deref(),
        Some("log level: debug, render=warn, ai=info")
    );
    assert!(!writes(&setup.filter, Info, "omsi_render::renderer"));
    assert!(writes(&setup.filter, Warn, "omsi_render::renderer"));
    assert!(!writes(
        &setup.filter,
        Info,
        "openomsi_game::output::implementation::camera"
    ));
    assert!(!writes(
        &setup.filter,
        Debug,
        "openomsi_game::gameplay::implementation::traffic"
    ));
    assert!(writes(
        &setup.filter,
        Info,
        "openomsi_game::gameplay::implementation::traffic"
    ));
    // the channels given no level follow the level
    assert!(writes(
        &setup.filter,
        Debug,
        "openomsi_game::gameplay::implementation::journey"
    ));
    assert!(writes(&setup.filter, Debug, "omsi_script::compile"));
    assert!(writes(&setup.filter, Debug, "omsi_content::vehicle"));
    assert!(!writes(&setup.filter, Debug, "wgpu_hal::vulkan"));
}

#[test]
fn an_entry_that_is_not_understood_is_left_out_with_a_warning() {
    let setup = log_setup(
        None,
        Some("engine=debug,script=loud,audio,input=debug"),
        None,
    );
    assert_eq!(setup.warnings.len(), 3, "{:?}", setup.warnings);
    assert!(
        setup.warnings[0].contains("no channel \"engine\""),
        "{:?}",
        setup.warnings
    );
    assert!(
        setup.warnings[0].contains("script, content, audio"),
        "{:?}",
        setup.warnings
    );
    assert!(
        setup.warnings[1].contains("\"loud\" is none of"),
        "{:?}",
        setup.warnings
    );
    assert!(
        setup.warnings[2].contains("is not channel=level"),
        "{:?}",
        setup.warnings
    );
    // the entry that was understood still counts
    assert!(writes(
        &setup.filter,
        log::Level::Debug,
        "openomsi_game::input::implementation::keyboard"
    ));
    assert!(!writes(&setup.filter, log::Level::Debug, "omsi_script::vm"));
    assert!(!writes(
        &setup.filter,
        log::Level::Debug,
        "omsi_audio::mixer"
    ));
}

#[test]
fn rust_log_wins_over_the_channels_too() {
    let setup = log_setup(None, Some("physics=debug"), Some("warn"));
    assert_eq!(setup.filter, "warn");
    assert!(
        setup.warnings[0].contains("--log-channels physics=debug"),
        "{:?}",
        setup.warnings
    );
}

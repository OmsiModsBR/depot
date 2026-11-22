use openomsi_game::testing::core::startup::{launcher_process, log_source};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|a| a.to_string()).collect()
}

/// The launcher and the preview helper write one file, so each names itself on its lines; a
/// game's lines go to its own `game.log` and carry no name.
#[test]
fn the_launcher_and_the_helper_name_themselves_and_a_game_does_not() {
    assert_eq!(log_source(&args(&["depot"])), Some("launcher"));
    assert_eq!(
        log_source(&args(&["depot", "--launcher"])),
        Some("launcher")
    );
    assert_eq!(
        log_source(&args(&["depot", "--preview-helper"])),
        Some("preview-helper")
    );
    assert_eq!(
        log_source(&args(&["depot", "--no-menu", "--map", "x"])),
        None
    );
}

#[test]
fn a_bare_start_and_the_launcher_flag_are_the_launcher() {
    assert!(launcher_process(&args(&["depot"])));
    assert!(launcher_process(&args(&[
        "depot",
        "--launcher",
        "--parent-pid",
        "1"
    ])));
    assert!(!launcher_process(&args(&["depot", "--preview-helper"])));
    assert!(!launcher_process(&args(&["depot", "--no-menu"])));
}

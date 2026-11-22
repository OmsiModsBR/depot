#[test]
fn a_panic_is_found_and_a_clean_end_is_not() {
    let dir = std::env::temp_dir().join("depot-crash-test");
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("game.log");
    std::fs::write(&p, "[t INFO x] loading\n[t ERROR openomsi_game] the game stopped on an error (build x): panicked at a.rs:1:1:\n    index out of bounds\n\n   0: std::backtrace\n").unwrap();
    let (what, tail) = omsi_launcher_ui::state::crash_of(&p).unwrap();
    assert!(what.contains("index out of bounds"), "{what}");
    assert!(tail.contains("loading"));
    std::fs::write(
        &p,
        "[t INFO x] loading\n[t INFO openomsi_game::app_events] game ends\n",
    )
    .unwrap();
    assert!(omsi_launcher_ui::state::crash_of(&p).is_none());
    std::fs::write(&p, "[t ERROR omsi_render] the graphics device was lost (Unknown): Unexpected error variant\n[t INFO openomsi_game::app_events] game ends\n").unwrap();
    assert!(omsi_launcher_ui::state::crash_of(&p)
        .unwrap()
        .0
        .contains("device was lost"));
    // an error before the game started, or one it got over, is not the crash
    std::fs::write(&p, "[t ERROR omsi_render] a part of the picture could not be recorded (left out)\n[t INFO x] starting the game: omsi\n[t INFO omsi_render] renderer: compiling the sky and clouds shaders\n").unwrap();
    assert!(omsi_launcher_ui::state::crash_of(&p).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A device lost in the middle of a frame (#48): wgpu's panic over it is caught and only
/// warned of, and the report names the loss - not the panic - as what ended the game. A
/// card that hung when the game closed after a clean end is no crash.
#[test]
fn a_loss_caught_mid_frame_is_reported_as_the_loss() {
    let dir = std::env::temp_dir().join(format!("depot-crash-lost-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("game.log");
    std::fs::write(&p, "[t INFO x] starting the game: omsi\n\
        [t ERROR omsi_render::gpu::lost] the graphics device was lost (Unknown: Parent device is lost)\n\
        [t WARN openomsi_game] caught by the renderer: panicked at lost.rs:1:1:\n\
        \x20   Error in Device::poll: Parent device is lost\n\
        [t ERROR openomsi_game::app_events] ending the session: the graphics device was lost (Unknown: Parent device is lost)\n\
        [t INFO openomsi_game::app_events] game ends\n").unwrap();
    let (what, _) = omsi_launcher_ui::state::crash_of(&p).unwrap();
    assert!(
        what.contains("ending the session: the graphics device was lost"),
        "{what}"
    );
    assert!(
        !what.contains("Device::poll") && !what.contains("game ends"),
        "{what}"
    );
    std::fs::write(&p, "[t INFO x] starting the game: omsi\n\
        [t INFO openomsi_game::app_events] game ends\n\
        [t WARN openomsi_game] the graphics device did not close in order (the game had ended already)\n").unwrap();
    assert!(omsi_launcher_ui::state::crash_of(&p).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The report says what the game ran on (the start of the log), and its title is the
/// error alone, not the records after it (#1187).
#[test]
fn the_report_has_the_computer_and_a_clean_title() {
    let dir = std::env::temp_dir().join(format!("depot-crash-machine-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("game.log");
    let mut log = String::from("[t INFO openomsi_game::applog] system: windows x86_64 (10.0), 8 threads, 8192 MB memory\n[t INFO openomsi_game::applog] command line: openomsi --map maps/X/global.cfg\n[t INFO omsi_render] graphics adapter: GTX 750 (DiscreteGpu, Dx12, 2048 MB of its own), texture memory taken for it: 716 MB\n");
    // (more than the end that goes with the report)
    for k in 0..400 {
        log.push_str(&format!(
            "[t INFO openomsi_game::scene] tile loading: placed tile {k},0\n"
        ));
    }
    log.push_str("[t ERROR openomsi_game::app_events] ending the session: the graphics device was lost (Unknown: Out of memory)\n[t INFO openomsi_game::app_events] game ends\n[t WARN openomsi_game::scene] tile loading: first-area batch prepared in 352.71 s\n");
    std::fs::write(&p, log).unwrap();
    let (what, tail) = omsi_launcher_ui::state::crash_of(&p).unwrap();
    assert_eq!(
        what,
        "ending the session: the graphics device was lost (Unknown: Out of memory)"
    );
    let (machine, end) = tail
        .split_once(&format!("\n{}\n", omsi_launcher_ui::state::CRASH_TAIL_GAP))
        .unwrap();
    assert!(
        machine.contains("8192 MB memory")
            && machine.contains("GTX 750")
            && machine.contains("maps/X/global.cfg"),
        "{machine}"
    );
    assert!(end.contains("352.71 s") && !end.contains("placed tile 0,0"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A crash report is copied and sent: the user's folder in the log's lines goes out as the
/// placeholder, also from a log written before the logs had it (#648).
#[test]
fn a_crash_report_holds_the_placeholder_and_not_the_users_folder() {
    let Some(home) = ["HOME", "USERPROFILE"]
        .into_iter()
        .find_map(|k| std::env::var(k).ok())
        .filter(|h| h.len() > 3)
    else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("depot-crash-private-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("game.log");
    let sep = std::path::MAIN_SEPARATOR;
    std::fs::write(
        &p,
        format!(
            "[t INFO openomsi_game] command line: {home}{sep}Depot{sep}depot --no-menu\n\
             [t ERROR openomsi_game] the game stopped on an error (build x): panicked at a.rs:1:1:\n\
             \x20   cannot read {home}{sep}.depot{sep}settings.cfg\n\n   0: std::backtrace\n"
        ),
    )
    .unwrap();
    let (what, tail) = omsi_launcher_ui::state::crash_of(&p).unwrap();
    for text in [&what, &tail] {
        assert!(!text.contains(&home), "{text}");
        assert!(text.contains("USER_FOLDER"), "{text}");
    }
    assert!(
        what.contains(&format!("USER_FOLDER{sep}.depot{sep}settings.cfg")),
        "{what}"
    );
    assert!(
        tail.contains(&format!("USER_FOLDER{sep}Depot{sep}depot --no-menu")),
        "{tail}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The error's text is cut to its length after the user's folder is replaced: a cut in the
/// middle of the folder left the start of it in the report.
#[test]
fn the_user_folder_is_replaced_before_the_error_is_cut() {
    let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
        return;
    };
    let home = home.to_string_lossy().to_string();
    if home.len() < 4 {
        return;
    }
    let dir = std::env::temp_dir().join(format!("depot-crash-cut-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("game.log");
    // the folder starts two characters before the cut
    let lead = "the game stopped on an error: ";
    let fill = "x".repeat(598 - lead.len() - 1);
    std::fs::write(&p, format!("[t ERROR g] {lead}{fill} {home}/a.cfg\n")).unwrap();
    let (what, _) = omsi_launcher_ui::state::crash_of(&p).unwrap();
    assert!(what.ends_with(" US"), "{what}");
    assert!(!what.ends_with(&home[..2]) || home.starts_with("US"), "{what}");
    let _ = std::fs::remove_dir_all(&dir);
}

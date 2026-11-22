use openomsi_game::testing::core::startup::past_the_logger;

/// What the program prints itself beside a log line (a fatal message, the error `main`
/// ended on, a panic): as it is in a terminal, not a second time into a log file, and with
/// the user's folder replaced when the logger left the line out.
#[test]
fn a_text_past_the_logger_never_reaches_a_log_file_as_it_is() {
    let text = "Project Depot: the window cannot be opened";
    assert_eq!(past_the_logger(text, true, true).as_deref(), Some(text));
    assert_eq!(past_the_logger(text, true, false).as_deref(), Some(text));
    assert_eq!(past_the_logger(text, false, true), None);
    assert_eq!(past_the_logger(text, false, false).as_deref(), Some(text));
    // (this machine's own folder, whatever it is: a home of one piece is nobody's own)
    let Some(home) = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .filter(|h| h.trim_matches(['/', '\\']).contains(['/', '\\']))
    else {
        return;
    };
    let said = past_the_logger(&format!("Error: reading {home}/a.cfg"), false, false).unwrap();
    assert_eq!(said, "Error: reading USER_FOLDER/a.cfg");
}

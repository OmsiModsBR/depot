#[test]
fn the_servers_word_is_read_back_from_the_log_of_this_run() {
    let dir = std::env::temp_dir().join(format!("depot-disc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("game.log");
    let left = openomsi_game::testing::lan::LEFT_SERVER;
    std::fs::write(&p, format!("[x INFO a] starting the game: one\n[x WARN openomsi_game::lan] {left}old\n[x INFO a] starting the game: two\n[x WARN openomsi_game::lan] {left}Expulsé : conduite dangereuse\n[x INFO a] game ends\n")).unwrap();
    assert_eq!(
        openomsi_game::testing::launcher::state::disconnect_of(&p).as_deref(),
        Some("Expulsé : conduite dangereuse")
    );
    // a run that ended in any other way: nothing
    std::fs::write(&p, format!("[x INFO a] starting the game: one\n[x WARN openomsi_game::lan] {left}old\n[x INFO a] starting the game: two\n[x INFO a] game ends\n")).unwrap();
    assert!(openomsi_game::testing::launcher::state::disconnect_of(&p).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

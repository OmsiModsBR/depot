use openomsi_game::testing::launcher::tray::*;

/// The menu has the concept's four lines, each once, Stop game apart from the rest.
#[test]
fn the_menu_has_each_line_once() {
    let cmds: Vec<TrayCmd> = MENU.iter().filter_map(|x| x.0).collect();
    assert_eq!(
        cmds,
        [
            TrayCmd::Show,
            TrayCmd::OpenLog,
            TrayCmd::StopGame,
            TrayCmd::Quit
        ]
    );
    assert!(MENU[2].0.is_none());
}

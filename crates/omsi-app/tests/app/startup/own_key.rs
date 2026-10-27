/// OMSI's own file as it comes has no key of the player's own.
#[test]
fn the_stock_file_has_none_of_the_players() {
    let root = std::path::Path::new("../../../OMSI 2 Original");
    if !root.join("Inputs/keyboard.cfg").exists() {
        return;
    }
    assert!(openomsi_game::testing::startup::own_bindings(root, 0).is_empty());
    assert!(
        openomsi_game::testing::startup::own_bindings(root, omsi_content::input::KEY_SHIFT)
            .is_empty()
    );
}

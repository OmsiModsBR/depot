use omsi_content::KeyBinding;
use openomsi_game::testing::diagnostics::rebound_key;

fn bind(action: &str, scan_code: i32, modifier: i32) -> KeyBinding {
    KeyBinding {
        action: action.into(),
        scan_code,
        modifier,
    }
}

#[test]
fn hint_names_the_players_own_key_only() {
    // the stock D (scan 32) keeps the hint's own wording; Ctrl+D is named (#461)
    assert_eq!(
        rebound_key(&[bind("automatic_D", 32, 0)], "automatic_D"),
        None
    );
    assert_eq!(
        rebound_key(&[bind("automatic_D", 32, 4)], "automatic_D").as_deref(),
        Some("Ctrl+D")
    );
    assert_eq!(
        rebound_key(&[bind("automatic_D", 0, 0)], "automatic_D"),
        None
    );
    assert_eq!(rebound_key(&[], "automatic_D"), None);
    // the electrics' key: E is the stock one, a moved one is named
    assert_eq!(
        rebound_key(
            &[bind("cp_batterietrennschalter_toggle", 18, 0)],
            "cp_batterietrennschalter_toggle"
        ),
        None
    );
    assert_eq!(
        rebound_key(
            &[bind("cp_batterietrennschalter_toggle", 18, 4)],
            "cp_batterietrennschalter_toggle"
        )
        .as_deref(),
        Some("Ctrl+E")
    );
}

//! When Vanilla's Direct3D 9Ex renderer cannot open (spec 001, FR-011a): the player is told
//! why and to choose Vanilla+ in the launcher.

use openomsi_game::testing::output::backend::vanilla_cannot_open;

#[test]
fn the_open_failure_names_the_cause_and_points_to_vanilla_plus() {
    let text = vanilla_cannot_open("Direct3D 9Ex: no device for this adapter");
    assert!(
        text.contains("Direct3D 9Ex: no device for this adapter"),
        "{text}"
    );
    assert!(text.contains("Choose Vanilla+ in the launcher"), "{text}");
}

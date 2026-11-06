use openomsi_game::testing::app_events::reduced_texture_budget;

#[test]
fn the_first_shortage_sets_a_budget() {
    assert_eq!(reduced_texture_budget(0), 600_000_000);
}

#[test]
fn each_later_shortage_takes_two_fifths_off() {
    assert_eq!(reduced_texture_budget(1_000_000_000), 600_000_000);
    assert_eq!(reduced_texture_budget(600_000_000), 360_000_000);
}

#[test]
fn the_budget_never_goes_under_300_mb() {
    assert_eq!(reduced_texture_budget(360_000_000), 300_000_000);
    assert_eq!(reduced_texture_budget(300_000_000), 300_000_000);
}

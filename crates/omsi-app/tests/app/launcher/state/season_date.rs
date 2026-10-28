#[test]
fn a_season_keeps_the_day_within_its_month() {
    assert_eq!(
        openomsi_game::testing::launcher::state::date_in_month("2026-03-31", "04"),
        "2026-04-30"
    );
    assert_eq!(
        openomsi_game::testing::launcher::state::date_in_month("2026-03-31", "07"),
        "2026-07-31"
    );
    assert_eq!(
        openomsi_game::testing::launcher::state::date_in_month("2026-11-05", "01"),
        "2026-01-05"
    );
}

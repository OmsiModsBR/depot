#[test]
fn a_line_is_read_back() {
    assert_eq!(
        openomsi_game::testing::settings::bus_seats::parse(
            "vehicles/man_nl_ng/man_en92_main.bus|0.1|-0.2|0.05|3"
        ),
        Some((
            "vehicles/man_nl_ng/man_en92_main.bus".to_string(),
            [0.1, -0.2, 0.05],
            3.0
        ))
    );
    // (a | in the path stays in it: the numbers are the last four fields)
    assert_eq!(
        openomsi_game::testing::settings::bus_seats::parse("a|b.bus|0|0|0|0").map(|e| e.0),
        Some("a|b.bus".to_string())
    );
    assert_eq!(
        openomsi_game::testing::settings::bus_seats::parse("broken"),
        None
    );
}

use openomsi_game::testing::admin::{duty_arg, parse_duty};

#[test]
fn a_duty_goes_to_the_game_and_back() {
    assert_eq!(duty_arg("15", "5", 2, 7), "15 5 2 7");
    assert_eq!(
        parse_duty("15 5 2 7"),
        Some(("15".into(), "5".into(), 2, 7))
    );
    // names with spaces and percent signs
    let a = duty_arg("KI-Zug", "ZOB RB 100%", 0, 0);
    assert_eq!(a, "KI-Zug ZOB%20RB%20100%25 0 0");
    assert_eq!(
        parse_duty(&a),
        Some(("KI-Zug".into(), "ZOB RB 100%".into(), 0, 0))
    );
    // missing or extra parts, not numbers: not a duty
    for bad in [
        "",
        "15",
        "15 5",
        "15 5 2",
        "15 5 2 7 9",
        "15 5 x 7",
        "15 5 2 -1",
    ] {
        assert_eq!(parse_duty(bad), None, "{bad}");
    }
}

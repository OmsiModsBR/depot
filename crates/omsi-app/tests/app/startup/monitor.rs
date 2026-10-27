use openomsi_game::testing::startup::*;

#[test]
fn the_monitor_holding_a_point_is_found_whatever_its_place() {
    // a monitor left of the primary one, the primary one, one above it
    let rects = [
        ((-1920, 0), (1920, 1080)),
        ((0, 0), (2560, 1440)),
        ((0, -1080), (1920, 1080)),
    ];
    assert_eq!(monitor_holding(&rects, (-100, 500)), Some(0));
    assert_eq!(monitor_holding(&rects, (0, 0)), Some(1));
    assert_eq!(monitor_holding(&rects, (2559, 1439)), Some(1));
    assert_eq!(monitor_holding(&rects, (2560, 1439)), None);
    assert_eq!(monitor_holding(&rects, (100, -1)), Some(2));
    assert_eq!(monitor_holding(&rects, (5000, 5000)), None);
    assert_eq!(monitor_holding(&[], (0, 0)), None);
}

#[test]
fn a_monitor_left_of_the_primary_one_gets_through_the_argument_parser() {
    use clap::Parser;
    for form in [
        vec!["depot", "--monitor-at=-1500,300"],
        vec!["depot", "--monitor-at", "-1500,300"],
    ] {
        let args = openomsi_game::testing::cli::Args::try_parse_from(&form)
            .unwrap_or_else(|e| panic!("{form:?}: {e}"));
        assert_eq!(
            args.monitor_at.as_deref().and_then(parse_point),
            Some((-1500, 300))
        );
    }
}

#[test]
fn the_monitor_point_is_read_from_the_command_line() {
    assert_eq!(parse_point("-1500,300"), Some((-1500, 300)));
    assert_eq!(parse_point(" 10 , 20 "), Some((10, 20)));
    assert_eq!(parse_point("10"), None);
    assert_eq!(parse_point("a,b"), None);
}

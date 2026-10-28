mod repeat;

use omsi_timetable::Tour;
use openomsi_game::testing::launcher::timetable::*;

#[test]
fn times() {
    assert_eq!(parse_time("4:07"), Some(247.0));
    assert_eq!(parse_time("24:10"), Some(1450.0));
    assert_eq!(parse_time("4:7:30"), Some(247.5));
    assert_eq!(parse_time("4"), None);
    assert_eq!(parse_time("4:61"), None);
    assert_eq!(fmt_time(247.0), "4:07");
    assert_eq!(fmt_time(247.5), "4:07:30");
}

#[test]
fn numbers() {
    let t = |n: &str| Tour {
        number: n.into(),
        ..Default::default()
    };
    assert_eq!(next_number(&[t("1"), t("7"), t("x")]), "8");
    assert_eq!(next_number(&[]), "1");
}

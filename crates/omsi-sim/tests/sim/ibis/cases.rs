use omsi_sim::testing::ibis::*;

#[test]
fn layouts_fit_the_room() {
    let t = Target {
        line: 5,
        suffix: 0,
        route: Some(1),
        terminus_code: Some(34),
        route_index: Some(3),
        terminus_index: 7,
        stop: 0,
    };
    assert_eq!(
        layouts(&t, 10),
        vec![
            "005000001",
            "0050000001",
            "500",
            "01",
            "034",
            "00500",
            "5",
            "1",
            "34"
        ]
    );
    assert_eq!(
        layouts(&t, 5),
        vec!["500", "01", "034", "00500", "5", "1", "34"]
    );
    assert_eq!(layouts(&t, 2), vec!["01", "5", "1", "34"]);
    let t = Target {
        line: 137,
        suffix: 0,
        route: None,
        terminus_code: Some(5),
        ..Default::default()
    };
    assert_eq!(layouts(&t, 5), vec!["13700", "005", "137", "5"]);
}

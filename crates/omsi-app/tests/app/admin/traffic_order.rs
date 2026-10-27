use openomsi_game::testing::admin::TrafficOrder;

#[test]
fn a_traffic_order_is_read() {
    assert_eq!(TrafficOrder::parse("clear"), Some(TrafficOrder::Clear));
    assert_eq!(TrafficOrder::parse(" 20 "), Some(TrafficOrder::Density(20)));
    assert_eq!(TrafficOrder::parse("0"), Some(TrafficOrder::Density(0)));
    assert_eq!(TrafficOrder::parse("400"), Some(TrafficOrder::Density(120)));
    assert_eq!(
        TrafficOrder::parse("120"),
        Some(TrafficOrder::Density(
            openomsi_game::testing::game_lists::TRAFFIC_MAX
        ))
    );
    for bad in ["", "next", "-5", "2.5"] {
        assert_eq!(TrafficOrder::parse(bad), None, "{bad}");
    }
}

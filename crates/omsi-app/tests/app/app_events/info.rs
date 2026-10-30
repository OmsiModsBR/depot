use openomsi_game::testing::app_events::{distance_driven, odometer_reading, passengers_aboard};

#[test]
fn the_distance_driven_is_written_in_kilometres() {
    assert_eq!(distance_driven(0.0), "0.0 km");
    assert_eq!(distance_driven(12_345.0), "12.3 km");
}

#[test]
fn the_odometer_reads_kilometres_and_tenths() {
    assert_eq!(odometer_reading(75_556.639), "Odometer 75556.6 km");
}

/// The count stands before the word, which is singular for one passenger (in the
/// tables' language; without a lookup the English key is drawn as it is).
#[test]
fn one_passenger_is_written_in_the_singular() {
    assert_eq!(passengers_aboard(0, None), "0 Passengers");
    assert_eq!(passengers_aboard(1, None), "1 Passenger");
    assert_eq!(passengers_aboard(23, None), "23 Passengers");
}

/// With the places the bus has, the count is "aboard / places" and the word is the plural.
#[test]
fn the_passengers_come_with_the_places_of_the_bus() {
    assert_eq!(passengers_aboard(36, Some(111)), "36 / 111 Passengers");
    assert_eq!(passengers_aboard(1, Some(40)), "1 / 40 Passengers");
}

#[test]
fn the_layout_is_read_from_the_setting() {
    use openomsi_game::testing::app_events::InfoMode;
    assert_eq!(InfoMode::from_setting("full"), InfoMode::Full);
    assert_eq!(InfoMode::from_setting("compact"), InfoMode::Compact);
    assert_eq!(InfoMode::from_setting("essential"), InfoMode::Essential);
    assert_eq!(InfoMode::from_setting("custom"), InfoMode::Custom);
    assert_eq!(InfoMode::from_setting("sideways"), InfoMode::Full);
}

/// The custom layout leaves out the items switched off; the others keep their layout.
#[test]
fn the_custom_layout_leaves_out_what_is_switched_off() {
    use openomsi_game::testing::app_events::{info_bar, InfoData, InfoMode};
    let clock = omsi_sim::SimClock::default();
    let mut data = InfoData {
        clock: &clock,
        player: None,
        duty: None,
        passengers: None,
        capacity: None,
        metres: 0.0,
        limit: None,
        fps: None,
        items: [true; 9],
        labels: true,
        low_fuel: false,
        collapsed: false,
    };
    assert_eq!(info_bar(&data, InfoMode::Custom, false).groups.len(), 1);
    data.items[0] = false;
    assert!(info_bar(&data, InfoMode::Custom, false).groups.is_empty());
    // the other layouts do not read the switches
    assert_eq!(info_bar(&data, InfoMode::Full, false).groups.len(), 1);
}

/// Without a vehicle the bar has the time, and the frame rate when it is asked for.
#[test]
fn the_bar_without_a_vehicle_is_the_time_and_the_frame_rate() {
    use openomsi_game::testing::app_events::{info_bar, InfoData, InfoMode};
    let clock = omsi_sim::SimClock::default();
    let data = InfoData {
        clock: &clock,
        player: None,
        duty: None,
        passengers: None,
        capacity: None,
        metres: 0.0,
        limit: None,
        fps: Some((74.0, Some(52.0))),
        items: [true; 9],
        labels: true,
        low_fuel: false,
        collapsed: false,
    };
    let bar = info_bar(&data, InfoMode::Full, true);
    assert!(bar.bottom);
    assert_eq!(bar.groups.len(), 2);
    assert!(bar.groups[1][0].text.starts_with("74 fps"));
    let compact = info_bar(&data, InfoMode::Compact, false);
    assert_eq!(compact.groups[1][0].text, "74 / 52 fps");
    // (asked for, the frame rate is there in every layout)
    assert_eq!(info_bar(&data, InfoMode::Essential, false).groups.len(), 1);
}

/// The two-tier layouts: the clock in the first tier, the frame rate among the readouts of
/// the second, which the arrow folds away; the split layout keeps them on a plate of its own.
#[test]
fn the_two_tier_layouts_have_a_second_tier_that_folds_away() {
    use openomsi_game::testing::app_events::{info_bar, InfoData, InfoMode};
    let clock = omsi_sim::SimClock::default();
    let mut data = InfoData {
        clock: &clock,
        player: None,
        duty: None,
        passengers: None,
        capacity: None,
        metres: 0.0,
        limit: None,
        fps: Some((74.0, Some(52.0))),
        items: [true; 9],
        labels: true,
        low_fuel: true,
        collapsed: false,
    };
    let bar = info_bar(&data, InfoMode::TwoTier, true);
    assert!(bar.tiered && !bar.split && bar.bottom && !bar.collapsed);
    assert_eq!(bar.groups.len(), 1);
    assert_eq!(bar.tier2.len(), 1);
    assert!(bar.tier2[0].items[0].text.starts_with("74 fps"));
    let split = info_bar(&data, InfoMode::Split, false);
    assert!(split.tiered && split.split);
    data.collapsed = true;
    let folded = info_bar(&data, InfoMode::TwoTier, false);
    assert!(folded.tiered && folded.collapsed && folded.tier2.is_empty());
    assert_eq!(folded.groups.len(), 1);
    // (the one-row layouts have no second tier and no arrow)
    let full = info_bar(&data, InfoMode::Full, false);
    assert!(!full.tiered && full.tier2.is_empty());
}

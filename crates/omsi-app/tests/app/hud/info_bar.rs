use openomsi_game::testing::hud::{
    delay_item, distance_driven, driving_items, grade_item, grouped, odometer_reading,
    passengers_aboard, pedals_item, skid_item, temperatures, InfoTone,
};
use openomsi_game::testing::settings::INFO_ITEM_KEYS;

#[test]
fn the_distance_driven_is_written_in_kilometres() {
    assert_eq!(distance_driven(0.0), "0.0 km");
    assert_eq!(distance_driven(12_345.0), "12.3 km");
}

/// Kilometres and tenths, the thousands split by a thin space (the word is the icon's
/// tooltip).
#[test]
fn the_odometer_reads_kilometres_and_tenths() {
    assert_eq!(odometer_reading(75_556.639), "75\u{2009}556.6 km");
    assert_eq!(odometer_reading(18_427.3), "18\u{2009}427.3 km");
    assert_eq!(odometer_reading(412.94), "412.9 km");
    assert_eq!(distance_driven(1_234_567.0), "1\u{2009}234.6 km");
}

/// Groups of three from the right, in no language's own separator, never before a sign or a
/// lone digit.
#[test]
fn large_numbers_are_split_in_thousands() {
    assert_eq!(grouped(0.0, 1), "0.0");
    assert_eq!(grouped(999.96, 1), "1\u{2009}000.0");
    assert_eq!(grouped(123_456_789.0, 0), "123\u{2009}456\u{2009}789");
    assert_eq!(grouped(-5.0, 1), "0.0");
}

/// The count alone, or "aboard / places" when the bus has places to count; the word is the
/// icon's tooltip.
#[test]
fn the_passengers_come_with_the_places_of_the_bus() {
    assert_eq!(passengers_aboard(0, None), "0");
    assert_eq!(passengers_aboard(23, None), "23");
    assert_eq!(passengers_aboard(36, Some(111)), "36 / 111");
    assert_eq!(passengers_aboard(1, Some(40)), "1 / 40");
}

#[test]
fn the_temperatures_share_one_unit() {
    assert_eq!(temperatures(15.4, 18.6), "15 / 19 °C");
    assert_eq!(temperatures(-3.0, 21.0), "-3 / 21 °C");
}

/// The delay is a chip: red over a minute late, green over a minute early, the accent in
/// between; a true minus sign ahead, a plus behind.
#[test]
fn the_delay_chip_says_late_early_or_on_time() {
    let late = delay_item(192.0);
    assert_eq!(late.text, "+3:12");
    assert_eq!(late.tone, InfoTone::Delay(1));
    let early = delay_item(-100.0);
    assert_eq!(early.text, "−1:40");
    assert_eq!(early.tone, InfoTone::Delay(-1));
    assert_eq!(delay_item(59.0).tone, InfoTone::Delay(0));
    assert_eq!(delay_item(-59.0).tone, InfoTone::Delay(0));
    assert_eq!(delay_item(-59.0).text, "−0:59");
    assert!(late.tip.is_some());
}

/// The pedals are bars with no text, in the order the pedals stand (clutch, brake,
/// throttle), each in whole percent of how far it is pressed; "Pedals" under the mouse.
#[test]
fn the_pedals_are_three_bars_in_the_order_of_the_pedals() {
    let it = pedals_item(omsi_sim::Controls {
        throttle: 0.42,
        brake: 1.0,
        clutch: 0.0,
        steering: -0.3,
    });
    assert_eq!(it.tone, InfoTone::Pedals([0, 100, 42]));
    assert!(it.text.is_empty());
    assert_eq!(it.icon, None);
    assert_eq!(it.tip.as_deref(), Some("Pedals"));
    // (a value out of its range - a device that reads past its ends - stays in the bar)
    let past = pedals_item(omsi_sim::Controls {
        throttle: 1.3,
        brake: -0.2,
        clutch: 0.996,
        steering: 0.0,
    });
    assert_eq!(past.tone, InfoTone::Pedals([100, 0, 100]));
}

/// The road's grade in whole percent after the slope icon, a true minus downhill and no
/// "-0" on the flat.
#[test]
fn the_road_grade_is_written_in_whole_percent() {
    assert_eq!(grade_item(8.4).text, "8 %");
    assert_eq!(grade_item(-5.6).text, "−6 %");
    assert_eq!(grade_item(-0.4).text, "0 %");
    assert_eq!(grade_item(0.0).text, "0 %");
    let it = grade_item(12.0);
    assert_eq!(it.icon, Some("hud-bus-slope"));
    assert_eq!(it.tip.as_deref(), Some("Road grade"));
    assert_eq!(it.tone, InfoTone::Plain);
}

/// The skid lamp is its icon alone: white while skidding, dark otherwise.
#[test]
fn the_skid_lamp_is_lit_white_while_skidding() {
    let lit = skid_item(true);
    assert_eq!(lit.tone, InfoTone::Light(true));
    assert_eq!(lit.icon, Some("hud-bus-skid-white"));
    let out = skid_item(false);
    assert_eq!(out.tone, InfoTone::Light(false));
    assert_eq!(out.icon, Some("hud-bus-skid-dark"));
    for it in [lit, out] {
        assert!(it.text.is_empty());
        assert_eq!(it.tip.as_deref(), Some("Skidding"));
    }
}

/// The pedals, the grade and the skid lamp come in that order, each as a group, as far as
/// they are known and switched on: a vehicle without a rigid body has only its pedals.
#[test]
fn the_driving_items_are_the_pedals_the_grade_and_the_lamp() {
    let pedals = omsi_sim::Controls::default();
    let tones = |groups: Vec<Vec<openomsi_game::testing::hud::InfoItem>>| {
        groups.iter().map(|g| g[0].tone).collect::<Vec<_>>()
    };
    assert_eq!(
        tones(driving_items(pedals, Some(4.0), Some(true), |_| true)),
        [
            InfoTone::Pedals([0, 0, 0]),
            InfoTone::Plain,
            InfoTone::Light(true)
        ]
    );
    assert_eq!(
        tones(driving_items(pedals, None, None, |_| true)),
        [InfoTone::Pedals([0, 0, 0])]
    );
    // (the Custom layout's switches 9, 10 and 11)
    assert_eq!(
        tones(driving_items(pedals, Some(4.0), Some(false), |i| i != 9)),
        [InfoTone::Plain, InfoTone::Light(false)]
    );
    assert!(driving_items(pedals, Some(4.0), Some(false), |i| i < 9).is_empty());
}

#[test]
fn the_layout_is_read_from_the_setting() {
    use openomsi_game::testing::hud::InfoMode;
    assert_eq!(InfoMode::from_setting("full"), InfoMode::Full);
    assert_eq!(InfoMode::from_setting("compact"), InfoMode::Compact);
    assert_eq!(InfoMode::from_setting("essential"), InfoMode::Essential);
    assert_eq!(InfoMode::from_setting("custom"), InfoMode::Custom);
    assert_eq!(InfoMode::from_setting("sideways"), InfoMode::Full);
}

/// The custom layout leaves out the items switched off; the others keep their layout.
#[test]
fn the_custom_layout_leaves_out_what_is_switched_off() {
    use openomsi_game::testing::hud::{info_bar, InfoData, InfoMode};
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
        items: [true; INFO_ITEM_KEYS.len()],
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
    use openomsi_game::testing::hud::{info_bar, InfoData, InfoMode};
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
        items: [true; INFO_ITEM_KEYS.len()],
        labels: true,
        low_fuel: false,
        collapsed: false,
    };
    let bar = info_bar(&data, InfoMode::Full, true);
    assert!(bar.bottom && !bar.large);
    // (the clock after its icon, named under the mouse)
    assert_eq!(bar.groups[0][0].icon, Some("hud-time"));
    assert!(bar.groups[0][0].tip.is_some());
    assert_eq!(bar.groups.len(), 2);
    assert!(bar.groups[1][0].text.starts_with("74 fps"));
    let compact = info_bar(&data, InfoMode::Compact, false);
    assert_eq!(compact.groups[1][0].text, "74 / 52 fps");
    // (asked for, the frame rate is there in every layout)
    let essentials = info_bar(&data, InfoMode::Essential, false);
    assert_eq!(essentials.groups.len(), 1);
    // (the Essentials layout draws its speed and sign larger)
    assert!(essentials.large && !compact.large);
}

/// The two-tier layouts: the clock in the first tier, the frame rate among the readouts of
/// the second, which the arrow folds away; the split layout keeps them on a plate of its own.
#[test]
fn the_two_tier_layouts_have_a_second_tier_that_folds_away() {
    use openomsi_game::testing::hud::{info_bar, InfoData, InfoMode};
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
        items: [true; INFO_ITEM_KEYS.len()],
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

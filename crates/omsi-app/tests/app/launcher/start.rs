/// A start without a bus leaves the bus, its paint, number and plate and a depot file picked
/// for it out of the duty, on foot - the choice itself keeps them for the next start.
#[test]
fn a_start_without_a_bus_leaves_the_bus_out_of_the_duty_only() {
    let mut d = omsi_launcher_lib::Duty {
        bus: "Vehicles/A/a.bus".into(),
        paint: Some("Red".into()),
        number: Some("3201".into()),
        plate: Some("B-V 1".into()),
        hof: Some("Picked".into()),
        on_foot: Some(false),
        ..Default::default()
    };
    openomsi_game::testing::launcher::state::without_bus(&mut d, Some("Map's own".into()));
    assert!(d.bus.is_empty());
    assert_eq!((&d.paint, &d.number, &d.plate), (&None, &None, &None));
    assert_eq!(d.hof.as_deref(), Some("Map's own"));
    assert_eq!(d.on_foot, Some(true));
    // (the arguments of a duty without a bus: omsi-launcher-core's a_start_without_a_bus_names_none)
}

use openomsi_game::testing::launcher::start::*;

#[test]
fn dates_are_typed_day_first_or_as_the_file_writes_them() {
    assert_eq!(date_typed("14/06/2024").as_deref(), Some("2024-06-14"));
    assert_eq!(date_typed("1.2.1989").as_deref(), Some("1989-02-01"));
    assert_eq!(date_typed("2024-06-14").as_deref(), Some("2024-06-14"));
    assert_eq!(date_typed("29/02/2024").as_deref(), Some("2024-02-29"));
    assert_eq!(date_typed("29/02/2023"), None);
    assert_eq!(date_typed("31/04/2024"), None);
    assert_eq!(date_typed("14/13/2024"), None);
    assert_eq!(date_typed("14/06"), None);
    assert_eq!(date_shown("2024-06-14"), "14/06/2024");
}

#[test]
fn times_are_typed_with_or_without_a_colon() {
    assert_eq!(time_typed("08:30"), Some(8 * 60 + 30));
    assert_eq!(time_typed("8.05"), Some(8 * 60 + 5));
    assert_eq!(time_typed("2215"), Some(22 * 60 + 15));
    assert_eq!(time_typed("7"), Some(7 * 60));
    assert_eq!(time_typed("7:"), Some(7 * 60));
    assert_eq!(time_typed("24:00"), None);
    assert_eq!(time_typed("12:60"), None);
    assert_eq!(time_typed("noon"), None);
}

#[test]
fn start_point_search_ignores_case() {
    assert!(entry_matches("Terminal Laranjeiras", "Laranjeiras"));
    assert!(entry_matches("Terminal Laranjeiras", "  LARANJEIRAS "));
    assert!(entry_matches("Laranjeiras Velha", "laranjeiras"));
    assert!(entry_matches("Terminal Laranjeiras", ""));
    assert!(!entry_matches("Terminal Centro", "Laranjeiras"));
}

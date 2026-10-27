use openomsi_game::testing::server::allows;

#[test]
fn the_list_allows_its_buses_only() {
    let list = vec![
        "Vehicles/MAN_SD200/MAN_SD77.bus".to_string(),
        "Vehicles/MAN_SD202/MAN_D92.bus".to_string(),
    ];
    assert!(allows(&list, "Vehicles/MAN_SD200/MAN_SD77.bus"));
    // another case, backslashes, a folder before it
    assert!(allows(&list, "vehicles\\man_sd200\\MAN_SD77.bus"));
    assert!(allows(&list, "D:/OMSI 2/Vehicles/MAN_SD202/MAN_D92.bus"));
    // the issue's: another bus of the same folder, a mod bus
    assert!(!allows(&list, "Vehicles/MAN_SD200/MAN_SD83.bus"));
    assert!(!allows(&list, "Vehicles/Some_Mod/Bus.bus"));
    // (a file whose name only ends like a listed one)
    assert!(!allows(&list, "Vehicles/MAN_SD200/XMAN_SD77.bus"));
    // no list: every bus
    assert!(allows(&[], "Vehicles/Some_Mod/Bus.bus"));
}

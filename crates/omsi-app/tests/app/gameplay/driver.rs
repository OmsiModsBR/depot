use openomsi_game::testing::driver::lever_grade;

#[test]
fn a_gear_lever_is_known_by_its_variable() {
    assert_eq!(lever_grade("Gearbox_Schalthebel", None), 2);
    assert_eq!(lever_grade("cp_gearlever_pos", None), 2);
    assert_eq!(lever_grade("Antrieb", None), 1);
    assert_eq!(lever_grade("Gear_Selected", None), 1);
}

#[test]
fn a_switch_is_no_gear_lever() {
    // German "schalt" is shifting gears and every switch alike: the Trollino's wiper switch
    // was taken for its gear lever, and the driver's hand went to it at every stop
    assert_eq!(lever_grade("cockpit_wischer_drehschalter", None), 0);
    assert_eq!(lever_grade("Lichtschalter", None), 0);
    assert_eq!(lever_grade("cp_blinker_schalter", None), 0);
    assert_eq!(lever_grade("heizung_luefter_schalter", None), 0);
    assert_eq!(lever_grade("cp_wiper_switch", None), 0);
    assert_eq!(lever_grade("Steering_Wheel", None), 0);
}

#[test]
fn a_forced_name_wins() {
    assert_eq!(
        lever_grade("cockpit_wischer_drehschalter", Some("wischer")),
        3
    );
    assert_eq!(lever_grade("Gearbox_Schalthebel", Some("wischer")), 0);
}

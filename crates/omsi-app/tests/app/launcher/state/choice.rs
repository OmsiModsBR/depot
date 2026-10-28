/// `launcher-duty.json` from before the number plate field: the missing key falls back to
/// the default (no plate), and a typed plate survives a round trip.
#[test]
fn a_remembered_join_is_not_resumed_at_launch() {
    let saved: openomsi_game::testing::launcher::state::Choice =
        serde_json::from_str(r#"{"lan_mode":"join","lan_addr":"main.example.org"}"#).unwrap();
    assert_eq!(saved.lan_mode, "join");
    assert_eq!(
        openomsi_game::testing::launcher::state::Choice::fresh(saved).lan_mode,
        "off"
    );
    let host: openomsi_game::testing::launcher::state::Choice =
        serde_json::from_str(r#"{"lan_mode":"host"}"#).unwrap();
    assert_eq!(
        openomsi_game::testing::launcher::state::Choice::fresh(host).lan_mode,
        "host"
    );
}

/// A choice the Drive page left (it always held a bus) is no preselection on a computer:
/// only a bus picked on Vehicles is.
#[test]
fn the_drive_pages_bus_is_not_a_preselection() {
    let old: openomsi_game::testing::launcher::state::Choice = serde_json::from_str(r#"{"bus":"Vehicles/x.bus","paint":"Red","number":"12","plate":"AB","hof":"H","hof_manual":true,"version":2}"#).unwrap();
    let c = openomsi_game::testing::launcher::state::Choice::fresh(old);
    assert_eq!(
        (
            c.bus.as_str(),
            c.paint.as_str(),
            c.number.as_str(),
            c.plate.as_str(),
            c.hof_manual
        ),
        ("", "", "", "", false)
    );
    assert_eq!(c.version, 3);
    // (one picked since stays)
    let mut now = openomsi_game::testing::launcher::state::Choice::default();
    now.bus = "Vehicles/y.bus".into();
    assert_eq!(
        openomsi_game::testing::launcher::state::Choice::fresh(now).bus,
        "Vehicles/y.bus"
    );
}

/// "Use for the next start" puts the card's choices in the next start, a depot file
/// picked by hand among them; Clear leaves nothing of it, the hand-picked depot file
/// included, so the duty carries the automatic one.
#[test]
fn a_used_bus_and_its_clear() {
    let mut c = openomsi_game::testing::launcher::state::Choice::default();
    c.hof = "Automatic".into();
    c.use_bus(omsi_launcher_ui::vehicles::UseBus {
        file: "Vehicles/A/a.bus".into(),
        paint: "Red".into(),
        hof: Some("Picked".into()),
        number: Some("3201".into()),
        plate: "B-V 1".into(),
    });
    assert_eq!(
        (
            c.bus.as_str(),
            c.paint.as_str(),
            c.hof.as_str(),
            c.number.as_str(),
            c.plate.as_str()
        ),
        ("Vehicles/A/a.bus", "Red", "Picked", "3201", "B-V 1")
    );
    assert!(c.hof_manual);
    c.clear_bus();
    assert_eq!(
        (
            c.bus.as_str(),
            c.paint.as_str(),
            c.number.as_str(),
            c.plate.as_str()
        ),
        ("", "", "", "")
    );
    assert!(!c.hof_manual);
    // (automatic depot file and no number: nothing picked by hand)
    c.use_bus(omsi_launcher_ui::vehicles::UseBus {
        file: "Vehicles/A/a.bus".into(),
        paint: String::new(),
        hof: None,
        number: None,
        plate: String::new(),
    });
    assert!(!c.hof_manual);
    assert_eq!(c.number, "");
}

#[test]
fn an_old_duty_file_loads_and_a_typed_plate_is_kept() {
    let old: openomsi_game::testing::launcher::state::Choice =
        serde_json::from_str(r#"{"bus":"Vehicles/x.bus","map":"maps/x/global.cfg"}"#).unwrap();
    assert_eq!(old.plate, "");
    let mut c = openomsi_game::testing::launcher::state::Choice::default();
    c.plate = "B-AB 1234".into();
    let back: openomsi_game::testing::launcher::state::Choice =
        serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
    assert_eq!(back.plate, "B-AB 1234");
}

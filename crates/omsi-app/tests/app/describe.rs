use openomsi_game::testing::describe::*;

fn eng() -> ControlNames {
    ControlNames::from_table(
        "ENG",
        &[
            ("cp_batterietrennschalter_toggle", "Electricity On/Off"),
            ("parking_brake_toggle", "Parking Brake On/Off"),
            ("bus_doorfront0", "Front Door, 1st Wing Open/Close"),
            ("automatic_R", "Automatic Gearbox: R"),
            ("door_haltewunsch", "Stop Request Button"),
            ("IBIS_loeschen", "IBIS: Delete"),
        ],
    )
}

#[test]
fn official_texts_first() {
    let n = eng();
    assert_eq!(n.control("kw_batterietrennschalter"), "Electricity On/Off");
    assert_eq!(n.control("parking_brake_mouse"), "Parking Brake On/Off");
    assert_eq!(
        n.control("bus_doorfront0_external"),
        "Front Door, 1st Wing Open/Close (outside)"
    );
    assert_eq!(n.control("automatic_R_mouse"), "Automatic Gearbox: R");
    assert_eq!(n.control("door_haltewunsch_2"), "Stop Request Button 2");
    assert_eq!(n.control("IBIS_Loeschen"), "IBIS: Delete");
}

#[test]
fn german_names_translated() {
    let n = eng();
    assert_eq!(n.control("cp_Fahrertuer"), "Driver's door");
    assert_eq!(n.control("Fahrertuer_Links"), "Left driver's door");
    assert_eq!(n.control("Spiegel_Links"), "Left mirror");
    assert_eq!(
        n.control("cp_spiegelheizung_toggle"),
        "Mirror heating on/off"
    );
    assert_eq!(n.control("cp_kneeling_toggle"), "Kneeling on/off");
    assert_eq!(
        n.control("cp_schalter_hub_dn_toggle"),
        "Lift down switch on/off"
    );
    assert_eq!(n.control("taster_heiz_DEF"), "Heating DEF button");
    assert_eq!(
        n.control("cp_heizregler_fussraum"),
        "Heating control footwell"
    );
    assert_eq!(
        n.control("cp_klappfenster_OL1"),
        "Tilting window upper left 1"
    );
    assert_eq!(n.control("door_aussenoeffner"), "Door outside opener");
    assert_eq!(
        n.control("matrix_seitenschildklemme"),
        "Destination display side sign clamp"
    );
    assert_eq!(n.control("cp_dachluke_1"), "Roof hatch 1");
    assert_eq!(n.control("cp_Fahrgastpendel"), "Passenger swing gate");
    assert_eq!(n.control("cp_schluessel_mov"), "Ignition key");
    assert_eq!(n.control("haltestellenbremse"), "Stop brake");
    assert_eq!(
        n.part("Zahltisch_Wechsler_0_05"),
        "Cash desk coin changer 0.05"
    );
    assert_eq!(n.part("SD_Panel_Tachonadel"), "Panel speedometer needle");
    assert_eq!(n.part("GN92_Lenksaeule"), "Steering column");
    assert_eq!(n.part("N92_Blinker_Schalter"), "Indicator switch");
    assert_eq!(n.part("D_Panel_IBIS_0"), "Panel IBIS 0");
    assert_eq!(n.part("Kinderwagenknopf"), "Buggy button");
    assert_eq!(n.part("Haltewunschtaster"), "Stop request button");
    assert_eq!(n.part("Innenbeleuchtung"), "Interior light");
    assert_eq!(n.part("Türflügel_vorne"), "Front door leaf");
    assert_eq!(n.part("EN92_fenster_ext"), "Outer window");
    assert_eq!(n.control("kw_m_engine_startbutton"), "Engine start button");
    // the Citaro LE VER/BVG mod's switches (acceptance test: shown in German)
    assert_eq!(n.control("cp_tuersperre"), "Door lock");
    assert_eq!(n.control("cp_schulfahrschalter"), "Training run switch");
    assert_eq!(n.control("ALMEX_Funktion"), "ALMEX function");
    assert_eq!(n.control("comp_button_qiut"), "Computer button quit");
    assert_eq!(
        n.control("cp_frostregler_temp"),
        "Frost control temperature"
    );
    assert_eq!(
        n.control("cp_schalter_knick_ovrd_toggle"),
        "Articulation override switch on/off"
    );
}

/// Every word of every stock and add-on `[mouseevent]` name is translated (a German word
/// that stays is shown verbatim on an English HUD).
#[test]
fn mod_switch_names_have_no_german_left() {
    let n = eng();
    let german = [
        "sperre", "schul", "funktion", "schalter", "regler", "taster", "tuer", "fenster", "licht",
        "heiz", "knopf", "hebel",
    ];
    for name in [
        "cp_tuersperre",
        "cp_schulfahrschalter",
        "ALMEX_Funktion",
        "cp_doorEntriegelung_01",
        "cp_frostregler_temp",
        "cp_klappfenster_UR4",
        "taster_heiz_Misch",
        "cp_schalter_knick_ovrd_toggle",
        "cp_licht_unterdeck_toggle",
        "rlbnd_linie_select_100_+",
        "rollband_auf",
        "door_kinderwagenwunsch",
        "ramplift_wunsch",
        "cp_novitus_kasetka",
        "rbl_rueck",
        "klappsitzL",
        "Fahrertuer_Rechts",
    ] {
        let t = n.control(name).to_lowercase();
        assert!(!german.iter().any(|g| t.contains(g)), "{name} -> {t}");
    }
}

#[test]
fn german_setting_keeps_german() {
    let n = ControlNames::from_table("deutsch", &[]);
    assert_eq!(n.lang, "DEU");
    assert_eq!(n.control("cp_spiegelheizung_toggle"), "Spiegelheizung");
}

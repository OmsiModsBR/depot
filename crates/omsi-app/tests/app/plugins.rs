use omsi_plugin::InfoValue;
use omsi_plugin::PluginIo;
use openomsi_game::testing::plugins::*;

fn snapshot(info: Vec<(&'static str, InfoValue)>) -> Io<'static> {
    Io {
        vehicle: None,
        others: Vec::new(),
        dt: 0.0,
        message: None,
        info,
        commands: Vec::new(),
        keys: Vec::new(),
        events: Vec::new(),
    }
}

#[test]
fn game_info_names_require_prefix_and_ignore_ascii_case() {
    let io = snapshot(vec![("heading", InfoValue::Num(93.0))]);
    assert_eq!(io.game_number("OPENOMSI_Heading"), Some(93.0));
    for name in [
        "heading",
        "openomsi",
        "openomsi_",
        "openomsi_unknown",
        "openomsí_heading",
        "💡💡💡heading",
    ] {
        assert_eq!(io.game_value(name), None, "{name}");
    }
}

#[test]
fn game_info_callbacks_keep_number_boolean_and_text_types() {
    let io = snapshot(vec![
        ("speed", InfoValue::Num(42.5)),
        ("paused", InfoValue::Bool(true)),
        ("on_foot", InfoValue::Bool(false)),
        ("destination", InfoValue::Text("Žďár nad Sázavou".into())),
        ("map_path", InfoValue::Text(String::new())),
    ]);
    assert_eq!(io.game_number("openomsi_speed"), Some(42.5));
    assert_eq!(io.game_number("openomsi_paused"), Some(1.0));
    assert_eq!(io.game_number("openomsi_on_foot"), Some(0.0));
    assert_eq!(
        io.game_string("openomsi_destination").as_deref(),
        Some("Žďár nad Sázavou")
    );
    assert_eq!(io.game_string("openomsi_map_path"), Some(String::new()));
    assert_eq!(io.game_number("openomsi_destination"), None);
    assert_eq!(io.game_string("openomsi_speed"), None);
    assert_eq!(io.game_string("openomsi_paused"), None);
}

#[test]
fn game_info_numbers_reject_non_finite_values_and_float_overflow() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
        let io = snapshot(vec![("speed", InfoValue::Num(value))]);
        assert_eq!(io.game_number("openomsi_speed"), None);
    }
}

#[test]
fn legacy_vehicle_callbacks_still_require_a_player_vehicle() {
    let mut io = snapshot(vec![
        ("clock", InfoValue::Num(32400.0)),
        (
            "map_path",
            InfoValue::Text("maps/example/global.cfg".into()),
        ),
    ]);
    assert_eq!(io.var("openomsi_clock"), None);
    assert_eq!(io.string("openomsi_map_path"), None);
}

use omsi_sim::testing::vehicle_api::*;
use std::collections::HashMap;

fn snap(vars: &[(&str, f32)], texts: &[(&str, &str)]) -> ApiValue {
    let v: HashMap<String, f32> = vars
        .iter()
        .map(|(k, x)| (k.to_ascii_lowercase(), *x))
        .collect();
    let t: HashMap<String, String> = texts
        .iter()
        .map(|(k, x)| (k.to_string(), x.to_string()))
        .collect();
    let var = move |n: &str| v.get(&n.to_ascii_lowercase()).copied();
    let text = move |n: &str| t.get(n).cloned().unwrap_or_default();
    snapshot(&Inputs {
        var: &var,
        text: &text,
        speed_kmh: 42.349_998,
        steer_deg: 0.0,
        heading: 90.0,
        pitch: 0.0,
        bank: 0.0,
        position: (1.0, 2.0, 3.0),
        engine_running: true,
        interior_light: 0.5,
        crashes: 0,
        last_impact_j: 0.0,
        dirt: 0.0,
        trailers: 0,
    })
}

fn at<'a>(v: &'a ApiValue, path: &str) -> &'a ApiValue {
    path.split('.')
        .fold(v, |cur, key| match (cur, key.parse::<usize>()) {
            (ApiValue::List(l), Ok(n)) => &l[n],
            _ => cur.get(key).unwrap_or_else(|| panic!("no {key} in {path}")),
        })
}

#[test]
fn numbers_are_rounded_without_float_noise() {
    let s = snap(&[], &[]);
    assert_eq!(at(&s, "motion.speedKmh"), &ApiValue::Num(42.3));
}

#[test]
fn a_bus_without_a_signal_reports_null_or_false() {
    let s = snap(&[], &[]);
    assert_eq!(at(&s, "engine.rpm"), &ApiValue::Null);
    assert_eq!(at(&s, "battery.on"), &ApiValue::Null);
    assert_eq!(at(&s, "doors.count"), &ApiValue::Num(0.0));
    assert_eq!(at(&s, "brakes.parking"), &ApiValue::Bool(false));
    assert_eq!(at(&s, "engine.running"), &ApiValue::Bool(true));
}

#[test]
fn doors_are_listed_as_far_as_the_bus_has_them() {
    let s = snap(&[("door_0", 1.0), ("door_1", 0.0), ("door_2", 0.4)], &[]);
    assert_eq!(at(&s, "doors.count"), &ApiValue::Num(3.0));
    assert_eq!(at(&s, "doors.list.0.number"), &ApiValue::Num(1.0));
    assert_eq!(at(&s, "doors.list.0.isOpen"), &ApiValue::Bool(true));
    assert_eq!(at(&s, "doors.list.1.isOpen"), &ApiValue::Bool(false));
    assert_eq!(at(&s, "doors.list.2.open"), &ApiValue::Num(0.4));
    assert_eq!(at(&s, "doors.anyOpen"), &ApiValue::Bool(true));
    // a gap ends the list (door_4 without door_3 is not a door of this bus)
    let g = snap(&[("door_0", 0.0), ("door_2", 1.0)], &[]);
    assert_eq!(at(&g, "doors.count"), &ApiValue::Num(1.0));
}

#[test]
fn what_the_passengers_are_told_decides_whether_a_door_is_open() {
    // a mod bus whose `door_0` is something else, but whose boarding doors are shut
    let s = snap(
        &[
            ("door_0", 1.0),
            ("PAX_Entry0_Open", 0.0),
            ("PAX_Exit0_Open", 0.0),
        ],
        &[],
    );
    assert_eq!(at(&s, "doors.anyOpen"), &ApiValue::Bool(false));
    let o = snap(&[("PAX_Entry0_Open", 1.0), ("PAX_Entry0_Req", 1.0)], &[]);
    assert_eq!(at(&o, "doors.anyOpen"), &ApiValue::Bool(true));
    assert_eq!(
        at(&o, "passengers.entries.0.requested"),
        &ApiValue::Bool(true)
    );
}

#[test]
fn engine_battery_lights_and_texts() {
    let s = snap(
        &[
            ("engine_n", 812.4),
            ("elec_busbar_main", 1.0),
            ("batterie_on", 1.0),
            ("lights_abbl", 1.0),
            ("lights_blinker_l", 1.0),
            ("bremse_feststell", 1.0),
        ],
        &[("number", "4711"), ("act_busstop", "Hauptbahnhof")],
    );
    assert_eq!(at(&s, "engine.rpm"), &ApiValue::Num(812.0));
    assert_eq!(at(&s, "electrics.on"), &ApiValue::Bool(true));
    assert_eq!(at(&s, "battery.on"), &ApiValue::Bool(true));
    assert_eq!(at(&s, "lights.headlights"), &ApiValue::Num(2.0));
    assert_eq!(at(&s, "lights.indicator"), &ApiValue::Num(1.0));
    assert_eq!(at(&s, "brakes.parking"), &ApiValue::Bool(true));
    assert_eq!(at(&s, "info.number"), &ApiValue::Str("4711".into()));
    assert_eq!(
        at(&s, "info.nextStop"),
        &ApiValue::Str("Hauptbahnhof".into())
    );
}

#[test]
fn a_timetable_gives_the_route_with_its_stops() {
    let var = |_: &str| -> Option<f32> { None };
    let text = |n: &str| {
        if n == "IBIS_terminus_name" {
            " Hauptbahnhof ".to_string()
        } else {
            String::new()
        }
    };
    let stops = vec![
        ("Depot".to_string(), 3600.0, 3660.0),
        ("Markt".to_string(), 3900.0, 3930.0),
        ("Bahnhof".to_string(), 4500.0, 4500.0),
    ];
    let r = route(&RouteInputs {
        var: &var,
        text: &text,
        hof: None,
        line: " 5E ",
        delay_s: 61.4,
        stops: &stops,
        next: 1,
    });
    assert_eq!(at(&r, "active"), &ApiValue::Bool(true));
    assert_eq!(at(&r, "line"), &ApiValue::Str("5E".into()));
    assert_eq!(at(&r, "destination"), &ApiValue::Str("Hauptbahnhof".into()));
    assert_eq!(at(&r, "delaySec"), &ApiValue::Num(61.0));
    assert_eq!(at(&r, "current.name"), &ApiValue::Str("Markt".into()));
    assert_eq!(at(&r, "current.arrival"), &ApiValue::Str("01:05".into()));
    assert_eq!(at(&r, "current.departureSec"), &ApiValue::Num(3930.0));
    assert_eq!(at(&r, "terminus.name"), &ApiValue::Str("Bahnhof".into()));
    assert_eq!(at(&r, "stops.0.served"), &ApiValue::Bool(true));
    assert_eq!(at(&r, "stops.1.current"), &ApiValue::Bool(true));
    assert_eq!(at(&r, "stops.2.served"), &ApiValue::Bool(false));
}

#[test]
fn without_a_timetable_the_route_is_quiet() {
    let var = |_: &str| -> Option<f32> { None };
    let text = |_: &str| String::new();
    let r = route(&RouteInputs {
        var: &var,
        text: &text,
        hof: None,
        line: "",
        delay_s: 0.0,
        stops: &[],
        next: 0,
    });
    assert_eq!(at(&r, "active"), &ApiValue::Bool(false));
    assert_eq!(at(&r, "current"), &ApiValue::Null);
    assert_eq!(at(&r, "terminus"), &ApiValue::Null);
    assert_eq!(at(&r, "source"), &ApiValue::Str("none".into()));
}

#[test]
fn the_indicator_switch_wins_over_the_lamps() {
    let s = snap(
        &[("lights_sw_blinker", 2.0), ("lights_blinker_l", 1.0)],
        &[],
    );
    assert_eq!(at(&s, "lights.indicator"), &ApiValue::Num(2.0));
    let h = snap(&[("lights_sw_warnblinker", 1.0)], &[]);
    assert_eq!(at(&h, "lights.hazard"), &ApiValue::Bool(true));
}

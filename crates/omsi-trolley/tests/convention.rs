//! Reading the convention's parameter file and vehicle registry.

use omsi_trolley::{parse_params, parse_registry, ConventionError};

const PARAMS: &str = "\
[global]
active_map=Some_Map
spline_keywords=Wire_A.sli, wire_b ,,Overhead
wire_height=5.8
wire_x_left=-0.255
wire_x_right=0.255 ; a comment after the value

[physics]
pole_raise_duration=1.35
";

#[test]
fn the_wire_keys_are_read() {
    let p = parse_params(PARAMS).unwrap();
    assert_eq!(p.spline_keywords, ["wire_a.sli", "wire_b", "overhead"]);
    assert_eq!(p.wire_height, 5.8);
    assert_eq!((p.wire_x_left, p.wire_x_right), (-0.255, 0.255));
}

#[test]
fn every_key_is_kept_for_later_phases() {
    let p = parse_params(PARAMS).unwrap();
    assert_eq!(p.get("Physics", "Pole_Raise_Duration"), Some("1.35"));
    assert_eq!(p.get("global", "active_map"), Some("Some_Map"));
}

#[test]
fn wire_splines_are_found_by_keyword_whatever_the_case() {
    let p = parse_params(PARAMS).unwrap();
    assert!(p.is_wire_spline("Splines\\Net\\WIRE_A.sli"));
    assert!(p.is_wire_spline("Splines\\overhead_lines\\x.sli"));
    assert!(!p.is_wire_spline("Splines\\Roads\\street_2lanes.sli"));
}

#[test]
fn a_missing_wire_key_is_named() {
    let text = PARAMS.replace("wire_height=5.8\n", "");
    assert_eq!(
        parse_params(&text),
        Err(ConventionError::Missing {
            section: "global".into(),
            key: "wire_height".into()
        })
    );
}

#[test]
fn a_wire_key_that_is_not_a_number_is_named() {
    let text = PARAMS.replace("wire_height=5.8", "wire_height=high");
    let e = parse_params(&text).unwrap_err();
    assert_eq!(e.to_string(), "[global] wire_height=high is not a number");
}

#[test]
fn registry_entries_come_in_file_order_with_their_keys() {
    let reg = parse_registry(
        "\
; a comment
[vehicle:first]
enabled=1
bus_file=Vehicles\\A\\a.bus
pole_base_height=3.455

[something_else]
x=1

[Vehicle:second]
enabled=0
bus_file=Vehicles\\B\\b.bus

[vehicle:third]
BUS_FILE=Vehicles\\C\\c.bus
",
    );
    let names: Vec<_> = reg.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["first", "second", "third"]);
    assert!(reg[0].enabled);
    assert_eq!(reg[0].get("bus_file"), Some("Vehicles\\A\\a.bus"));
    assert_eq!(reg[0].get("POLE_BASE_HEIGHT"), Some("3.455"));
    assert!(!reg[1].enabled, "enabled=0 switches an entry off");
    assert!(reg[2].enabled, "no enabled key counts as enabled");
    assert_eq!(reg[2].get("bus_file"), Some("Vehicles\\C\\c.bus"));
}

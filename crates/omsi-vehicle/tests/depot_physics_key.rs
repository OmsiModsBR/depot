//! The `[depot_physics]` key of a `.bus` file (spec 002, `contracts/bus-key.md`): a section with
//! no value lines whose presence marks a vehicle for Project Depot Physics. It is no unknown
//! keyword, what is under it never reads as another section's values, and nothing in it can stop
//! the load.

use omsi_cfg::CfgFile;
use omsi_vehicle::Vehicle;

fn load(text: &str) -> Vehicle {
    Vehicle::parse(&CfgFile::from_str("test.bus", text))
}

#[test]
fn a_file_without_the_key_has_it_off() {
    let v = load("[friendlyname]\nA\nB\nC\n[mass]\n10\n");
    assert!(!v.depot_physics);
}

#[test]
fn the_key_once_turns_it_on() {
    let v = load("[friendlyname]\nA\nB\nC\n[depot_physics]\n[mass]\n10\n");
    assert!(v.depot_physics);
    assert_eq!(v.mass, 10.0);
}

#[test]
fn the_key_is_no_unknown_keyword() {
    let v = load("[depot_physics]\n[mass]\n10\n");
    assert!(v.unknown_keywords.iter().all(|(k, _)| k != "depot_physics"));
}

#[test]
fn the_key_twice_is_the_same_as_once() {
    let v = load("[depot_physics]\n[mass]\n10\n[depot_physics]\n");
    assert!(v.depot_physics);
    assert_eq!(v.mass, 10.0);
}

#[test]
fn lines_under_the_key_are_left_out_and_never_read_as_another_section() {
    let v = load("[depot_physics]\n1.0\nsome words\n[mass]\n10\n");
    assert!(v.depot_physics);
    assert_eq!(v.mass, 10.0);
    // the numbers under it did not become the mass of the section before it, nor anything else
    let v = load("[mass]\n7\n[depot_physics]\n99\n");
    assert!(v.depot_physics);
    assert_eq!(v.mass, 7.0);
}

#[test]
fn the_key_at_the_end_of_the_file_is_read() {
    assert!(load("[mass]\n10\n[depot_physics]").depot_physics);
    assert!(load("[mass]\n10\n[depot_physics]\n").depot_physics);
}

#[test]
fn an_indented_or_misspelt_key_is_free_text() {
    // (OMSI's rule: a keyword is the whole line)
    assert!(!load("  [depot_physics]\n").depot_physics);
    assert!(!load("[depot_physics] \n").depot_physics);
}

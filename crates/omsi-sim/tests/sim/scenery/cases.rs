use omsi_script::Program;
use omsi_sim::testing::scenery::*;
use std::sync::Arc;

/// The map's strings of a placed object are its string variables, in order, before its
/// {init} runs; a script without that many string variables takes the first ones.
#[test]
fn the_map_strings_are_the_string_variables() {
    let mut p = Program::default();
    p.declare_str_var("A");
    p.declare_str_var("B");
    let inst = SceneryInstance::new(
        Arc::new(p),
        &[],
        omsi_sim::SimClock::default(),
        &["bss1\\14.jpg".into(), "x".into(), "ignored".into()],
    );
    assert_eq!(inst.str_var("A"), "bss1\\14.jpg");
    assert_eq!(inst.str_var("B"), "x");
}

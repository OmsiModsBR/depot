//! #38: a bus whose gearbox is worked out by a script from the speed and the pedal ("automated
//! manual" / automatic: no gate keys). The engine only supplies `Velocity`, `n_Wheel`,
//! `Throttle` and `Clutch`; the shifting is the script's, so the testable part on the engine side
//! is that those inputs reach a frame script and its result comes back out. Everything here is a
//! hand-written script of a few lines, not a real bus. It runs scripts only: no physics step, so
//! it neither depends on nor changes the physics model.

mod support;

use omsi_sim::{VehicleHost, VehicleInstance, VehicleType};
use std::sync::Arc;
use support::scratch::ScratchPack;

/// A shift-by-speed gearbox in four gears with a throttle condition and hysteresis.
const AMT: &str = "\
{frame}
(L.L.Throttle) 0.2 >
{if}
  (L.L.Velocity) 12 > (L.L.gear) 1 = &&
  {if} 2 (S.L.gear) {endif}
  (L.L.Velocity) 28 > (L.L.gear) 2 = &&
  {if} 3 (S.L.gear) {endif}
  (L.L.Velocity) 45 > (L.L.gear) 3 = &&
  {if} 4 (S.L.gear) {endif}
{endif}
(L.L.Velocity) 8 < (L.L.gear) 2 = &&
{if} 1 (S.L.gear) {endif}
(L.L.Velocity) 22 < (L.L.gear) 3 = &&
{if} 2 (S.L.gear) {endif}
(L.L.Velocity) 38 < (L.L.gear) 4 = &&
{if} 3 (S.L.gear) {endif}
{end}
";

fn amt_vehicle(script: &str) -> VehicleInstance {
    let pack = ScratchPack::new("gearbox");
    let dir = pack.path();
    std::fs::write(dir.join("gearbox.osc"), script).unwrap();
    std::fs::write(dir.join("vars.txt"), "gear\n").unwrap();
    let program = omsi_script::compile(&omsi_script::CompileInput {
        builtin_vars: ["Velocity", "Throttle", "n_Wheel", "Clutch"]
            .map(String::from)
            .to_vec(),
        scripts: vec![dir.join("gearbox.osc")],
        varlists: vec![dir.join("vars.txt")],
        ..Default::default()
    });
    assert!(program.errors.is_empty(), "{:?}", program.errors);
    let ty = Arc::new(VehicleType {
        def: Default::default(),
        model: Default::default(),
        model_dir: dir.to_path_buf(),
        program: Arc::new(program),
        meshes: Vec::new(),
        paint_schemes: Vec::new(),
        texchanges: Vec::new(),
        wheel_meshes: Vec::new(),
        suspension_axles: Vec::new(),
        missing_packs: Vec::new(),
        mesh_bounds: Vec::new(),
        mesh_boxes: Vec::new(),
    });
    VehicleInstance::new(ty, VehicleHost::new(Default::default()))
}

fn drive(v: &mut VehicleInstance, speed: f32, throttle: f32) -> f32 {
    assert!(
        v.set_var("Velocity", speed),
        "Velocity is a variable of the script set"
    );
    assert!(v.set_var("Throttle", throttle));
    v.update_scripts_only(0.02);
    v.var("gear").expect("the script's gear variable")
}

#[test]
fn a_throttle_ramp_walks_through_every_gear() {
    let mut v = amt_vehicle(AMT);
    v.set_var("gear", 1.0);
    let mut seen = vec![1.0];
    // speed rises 0..60 km/h in steps, the pedal pressed
    for step in 0..=120 {
        let g = drive(&mut v, step as f32 * 0.5, 0.8);
        if *seen.last().unwrap() != g {
            seen.push(g);
        }
    }
    assert_eq!(
        seen,
        vec![1.0, 2.0, 3.0, 4.0],
        "upshifts in order, each once"
    );
}

#[test]
fn without_the_pedal_it_stays_where_it_is_and_with_it_it_shifts() {
    let mut v = amt_vehicle(AMT);
    v.set_var("gear", 1.0);
    for s in [5.0, 13.0, 20.0] {
        assert_eq!(
            drive(&mut v, s, 0.0),
            1.0,
            "no pedal, no upshift at {s} km/h"
        );
    }
    assert_eq!(drive(&mut v, 20.0, 0.9), 2.0);
}

#[test]
fn slowing_down_shifts_back_with_hysteresis() {
    let mut v = amt_vehicle(AMT);
    v.set_var("gear", 4.0);
    assert_eq!(
        drive(&mut v, 40.0, 0.0),
        4.0,
        "between the thresholds the gear is kept"
    );
    assert_eq!(drive(&mut v, 37.0, 0.0), 3.0);
    assert_eq!(drive(&mut v, 21.0, 0.0), 2.0);
    assert_eq!(drive(&mut v, 7.0, 0.0), 1.0);
}

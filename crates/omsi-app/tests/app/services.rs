use clap::Parser;
use openomsi_game::testing::cli::Args;
use openomsi_game::testing::services::*;
use std::path::PathBuf;
use std::sync::Arc;

/// A vehicle of `script` alone, with the variables `vars`.
fn scripted(script: &str, vars: &str) -> omsi_sim::VehicleInstance {
    let dir = std::env::temp_dir().join(format!(
        "omsi_services_{}_{}",
        std::process::id(),
        vars.len()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.osc"), script).unwrap();
    std::fs::write(dir.join("vars.txt"), vars).unwrap();
    let program = omsi_script::compile(&omsi_script::CompileInput {
        scripts: vec![dir.join("main.osc")],
        varlists: vec![dir.join("vars.txt")],
        ..Default::default()
    });
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(program.errors.is_empty(), "{:?}", program.errors);
    let ty = Arc::new(omsi_sim::VehicleType {
        def: Default::default(),
        model: Default::default(),
        model_dir: PathBuf::new(),
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
    omsi_sim::VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()))
}

#[test]
fn a_bus_without_a_repair_time_is_repaired_at_once() {
    // (the NEOMAN Overhaul: a `malfunction_reset` and no `malfunction_gettime`)
    let mut v = scripted(
        "{trigger:malfunction_reset}\n0 (S.L.Fail_Gelenk_true)\n{end}\n",
        "Fail_Gelenk_true\n",
    );
    v.set_var("Fail_Gelenk_true", 1.0);
    let args = Args {
        repair: true,
        ..Args::parse_from(["depot"])
    };
    let mut clock = omsi_sim::SimClock::default();
    let msg = run_services(&args, &mut v, &mut clock, 30.0, false);
    assert_eq!(msg, vec!["repaired".to_string()]);
    assert_eq!(v.var("Fail_Gelenk_true"), Some(0.0));
    assert_eq!(
        clock.time,
        omsi_sim::SimClock::default().time,
        "no team to wait for"
    );
}

#[test]
fn a_repair_time_is_waited_for_and_the_team_away_from_the_depot() {
    let mut v = scripted(
        "{trigger:malfunction_gettime}\n45\n{end}\n{trigger:malfunction_reset}\n0 (S.L.broken)\n{end}\n",
        "broken\n",
    );
    v.set_var("broken", 1.0);
    let args = Args {
        repair: true,
        ..Args::parse_from(["depot"])
    };
    let mut clock = omsi_sim::SimClock::default();
    let was = clock.time;
    let msg = run_services(&args, &mut v, &mut clock, 30.0, false);
    assert_eq!(v.var("broken"), Some(0.0));
    assert_eq!(clock.time - was, (45.0 + 30.0) * 60.0, "{msg:?}");
}

//! What the tests in `tests/` reach of the crate's insides, built only with the
//! `test-support` feature (the crate's own dev-dependency turns it on; the game builds
//! without it).
//!
//! One module per module of the crate whose tests moved out, each re-exporting what that
//! module has public, plus the builders the tests share.

/// A vehicle of the script `osc` that declares the variables `varlist` and the string
/// variables `stringvarlist` (one a line).
pub fn script_test_vehicle(
    osc: &str,
    varlist: &str,
    stringvarlist: &str,
) -> omsi_sim::VehicleInstance {
    // (a folder of its own: tests run side by side)
    static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("omsi_ibis_dest_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("ibis.osc");
    let vars = dir.join("vars.txt");
    let strings = dir.join("strings.txt");
    std::fs::write(&vars, varlist).unwrap();
    std::fs::write(&strings, stringvarlist).unwrap();
    std::fs::write(&script, osc).unwrap();
    let program = omsi_script::compile(&omsi_script::CompileInput {
        scripts: vec![script],
        varlists: vec![vars],
        stringvarlists: vec![strings],
        ..Default::default()
    });
    assert!(program.errors.is_empty(), "{:?}", program.errors);
    let ty = std::sync::Arc::new(omsi_sim::VehicleType {
        def: Default::default(),
        model: Default::default(),
        model_dir: dir.clone(),
        program: std::sync::Arc::new(program),
        meshes: Vec::new(),
        paint_schemes: Vec::new(),
        texchanges: Vec::new(),
        wheel_meshes: Vec::new(),
        suspension_axles: Vec::new(),
        missing_packs: Vec::new(),
        mesh_bounds: Vec::new(),
        mesh_boxes: Vec::new(),
    });
    std::fs::remove_dir_all(dir).unwrap();
    omsi_sim::VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()))
}

pub mod bus_service {
    pub use crate::bus_service::*;
}
pub mod cli {
    pub use crate::cli::*;
}
pub mod editor {
    pub use crate::editor::*;
}
pub mod humans {
    pub use crate::humans::*;

    pub mod pax {
        pub use crate::humans::pax::*;
    }
}
pub mod journey {
    pub use crate::journey::*;
}
pub mod lights {
    pub use crate::lights::*;
}
pub mod line_times {
    pub use crate::line_times::*;
}
pub mod map_surface {
    pub use crate::map_surface::*;
}
pub mod money {
    pub use crate::money::*;
}
pub mod on_foot {
    pub use crate::on_foot::*;
}
pub mod player {
    pub use crate::player::*;
}
pub mod scene {
    pub use crate::scene::*;

    pub mod ground_query {
        pub use crate::scene::ground_query::*;
    }
}
pub mod schedule {
    pub use crate::schedule::*;
}
pub mod schedule_paper {
    pub use crate::schedule_paper::*;
}
pub mod services {
    pub use crate::services::*;
}
pub mod situation {
    pub use crate::situation::*;
}
pub mod startup {
    pub use crate::startup::*;
}
pub mod tiles {
    pub use crate::tiles::*;
}
pub mod traffic {
    pub use crate::traffic::*;
}
pub mod world_load {
    pub use crate::world_load::*;
}

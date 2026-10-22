//! Simulation runtime.

pub mod ai;
pub mod anim;
pub mod clock;
pub mod collision;
pub mod daylight;
pub mod host;
pub use omsi_html as htmlengine;
pub mod ibis;
pub mod input;
pub mod particles;
pub mod people;
pub mod physics;
pub mod scenery;
pub mod startup;
pub mod textures;
pub mod traffic;
pub mod vehicle;
pub mod vehicle_api;

/// The old paths of `people`, kept for `tests/people_traces.rs`: the trace comparison
/// compiles it on the base commit too, where they are the only ones.
#[doc(hidden)]
pub use people as human;
#[doc(hidden)]
pub use people::omsi_anim as human_omsi;

pub use anim::{AnimState, MeshAnimator};
pub use clock::SimClock;
pub use daylight::Daylight;
pub use host::VehicleHost;
pub use input::{engine_action, EngineAction, KeyboardAxes};
pub use physics::{Controls, Dynamics, KinematicModel, PhysicsState, VehicleMotion};
pub use textures::{htmltex, scripttex, texttex};
pub use vehicle::{VehicleInstance, VehicleType};

#[cfg(feature = "test-support")]
pub mod testing;

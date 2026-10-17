//! Simulation runtime.

pub mod ai;
pub mod anim;
pub mod clock;
pub mod collision;
pub mod crowd;
pub mod daylight;
pub mod host;
pub use omsi_html as htmlengine;
pub mod htmltex;
pub mod ibis;
pub mod human;
pub mod human_omsi;
pub mod input;
pub mod particles;
pub mod physics;
/// The physics' old path, kept for the trace harnesses (`tests/physics_traces.rs` and
/// `tests/ai_traces.rs`): `scripts/physics-trace-diff.sh` builds
/// them on the base commit as well, so they move to `physics` once that path is on the base.
pub use physics as rigid;
pub mod scenery;
pub mod scripttex;
pub mod startup;
pub mod texttex;
pub mod traffic;
pub mod vehicle;
pub mod vehicle_api;

pub use anim::{AnimState, MeshAnimator};
pub use clock::SimClock;
pub use daylight::Daylight;
pub use host::VehicleHost;
pub use input::{engine_action, EngineAction, KeyboardAxes};
pub use physics::{Controls, Dynamics, KinematicModel, PhysicsState, VehicleMotion};
pub use vehicle::{VehicleInstance, VehicleType};

//! Simulation runtime.

pub mod ai;
/// The AI body's module under its old name, for code written against it (the trace
/// harnesses, which `scripts/physics-trace-diff.sh` also runs on older bases).
pub use ai as ai_motion;
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
pub mod rigid;
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

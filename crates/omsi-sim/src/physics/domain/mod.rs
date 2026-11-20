//! What the physics is about: the objects, their fields and what reading them gives. What they
//! do is in `implementation`, one module per domain.

mod body;
mod ground;
mod impact;
mod kinematic;
pub(crate) mod params;
mod spec;
mod train;
mod vehicle;
mod wheel;

pub use body::{CoupledPart, Payload, RigidBody};
pub use ground::{Ground, GroundProbe, RoadWeather, WallFace};
pub use impact::Impact;
pub use kinematic::KinematicModel;
pub use spec::{AiSpec, AiWheelSpec, AxleSpec, PhysicsSpec};
pub use train::{BodyInputs, Joint, JointKind};
pub(crate) use vehicle::EngineState;
pub use vehicle::{Controls, Dynamics, Engine, PhysicsState, VehicleMotion, WheelState};
pub use wheel::RigidWheel;

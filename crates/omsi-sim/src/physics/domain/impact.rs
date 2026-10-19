//! What a hit of the body against an obstacle tells the scripts ([`Impact`]).

use glam::Vec3;

/// One hit of the body against an obstacle.
#[derive(Debug, Clone, Copy)]
pub struct Impact {
    /// Where it hit, in the body frame relative to the model origin (x right, y forward, z up).
    pub point: Vec3,
    /// Closing speed along the contact normal (m/s).
    pub speed: f32,
    /// Kinetic energy the impact took away (J).
    pub energy: f32,
    /// Index into the obstacle list the body was tested against; for a wheel stopped by a
    /// face (`RigidBody::wheel_impacts`), the wheel's.
    pub obstacle: usize,
    /// A `[crashmode_pole]` obstacle that broke off.
    pub broke: bool,
    /// Direction the obstacle was pushed (world, ground plane).
    pub push: Vec3,
}

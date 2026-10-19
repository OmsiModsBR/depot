//! The kinematic model ([`KinematicModel`]).

/// A deliberately simple model: the forces along the vehicle (drive, brakes, rolling
/// resistance), the front wheels' angle eased towards the driver's, and the heading turned by
/// the curvature tan(angle) / wheelbase. The vehicle follows the ground under its wheels.
#[derive(Debug, Clone, Copy, Default)]
pub struct KinematicModel {
    pub mass_kg: f32,
    pub rolling_resistance: f32,
    /// From the rearmost axle to the frontmost (m, at least 1).
    pub wheelbase: f32,
    pub max_steer_deg: f32,
}

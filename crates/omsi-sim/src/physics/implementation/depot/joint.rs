//! The joints of articulated vehicles as Project Depot Physics bends the section behind.

/// The least arm (m) from a section's `[rot_pnt_long]` line to its joint that the curvature
/// divides by: a guard for a file whose joint stands on that line.
const ARM_MIN: f32 = 0.5;

/// The curvature (1/m, positive to the right) a section behind a joint bent by `alpha` turns
/// on: sin alpha over the distance `follow_arm` from its joint to its own `[rot_pnt_long]` line.
pub(in crate::physics) fn follower_kappa(alpha: f32, follow_arm: f32) -> f32 {
    alpha.sin() / follow_arm.abs().max(ARM_MIN)
}

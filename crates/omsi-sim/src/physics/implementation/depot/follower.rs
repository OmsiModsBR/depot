//! `D-2`: the tyres of a section behind a joint steer on the road as its axles show it, every
//! axle pointing at the centre of the turn the joint bends the section on, and each tyre is
//! held across where it touches the road.

use glam::{Quat, Vec3};

/// Whether the tyres of a section behind a joint steer on the road.
pub(in crate::physics) const TYRES_STEER: bool = true;

/// Where the hold across of a tyre of a section behind a joint acts, from the centre of gravity
/// (world), the tyre touching the road at `r`: at the tyre.
pub(in crate::physics) fn hold_at(r: Vec3, _rot: Quat, _line: f32) -> Vec3 {
    r
}

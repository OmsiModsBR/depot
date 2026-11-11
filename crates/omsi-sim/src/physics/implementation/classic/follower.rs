//! `K-2`: a section behind a joint is held across at its turning line, the `[rot_pnt_long]`
//! line: its axles show the bend the joint makes it turn on (the scripts animate it from
//! `Axle_Steering`), but its tyres stay square to the body and their hold across acts on that
//! line, so the section turns about it whatever axles it has.

use glam::{Quat, Vec3};

/// Whether the tyres of a section behind a joint steer on the road.
pub(in crate::physics) const TYRES_STEER: bool = false;

/// Where the hold across of a tyre of a section behind a joint acts, from the centre of gravity
/// (world), the tyre touching the road at `r`, the body turned by `rot` and its turning line at
/// `line` (body frame): on the line, level with the tyre.
pub(in crate::physics) fn hold_at(r: Vec3, rot: Quat, line: f32) -> Vec3 {
    let tyre = rot.inverse().mul_vec3(r);
    rot.mul_vec3(Vec3::new(tyre.x, line, tyre.z))
}

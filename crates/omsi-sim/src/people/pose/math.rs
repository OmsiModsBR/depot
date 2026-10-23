//! The small vector, rotation and two-bone IK helpers the pose is built from.

use glam::{Affine3A, Mat3A, Quat, Vec3, Vec3A};

/// Move `x` towards `to` by at most `step`.
pub(super) fn approach(x: f32, to: f32, step: f32) -> f32 {
    if (to - x).abs() <= step {
        to
    } else {
        x + step * (to - x).signum()
    }
}

/// Exponential smoothing factor for time constant `tau`.
pub(super) fn ease_k(dt: f32, tau: f32) -> f32 {
    1.0 - (-dt / tau.max(1e-3)).exp()
}

/// Rotation about z turning +y towards +x by `deg` (OMSI's heading sense).
pub(super) fn yaw_quat(deg: f32) -> Quat {
    Quat::from_rotation_z(-deg.to_radians())
}

/// A frame from a bone axis and a second direction (made perpendicular to it).
pub(super) fn basis(axis: Vec3, front: Vec3) -> Mat3A {
    let a = axis.normalize_or(Vec3::Z);
    let mut f = front - a * a.dot(front);
    if f.length_squared() < 1e-8 {
        f = a.any_orthonormal_vector();
    }
    let f = f.normalize();
    Mat3A::from_cols(a.into(), f.into(), a.cross(f).into())
}

/// The rotation taking the rest frame (axis0, front0) onto (axis1, front1).
pub(super) fn bone_rot(axis0: Vec3, front0: Vec3, axis1: Vec3, front1: Vec3) -> Mat3A {
    basis(axis1, front1) * basis(axis0, front0).transpose()
}

/// A rigid bone transform: rotation `r` about the rest joint `rest`, which moves to `at`.
pub(super) fn joint_xf(rest: Vec3, at: Vec3, r: Mat3A) -> Affine3A {
    Affine3A::from_mat3_translation(r.into(), at - Vec3::from(r * Vec3A::from(rest)))
}

pub(super) fn about(pivot: Vec3, q: Quat) -> Affine3A {
    Affine3A::from_translation(pivot) * Affine3A::from_quat(q) * Affine3A::from_translation(-pivot)
}

/// Two-bone IK: from `root` with bone lengths `l1`, `l2` to `target`, bending towards
/// `pole`. Returns the middle joint, the end joint actually reached, and the bend-plane
/// normal (the hinge).
pub(super) fn two_bone(
    root: Vec3,
    l1: f32,
    l2: f32,
    target: Vec3,
    pole: Vec3,
) -> (Vec3, Vec3, Vec3) {
    let d = target - root;
    let len = d.length();
    let dir = if len > 1e-5 { d / len } else { -Vec3::Z };
    let reach = (l1 + l2) * 0.9995;
    // (a bone next to nothing puts the floor over the reach: clamp would panic)
    let dist = len.clamp(((l1 - l2).abs() + 1e-3).min(reach), reach);
    let mut n = dir.cross(pole);
    if n.length_squared() < 1e-8 {
        n = dir.cross(Vec3::Y);
        if n.length_squared() < 1e-8 {
            n = Vec3::X;
        }
    }
    let n = n.normalize();
    let cos1 = ((l1 * l1 + dist * dist - l2 * l2) / (2.0 * l1 * dist)).clamp(-1.0, 1.0);
    let a1 = cos1.acos();
    let mid = root + Quat::from_axis_angle(n, a1) * dir * l1;
    let end = root + dir * dist;
    (mid, end, n)
}

/// Limit a rotation to `max` radians.
pub(super) fn limit_quat(q: Quat, max: f32) -> Quat {
    let q = if q.w < 0.0 { -q } else { q };
    let (axis, angle) = q.to_axis_angle();
    if angle.abs() > max && axis.is_finite() {
        Quat::from_axis_angle(axis, max * angle.signum())
    } else {
        q
    }
}

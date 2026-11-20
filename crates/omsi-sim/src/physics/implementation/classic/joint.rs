//! `K-2` to `K-4`: the joints of articulated vehicles as OMSI Classic runs them, where they
//! differ from Project Depot Physics (see `docs/physics/classic-model.md`).

use crate::physics::domain::{Joint, JointKind, RigidBody};
use glam::{Quat, Vec3};

/// The most the direction of travel at a joint may lean from a section's axis before the
/// tangent of `K-4` is taken (rad): a guard against an endless curvature, not a rule.
const LEAN_MAX: f32 = 1.5;
/// The least arm (m) from a section's `[rot_pnt_long]` line to its joint that `K-4` divides by:
/// a guard for a file whose joint stands on that line.
const ARM_MIN: f32 = 0.5;
/// How close (rad) to `alpha_max` a joint counts as at its limit (`K-3`): the joint's
/// constraint stops it at the limit to within the solver's tolerance.
const LIMIT_SLACK: f32 = 0.0035;

/// `K-4`: the curvature (1/m, positive to the right) a section behind a joint turns on. The
/// joint's point moves the way the section ahead carries it - `atan(lead_arm · lead_kappa)`
/// from the lead's axis, `lead_arm` the joint's place less the lead's own `[rot_pnt_long]` -,
/// which is `alpha` more from the follower's axis; the follower turns about its own
/// `[rot_pnt_long]` line so that its joint point, `follow_arm` ahead of that line, moves that
/// way.
pub(in crate::physics) fn follower_kappa(
    alpha: f32,
    lead_arm: f32,
    lead_kappa: f32,
    follow_arm: f32,
) -> f32 {
    let arm = if follow_arm.abs() < ARM_MIN {
        ARM_MIN.copysign(follow_arm)
    } else {
        follow_arm
    };
    let lean = ((lead_arm * lead_kappa).atan() + alpha).clamp(-LEAN_MAX, LEAN_MAX);
    let kappa = lean.tan() / arm;
    if kappa.is_finite() {
        kappa
    } else {
        0.0
    }
}

/// `K-3`: whether a turntable joint stands at its `alpha_max` (a hitch, type 0, has no limit).
pub(in crate::physics) fn at_alpha_limit(joint: &Joint, bodies: &[RigidBody]) -> bool {
    joint.kind == JointKind::Articulation
        && joint.angles(bodies).0.abs() >= joint.alpha_max - LIMIT_SLACK
}

/// `K-2`: every section behind a joint put back on its joint at the end of
/// the step, the joints in order from the front. A turntable's section is turned to the lead's
/// attitude with its own yaw and pitch against it, held to the joint's angles, and no roll
/// against it; a hitch's section is swung about its `[rot_pnt_long]` point until its joint
/// points at the lead's. Then the section is moved until the two joint points meet. The
/// sections keep their velocities.
pub(in crate::physics) fn seat_followers(bodies: &mut [RigidBody], joints: &[Joint]) {
    for j in joints {
        let lead_rot = bodies[j.lead].orientation;
        let (_, _, err) = j.ball(&bodies[j.lead], &bodies[j.follow]);
        let f = &mut bodies[j.follow];
        match j.kind {
            JointKind::Articulation => {
                let rel = lead_rot.inverse() * f.orientation;
                let fwd = rel.mul_vec3(Vec3::Y);
                // the follower's yaw against the lead (left positive, which is `alpha`) and
                // its pitch (nose up positive, which is minus `beta`): a puller's section
                // pitches about its own transverse axis and then yaws about the lead's
                // vertical, a pusher's yaws about its own vertical behind the hinge, which
                // pitches about the lead's transverse axis (the joint's axes, `Joint::axes`)
                let pusher = j.pitch_offset > 0.0;
                let (yaw, pitch) = if pusher {
                    ((-fwd.x).clamp(-1.0, 1.0).asin(), fwd.z.atan2(fwd.y))
                } else {
                    ((-fwd.x).atan2(fwd.y), fwd.z.clamp(-1.0, 1.0).asin())
                };
                let yaw = yaw.clamp(-j.alpha_max.max(0.0), j.alpha_max.max(0.0));
                let (lo, hi) = (-j.beta_max, -j.beta_min);
                let pitch = if lo <= hi { pitch.clamp(lo, hi) } else { pitch };
                let (yaw, pitch) = (Quat::from_rotation_z(yaw), Quat::from_rotation_x(pitch));
                let rel = if pusher { pitch * yaw } else { yaw * pitch };
                f.orientation = (lead_rot * rel).normalize();
            }
            JointKind::Ball => {
                let at = |b: &RigidBody, p: Vec3| b.orientation.mul_vec3(p - b.cog);
                let pivot = at(f, Vec3::new(0.0, f.rot_pnt_long, 0.0));
                let ball = at(f, j.anchor_follow);
                let (from, to) = (ball - pivot, ball + err - pivot);
                if from.length() > 1e-3 && to.length() > 1e-3 {
                    let turn = Quat::from_rotation_arc(from.normalize(), to.normalize());
                    // turned about the pivot: the centre of gravity swings with it
                    f.position += (pivot - turn.mul_vec3(pivot)).as_dvec3();
                    f.orientation = (turn * f.orientation).normalize();
                }
            }
        }
        let (_, _, err) = j.ball(&bodies[j.lead], &bodies[j.follow]);
        let f = &mut bodies[j.follow];
        let shift = err.as_dvec3();
        f.position += shift;
        f.last_position += shift;
    }
}

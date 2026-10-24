//! The body ([`RigidBody`]) and the parts it tows ([`CoupledPart`]), with what reading them gives.

use super::{Impact, RigidWheel};
use glam::{DVec3, Quat, Vec3};

/// A part towed behind the body (the rear section of an articulated bus). The parts follow
/// kinematically, so what acts on them along their length arrives at the joint: the drive
/// of a driven axle - the O530G/GL Facelift and the stock GN92 are pushers, whose front
/// sections roll free -, the brakes and rolling resistance, the weight on a grade and the
/// mass. The body takes the part of it along its own axis (cosine of the articulation
/// angle); the side part is what a pusher's joint damping and the rear section's tyres
/// hold. Handed on whole, it turned a pusher standing against its brakes round on the spot
/// and drove every bend into a jack-knife: the kinematic rear section has no tyres of its
/// own to resist it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoupledPart {
    /// The coupling on this body (body frame, relative to the model origin).
    pub point: Vec3,
    /// The direction of travel of the part hung on that coupling (world).
    pub dir: Vec3,
    /// Mass (kg).
    pub mass: f32,
    /// Driven wheels, their radius (m) and the static load on them (N): they pass on no
    /// more than friction times that load.
    pub driven_wheels: usize,
    pub radius: f32,
    pub driven_load: f32,
    /// What its brakes and its rolling resistance hold (N), and the static load on all its
    /// wheels (N).
    pub brake: f32,
    pub load: f32,
}

#[derive(Debug, Clone)]
pub struct RigidBody {
    pub(crate) mass: f32,
    /// Body-frame inertia (diagonal, kg m²).
    pub(crate) inertia: Vec3,
    /// Centre of gravity relative to the model origin (m).
    pub(crate) cog: Vec3,
    /// World position of the centre of gravity.
    pub(crate) position: DVec3,
    pub(crate) orientation: Quat,
    pub(crate) velocity: Vec3,
    /// Angular velocity in the body frame (rad/s).
    pub(crate) omega: Vec3,
    pub(crate) wheels: Vec<RigidWheel>,
    /// Axle index of each wheel (two per axle, left then right).
    pub(crate) wheel_axle: Vec<usize>,
    pub(crate) steer_deg: f32,
    /// The curvature the steering asks for this step (1/m, positive to the right).
    pub(crate) kappa: f32,
    pub(crate) max_steer_deg: f32,
    /// `[rot_pnt_long]` and `[inv_min_turnradius]`: the line the bus turns about and the
    /// curvature of the full lock.
    pub(crate) rot_pnt_long: f32,
    pub(crate) inv_min_turn_radius: f32,
    /// sqrt(sum of the springs / mass) (1/s): OMSI damps the body's pitch and roll by
    /// sin(x)/x a frame with x = 1.5 of it times the frame.
    pub(crate) body_freq: f32,
    /// OMSI's two states of the tyres across: holding - the bus follows
    /// its wheels however quickly they are turned, as long as the bend asks no more than
    /// the grip of all its tyres and no wheel spins or locks - or sliding, each tyre giving
    /// at most its own grip until the sideways speed at every tyre is under 0.1 m/s again.
    pub(crate) holding: bool,
    /// The force (N) a joint pushed the body with across it over the last substep (to its
    /// right): the tyres take it besides the bend's pull while they hold.
    pub(crate) joint_across: f32,
    pub(crate) rolling_resistance: f32,
    /// Body-frame acceleration of the last step (m/s²), for `A_Trans_*`.
    pub(crate) accel_body: Vec3,
    pub(crate) friction: f32,
    /// Wheels a face stopped during the last step, one entry per wheel.
    pub(crate) wheel_impacts: Vec<Impact>,
    /// The parts coupled behind (see [`CoupledPart`]); `drive_torque` is shared by their
    /// driven wheels and the body's own.
    pub(crate) coupled: Vec<CoupledPart>,
    /// Obstacles (by id) the body was put down inside: left alone until it is out of them
    /// (a bus spawned in a shelter's or a depot hall's box was held there for good - every
    /// move towards the nearest side pushed it back in). `None` until the first collision
    /// check after `place`.
    pub(crate) spawned_inside: Option<Vec<i64>>,
    /// The faces a wheel cannot climb stop it (a platform's edge, a kerb too high for it).
    /// Off with the collisions with objects: the wheels then keep to the ground under them,
    /// as OMSI's do - its wheels know no faces - and a step nobody sees (a height profile
    /// over the drawn road, a helper object's collision box) is no invisible wall.
    pub(crate) wheel_walls: bool,
    /// The time the frames brought that no substep has run yet (s, less than a substep), and
    /// where the body stood a substep before the last: the body is shown that far between the
    /// two (see `RigidBody::shown_origin`), so that its motion on screen stays smooth while a
    /// frame runs one substep and the next two.
    pub(crate) unstepped: f32,
    pub(crate) last_position: DVec3,
    pub(crate) last_orientation: Quat,
}

impl RigidBody {
    /// The body's mass (kg).
    pub fn mass(&self) -> f32 {
        self.mass
    }

    /// The body-frame inertia (diagonal, kg m²).
    pub fn inertia(&self) -> Vec3 {
        self.inertia
    }

    /// The centre of gravity relative to the model origin (m).
    pub fn cog(&self) -> Vec3 {
        self.cog
    }

    /// The centre of gravity in the world.
    pub fn position(&self) -> DVec3 {
        self.position
    }

    /// The body's rotation, model frame to world.
    pub fn orientation(&self) -> Quat {
        self.orientation
    }

    /// The velocity of the centre of gravity (m/s, world).
    pub fn velocity(&self) -> Vec3 {
        self.velocity
    }

    /// The angular velocity (rad/s, body frame).
    pub fn omega(&self) -> Vec3 {
        self.omega
    }

    /// The body-frame acceleration of the last step (m/s²), for `A_Trans_*`.
    pub fn accel_body(&self) -> Vec3 {
        self.accel_body
    }

    /// The wheels, two per axle (left then right).
    pub fn wheels(&self) -> &[RigidWheel] {
        &self.wheels
    }

    /// The wheels a face stopped during the last step.
    pub fn wheel_impacts(&self) -> &[Impact] {
        &self.wheel_impacts
    }

    /// The steering angle (deg).
    pub fn steer_deg(&self) -> f32 {
        self.steer_deg
    }

    /// The steering lock of the front wheels (deg).
    pub fn max_steer_deg(&self) -> f32 {
        self.max_steer_deg
    }

    /// The curvature the steering asks for this step (1/m, positive to the right).
    pub fn kappa(&self) -> f32 {
        self.kappa
    }

    /// Whether the tyres hold across (OMSI's holding state) rather than slide.
    pub fn holding(&self) -> bool {
        self.holding
    }

    /// The rolling resistance (N).
    pub fn rolling_resistance(&self) -> f32 {
        self.rolling_resistance
    }

    /// The grip of the road under the body (the surface's friction).
    pub fn friction(&self) -> f32 {
        self.friction
    }

    /// The parts coupled behind.
    pub fn coupled(&self) -> &[CoupledPart] {
        &self.coupled
    }

    /// The obstacles (by id) the body was put down inside, `None` until the first collision check after `place`.
    pub fn spawned_inside(&self) -> Option<&[i64]> {
        self.spawned_inside.as_deref()
    }
}

/// What the tests set on a body, built only with the `test-support` feature.
#[cfg(feature = "test-support")]
impl RigidBody {
    /// The wheels, to set what a test changes on them (see the setters of
    /// [`RigidWheel`]).
    pub fn wheels_mut(&mut self) -> &mut [RigidWheel] {
        &mut self.wheels
    }

    /// Sets the velocity of the centre of gravity (m/s, world).
    pub fn set_velocity(&mut self, v: Vec3) {
        self.velocity = v;
    }

    /// Sets the grip of the road under the body.
    pub fn set_friction(&mut self, friction: f32) {
        self.friction = friction;
    }

    /// The trailers' parts towed by the body, to change their brakes.
    pub fn coupled_mut(&mut self) -> &mut Vec<CoupledPart> {
        &mut self.coupled
    }

    /// Sets the mass (kg).
    pub fn set_mass(&mut self, mass: f32) {
        self.mass = mass;
    }
}

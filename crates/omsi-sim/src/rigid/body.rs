//! The body: its mass, inertia and pose built from the `.bus` file ([`RigidBody`]), the parts
//! it tows ([`CoupledPart`]), and its motion over a substep (semi-implicit Euler) with OMSI's
//! damping of pitch and roll ([`body_damping`]).

use super::collide::Impact;
use super::ground::GroundProbe;
use super::params::{BODY_DAMPING, BUMP, DRY_GRIP, GRAVITY, MAX_STEER, OMSI_FRAME, TIP_OVER};
use super::spec::PhysicsSpec;
use super::train::{step_train, BodyInputs};
use super::tyre::Substep;
use super::wheel::RigidWheel;
use crate::collision::Obb;
use glam::{DVec3, Quat, Vec3};
use omsi_vehicle::Vehicle;

/// What is left of the body's pitch and roll rate after `dt` seconds of OMSI's damping.
pub(super) fn body_damping(body_freq: f32, dt: f32) -> f32 {
    let x0 = BODY_DAMPING * body_freq * OMSI_FRAME;
    if x0 <= 1e-6 || dt <= 0.0 {
        return 1.0;
    }
    (x0.sin() / x0).clamp(0.1, 1.0).powf(dt / OMSI_FRAME)
}

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
}

impl RigidBody {
    /// `hub_heights[a]`: unloaded hub height of axle `a` above the model origin; the tyre
    /// radius when not given. The player's vehicle gives none: OMSI's springs stand on the
    /// model origin's plane, whatever the model's wheels look like.
    pub fn from_definition(def: &Vehicle, hub_heights: &[Option<f32>]) -> RigidBody {
        let spec = PhysicsSpec::from(def);
        let mass = spec.mass_kg;
        let mut wheels = Vec::new();
        let mut wheel_axle = Vec::new();
        for (ai, a) in spec.axles.iter().enumerate() {
            let r = a.radius;
            let z = hub_heights.get(ai).copied().flatten().unwrap_or(r);
            for side in [-1.0f32, 1.0] {
                wheels.push(RigidWheel { spin: 0.0, slipping: false, locked: false, inertia_inv: a.inertia_inv, attach: Vec3::new(side * a.track, a.long, z), lever: side * (a.outer - a.track), radius: r, driven: a.driven, steered: (a.long - spec.front_long).abs() < 0.01, steer: 0.0, spring: a.spring, damper: a.damper, max_force: a.max_force, spring_factor: 1.0, rest_load: 0.0, compression: 0.0, compression_rate: 0.0, touch: None, on_ground: true, rotation_deg: 0.0, rpm: 0.0, load: 0.0, ground_z: 0.0, ground_seen: false, walls: Vec::new(), step_force: 0.0 });
                wheel_axle.push(ai);
            }
        }
        for (w, a) in wheels.iter_mut().zip(&wheel_axle) {
            w.rest_load = spec.rest_loads.get(*a).copied().unwrap_or(0.0);
        }
        RigidBody { mass, inertia: spec.inertia, cog: spec.cog, position: DVec3::ZERO, orientation: Quat::IDENTITY, velocity: Vec3::ZERO, omega: Vec3::ZERO, wheels, wheel_axle, steer_deg: 0.0, kappa: 0.0, max_steer_deg: spec.max_steer_deg, rot_pnt_long: spec.rot_pnt_long, inv_min_turn_radius: spec.inv_min_turn_radius, body_freq: spec.body_freq, holding: true, rolling_resistance: spec.rolling_resistance, accel_body: Vec3::ZERO, friction: DRY_GRIP, wheel_impacts: Vec::new(), coupled: Vec::new(), spawned_inside: None, wheel_walls: true }
    }

    /// Place the body at rest with its wheels on the ground plane at `origin.z`: heading
    /// `heading_deg`, springs already carrying the load (no drop and bounce at the start).
    pub fn place(&mut self, origin: DVec3, heading_deg: f64) {
        self.orientation = Quat::from_rotation_z((-heading_deg).to_radians() as f32);
        let origin = origin + DVec3::Z * self.rest_sag() as f64;
        self.position = origin + self.orientation.mul_vec3(self.cog).as_dvec3();
        self.velocity = Vec3::ZERO;
        self.omega = Vec3::ZERO;
        self.spawned_inside = None;
        for w in self.wheels.iter_mut() {
            w.compression = w.rest_compression().min(BUMP);
            w.compression_rate = 0.0;
            w.touch = None;
        }
        // standing: the ground holds the body up against gravity
        self.accel_body = Vec3::new(0.0, 0.0, GRAVITY);
        for w in self.wheels.iter_mut() {
            w.ground_seen = false;
            w.walls.clear();
        }
    }

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

    /// The wheels, to set what a test or a script changes on them (see the setters of
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

    /// Model origin (the point the renderer places) in the world.
    pub fn origin(&self) -> DVec3 {
        self.position - self.orientation.mul_vec3(self.cog).as_dvec3()
    }

    /// Heading (deg, clockwise from north), pitch and bank (deg) of the body.
    pub fn heading_pitch_bank(&self) -> (f64, f32, f32) {
        let f = self.orientation.mul_vec3(Vec3::Y);
        let r = self.orientation.mul_vec3(Vec3::X);
        let heading = (f.x.atan2(f.y) as f64).to_degrees().rem_euclid(360.0);
        let pitch = f.z.clamp(-1.0, 1.0).asin().to_degrees();
        let bank = (-r.z).clamp(-1.0, 1.0).asin().to_degrees();
        (heading, pitch, bank)
    }

    /// The steering angle of wheel `i`'s axle (rad, positive to the right): the angle at the
    /// middle of the axle, atan((long - rot_pnt_long) x curvature), the same for its left and
    /// right wheel. That is what OMSI hands the scripts as `Axle_Steering_<n>_L` and
    /// `_R` alike (both variables get the same value) -
    /// the tyres' own angles (Ackermann, `RigidWheel::steer`) made the steering
    /// wheel of the cab, which turns with `Axle_Steering_0_L`, go further to the left than
    /// to the right (#953, #1073).
    pub fn axle_steer(&self, i: usize) -> f32 {
        let Some(w) = self.wheels.get(i) else { return 0.0 };
        ((w.attach.y - self.rot_pnt_long) * self.kappa).atan().clamp(-MAX_STEER, MAX_STEER)
    }

    /// Forward speed (m/s) in the body frame.
    pub fn forward_speed(&self) -> f32 {
        self.velocity.dot(self.orientation.mul_vec3(Vec3::Y))
    }

    pub fn kinetic_energy(&self) -> f32 {
        0.5 * self.mass * self.velocity.length_squared() + 0.5 * (self.inertia * self.omega * self.omega).element_sum()
    }

    /// Inverse of the effective mass the body shows along world direction `n` at world
    /// offset `r` from its centre of gravity.
    pub(super) fn inv_mass_at(&self, r: Vec3, n: Vec3) -> f32 {
        let inv = self.orientation.inverse();
        let (rb, nb) = (inv.mul_vec3(r), inv.mul_vec3(n));
        let w = rb.cross(nb) / self.inertia;
        1.0 / self.mass + w.cross(rb).dot(nb)
    }

    /// Apply the world impulse `p` at world offset `r` from the centre of gravity.
    pub(super) fn apply_impulse(&mut self, r: Vec3, p: Vec3) {
        let inv = self.orientation.inverse();
        self.velocity += p / self.mass;
        self.omega += inv.mul_vec3(r).cross(inv.mul_vec3(p)) / self.inertia;
    }

    /// The body's `[boundingbox]` in the world: its footprint turned with the heading, its
    /// height range over all eight corners (a pitched bus reaches lower at one end).
    pub fn body_box(&self, bb: [f32; 6]) -> Obb {
        let (heading, _, _) = self.heading_pitch_bank();
        let origin = self.origin();
        let mut o = Obb::from_box(bb, origin, heading);
        let centre = origin + self.orientation.mul_vec3(Vec3::new(bb[3], bb[4], bb[5])).as_dvec3();
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for sx in [-0.5f32, 0.5] {
            for sy in [-0.5f32, 0.5] {
                for sz in [-0.5f32, 0.5] {
                    let z = centre.z + self.orientation.mul_vec3(Vec3::new(bb[0] * sx, bb[1] * sy, bb[2] * sz)).z as f64;
                    lo = lo.min(z);
                    hi = hi.max(z);
                }
            }
        }
        o.z0 = lo;
        o.z1 = hi;
        o
    }

    /// The body's `[boundingbox]` as it stands, turned, pitched and banked, grown by
    /// `margin` on every side.
    pub fn body_solid(&self, bb: [f32; 6], margin: f32) -> crate::collision::Box3 {
        let r = self.orientation;
        let centre = self.origin() + r.mul_vec3(Vec3::new(bb[3], bb[4], bb[5])).as_dvec3();
        crate::collision::Box3 {
            center: centre,
            axes: [r.mul_vec3(Vec3::X).as_dvec3(), r.mul_vec3(Vec3::Y).as_dvec3(), r.mul_vec3(Vec3::Z).as_dvec3()],
            half: (Vec3::new(bb[0], bb[1], bb[2]) * 0.5 + Vec3::splat(margin)).as_dvec3(),
        }
    }

    /// One step. `drive_torque` (N m at the driven wheels, from `M_Wheel`), `brake` per
    /// wheel (N), `steer` −1..1, `probe(x, y, z_top)` the ground at a point: a train of one
    /// body (see [`step_train`]).
    pub fn step(&mut self, dt: f32, drive_torque: f32, brake: &[f32], steer: f32, probe: &dyn Fn(f64, f64, f64) -> GroundProbe) {
        step_train(std::slice::from_mut(self), &[], dt, drive_torque, &[BodyInputs { brake, steer: Some(steer), air: true }], probe);
    }

    /// The start of a slice: the body turns on curvature `kappa` (1/m, positive to the right).
    pub(super) fn steer_slice(&mut self, kappa: f32) {
        // The steering sets the curvature the bus turns on (`steer` 1 = the full lock of
        // `[inv_min_turnradius]`) and every axle points at the centre of that turn on the
        // `[rot_pnt_long]` line, as OMSI does it. How fast the curvature may change is the
        // input's business (keys, mouse, wheel), not the axle's: the old fixed rate here
        // lagged every mouse and controller movement by up to 0.4 s.
        self.kappa = kappa;
        let front = self.wheels.iter().map(|w| w.attach.y).fold(f32::MIN, f32::max);
        for w in self.wheels.iter_mut() {
            // each tyre square to the line from the centre of the turn to itself - the inner
            // one turned further than the outer (Ackermann). With both at the axle's angle the
            // tyres of an axle pulled against each other through the side constraint: the
            // scrub ate the drive, 60 % steering left a twentieth of the push and full lock
            // none ("the engine revs but the bus gets slower the more I steer")
            let across = (1.0 - w.attach.x * kappa).max(0.2);
            w.steer = ((w.attach.y - self.rot_pnt_long) * kappa / across).atan().clamp(-MAX_STEER, MAX_STEER);
            w.steered = w.steer != 0.0;
        }
        self.steer_deg = ((front - self.rot_pnt_long) * kappa).atan().clamp(-MAX_STEER, MAX_STEER).to_degrees();
        self.wheel_impacts.clear();
    }

    /// The body moved on by the substep under what acts on it.
    pub(super) fn integrate(&mut self, s: &Substep, h: f32, accel_sum: &mut Vec3) {
        let (rot, body_fwd, towed, force, torque) = (s.rot, s.body_fwd, s.towed, s.force, s.torque);
        let along = force.dot(body_fwd);
        let acc = (force - body_fwd * along) / self.mass + body_fwd * (along / (self.mass + towed));
        *accel_sum += rot.inverse().mul_vec3(acc - Vec3::new(0.0, 0.0, -GRAVITY));
        self.velocity += acc * h;
        self.position += (self.velocity * h).as_dvec3();
        let iw = self.inertia * self.omega;
        let omega_dot = (torque - self.omega.cross(iw)) / self.inertia;
        self.omega += omega_dot * h;
        let dq = Quat::from_scaled_axis(rot.mul_vec3(self.omega) * h);
        self.orientation = (dq * rot).normalize();
    }

    /// The end of a slice of `substeps`: the acceleration the body felt, and the safety net
    /// against tipping over.
    pub(super) fn end_slice(&mut self, substeps: usize, accel_sum: Vec3) {
        self.accel_body = accel_sum / substeps as f32;
        // rotation keeps the body from tipping over completely (safety net for bad ground)
        let up = self.orientation.mul_vec3(Vec3::Z);
        if up.z < TIP_OVER {
            let (heading, _, _) = self.heading_pitch_bank();
            self.orientation = Quat::from_rotation_z((-heading).to_radians() as f32);
            self.omega = Vec3::ZERO;
        }
    }
}

#[cfg(test)]
mod tests {
    /// The body's pitch and roll are damped as much a second at any frame rate: as OMSI
    /// damps them at 30 frames a second (a 12 t bus on 240-280 kN/m springs).
    #[test]
    fn body_damping_does_not_depend_on_the_frame_rate() {
        let f = ((2.0 * 240.0 + 2.0 * 280.0) / 12.0f32).sqrt();
        let second = |fps: f32| (0..fps as usize).fold(1.0f32, |k, _| k * super::body_damping(f, 1.0 / fps));
        let (a, b, c) = (second(30.0), second(60.0), second(144.0));
        assert!((a - b).abs() < 1e-3 && (a - c).abs() < 1e-3, "{a} {b} {c}");
        assert!(a > 0.2 && a < 0.5, "{a}");
    }
}

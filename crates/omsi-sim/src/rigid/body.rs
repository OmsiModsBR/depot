//! The body: its mass, inertia and pose built from the `.bus` file ([`RigidBody`]), the parts
//! it tows ([`CoupledPart`]), and its motion over a substep (semi-implicit Euler) with OMSI's
//! damping of pitch and roll ([`body_damping`]).

use super::collide::Impact;
use super::ground::GroundProbe;
use super::params::{
    BODY_DAMPING, BUMP, DEFAULT_COG_HEIGHT, DEFAULT_DAMPER, DEFAULT_INERTIA_INV, DEFAULT_MAX_FORCE,
    DEFAULT_SPRING, DRY_GRIP, GRAVITY, LOCK_DEG_MAX, LOCK_DEG_MIN, MAX_STEER, OMSI_FRAME,
    ROLLING_COEFFICIENT, TIP_OVER, TRACK_SHARE,
};
use super::suspension::wheel_rest_loads;
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
    pub mass: f32,
    /// Body-frame inertia (diagonal, kg m²).
    pub inertia: Vec3,
    /// Centre of gravity relative to the model origin (m).
    pub cog: Vec3,
    /// World position of the centre of gravity.
    pub position: DVec3,
    pub orientation: Quat,
    pub velocity: Vec3,
    /// Angular velocity in the body frame (rad/s).
    pub omega: Vec3,
    pub wheels: Vec<RigidWheel>,
    /// Axle index of each wheel (two per axle, left then right).
    pub wheel_axle: Vec<usize>,
    pub steer_deg: f32,
    /// The curvature the steering asks for this step (1/m, positive to the right).
    pub kappa: f32,
    pub max_steer_deg: f32,
    /// `[rot_pnt_long]` and `[inv_min_turnradius]`: the line the bus turns about and the
    /// curvature of the full lock.
    pub rot_pnt_long: f32,
    pub inv_min_turn_radius: f32,
    /// sqrt(sum of the springs / mass) (1/s): OMSI damps the body's pitch and roll by
    /// sin(x)/x a frame with x = 1.5 of it times the frame.
    pub body_freq: f32,
    /// OMSI's two states of the tyres across: holding - the bus follows
    /// its wheels however quickly they are turned, as long as the bend asks no more than
    /// the grip of all its tyres and no wheel spins or locks - or sliding, each tyre giving
    /// at most its own grip until the sideways speed at every tyre is under 0.1 m/s again.
    pub holding: bool,
    pub rolling_resistance: f32,
    /// Body-frame acceleration of the last step (m/s²), for `A_Trans_*`.
    pub accel_body: Vec3,
    pub friction: f32,
    /// Wheels a face stopped during the last step, one entry per wheel.
    pub wheel_impacts: Vec<Impact>,
    /// The parts coupled behind (see [`CoupledPart`]); `drive_torque` is shared by their
    /// driven wheels and the body's own.
    pub coupled: Vec<CoupledPart>,
    /// Obstacles (by id) the body was put down inside: left alone until it is out of them
    /// (a bus spawned in a shelter's or a depot hall's box was held there for good - every
    /// move towards the nearest side pushed it back in). `None` until the first collision
    /// check after `place`.
    pub spawned_inside: Option<Vec<i64>>,
    /// The faces a wheel cannot climb stop it (a platform's edge, a kerb too high for it).
    /// Off with the collisions with objects: the wheels then keep to the ground under them,
    /// as OMSI's do - its wheels know no faces - and a step nobody sees (a height profile
    /// over the drawn road, a helper object's collision box) is no invisible wall.
    pub wheel_walls: bool,
}

impl RigidBody {
    /// `hub_heights[a]`: unloaded hub height of axle `a` above the model origin; the tyre
    /// radius when the model does not say (see `VehicleType::wheel_geometry`).
    pub fn from_definition(def: &Vehicle, hub_heights: &[Option<f32>]) -> RigidBody {
        let mass = if def.mass < 100.0 { def.mass * 1000.0 } else { def.mass }.max(500.0);
        let moi = def.moment_of_inertia;
        let scale = if moi[0] < 5000.0 { 1000.0 } else { 1.0 };
        // `[momentofintertia]` goes to ODE as I11, I22, I33 of OMSI's y-up body frame:
        // about the lateral axis (pitch), the vertical one (yaw) and the
        // longitudinal one (roll) - roll takes the third value, yaw the second,
        // whatever the SDK's comment says. Here (right, forward, up): pitch,
        // roll, yaw. Read in the comment's order the SD202 rolled on 80 t m² instead of
        // 300, twice as fast, and every uneven patch rocked it like a boat.
        let inertia = Vec3::new((moi[0] * scale).max(100.0), (moi[2] * scale).max(100.0), (moi[1] * scale).max(100.0));
        let cog_xy = def.cog.map(|c| (c[0], c[1])).unwrap_or((0.0, 0.0));
        let cog_z = if def.cog_height > 0.0 { def.cog_height } else { def.cog.map(|c| c[2]).filter(|z| *z > 0.0).unwrap_or(DEFAULT_COG_HEIGHT) };
        let cog = Vec3::new(cog_xy.0, cog_xy.1, cog_z);
        let front_long = def.axles.iter().map(|a| a.long).fold(f32::MIN, f32::max);
        let mut wheels = Vec::new();
        let mut wheel_axle = Vec::new();
        for (ai, a) in def.axles.iter().enumerate() {
            let r = (a.wheel_diameter / 2.0).max(0.15);
            let z = hub_heights.get(ai).copied().flatten().unwrap_or(r);
            let k = if a.spring > 0.0 { a.spring * 1000.0 } else { DEFAULT_SPRING * 1000.0 };
            let c = if a.damper > 0.0 { a.damper * 1000.0 } else { DEFAULT_DAMPER * 1000.0 };
            let max_force = if a.max_force > 0.0 { a.max_force * 1000.0 } else { DEFAULT_MAX_FORCE * 1000.0 };
            // where the tyres stand across (the middle of the band between the two widths)
            // and where the strut pushes the body (the outer width)
            let outer = (a.max_width / 2.0).max(0.3);
            let inner = if a.min_width > 0.0 && a.min_width < a.max_width { (a.max_width + a.min_width) / 4.0 } else { outer * TRACK_SHARE };
            for side in [-1.0f32, 1.0] {
                let inertia_inv = if a.inertia_inv > 0.0 { a.inertia_inv } else { DEFAULT_INERTIA_INV };
                wheels.push(RigidWheel { spin: 0.0, slipping: false, locked: false, inertia_inv, attach: Vec3::new(side * inner, a.long, z), lever: side * (outer - inner), radius: r, driven: a.driven, steered: (a.long - front_long).abs() < 0.01, steer: 0.0, spring: k, damper: c, max_force, spring_factor: 1.0, rest_load: 0.0, compression: 0.0, compression_rate: 0.0, touch: None, on_ground: true, rotation_deg: 0.0, rpm: 0.0, load: 0.0, ground_z: 0.0, ground_seen: false, walls: Vec::new(), step_force: 0.0 });
                wheel_axle.push(ai);
            }
        }
        let loads = wheel_rest_loads(def);
        for (w, a) in wheels.iter_mut().zip(&wheel_axle) {
            w.rest_load = loads.get(*a).copied().unwrap_or(0.0);
        }
        let s = (front_long - def.rot_pnt_long).abs().max(1.0);
        let max_steer_deg = (def.inv_min_turn_radius * s).atan().to_degrees().clamp(LOCK_DEG_MIN, LOCK_DEG_MAX);
        // a file without a usable `[inv_min_turnradius]` still steers its front axle as far
        // as the old default lock
        let inv_min_turn_radius = if def.inv_min_turn_radius > 0.0 { def.inv_min_turn_radius } else { max_steer_deg.to_radians().tan() / s };
        let springs: f32 = def.axles.iter().map(|a| 2.0 * if a.spring > 0.0 { a.spring } else { DEFAULT_SPRING }).sum();
        let body_freq = (springs / (mass / 1000.0)).max(0.0).sqrt();
        RigidBody { mass, inertia, cog, position: DVec3::ZERO, orientation: Quat::IDENTITY, velocity: Vec3::ZERO, omega: Vec3::ZERO, wheels, wheel_axle, steer_deg: 0.0, kappa: 0.0, max_steer_deg, rot_pnt_long: def.rot_pnt_long, inv_min_turn_radius, body_freq, holding: true, rolling_resistance: if def.rolling_resistance > 0.0 { def.rolling_resistance } else { ROLLING_COEFFICIENT * mass * GRAVITY }, accel_body: Vec3::ZERO, friction: DRY_GRIP, wheel_impacts: Vec::new(), coupled: Vec::new(), spawned_inside: None, wheel_walls: true }
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

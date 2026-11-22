//! The body: its mass, inertia and pose built from the `.bus` file ([`RigidBody`]), the parts
//! it tows ([`CoupledPart`]), and its motion over a substep (semi-implicit Euler) with OMSI's
//! damping of pitch and roll ([`body_damping`]).

use super::depot;
use super::surface::surface_grip;
use super::train::step_train;
use super::tyre::Substep;
use crate::collision::Obb;
use crate::physics::domain::params::{
    BODY_DAMPING, BUMP, DRY_GRIP, GRAVITY, OMSI_FRAME, SUBSTEP_DT, TIP_OVER,
};
use crate::physics::domain::{
    BodyInputs, Engine, EngineState, Ground, GroundProbe, Payload, PhysicsSpec, RigidBody,
    RigidWheel, RoadWeather, SteerState,
};
use glam::{DVec3, Quat, Vec3};
use omsi_vehicle::Vehicle;

/// What is left of the body's pitch and roll rate after `dt` seconds of OMSI's damping.
pub fn body_damping(body_freq: f32, dt: f32) -> f32 {
    let x0 = BODY_DAMPING * body_freq * OMSI_FRAME;
    if x0 <= 1e-6 || dt <= 0.0 {
        return 1.0;
    }
    (x0.sin() / x0).clamp(0.1, 1.0).powf(dt / OMSI_FRAME)
}

impl RigidBody {
    /// `hub_heights[a]`: unloaded hub height of axle `a` above the model origin; the tyre
    /// radius when not given. The player's vehicle gives none: OMSI's springs stand on the
    /// model origin's plane, whatever the model's wheels look like.
    pub fn from_definition(def: &Vehicle, hub_heights: &[Option<f32>]) -> RigidBody {
        RigidBody::for_engine(def, hub_heights, Engine::Classic)
    }

    /// [`RigidBody::from_definition`] under the rules of `engine`, which the body keeps.
    pub fn for_engine(def: &Vehicle, hub_heights: &[Option<f32>], engine: Engine) -> RigidBody {
        let spec = PhysicsSpec::for_engine(def, engine);
        let mass = spec.mass_kg;
        let mut wheels = Vec::new();
        let mut wheel_axle = Vec::new();
        for (ai, a) in spec.axles.iter().enumerate() {
            let r = a.radius;
            let z = hub_heights.get(ai).copied().flatten().unwrap_or(r);
            for side in [-1.0f32, 1.0] {
                wheels.push(RigidWheel {
                    spin: 0.0,
                    slipping: false,
                    locked: false,
                    inertia_inv: a.inertia_inv,
                    attach: Vec3::new(side * a.track, a.long, z),
                    lever: side * (a.outer - a.track),
                    radius: r,
                    driven: a.driven,
                    steered: (a.long - spec.front_long).abs() < 0.01,
                    steer: 0.0,
                    hold_reach: f32::INFINITY,
                    spring: a.spring,
                    damper: a.damper,
                    max_force: a.max_force,
                    spring_factor: 1.0,
                    rest_load: 0.0,
                    compression: 0.0,
                    compression_rate: 0.0,
                    touch: None,
                    on_ground: true,
                    rotation_deg: 0.0,
                    rpm: 0.0,
                    load: 0.0,
                    ground_z: 0.0,
                    ground_seen: false,
                    surface: 0,
                    mu: DRY_GRIP,
                    walls: Vec::new(),
                    step_force: 0.0,
                });
                wheel_axle.push(ai);
            }
        }
        for (w, a) in wheels.iter_mut().zip(&wheel_axle) {
            w.rest_load = spec.rest_loads.get(*a).copied().unwrap_or(0.0);
        }
        RigidBody {
            rules: EngineState::new(engine, spec.rot_pnt_long),
            mass,
            empty_mass: mass,
            payload: 0.0,
            inertia: spec.inertia,
            empty_inertia: spec.inertia,
            cog: spec.cog,
            empty_cog: spec.cog,
            position: DVec3::ZERO,
            orientation: Quat::IDENTITY,
            velocity: Vec3::ZERO,
            omega: Vec3::ZERO,
            wheels,
            wheel_axle,
            steer_deg: 0.0,
            steer_axles: spec
                .steer_axles
                .iter()
                .map(|s| (*s, SteerState::default()))
                .collect(),
            leading: false,
            file_turn_line: spec.turn_line,
            kappa: 0.0,
            max_steer_deg: spec.max_steer_deg,
            rot_pnt_long: spec.turn_line,
            inv_min_turn_radius: spec.inv_min_turn_radius,
            body_freq: spec.body_freq,
            empty_body_freq: spec.body_freq,
            holding: true,
            joint_across: 0.0,
            rolling_resistance: spec.rolling_resistance,
            accel_body: Vec3::ZERO,
            friction: DRY_GRIP,
            road: None,
            wheel_impacts: Vec::new(),
            coupled: Vec::new(),
            spawned_inside: None,
            wheel_walls: true,
            unstepped: 0.0,
            last_position: DVec3::ZERO,
            last_orientation: Quat::IDENTITY,
        }
    }

    /// The body leads its vehicle: its front axle is the driver's, so a steerable axle
    /// declared there is left out (spec 004; reported when the vehicle appears,
    /// `resolve_physics_mode`).
    pub fn lead(&mut self) {
        self.leading = true;
        let wheels = &self.wheels;
        let axles = &self.wheel_axle;
        self.steer_axles.retain(|(s, _)| {
            !axles
                .iter()
                .zip(wheels)
                .any(|(a, w)| *a == s.axle && w.steered)
        });
        let rest = self.wheel_axle.iter().step_by(2).count().saturating_sub(1);
        if !self.steer_axles.is_empty() && self.steer_axles.len() == rest {
            log::info!(
                "every axle but the front one steers: while they do, the bus turns about its [rot_pnt_long] line"
            );
        }
    }

    /// Place the body at rest with its wheels on the ground plane at `origin.z`: heading
    /// `heading_deg`, springs already carrying the load (no drop and bounce at the start).
    pub fn place(&mut self, origin: DVec3, heading_deg: f64) {
        self.orientation = Quat::from_rotation_z((-heading_deg).to_radians() as f32);
        let origin = origin + DVec3::Z * self.rest_sag() as f64;
        self.position = origin + self.orientation.mul_vec3(self.cog).as_dvec3();
        self.velocity = Vec3::ZERO;
        self.omega = Vec3::ZERO;
        self.rules.reset();
        self.spawned_inside = None;
        self.unstepped = 0.0;
        (self.last_position, self.last_orientation) = (self.position, self.orientation);
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

    /// Each tyre's grip from the ground it last stood on and the road's weather `road` (`C-5`,
    /// [`surface_grip`]); the body's own value, for the parts coupled behind it, is the mean of
    /// its tyres'.
    pub fn set_road(&mut self, road: &RoadWeather) {
        for w in self.wheels.iter_mut() {
            w.mu = surface_grip(w.surface, road);
        }
        if !self.wheels.is_empty() {
            self.friction =
                self.wheels.iter().map(|w| w.mu).sum::<f32>() / self.wheels.len() as f32;
        }
        self.road = Some(*road);
    }

    /// The ground byte under each wheel that found the ground this step: the road face at
    /// the height it found under the wheel's point on the model's origin plane (where the
    /// spring measures, see `suspension`). A wheel on the terrain, or with nothing under it,
    /// keeps the byte it had, as OMSI's wheels do.
    pub fn read_surfaces(&mut self, ground: &dyn Ground) {
        let (rot, position, cog) = (self.orientation, self.position, self.cog);
        for w in self.wheels.iter_mut().filter(|w| w.touch.is_some()) {
            let at = position
                + rot
                    .mul_vec3(Vec3::new(w.attach.x, w.attach.y, 0.0) - cog)
                    .as_dvec3();
            if let Some(kind) = ground.surface(at.x, at.y, w.ground_z) {
                w.surface = kind;
            }
        }
    }

    /// What the body carries (the people aboard): its mass weighs on the body as the body's own
    /// mass does - in the gravity on the springs and the tyres' grip, and in what it takes to
    /// speed it up, brake it and turn it. The body's pitch and roll damping (`body_freq`) stays
    /// the empty body's under Classic (`K-5`); under Depot it follows the heavier body on the
    /// same springs. Where the payload sits moves the centre of
    /// gravity and adds to the inertia by the engine's rule (`loaded_body`: Classic keeps
    /// `[cog]` and `[momentofintertia]`, Depot sums the people into the body); the model
    /// origin stays where it is and the body keeps its motion.
    pub fn set_payload(&mut self, load: Payload) {
        let kg = if load.kg.is_finite() {
            load.kg.max(0.0)
        } else {
            0.0
        };
        let load = Payload { kg, ..load };
        self.payload = kg;
        self.mass = self.empty_mass + kg;
        let engine = self.engine();
        self.body_freq = engine.loaded_body_freq(self.empty_body_freq, self.empty_mass, self.mass);
        let (cog, inertia) =
            engine.loaded_body((self.empty_cog, self.empty_mass, self.empty_inertia), load);
        self.inertia = inertia;
        self.move_cog(cog);
    }

    /// Puts the centre of gravity at `cog` (body frame) with the model origin and the motion
    /// of every point of the body unchanged: the new centre is where the body's point at
    /// `cog` is, moving as that point moves.
    fn move_cog(&mut self, cog: Vec3) {
        let shift = cog - self.cog;
        if shift == Vec3::ZERO {
            return;
        }
        let along = self.orientation.mul_vec3(shift);
        self.position += along.as_dvec3();
        self.velocity += self.orientation.mul_vec3(self.omega).cross(along);
        self.last_position += self.last_orientation.mul_vec3(shift).as_dvec3();
        self.cog = cog;
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
        if self.steers_axles() {
            return depot::steering::axle_angle(self, i);
        }
        let Some(w) = self.wheels.get(i) else {
            return 0.0;
        };
        ((w.attach.y - self.rot_pnt_long) * self.kappa).atan()
    }

    /// The body steers its own axles (`Engine::steers_axles`).
    pub(super) fn steers_axles(&self) -> bool {
        self.engine().steers_axles(self)
    }

    /// The curvature (1/m) the driver's steering `s` (−1..1) asks for: `s` times the full lock
    /// (`C-6`, `D-5`).
    pub(super) fn steering_kappa(&self, s: f32) -> f32 {
        let s = s.clamp(-1.0, 1.0);
        if self.steers_axles() {
            s * depot::steering::body_full_lock(self)
        } else {
            s * self.inv_min_turn_radius
        }
    }

    /// Forward speed (m/s) in the body frame.
    pub fn forward_speed(&self) -> f32 {
        self.velocity.dot(self.orientation.mul_vec3(Vec3::Y))
    }

    /// The grade of the road under the body, in percent, positive where the front stands
    /// higher: the rise of the ground from the rearmost to the frontmost axle with a tyre on
    /// it, over the run between them. It is the road's and not the body's pitch, so braking
    /// or a load does not change it. None while fewer than two axles stand on the ground.
    pub fn road_grade(&self) -> Option<f32> {
        // (each axle: where it stands along the body, and the mean height of the ground
        // under its tyres that touch it)
        let axles: Vec<(f32, f64)> = self
            .wheels
            .chunks(2)
            .filter_map(|pair| {
                let down: Vec<f64> = pair
                    .iter()
                    .filter(|w| w.on_ground && w.ground_seen)
                    .map(|w| w.ground_z)
                    .collect();
                (!down.is_empty()).then(|| {
                    (
                        pair[0].attach.y,
                        down.iter().sum::<f64>() / down.len() as f64,
                    )
                })
            })
            .collect();
        let rear = axles.iter().min_by(|a, b| a.0.total_cmp(&b.0))?;
        let front = axles.iter().max_by(|a, b| a.0.total_cmp(&b.0))?;
        let length = front.0 - rear.0;
        let rise = (front.1 - rear.1) as f32;
        // (one axle on the ground, or two at the same place, has no run to measure a slope on)
        if length < 0.5 || rise.abs() >= length {
            return None;
        }
        Some(rise / (length * length - rise * rise).sqrt() * 100.0)
    }

    /// Whether the body skids: its tyres slide across the road (they let go of it, see
    /// [`holding`](Self::holding)), or a tyre on the ground spins or is locked by its brake.
    pub fn skidding(&self) -> bool {
        !self.holding || self.wheels.iter().any(|w| w.on_ground && w.slipping)
    }

    pub fn kinetic_energy(&self) -> f32 {
        0.5 * self.mass * self.velocity.length_squared()
            + 0.5 * (self.inertia * self.omega * self.omega).element_sum()
    }

    /// Inverse of the effective mass the body shows along world direction `n` at world
    /// offset `r` from its centre of gravity.
    pub(crate) fn inv_mass_at(&self, r: Vec3, n: Vec3) -> f32 {
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
        let centre = origin
            + self
                .orientation
                .mul_vec3(Vec3::new(bb[3], bb[4], bb[5]))
                .as_dvec3();
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for sx in [-0.5f32, 0.5] {
            for sy in [-0.5f32, 0.5] {
                for sz in [-0.5f32, 0.5] {
                    let z = centre.z
                        + self
                            .orientation
                            .mul_vec3(Vec3::new(bb[0] * sx, bb[1] * sy, bb[2] * sz))
                            .z as f64;
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
            axes: [
                r.mul_vec3(Vec3::X).as_dvec3(),
                r.mul_vec3(Vec3::Y).as_dvec3(),
                r.mul_vec3(Vec3::Z).as_dvec3(),
            ],
            half: (Vec3::new(bb[0], bb[1], bb[2]) * 0.5 + Vec3::splat(margin)).as_dvec3(),
        }
    }

    /// One step. `drive_torque` (N m at the driven wheels, from `M_Wheel`), `brake` per
    /// wheel (N), `steer` −1..1, `probe(x, y, z_top)` the ground at a point: a train of one
    /// body (see [`step_train`]).
    pub fn step(
        &mut self,
        dt: f32,
        drive_torque: f32,
        brake: &[f32],
        steer: f32,
        probe: &dyn Fn(f64, f64, f64) -> GroundProbe,
    ) {
        step_train(
            std::slice::from_mut(self),
            &[],
            dt,
            drive_torque,
            &[BodyInputs {
                brake,
                steer: Some(steer),
                air: true,
            }],
            probe,
        );
    }

    /// The start of a substep: the body turns on curvature `kappa` (1/m, positive to the right),
    /// set by the steering or, for a section behind a joint (`follower`), by the joint's bend;
    /// `lead_full_lock` is the leading section's full-lock curvature, which the steerable axles
    /// of a section behind a joint measure that bend against (`D-6`).
    pub(super) fn steer_to(&mut self, kappa: f32, follower: bool, lead_full_lock: f32) {
        // The steering sets the curvature the bus turns on (`steer` 1 = the full lock of
        // `[inv_min_turnradius]`) and every axle points at the centre of that turn on the
        // `[rot_pnt_long]` line, as OMSI does it. How fast the curvature may change is the
        // input's business (keys, mouse, wheel), not the axle's: the old fixed rate here
        // lagged every mouse and controller movement by up to 0.4 s.
        // (a section with steerable axles: `D-5` to `D-7`, its axles in the basic mode following
        // the steering, or behind a joint the joint's bend over the leading section's full lock)
        if self.steers_axles() {
            let full_lock = if follower {
                lead_full_lock
            } else {
                depot::steering::body_full_lock(self)
            };
            return depot::steering::steer_to(self, kappa, full_lock);
        }
        self.kappa = kappa;
        let front = self
            .wheels
            .iter()
            .map(|w| w.attach.y)
            .fold(f32::MIN, f32::max);
        for w in self.wheels.iter_mut() {
            // a section behind a joint: its axles show the bend (`axle_steer`), its tyres stay
            // square to the body (`C-9`)
            if follower {
                w.steer = 0.0;
                w.steered = false;
                continue;
            }
            // each tyre square to the line from the centre of the turn to itself - the inner
            // one turned further than the outer (Ackermann). With both at the axle's angle the
            // tyres of an axle pulled against each other through the side constraint: the
            // scrub ate the drive, 60 % steering left a twentieth of the push and full lock
            // none ("the engine revs but the bus gets slower the more I steer")
            let across = (1.0 - w.attach.x * kappa).max(0.2);
            w.steer = ((w.attach.y - self.rot_pnt_long) * kappa / across).atan();
            w.steered = w.steer != 0.0;
        }
        self.steer_deg = ((front - self.rot_pnt_long) * kappa).atan().to_degrees();
    }

    /// The body moved on by the substep under what acts on it.
    pub(super) fn integrate(&mut self, s: &Substep, h: f32, accel_sum: &mut Vec3) {
        (self.last_position, self.last_orientation) = (self.position, self.orientation);
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

    /// The end of a substep: OMSI's damping of pitch and roll over it, and the safety net
    /// against tipping over.
    pub(super) fn end_substep(&mut self, h: f32) {
        // OMSI damps the body's pitch and roll - never its yaw - by sin(x)/x each frame,
        // x = 1.5 x sqrt(springs / mass) x frame. Its frames are OMSI's (its options.cfg caps
        // them at 30 a second): taken per frame of this game, a machine drawing 150 frames a
        // second damped the body a fifth as much, and the bus rocked on its springs like a
        // boat. The same damping a second as OMSI's at 30 frames, however many are drawn:
        let k = body_damping(self.body_freq, h);
        self.omega.x *= k;
        self.omega.y *= k;
        // rotation keeps the body from tipping over completely (safety net for bad ground)
        let up = self.orientation.mul_vec3(Vec3::Z);
        if up.z < TIP_OVER {
            let (heading, _, _) = self.heading_pitch_bank();
            self.orientation = Quat::from_rotation_z((-heading).to_radians() as f32);
            self.omega = Vec3::ZERO;
        }
    }

    /// The model origin as the body is shown: between where it stood a substep before the
    /// last and where it stands, as far as the time not yet stepped reaches into the next
    /// substep (a substep behind the physics at most).
    pub fn shown_origin(&self) -> DVec3 {
        let (position, orientation) = self.shown_pose();
        position - orientation.mul_vec3(self.cog).as_dvec3()
    }

    /// Heading (deg, clockwise from north), pitch and bank (deg) of the body as it is shown
    /// (see [`Self::shown_origin`]).
    pub fn shown_heading_pitch_bank(&self) -> (f64, f32, f32) {
        let (_, orientation) = self.shown_pose();
        let f = orientation.mul_vec3(Vec3::Y);
        let r = orientation.mul_vec3(Vec3::X);
        let heading = (f.x.atan2(f.y) as f64).to_degrees().rem_euclid(360.0);
        let pitch = f.z.clamp(-1.0, 1.0).asin().to_degrees();
        let bank = (-r.z).clamp(-1.0, 1.0).asin().to_degrees();
        (heading, pitch, bank)
    }

    fn shown_pose(&self) -> (DVec3, Quat) {
        let t = (self.unstepped / SUBSTEP_DT).clamp(0.0, 1.0);
        (
            self.last_position.lerp(self.position, t as f64),
            self.last_orientation.slerp(self.orientation, t),
        )
    }
}

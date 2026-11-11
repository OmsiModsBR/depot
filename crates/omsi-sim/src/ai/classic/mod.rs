//! The OMSI Classic AI body (`KA-1` to `KA-11` of `docs/physics/classic-model.md`): on its route
//! a vehicle is no rigid body. Its rotation point sits on the lane, its heading is a state of its
//! own integrated by a kinematic bicycle model, and its height and attitude come from the path:
//! it has no springs, no settling and no dynamic pitch or roll.
//!
//! Every rule here is **to validate** against OMSI 2 (see the status of each `KA-*` row).

use super::MotionKind;
use crate::physics::AiSpec;
use crate::vehicle::VehicleInstance;
use glam::{DVec2, DVec3, Vec3};
use omsi_vehicle::Vehicle;

/// How far ahead along the way the curvature is measured (m).
const CURVATURE_SPAN: f32 = 2.0;

#[derive(Debug, Clone)]
pub struct AiClassic {
    pub kind: MotionKind,
    /// The line the body turns about, `[rot_pnt_long]`, and `L`, the length of the kinematic
    /// bicycle from it to the front of the vehicle (m).
    rot_long: f32,
    length: f32,
    front_long: f32,
    rear_long: f32,
    /// Each axle's place along the body (m), the largest front wheel angle (deg).
    axle_long: Vec<f32>,
    max_steer: f32,
    started: bool,
    /// World position of the rotation point.
    rear: DVec2,
    /// The path's heading at the rotation point and the body's heading relative to it, `ψ_r`
    /// (deg, clockwise from north).
    path_heading: f64,
    relative: f32,
    /// Heading (deg, clockwise from north).
    pub heading: f64,
    /// The front wheel angle (deg, + = right).
    pub steer: f32,
    last_speed: f32,
    /// Longitudinal and lateral acceleration (m/s², forward / right positive).
    pub a_long: f32,
    pub a_lat: f32,
    /// Pose for the vehicle: origin, pitch and bank (deg).
    pub position: DVec3,
    pub pitch_deg: f32,
    pub bank_deg: f32,
    /// Per axle (left, right): the spring travel about the static sag, always 0 on the route.
    pub suspension: Vec<[f32; 2]>,
}

fn dir(h: f64) -> DVec2 {
    let r = h.to_radians();
    DVec2::new(r.sin(), r.cos())
}

fn heading_of(d: DVec2) -> Option<f64> {
    (d.length() > 1e-6).then(|| d.x.atan2(d.y).to_degrees())
}

fn wrap(d: f64) -> f64 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

impl AiClassic {
    pub fn new(def: &Vehicle, kind: MotionKind) -> AiClassic {
        let spec = AiSpec::from(def);
        let axle_long: Vec<f32> = def.axles.iter().map(|a| a.long).collect();
        AiClassic {
            kind,
            rot_long: spec.rot_long,
            length: (spec.front_long - spec.rot_long).abs().max(0.8),
            front_long: spec.front_long,
            rear_long: spec.rear_long,
            suspension: vec![[0.0; 2]; spec.axle_count],
            axle_long,
            max_steer: spec.max_steer,
            started: false,
            rear: DVec2::ZERO,
            path_heading: 0.0,
            relative: 0.0,
            heading: 0.0,
            steer: 0.0,
            last_speed: 0.0,
            a_long: 0.0,
            a_lat: 0.0,
            position: DVec3::ZERO,
            pitch_deg: 0.0,
            bank_deg: 0.0,
        }
    }

    /// The path's curvature at its point `d` metres along it (1/m, + = to the right): the
    /// change of heading over [`CURVATURE_SPAN`].
    fn curvature(way: &dyn Fn(f32) -> DVec3, d: f32) -> f64 {
        let (a, b, c) = (
            way(d).truncate(),
            way(d + 0.5 * CURVATURE_SPAN).truncate(),
            way(d + CURVATURE_SPAN).truncate(),
        );
        match (heading_of(b - a), heading_of(c - b)) {
            (Some(h0), Some(h1)) => wrap(h1 - h0).to_radians() / (0.5 * CURVATURE_SPAN as f64),
            _ => 0.0,
        }
    }

    /// Put the body onto its way, standing still and straight.
    pub fn place(
        &mut self,
        way: &dyn Fn(f32) -> DVec3,
        ground: Option<&dyn Fn(f64, f64) -> Option<f64>>,
        contact: Option<&dyn crate::physics::Ground>,
        speed: f32,
    ) {
        self.started = false;
        self.last_speed = speed;
        self.step(0.0, speed, way, ground, contact);
    }

    /// Follow the way for `dt` seconds at `speed` (m/s, the planner's speed). The ground and
    /// the contacts are not asked: the route's own height is the vehicle's (`KA-4`).
    pub fn step(
        &mut self,
        dt: f32,
        speed: f32,
        way: &dyn Fn(f32) -> DVec3,
        _ground: Option<&dyn Fn(f64, f64) -> Option<f64>>,
        _contact: Option<&dyn crate::physics::Ground>,
    ) {
        if dt > 0.0 {
            let a = (speed - self.last_speed) / dt;
            self.a_long += (a - self.a_long) * (dt / 0.15).min(1.0);
        }
        self.last_speed = speed;
        if self.kind != MotionKind::Road {
            self.follow_without_bicycle(way);
            return;
        }
        // (KA-1) the rotation point is on the lane; the heading is the path's turned by the
        // relative angle, which follows `sin ψ_r = L κ`
        let on = way(self.rot_long);
        self.rear = on.truncate();
        let ahead = way(self.rot_long + 1.0).truncate();
        let path = heading_of(ahead - self.rear).unwrap_or(self.path_heading);
        let kappa = Self::curvature(way, self.rot_long);
        let wanted = (self.length as f64 * kappa)
            .clamp(-1.0, 1.0)
            .asin()
            .to_degrees() as f32;
        if !self.started {
            self.relative = wanted;
            self.started = true;
        } else if dt > 0.0 {
            // one body length of travel brings the angle in
            let rate = (speed.abs() * dt / self.length).min(1.0);
            self.relative += (wanted - self.relative) * rate;
        }
        self.path_heading = path;
        self.heading = (path + self.relative as f64).rem_euclid(360.0);
        // each axle's wheels point at the turning centre: `δ = atan((y - r) κ)`; the front
        // wheel angle is what the scripts read
        let front = self.axle_long.iter().copied().fold(f32::MIN, f32::max);
        self.steer = if self.axle_long.is_empty() {
            0.0
        } else {
            (((front - self.rot_long) as f64 * kappa).atan().to_degrees() as f32)
                .clamp(-self.max_steer.max(1.0), self.max_steer.max(1.0))
        };
        self.a_lat = (speed * speed) * kappa as f32;
        self.pose(way);
    }

    /// Rail and air vehicles follow the way's own heading at their origin.
    fn follow_without_bicycle(&mut self, way: &dyn Fn(f32) -> DVec3) {
        let origin = way(0.0);
        let ahead = way(1.0);
        self.rear = origin.truncate();
        self.heading = heading_of(ahead.truncate() - origin.truncate()).unwrap_or(self.heading);
        self.position = origin;
        self.pitch_deg = ((ahead.z - origin.z) as f32).atan2(1.0).to_degrees();
    }

    /// (KA-4) The pose from the path alone: the height of the origin, the pitch between the
    /// axles' heights and no roll.
    fn pose(&mut self, way: &dyn Fn(f32) -> DVec3) {
        let o = self.rear - dir(self.heading) * self.rot_long as f64;
        let (front, rear) = (way(self.front_long), way(self.rear_long));
        let span = (self.front_long - self.rear_long).abs().max(0.5);
        self.position = DVec3::new(o.x, o.y, way(0.0).z);
        self.pitch_deg = (((front.z - rear.z) as f32) / span).atan().to_degrees();
        self.bank_deg = 0.0;
    }

    /// Hand the pose to the vehicle: position and attitude, the accelerations its scripts
    /// read and each wheel's travel.
    pub fn apply(&self, v: &mut VehicleInstance) {
        let (lift, delta) = if self.kind == MotionKind::Road {
            v.ai_rest_offset()
        } else {
            (0.0, 0.0)
        };
        v.position = self.position + DVec3::new(0.0, 0.0, lift as f64);
        v.heading = self.heading;
        v.pitch = self.pitch_deg;
        v.bank = self.bank_deg;
        v.motion.accel = Vec3::new(self.a_lat, self.a_long, 0.0);
        for (axle, s) in v.motion.wheels.iter_mut().zip(&self.suspension) {
            axle[0].suspension = s[0] + delta;
            axle[1].suspension = s[1] + delta;
        }
    }
}

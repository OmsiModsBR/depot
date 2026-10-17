//! The kinematic model's step, and a vehicle standing still on its wheels.

use crate::physics::domain::{AxleSpec, KinematicModel, PhysicsSpec, VehicleMotion, WheelState};
use glam::Vec3;
use omsi_vehicle::Vehicle;

impl VehicleMotion {
    /// Standing still on the wheels of `def`.
    pub fn at_rest(def: &Vehicle) -> VehicleMotion {
        let wheels = def
            .axles
            .iter()
            .map(|a| {
                let a = AxleSpec::of(a);
                let mk = |lat: f32| WheelState { long: a.long, lat, radius: a.shown_radius, driven: a.driven, rotation_deg: 0.0, rpm: 0.0, suspension: 0.0 };
                [mk(-a.half_width), mk(a.half_width)]
            })
            .collect();
        VehicleMotion { speed: 0.0, steer_deg: 0.0, accel: Vec3::ZERO, a_trans: Vec3::ZERO, wheels }
    }
}

impl KinematicModel {
    pub fn from_definition(def: &Vehicle) -> KinematicModel {
        // (its lock: inv_min_turnradius = tan(alpha_max) / s → alpha_max, with s from the front
        // axle to `[rot_pnt_long]`, held to a range - see `PhysicsSpec::kinematic_max_steer_deg`)
        let spec = PhysicsSpec::from(def);
        KinematicModel { mass_kg: spec.kinematic_mass_kg, rolling_resistance: spec.rolling_resistance, wheelbase: spec.wheelbase, max_steer_deg: spec.kinematic_max_steer_deg }
    }

    /// Advance `m` one step, the driver steering at `steering`. `drive_torque` is `M_Wheel`,
    /// `brake_forces` the per-wheel brake forces in the same order as `m.wheels`. Returns the
    /// displacement along the vehicle's forward axis and the heading change in degrees.
    pub fn step(&self, m: &mut VehicleMotion, steering: f32, dt: f32, drive_torque: f32, brake_forces: &[f32]) -> (f32, f32) {
        let dt = dt.clamp(0.0, crate::rigid::KINEMATIC_DT_MAX);
        // steering
        let target = steering.clamp(-1.0, 1.0) * self.max_steer_deg;
        let rate = self.max_steer_deg * 2.5 * dt;
        m.steer_deg += (target - m.steer_deg).clamp(-rate, rate);

        // forces along the forward axis
        let driven_radius = m.wheels.iter().find(|w| w[0].driven).or(m.wheels.first()).map(|w| w[0].radius).unwrap_or(0.5);
        let f_drive = drive_torque / driven_radius;
        let f_brake: f32 = brake_forces.iter().sum::<f32>().max(0.0);
        let f_roll = self.rolling_resistance.max(0.0);
        let v = m.speed;
        let resist = f_brake + f_roll;
        let mut f = f_drive;
        // resistances oppose motion; at rest they can only cancel the drive force
        if v.abs() > 0.05 {
            f -= resist * v.signum();
        } else if f_drive.abs() <= resist {
            f = 0.0;
            m.speed = 0.0;
        } else {
            f -= resist * f_drive.signum();
        }
        let a = f / self.mass_kg;
        let new_speed = v + a * dt;
        // resistances must not reverse the direction of travel
        if v.abs() > 0.05 && new_speed.signum() != v.signum() && f_drive.abs() <= resist {
            m.speed = 0.0;
        } else {
            m.speed = new_speed;
        }
        m.accel = Vec3::new(0.0, a, 0.0);
        let ds = m.speed * dt;

        // kinematic turning about the rotation point
        let curvature = m.steer_deg.to_radians().tan() / self.wheelbase;
        let dheading = (ds * curvature).to_degrees();

        // wheels
        for axle in m.wheels.iter_mut() {
            for w in axle.iter_mut() {
                let omega = m.speed / w.radius; // rad/s
                w.rpm = omega * 60.0 / std::f32::consts::TAU;
                w.rotation_deg = (w.rotation_deg + omega.to_degrees() * dt).rem_euclid(360.0);
            }
        }
        (ds, dheading)
    }
}

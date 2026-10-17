//! A wheel of the body ([`RigidWheel`]) and its own turning - gripping, slipping or locked,
//! as OMSI keeps it.

use super::tyre::Contact;
use crate::physics::domain::params::{DROOP, SLIP_MARGIN, STANDING};
use crate::physics::domain::RigidWheel;
use glam::{DVec3, Quat, Vec3};

pub(super) fn sign(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

impl RigidWheel {
    /// One substep of the wheel's turning:
    /// a gripping wheel turns with the ground and starts to slip when what it has to pass
    /// on (`f_long`, before the friction circle cut it) is more than the road takes; a
    /// slipping one turns by its drive, its brake and the sliding friction, locks when the
    /// brake stops it, and grips again when its speed crosses the ground's.
    pub(super) fn advance_spin(&mut self, c: &Contact, f_long_wanted: f32, friction: f32, h: f32) {
        let r = self.radius.max(0.05);
        let ground = c.v_long / r;
        let mu_n = friction * c.grip;
        if !self.slipping {
            let wanted = if c.v_long.abs() > STANDING { c.drive - c.brake * c.v_long.signum() } else { f_long_wanted };
            if wanted.abs() > mu_n * SLIP_MARGIN && mu_n > 0.0 {
                self.slipping = true;
                self.locked = false;
                self.spin = ground;
            } else {
                self.spin = ground;
            }
        }
        if self.slipping {
            let slide_before = self.spin * r - c.v_long;
            if self.locked {
                // it turns again when the road's pull and the drive beat the brake
                if (sign(-c.v_long) * mu_n - c.drive).abs() > c.brake {
                    self.locked = false;
                } else {
                    self.spin = 0.0;
                }
            }
            if !self.locked {
                let before = self.spin;
                self.spin += h * self.inertia_inv * (c.drive - sign(self.spin) * c.brake - sign(slide_before) * mu_n);
                if before != 0.0 && sign(before) != sign(self.spin) && c.brake > 0.0 {
                    self.spin = 0.0;
                    self.locked = true;
                }
            }
            let slide_after = self.spin * r - c.v_long;
            // gripping again: the wheel's speed crossed the ground's (or both stand)
            // (a slide that only now begins - zero before - has crossed nothing)
            if sign(slide_after) * sign(slide_before) < 0.0 || (c.v_long.abs() < STANDING && self.spin.abs() * r < STANDING && c.drive.abs() <= c.brake.max(mu_n)) {
                self.slipping = false;
                self.locked = false;
                self.spin = ground;
            }
        }
        self.rpm = self.spin * 60.0 / std::f32::consts::TAU;
        self.rotation_deg = (self.rotation_deg + self.spin.to_degrees() * h).rem_euclid(360.0);
    }

    /// Compression the static load gives with the current spring factor.
    pub fn rest_compression(&self) -> f32 {
        self.rest_load / (self.spring * self.spring_factor.max(0.05))
    }
}

/// Where a wheel's tyre is in the world this substep: its plane (forward and right, turned by
/// the wheel's steering angle, and forward on the level), and its hub with the spring
/// unloaded (`hub0`) and where the strut holds it now (`hub`, at most the droop below).
pub(super) struct TyreFrame {
    pub(super) fwd: Vec3,
    pub(super) right: Vec3,
    pub(super) fwd_h: Vec3,
    pub(super) hub0: DVec3,
    pub(super) hub: DVec3,
}

impl TyreFrame {
    /// Wheel `w`'s tyre on a body turned by `rot` whose centre of gravity (`cog`, body frame)
    /// stands at `position`.
    pub(super) fn of(w: &RigidWheel, rot: Quat, position: DVec3, cog: Vec3) -> TyreFrame {
        let up = rot.mul_vec3(Vec3::Z);
        let ang = w.steer;
        let fwd = rot.mul_vec3(Vec3::new(ang.sin(), ang.cos(), 0.0));
        let right = rot.mul_vec3(Vec3::new(ang.cos(), -ang.sin(), 0.0));
        let hub0 = position + rot.mul_vec3(w.attach - cog).as_dvec3();
        let hub = hub0 + (up * w.compression.max(-DROOP)).as_dvec3();
        TyreFrame { fwd, right, fwd_h: Vec3::new(fwd.x, fwd.y, 0.0).normalize_or(Vec3::Y), hub0, hub }
    }
}

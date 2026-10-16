//! A wheel of the body ([`RigidWheel`]) and its own turning - gripping, slipping or locked,
//! as OMSI keeps it.

use super::ground::WallFace;
use super::params::{DROOP, SLIP_MARGIN, STANDING};
use super::tyre::Contact;
use glam::{DVec3, Quat, Vec3};

#[derive(Debug, Clone)]
pub struct RigidWheel {
    /// The wheel's own turning (rad/s) and its state as OMSI keeps it:
    /// gripping (turning with the ground), slipping (spinning or skidding: the tyre passes on
    /// the sliding friction and the wheel turns by its own torques), locked (a brake that
    /// stopped it). `inertia_inv`: the axle's `achse_inertia_inv` (0.002 when not given).
    pub spin: f32,
    pub slipping: bool,
    pub locked: bool,
    pub inertia_inv: f32,
    /// Hub in the body frame (x right, y forward, z up, relative to the model origin) with
    /// the spring unloaded: the model's wheel centre when `Axle_Suspension` reads zero.
    pub attach: Vec3,
    /// How much farther out than the hub the strut's force acts (m, signed like `attach.x`).
    /// OMSI measures the spring's travel where the tyres stand - halfway between
    /// `achse_minwidth` and `achse_maxwidth`, (max + min) / 4 from the middle - but turns the
    /// body with the force at `achse_maxwidth` / 2.
    pub lever: f32,
    pub radius: f32,
    pub driven: bool,
    pub steered: bool,
    /// The wheel's steering angle this step (rad, positive to the right): every axle points
    /// at the turning centre on the `[rot_pnt_long]` line, atan((long - rot_pnt_long) x
    /// curvature), so an axle behind that line steers the other way.
    pub steer: f32,
    /// Spring rate (N/m), damper rate (N s/m) and the load the strut is built for (N).
    pub spring: f32,
    pub damper: f32,
    pub max_force: f32,
    /// `Axle_Springfactor_*`: air suspensions scale the spring with the bellows pressure.
    pub spring_factor: f32,
    /// Static load at rest (N), from the mass split over the axles.
    pub rest_load: f32,
    /// Current compression (m, positive = the wheel pushed up into the body) and its rate.
    pub compression: f32,
    pub compression_rate: f32,
    /// The compression at which the tyre would just touch the ground, last substep (None
    /// in the air or right after [`RigidBody::place`](super::RigidBody::place)).
    pub touch: Option<f32>,
    pub on_ground: bool,
    pub rotation_deg: f32,
    pub rpm: f32,
    /// Load on the tyre this step (N).
    pub load: f32,
    /// World height of the ground the tyre stands on this step (the surface under the hub
    /// or the edge it is climbing).
    pub ground_z: f64,
    /// `ground_z` came from the ground (not yet after [`RigidBody::place`](super::RigidBody::place)): steps up to
    /// [`CLIMB`](super::CLIMB) radii over it are ground, higher ones walls.
    pub ground_seen: bool,
    /// Faces the tyre is up against.
    pub walls: Vec<WallFace>,
    /// Horizontal push (N) of a step the tyre climbs or a wall it stands against.
    pub step_force: f32,
}

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
    /// The hub in the body frame (x right, y forward, z up, relative to the model origin) with
    /// the spring unloaded.
    pub fn attach(&self) -> Vec3 {
        self.attach
    }

    /// The tyre radius (m).
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// Whether the wheel steers.
    pub fn steered(&self) -> bool {
        self.steered
    }

    /// The spring rate (N/m).
    pub fn spring(&self) -> f32 {
        self.spring
    }

    /// `Axle_Springfactor_*`: the spring's scale by the bellows pressure.
    pub fn spring_factor(&self) -> f32 {
        self.spring_factor
    }

    /// The static load at rest (N).
    pub fn rest_load(&self) -> f32 {
        self.rest_load
    }

    /// The compression (m, positive = the wheel pushed up into the body).
    pub fn compression(&self) -> f32 {
        self.compression
    }

    /// The compression's rate (m/s).
    pub fn compression_rate(&self) -> f32 {
        self.compression_rate
    }

    /// Whether the tyre stands on the ground.
    pub fn on_ground(&self) -> bool {
        self.on_ground
    }

    /// The load on the tyre this step (N).
    pub fn load(&self) -> f32 {
        self.load
    }

    /// The world height of the ground the tyre stands on this step.
    pub fn ground_z(&self) -> f64 {
        self.ground_z
    }

    /// The horizontal push (N) of a step the tyre climbs or a wall it stands against.
    pub fn step_force(&self) -> f32 {
        self.step_force
    }

    /// The wheel's own turning (rad/s).
    pub fn spin(&self) -> f32 {
        self.spin
    }

    /// Whether the wheel slips (spins or skids).
    pub fn slipping(&self) -> bool {
        self.slipping
    }

    /// Whether a brake stopped the wheel.
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// Whether the wheel is driven.
    pub fn driven(&self) -> bool {
        self.driven
    }

    /// The wheel's steering angle this step (rad, positive to the right).
    pub fn steer(&self) -> f32 {
        self.steer
    }

    /// The compression at which the tyre would just touch the ground, last substep.
    pub fn touch(&self) -> Option<f32> {
        self.touch
    }

    /// Whether `ground_z` came from the ground (not yet after `RigidBody::place`).
    pub fn ground_seen(&self) -> bool {
        self.ground_seen
    }

    /// The wheel's revolutions per minute.
    pub fn rpm(&self) -> f32 {
        self.rpm
    }

    /// The wheel's turn about its axle (deg).
    pub fn rotation_deg(&self) -> f32 {
        self.rotation_deg
    }

    /// Sets the wheel's own turning (rad/s).
    pub fn set_spin(&mut self, spin: f32) {
        self.spin = spin;
    }

    /// Sets `Axle_Springfactor_*`, the spring's scale by the bellows pressure.
    pub fn set_spring_factor(&mut self, factor: f32) {
        self.spring_factor = factor;
    }

    /// The faces the tyre is up against.
    pub fn walls(&self) -> &[WallFace] {
        &self.walls
    }

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

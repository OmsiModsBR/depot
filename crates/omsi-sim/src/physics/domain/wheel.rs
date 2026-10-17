//! A wheel of the body ([`RigidWheel`]), with what reading it gives.

use super::WallFace;
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct RigidWheel {
    /// The wheel's own turning (rad/s) and its state as OMSI keeps it:
    /// gripping (turning with the ground), slipping (spinning or skidding: the tyre passes on
    /// the sliding friction and the wheel turns by its own torques), locked (a brake that
    /// stopped it). `inertia_inv`: the axle's `achse_inertia_inv` (0.002 when not given).
    pub(crate) spin: f32,
    pub(crate) slipping: bool,
    pub(crate) locked: bool,
    pub(crate) inertia_inv: f32,
    /// Hub in the body frame (x right, y forward, z up, relative to the model origin) with
    /// the spring unloaded: the model's wheel centre when `Axle_Suspension` reads zero.
    pub(crate) attach: Vec3,
    /// How much farther out than the hub the strut's force acts (m, signed like `attach.x`).
    /// OMSI measures the spring's travel where the tyres stand - halfway between
    /// `achse_minwidth` and `achse_maxwidth`, (max + min) / 4 from the middle - but turns the
    /// body with the force at `achse_maxwidth` / 2.
    pub(crate) lever: f32,
    pub(crate) radius: f32,
    pub(crate) driven: bool,
    pub(crate) steered: bool,
    /// The wheel's steering angle this step (rad, positive to the right): every axle points
    /// at the turning centre on the `[rot_pnt_long]` line, atan((long - rot_pnt_long) x
    /// curvature), so an axle behind that line steers the other way.
    pub(crate) steer: f32,
    /// Spring rate (N/m), damper rate (N s/m) and the load the strut is built for (N).
    pub(crate) spring: f32,
    pub(crate) damper: f32,
    pub(crate) max_force: f32,
    /// `Axle_Springfactor_*`: air suspensions scale the spring with the bellows pressure.
    pub(crate) spring_factor: f32,
    /// Static load at rest (N), from the mass split over the axles.
    pub(crate) rest_load: f32,
    /// Current compression (m, positive = the wheel pushed up into the body) and its rate.
    pub(crate) compression: f32,
    pub(crate) compression_rate: f32,
    /// The compression at which the tyre would just touch the ground, last substep (None
    /// in the air or right after [`RigidBody::place`](crate::physics::RigidBody::place)).
    pub(crate) touch: Option<f32>,
    pub(crate) on_ground: bool,
    pub(crate) rotation_deg: f32,
    pub(crate) rpm: f32,
    /// Load on the tyre this step (N).
    pub(crate) load: f32,
    /// World height of the ground the tyre stands on this step (the surface under the hub
    /// or the edge it is climbing).
    pub(crate) ground_z: f64,
    /// `ground_z` came from the ground (not yet after [`RigidBody::place`](crate::physics::RigidBody::place)): steps up to
    /// [`CLIMB`](crate::physics::CLIMB) radii over it are ground, higher ones walls.
    pub(crate) ground_seen: bool,
    /// Faces the tyre is up against.
    pub(crate) walls: Vec<WallFace>,
    /// Horizontal push (N) of a step the tyre climbs or a wall it stands against.
    pub(crate) step_force: f32,
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
}

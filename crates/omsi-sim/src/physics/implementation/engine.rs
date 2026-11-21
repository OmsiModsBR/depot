//! Where OMSI Classic and Project Depot Physics differ: one method per rule, each taking the
//! engine's own (`classic`, `depot`). Everything not here is common to both
//! (`docs/physics/common-model.md`); what each engine's rules do is in
//! `docs/physics/classic-model.md` and `docs/physics/depot-model.md`.

use super::{classic, depot};
use crate::physics::domain::{Engine, EngineState, Joint, Payload, RigidBody, SteerAxle};
use glam::Vec3;
use omsi_vehicle::Vehicle;

impl Engine {
    /// Whether `body` steers its own axles: a section with steerable axles under Project Depot
    /// Physics, leading or behind a joint (`D-5` to `D-7`). OMSI Classic points every axle at
    /// the centre of the turn (`C-6`), and a section behind a joint without them keeps `C-9`.
    pub(in crate::physics) fn steers_axles(self, body: &RigidBody) -> bool {
        match self {
            Engine::Classic => false,
            Engine::Depot => !body.steer_axles.is_empty(),
        }
    }

    /// The section's steerable axles (`D-5`); OMSI Classic has none and leaves the parameters
    /// out.
    pub(in crate::physics) fn steer_axles(self, def: &Vehicle) -> Vec<SteerAxle> {
        match self {
            Engine::Classic => Vec::new(),
            Engine::Depot => depot::steering::steer_axles(def),
        }
    }

    /// The static load on each wheel of each axle (N): the weight shared about the model's
    /// origin (`K-1`) or about `[cog]` (`D-1`).
    pub(in crate::physics) fn wheel_rest_loads(self, def: &Vehicle) -> Vec<f32> {
        match self {
            Engine::Classic => classic::load_share::wheel_rest_loads(def),
            Engine::Depot => depot::load_share::wheel_rest_loads(def),
        }
    }

    /// The static load on each wheel of each axle (N) as the kinematic model stands on it
    /// (`KA-4`, `DA-6`).
    pub(in crate::physics) fn kinematic_wheel_rest_loads(self, def: &Vehicle) -> Vec<f32> {
        match self {
            Engine::Classic => classic::load_share::kinematic_wheel_rest_loads(def),
            Engine::Depot => depot::load_share::kinematic_wheel_rest_loads(def),
        }
    }

    /// What a strut's force `n` adds to the body's torque beyond its own lever about `cog`
    /// (`K-1`, `D-1`).
    pub(in crate::physics) fn strut_reference_torque(self, cog: Vec3, n: f32) -> Vec3 {
        match self {
            Engine::Classic => classic::load_share::strut_reference_torque(cog, n),
            Engine::Depot => depot::load_share::strut_reference_torque(cog, n),
        }
    }

    /// The body's centre of gravity and inertia with what it carries, from the empty body's
    /// (`[cog]`, mass in kg, `[momentofintertia]`) (`D-2`; Classic keeps the empty body's).
    pub(in crate::physics) fn loaded_body(
        self,
        empty: (Vec3, f32, Vec3),
        load: Payload,
    ) -> (Vec3, Vec3) {
        match self {
            Engine::Classic => classic::load_share::loaded_body(empty, load),
            Engine::Depot => depot::load_share::loaded_body(empty, load),
        }
    }

    /// The body's pitch and roll frequency (1/s) with `mass` (kg) aboard, from the empty
    /// body's `empty_freq` and `empty_mass` (`K-5`, `D-4`).
    pub(in crate::physics) fn loaded_body_freq(
        self,
        empty_freq: f32,
        empty_mass: f32,
        mass: f32,
    ) -> f32 {
        match self {
            Engine::Classic => classic::load_share::loaded_body_freq(empty_freq),
            Engine::Depot => depot::load_share::loaded_body_freq(empty_freq, empty_mass, mass),
        }
    }

    /// Whether the joint `j` stands where its sections' tyres let go (`K-3`; Depot has no such
    /// place).
    pub(in crate::physics) fn joint_lets_tyres_go(self, j: &Joint, bodies: &[RigidBody]) -> bool {
        match self {
            Engine::Classic => classic::joint::at_alpha_limit(j, bodies),
            Engine::Depot => false,
        }
    }

    /// What the engine does to a train at the end of a step that ran: OMSI Classic puts the
    /// sections behind back on their joints (`K-2`); Depot leaves them where the solver
    /// left them.
    pub(in crate::physics) fn after_step(self, bodies: &mut [RigidBody], joints: &[Joint]) {
        match self {
            Engine::Classic => classic::joint::seat_followers(bodies, joints),
            Engine::Depot => {}
        }
    }
}

/// What the tyres of a body are asked to take across in a substep (`C-7`).
pub(in crate::physics) struct AcrossLoad<I> {
    /// The grip of all its tyres together (N).
    pub grip_all: f32,
    /// The bend's pull, less what a joint pushes the body with (N, to its right).
    pub across: f32,
    /// Each tyre's lever (m forward of the centre of gravity) and grip (N).
    pub tyres: I,
    /// The body's yaw rate (rad/s, body z), its forward speed (m/s), the substep (s) and its
    /// yaw inertia (kg m²).
    pub yaw_rate: f32,
    pub forward_speed: f32,
    pub h: f32,
    pub yaw_inertia: f32,
}

impl EngineState {
    /// Whether the tyres still hold the bend: all of them together (`C-7`), or axle by axle
    /// with the force it takes to start or stop the body turning (`D-3`, which keeps the yaw
    /// it measured).
    pub(in crate::physics) fn holds_across<I: Iterator<Item = (f32, f32)>>(
        &mut self,
        load: AcrossLoad<I>,
    ) -> bool {
        match self {
            EngineState::Classic { .. } => classic::grip::holds_across(load.grip_all, load.across),
            EngineState::Depot { yaw } => {
                yaw.accel = depot::grip::yaw_accel(
                    yaw.accel,
                    load.yaw_rate,
                    yaw.last_rate,
                    load.forward_speed,
                    load.h,
                );
                yaw.last_rate = load.yaw_rate;
                depot::grip::holds_across(load.tyres, load.across, load.yaw_inertia * yaw.accel)
            }
        }
    }
}

impl Joint {
    /// The curvature (1/m, positive to the right) the joint, bent by `alpha`, makes the
    /// follower turn on about its `[rot_pnt_long]` line: the way the lead carries the joint as
    /// well (`K-4`), or sin alpha over the follower's arm (Depot), the arm from the follower's
    /// own turning line (with steerable axles, the one `D-5` gives it). The follower's axles
    /// show it (`axle_steer`); its tyres do not steer unless it has steerable axles (`D-6`).
    pub(in crate::physics) fn follower_kappa(&self, alpha: f32, bodies: &[RigidBody]) -> f32 {
        let (l, f) = (&bodies[self.lead], &bodies[self.follow]);
        let follow_arm = self.anchor_follow.y - f.rot_pnt_long;
        match (f.rules, l.rules) {
            (EngineState::Classic { .. }, EngineState::Classic { file_rot_pnt_long }) => {
                classic::joint::follower_kappa(
                    alpha,
                    self.anchor_lead.y - file_rot_pnt_long,
                    l.kappa,
                    follow_arm,
                )
            }
            // (a train runs one engine: the front section's file decides for all of them)
            _ => depot::joint::follower_kappa(alpha, follow_arm),
        }
    }
}

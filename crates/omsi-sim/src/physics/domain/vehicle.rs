//! What the rest of the game hands a vehicle's physics and reads of it: the driver's
//! [`Controls`], the frame's [`VehicleMotion`], what moves the vehicle ([`Dynamics`]) and its
//! physics as it stands ([`PhysicsState`]).

use super::{KinematicModel, RigidBody};
use glam::Vec3;
use omsi_cfg::PhysicsMode;

/// The rules a vehicle's physics follows where OMSI Classic and Project Depot Physics differ
/// (spec 002). Decided when the vehicle appears and held by its [`RigidBody`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Engine {
    #[default]
    Classic,
    Depot,
}

impl From<PhysicsMode> for Engine {
    fn from(mode: PhysicsMode) -> Engine {
        match mode {
            PhysicsMode::Classic => Engine::Classic,
            PhysicsMode::Depot => Engine::Depot,
        }
    }
}

impl From<Engine> for PhysicsMode {
    fn from(engine: Engine) -> PhysicsMode {
        match engine {
            Engine::Classic => PhysicsMode::Classic,
            Engine::Depot => PhysicsMode::Depot,
        }
    }
}

/// The engine a body follows and what only that engine keeps of it between steps: the rules
/// themselves are in `implementation/engine.rs`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum EngineState {
    /// OMSI Classic, with the file's own `[rot_pnt_long]` (m, not counted from `[cog]`): where
    /// it measures the lead's arm to a joint when it bends the section behind (`K-4`).
    Classic { file_rot_pnt_long: f32 },
    /// Project Depot Physics, with the yaw its tyres were asked for (`D-3`).
    Depot { yaw: YawLag },
}

/// The yaw acceleration the body's turning asked of the tyres (rad/s², body z), smoothed over
/// the tyres' relaxation, and the yaw rate it was measured from (`D-3`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct YawLag {
    pub(crate) accel: f32,
    pub(crate) last_rate: f32,
}

impl EngineState {
    /// The state a new body of `engine` starts with; `file_rot_pnt_long` the `.bus` file's
    /// `[rot_pnt_long]` (m).
    pub(crate) fn new(engine: Engine, file_rot_pnt_long: f32) -> EngineState {
        match engine {
            Engine::Classic => EngineState::Classic { file_rot_pnt_long },
            Engine::Depot => EngineState::Depot {
                yaw: YawLag::default(),
            },
        }
    }

    /// The engine.
    pub(crate) fn engine(&self) -> Engine {
        match self {
            EngineState::Classic { .. } => Engine::Classic,
            EngineState::Depot { .. } => Engine::Depot,
        }
    }

    /// Forgets what the body's motion left behind (the body was put down anew).
    pub(crate) fn reset(&mut self) {
        if let EngineState::Depot { yaw } = self {
            *yaw = YawLag::default();
        }
    }
}

/// Driver inputs, all in `0..1` except steering in `-1..1` (positive = right).
#[derive(Debug, Clone, Copy, Default)]
pub struct Controls {
    pub throttle: f32,
    pub brake: f32,
    pub clutch: f32,
    pub steering: f32,
}

#[derive(Debug, Clone)]
pub struct WheelState {
    /// Longitudinal axle position (m, forward positive) and lateral offset (m, right positive).
    pub long: f32,
    pub lat: f32,
    pub radius: f32,
    pub driven: bool,
    pub rotation_deg: f32,
    pub rpm: f32,
    pub suspension: f32,
}

/// The vehicle's motion this frame, as the rest of the game reads it (the dashboard, the
/// sounds, the passengers, the network ...), whichever model moves the vehicle: its rigid
/// body, the kinematic model or the AI. Read-only outside the physics: it writes it once a
/// frame, together with the scripts' variables (see `VehicleInstance::motion`).
#[derive(Debug, Clone)]
pub struct VehicleMotion {
    /// Longitudinal speed, m/s (forward positive).
    pub speed: f32,
    /// The front wheels' steering angle, degrees, positive = right.
    pub steer_deg: f32,
    /// The body's acceleration (m/s², x right, y forward, z up) as its model has it: the
    /// rigid body's carries gravity, as an accelerometer reads it.
    pub accel: Vec3,
    /// What the scripts read as `A_Trans_*`: the acceleration without gravity.
    pub a_trans: Vec3,
    /// Per axle: (left, right) wheels.
    pub wheels: Vec<[WheelState; 2]>,
}

impl VehicleMotion {
    pub fn velocity_kmh(&self) -> f32 {
        self.speed * 3.6
    }
}

/// What moves a vehicle. (A simplified body for the AI is to come as a third kind.)
// (the rigid body unboxed: there is one a vehicle, and it moves out for its step and back
// every frame, which a box would allocate anew)
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Dynamics {
    /// A rigid body on its wheels: the player's vehicle, and the one the launcher shows (see
    /// `crate::rigid`).
    Rigid(RigidBody),
    /// The kinematic model: the player's vehicle on rails, and the copies the AI and other
    /// games drive (their pose is set from outside, the model is not stepped).
    Kinematic(KinematicModel),
}

/// A vehicle's physics as it stands - what moves it, its motion and the driver's controls -
/// to be put back with `VehicleInstance::set_physics_state` (runs tried from the same start).
#[derive(Debug, Clone)]
pub struct PhysicsState {
    pub(crate) dynamics: Dynamics,
    pub(crate) motion: VehicleMotion,
    pub(crate) controls: Controls,
}

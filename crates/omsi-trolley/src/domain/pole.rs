//! The state of a trolleybus's two poles, what moves them and what they show.

use glam::{DMat3, DVec3};

use super::network::{SpanId, WireSide};

/// Where a pole is in its life on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolePhase {
    /// Down on its hook.
    Hooked,
    /// Going up onto a wire it has found.
    Rising,
    /// On the wire.
    OnWire,
    /// Going down to its hook.
    Lowering,
    /// Off the wire against the driver's will: it springs up, and after a moment the
    /// retriever pulls it down to its hook.
    Dewired,
    /// Taken off the wire by the retract control: down halfway, a pause, then to the hook.
    Retracting,
}

/// Why a pole left the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DewireReason {
    /// The wire stayed out of the pole's reach too long.
    Reach,
    /// The shoe pressed on the wire too lightly too long.
    Force,
    /// The wire pulled the shoe sideways too hard too long.
    SideForce,
    /// The pole turned too far to the side.
    SideAngle,
    /// The wire ended under the shoe.
    Gap,
}

/// A place on one wire: span, which of its two wires, the piece of it (points `seg` and
/// `seg + 1`) and how far along that piece (0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WireCursor {
    pub span: SpanId,
    pub side: WireSide,
    pub seg: usize,
    pub t: f64,
}

/// One pole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleState {
    pub phase: PolePhase,
    /// Where its shoe is on the wire (rising and on the wire).
    pub cursor: Option<WireCursor>,
    /// Seconds in the current phase.
    pub timer: f64,
    /// Yaw and pitch (output degrees) where lowering, a dewirement or retracting started.
    pub lowered_from: (f64, f64),
    /// Why it left the wire (while dewired).
    pub dewire: Option<DewireReason>,
    /// Seconds each dewirement condition has held without a break: reach, force, side force.
    pub held: [f64; 3],
    /// How much longer than the pole the way from its foot to the wire is (metres; 0 when
    /// the pole reaches it).
    pub reach_error: f64,
}

impl Default for PoleState {
    fn default() -> Self {
        PoleState {
            phase: PolePhase::Hooked,
            cursor: None,
            timer: 0.0,
            lowered_from: (0.0, 0.0),
            dewire: None,
            held: [0.0; 3],
            reach_error: 0.0,
        }
    }
}

/// Where the part that carries the poles stands: its origin in the world and its rotation
/// (body frame to world), suspension included.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyPose {
    pub position: DVec3,
    pub rotation: DMat3,
}

/// What the vehicle asks of its poles this frame (left, right).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PoleInputs {
    /// The player holds the pole's rope (its control is pressed).
    pub grab: [bool; 2],
    /// Put both poles back on their hooks at once.
    pub reset: bool,
    /// The retract control: take both poles off the wire to their hooks.
    pub retract: bool,
}

/// What one pole shows.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PoleOutput {
    /// Degrees from the modelled pose: yaw (positive turns the shoe to the left), pitch
    /// (positive up), the shoe's turn against the pole.
    pub yaw: f64,
    pub pitch: f64,
    pub shoe_rot: f64,
    /// The end of the pole in the body frame.
    pub shoe_pos: DVec3,
    /// Newtons pressing the shoe on the wire (0 off the wire).
    pub force: f64,
    pub on_wire: bool,
    /// Off the wire by a dewirement, until the pole is back on its hook.
    pub dewired: bool,
}

/// What the poles show this frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PoleOutputs {
    pub poles: [PoleOutput; 2],
    /// Both shoes are on the wire.
    pub wire_present: bool,
    /// The map id of the spline the left shoe is on and how far along it (metres), or -1.
    pub spline_id: i64,
    pub progress: f64,
}

/// Something worth a line in the detailed log.
#[derive(Debug, Clone, PartialEq)]
pub enum PoleEvent {
    Captured {
        pole: usize,
        spline_id: i64,
        sideways: f64,
    },
    NoWire {
        pole: usize,
    },
    Joint {
        pole: usize,
        from: i64,
        to: i64,
        gap: f64,
    },
    WireEnds {
        pole: usize,
        spline_id: i64,
    },
    Lowered {
        pole: usize,
    },
    Dewired {
        pole: usize,
        reason: DewireReason,
    },
    Retracted {
        pole: usize,
    },
    StallReset {
        seconds: f64,
    },
}

/// The two poles of the player's trolleybus.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PoleSim {
    pub poles: [PoleState; 2],
    pub(crate) grab_before: [bool; 2],
    pub(crate) retract_before: bool,
    /// What happened in the last step.
    pub events: Vec<PoleEvent>,
}

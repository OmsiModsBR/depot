//! The types of the player: the player's bus with its controls and cameras, the driver on
//! the bus's `[drivpos]`, the walk about the bus, a rail vehicle bound to its track. Their
//! procedures are in `implementation/`.

use glam::Vec3;
use omsi_render::MeshId;
use omsi_sim::people::{HumanType, Pose};
use std::sync::Arc;

/// The gear lever of a manual bus, as found in its model (`find_shifter`).
pub(crate) struct Shifter {
    /// The lever mesh (index into the vehicle's meshes).
    pub(crate) mesh: usize,
    /// Variables watched for gear changes: the lever's own and any gear variable the
    /// vehicle has.
    pub(crate) vars: Vec<String>,
    /// The clutch pedal's variable, if the vehicle has one.
    pub(crate) clutch: Option<String>,
    /// Where the hand holds the knob, and the lever's axis (foot to knob), at rest, model
    /// frame.
    pub(crate) grab: Vec3,
    pub(crate) axis: Vec3,
    /// The hand that works it (0 left, 1 right): the one on the lever's side of the seat.
    pub(crate) hand: usize,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum ShiftPhase {
    /// The hand is on the rim.
    Away,
    /// On its way to the knob.
    Reach,
    /// On the knob.
    Hold,
    /// On its way back to the rim.
    Back,
}

pub(crate) struct ShiftState {
    pub(crate) phase: ShiftPhase,
    /// 0 hand on the rim .. 1 hand on the knob (linear, eased where it is used).
    pub(crate) w: f32,
    /// How long (s) the hand takes over the way to the knob this time.
    pub(crate) reach: f32,
    /// The bus has stood still long enough (hysteresis on its speed).
    pub(crate) stopped: bool,
    pub(crate) still_for: f32,
    /// Time (s) the stopped bus's hand stays off the lever after the wheel was turned too
    /// far for one hand.
    pub(crate) cool: f32,
    pub(crate) last_pos: Option<glam::DVec3>,
    pub(crate) last_knob: Option<Vec3>,
    pub(crate) last_vars: Vec<f32>,
    pub(crate) last_clutch: f32,
    pub(crate) was_active: bool,
    /// Time (s) since the lever or the clutch last moved.
    pub(crate) idle: f32,
    /// A shift is under way and the hand has not started for the lever yet.
    pub(crate) pending: bool,
    pub(crate) waiting: f32,
    /// Progress (0..1) of the driver's own push on the knob; 1 when there is none.
    pub(crate) stroke: f32,
    /// Where the knob was when the shift began and how far it has moved since.
    pub(crate) event_from: Vec3,
    pub(crate) event_travel: f32,
}

pub(crate) struct Wheel {
    /// The steering wheel mesh (index into the vehicle's meshes).
    pub(crate) mesh: usize,
    /// Its `[newanim]` variable and factor: the wheel's angle in degrees is their product.
    pub(crate) var: String,
    pub(crate) factor: f32,
    /// The turning axis, pointing at the driver.
    pub(crate) axis: Vec3,
    /// Centre, axis (towards the driver) and the rim's up and right in the wheel's plane,
    /// at rest, in the model frame; the rim radius the hands hold.
    pub(crate) centre: Vec3,
    pub(crate) up: Vec3,
    pub(crate) right: Vec3,
    pub(crate) radius: f32,
    /// The rim's own thickness (its tube's radius, m).
    pub(crate) tube: f32,
}

pub struct DriverFigure {
    pub(crate) ty: Arc<HumanType>,
    /// The figure's meshes with the fingers closed round the rim (`curl_hands`).
    pub(crate) curled: Vec<(Vec<Vec3>, Vec<Vec3>)>,
    /// Wrist to knuckles (m).
    pub(crate) knuckles: f32,
    /// Where the closed fingers hold their bar, rest frame (see `grip_centres`), and how far
    /// the wrists' targets are moved (person frame) so that the bar they hold is the rim:
    /// the hand does not always turn as far as the grip asks (its bend at the wrist is
    /// limited), and the fingers closed round the air beside the rim.
    pub(crate) grip_rest: [Option<Vec3>; 2],
    pub(crate) grip_fix: [Vec3; 2],
    /// The radius the fingers are closed round (the rim's tube and the fingers' half
    /// thickness) that `curled` and `grip_rest` are made for.
    pub(crate) grip_radius: f32,
    pub(crate) pose: Pose,
    pub(crate) meshes: Vec<(MeshId, usize)>,
    pub(crate) skins: Vec<(Vec<Vec3>, Vec<Vec3>)>,
    /// Hip point, floor point in front of the seat, heading (model frame).
    pub(crate) hip: Vec3,
    pub(crate) floor: Vec3,
    pub(crate) heading: f32,
    /// The seat's four `[interiorlight]`s (see `PassPos::illumination`): the lamps that
    /// light the figure, as OMSI lights a seated person.
    pub(crate) lamps: [i32; 4],
    pub(crate) wheel: Option<Wheel>,
    /// The sign that turns the wheel variable's angle into the angle seen from the seat
    /// (found by comparing it with the mesh as turned; 0 until then).
    pub(crate) sign: f32,
    /// Extra forward lean that brings the hands to the rim (degrees, see SLIDE_MAX), and
    /// what it is with the hands at their places.
    pub(crate) lean: f32,
    pub(crate) base_lean: f32,
    /// Whether hands/arms should remain visible in cab (first-person) view behind settings.
    pub show_hands_in_cab: bool,
    pub(crate) shown: bool,
    /// Posed at least once (the first pose is settled, not eased in from standing).
    pub(crate) settled: bool,
    /// How far the seat is slid forward so that the hands reach the rim (m): a figure of
    /// the stock size on the LiAZ's `[drivpos]` sat 0.7 m behind its wheel with its arms
    /// stretched out in the air.
    pub(crate) slide: f32,
    /// Where each hand holds the rim, or is on its way to (see `steer_hands`).
    pub(crate) hands: [Hand; 2],
    pub(crate) hands_placed: bool,
    /// The wheel's angle seen from the seat (degrees, clockwise) now and a frame ago, and
    /// how long it has been held still (s).
    pub(crate) theta: f32,
    pub(crate) last_theta: f32,
    pub(crate) still: f32,
    /// How fast the wheel turns (degrees/s, smoothed).
    pub(crate) rate: f32,
    /// The elbows as last posed (model frame): the forearms the hands continue.
    pub(crate) elbows: Option<[Vec3; 2]>,
    /// How each hand lies now (wrist to knuckles, palm; model frame), eased.
    pub(crate) frames: [Option<(Vec3, Vec3)>; 2],
    /// How far each fist is rolled round the rim (degrees), eased.
    pub(crate) rolls: [Option<f32>; 2],
    /// Per mesh and vertex, the hand it belongs to (0 left, 1 right, -1 none), and a
    /// scratch copy of a mesh with a hand opening.
    pub(crate) hand_of: Vec<Vec<i8>>,
    /// Per mesh and vertex, the arm (upper arm and forearm) it belongs to (0 left, 1 right,
    /// -1 none): the arms stay in the cab view along with the hands.
    pub(crate) arm_of: Vec<Vec<i8>>,
    pub(crate) blend: (Vec<Vec3>, Vec<Vec3>),
    /// The gear lever, when the bus's model has one near the seat, and what the hand
    /// working it is doing.
    pub(crate) shifter: Option<Shifter>,
    pub(crate) shift: ShiftState,
}

/// A hand on the rim: the point it holds, as an angle on the wheel (the angle seen from the
/// seat less the wheel's), or its way to a new hold.
#[derive(Clone, Copy, Default)]
pub(crate) struct Hand {
    pub(crate) on_rim: f32,
    pub(crate) mv: Option<Regrip>,
}

/// A hand let go of the rim and moving to hold it again: from and to angles seen from the
/// seat (degrees), progress 0..1 and duration (s).
#[derive(Clone, Copy)]
pub(crate) struct Regrip {
    pub(crate) from: f32,
    pub(crate) to: f32,
    pub(crate) t: f32,
    pub(crate) dur: f32,
    /// How fast the hand moves along the rim as it lets go and as it takes hold (degrees
    /// over the whole move): it leaves the rim moving with it and meets it the same way,
    /// not stopping dead in one frame.
    pub(crate) v0: f32,
    pub(crate) v1: f32,
}

/// Where the hands go this frame (model frame): the wrists, how each hand lies (wrist to
/// knuckles, the way the palm faces) and the points of the rim they hold.
pub(crate) struct Targets {
    pub(crate) grips: [Vec3; 2],
    pub(crate) frames: [(Vec3, Vec3); 2],
    pub(crate) tubes: [Vec3; 2],
}

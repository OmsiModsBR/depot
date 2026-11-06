//! The types of the player: the player's bus with its controls and cameras, the driver on
//! the bus's `[drivpos]`, the walk about the bus, a rail vehicle bound to its track. Their
//! procedures are in `implementation/`.

use crate::gameplay::humans::BusId;
use glam::{DVec2, DVec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootCam {
    /// Out of the walker's eyes (F1).
    First,
    /// The free camera (F4): the walker stands where it was.
    Free,
}

pub struct OnFoot {
    /// The feet on the ground.
    pub pos: DVec3,
    pub heading: f64,
    pub vel: DVec2,
    /// Height of the feet over the ground and its rate (a jump).
    pub lift: f64,
    pub vz: f64,
    /// Sitting on this seat of this bus.
    pub seat: Option<(BusId, usize)>,
    /// Standing or walking inside this bus, at this point of its cabin (bus frame): the
    /// walker goes with the bus, on its floor, and in and out through its open doors.
    pub inside: Option<(BusId, glam::Vec3)>,
    pub cam: FootCam,
    /// Where the camera looks (degrees).
    pub yaw: f32,
    pub pitch: f32,
    /// The camera as it eases towards where it belongs.
    pub eye: Option<DVec3>,
    pub eye_yaw: f32,
    /// How far the eyes' glide out of the cab's camera was behind the walker's head when it
    /// ended: let go of over a third of a second, so that the camera does not jump the
    /// last bit onto the head (the bus's own placement of the eyes takes over there).
    pub lag: DVec3,
    /// Just got up from the driver's seat: seconds (1 → 0) the eyes take to leave the cab's
    /// camera, slowly at first, instead of jumping to where the walker's head is.
    pub settle: f32,
    /// The view the player had in the bus (back to it at the wheel).
    pub view_before: String,
    /// The figure (a human type index, taken modulo their number).
    pub kind: u64,
    /// Just sat down: the view turns to where the seat faces.
    pub face_seat: bool,
    /// Stepping through a door or out of the cab: walked over a moment, not jumped.
    pub transit: Option<Transit>,
    /// Arrived at the end of a walk that had somewhere to go (the wheel): done next frame.
    pub arrive: Option<Then>,
    /// Down on the knees (C, #1148): the eyes low, for a picture from below.
    pub kneel: bool,
    /// How far down the eyes are on their way (0 standing .. 1 kneeling).
    pub crouch: f32,
}

/// What a walk leads to once walked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Then {
    Nothing,
    /// the own bus's wheel
    Wheel,
    /// the wheel of the placed vehicle with this uid
    Placed(u64),
}

/// A step the walker takes by itself: from `from` to `to` (world, feet) over `dur` seconds,
/// then inside `after` (a bus and the cabin point) or on the ground.
#[derive(Debug, Clone, Copy)]
pub struct Transit {
    pub from: DVec3,
    pub to: DVec3,
    pub t: f32,
    pub dur: f32,
    pub after: Option<(BusId, glam::Vec3)>,
    pub then: Then,
}

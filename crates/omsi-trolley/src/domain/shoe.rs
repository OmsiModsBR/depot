//! Where on a shoe the wire runs: the one rule's measures, and what it finds on a shoe.
//!
//! The rule, the same for every model (`implementation::shoe`, and `docs/TROLLEYBUS.md`,
//! "Where the shoe meets the wire"):
//! - the shoe's points are the mesh points within [`SHOE_REACH`] of the shoe's pivot, along
//!   the ground;
//! - the wire runs straight above the pivot, where nothing of the shoe stands above the
//!   wire's top: its centre lies [`WIRE_RADIUS`] below the shoe's highest point;
//! - and the groove the wire runs in (the highest point within [`SHOE_AXIS`] of the pivot's
//!   line) stays at least [`SHOE_SEAT`] below the wire's centre, so the wire lies in it.
//!
//! A shoe whose points give no place within [`SHOE_CONTACT_MAX`] of its pivot has none: the
//! pivot stands for it.

use glam::DVec3;

/// How far (metres, along the ground) from a shoe's pivot its mesh points count as the shoe:
/// a trolleybus shoe is about 10 cm long, so its body lies within 6 cm of its pivot, and
/// the pole's end, further back, does not.
pub const SHOE_REACH: f64 = 0.06;

/// How far (metres, along the ground) from the pivot's line a point counts as on it: the
/// floor of the groove, which is about 1.5 cm wide.
pub const SHOE_AXIS: f64 = 0.01;

/// The contact wire's radius (metres): the round grooved wire trolleybus lines hang
/// (85 to 107 mm², about 11 to 12 mm across).
pub const WIRE_RADIUS: f64 = 0.006;

/// How far (metres) below the wire's centre the floor of the groove stays at least: the
/// wire seats in the groove instead of resting on its edges.
pub const SHOE_SEAT: f64 = 0.002;

/// How far (metres) from the shoe's pivot the wire may be: further, the points are not the
/// shoe's (a model whose shoe mesh is not where its pivot is).
pub const SHOE_CONTACT_MAX: f64 = 0.25;

/// What the rule found on one shoe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShoeFit {
    /// The wire's centre on the shoe, in the model's frame.
    pub contact: DVec3,
    /// How many of the points given are the shoe's.
    pub points: usize,
    /// The height (model frame) of the shoe's top.
    pub top: f64,
    /// The height of its groove (None: no point on the pivot's line).
    pub groove: Option<f64>,
}

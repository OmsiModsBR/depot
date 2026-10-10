//! The rigid body's constants: how far a wheel travels, the tyre and its reach over the
//! ground, the walls a tyre cannot climb and the crashes.

/// How far a wheel may hang below its unloaded position (m).
pub const DROOP: f32 = 0.15;
/// Compression at which the bump stop takes over (m). A deflated air suspension rests on it.
pub const BUMP: f32 = 0.24;
/// The wheel between the road and the strut: its share of the corner's mass (axle, hub,
/// brake, tyre - about an eighth on a bus), and the tyre's vertical stiffness (N/m) and
/// damping (N s/m). Taken as massless, the wheel stood wherever the road put it: a bump
/// shoved it into the arch within a substep, the strut's damper kicked the body into the
/// air, and the wheel then dropped to its full droop at once - the bus hopped over a
/// manhole cover with its wheels flicking in and out of the body.
pub(super) const UNSPRUNG: f32 = 0.12;
pub(super) const TYRE_K: f32 = 900_000.0;
pub(super) const TYRE_C: f32 = 3_000.0;
/// The frame OMSI's per-frame damping of the body's pitch and roll is measured in (see
/// `RigidBus::step`): a thirtieth of a second, the rate its options.cfg caps OMSI at.
pub(super) const OMSI_FRAME: f32 = 1.0 / 30.0;

/// OMSI has no tyre between the road and the spring: `achse_feder` alone carries the body
/// (force = feder x penetration - daempfer x speed). The
/// tyre here is at least this many times as stiff as the spring, so that the two in series
/// give the body 94 % of the file's spring rate instead of the 79 % a fixed 900 kN/m tyre
/// left a bus with 240 kN/m springs - that softness was a good part of its sway.
pub(super) const TYRE_OVER_SPRING: f32 = 15.0;
/// Tyre samples along the envelope's length (see [`tyre_line`]).
pub(super) const TYRE_SAMPLES: usize = 13;
/// Samples taken: the lattice they stand on covers the envelope with one more to spare, and
/// two more beyond each end tell the edges near the ends from the grade.
pub(super) const LATTICE: usize = TYRE_SAMPLES + 5;
/// How far the tyre reaches for the ground ahead and behind, in radii. A pneumatic tyre
/// wraps round an edge and climbs it over about twice the length a rigid disc would: a
/// 15 cm kerb then costs about half the wheel load in push, not all of it, and a lowered
/// kerb of 3 cm is a nudge instead of the wall a rigid disc makes of it.
pub(super) const ENVELOPE: f32 = 2.0;
/// How far (m) the rise between two samples may depart from the rises beside it before it
/// is an edge rather than the road's own grade or curve.
pub(super) const EDGE: f64 = 0.01;
/// Halvings that place an edge between two samples (to a 64th of their spacing) and a face
/// the tyre cannot climb (to a 128th of the bracket).
pub(super) const EDGE_HALVINGS: usize = 6;
pub(super) const FACE_HALVINGS: usize = 7;
/// The highest step a tyre climbs, in radii over the ground it stands on; anything higher
/// is a wall. A step of the full radius would only give way to an infinite push (and the
/// hub and 5 cm, climbed before, took a bus over a 0.52 m step from 10 km/h). Up to 0.8 the
/// steepest slope the envelope meets (2.4) is still one the push takes in full (see
/// `step`), so a climb costs exactly the lift; and it keeps the stock maps' 30 cm gaps
/// between a side road and its junction something a bus drives over.
pub const CLIMB: f32 = 0.8;
/// Half the width of a tyre, in radii (a 275 mm bus tyre on a 0.47 m radius).
pub(super) const HALF_WIDTH: f32 = 0.3;
/// How far beside the tyre (in half widths from the hub) a face is looked for.
pub(super) const SIDE_REACH: f32 = 2.0;
/// How far (m) either side of a face's first point its direction is measured.
pub(super) const FACE_SPAN: f32 = 0.2;
/// A face this far above the axle next to the tyre is a wall, not something far overhead.
pub(super) const WALL_HEIGHT: f64 = 1.5;
/// Stiffness (N/m) and damping (N s/m) of a tyre pressed into a wall it cannot climb.
pub(super) const WALL_K: f32 = 1.5e6;
pub(super) const WALL_C: f32 = 6.0e4;
/// Share of the closing speed an obstacle gives back, and the sliding friction along it.
pub(super) const RESTITUTION: f32 = 0.2;
pub(super) const SCRAPE_FRICTION: f32 = 0.4;
/// Below this closing speed (m/s) a touch is a push, not a crash.
pub const CRASH_SPEED: f32 = 0.4;

/// `achse_inertia_inv` where a `[newachse]` does not say (OMSI LoadFromFile: 0.002).
pub const DEFAULT_INERTIA_INV: f32 = 0.002;

/// Below this speed (m/s) at the contact a tyre stands and its brake holds statically.
pub(super) const STANDING: f32 = 0.15;

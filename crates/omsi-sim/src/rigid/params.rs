//! The rigid body's constants, each with where its value comes from:
//!
//! - *OMSI*: what OMSI 2 does, or reads where a file does not say;
//! - *physics*: a physical constant or a textbook value;
//! - *fallback*: what a `.bus` file without the value gets;
//! - *tuned*: chosen here, for the case its note gives.
//!
//! Left in the code are unit conversions (t to kg, kN to N, rad to deg), halves and doubles,
//! the envelope's own geometry (see `ground::tyre_line`) and the floors that keep a
//! division away from zero or a broken file's values in range.

// The step.

/// The longest frame stepped (s): a longer one (a hitch, a frame after loading) is cut to it.
/// *tuned*
pub(super) const FRAME_DT_MAX: f32 = 0.25;
/// A frame is stepped in slices of at most this (s), each with its own steering and pitch and
/// roll damping: a long frame (below 20 fps) covers the whole time the clock counts. *tuned*
pub(super) const SLICE_DT: f32 = 0.05;
/// A slice is cut into substeps of about this (s), at least `SUBSTEPS_MIN` and at most
/// `SUBSTEPS_MAX` of them: 12 ms substeps were too long for the stiff tyres. *tuned*
pub(super) const SUBSTEP_DT: f32 = 0.0042;
pub(super) const SUBSTEPS_MIN: usize = 4;
pub(super) const SUBSTEPS_MAX: usize = 16;
/// Passes over the constraints of a substep: the tyres' alone take 8 (OMSI 2's hold across
/// converges within them); with joints the sections share their tyres through the joint, which
/// takes a few more. *tuned*
pub(super) const PASSES: usize = 8;
pub(super) const JOINT_PASSES: usize = 16;
/// How much of a joint's error a substep takes away (Baumgarte), and the fastest it does so
/// (m/s, rad/s): a part put down a little off its joint is drawn to it, not thrown. *tuned*
pub(super) const BAUMGARTE: f32 = 0.2;
pub(super) const MAX_BIAS: f32 = 1.0;
/// The joint's limits are looked at from this far (rad) inside them. *tuned*
pub(super) const LIMIT_WINDOW: f32 = 0.3;

// The body.

/// Gravity (m/s²). *physics*
pub(super) const GRAVITY: f32 = 9.81;
/// The air's push, ½ ρ cw A v²: its density (kg/m³) and a bus's drag coefficient. *physics*
pub(super) const AIR_DENSITY: f32 = 1.2;
pub(super) const DRAG_COEFFICIENT: f32 = 0.6;
/// The front the air pushes on, from the mass (kg per m²), between a car's and a bus's (m²).
/// Without the air a bus rolling downhill without its brakes ran on to any speed. *tuned*
pub(super) const MASS_PER_FRONT_AREA: f32 = 2200.0;
pub(super) const FRONT_AREA_MIN: f32 = 2.0;
pub(super) const FRONT_AREA_MAX: f32 = 8.5;
/// The frame OMSI's per-frame damping of the body's pitch and roll is measured in (see
/// `RigidBus::step`): a thirtieth of a second, the rate its options.cfg caps OMSI at. *OMSI*
pub(super) const OMSI_FRAME: f32 = 1.0 / 30.0;
/// OMSI damps the body's pitch and roll by sin(x)/x a frame, with x this many times the
/// body's frequency (`RigidBody::body_freq`) times the frame. *OMSI*
pub(super) const BODY_DAMPING: f32 = 1.5;
/// The body is set back on its wheels when its up axis leans further than this (its z, the
/// cosine of 72.5°): the safety net against tipping over on bad ground. *tuned*
pub(super) const TIP_OVER: f32 = 0.3;
/// Height (m) of the centre of gravity when `[schwerpunkt]` gives none. *fallback*
pub(super) const DEFAULT_COG_HEIGHT: f32 = 1.0;
/// Rolling resistance as a share of the weight when `[rollwiderstand]` gives none. *physics*
/// (a bus tyre on asphalt), *fallback*
pub(super) const ROLLING_COEFFICIENT: f32 = 0.008;
/// The grip of a dry road (μ), the body's until the vehicle sets the road's own (the dry
/// value of `vehicle::road_grip`). *tuned*
pub(super) const DRY_GRIP: f32 = 0.85;

// Steering.

/// The furthest a wheel or an axle turns (rad, about 60°). *tuned*
pub(super) const MAX_STEER: f32 = 1.05;
/// The range of the full lock (deg): a file's is held to it, and one without a usable
/// `[inv_min_turnradius]` gets the least, the old default. *fallback*
pub(super) const LOCK_DEG_MIN: f32 = 10.0;
pub(super) const LOCK_DEG_MAX: f32 = 60.0;

// The wheels and their suspension.

/// How far a wheel may hang below its unloaded position (m). *tuned*
pub const DROOP: f32 = 0.15;
/// The bump stop (m): the most a spring is pressed when its rate does not say
/// (`achse_maxforce` over it), and where a deflated air suspension rests. *tuned*
pub const BUMP: f32 = 0.24;
/// Where the tyres stand across, as a share of half `achse_maxwidth`, when `achse_minwidth`
/// does not say (with it, halfway between the two widths). *fallback*
pub(super) const TRACK_SHARE: f32 = 0.85;
/// `achse_inertia_inv` where a `[newachse]` does not say (OMSI LoadFromFile: 0.002). *OMSI*
pub const DEFAULT_INERTIA_INV: f32 = 0.002;
/// The least static load a wheel takes, as a share of an even split of the weight, however
/// far the centre of gravity lies from its axle. *tuned*
pub(super) const MIN_LOAD_SHARE: f32 = 0.2;
/// How high over the model origin's plane OMSI 2's ground query looks from (m): a wheel
/// that finds nothing within its climb looks again from there. *OMSI*
pub(super) const OMSI_PROBE_HEIGHT: f64 = 3.0;
/// The most a tyre carries in the push that takes it up a kerb, in loads its strut is
/// built for (`achse_maxforce`). *tuned*
pub(super) const STEP_LOAD_MAX: f32 = 2.0;

// The tyres.

/// Below this speed (m/s) at the contact a tyre stands and its brake holds statically.
/// *tuned*
pub(super) const STANDING: f32 = 0.15;
/// A gripping wheel starts to slip when what it has to pass on is more than this times what
/// the road takes. *tuned*
pub(super) const SLIP_MARGIN: f32 = 1.02;
/// The body follows its wheels while its speed across is under the first (m/s) and its speed
/// along over the second: holding, the tyres then take no roll. *tuned*
pub(super) const GRIPPED_ACROSS: f32 = 0.5;
pub(super) const GRIPPED_ALONG: f32 = 1.0;
/// While the tyres hold, each may pass this many times its grip for the moment it takes the
/// body to follow a quick turn of the wheel (OMSI sets the yaw rate outright). *tuned*
pub(super) const HOLD_REACH: f32 = 8.0;
/// The tyres hold again once the sideways speed at every tyre is under this (m/s). *OMSI*
pub(super) const REHOLD_SPEED: f32 = 0.1;

// The ground under a tyre.

/// Tyre samples along the envelope's length (see [`tyre_line`]). *tuned*
///
/// [`tyre_line`]: super::ground::tyre_line
pub(super) const TYRE_SAMPLES: usize = 13;
/// Samples taken: the lattice they stand on covers the envelope with one more to spare, and
/// two more beyond each end tell the edges near the ends from the grade.
pub(super) const LATTICE: usize = TYRE_SAMPLES + 5;
/// How far the tyre reaches for the ground ahead and behind, in radii. A pneumatic tyre
/// wraps round an edge and climbs it over about twice the length a rigid disc would: a
/// 15 cm kerb then costs about half the wheel load in push, not all of it, and a lowered
/// kerb of 3 cm is a nudge instead of the wall a rigid disc makes of it. *tuned*
pub(super) const ENVELOPE: f32 = 2.0;
/// How far (m) the rise between two samples may depart from the rises beside it before it
/// is an edge rather than the road's own grade or curve. *tuned*
pub(super) const EDGE: f64 = 0.01;
/// Halvings that place an edge between two samples (to a 64th of their spacing) and a face
/// the tyre cannot climb (to a 128th of the bracket). *tuned*
pub(super) const EDGE_HALVINGS: usize = 6;
pub(super) const FACE_HALVINGS: usize = 7;
/// The highest step a tyre climbs, in radii over the ground it stands on; anything higher
/// is a wall. A step of the full radius would only give way to an infinite push (and the
/// hub and 5 cm, climbed before, took a bus over a 0.52 m step from 10 km/h). Up to 0.8 the
/// steepest slope the envelope meets (2.4) is still one the push takes in full (see
/// `step`), so a climb costs exactly the lift; and it keeps the stock maps' 30 cm gaps
/// between a side road and its junction something a bus drives over. *tuned*
pub const CLIMB: f32 = 0.8;
/// Right after `place`, which knows no ground, and when nothing lower is found, a tyre takes
/// the ground up to this far (m) over its hub: a body set down a little too low is lifted.
/// *tuned*
pub const HUB_REACH: f64 = 0.05;
/// Half the width of a tyre, in radii (a 275 mm bus tyre on a 0.47 m radius). *physics*
pub(super) const HALF_WIDTH: f32 = 0.3;

// Walls and crashes.

/// How far beside the tyre (in half widths from the hub) a face is looked for. *tuned*
pub(super) const SIDE_REACH: f32 = 2.0;
/// How far (m) either side of a face's first point its direction is measured. *tuned*
pub(super) const FACE_SPAN: f32 = 0.2;
/// A face this far above the axle next to the tyre is a wall, not something far overhead.
/// *tuned*
pub const WALL_HEIGHT: f64 = 1.5;
/// A face is kept while the hub is less than this many radii from it, and while the ground
/// this far (m) behind its line is still the face and this far before it still clear.
/// *tuned*
pub(super) const WALL_KEEP: f32 = 2.5;
pub(super) const WALL_BEHIND: f64 = 0.05;
pub(super) const WALL_BEFORE: f64 = 0.03;
/// A face kept already answers a direction when its normal points back along it by more
/// than this (the cosine of 60°). *tuned*
pub(super) const WALL_FACING: f32 = 0.5;
/// A face whose normal points back along the line that found it by less than this is
/// grazed, not met: it pushes straight back along the line. *tuned*
pub(super) const WALL_GRAZE: f32 = 0.1;
/// Stiffness (N/m) and damping (N s/m) of a tyre pressed into a wall it cannot climb.
/// *tuned*
pub(super) const WALL_K: f32 = 1.5e6;
pub(super) const WALL_C: f32 = 6.0e4;
/// Share of the closing speed an obstacle gives back, and the sliding friction along it.
/// *tuned*
pub(super) const RESTITUTION: f32 = 0.2;
pub(super) const SCRAPE_FRICTION: f32 = 0.4;
/// A blow gives some of the speed back only above this closing speed (m/s); a gentle push
/// is dead. *tuned*
pub(super) const BOUNCE_SPEED: f32 = 1.0;
/// Below this closing speed (m/s) a touch is a push, not a crash. *tuned*
pub const CRASH_SPEED: f32 = 0.4;
/// How far (m) clear of an obstacle the body is put. *tuned*
pub(super) const CLEARANCE: f32 = 0.001;
/// An obstacle the body overlaps by more than this (m), plus what it and its ends (this far
/// from its middle, m) could have moved in this many frames, is one it has been standing in
/// all along (spawned in a depot hall's box): it never pushes. *tuned*
pub(super) const INSIDE_DEPTH: f64 = 0.5;
pub(super) const INSIDE_REACH: f64 = 6.0;
pub(super) const INSIDE_FRAMES: f64 = 2.0;

// The AI traffic and the kinematic model. (They stay as they are while the AI's physics is
// paused: see `crate::ai`.)

/// How fast an AI driver turns the front wheels (deg/s): a vehicle over `AI_HEAVY_MASS` (kg)
/// the slower rate. *tuned*
pub(crate) const AI_STEER_RATE_HEAVY: f32 = 25.0;
pub(crate) const AI_STEER_RATE: f32 = 40.0;
pub(crate) const AI_HEAVY_MASS: f32 = 6000.0;
/// The axle angle an AI vehicle hands its scripts is held to this (rad), and an axle closer
/// than `AI_STEER_ARM_MIN` (m) to the `[rot_pnt_long]` line is not steered. *tuned*
pub(crate) const AI_AXLE_STEER_MAX: f32 = 1.2;
pub(crate) const AI_STEER_ARM_MIN: f32 = 0.5;
/// The longest frame the kinematic model steps (s): a longer one is cut to it. *tuned*
pub(crate) const KINEMATIC_DT_MAX: f32 = 0.1;

/// The values the player's physics once gave a `.bus` file that leaves one out or gives one
/// out of range. The player's vehicle now reads its file as OMSI does (`spec`); these are kept,
/// unused, behind the `model-fallbacks` feature.
#[cfg(feature = "model-fallbacks")]
#[allow(dead_code)]
pub(super) mod fallbacks {
    /// The axle's spring (kN/m), damper (kN s/m) and the load its strut is built for (kN) when
    /// `achse_feder`, `achse_daempfer` or `achse_maxforce` is missing or zero. *fallback*
    pub const DEFAULT_SPRING: f32 = 150.0;
    pub const DEFAULT_DAMPER: f32 = 12.0;
    pub const DEFAULT_MAX_FORCE: f32 = 200.0;
}

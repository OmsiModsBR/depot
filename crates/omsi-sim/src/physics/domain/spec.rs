//! What a `.bus` file's physics values mean, in SI units: for the rigid body and the kinematic
//! model ([`PhysicsSpec`], [`AxleSpec`]) and for the AI's body ([`AiSpec`], [`AiWheelSpec`]).
//! Read from the file in `implementation::spec`, the one place a file's physics is read.

use glam::Vec3;

/// One axle's `achse_*` lines in SI units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxleSpec {
    /// Along the body (m, forward positive).
    pub long: f32,
    /// The tyre's radius (m) the physics drives with: half `achse_raddurchmesser`, as OMSI
    /// divides by it.
    pub radius: f32,
    /// The tyre's radius (m) the kinematic model's parts stand on (the AI's copies), at least
    /// 0.15 m.
    pub kinematic_radius: f32,
    /// The tyre's radius (m) the wheels are shown and turned with (`VehicleMotion::at_rest`,
    /// a trailer's wheels), at least 0.1 m.
    pub shown_radius: f32,
    /// Half of `achse_maxwidth` (m), as the wheels are shown.
    pub half_width: f32,
    /// Where the strut pushes the body across (m: half `achse_maxwidth`) and where the tyres
    /// stand (the middle of the band between the two widths), as OMSI puts them.
    pub outer: f32,
    pub track: f32,
    /// Where the kinematic model's parts (the AI's copies) put the tyres across: the middle of
    /// the band, or 0.85 of the outer half-width (at least 0.3 m) without an inner width.
    pub kinematic_track: f32,
    /// Spring (N/m), damper (N s/m) and the strut's largest force (N) of each wheel, as the
    /// file gives them: a file without them has none, as in OMSI (no spring carries nothing,
    /// no largest force lets the strut push nothing).
    pub spring: f32,
    pub damper: f32,
    pub max_force: f32,
    /// The spring (N/m) the static sag of a hub is worked out with (`VehicleType::hub_heights`,
    /// a trailer's rest): 150 kN/m without one in the file.
    pub sag_spring: f32,
    /// 1 / the wheel's rotational inertia.
    pub inertia_inv: f32,
    pub driven: bool,
}

/// The physics of a `.bus` file in SI units, as the rigid body and the kinematic model read it.
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicsSpec {
    /// Mass (kg, see [`mass_kg`]).
    pub mass_kg: f32,
    /// The kinematic model's (kg, see [`kinematic_mass_kg`]).
    pub kinematic_mass_kg: f32,
    /// Moments of inertia (kg m²) about the body's right, forward and up axes: pitch, roll and
    /// yaw, from the file's t m² with no unit guess and no minimum. `[momentofintertia]` gives
    /// them in that order - about the lateral, the longitudinal and the vertical axis, as the
    /// SDK's comment says ("um x, y und z-Achse, also Quer-, Längs- und Hochachse").
    pub inertia: Vec3,
    /// Centre of gravity (m, right, forward, up from the model origin). Its height is
    /// `[schwerpunkt]`, the lever the weight shifts by as OMSI has it, and none for a file
    /// without one: the body then leans into no acceleration.
    pub cog: Vec3,
    /// Rolling resistance (N): `[rollwiderstand]` as the file gives it, a force spread over
    /// the wheels whatever the weight, and none for a file without one (OMSI's reading). The
    /// kinematic model's as well.
    pub rolling_resistance: f32,
    /// `[rot_pnt_long]` as the file gives it.
    pub rot_pnt_long: f32,
    /// The frontmost axle (m along the body; `f32::MIN` without axles).
    pub front_long: f32,
    /// The curvature (1/m) of the full lock, `[inv_min_turnradius]` as the file gives it (a
    /// file without one does not steer, as in OMSI), and the front axle's angle (deg) on it.
    pub max_steer_deg: f32,
    pub inv_min_turn_radius: f32,
    /// The kinematic model's full lock (deg): the file's held to 10..60°, and 10° for a file
    /// without a usable `[inv_min_turnradius]`.
    pub kinematic_max_steer_deg: f32,
    /// sqrt(springs / mass) in OMSI's units (kN/m over t), what its damping of pitch and roll
    /// scales with.
    pub body_freq: f32,
    /// From the rearmost axle to the frontmost (m, at least 1): the kinematic model's.
    pub wheelbase: f32,
    pub axles: Vec<AxleSpec>,
    /// The static load on each wheel of each axle (N, see [`wheel_rest_loads`]).
    pub rest_loads: Vec<f32>,
}

/// One wheel of the AI's body on its spring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AiWheelSpec {
    pub axle: usize,
    pub side: usize,
    /// Body frame: right and forward (m).
    pub lat: f32,
    pub long: f32,
    /// Spring (N/m) and damper (Ns/m) of this wheel.
    pub k: f32,
    pub c: f32,
}

/// The `.bus` file as the AI's body (`ai::AiBody`) reads it: its own mass, springs and
/// inertia, kept apart from the rigid body's until the AI's physics is taken up again.
#[derive(Debug, Clone, PartialEq)]
pub struct AiSpec {
    pub rot_long: f32,
    pub wheelbase: f32,
    pub front_long: f32,
    pub rear_long: f32,
    /// Largest front wheel angle (deg).
    pub max_steer: f32,
    /// Over `AI_HEAVY_MASS`: the slower steering.
    pub heavy: bool,
    pub wheels: Vec<AiWheelSpec>,
    pub axle_count: usize,
    pub mass: f32,
    pub cog: f32,
    /// Moments of inertia about the lateral (pitch) and longitudinal (roll) axes (kg m²).
    pub inertia: [f32; 2],
    /// Stiffness and damping of the body in heave (N/m), pitch and roll (Nm/rad).
    pub spring: [f32; 3],
    pub damper: [f32; 3],
}

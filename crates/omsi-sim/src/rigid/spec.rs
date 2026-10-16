//! What a `.bus` file's physics values mean, decided in one place: the units of `[mass]`,
//! `[momentofintertia]` and the axles' `achse_*` lines, and what a file that leaves one out
//! gets ([`PhysicsSpec`], [`AxleSpec`], [`AiSpec`]). Every model reads its values from here -
//! the rigid body, the coupled parts, the kinematic model and the AI's body -, so a heuristic
//! changes in one place.
//!
//! The values are as each model has always read them, floors and all, and where two models
//! read the same line differently each has its own field with its own name (a coupled part's
//! mass has no floor, the AI's body has its own mass, springs and inertia). Bringing them
//! together is behaviour, for its own change.

use super::params::{
    DEFAULT_COG_HEIGHT, DEFAULT_DAMPER, DEFAULT_INERTIA_INV, DEFAULT_MAX_FORCE, DEFAULT_SPRING,
    GRAVITY, LOCK_DEG_MAX, LOCK_DEG_MIN, MIN_LOAD_SHARE, ROLLING_COEFFICIENT, TRACK_SHARE,
};
use glam::Vec3;
use omsi_vehicle::{Axle, Vehicle};

/// `[mass]` in kilograms: tonnes, or kilograms in a few files (a value of 100 or more).
fn file_mass_kg(def: &Vehicle) -> f32 {
    if def.mass < 100.0 {
        def.mass * 1000.0
    } else {
        def.mass
    }
}

/// The vehicle's mass (kg), at least 500 kg: the rigid body's, its static loads' and the
/// kinematic model's.
pub fn mass_kg(def: &Vehicle) -> f32 {
    file_mass_kg(def).max(500.0)
}

/// The mass (kg) a part coupled behind the body hands it (`CoupledPart::mass`): no floor.
pub(crate) fn coupled_mass_kg(def: &Vehicle) -> f32 {
    file_mass_kg(def).max(0.0)
}

/// The rolling resistance (N) a part coupled behind the body hands it: the file's, none when
/// it gives none.
pub(crate) fn coupled_rolling_resistance(def: &Vehicle) -> f32 {
    def.rolling_resistance.max(0.0)
}

/// One axle's `achse_*` lines in SI units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxleSpec {
    /// Along the body (m, forward positive).
    pub long: f32,
    /// The tyre's radius (m) the physics stands on, at least 0.15 m.
    pub radius: f32,
    /// The tyre's radius (m) the wheels are shown and turned with (`VehicleMotion::at_rest`,
    /// a trailer's wheels), at least 0.1 m.
    pub shown_radius: f32,
    /// Half of `achse_maxwidth` (m), as the wheels are shown.
    pub half_width: f32,
    /// Where the strut pushes the body across (m: half the outer width, at least 0.3) and where
    /// the tyres stand (the middle of the band between the two widths).
    pub outer: f32,
    pub track: f32,
    /// Spring (N/m), damper (N s/m) and the strut's largest force (N) of each wheel, with the
    /// defaults of a file without them.
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

impl AxleSpec {
    pub fn of(a: &Axle) -> AxleSpec {
        let outer = (a.max_width / 2.0).max(0.3);
        let track = if a.min_width > 0.0 && a.min_width < a.max_width { (a.max_width + a.min_width) / 4.0 } else { outer * TRACK_SHARE };
        AxleSpec {
            long: a.long,
            radius: (a.wheel_diameter / 2.0).max(0.15),
            shown_radius: (a.wheel_diameter / 2.0).max(0.1),
            half_width: a.max_width / 2.0,
            outer,
            track,
            spring: if a.spring > 0.0 { a.spring * 1000.0 } else { DEFAULT_SPRING * 1000.0 },
            damper: if a.damper > 0.0 { a.damper * 1000.0 } else { DEFAULT_DAMPER * 1000.0 },
            max_force: if a.max_force > 0.0 { a.max_force * 1000.0 } else { DEFAULT_MAX_FORCE * 1000.0 },
            sag_spring: if a.spring > 0.0 { a.spring * 1000.0 } else { 150_000.0 },
            inertia_inv: if a.inertia_inv > 0.0 { a.inertia_inv } else { DEFAULT_INERTIA_INV },
            driven: a.driven,
        }
    }
}

/// The physics of a `.bus` file in SI units, as the rigid body and the kinematic model read it.
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicsSpec {
    /// Mass (kg, see [`mass_kg`]).
    pub mass_kg: f32,
    /// Moments of inertia (kg m²) about the body's right, forward and up axes: pitch, roll and
    /// yaw. `[momentofintertia]` goes to ODE as I11, I22, I33 of OMSI's y-up body frame: about
    /// the lateral axis (pitch), the vertical one (yaw) and the longitudinal one (roll) - roll
    /// takes the third value, yaw the second, whatever the SDK's comment says. Read in the
    /// comment's order the SD202 rolled on 80 t m² instead of 300, twice as fast, and every
    /// uneven patch rocked it like a boat.
    pub inertia: Vec3,
    /// Centre of gravity (m, right, forward, up from the model origin).
    pub cog: Vec3,
    /// Rolling resistance (N): the file's, or a bus tyre's share of the weight.
    pub rolling_resistance: f32,
    /// `[rollwiderstand]` as the file gives it (N), the kinematic model's.
    pub file_rolling_resistance: f32,
    /// `[rot_pnt_long]` as the file gives it.
    pub rot_pnt_long: f32,
    /// The frontmost axle (m along the body; `f32::MIN` without axles).
    pub front_long: f32,
    /// The full lock (deg) and the curvature (1/m) it turns on; a file without a usable
    /// `[inv_min_turnradius]` steers as far as the old default lock.
    pub max_steer_deg: f32,
    pub inv_min_turn_radius: f32,
    /// sqrt(springs / mass) in OMSI's units (kN/m over t), what its damping of pitch and roll
    /// scales with.
    pub body_freq: f32,
    /// From the rearmost axle to the frontmost (m, at least 1): the kinematic model's.
    pub wheelbase: f32,
    pub axles: Vec<AxleSpec>,
    /// The static load on each wheel of each axle (N, see [`wheel_rest_loads`]).
    pub rest_loads: Vec<f32>,
}

impl PhysicsSpec {
    pub fn from(def: &Vehicle) -> PhysicsSpec {
        let mass = mass_kg(def);
        let moi = def.moment_of_inertia;
        let scale = if moi[0] < 5000.0 { 1000.0 } else { 1.0 };
        let inertia = Vec3::new((moi[0] * scale).max(100.0), (moi[2] * scale).max(100.0), (moi[1] * scale).max(100.0));
        let cog_xy = def.cog.map(|c| (c[0], c[1])).unwrap_or((0.0, 0.0));
        let cog_z = if def.cog_height > 0.0 { def.cog_height } else { def.cog.map(|c| c[2]).filter(|z| *z > 0.0).unwrap_or(DEFAULT_COG_HEIGHT) };
        let front_long = def.axles.iter().map(|a| a.long).fold(f32::MIN, f32::max);
        let rear_long = def.axles.iter().map(|a| a.long).fold(f32::MAX, f32::min);
        let s = (front_long - def.rot_pnt_long).abs().max(1.0);
        let max_steer_deg = (def.inv_min_turn_radius * s).atan().to_degrees().clamp(LOCK_DEG_MIN, LOCK_DEG_MAX);
        let inv_min_turn_radius = if def.inv_min_turn_radius > 0.0 { def.inv_min_turn_radius } else { max_steer_deg.to_radians().tan() / s };
        let springs: f32 = def.axles.iter().map(|a| 2.0 * if a.spring > 0.0 { a.spring } else { DEFAULT_SPRING }).sum();
        PhysicsSpec {
            mass_kg: mass,
            inertia,
            cog: Vec3::new(cog_xy.0, cog_xy.1, cog_z),
            rolling_resistance: if def.rolling_resistance > 0.0 { def.rolling_resistance } else { ROLLING_COEFFICIENT * mass * GRAVITY },
            file_rolling_resistance: def.rolling_resistance,
            rot_pnt_long: def.rot_pnt_long,
            front_long,
            max_steer_deg,
            inv_min_turn_radius,
            body_freq: (springs / (mass / 1000.0)).max(0.0).sqrt(),
            wheelbase: (front_long - rear_long).abs().max(1.0),
            axles: def.axles.iter().map(AxleSpec::of).collect(),
            rest_loads: wheel_rest_loads(def),
        }
    }
}

/// The static load on each wheel of each axle (N): the weight shared so that the axle loads
/// balance the centre of gravity (a least-squares split when there are more than two axles).
pub fn wheel_rest_loads(def: &Vehicle) -> Vec<f32> {
    let mass = mass_kg(def);
    let cog_y = def.cog.map(|c| c[1]).unwrap_or(0.0);
    let g = GRAVITY;
    let n = (def.axles.len() * 2).max(1) as f32;
    let sy: f32 = def.axles.iter().map(|a| 2.0 * a.long).sum();
    let syy: f32 = def.axles.iter().map(|a| 2.0 * a.long * a.long).sum();
    let det = n * syy - sy * sy;
    let (a, b) = if det.abs() > 1e-6 {
        let rhs1 = mass * g;
        let rhs2 = mass * g * cog_y;
        ((rhs1 * syy - sy * rhs2) / det, (n * rhs2 - sy * rhs1) / det)
    } else {
        (mass * g / n, 0.0)
    };
    def.axles.iter().map(|x| (a + b * x.long).max(mass * g / n * MIN_LOAD_SHARE)).collect()
}

/// The least-squares split of a load `total` with its centre at `centre` over points at
/// `(long, count)`: the load per point grows linearly along the vehicle, so that the shares
/// balance the load and its moment (exactly, with two places to rest on).
pub(super) fn split(points: &[(f32, f32)], total: f32, centre: f32) -> (f32, f32) {
    let n: f32 = points.iter().map(|p| p.1).sum::<f32>().max(1.0);
    let sy: f32 = points.iter().map(|p| p.1 * p.0).sum();
    let syy: f32 = points.iter().map(|p| p.1 * p.0 * p.0).sum();
    let det = n * syy - sy * sy;
    if det.abs() > 1e-6 {
        ((total * syy - sy * total * centre) / det, (n * total * centre - sy * total) / det)
    } else {
        (total / n, 0.0)
    }
}

/// [`wheel_rest_loads`] for a part that also rests on its front joint at `support_long` (an
/// articulated bus's rear section, a semitrailer): the static load on each wheel of each axle
/// (N), and what the joint carries (N), which the part in front takes on its axles (see
/// [`super::point_load_shares`]).
pub fn wheel_rest_loads_supported(def: &Vehicle, support_long: f32) -> (Vec<f32>, f32) {
    let mass = mass_kg(def);
    let weight = mass * 9.81;
    let cog_y = def.cog.map(|c| c[1]).unwrap_or(0.0);
    let mut points: Vec<(f32, f32)> = def.axles.iter().map(|a| (a.long, 2.0)).collect();
    points.push((support_long, 1.0));
    let (a, b) = split(&points, weight, cog_y);
    let n = points.iter().map(|p| p.1).sum::<f32>();
    let joint = (a + b * support_long).clamp(0.0, weight);
    (def.axles.iter().map(|x| (a + b * x.long).max(weight / n * 0.2)).collect(), joint)
}

/// The line an AI vehicle turns about (`[rot_pnt_long]`, normally the rear axle) and the
/// distance from it to the front axle: (rotation point, wheelbase), both in metres along the
/// body.
pub fn ai_rotation_point(def: &Vehicle) -> (f32, f32) {
    let front = def.axles.iter().map(|a| a.long).fold(f32::MIN, f32::max);
    let rear = def.axles.iter().map(|a| a.long).fold(f32::MAX, f32::min);
    if def.axles.is_empty() {
        return (-1.3, 2.6);
    }
    // a rotation point at (or in front of) the front axle is a file without one
    let rot = if def.axles.len() >= 2 && def.rot_pnt_long < front - 0.5 { def.rot_pnt_long } else { rear };
    (rot, (front - rot).max(0.8))
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

/// The `.bus` file as the AI's body (`ai_motion::AiBody`) reads it: its own mass, springs and
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

impl AiSpec {
    pub fn from(def: &Vehicle) -> AiSpec {
        let (rot_long, wheelbase) = ai_rotation_point(def);
        let front_long = def.axles.iter().map(|a| a.long).fold(f32::MIN, f32::max);
        let rear_long = def.axles.iter().map(|a| a.long).fold(f32::MAX, f32::min);
        let (front_long, rear_long) = if def.axles.len() >= 2 { (front_long, rear_long) } else { (rot_long + wheelbase, rot_long) };
        // `[mass]` is in tonnes (a few files give kilograms), `[momentofintertia]` in the
        // same unit times m²: 1 t and 1.49 for a Golf, 10.9 t and 300 for an SD200
        let tonnes = def.mass < 100.0;
        let mass = (if tonnes { def.mass * 1000.0 } else { def.mass }).max(300.0);
        let unit = if tonnes { 1000.0 } else { 1.0 };
        let heavy = mass > super::params::AI_HEAVY_MASS;
        // inv_min_turnradius = tan(max angle) / wheelbase
        let max_steer = if def.inv_min_turn_radius > 0.0 { (def.inv_min_turn_radius * wheelbase).atan().to_degrees().clamp(15.0, 55.0) } else { 35.0 };
        let mut wheels = Vec::new();
        for (i, a) in def.axles.iter().enumerate() {
            let half = (a.max_width * 0.5).max(0.4);
            for (side, lat) in [(0, -half), (1, half)] {
                // `achse_feder` / `achse_daempfer`: kN/m and kNs/m per side
                wheels.push(AiWheelSpec { axle: i, side, lat, long: a.long, k: a.spring.max(0.0) * 1000.0, c: a.damper.max(0.0) * 1000.0 });
            }
        }
        let n = wheels.len().max(1) as f32;
        if wheels.iter().any(|w| w.k <= 0.0) {
            // no springs in the file: 1.5 Hz with a third of critical damping
            let k_total = mass * (std::f32::consts::TAU * 1.5).powi(2);
            let c_total = 2.0 * 0.35 * (k_total * mass).sqrt();
            for w in wheels.iter_mut() {
                w.k = k_total / n;
                w.c = c_total / n;
            }
        }
        let mut spring = [0.0f32; 3];
        let mut damper = [0.0f32; 3];
        for w in &wheels {
            spring[0] += w.k;
            spring[1] += w.k * w.long * w.long;
            spring[2] += w.k * w.lat * w.lat;
            damper[0] += w.c;
            damper[1] += w.c * w.long * w.long;
            damper[2] += w.c * w.lat * w.lat;
        }
        let half_track = wheels.iter().map(|w| w.lat.abs()).fold(0.5, f32::max);
        // (pitch and roll: the first and the third value, as OMSI reads them - see
        // `PhysicsSpec::inertia`)
        let mut inertia = [def.moment_of_inertia[0] * unit, def.moment_of_inertia[2] * unit];
        if inertia[0] <= 0.0 {
            inertia[0] = mass * (front_long * front_long + rear_long * rear_long) * 0.5;
        }
        if inertia[1] <= 0.0 {
            inertia[1] = mass * half_track * half_track * 1.2;
        }
        // keep every mode below ~6 Hz: the springs are integrated at a few hundred Hz at most
        for (i, j) in [(1usize, 0usize), (2, 1)] {
            let max_k = inertia[j] * 38.0f32.powi(2);
            if spring[i] > max_k {
                inertia[j] = spring[i] / 38.0f32.powi(2);
            }
        }
        // at least 0.8 of critical damping in every mode: a driven car settles after a
        // bump or a stop instead of swinging on (the files' dampers are tuned for the
        // player's physics, which damps through the tyres as well)
        let heave_c = 2.0 * 0.8 * (spring[0] * mass).sqrt();
        damper[0] = damper[0].max(heave_c);
        for (i, j) in [(1usize, 0usize), (2, 1)] {
            damper[i] = damper[i].max(2.0 * 0.8 * (spring[i] * inertia[j]).sqrt());
        }
        AiSpec {
            rot_long,
            wheelbase,
            front_long,
            rear_long,
            max_steer,
            heavy,
            wheels,
            axle_count: def.axles.len().max(1),
            mass,
            cog: if def.cog_height > 0.0 { def.cog_height } else { 0.6 },
            inertia,
            spring,
            damper,
        }
    }
}

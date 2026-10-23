//! What a `.bus` file's physics values mean, decided in one place: the units of `[mass]`,
//! `[momentofintertia]` and the axles' `achse_*` lines, and what a file that leaves one out
//! gets ([`PhysicsSpec`], [`AxleSpec`], [`AiSpec`]). Every model reads its values from here -
//! the rigid body, the coupled parts, the kinematic model and the AI's body -, so a heuristic
//! changes in one place.
//!
//! The player's vehicle reads its file as OMSI does: in tonnes, kN and metres, with no guess
//! at units, no minimum and nothing invented for a value the file leaves out (OMSI's own
//! defaults aside: 1 t, 100 t m² about each axis, an `achse_inertia_inv` of 0.002). A model
//! configured wrongly behaves wrongly, as in OMSI. The kinematic model, which drives the AI's
//! copies, and the AI's body keep their old readings, floors and all, in fields and functions
//! of their own (`kinematic_*`, [`AiSpec`]) until the AI's physics is taken up again; the
//! values the player's vehicle no longer uses are kept behind the `model-fallbacks` feature
//! (`params::fallbacks`) until the physics is validated in game.

use crate::physics::domain::params::{
    AI_HEAVY_MASS, BUMP, DEFAULT_INERTIA_INV, GRAVITY, LOCK_DEG_MAX, LOCK_DEG_MIN, MIN_LOAD_SHARE,
    TRACK_SHARE,
};
use crate::physics::domain::{AiSpec, AiWheelSpec, AxleSpec, PhysicsSpec};
use glam::Vec3;
use omsi_vehicle::{Axle, Vehicle};

/// `[mass]` in kilograms, as OMSI reads it: in tonnes, whatever the number (a file that gives
/// kilograms weighs a thousand times as much), and 1 t for a file without one. The rigid
/// body's, its static loads' and a coupled part's.
pub fn mass_kg(def: &Vehicle) -> f32 {
    if def.mass_missing {
        1000.0
    } else {
        def.mass * 1000.0
    }
}

/// `[mass]` in kilograms as the kinematic model (the AI's copies) reads it: tonnes, or
/// kilograms in a few files (a value of 100 or more), at least 500 kg. Kept apart from
/// [`mass_kg`] until the AI's physics is taken up again.
pub fn kinematic_mass_kg(def: &Vehicle) -> f32 {
    let kg = if def.mass < 100.0 {
        def.mass * 1000.0
    } else {
        def.mass
    };
    kg.max(500.0)
}

/// The rolling resistance (N) a part coupled behind the body hands it: the file's, none when
/// it gives none.
pub(crate) fn coupled_rolling_resistance(def: &Vehicle) -> f32 {
    def.rolling_resistance.max(0.0)
}

impl AxleSpec {
    pub fn of(a: &Axle) -> AxleSpec {
        let kinematic_outer = (a.max_width / 2.0).max(0.3);
        let kinematic_track = if a.min_width > 0.0 && a.min_width < a.max_width {
            (a.max_width + a.min_width) / 4.0
        } else {
            kinematic_outer * TRACK_SHARE
        };
        AxleSpec {
            long: a.long,
            radius: a.wheel_diameter / 2.0,
            kinematic_radius: (a.wheel_diameter / 2.0).max(0.15),
            shown_radius: (a.wheel_diameter / 2.0).max(0.1),
            half_width: a.max_width / 2.0,
            outer: a.max_width / 2.0,
            track: (a.max_width + a.min_width) / 4.0,
            kinematic_track,
            spring: a.spring * 1000.0,
            damper: a.damper * 1000.0,
            max_force: a.max_force * 1000.0,
            sag_spring: if a.spring > 0.0 {
                a.spring * 1000.0
            } else {
                150_000.0
            },
            inertia_inv: if a.inertia_inv > 0.0 {
                a.inertia_inv
            } else {
                DEFAULT_INERTIA_INV
            },
            driven: a.driven,
        }
    }
}

impl PhysicsSpec {
    pub fn from(def: &Vehicle) -> PhysicsSpec {
        let mass = mass_kg(def);
        // in t m², as OMSI reads it whatever the numbers, and 100 t m² about each axis for a
        // file without the section (OMSI's default; this parser leaves it at zero)
        let moi = if def.moment_of_inertia == [0.0; 3] {
            [100.0; 3]
        } else {
            def.moment_of_inertia
        };
        // the file's first, second and third value are pitch, yaw and roll (see `inertia`)
        let inertia = Vec3::new(moi[0], moi[2], moi[1]) * 1000.0;
        let cog_xy = def.cog.map(|c| (c[0], c[1])).unwrap_or((0.0, 0.0));
        let front_long = def.axles.iter().map(|a| a.long).fold(f32::MIN, f32::max);
        let rear_long = def.axles.iter().map(|a| a.long).fold(f32::MAX, f32::min);
        // the kinematic model's lock: the file's, held to 10..60° (with the arm from the
        // turning line at least 1 m), and the least for a file without a usable one
        let s = (front_long - def.rot_pnt_long).abs().max(1.0);
        let kinematic_max_steer_deg = (def.inv_min_turn_radius * s)
            .atan()
            .to_degrees()
            .clamp(LOCK_DEG_MIN, LOCK_DEG_MAX);
        let springs: f32 = def.axles.iter().map(|a| 2.0 * a.spring).sum();
        PhysicsSpec {
            mass_kg: mass,
            kinematic_mass_kg: kinematic_mass_kg(def),
            inertia,
            cog: Vec3::new(cog_xy.0, cog_xy.1, def.cog_height),
            rolling_resistance: def.rolling_resistance,
            rot_pnt_long: def.rot_pnt_long,
            front_long,
            max_steer_deg: (def.inv_min_turn_radius * (front_long - def.rot_pnt_long).abs())
                .atan()
                .to_degrees(),
            kinematic_max_steer_deg,
            inv_min_turn_radius: def.inv_min_turn_radius,
            body_freq: (springs / (mass / 1000.0)).max(0.0).sqrt(),
            wheelbase: (front_long - rear_long).abs().max(1.0),
            axles: def.axles.iter().map(AxleSpec::of).collect(),
            rest_loads: wheel_rest_loads(def),
        }
    }
}

/// The static load on each wheel of each axle (N): the weight shared so that the axle loads
/// balance the centre of gravity (a least-squares split when there are more than two axles).
/// A wheel carries no less than nothing - an OMSI strut only pushes -, however far the centre
/// of gravity lies from its axle.
pub fn wheel_rest_loads(def: &Vehicle) -> Vec<f32> {
    rest_loads(def, mass_kg(def), 0.0)
}

/// [`wheel_rest_loads`] as the kinematic model (the AI's copies and their parts) reads them:
/// with its mass ([`kinematic_mass_kg`]), each wheel taking at least a share of an even split
/// (`MIN_LOAD_SHARE`).
pub fn kinematic_wheel_rest_loads(def: &Vehicle) -> Vec<f32> {
    rest_loads(def, kinematic_mass_kg(def), MIN_LOAD_SHARE)
}

/// How far each axle's spring sits down under the vehicle's weight and how far its tyre
/// reaches below the hub (m), as the kinematic model (the AI's copies) stands on them: with
/// its mass and the springs a file without them once had (`AxleSpec::sag_spring`), from the
/// unloaded hub heights `hubs` (`VehicleType::hub_heights`).
pub fn kinematic_rest_sag(def: &Vehicle, hubs: &[Option<f32>]) -> Vec<(f32, f32)> {
    let loads = kinematic_wheel_rest_loads(def);
    def.axles
        .iter()
        .enumerate()
        .map(|(a, axle)| {
            let axle = AxleSpec::of(axle);
            let r = axle.kinematic_radius;
            let z = hubs.get(a).copied().flatten().unwrap_or(r);
            let sag = loads.get(a).copied().unwrap_or(0.0) / axle.sag_spring;
            (sag.min(BUMP), r - z)
        })
        .collect()
}

fn rest_loads(def: &Vehicle, mass: f32, least_share: f32) -> Vec<f32> {
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
    def.axles
        .iter()
        .map(|x| (a + b * x.long).max(mass * g / n * least_share))
        .collect()
}

/// The least-squares split of a load `total` with its centre at `centre` over points at
/// `(long, count)`: the load per point grows linearly along the vehicle, so that the shares
/// balance the load and its moment (exactly, with two places to rest on).
pub fn split(points: &[(f32, f32)], total: f32, centre: f32) -> (f32, f32) {
    let n: f32 = points.iter().map(|p| p.1).sum::<f32>().max(1.0);
    let sy: f32 = points.iter().map(|p| p.1 * p.0).sum();
    let syy: f32 = points.iter().map(|p| p.1 * p.0 * p.0).sum();
    let det = n * syy - sy * sy;
    if det.abs() > 1e-6 {
        (
            (total * syy - sy * total * centre) / det,
            (n * total * centre - sy * total) / det,
        )
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
    let joint = (a + b * support_long).clamp(0.0, weight);
    (
        def.axles
            .iter()
            .map(|x| (a + b * x.long).max(0.0))
            .collect(),
        joint,
    )
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
    let rot = if def.axles.len() >= 2 && def.rot_pnt_long < front - 0.5 {
        def.rot_pnt_long
    } else {
        rear
    };
    (rot, (front - rot).max(0.8))
}

impl AiSpec {
    pub fn from(def: &Vehicle) -> AiSpec {
        let (rot_long, wheelbase) = ai_rotation_point(def);
        let front_long = def.axles.iter().map(|a| a.long).fold(f32::MIN, f32::max);
        let rear_long = def.axles.iter().map(|a| a.long).fold(f32::MAX, f32::min);
        let (front_long, rear_long) = if def.axles.len() >= 2 {
            (front_long, rear_long)
        } else {
            (rot_long + wheelbase, rot_long)
        };
        // `[mass]` is in tonnes (a few files give kilograms), `[momentofintertia]` in the
        // same unit times m²: 1 t and 1.49 for a Golf, 10.9 t and 300 for an SD200
        let tonnes = def.mass < 100.0;
        let mass = (if tonnes { def.mass * 1000.0 } else { def.mass }).max(300.0);
        let unit = if tonnes { 1000.0 } else { 1.0 };
        let heavy = mass > AI_HEAVY_MASS;
        // inv_min_turnradius = tan(max angle) / wheelbase
        let max_steer = if def.inv_min_turn_radius > 0.0 {
            (def.inv_min_turn_radius * wheelbase)
                .atan()
                .to_degrees()
                .clamp(15.0, 55.0)
        } else {
            35.0
        };
        let mut wheels = Vec::new();
        for (i, a) in def.axles.iter().enumerate() {
            let half = (a.max_width * 0.5).max(0.4);
            for (side, lat) in [(0, -half), (1, half)] {
                // `achse_feder` / `achse_daempfer`: kN/m and kNs/m per side
                wheels.push(AiWheelSpec {
                    axle: i,
                    side,
                    lat,
                    long: a.long,
                    k: a.spring.max(0.0) * 1000.0,
                    c: a.damper.max(0.0) * 1000.0,
                });
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
        // (pitch and roll: the first and the third value, as the player's body reads them - see
        // `PhysicsSpec::inertia`)
        let mut inertia = [
            def.moment_of_inertia[0] * unit,
            def.moment_of_inertia[2] * unit,
        ];
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
            cog: if def.cog_height > 0.0 {
                def.cog_height
            } else {
                0.6
            },
            inertia,
            spring,
            damper,
        }
    }
}

//! The conformance matrix of the player's vehicle, run for an engine: the five conformance
//! vehicles (`support::vehicles`) put through the checks and compared with the closed forms
//! of `docs/physics/` (`support::closed_form`). `physics_model_classic.rs` and
//! `physics_model_depot.rs` hold the tests, each naming the equations it checks.

use super::closed_form::{compression, section_residual, static_share};
use super::ground::flat;
use glam::{DVec3, Vec3};
use omsi_sim::physics::{
    place_train, step_train, support_rest_loads, BodyInputs, Engine, Joint, RigidBody,
};
use omsi_vehicle::Vehicle;

pub const G: f64 = 9.81;
pub const DT: f32 = 1.0 / 60.0;

/// The sections put down at the origin facing +y, joined in order, and held still by their
/// brakes for ten seconds on the flat so that the suspension has settled.
pub fn settled(defs: &[Vehicle], engine: Engine) -> Vec<RigidBody> {
    let (mut bodies, joints) = build(defs, engine);
    let brakes: Vec<Vec<f32>> = bodies
        .iter()
        .map(|b| vec![5000.0; b.wheels().len()])
        .collect();
    for _ in 0..600 {
        step(&mut bodies, &joints, &brakes, 0.0, 0.0);
    }
    bodies
}

/// The sections put down at the origin facing +y and joined in order.
fn build(defs: &[Vehicle], engine: Engine) -> (Vec<RigidBody>, Vec<Joint>) {
    let mut bodies: Vec<RigidBody> = defs
        .iter()
        .map(|d| RigidBody::for_engine(d, &[], engine))
        .collect();
    let joints: Vec<Joint> = (1..defs.len())
        .map(|i| {
            let back = Vec3::from(defs[i - 1].coupling_back.as_ref().unwrap().pos);
            let at = Vec3::from(defs[i].coupling_front.as_ref().unwrap().pos);
            Joint::new(i - 1, i, &defs[i], back, at)
        })
        .collect();
    let refs: Vec<&Vehicle> = defs.iter().collect();
    support_rest_loads(&mut bodies, &joints, &refs);
    place_train(&mut bodies, &joints, DVec3::ZERO, &vec![0.0; defs.len()]);
    (bodies, joints)
}

/// One frame of the train: the brake on every wheel of every section, the drive torque of
/// the driven wheels and the steering (1 = full lock).
fn step(bodies: &mut [RigidBody], joints: &[Joint], brakes: &[Vec<f32>], torque: f32, steer: f32) {
    let inputs: Vec<BodyInputs> = brakes
        .iter()
        .enumerate()
        .map(|(i, b)| BodyInputs {
            brake: b,
            steer: (i == 0).then_some(steer),
            air: i == 0,
        })
        .collect();
    step_train(bodies, joints, DT, torque, &inputs, &flat);
}

/// Rolling at `v` m/s along +y, the wheels turning with the road.
fn launch(bodies: &mut [RigidBody], v: f32) {
    for b in bodies {
        b.set_velocity(b.orientation().mul_vec3(Vec3::Y) * v);
        for w in b.wheels_mut() {
            w.set_spin(v / w.radius());
        }
    }
}

/// The axles of a section as `(y, k)`: `k` of the whole axle in N/m (two wheels).
fn axles(def: &Vehicle) -> Vec<(f64, f64)> {
    def.axles
        .iter()
        .map(|a| (a.long as f64, 2.0 * a.spring as f64 * 1000.0))
        .collect()
}

/// The `y` the engine shares the weight about: the model's origin (Classic, `K-1`) or `[cog]`
/// (Depot, `D-1`).
fn reference(def: &Vehicle, engine: Engine) -> f64 {
    match engine {
        Engine::Classic => 0.0,
        Engine::Depot => def.cog.map_or(0.0, |c| c[1] as f64),
    }
}

/// C-3, K-1 / D-1: a solo vehicle at rest sags as the share of its weight about the engine's
/// reference says, axle by axle (`z - theta y`).
pub fn static_sag_solo(defs: Vec<Vehicle>, engine: Engine) {
    let bodies = settled(&defs, engine);
    let (def, body) = (&defs[0], &bodies[0]);
    let weight = body.mass() as f64 * G;
    let share = static_share(&axles(def), weight, reference(def, engine));
    for w in body.wheels() {
        let want = compression(share, w.attach().y as f64);
        assert!(
            (w.compression() as f64 - want).abs() < 0.002,
            "axle at {} m: compression {} m, want {want} m",
            w.attach().y,
            w.compression()
        );
    }
}

/// C-3, C-9, K-1 / D-1: a train at rest. Each section is in equilibrium with the weight shared
/// about the engine's reference: its wheels and the joints it hangs on carry its weight and what
/// it holds of the follower, in force and in moment, taking the joints' loads from the rear.
/// The struts turn the body about the reference (K-1 origin, D-1 `[cog]`); a joint's load
/// turns it about `[cog]`.
pub fn static_balance_train(defs: Vec<Vehicle>, engine: Engine) {
    let bodies = settled(&defs, engine);
    let mut up_back = 0.0_f64; // what the follower pushes down on this section
    for (def, body) in defs.iter().zip(&bodies).rev() {
        let weight = body.mass() as f64 * G;
        let wheels: f64 = body.wheels().iter().map(|w| w.load() as f64).sum();
        let moment: f64 = body
            .wheels()
            .iter()
            .map(|w| w.load() as f64 * w.attach().y as f64)
            .sum();
        let back_y = def.coupling_back.as_ref().map_or(0.0, |c| c.pos[1] as f64);
        let cog_y = def.cog.map_or(0.0, |c| c[1] as f64);
        let front_y = def.coupling_front.as_ref().map_or(0.0, |c| c.pos[1] as f64);
        // the front joint's load is what is left of the weight once the wheels and the
        // follower have taken theirs
        let up_front = if def.coupling_front.is_some() {
            weight + up_back - wheels
        } else {
            0.0
        };
        let (force, torque) = section_residual(
            wheels,
            moment,
            weight,
            reference(def, engine),
            cog_y,
            up_front,
            front_y,
            up_back,
            back_y,
        );
        // the struts push along the body's up axis, which the pitch tilts: what the tyres'
        // horizontal forces take about `[cog]` is a moment of `weight * cog height * pitch`
        let (_, pitch, _) = body.orientation().to_euler(glam::EulerRot::ZXY);
        let tilt = weight * def.cog_height as f64 * pitch.abs() as f64;
        let scale = weight * 5.0;
        assert!(
            force.abs() < 1e-3 * weight && torque.abs() < 2e-3 * scale + 1.5 * tilt,
            "section on {} axles: residual {force} N and {torque} N m",
            def.axles.len()
        );
        up_back = up_front;
    }
}

/// C-4, C-5, C-8: braking below the grip. Every wheel brakes with 0.35 of its static load, so
/// the train slows at `0.35 g` plus the rolling resistance, and from `v` it stops in
/// `v^2 / 2a`.
pub fn braking_distance(defs: Vec<Vehicle>, engine: Engine) {
    let (mut bodies, joints) = build(&defs, engine);
    let v = 5.0_f32;
    launch(&mut bodies, v);
    let brakes: Vec<Vec<f32>> = bodies
        .iter()
        .map(|b| b.wheels().iter().map(|w| 0.35 * w.rest_load()).collect())
        .collect();
    let mass: f64 = bodies.iter().map(|b| b.mass() as f64).sum();
    let rolling: f64 = bodies.iter().map(|b| b.rolling_resistance() as f64).sum();
    let braked: f64 = brakes.iter().flatten().map(|&f| f as f64).sum();
    let a = (braked + rolling) / mass;
    let start = bodies[0].position().y;
    for _ in 0..600 {
        step(&mut bodies, &joints, &brakes, 0.0, 0.0);
        if bodies[0].forward_speed() < 0.02 {
            break;
        }
    }
    let stopped = bodies[0].position().y - start;
    let want = (v as f64).powi(2) / (2.0 * a);
    assert!(
        (stopped - want).abs() < 0.05 * want,
        "stopped in {stopped} m, want {want} m (decelerating at {a} m/s2)"
    );
}

/// C-6, C-7: a steady turn at full lock and walking pace. The wheels point at the turning
/// line, `delta_i = atan((y_i - y_turn) kappa)` with `y_turn = [cog] y + [rot_pnt_long]`, and
/// the body turns on it, `omega_z = v kappa`.
pub fn steady_turn(defs: Vec<Vehicle>, engine: Engine) {
    let def = defs[0].clone();
    let (mut bodies, joints) = build(&defs, engine);
    let v = 3.0_f32;
    launch(&mut bodies, v);
    let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
    let radius = bodies
        .iter()
        .flat_map(|b| b.wheels())
        .find(|w| w.driven())
        .map_or(0.5, |w| w.radius());
    for _ in 0..600 {
        let b = &bodies[0];
        let mass: f32 = bodies.iter().map(|b| b.mass()).sum();
        let rolling: f32 = bodies.iter().map(|b| b.rolling_resistance()).sum();
        let torque = ((v - b.forward_speed()) * mass * 2.0 + rolling) * radius;
        step(&mut bodies, &joints, &brakes, torque, 1.0);
    }
    let b = &bodies[0];
    let kappa = def.inv_min_turn_radius as f64;
    let y_turn = def.cog.map_or(0.0, |c| c[1] as f64) + def.rot_pnt_long as f64;
    assert!(
        b.holding(),
        "the tyres must hold at {v} m/s on a bend of {} m",
        1.0 / kappa
    );
    for w in b.wheels() {
        // each tyre points square to the line from the turning centre (Ackermann); the
        // tyres stand halfway between `achse_minwidth` and `achse_maxwidth`
        let axle = def
            .axles
            .iter()
            .find(|a| (a.long - w.attach().y).abs() < 1e-4)
            .unwrap();
        let x = w.attach().x.signum() as f64 * (axle.max_width + axle.min_width) as f64 / 4.0;
        let want = ((w.attach().y as f64 - y_turn) * kappa / (1.0 - x * kappa)).atan();
        assert!(
            (w.steer() as f64 - want).abs() < 0.2_f64.to_radians(),
            "tyre at ({x}, {}) m steers {} rad, want {want} rad",
            w.attach().y,
            w.steer()
        );
    }
    let want = b.forward_speed() as f64 * kappa;
    assert!(
        (b.omega().z.abs() as f64 - want).abs() < 0.03 * want,
        "yaw rate {} rad/s, want {want} rad/s",
        b.omega().z
    );
}

/// C-4, C-5, C-8: pulling away below the grip. The drive force is `0.15` of the train's
/// weight, so it speeds up at `0.15 g` less the rolling resistance, and after `t` seconds
/// from rest it goes at `a t`.
pub fn pull_away(defs: Vec<Vehicle>, engine: Engine) {
    let (mut bodies, joints) = build(&defs, engine);
    let radius = bodies
        .iter()
        .flat_map(|b| b.wheels())
        .find(|w| w.driven())
        .map(|w| w.radius() as f64)
        .unwrap();
    let mass: f64 = bodies.iter().map(|b| b.mass() as f64).sum();
    let rolling: f64 = bodies.iter().map(|b| b.rolling_resistance() as f64).sum();
    let force = 0.15 * mass * G;
    let torque = (force * radius) as f32;
    let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
    let frames = 180;
    for _ in 0..frames {
        step(&mut bodies, &joints, &brakes, torque, 0.0);
    }
    let a = (force - rolling) / mass;
    let want = a * frames as f64 * DT as f64;
    let got = bodies[0].forward_speed() as f64;
    assert!(
        (got - want).abs() < 0.05 * want,
        "after 3 s at {got} m/s, want {want} m/s (accelerating at {a} m/s2)"
    );
}

/// C-9: the steady articulation angles at half lock and walking pace. A section behind a
/// joint turns about its own turning line; its ball moves as the lead's does, so the angle
/// `alpha` satisfies `R sin(alpha) + D cos(alpha) = d` and the next section turns on
/// `R cos(alpha) - D sin(alpha)`, where `R` is the radius the lead turns on, `D` the distance
/// of its ball from its turning line and `d` the distance of the follower's ball from its own.
/// Sections with one axle only: a section with two holds its tyres on a line the joint cannot
/// reach (the last row of the open items in `classic-model.md`).
pub fn articulation_angles(defs: Vec<Vehicle>, engine: Engine) {
    let (mut bodies, joints) = build(&defs, engine);
    let (v, steer) = (3.0_f32, 0.5_f32);
    launch(&mut bodies, v);
    let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
    let radius = bodies
        .iter()
        .flat_map(|b| b.wheels())
        .find(|w| w.driven())
        .map(|w| w.radius())
        .unwrap();
    let mass: f32 = bodies.iter().map(|b| b.mass()).sum();
    let rolling: f32 = bodies.iter().map(|b| b.rolling_resistance()).sum();
    for _ in 0..1500 {
        let torque = ((v - bodies[0].forward_speed()) * mass * 2.0 + rolling) * radius;
        step(&mut bodies, &joints, &brakes, torque, steer);
    }
    let line = |d: &Vehicle| d.cog.unwrap()[1] as f64 + d.rot_pnt_long as f64;
    let mut r = 1.0 / (steer as f64 * defs[0].inv_min_turn_radius as f64);
    for (k, joint) in joints.iter().enumerate() {
        let (lead, follower) = (&defs[k], &defs[k + 1]);
        let ball = lead.coupling_back.as_ref().unwrap().pos[1] as f64 - line(lead);
        let d = follower.coupling_front.as_ref().unwrap().pos[1] as f64 - line(follower);
        let alpha = (d / (r * r + ball * ball).sqrt()).asin() - ball.atan2(r);
        let got = joint.angles(&bodies).0 as f64;
        assert!(
            (got - alpha).abs() < 0.3_f64.to_radians(),
            "joint {k}: {} deg, want {} deg",
            got.to_degrees(),
            alpha.to_degrees()
        );
        r = r * alpha.cos() - ball * alpha.sin();
    }
}

/// C-7: the body follows its wheels while the bend asks less than the grip and slides when it
/// asks more. At full lock the bend asks `m v^2 kappa`, the grip gives `mu m g`: the limit is
/// `sqrt(mu g / kappa)`. The steering is put on over one second, and the test goes at `holds_at_fraction` of the limit and at 1.4 of it, looking at the state after five seconds.
pub fn release_by_speed(defs: Vec<Vehicle>, engine: Engine, holds_at_fraction: f64) {
    let kappa = defs[0].inv_min_turn_radius as f64;
    let limit = (0.85 * G / kappa).sqrt();
    let holds_at = |v: f64| {
        let (mut bodies, joints) = build(&defs, engine);
        launch(&mut bodies, v as f32);
        let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
        let radius = bodies
            .iter()
            .flat_map(|b| b.wheels())
            .find(|w| w.driven())
            .map(|w| w.radius())
            .unwrap();
        let mass: f32 = bodies.iter().map(|b| b.mass()).sum();
        let rolling: f32 = bodies.iter().map(|b| b.rolling_resistance()).sum();
        let mut held = true;
        for i in 0..300 {
            let torque = ((v as f32 - bodies[0].forward_speed()) * mass * 2.0 + rolling) * radius;
            step(
                &mut bodies,
                &joints,
                &brakes,
                torque,
                (i as f32 / 60.0).min(1.0),
            );
            held = bodies[0].holding();
        }
        held
    };
    let slow = holds_at_fraction * limit;
    assert!(holds_at(slow), "must hold at {slow} m/s");
    assert!(!holds_at(1.4 * limit), "must slide at {} m/s", 1.4 * limit);
}

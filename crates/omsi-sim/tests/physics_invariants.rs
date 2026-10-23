//! Invariants of the vehicle physics model that had no test of their own (I1 and I4 of
//! the model's document): what the refactor must keep true whatever it moves.

mod support;

use glam::{DVec3, Vec3};
use omsi_sim::physics::{support_rest_loads, Joint, RigidBody};
use omsi_vehicle::{Axle, Vehicle};
use support::ground::flat;
use support::vehicles::{bus, gn92};

const G: f32 = 9.81;

fn axle(long: f32, driven: bool) -> Axle {
    Axle {
        long,
        max_width: 2.4,
        min_width: 1.76,
        wheel_diameter: 0.94,
        spring: 260.0,
        max_force: 100.0,
        damper: 20.0,
        driven,
        inertia_inv: 0.0,
    }
}

/// A 15 m bus on three axles, its centre of gravity off the middle (in kg, not t).
fn three_axles() -> Vehicle {
    Vehicle {
        mass: 14_500.0,
        cog: Some([0.0, -0.6, 1.1]),
        axles: vec![axle(4.1, false), axle(-2.2, true), axle(-3.6, false)],
        ..bus()
    }
}

/// The weight and its moment about the model origin (along y) that the wheels' static
/// loads carry: what `wheels` hold and where.
fn carried(rb: &RigidBody) -> (f32, f32) {
    let load: f32 = rb.wheels().iter().map(|w| w.rest_load()).sum();
    let moment: f32 = rb
        .wheels()
        .iter()
        .map(|w| w.rest_load() * w.attach().y)
        .sum();
    (load, moment)
}

fn reach(rb: &RigidBody) -> f32 {
    rb.wheels()
        .iter()
        .map(|w| w.attach().y.abs())
        .fold(0.0, f32::max)
}

/// I1: standing, the static loads of the wheels carry the weight, Σ 2 L_e = m g, and
/// balance it about the centre of gravity, Σ 2 L_e y_e = m g c_y.
#[test]
fn i1_the_static_loads_carry_and_balance_the_weight() {
    for (name, def) in [
        ("two axles", bus()),
        ("three axles", three_axles()),
        (
            "centre of gravity forward",
            Vehicle {
                cog: Some([0.0, 0.9, 1.2]),
                ..bus()
            },
        ),
    ] {
        let rb = RigidBody::from_definition(&def, &[]);
        let weight = rb.mass() * G;
        let (load, moment) = carried(&rb);
        assert!(
            (load - weight).abs() <= 1e-5 * weight,
            "{name}: the wheels carry {load} N of {weight} N"
        );
        let want = weight * rb.cog().y;
        assert!(
            (moment - want).abs() <= 1e-5 * weight * reach(&rb),
            "{name}: moment {moment} N m, the weight's {want} N m"
        );
    }
}

/// I1 across a joint: the rear section rests on its axle and the joint, the front section
/// takes what the joint carries on its own axles; together the wheels carry both weights,
/// and each section is balanced with the joint's load where the joint is.
#[test]
fn i1_with_a_joint_the_wheels_carry_both_sections() {
    let (front, rear) = gn92(false);
    let mut bodies = vec![
        RigidBody::from_definition(&front, &[]),
        RigidBody::from_definition(&rear, &[]),
    ];
    let back = Vec3::from(front.coupling_back.as_ref().unwrap().pos);
    let at = Vec3::from(rear.coupling_front.as_ref().unwrap().pos);
    let joints = vec![Joint::new(0, 1, &rear, back, at)];
    support_rest_loads(&mut bodies, &joints, &[&front, &rear]);
    let weights: Vec<f32> = bodies.iter().map(|b| b.mass() * G).collect();
    let total: f32 = weights.iter().sum();
    let (front_load, front_moment) = carried(&bodies[0]);
    let (rear_load, rear_moment) = carried(&bodies[1]);
    assert!(
        (front_load + rear_load - total).abs() <= 1e-5 * total,
        "the wheels carry {} N of {total} N",
        front_load + rear_load
    );
    // what the joint carries: the rear section's weight less what its wheels hold
    let joint = weights[1] - rear_load;
    assert!(
        joint > 0.0 && (front_load - weights[0] - joint).abs() <= 1e-5 * total,
        "joint {joint} N, front wheels {front_load} N"
    );
    let tol = 1e-5 * total * reach(&bodies[0]).max(reach(&bodies[1]));
    let want = weights[1] * bodies[1].cog().y - joint * at.y;
    assert!(
        (rear_moment - want).abs() <= tol,
        "rear section: moment {rear_moment}, want {want}"
    );
    let want = weights[0] * bodies[0].cog().y + joint * back.y;
    assert!(
        (front_moment - want).abs() <= tol,
        "front section: moment {front_moment}, want {want}"
    );
}

/// I4: while the tyres hold, the bus follows its wheels - in a steady bend its yaw rate is
/// the one the steering geometry asks for, ω_z = v κ (T6: a step of 0.25 of the lock at
/// 50 km/h; and a tighter bend slower).
#[test]
fn i4_holding_the_bus_follows_its_wheels() {
    for (kmh, steer) in [(50.0f32, 0.25f32), (30.0, 0.5), (15.0, 1.0)] {
        let mut rb = RigidBody::from_definition(&bus(), &[]);
        rb.place(DVec3::ZERO, 0.0);
        let dt = 1.0 / 60.0;
        let brakes = [0.0; 4];
        for _ in 0..120 {
            rb.step(dt, 0.0, &brakes, 0.0, &flat);
        }
        let v = kmh / 3.6;
        rb.set_velocity(rb.orientation().mul_vec3(Vec3::Y) * v);
        for w in rb.wheels_mut() {
            w.set_spin(v / w.radius());
        }
        let r = rb
            .wheels()
            .iter()
            .find(|w| w.driven())
            .map(|w| w.radius())
            .unwrap();
        let hold = |rb: &RigidBody| {
            ((v - rb.forward_speed()) * rb.mass() * 2.0 + rb.rolling_resistance()) * r
        };
        for _ in 0..120 {
            let m = hold(&rb);
            rb.step(dt, m, &brakes, 0.0, &flat);
        }
        // the last second of six in the bend
        let (mut worst, mut held) = (0.0f32, 0);
        for k in 0..360 {
            let m = hold(&rb);
            rb.step(dt, m, &brakes, steer, &flat);
            if k >= 300 && rb.holding() {
                held += 1;
                let ratio = rb.omega().z.abs() / (rb.forward_speed() * rb.kappa()).abs();
                worst = worst.max((ratio - 1.0).abs());
            }
        }
        assert_eq!(held, 60, "{kmh} km/h, steering {steer}: the tyres let go");
        assert!(
            worst < 0.02,
            "{kmh} km/h, steering {steer}: the yaw rate is {worst} off v κ"
        );
    }
}

/// I4 from the first frame: holding, the bus turns about its `[rot_pnt_long]` line as soon as
/// the wheel turns: a quick turn of the wheel at speed gives the yaw rate v κ from the first
/// frame, not after the tyres have pushed the body round.
#[test]
fn i4_holding_the_bus_turns_about_its_rotation_point_at_once() {
    for (kmh, steer) in [(50.0f32, 0.25f32), (30.0, 0.5), (15.0, 1.0)] {
        let mut rb = RigidBody::from_definition(&bus(), &[]);
        rb.place(DVec3::ZERO, 0.0);
        let dt = 1.0 / 60.0;
        let brakes = [0.0; 4];
        for _ in 0..120 {
            rb.step(dt, 0.0, &brakes, 0.0, &flat);
        }
        let v = kmh / 3.6;
        rb.set_velocity(rb.orientation().mul_vec3(Vec3::Y) * v);
        for w in rb.wheels_mut() {
            w.set_spin(v / w.radius());
        }
        for _ in 0..60 {
            rb.step(dt, 0.0, &brakes, 0.0, &flat);
        }
        let mut yaw_off = 0.0f32;
        for _ in 0..10 {
            rb.step(dt, 0.0, &brakes, steer, &flat);
            assert!(
                rb.holding(),
                "{kmh} km/h, steering {steer}: the tyres let go"
            );
            let ratio = rb.omega().z / (rb.forward_speed() * rb.kappa());
            yaw_off = yaw_off.max((ratio.abs() - 1.0).abs());
        }
        assert!(
            yaw_off < 0.02,
            "{kmh} km/h, steering {steer}: the yaw rate is {yaw_off} off v κ"
        );
    }
}

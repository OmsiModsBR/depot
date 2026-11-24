//! The scripts' control of steerable axles (spec 004, phase 4): `Axle_Steer_Mode_<n>` and
//! `Axle_Steer_Angle_<n>` in, `Axle_Steer_Locked_<n>` out, and a saved situation's angle
//! (`D-6` of `docs/physics/depot-model.md`, `contracts/script-vars.md`).

mod support;

use omsi_model::Model;
use omsi_script::Program;
use omsi_sim::physics::Ground;
use omsi_sim::{Controls, SimClock, VehicleHost, VehicleInstance, VehicleType};
use std::path::PathBuf;
use std::sync::Arc;
use support::ground::flat;
use support::vehicles::{v2_tag_steer, v5_last_axle_steer};

const DT: f32 = 1.0 / 60.0;
/// V2's tag axle: axle 2, its left wheel the fifth.
const TAG: usize = 2;
const TAG_WHEEL: usize = 4;

/// V2 with its tag axle steerable (15°, 200°/s) under Project Depot Physics, on the flat; the
/// scripts' varlist declares the tag axle's variables when `declared`.
fn bus(declared: bool) -> VehicleInstance {
    bus_of(v2_tag_steer().remove(0), declared)
}

fn bus_of(def: omsi_vehicle::Vehicle, declared: bool) -> VehicleInstance {
    let mut v = instance(vehicle_type(def, declared.then_some(TAG)));
    v.enable_rigid_body();
    v
}

/// A vehicle of type `ty` under Project Depot Physics on the flat, not yet on its wheels.
fn instance(ty: Arc<VehicleType>) -> VehicleInstance {
    let mut host = VehicleHost::new(SimClock::default());
    host.street_cond = 0.0;
    host.temperature = 15.0;
    let mut v = VehicleInstance::new(ty, host);
    v.set_physics_mode(omsi_cfg::PhysicsMode::Depot);
    v.contact = Some(Arc::new(flat) as Arc<dyn Ground>);
    v
}

/// The type of `def`, its scripts declaring the steerable axle variables of axle `axle`.
fn vehicle_type(def: omsi_vehicle::Vehicle, axle: Option<usize>) -> Arc<VehicleType> {
    let mut program = Program::default();
    for n in ["M_Wheel", "Velocity"] {
        program.declare_var(n);
    }
    if let Some(axle) = axle {
        for n in ["Mode", "Angle", "Locked"] {
            program.declare_var(&format!("Axle_Steer_{n}_{axle}"));
        }
    }
    Arc::new(VehicleType {
        def,
        model: Model::default(),
        model_dir: PathBuf::new(),
        program: Arc::new(program),
        meshes: Vec::new(),
        paint_schemes: Vec::new(),
        texchanges: Vec::new(),
        wheel_meshes: Vec::new(),
        suspension_axles: Vec::new(),
        missing_packs: Vec::new(),
        mesh_bounds: Vec::new(),
        mesh_boxes: Vec::new(),
    })
}

/// `n` frames with the steering at `steering` and `m_wheel` on the driven wheels.
fn run(v: &mut VehicleInstance, n: usize, steering: f32, m_wheel: f32) {
    for _ in 0..n {
        v.set_var("M_Wheel", m_wheel);
        v.set_controls(Controls {
            steering,
            ..Default::default()
        });
        v.update(DT);
    }
}

fn tag(v: &VehicleInstance) -> f32 {
    v.rigid().unwrap().axle_steer(TAG_WHEEL)
}

fn yaw_rate(v: &VehicleInstance) -> f32 {
    v.rigid().unwrap().omega().z
}

/// Checks: D-6 (each mode in the first step after it is written; the locked flag)
#[test]
fn each_mode_takes_effect_in_the_first_step() {
    let mut v = bus(true);
    run(&mut v, 60, 1.0, 0.0);
    let lock = 15f32.to_radians();
    assert!((tag(&v) + lock).abs() < 1e-4, "mode 0: {}", tag(&v));
    assert_eq!(v.var(&format!("Axle_Steer_Locked_{TAG}")), Some(0.0));
    // mode 1: straight at the centring rate, from the first step on
    v.set_var(&format!("Axle_Steer_Mode_{TAG}"), 1.0);
    run(&mut v, 1, 1.0, 0.0);
    let step = 200f32.to_radians() * DT;
    assert!((tag(&v) - (-lock + step)).abs() < 1e-4, "{}", tag(&v));
    assert_eq!(v.var(&format!("Axle_Steer_Locked_{TAG}")), Some(0.0));
    run(&mut v, 6, 1.0, 0.0);
    assert_eq!(tag(&v), 0.0);
    assert_eq!(v.var(&format!("Axle_Steer_Locked_{TAG}")), Some(1.0));
    // mode 2: the script's angle, clamped to the lock
    v.set_var(&format!("Axle_Steer_Mode_{TAG}"), 2.0);
    v.set_var(&format!("Axle_Steer_Angle_{TAG}"), 1.0);
    run(&mut v, 1, 1.0, 0.0);
    assert!((tag(&v) - step).abs() < 1e-4, "{}", tag(&v));
    assert_eq!(v.var(&format!("Axle_Steer_Locked_{TAG}")), Some(0.0));
    run(&mut v, 20, 1.0, 0.0);
    assert!((tag(&v) - lock).abs() < 1e-4, "{}", tag(&v));
    // back to mode 0
    v.set_var(&format!("Axle_Steer_Mode_{TAG}"), 0.0);
    run(&mut v, 20, 1.0, 0.0);
    assert!((tag(&v) + lock).abs() < 1e-4, "{}", tag(&v));
}

/// Checks: D-6 (a mode it does not know, or a script angle that is not a number, is mode 0)
#[test]
fn an_unknown_mode_or_angle_is_the_basic_mode() {
    let lock = 15f32.to_radians();
    for (mode, angle) in [(7.0, 0.0), (-1.0, 0.0), (0.5, 0.0), (2.0, f32::NAN)] {
        let mut v = bus(true);
        v.set_var(&format!("Axle_Steer_Mode_{TAG}"), mode);
        v.set_var(&format!("Axle_Steer_Angle_{TAG}"), angle);
        run(&mut v, 30, 1.0, 0.0);
        assert!(
            (tag(&v) + lock).abs() < 1e-4,
            "mode {mode}, angle {angle}: {}",
            tag(&v)
        );
        assert_eq!(v.var(&format!("Axle_Steer_Locked_{TAG}")), Some(0.0));
    }
}

/// Checks: D-6 (a script that does not declare the variables: mode 0, nothing written)
#[test]
fn undeclared_variables_leave_the_basic_mode() {
    let mut v = bus(false);
    run(&mut v, 30, 1.0, 0.0);
    assert!((tag(&v) + 15f32.to_radians()).abs() < 1e-4);
    assert_eq!(v.var(&format!("Axle_Steer_Locked_{TAG}")), None);
}

/// Checks: D-5, D-6 (locking and unlocking in a steady turn at 25 km/h: no jerk; SC-003, with
/// a tag axle that centres at 20°/s)
#[test]
fn locking_and_unlocking_in_a_turn_moves_the_body_smoothly() {
    let def = support::vehicles::steerable(support::vehicles::v2_rigid_6x2(), 0, TAG, 15.0, 20.0);
    let mut v = bus_of(def.into_iter().next().unwrap(), true);
    v.set_speed(25.0 / 3.6);
    let steering = 0.3;
    // (a drive that keeps it near 25 km/h)
    let hold = |v: &VehicleInstance| {
        ((25.0 / 3.6 - v.rigid().unwrap().forward_speed()) * 30_000.0 + 2_000.0) * 0.47
    };
    for _ in 0..300 {
        let m = hold(&v);
        run(&mut v, 1, steering, m);
    }
    assert!(v.rigid().unwrap().holding());
    let mut worst = 0.0_f32;
    for mode in [1.0, 0.0] {
        v.set_var(&format!("Axle_Steer_Mode_{TAG}"), mode);
        for _ in 0..90 {
            let before = yaw_rate(&v);
            let m = hold(&v);
            run(&mut v, 1, steering, m);
            worst = worst.max(((yaw_rate(&v) - before) / before).abs());
        }
    }
    assert!(v.rigid().unwrap().holding());
    assert!(worst < 0.02, "the yaw rate moved by {worst} in one frame");
}

/// Checks: D-6 (a saved situation's angle: no swing back from straight)
#[test]
fn a_saved_situation_puts_the_axle_back_at_its_angle() {
    let mut v = bus(true);
    let saved = |name: &str| (name == format!("Axle_Steering_{TAG}_L")).then_some(-0.2_f32);
    v.restore_steer_axles(saved);
    assert!((tag(&v) + 0.2).abs() < 1e-6, "{}", tag(&v));
    // (beyond the lock: at the lock)
    v.restore_steer_axles(|_| Some(1.0));
    assert!((tag(&v) - 15f32.to_radians()).abs() < 1e-6, "{}", tag(&v));
    // an old situation without it: straight
    let mut v = bus(true);
    v.restore_steer_axles(|_| None);
    assert_eq!(tag(&v), 0.0);
}

/// Checks: D-6 (a rear section's steerable axle answers to its number over the whole vehicle:
/// V5's fourth axle is `Axle_Steer_Mode_3`)
#[test]
fn a_rear_sections_axle_answers_to_its_number_over_the_vehicle() {
    let mut defs = v5_last_axle_steer();
    let rear = vehicle_type(defs.remove(1), None);
    // (the front section's scripts, which the rear section's variables live in)
    let mut v = instance(vehicle_type(defs.remove(0), Some(3)));
    v.attach_trailer(rear);
    v.enable_rigid_body();
    let fourth = |v: &VehicleInstance| v.trailers[0].rigid().unwrap().axle_steer(2);
    run(&mut v, 60, 0.0, 3000.0);
    run(&mut v, 180, 0.7, 1500.0);
    assert!(fourth(&v) < -0.05, "the fourth axle at {} rad", fourth(&v));
    assert_eq!(v.var("Axle_Steer_Locked_3"), Some(0.0));
    v.set_var("Axle_Steer_Mode_3", 1.0);
    run(&mut v, 120, 0.7, 1500.0);
    assert_eq!(fourth(&v), 0.0);
    assert_eq!(v.var("Axle_Steer_Locked_3"), Some(1.0));
}

//! Traces of the vehicle physics, the safety net of its refactor (Fase 0 of the plan):
//! scenarios that drive the rigid body, a train of articulated sections and the vehicle
//! with its script variables through every branch of the model, and write out the whole
//! state frame by frame.
//!
//! With `OMSI_PHYSICS_TRACE_DIR` set, each scenario writes `<dir>/<name>.trace`:
//! `scripts/physics-trace-diff.sh` runs them on a base commit and on the change and
//! compares the two. A change that only moves code leaves every trace as it was, to the
//! bit. Without it, each scenario runs twice and both runs must write the same trace.
//!
//! Numbers are written as `{:?}` writes them - the shortest decimal that reads back to
//! the same bits, NaN with its bits -, so equal text is equal bits. The file uses only the
//! crate's public API: the script copies it as it is into the commit it compares with.

use glam::{DVec2, DVec3, Quat, Vec3};
use omsi_model::Model;
use omsi_script::Program;
use omsi_sim::ai_motion::{AiBody, MotionKind};
use omsi_sim::collision::{CollisionWorld, Obb};
use omsi_sim::rigid::{place_train, step_train, support_rest_loads, BodyInputs, Ground, GroundProbe, Impact, Joint, RigidBody};
use omsi_sim::vehicle::{road_grip, AiFrame};
use omsi_sim::{Controls, SimClock, VehicleHost, VehicleInstance, VehicleType};
use omsi_vehicle::vehicle::Coupling;
use omsi_vehicle::{Axle, Vehicle};
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;

type Probe<'a> = &'a dyn Fn(f64, f64, f64) -> GroundProbe;

// ---------------------------------------------------------------------------------------
// Writing the state

fn n32(x: f32) -> String {
    if x.is_nan() {
        format!("NaN:{:08x}", x.to_bits())
    } else {
        format!("{x:?}")
    }
}

fn n64(x: f64) -> String {
    if x.is_nan() {
        format!("NaN:{:016x}", x.to_bits())
    } else {
        format!("{x:?}")
    }
}

fn v3(v: Vec3) -> String {
    format!("{} {} {}", n32(v.x), n32(v.y), n32(v.z))
}

fn dv3(v: DVec3) -> String {
    format!("{} {} {}", n64(v.x), n64(v.y), n64(v.z))
}

fn quat(q: Quat) -> String {
    format!("{} {} {} {}", n32(q.x), n32(q.y), n32(q.z), n32(q.w))
}

/// One scenario's trace: a line for each frame with what went in, then the state.
#[derive(Default)]
struct Trace {
    text: String,
    frame: usize,
}

impl Trace {
    fn frame(&mut self, dt: f32, inputs: &str) {
        let _ = writeln!(self.text, "frame {} dt {} {inputs}", self.frame, n32(dt));
        self.frame += 1;
    }

    fn body(&mut self, i: usize, rb: &RigidBody) {
        let t = &mut self.text;
        let _ = writeln!(
            t,
            "  body {i} p {} q {} v {} w {} holding {} kappa {} steer {} accel {} mu {} inside {:?}",
            dv3(rb.position),
            quat(rb.orientation),
            v3(rb.velocity),
            v3(rb.omega),
            rb.holding,
            n32(rb.kappa),
            n32(rb.steer_deg),
            v3(rb.accel_body),
            n32(rb.friction),
            rb.spawned_inside,
        );
        for (k, w) in rb.wheels.iter().enumerate() {
            let walls: Vec<String> = w.walls.iter().map(|f| format!("{} {} {} {}", n64(f.point.x), n64(f.point.y), n32(f.normal.x), n32(f.normal.y))).collect();
            let _ = writeln!(
                t,
                "    wheel {k} spin {} slipping {} locked {} steer {} comp {} rate {} touch {} on_ground {} ground {} seen {} load {} rest {} sf {} rpm {} rot {} step {} walls [{}]",
                n32(w.spin),
                w.slipping,
                w.locked,
                n32(w.steer),
                n32(w.compression),
                n32(w.compression_rate),
                w.touch.map(n32).unwrap_or_else(|| "-".into()),
                w.on_ground,
                n64(w.ground_z),
                w.ground_seen,
                n32(w.load),
                n32(w.rest_load),
                n32(w.spring_factor),
                n32(w.rpm),
                n32(w.rotation_deg),
                n32(w.step_force),
                walls.join(", "),
            );
        }
        for c in &rb.coupled {
            let _ = writeln!(t, "    coupled point {} dir {} mass {} brake {} load {}", v3(c.point), v3(c.dir), n32(c.mass), n32(c.brake), n32(c.load));
        }
        for hit in &rb.wheel_impacts {
            self.impact("wheel_impact", hit);
        }
    }

    fn impact(&mut self, what: &str, hit: &Impact) {
        let _ = writeln!(
            self.text,
            "    {what} point {} speed {} energy {} obstacle {} broke {} push {}",
            v3(hit.point),
            n32(hit.speed),
            n32(hit.energy),
            hit.obstacle,
            hit.broke,
            v3(hit.push)
        );
    }

    fn joint(&mut self, i: usize, j: &Joint, bodies: &[RigidBody]) {
        let (alpha, beta) = j.angles(bodies);
        let _ = writeln!(self.text, "  joint {i} alpha {} beta {} gap {}", n32(alpha), n32(beta), n32(j.gap(bodies)));
    }

    /// The vehicle as the rest of the game and its scripts see it.
    fn vehicle(&mut self, v: &VehicleInstance, vars: &[String]) {
        let _ = writeln!(
            self.text,
            "  vehicle p {} heading {} pitch {} bank {} speed {} steer {} accel {} a_trans {} crashes {} impact {} coll {}",
            dv3(v.position),
            n64(v.heading),
            n32(v.pitch),
            n32(v.bank),
            n32(v.motion().speed),
            n32(v.motion().steer_deg),
            v3(v.motion().accel),
            v3(v.motion().a_trans),
            v.crashes,
            n32(v.last_impact),
            v3(Vec3::from(v.host.coll_pos)),
        );
        for (a, axle) in v.motion().wheels.iter().enumerate() {
            for (s, w) in axle.iter().enumerate() {
                let _ = writeln!(self.text, "    model_wheel {a} {s} rot {} rpm {} suspension {}", n32(w.rotation_deg), n32(w.rpm), n32(w.suspension));
            }
        }
        for name in vars {
            let value = v.var(name).map(n32).unwrap_or_else(|| "-".into());
            let _ = writeln!(self.text, "    var {name} {value}");
        }
        if let Some(rb) = v.rigid() {
            self.body(0, rb);
        }
        for (i, part) in v.trailers.iter().enumerate() {
            let _ = writeln!(self.text, "  part {} p {} heading {}", i + 1, dv3(part.position), n64(part.heading));
            if let Some(rb) = part.rigid() {
                self.body(i + 1, rb);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Running the scenarios

/// Write the scenario's trace where `OMSI_PHYSICS_TRACE_DIR` says; without it, run it
/// twice and compare (and look for a body that left the numbers).
fn check(name: &str, scenario: fn() -> String) {
    let first = scenario();
    assert!(first.lines().count() > 10, "{name}: no trace");
    // (the inputs may be no number - T14 -, the state never)
    let lost = |l: &&str| !l.starts_with("frame") && l.split_whitespace().any(|w| w == "inf" || w == "-inf" || w.starts_with("NaN"));
    if let Some(l) = first.lines().find(lost) {
        panic!("{name}: the state left the numbers:\n{l}");
    }
    match std::env::var_os("OMSI_PHYSICS_TRACE_DIR") {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{name}.trace")), first).unwrap();
        }
        None => {
            let second = scenario();
            if first != second {
                let (k, (a, b)) = first.lines().zip(second.lines()).enumerate().find(|(_, (a, b))| a != b).unwrap_or((0, ("", "")));
                panic!("{name}: two runs differ at line {}:\n{a}\n{b}", k + 1);
            }
        }
    }
}

macro_rules! scenarios {
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                check(stringify!($name), scenarios::$name);
            }
        )*
    };
}

scenarios!(
    t01_parked_on_the_flat,
    t02_parked_on_a_grade_then_released,
    t03_accelerates_to_50,
    t04_spins_on_ice,
    t05_brakes_beyond_the_grip,
    t06_steering_step_at_50,
    t07_full_lock_at_60,
    t08a_humps,
    t08b_climbs_a_kerb,
    t09_drives_into_a_wall_face,
    t10a_crashes_into_a_wall,
    t10b_breaks_off_a_post,
    t10c_a_car_hits_the_side,
    t11_puller_turns_90_degrees,
    t12_pusher_starts_on_a_grade,
    t13_frame_times_vary,
    t14_inputs_that_are_no_number,
    t15_spring_factor_varies,
    t16_wheels_of_two_sizes,
    b1_vehicle_with_script_variables,
    b2_articulated_vehicle,
    b3_vehicle_crash,
    b4_kinematic_vehicle,
    b5_ai_vehicle_follows_a_lane,
    b6_kinematic_vehicle_crash,
    b7_articulated_vehicle_bends_on_ice,
);

mod scenarios {
    use super::*;

    // -- the vehicles (the synthetic ones of the crate's own tests) --

    /// A 12 m solo bus on two axles (`rigid`'s tests' `bus()`).
    pub fn bus() -> Vehicle {
        let axle = |long: f32, spring: f32, max_force: f32, driven: bool| Axle { long, max_width: 2.4, min_width: 1.76, wheel_diameter: 0.94, spring, max_force, damper: 20.0, driven, inertia_inv: 0.0 };
        Vehicle {
            mass: 10.2,
            moment_of_inertia: [600.0, 90.0, 400.0],
            cog_height: 1.2,
            rolling_resistance: 1000.0,
            rot_pnt_long: -2.7,
            inv_min_turn_radius: 0.13,
            axles: vec![axle(3.238, 240.0, 90.0, false), axle(-2.637, 280.0, 116.0, true)],
            bounding_box: Some([2.472, 11.663, 2.491, 0.0, 0.001, 1.622]),
            ..Default::default()
        }
    }

    /// The stock GN92's two sections (`train`'s tests' `gn92()`): a pusher, whose front
    /// section drives no wheel; `puller` drives the front section's rear axle instead.
    pub fn gn92(puller: bool) -> (Vehicle, Vehicle) {
        let axle = |long: f32, max_width: f32, min_width: f32, driven: bool| Axle { long, max_width, min_width, wheel_diameter: 0.94, spring: 280.0, max_force: 116.0, damper: 20.0, driven, inertia_inv: 0.0 };
        let front = Vehicle {
            mass: 8.0,
            moment_of_inertia: [300.0, 80.0, 300.0],
            cog_height: 1.2,
            rolling_resistance: 900.0,
            rot_pnt_long: -2.7,
            inv_min_turn_radius: 0.13,
            axles: vec![Axle { spring: 240.0, max_force: 90.0, ..axle(3.238, 2.25, 1.76, false) }, axle(-2.637, 2.4, 1.2, puller)],
            coupling_back: Some(Coupling { pos: [0.0, -4.331, 0.315] }),
            ..Default::default()
        };
        let rear = Vehicle {
            mass: 7.2,
            moment_of_inertia: [150.0, 40.0, 150.0],
            cog_height: 1.2,
            rolling_resistance: 500.0,
            rot_pnt_long: -0.387,
            axles: vec![axle(-0.387, 2.4, 1.2, !puller)],
            coupling_front: Some(Coupling { pos: [0.0, 4.169, 0.315] }),
            coupling_front_character: Some([52.5, -20.0, 20.0, 1.0]),
            ..Default::default()
        };
        (front, rear)
    }

    // -- the grounds --

    /// The ground at height `z` for a probe reaching up to `top`: a face too high is above,
    /// with the road at 0 still below it (a kerb, a platform).
    fn step_at(z: f64, top: f64) -> GroundProbe {
        if z <= top {
            GroundProbe { below: Some(z), above: None }
        } else {
            GroundProbe { below: Some(0.0), above: Some(z) }
        }
    }

    pub fn flat(_x: f64, _y: f64, _top: f64) -> GroundProbe {
        GroundProbe { below: Some(0.0), above: None }
    }

    /// A grade of `s` all along y (z = s y).
    fn grade(s: f64) -> impl Fn(f64, f64, f64) -> GroundProbe {
        move |_x, y, top| {
            let z = y * s;
            if z <= top {
                GroundProbe { below: Some(z), above: None }
            } else {
                GroundProbe { below: None, above: Some(z) }
            }
        }
    }

    /// The road at 0 with a step of `step` metres from y = `at` on.
    fn road(at: f64, step: f64) -> impl Fn(f64, f64, f64) -> GroundProbe {
        move |_x, y, top| step_at(if y >= at { step } else { 0.0 }, top)
    }

    /// A 6 cm hump under the right-hand wheels, an 8 cm one under the left-hand ones, then
    /// a 15 cm kerb across the road.
    fn humps_and_kerb(x: f64, y: f64, top: f64) -> GroundProbe {
        let z = if x > 0.0 && (20.0..20.6).contains(&y) {
            0.06
        } else if x < 0.0 && (35.0..35.6).contains(&y) {
            0.08
        } else if y >= 60.0 {
            0.15
        } else {
            0.0
        };
        step_at(z, top)
    }

    // -- driving a body --

    fn inputs(torque: f32, brakes: &[f32], steer: f32) -> String {
        let b: Vec<String> = brakes.iter().map(|b| n32(*b)).collect();
        format!("torque {} brake [{}] steer {}", n32(torque), b.join(" "), n32(steer))
    }

    /// `frames` frames of `dt`: `input` gives the drive torque, the brake on each wheel and
    /// the steering from the body as it stands.
    fn run(t: &mut Trace, rb: &mut RigidBody, frames: usize, dt: f32, g: Probe, mut input: impl FnMut(&RigidBody) -> (f32, f32, f32)) {
        for _ in 0..frames {
            let (torque, brake, steer) = input(rb);
            let brakes = vec![brake; rb.wheels.len()];
            t.frame(dt, &inputs(torque, &brakes, steer));
            rb.step(dt, torque, &brakes, steer, g);
            t.body(0, rb);
        }
    }

    /// The bus put down at the origin facing +y.
    fn placed(def: &Vehicle) -> RigidBody {
        let mut rb = RigidBody::from_definition(def, &[]);
        rb.place(DVec3::ZERO, 0.0);
        rb
    }

    /// Rolling at `v` m/s, the wheels turning with the road.
    fn launch(rb: &mut RigidBody, v: f32) {
        rb.velocity = rb.orientation.mul_vec3(Vec3::Y) * v;
        for w in rb.wheels.iter_mut() {
            w.spin = v / w.radius;
        }
    }

    /// The torque that holds the bus near `v` m/s (as `examples/handling.rs` drives).
    fn hold(rb: &RigidBody, v: f32) -> f32 {
        let r = rb.wheels.iter().find(|w| w.driven).map(|w| w.radius).unwrap_or(0.5);
        ((v - rb.forward_speed()) * rb.mass * 2.0 + rb.rolling_resistance) * r
    }

    // -- T1..T16: the rigid body and the train --

    pub fn t01_parked_on_the_flat() -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 300, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        t.text
    }

    pub fn t02_parked_on_a_grade_then_released() -> String {
        let mut t = Trace::default();
        let g = grade(0.08);
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 180, 1.0 / 60.0, &g, |_| (0.0, 15_000.0, 0.0));
        run(&mut t, &mut rb, 300, 1.0 / 60.0, &g, |_| (0.0, 0.0, 0.0));
        t.text
    }

    pub fn t03_accelerates_to_50() -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 30, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        run(&mut t, &mut rb, 900, 1.0 / 60.0, &flat, |rb| (if rb.forward_speed() < 50.0 / 3.6 { 8000.0 } else { 0.0 }, 0.0, 0.0));
        run(&mut t, &mut rb, 120, 1.0 / 60.0, &flat, |_| (0.0, 0.0, 0.0));
        t.text
    }

    pub fn t04_spins_on_ice() -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        rb.friction = road_grip(1.0, -5.0);
        run(&mut t, &mut rb, 30, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        run(&mut t, &mut rb, 180, 1.0 / 60.0, &flat, |_| (20_000.0, 0.0, 0.0));
        run(&mut t, &mut rb, 180, 1.0 / 60.0, &flat, |_| (0.0, 0.0, 0.0));
        run(&mut t, &mut rb, 120, 1.0 / 60.0, &flat, |_| (0.0, 3000.0, 0.0));
        t.text
    }

    pub fn t05_brakes_beyond_the_grip() -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        launch(&mut rb, 50.0 / 3.6);
        run(&mut t, &mut rb, 360, 1.0 / 60.0, &flat, |_| (0.0, 60_000.0, 0.0));
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &flat, |_| (0.0, 0.0, 0.0));
        t.text
    }

    /// A step of the steering at `kmh` and back to the middle (`examples/handling.rs`).
    fn steering_step(kmh: f32, steer: f32, frames: usize) -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        let v = kmh / 3.6;
        run(&mut t, &mut rb, 120, 1.0 / 60.0, &flat, |_| (0.0, 0.0, 0.0));
        launch(&mut rb, v);
        run(&mut t, &mut rb, 120, 1.0 / 60.0, &flat, |rb| (hold(rb, v), 0.0, 0.0));
        run(&mut t, &mut rb, frames, 1.0 / 60.0, &flat, |rb| (hold(rb, v), 0.0, steer));
        run(&mut t, &mut rb, 240, 1.0 / 60.0, &flat, |rb| (hold(rb, v), 0.0, 0.0));
        t.text
    }

    pub fn t06_steering_step_at_50() -> String {
        steering_step(50.0, 0.25, 360)
    }

    pub fn t07_full_lock_at_60() -> String {
        steering_step(60.0, 1.0, 240)
    }

    pub fn t08a_humps() -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &humps_and_kerb, |_| (0.0, 5000.0, 0.0));
        launch(&mut rb, 40.0 / 3.6);
        run(&mut t, &mut rb, 360, 1.0 / 60.0, &humps_and_kerb, |rb| (hold(rb, 40.0 / 3.6), 0.0, 0.0));
        t.text
    }

    pub fn t08b_climbs_a_kerb() -> String {
        let mut t = Trace::default();
        let kerb = road(6.0, 0.15);
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &kerb, |_| (0.0, 5000.0, 0.0));
        run(&mut t, &mut rb, 600, 1.0 / 60.0, &kerb, |_| (9500.0, 0.0, 0.0));
        t.text
    }

    pub fn t09_drives_into_a_wall_face() -> String {
        let mut t = Trace::default();
        let wall = road(10.0, 0.52);
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &wall, |_| (0.0, 5000.0, 0.0));
        launch(&mut rb, 10.0 / 3.6);
        run(&mut t, &mut rb, 300, 1.0 / 60.0, &wall, |rb| (hold(rb, 10.0 / 3.6).max(0.0), 0.0, 0.0));
        run(&mut t, &mut rb, 120, 1.0 / 60.0, &wall, |_| (-6000.0, 0.0, 0.0));
        t.text
    }

    /// `frames` frames at 30 a second against `obstacles`, broken posts left alone.
    fn crash(t: &mut Trace, rb: &mut RigidBody, frames: usize, torque: f32, obstacles: &[Obb], broken: &mut Vec<usize>) {
        let bb = bus().bounding_box.unwrap();
        let dt = 1.0 / 30.0;
        for _ in 0..frames {
            let brakes = vec![0.0; rb.wheels.len()];
            t.frame(dt, &inputs(torque, &brakes, 0.0));
            rb.step(dt, torque, &brakes, 0.0, &flat);
            let gone = broken.clone();
            let hits = rb.collide(bb, obstacles, &|i| gone.contains(&i), dt);
            t.body(0, rb);
            for hit in &hits {
                t.impact("impact", hit);
                if hit.broke && !broken.contains(&hit.obstacle) {
                    broken.push(hit.obstacle);
                }
            }
        }
    }

    pub fn t10a_crashes_into_a_wall() -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        rb.velocity = Vec3::new(0.0, 40.0 / 3.6, 0.0);
        let wall = [Obb::from_box([20.0, 1.0, 3.0, 0.0, 0.0, 1.5], DVec3::new(0.0, 12.0, 0.0), 0.0)];
        crash(&mut t, &mut rb, 90, 0.0, &wall, &mut Vec::new());
        crash(&mut t, &mut rb, 60, -8000.0, &wall, &mut Vec::new());
        // a glancing blow, heading 20 degrees into a wall along the road
        let mut rb = placed(&bus());
        rb.place(DVec3::ZERO, 20.0);
        run(&mut t, &mut rb, 30, 1.0 / 60.0, &flat, |_| (0.0, 0.0, 0.0));
        rb.velocity = rb.orientation.mul_vec3(Vec3::Y) * 30.0 / 3.6;
        let side = [Obb::from_box([1.0, 60.0, 3.0, 0.0, 0.0, 1.5], DVec3::new(4.5, 10.0, 0.0), 0.0)];
        crash(&mut t, &mut rb, 60, 0.0, &side, &mut Vec::new());
        t.text
    }

    pub fn t10b_breaks_off_a_post() -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        rb.velocity = Vec3::new(0.0, 30.0 / 3.6, 0.0);
        let mut post = Obb::from_box([0.2, 0.2, 6.0, 0.0, 0.0, 3.0], DVec3::new(0.5, 10.0, 0.0), 0.0);
        post.pole = Some((0.05, 0.5));
        post.id = 11;
        crash(&mut t, &mut rb, 60, 0.0, &[post], &mut Vec::new());
        t.text
    }

    pub fn t10c_a_car_hits_the_side() -> String {
        let mut t = Trace::default();
        let bb = bus().bounding_box.unwrap();
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        let dt = 1.0 / 30.0;
        for k in 0..30 {
            // from the left, heading east at 50 km/h (kinematic: it keeps its speed)
            let car = Obb::from_box([1.7, 4.2, 1.4, 0.0, 0.0, 0.7], DVec3::new(-4.0 + 50.0 / 3.6 * k as f64 / 30.0, 0.0, 0.0), 90.0).moving(DVec2::new(50.0 / 3.6, 0.0), 1000.0, 7);
            let brakes = [5000.0; 4];
            t.frame(dt, &inputs(0.0, &brakes, 0.0));
            rb.step(dt, 0.0, &brakes, 0.0, &flat);
            let hits = rb.collide(bb, &[car], &|_| false, dt);
            t.body(0, &rb);
            for hit in &hits {
                t.impact("impact", hit);
            }
        }
        run(&mut t, &mut rb, 120, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        t.text
    }

    /// Two sections on one joint, put down at the origin facing +y.
    fn train(front: &Vehicle, rear: &Vehicle) -> (Vec<RigidBody>, Vec<Joint>) {
        let mut bodies = vec![RigidBody::from_definition(front, &[]), RigidBody::from_definition(rear, &[])];
        let back = Vec3::from(front.coupling_back.as_ref().unwrap().pos);
        let at = Vec3::from(rear.coupling_front.as_ref().unwrap().pos);
        let joints = vec![Joint::new(0, 1, rear, back, at)];
        support_rest_loads(&mut bodies, &joints, &[front, rear]);
        place_train(&mut bodies, &joints, DVec3::ZERO, &[0.0, 0.0]);
        (bodies, joints)
    }

    /// `frames` frames of the train: `input` gives the drive torque, the brake on each wheel
    /// of each section and the steering.
    fn run_train(t: &mut Trace, bodies: &mut [RigidBody], joints: &[Joint], frames: usize, g: Probe, mut input: impl FnMut(&[RigidBody]) -> (f32, [f32; 2], f32)) {
        let dt = 1.0 / 60.0;
        for _ in 0..frames {
            let (torque, brake, steer) = input(bodies);
            let (b0, b1) = (vec![brake[0]; bodies[0].wheels.len()], vec![brake[1]; bodies[1].wheels.len()]);
            t.frame(dt, &format!("{} rear [{}]", inputs(torque, &b0, steer), n32(brake[1])));
            let inputs = [BodyInputs { brake: &b0, steer: Some(steer), air: true }, BodyInputs { brake: &b1, steer: None, air: false }];
            step_train(bodies, joints, dt, torque, &inputs, g);
            for (i, b) in bodies.iter().enumerate() {
                t.body(i, b);
            }
            for (i, j) in joints.iter().enumerate() {
                t.joint(i, j, bodies);
            }
        }
    }

    /// The torque that holds a train of `mass` kg near `v` m/s.
    fn cruise(at: f32, v: f32, mass: f32) -> f32 {
        ((v - at) * mass * 1.5 + 0.012 * mass * 9.81 * v.signum()) * 0.47
    }

    pub fn t11_puller_turns_90_degrees() -> String {
        let mut t = Trace::default();
        let (front, rear) = gn92(true);
        let (mut bodies, joints) = train(&front, &rear);
        let v = 20.0 / 3.6;
        run_train(&mut t, &mut bodies, &joints, 60, &flat, |_| (0.0, [5000.0; 2], 0.0));
        run_train(&mut t, &mut bodies, &joints, 300, &flat, |b| (cruise(b[0].forward_speed(), v, 15_200.0), [0.0; 2], 0.0));
        // into the bend until the front section has turned through 90 degrees
        let mut turned = false;
        run_train(&mut t, &mut bodies, &joints, 900, &flat, |b| {
            let heading = b[0].heading_pitch_bank().0;
            turned |= (90.0..180.0).contains(&heading);
            (cruise(b[0].forward_speed(), v, 15_200.0), [0.0; 2], if turned { 0.0 } else { 0.8 })
        });
        t.text
    }

    pub fn t12_pusher_starts_on_a_grade() -> String {
        let mut t = Trace::default();
        let (front, rear) = gn92(false);
        let (mut bodies, joints) = train(&front, &rear);
        let g = grade(0.06);
        run_train(&mut t, &mut bodies, &joints, 120, &g, |_| (0.0, [15_000.0; 2], 0.0));
        run_train(&mut t, &mut bodies, &joints, 480, &g, |_| (14_000.0, [0.0; 2], 0.0));
        run_train(&mut t, &mut bodies, &joints, 120, &g, |_| (0.0, [20_000.0; 2], 0.0));
        t.text
    }

    /// T6 at frame times from 1/144 s to the longest a step takes, and beyond.
    pub fn t13_frame_times_vary() -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        let v = 50.0 / 3.6;
        run(&mut t, &mut rb, 120, 1.0 / 60.0, &flat, |_| (0.0, 0.0, 0.0));
        launch(&mut rb, v);
        let schedule: [(usize, f32, f32); 12] = [
            (144, 1.0 / 144.0, 0.0),
            (288, 1.0 / 144.0, 0.25),
            (30, 1.0 / 30.0, 0.25),
            (1, 0.25, 0.25),
            (59, 1.0 / 60.0, 0.25),
            (1, 0.1, 0.25),
            (1, 0.0, 0.25),
            (59, 1.0 / 60.0, 0.0),
            (1, 0.3, 0.0),
            (30, 1.0 / 30.0, 0.0),
            (1, 0.25, 0.0),
            (90, 1.0 / 30.0, 0.0),
        ];
        for (frames, dt, steer) in schedule {
            run(&mut t, &mut rb, frames, dt, &flat, |rb| (hold(rb, v), 0.0, steer));
        }
        t.text
    }

    pub fn t14_inputs_that_are_no_number() -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &flat, |_| (5000.0, 0.0, 0.0));
        run(&mut t, &mut rb, 10, 1.0 / 60.0, &flat, |_| (f32::NAN, 0.0, 0.0));
        run(&mut t, &mut rb, 10, 1.0 / 60.0, &flat, |_| (5000.0, 0.0, f32::NAN));
        run(&mut t, &mut rb, 10, 1.0 / 60.0, &flat, |_| (f32::NEG_INFINITY, 0.0, 0.0));
        for _ in 0..10 {
            // a brake that is no number on one wheel
            let brakes = [f32::NAN, 0.0, 0.0, 0.0];
            t.frame(1.0 / 60.0, &inputs(5000.0, &brakes, 0.0));
            rb.step(1.0 / 60.0, 5000.0, &brakes, 0.0, &flat);
            t.body(0, &rb);
        }
        // a frame time that is no number
        t.frame(f32::NAN, &inputs(5000.0, &[0.0; 4], 0.0));
        rb.step(f32::NAN, 5000.0, &[0.0; 4], 0.0, &flat);
        t.body(0, &rb);
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &flat, |_| (5000.0, 0.0, 0.0));
        t.text
    }

    /// The air bellows let down to 30 %, then the right-hand side pumped up again (the
    /// bus kneels and comes back up), standing and then rolling.
    pub fn t15_spring_factor_varies() -> String {
        let mut t = Trace::default();
        let mut rb = placed(&bus());
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        for k in 0..300 {
            let sf = 1.0 - 0.7 * k as f32 / 300.0;
            rb.wheels.iter_mut().for_each(|w| w.spring_factor = sf);
            run(&mut t, &mut rb, 1, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        }
        for k in 0..120 {
            let sf = 0.3 + 0.7 * k as f32 / 120.0;
            rb.wheels.iter_mut().skip(1).step_by(2).for_each(|w| w.spring_factor = sf);
            run(&mut t, &mut rb, 1, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        }
        rb.wheels.iter_mut().for_each(|w| w.spring_factor = 1.0);
        launch(&mut rb, 30.0 / 3.6);
        run(&mut t, &mut rb, 120, 1.0 / 60.0, &flat, |rb| (hold(rb, 30.0 / 3.6), 0.0, 0.0));
        for k in 0..240 {
            let sf = 1.0 - 0.5 * (k as f32 / 40.0).sin().abs();
            rb.wheels.iter_mut().take(2).for_each(|w| w.spring_factor = sf);
            run(&mut t, &mut rb, 1, 1.0 / 60.0, &flat, |rb| (hold(rb, 30.0 / 3.6), 0.0, 0.1));
        }
        t.text
    }

    /// The bus on smaller wheels in front (0.8 m) than behind (1.0 m): rolling at 30 km/h it
    /// drives on, then brakes to a stop.
    pub fn t16_wheels_of_two_sizes() -> String {
        let mut t = Trace::default();
        let mut def = bus();
        def.axles[0].wheel_diameter = 0.8;
        def.axles[1].wheel_diameter = 1.0;
        let mut rb = placed(&def);
        run(&mut t, &mut rb, 60, 1.0 / 60.0, &flat, |_| (0.0, 5000.0, 0.0));
        launch(&mut rb, 30.0 / 3.6);
        run(&mut t, &mut rb, 120, 1.0 / 60.0, &flat, |_| (6000.0, 0.0, 0.0));
        run(&mut t, &mut rb, 240, 1.0 / 60.0, &flat, |_| (0.0, 20_000.0, 0.0));
        t.text
    }

    // -- B1..B7: the vehicle, through its script variables --

    const WHEEL_VARS: [&str; 6] = ["Wheel_Rotation", "Wheel_RotationSpeed", "Axle_Steering", "Axle_Suspension", "Axle_Brakeforce", "Axle_Springfactor"];

    /// The variables the physics reads and writes, for `axles` axles.
    fn var_names(axles: usize) -> Vec<String> {
        let mut names: Vec<String> = ["M_Wheel", "Brakeforce", "Velocity", "Velocity_Ground", "n_Wheel", "A_Trans_X", "A_Trans_Y", "A_Trans_Z"].map(String::from).to_vec();
        for a in 0..axles {
            for s in ["L", "R"] {
                names.extend(WHEEL_VARS.map(|v| format!("{v}_{a}_{s}")));
            }
        }
        names
    }

    /// A vehicle type of `def` with no model and a script set that only declares the
    /// physics' variables (no script runs: the scenario writes them as a script would).
    fn vehicle_type(def: Vehicle, axles: usize) -> Arc<VehicleType> {
        let mut program = Program::default();
        for n in var_names(axles) {
            program.declare_var(&n);
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

    fn instance(ty: Arc<VehicleType>) -> VehicleInstance {
        let mut host = VehicleHost::new(SimClock::default());
        host.street_cond = 0.0;
        host.temperature = 15.0;
        VehicleInstance::new(ty, host)
    }

    /// What a frame's script writes into the physics' variables.
    #[derive(Default, Clone)]
    struct Script {
        m_wheel: f32,
        brakeforce: f32,
        /// `Axle_Brakeforce_<a>_<s>` and `Axle_Springfactor_<a>_<s>` (absent: left alone).
        axle_brake: Vec<(usize, f32)>,
        spring: Vec<(usize, f32)>,
        steering: f32,
    }

    /// `frames` frames of `dt` of the vehicle: the script's values go in, the frame runs,
    /// the vehicle and its variables come out.
    fn frames(t: &mut Trace, v: &mut VehicleInstance, names: &[String], frames: usize, dt: f32, mut script: impl FnMut(&VehicleInstance) -> Script) {
        for _ in 0..frames {
            let s = script(v);
            v.set_var("M_Wheel", s.m_wheel);
            v.set_var("Brakeforce", s.brakeforce);
            for &(a, b) in &s.axle_brake {
                for side in ["L", "R"] {
                    v.set_var(&format!("Axle_Brakeforce_{a}_{side}"), b);
                }
            }
            for &(a, f) in &s.spring {
                for side in ["L", "R"] {
                    v.set_var(&format!("Axle_Springfactor_{a}_{side}"), f);
                }
            }
            v.set_controls(Controls { steering: s.steering, ..Default::default() });
            let ab: Vec<String> = s.axle_brake.iter().map(|(a, b)| format!("{a}:{}", n32(*b))).collect();
            let sf: Vec<String> = s.spring.iter().map(|(a, f)| format!("{a}:{}", n32(*f))).collect();
            t.frame(dt, &format!("M_Wheel {} Brakeforce {} axle_brake [{}] spring [{}] steering {} street {} temp {}", n32(s.m_wheel), n32(s.brakeforce), ab.join(" "), sf.join(" "), n32(s.steering), n32(v.host.street_cond), n32(v.host.temperature)));
            v.update(dt);
            t.vehicle(v, names);
        }
    }

    fn humps(_x: f64, y: f64, top: f64) -> GroundProbe {
        let z = if y > 30.0 && (y % 9.0) < 0.6 { 0.05 } else { 0.0 };
        step_at(z, top)
    }

    pub fn b1_vehicle_with_script_variables() -> String {
        let mut t = Trace::default();
        let names = var_names(2);
        let mut v = instance(vehicle_type(bus(), 2));
        v.contact = Some(Arc::new(humps) as Arc<dyn Ground>);
        v.enable_rigid_body();
        let drive = |m: f32, steering: f32| move |_: &VehicleInstance| Script { m_wheel: m, steering, ..Default::default() };
        frames(&mut t, &mut v, &names, 60, 1.0 / 60.0, |_| Script { brakeforce: 20_000.0, ..Default::default() });
        frames(&mut t, &mut v, &names, 360, 1.0 / 60.0, drive(7000.0, 0.0));
        frames(&mut t, &mut v, &names, 240, 1.0 / 60.0, drive(3000.0, 0.3));
        // the road freezes over: the grip follows in the same frame
        v.host.street_cond = 1.0;
        v.host.temperature = -5.0;
        frames(&mut t, &mut v, &names, 120, 1.0 / 60.0, drive(9000.0, -0.2));
        v.host.street_cond = 0.0;
        v.host.temperature = 15.0;
        // the rear axle's own brakes, then the vehicle's, then the bellows let down
        frames(&mut t, &mut v, &names, 120, 1.0 / 60.0, |_| Script { axle_brake: vec![(1, 8000.0)], ..Default::default() });
        frames(&mut t, &mut v, &names, 180, 1.0 / 30.0, |_| Script { brakeforce: 40_000.0, ..Default::default() });
        for k in 0..120 {
            let f = 1.0 - 0.5 * k as f32 / 120.0;
            frames(&mut t, &mut v, &names, 1, 1.0 / 60.0, |_| Script { brakeforce: 20_000.0, spring: vec![(0, f), (1, 1.0)], ..Default::default() });
        }
        t.text
    }

    /// A section of `articulation`'s tests' `sections()`: `axles` are (position, driven).
    fn section(axles: &[(f32, bool)], mass: f32) -> Vehicle {
        Vehicle {
            mass,
            moment_of_inertia: [300.0, 80.0, 300.0],
            cog_height: 1.2,
            inv_min_turn_radius: 0.13,
            rot_pnt_long: axles.iter().map(|a| a.0).fold(f32::MAX, f32::min),
            axles: axles.iter().map(|&(long, driven)| Axle { long, max_width: 2.4, min_width: 1.2, wheel_diameter: 0.94, spring: 280.0, max_force: 116.0, damper: 20.0, driven, inertia_inv: 0.0 }).collect(),
            coupling_front: Some(Coupling { pos: [0.0, 5.0, 0.3] }),
            coupling_back: Some(Coupling { pos: [0.0, -5.0, 0.3] }),
            coupling_front_character: Some([52.5, -20.0, 20.0, 1.0]),
            ..Default::default()
        }
    }

    /// A pusher's two sections (`articulation`'s tests' `sections()`): the rear section's
    /// wheels are axle 2 of the vehicle's variables.
    pub fn b2_articulated_vehicle() -> String {
        let mut t = Trace::default();
        let names = var_names(3);
        let front = vehicle_type(section(&[(3.2, false), (-2.6, false)], 8.0), 3);
        let rear = vehicle_type(section(&[(-0.4, true)], 7.2), 3);
        let mut v = instance(front);
        v.contact = Some(Arc::new(flat) as Arc<dyn Ground>);
        v.attach_trailer(rear);
        v.enable_rigid_body();
        frames(&mut t, &mut v, &names, 60, 1.0 / 60.0, |_| Script { brakeforce: 20_000.0, ..Default::default() });
        frames(&mut t, &mut v, &names, 300, 1.0 / 60.0, |_| Script { m_wheel: 9000.0, ..Default::default() });
        frames(&mut t, &mut v, &names, 360, 1.0 / 60.0, |_| Script { m_wheel: 3000.0, steering: 0.8, ..Default::default() });
        frames(&mut t, &mut v, &names, 120, 1.0 / 60.0, |_| Script { axle_brake: vec![(2, 10_000.0)], steering: 0.8, ..Default::default() });
        frames(&mut t, &mut v, &names, 120, 1.0 / 60.0, |_| Script { brakeforce: 40_000.0, ..Default::default() });
        t.text
    }

    pub fn b3_vehicle_crash() -> String {
        let mut t = Trace::default();
        let names = var_names(2);
        let mut v = instance(vehicle_type(bus(), 2));
        v.contact = Some(Arc::new(flat) as Arc<dyn Ground>);
        v.enable_rigid_body();
        let mut wall = Obb::from_box([20.0, 1.0, 3.0, 0.0, 0.0, 1.5], DVec3::new(0.0, 25.0, 0.0), 0.0);
        wall.id = 5;
        v.dynamic_boxes = vec![wall];
        frames(&mut t, &mut v, &names, 60, 1.0 / 60.0, |_| Script { brakeforce: 20_000.0, ..Default::default() });
        frames(&mut t, &mut v, &names, 420, 1.0 / 60.0, |v| Script { m_wheel: if v.motion().speed < 25.0 / 3.6 { 9000.0 } else { 0.0 }, ..Default::default() });
        frames(&mut t, &mut v, &names, 120, 1.0 / 60.0, |_| Script { m_wheel: -6000.0, ..Default::default() });
        t.text
    }

    /// Without a body (the model the player's bus had before the rigid body).
    pub fn b4_kinematic_vehicle() -> String {
        let mut t = Trace::default();
        let names = var_names(2);
        let mut v = instance(vehicle_type(bus(), 2));
        let rise = |_x: f64, y: f64| Some(if y > 40.0 { (y - 40.0) * 0.04 } else { 0.0 });
        v.ground = Some(Arc::new(rise));
        frames(&mut t, &mut v, &names, 60, 1.0 / 60.0, |_| Script { brakeforce: 20_000.0, ..Default::default() });
        frames(&mut t, &mut v, &names, 360, 1.0 / 60.0, |_| Script { m_wheel: 7000.0, ..Default::default() });
        frames(&mut t, &mut v, &names, 240, 1.0 / 60.0, |_| Script { m_wheel: 2000.0, steering: 0.4, ..Default::default() });
        frames(&mut t, &mut v, &names, 180, 1.0 / 60.0, |_| Script { axle_brake: vec![(0, 9000.0), (1, 9000.0)], ..Default::default() });
        t.text
    }

    /// An AI vehicle, two sections (the AI's path: no body, the parts follow on their
    /// coupling): the traffic's `AiBody` follows a lane - 40 m straight, then a bend of 25 m
    /// to the right, up a grade from y = 30 m - and hands the vehicle its pose, and
    /// `update_ai` writes the motion into the scripts' variables.
    pub fn b5_ai_vehicle_follows_a_lane() -> String {
        let lane = |s: f64| {
            if s < 40.0 {
                DVec2::new(0.0, s)
            } else {
                let a = (s - 40.0) / 25.0;
                DVec2::new(25.0 - 25.0 * a.cos(), 40.0 + 25.0 * a.sin())
            }
        };
        let rise = |_x: f64, y: f64| Some(if y > 30.0 { (y - 30.0) * 0.05 } else { 0.0 });
        let at = move |s: f64| {
            let p = lane(s);
            DVec3::new(p.x, p.y, rise(p.x, p.y).unwrap())
        };
        let mut t = Trace::default();
        let names = var_names(3);
        let front = vehicle_type(section(&[(3.2, false), (-2.6, true)], 8.0), 3);
        let rear = vehicle_type(section(&[(-0.4, false)], 7.2), 3);
        let mut body = AiBody::new(&front.def, MotionKind::Road);
        let mut v = instance(front);
        v.ground = Some(Arc::new(rise));
        v.attach_trailer(rear);
        let ground = v.ground.clone();
        let ground = ground.as_ref().map(|g| g.as_ref() as &dyn Fn(f64, f64) -> Option<f64>);
        body.place(&|d| at(d as f64), ground, None, 0.0);
        body.apply(&mut v);
        let dt = 1.0 / 60.0;
        let (mut s, mut speed) = (0.0f64, 0.0f32);
        for k in 0..1200 {
            // up to 30 km/h at 1 m/s², and from 14 s on brought to a stop at 2 m/s²
            let time = k as f32 * dt;
            speed = if time < 14.0 { (speed + dt).min(30.0 / 3.6) } else { (speed - 2.0 * dt).max(0.0) };
            s += (speed * dt) as f64;
            let way = |d: f32| at(s + d as f64);
            body.step(dt, speed, &way, ground, None);
            body.apply(&mut v);
            let frame = AiFrame { speed, odometer: s as f32, steer_deg: body.steer, blinker: if s > 35.0 && s < 80.0 { 2 } else { 0 }, brake: time >= 14.0, ..Default::default() };
            t.frame(dt, &format!("speed {} odometer {} steer {} blinker {} brake {}", n32(frame.speed), n32(frame.odometer), n32(frame.steer_deg), frame.blinker, frame.brake));
            v.update_ai(dt, &frame);
            t.vehicle(&v, &names);
        }
        t.text
    }

    /// Without a body, into a wall: the simple model backs out of it and stops.
    pub fn b6_kinematic_vehicle_crash() -> String {
        let mut t = Trace::default();
        let names = var_names(2);
        let mut v = instance(vehicle_type(bus(), 2));
        v.ground = Some(Arc::new(|_x: f64, _y: f64| Some(0.0)));
        v.collision = Some(Arc::new(CollisionWorld::default()));
        let mut wall = Obb::from_box([20.0, 1.0, 3.0, 0.0, 0.0, 1.5], DVec3::new(0.0, 25.0, 0.0), 0.0);
        wall.id = 5;
        v.dynamic_boxes = vec![wall];
        frames(&mut t, &mut v, &names, 60, 1.0 / 60.0, |_| Script { brakeforce: 20_000.0, ..Default::default() });
        frames(&mut t, &mut v, &names, 420, 1.0 / 60.0, |_| Script { m_wheel: 9000.0, ..Default::default() });
        frames(&mut t, &mut v, &names, 120, 1.0 / 60.0, |_| Script { m_wheel: -6000.0, steering: -0.5, ..Default::default() });
        t.text
    }

    /// B2's articulated bus on ice, into a bend at 13 km/h: the front section holds, and the
    /// rear one, its one axle on its own turning line, holds or slides by the pull of the bend
    /// its joint turns it on.
    pub fn b7_articulated_vehicle_bends_on_ice() -> String {
        let mut t = Trace::default();
        let names = var_names(3);
        let front = vehicle_type(section(&[(3.2, false), (-2.6, false)], 8.0), 3);
        let rear = vehicle_type(section(&[(-0.4, true)], 7.2), 3);
        let mut v = instance(front);
        v.host.street_cond = 1.0;
        v.host.temperature = -5.0;
        v.contact = Some(Arc::new(flat) as Arc<dyn Ground>);
        v.attach_trailer(rear);
        v.enable_rigid_body();
        let hold_13 = |steering: f32| move |v: &VehicleInstance| Script { m_wheel: if v.motion().speed < 13.0 / 3.6 { 3500.0 } else { 0.0 }, steering, ..Default::default() };
        frames(&mut t, &mut v, &names, 60, 1.0 / 60.0, |_| Script { brakeforce: 20_000.0, ..Default::default() });
        frames(&mut t, &mut v, &names, 600, 1.0 / 60.0, hold_13(0.0));
        frames(&mut t, &mut v, &names, 480, 1.0 / 60.0, hold_13(0.7));
        t.text
    }
}

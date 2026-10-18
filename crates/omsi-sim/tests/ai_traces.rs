//! Traces of the AI traffic's motion, the safety net of its refactor (Trilha F, F0): the
//! body that follows a lane (`ai::AiBody` on the road, on rails and in the air), what
//! the planner works out with it to pull out and back in, the kinematic model, the static
//! pose of an AI copy (`ai_rest_offset`) and the frame that hands the motion to the scripts
//! (`VehicleInstance::update_ai`), written out frame by frame.
//!
//! The same contract as `physics_traces.rs`: with `OMSI_PHYSICS_TRACE_DIR` set, each scenario
//! writes `<dir>/<name>.trace`, and `scripts/physics-trace-diff.sh` runs them on a base commit
//! and on the change and compares the two. A change that only moves code leaves every trace
//! as it was, to the bit. Without it, each scenario runs twice and both runs must write the
//! same trace. Numbers are written as `{:?}` writes them (NaN with its bits), so equal text is
//! equal bits. The file uses only the crate's public API: the script copies it as it is into
//! the commit it compares with.
//!
//! The vehicles are synthetic, built here as in `physics_traces.rs`, so that the scenarios run
//! without the game's content.

use glam::{DVec2, DVec3, Vec3};
use omsi_model::Model;
use omsi_script::Program;
use omsi_sim::ai::{
    back_in_ramp, pull_out_ramps, pull_out_room, rotation_point, straight_pull_out, AiBody,
    MotionKind, BACK_IN_LAT_ACCEL, PULL_OUT_ACCEL, PULL_OUT_CLEARANCE, PULL_OUT_WAIT,
};
use omsi_sim::collision::Obb;
use omsi_sim::physics::VehicleMotion;
use omsi_sim::physics::{Ground, GroundProbe};
use omsi_sim::vehicle::AiFrame;
use omsi_sim::{Controls, KinematicModel, SimClock, VehicleHost, VehicleInstance, VehicleType};
use omsi_vehicle::vehicle::Coupling;
use omsi_vehicle::{Axle, Vehicle};
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;

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

fn list32(xs: &[f32]) -> String {
    xs.iter().map(|x| n32(*x)).collect::<Vec<_>>().join(" ")
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

    fn line(&mut self, s: &str) {
        let _ = writeln!(self.text, "{s}");
    }

    /// The AI body as the traffic reads it.
    fn body(&mut self, b: &AiBody) {
        let susp: Vec<String> = b
            .suspension
            .iter()
            .map(|s| format!("{} {}", n32(s[0]), n32(s[1])))
            .collect();
        let _ = writeln!(
            self.text,
            "  ai kind {:?} p {} heading {} steer {} a_long {} a_lat {} pitch {} bank {} suspension [{}]",
            b.kind,
            dv3(b.position),
            n64(b.heading),
            n32(b.steer),
            n32(b.a_long),
            n32(b.a_lat),
            n32(b.pitch_deg),
            n32(b.bank_deg),
            susp.join(", "),
        );
    }

    /// The vehicle as the rest of the game and its scripts see it.
    fn vehicle(&mut self, v: &VehicleInstance, vars: &[String]) {
        let _ = writeln!(
            self.text,
            "  vehicle p {} heading {} pitch {} bank {} speed {} steer {} accel {} a_trans {}",
            dv3(v.position),
            n64(v.heading),
            n32(v.pitch),
            n32(v.bank),
            n32(v.motion().speed),
            n32(v.motion().steer_deg),
            v3(v.motion().accel),
            v3(v.motion().a_trans),
        );
        for (a, axle) in v.motion().wheels.iter().enumerate() {
            for (s, w) in axle.iter().enumerate() {
                let _ = writeln!(
                    self.text,
                    "    model_wheel {a} {s} rot {} rpm {} suspension {}",
                    n32(w.rotation_deg),
                    n32(w.rpm),
                    n32(w.suspension)
                );
            }
        }
        for name in vars {
            let value = v.var(name).map(n32).unwrap_or_else(|| "-".into());
            let _ = writeln!(self.text, "    var {name} {value}");
        }
        for (i, part) in v.trailers.iter().enumerate() {
            let _ = writeln!(
                self.text,
                "  part {} p {} heading {}",
                i + 1,
                dv3(part.position),
                n64(part.heading)
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Running the scenarios

/// Write the scenario's trace where `OMSI_PHYSICS_TRACE_DIR` says; without it, run it
/// twice and compare (and look for a state that left the numbers).
fn check(name: &str, scenario: fn() -> String) {
    let first = scenario();
    assert!(first.lines().count() > 10, "{name}: no trace");
    // (the inputs may be no number, the state never; a `known` line writes down a value that
    // is no number today, as it is)
    let lost = |l: &&str| {
        !l.starts_with("frame")
            && !l.starts_with("known")
            && l.split_whitespace()
                .any(|w| w == "inf" || w == "-inf" || w.starts_with("NaN"))
    };
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
                let (k, (a, b)) = first
                    .lines()
                    .zip(second.lines())
                    .enumerate()
                    .find(|(_, (a, b))| a != b)
                    .unwrap_or((0, ("", "")));
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
    // the road: four ways, flat and up a grade, three vehicles
    a01_straight_flat_car,
    a02_straight_flat_three_axles,
    a03_straight_flat_articulated,
    a04_straight_grade_car,
    a05_straight_grade_three_axles,
    a06_straight_grade_articulated,
    a07_tight_bend_flat_car,
    a08_tight_bend_flat_three_axles,
    a09_tight_bend_flat_articulated,
    a10_tight_bend_grade_car,
    a11_tight_bend_grade_three_axles,
    a12_tight_bend_grade_articulated,
    a13_s_bend_flat_car,
    a14_s_bend_flat_three_axles,
    a15_s_bend_flat_articulated,
    a16_s_bend_grade_car,
    a17_s_bend_grade_three_axles,
    a18_s_bend_grade_articulated,
    a19_lane_change_flat_car,
    a20_lane_change_flat_three_axles,
    a21_lane_change_flat_articulated,
    a22_lane_change_grade_car,
    a23_lane_change_grade_three_axles,
    a24_lane_change_grade_articulated,
    // what the wheels stand on
    a25_ground_faces_under_the_wheels,
    a26_plain_height_sampler,
    a27_bodies_of_odd_files,
    // the planner's pull-out and back-in
    p01_pull_out_and_back_in_curves,
    p02_room_to_pull_out,
    p03_pull_out_round_a_standing_bus,
    p04_pull_out_with_a_car_alongside,
    p05_clearance_from_moving_obstacles,
    // stopping and moving off
    s01_stops_at_a_line_and_moves_off,
    s02_articulated_stops_on_a_grade_and_moves_off,
    // rail and air
    r01_rail_through_a_curve,
    r02_rail_through_a_crossover,
    r03_aircraft_turns_and_climbs,
    // the rotation point
    q01_rotation_point_of_the_files,
    q02_rotation_point_absent,
    q03_rotation_point_zero,
    q04_rotation_point_on_the_rear_axle,
    q05_rotation_point_ahead_of_the_front_axle,
    // the static pose of an AI copy
    h01_rest_height,
    // the kinematic model
    k01_kinematic_model_frame_times,
    k02_kinematic_vehicle_frame_times,
    // the AI frame's script variables
    u01_ai_frame_at_a_stop,
);

mod scenarios {
    use super::*;

    // -- the vehicles --

    fn axle(long: f32, width: f32, spring: f32, damper: f32, driven: bool) -> Axle {
        Axle {
            long,
            max_width: width,
            min_width: width * 0.75,
            wheel_diameter: 0.6,
            spring,
            max_force: 0.0,
            damper,
            driven,
            inertia_inv: 0.0,
        }
    }

    /// A Golf (`ai`'s tests' `golf()`).
    pub fn car() -> Vehicle {
        Vehicle {
            mass: 1.0,
            moment_of_inertia: [1.49, 0.40, 1.56],
            cog_height: 0.4,
            rot_pnt_long: -1.304,
            inv_min_turn_radius: 0.1904,
            axles: vec![
                Axle {
                    wheel_diameter: 0.503,
                    ..axle(1.21, 1.606, 40.0, 2.0, true)
                },
                Axle {
                    wheel_diameter: 0.503,
                    ..axle(-1.304, 1.606, 50.0, 2.0, false)
                },
            ],
            ..Default::default()
        }
    }

    /// A three-axle lorry: a steered front axle and a driven tandem, turning about the
    /// tandem's middle.
    pub fn lorry() -> Vehicle {
        Vehicle {
            mass: 18.0,
            moment_of_inertia: [90.0, 20.0, 100.0],
            cog_height: 1.4,
            rot_pnt_long: -0.4,
            inv_min_turn_radius: 0.085,
            axles: vec![
                Axle {
                    wheel_diameter: 1.0,
                    ..axle(3.6, 2.05, 320.0, 22.0, false)
                },
                Axle {
                    wheel_diameter: 1.0,
                    ..axle(0.25, 1.85, 420.0, 25.0, true)
                },
                Axle {
                    wheel_diameter: 1.0,
                    ..axle(-1.05, 1.85, 420.0, 25.0, true)
                },
            ],
            ..Default::default()
        }
    }

    /// A section of an articulated bus (`physics_traces.rs`' `section()`).
    pub fn section(axles: &[(f32, bool)], mass: f32) -> Vehicle {
        Vehicle {
            mass,
            moment_of_inertia: [300.0, 80.0, 300.0],
            cog_height: 1.2,
            inv_min_turn_radius: 0.13,
            rot_pnt_long: axles.iter().map(|a| a.0).fold(f32::MAX, f32::min),
            axles: axles
                .iter()
                .map(|&(long, driven)| Axle {
                    long,
                    max_width: 2.4,
                    min_width: 1.2,
                    wheel_diameter: 0.94,
                    spring: 280.0,
                    max_force: 116.0,
                    damper: 20.0,
                    driven,
                    inertia_inv: 0.0,
                })
                .collect(),
            coupling_front: Some(Coupling {
                pos: [0.0, 5.0, 0.3],
            }),
            coupling_back: Some(Coupling {
                pos: [0.0, -5.0, 0.3],
            }),
            coupling_front_character: Some([52.5, -20.0, 20.0, 1.0]),
            ..Default::default()
        }
    }

    /// A 12 m solo bus (`physics_traces.rs`' `bus()`).
    pub fn bus() -> Vehicle {
        let axle = |long: f32, spring: f32, max_force: f32, driven: bool| Axle {
            long,
            max_width: 2.4,
            min_width: 1.76,
            wheel_diameter: 0.94,
            spring,
            max_force,
            damper: 20.0,
            driven,
            inertia_inv: 0.0,
        };
        Vehicle {
            mass: 10.2,
            moment_of_inertia: [600.0, 90.0, 400.0],
            cog_height: 1.2,
            rolling_resistance: 1000.0,
            rot_pnt_long: -2.7,
            inv_min_turn_radius: 0.13,
            ai_delta_height: 0.12,
            axles: vec![
                axle(3.238, 240.0, 90.0, false),
                axle(-2.637, 280.0, 116.0, true),
            ],
            bounding_box: Some([2.472, 11.663, 2.491, 0.0, 0.001, 1.622]),
            ..Default::default()
        }
    }

    /// A two-axle tram car on its bogies' pivots.
    fn tram() -> Vehicle {
        Vehicle {
            mass: 20.0,
            moment_of_inertia: [400.0, 60.0, 420.0],
            cog_height: 1.5,
            rot_pnt_long: -4.5,
            axles: vec![
                axle(4.5, 1.435, 600.0, 30.0, true),
                axle(-4.5, 1.435, 600.0, 30.0, true),
            ],
            ..Default::default()
        }
    }

    const AI_VARS: [&str; 10] = [
        "AI",
        "AI_Blinker_L",
        "AI_Blinker_R",
        "AI_Brakelight",
        "AI_Light",
        "AI_Interiorlight",
        "AI_Engine",
        "AI_Scheduled_AtStation",
        "AI_Scheduled_AtStation_Side",
        "TrafficPriorityWarningNeeded",
    ];
    const WHEEL_VARS: [&str; 4] = [
        "Wheel_Rotation",
        "Wheel_RotationSpeed",
        "Axle_Steering",
        "Axle_Suspension",
    ];

    /// The variables the AI frame writes, for `axles` axles.
    fn var_names(axles: usize) -> Vec<String> {
        let mut names: Vec<String> = ["Velocity", "n_Wheel", "A_Trans_X", "A_Trans_Y", "A_Trans_Z"]
            .map(String::from)
            .to_vec();
        names.extend(AI_VARS.map(String::from));
        for a in 0..axles {
            for s in ["L", "R"] {
                names.extend(WHEEL_VARS.map(|v| format!("{v}_{a}_{s}")));
            }
        }
        names
    }

    /// A vehicle type of `def` with no model and a script set that only declares the
    /// variables (no script runs). `suspension_axles`: the axles the model moves with
    /// `Axle_Suspension`.
    fn vehicle_type(def: Vehicle, axles: usize, suspension_axles: Vec<usize>) -> Arc<VehicleType> {
        let mut program = Program::default();
        for n in var_names(axles) {
            program.declare_var(&n);
        }
        // (the physics' own, for the kinematic vehicle)
        for n in ["M_Wheel", "Brakeforce"] {
            program.declare_var(n);
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
            suspension_axles,
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

    #[derive(Clone, Copy)]
    pub enum Kind {
        Car,
        ThreeAxles,
        Articulated,
    }

    /// The vehicle the AI drives (with its rear section), its body and its variables.
    fn ai_vehicle(kind: Kind) -> (VehicleInstance, AiBody, Vec<String>) {
        match kind {
            Kind::Car => {
                let ty = vehicle_type(car(), 2, vec![0, 1]);
                let body = AiBody::new(&ty.def, MotionKind::Road);
                (instance(ty), body, var_names(2))
            }
            Kind::ThreeAxles => {
                let ty = vehicle_type(lorry(), 3, Vec::new());
                let body = AiBody::new(&ty.def, MotionKind::Road);
                (instance(ty), body, var_names(3))
            }
            Kind::Articulated => {
                let front =
                    vehicle_type(section(&[(3.2, false), (-2.6, true)], 8.0), 3, vec![0, 1]);
                let rear = vehicle_type(section(&[(-0.4, false)], 7.2), 3, vec![0]);
                let body = AiBody::new(&front.def, MotionKind::Road);
                let mut v = instance(front);
                v.attach_trailer(rear);
                (v, body, var_names(3))
            }
        }
    }

    // -- the ways: (length, curvature) pieces, heading clockwise from north --

    /// The point `s` metres along a way of pieces (length m, curvature 1/m, + = right),
    /// starting at the origin northwards; beyond its end it runs on straight.
    fn along(pieces: &[(f64, f64)], s: f64) -> DVec2 {
        let (mut p, mut h, mut left) = (DVec2::ZERO, 0.0f64, s);
        for &(len, k) in pieces {
            let l = left.min(len);
            if k.abs() < 1e-12 {
                p += DVec2::new(h.sin(), h.cos()) * l;
            } else {
                let h1 = h + k * l;
                p += DVec2::new((h.cos() - h1.cos()) / k, (h1.sin() - h.sin()) / k);
                h = h1;
            }
            left -= l;
            if left <= 0.0 {
                return p;
            }
        }
        p + DVec2::new(h.sin(), h.cos()) * left
    }

    #[derive(Clone, Copy)]
    pub enum Way {
        Straight,
        /// 20 m straight, a right turn of 12 m radius (a junction), on straight.
        TightBend,
        /// Right, then left, 20 m radius, 45° each.
        SBend,
        /// One lane width (3.5 m) to the left over 30 m, as the planner lays it (smoothstep).
        LaneChange,
    }

    fn way_xy(w: Way, s: f64) -> DVec2 {
        use std::f64::consts::FRAC_PI_4;
        match w {
            Way::Straight => DVec2::new(0.0, s),
            Way::TightBend => along(&[(20.0, 0.0), (12.0 * 2.0 * FRAC_PI_4, 1.0 / 12.0)], s),
            Way::SBend => along(
                &[
                    (20.0, 0.0),
                    (20.0 * FRAC_PI_4, 1.0 / 20.0),
                    (20.0 * FRAC_PI_4, -1.0 / 20.0),
                ],
                s,
            ),
            Way::LaneChange => {
                let t = ((s - 20.0) / 30.0).clamp(0.0, 1.0);
                DVec2::new(-3.5 * t * t * (3.0 - 2.0 * t), s)
            }
        }
    }

    // -- the grounds --

    #[derive(Clone, Copy)]
    pub enum Slope {
        Flat,
        /// 6 % up from 10 m north and 2 % camber across x (the right side lower).
        Grade,
    }

    fn height(g: Slope, x: f64, y: f64) -> f64 {
        match g {
            Slope::Flat => 0.0,
            Slope::Grade => 0.06 * (y - 10.0).max(0.0) - 0.02 * x,
        }
    }

    /// The faces under a point as the ground query reports them: the ground at `height`.
    fn probe(g: Slope) -> impl Fn(f64, f64, f64) -> GroundProbe + Send + Sync + Copy + 'static {
        move |x, y, top| {
            let z = height(g, x, y);
            if z <= top {
                GroundProbe {
                    below: Some(z),
                    above: None,
                }
            } else {
                GroundProbe {
                    below: None,
                    above: Some(z),
                }
            }
        }
    }

    /// The lane `s` metres along: on the ground, its height in the middle of the lane.
    fn lane(w: Way, g: Slope, s: f64) -> DVec3 {
        let p = way_xy(w, s);
        DVec3::new(p.x, p.y, height(g, p.x, p.y))
    }

    // -- driving --

    /// The planner's speed: up to `v_max` at 1.2 m/s², braking at 2.5 m/s² to stand at `stop`
    /// metres. Returns the new speed and whether it brakes.
    fn planner_speed(speed: f32, s: f64, v_max: f32, stop: f64, dt: f32) -> (f32, bool) {
        let left = stop - s;
        if left <= (speed * speed / 5.0) as f64 + 0.05 {
            ((speed - 2.5 * dt).max(0.0), true)
        } else {
            ((speed + 1.2 * dt).min(v_max), false)
        }
    }

    /// One frame of the traffic for one vehicle: the body steps along its way, hands the
    /// vehicle its pose, and the AI frame writes the motion into the scripts' variables.
    #[allow(clippy::too_many_arguments)]
    fn ai_step(
        t: &mut Trace,
        body: &mut AiBody,
        v: &mut VehicleInstance,
        names: &[String],
        dt: f32,
        speed: f32,
        s: f64,
        way: &dyn Fn(f32) -> DVec3,
        ground: Option<&dyn Fn(f64, f64) -> Option<f64>>,
        contact: Option<&dyn Ground>,
        frame: AiFrame,
    ) {
        t.frame(
            dt,
            &format!(
                "speed {} s {} steer {} blinker {} brake {} lights {} at_station {} side {} priority {} engine_off {}",
                n32(speed),
                n64(s),
                n32(frame.steer_deg),
                frame.blinker,
                frame.brake,
                frame.lights,
                frame.at_station,
                n32(frame.at_station_side),
                frame.priority_warning,
                frame.engine_off
            ),
        );
        body.step(dt, speed, way, ground, contact);
        t.body(body);
        body.apply(v);
        let frame = AiFrame {
            steer_deg: body.steer,
            ..frame
        };
        v.update_ai(dt, &frame);
        t.vehicle(v, names);
    }

    /// A vehicle along a way and ground: from a standstill up to `v_max`, braking to stand
    /// at `stop` metres, a second more standing.
    fn road(way: Way, slope: Slope, kind: Kind, v_max: f32, stop: f64) -> String {
        let mut t = Trace::default();
        let (mut v, mut body, names) = ai_vehicle(kind);
        let contact = probe(slope);
        v.contact = Some(Arc::new(contact) as Arc<dyn Ground>);
        body.place(&|d| lane(way, slope, d as f64), None, Some(&contact), 0.0);
        t.line("placed");
        t.body(&body);
        body.apply(&mut v);
        let dt = 1.0 / 60.0;
        let (mut s, mut speed, mut still) = (0.0f64, 0.0f32, 0usize);
        for _ in 0..3600 {
            let (sp, brake) = planner_speed(speed, s, v_max, stop, dt);
            speed = sp;
            s += (speed * dt) as f64;
            let at = s;
            let blinker = match way {
                Way::LaneChange if (10.0..50.0).contains(&s) => 1,
                Way::TightBend if (5.0..40.0).contains(&s) => 2,
                _ => 0,
            };
            let frame = AiFrame {
                speed,
                odometer: s as f32,
                blinker,
                brake,
                lights: true,
                ..Default::default()
            };
            ai_step(
                &mut t,
                &mut body,
                &mut v,
                &names,
                dt,
                speed,
                s,
                &|d| lane(way, slope, at + d as f64),
                None,
                Some(&contact),
                frame,
            );
            still = if speed == 0.0 { still + 1 } else { 0 };
            if still > 60 {
                break;
            }
        }
        t.text
    }

    pub fn a01_straight_flat_car() -> String {
        road(Way::Straight, Slope::Flat, Kind::Car, 50.0 / 3.6, 120.0)
    }
    pub fn a02_straight_flat_three_axles() -> String {
        road(
            Way::Straight,
            Slope::Flat,
            Kind::ThreeAxles,
            50.0 / 3.6,
            120.0,
        )
    }
    pub fn a03_straight_flat_articulated() -> String {
        road(
            Way::Straight,
            Slope::Flat,
            Kind::Articulated,
            50.0 / 3.6,
            120.0,
        )
    }
    pub fn a04_straight_grade_car() -> String {
        road(Way::Straight, Slope::Grade, Kind::Car, 50.0 / 3.6, 120.0)
    }
    pub fn a05_straight_grade_three_axles() -> String {
        road(
            Way::Straight,
            Slope::Grade,
            Kind::ThreeAxles,
            50.0 / 3.6,
            120.0,
        )
    }
    pub fn a06_straight_grade_articulated() -> String {
        road(
            Way::Straight,
            Slope::Grade,
            Kind::Articulated,
            50.0 / 3.6,
            120.0,
        )
    }
    pub fn a07_tight_bend_flat_car() -> String {
        road(Way::TightBend, Slope::Flat, Kind::Car, 20.0 / 3.6, 70.0)
    }
    pub fn a08_tight_bend_flat_three_axles() -> String {
        road(
            Way::TightBend,
            Slope::Flat,
            Kind::ThreeAxles,
            20.0 / 3.6,
            70.0,
        )
    }
    pub fn a09_tight_bend_flat_articulated() -> String {
        road(
            Way::TightBend,
            Slope::Flat,
            Kind::Articulated,
            20.0 / 3.6,
            70.0,
        )
    }
    pub fn a10_tight_bend_grade_car() -> String {
        road(Way::TightBend, Slope::Grade, Kind::Car, 20.0 / 3.6, 70.0)
    }
    pub fn a11_tight_bend_grade_three_axles() -> String {
        road(
            Way::TightBend,
            Slope::Grade,
            Kind::ThreeAxles,
            20.0 / 3.6,
            70.0,
        )
    }
    pub fn a12_tight_bend_grade_articulated() -> String {
        road(
            Way::TightBend,
            Slope::Grade,
            Kind::Articulated,
            20.0 / 3.6,
            70.0,
        )
    }
    pub fn a13_s_bend_flat_car() -> String {
        road(Way::SBend, Slope::Flat, Kind::Car, 35.0 / 3.6, 90.0)
    }
    pub fn a14_s_bend_flat_three_axles() -> String {
        road(Way::SBend, Slope::Flat, Kind::ThreeAxles, 35.0 / 3.6, 90.0)
    }
    pub fn a15_s_bend_flat_articulated() -> String {
        road(Way::SBend, Slope::Flat, Kind::Articulated, 35.0 / 3.6, 90.0)
    }
    pub fn a16_s_bend_grade_car() -> String {
        road(Way::SBend, Slope::Grade, Kind::Car, 35.0 / 3.6, 90.0)
    }
    pub fn a17_s_bend_grade_three_axles() -> String {
        road(Way::SBend, Slope::Grade, Kind::ThreeAxles, 35.0 / 3.6, 90.0)
    }
    pub fn a18_s_bend_grade_articulated() -> String {
        road(
            Way::SBend,
            Slope::Grade,
            Kind::Articulated,
            35.0 / 3.6,
            90.0,
        )
    }
    pub fn a19_lane_change_flat_car() -> String {
        road(Way::LaneChange, Slope::Flat, Kind::Car, 40.0 / 3.6, 100.0)
    }
    pub fn a20_lane_change_flat_three_axles() -> String {
        road(
            Way::LaneChange,
            Slope::Flat,
            Kind::ThreeAxles,
            40.0 / 3.6,
            100.0,
        )
    }
    pub fn a21_lane_change_flat_articulated() -> String {
        road(
            Way::LaneChange,
            Slope::Flat,
            Kind::Articulated,
            40.0 / 3.6,
            100.0,
        )
    }
    pub fn a22_lane_change_grade_car() -> String {
        road(Way::LaneChange, Slope::Grade, Kind::Car, 40.0 / 3.6, 100.0)
    }
    pub fn a23_lane_change_grade_three_axles() -> String {
        road(
            Way::LaneChange,
            Slope::Grade,
            Kind::ThreeAxles,
            40.0 / 3.6,
            100.0,
        )
    }
    pub fn a24_lane_change_grade_articulated() -> String {
        road(
            Way::LaneChange,
            Slope::Grade,
            Kind::Articulated,
            40.0 / 3.6,
            100.0,
        )
    }

    /// The faces along a straight lane at 10 m, piece by piece: the road drawn 8 cm over the
    /// lane and cambered, a lane laid 0.9 m under its road, a junction's lane half a metre
    /// over the drawn road (the car comes down onto it), a road 6 m below (under a bridge the
    /// lane crosses: not this car's ground), the right wheels over the edge of the road, no
    /// face at all, and a road 0.32 m under the lane (just past how far down a wheel's ground
    /// is looked for). Halfway the planner puts the car 20 m further on (it is placed anew).
    pub fn a25_ground_faces_under_the_wheels() -> String {
        fn faces(x: f64, y: f64, top: f64) -> GroundProbe {
            let z = match y {
                y if y < 20.0 => Some(10.08 - 0.03 * x),
                y if y < 40.0 => Some(10.9),
                y if y < 60.0 => Some(9.5),
                y if y < 80.0 => Some(4.0),
                y if y < 100.0 => (x < 0.3).then_some(10.0).or(Some(9.2)),
                y if y < 120.0 => None,
                y if y < 135.0 => Some(9.68),
                _ => Some(10.0),
            };
            match z {
                Some(z) if z <= top => GroundProbe {
                    below: Some(z),
                    above: None,
                },
                Some(z) => GroundProbe {
                    below: Some(9.0),
                    above: Some(z),
                },
                None => GroundProbe {
                    below: None,
                    above: None,
                },
            }
        }
        let mut t = Trace::default();
        let (mut v, mut body, names) = ai_vehicle(Kind::Car);
        v.contact = Some(Arc::new(faces) as Arc<dyn Ground>);
        let way = |s: f64| DVec3::new(0.0, s, 10.0);
        body.place(&|d| way(d as f64), None, Some(&faces), 6.0);
        t.body(&body);
        let dt = 1.0 / 60.0;
        let speed = 6.0f32;
        let mut s = 0.0f64;
        for k in 0..1500 {
            s += (speed * dt) as f64;
            if k == 750 {
                s += 20.0;
            }
            let at = s;
            let frame = AiFrame {
                speed,
                odometer: s as f32,
                lights: k % 300 < 150,
                ..Default::default()
            };
            ai_step(
                &mut t,
                &mut body,
                &mut v,
                &names,
                dt,
                speed,
                s,
                &|d| way(at + d as f64),
                None,
                Some(&faces),
                frame,
            );
        }
        t.text
    }

    /// Without the world's faces: the plain height sampler, which is ignored where it is 3 m
    /// or more off the lane.
    pub fn a26_plain_height_sampler() -> String {
        fn sample(x: f64, y: f64) -> Option<f64> {
            match y {
                y if y < 30.0 => Some(0.05 * x + 0.02 * y),
                y if y < 50.0 => Some(5.0),
                y if y < 60.0 => None,
                _ => Some(0.6 + 0.01 * (y * 0.7).sin()),
            }
        }
        let mut t = Trace::default();
        let (mut v, mut body, names) = ai_vehicle(Kind::ThreeAxles);
        v.ground = Some(Arc::new(sample));
        let way = |s: f64| DVec3::new(0.0, s, if s < 30.0 { 0.02 * s } else { 0.6 });
        body.place(&|d| way(d as f64), Some(&sample), None, 0.0);
        t.body(&body);
        let dt = 1.0 / 60.0;
        let (mut s, mut speed) = (0.0f64, 0.0f32);
        for _ in 0..1500 {
            let (sp, brake) = planner_speed(speed, s, 30.0 / 3.6, 95.0, dt);
            speed = sp;
            s += (speed * dt) as f64;
            let at = s;
            let frame = AiFrame {
                speed,
                odometer: s as f32,
                brake,
                ..Default::default()
            };
            ai_step(
                &mut t,
                &mut body,
                &mut v,
                &names,
                dt,
                speed,
                s,
                &|d| way(at + d as f64),
                Some(&sample),
                None,
                frame,
            );
        }
        t.text
    }

    /// The bodies `AiBody::new` makes of files that leave things out or give them in other
    /// units: kilograms instead of tonnes, no springs, no inertia, no turning radius, no
    /// centre of gravity, springs far too stiff for the inertia, one axle, no axle. Each
    /// drives the S-bend.
    pub fn a27_bodies_of_odd_files() -> String {
        let mut t = Trace::default();
        let kilograms = Vehicle {
            mass: 1350.0,
            moment_of_inertia: [1600.0, 400.0, 1700.0],
            ..car()
        };
        let no_springs = Vehicle {
            axles: car()
                .axles
                .into_iter()
                .map(|a| Axle {
                    spring: 0.0,
                    damper: 0.0,
                    ..a
                })
                .collect(),
            ..car()
        };
        let no_inertia = Vehicle {
            moment_of_inertia: [0.0; 3],
            cog_height: 0.0,
            inv_min_turn_radius: 0.0,
            ..lorry()
        };
        let stiff = Vehicle {
            axles: car()
                .axles
                .into_iter()
                .map(|a| Axle { spring: 400.0, ..a })
                .collect(),
            moment_of_inertia: [0.2, 0.1, 0.05],
            ..car()
        };
        let one_axle = Vehicle {
            axles: vec![axle(0.5, 1.8, 100.0, 5.0, true)],
            ..car()
        };
        let no_axle = Vehicle {
            axles: Vec::new(),
            ..car()
        };
        let tight = Vehicle {
            inv_min_turn_radius: 2.0,
            ..car()
        };
        for (name, def) in [
            ("kilograms", kilograms),
            ("no_springs", no_springs),
            ("no_inertia", no_inertia),
            ("stiff", stiff),
            ("one_axle", one_axle),
            ("no_axle", no_axle),
            ("tight_lock", tight),
        ] {
            t.line(&format!("file {name}"));
            let mut body = AiBody::new(&def, MotionKind::Road);
            let contact = probe(Slope::Grade);
            body.place(
                &|d| lane(Way::SBend, Slope::Grade, d as f64),
                None,
                Some(&contact),
                0.0,
            );
            t.body(&body);
            let dt = 1.0 / 30.0;
            let (mut s, mut speed) = (0.0f64, 0.0f32);
            for _ in 0..600 {
                let (sp, _) = planner_speed(speed, s, 30.0 / 3.6, 75.0, dt);
                speed = sp;
                s += (speed * dt) as f64;
                let at = s;
                t.frame(dt, &format!("speed {} s {}", n32(speed), n64(s)));
                body.step(
                    dt,
                    speed,
                    &|d| lane(Way::SBend, Slope::Grade, at + d as f64),
                    None,
                    Some(&contact),
                );
                t.body(&body);
            }
        }
        t.text
    }

    // -- the planner --

    /// The curves the planner lays out: `pull_out_ramps` for a gap ahead, from a standstill
    /// and rolling; `straight_pull_out` along one of them; `back_in_ramp` at a speed.
    pub fn p01_pull_out_and_back_in_curves() -> String {
        let mut t = Trace::default();
        for rolling in [false, true] {
            for front in [1.8f32, 2.1, 3.6, 6.0] {
                for real in [0.0f32, 0.5, 1.0, 2.0, 3.0, 5.0, 8.0, 12.0, 20.0, 30.0] {
                    t.line(&format!(
                        "pull_out_ramps real {} front {} rolling {rolling} -> [{}]",
                        n32(real),
                        n32(front),
                        list32(&pull_out_ramps(real, front, rolling))
                    ));
                }
            }
        }
        for (side, ramp) in [(3.3f32, 8.0f32), (3.3, 4.0), (-2.5, 12.0), (3.0, 0.05)] {
            let way = straight_pull_out(side, ramp);
            let pts: Vec<String> = (-4..=((ramp + 6.0) as i32 * 2))
                .map(|i| dv3(way(i as f32 * 0.5)))
                .collect();
            t.line(&format!(
                "straight_pull_out side {} ramp {} -> [{}]",
                n32(side),
                n32(ramp),
                pts.join(", ")
            ));
        }
        for offset in [0.5f32, 2.5, 3.28, 5.0, -3.3] {
            for kmh in [0.0f32, 5.0, 20.0, 34.0, 44.0, 60.0, 100.0] {
                for lat in [BACK_IN_LAT_ACCEL, 0.2, 4.0] {
                    t.line(&format!(
                        "back_in_ramp offset {} speed {} lat {} -> {}",
                        n32(offset),
                        n32(kmh / 3.6),
                        n32(lat),
                        n32(back_in_ramp(offset, kmh / 3.6, lat))
                    ));
                }
            }
        }
        t.line(&format!(
            "constants clearance {} wait {} accel {} back_in {}",
            n64(PULL_OUT_CLEARANCE),
            n32(PULL_OUT_WAIT),
            n32(PULL_OUT_ACCEL),
            n32(BACK_IN_LAT_ACCEL)
        ));
        t.text
    }

    /// How far behind a standing bus each vehicle stops to get out round it.
    pub fn p02_room_to_pull_out() -> String {
        let mut t = Trace::default();
        let front = section(&[(3.2, false), (-2.6, true)], 8.0);
        let cases: [(&str, Vehicle, (f32, f32, f32)); 4] = [
            ("car", car(), (2.1, 2.2, 0.85)),
            ("lorry", lorry(), (4.6, 3.4, 1.25)),
            ("articulated", front, (5.6, 5.4, 1.25)),
            ("bus", bus(), (5.9, 5.8, 1.25)),
        ];
        for (name, def, extent) in cases {
            for half_width in [1.25f64, 1.6] {
                for side in [3.3f32, 3.0, 4.5] {
                    t.line(&format!(
                        "pull_out_room {name} obstacle {} side {} -> {}",
                        n64(half_width),
                        n32(side),
                        n32(pull_out_room(&def, extent, half_width, side))
                    ));
                }
            }
        }
        t.text
    }

    /// A car standing `gap` metres behind a standing bus pulls out round it: every frame the
    /// clearance the planner would sweep from here, then the body's step along the curve.
    fn pull_out(gap: f32, alongside: bool) -> String {
        let mut t = Trace::default();
        let def = car();
        let extent = (2.1f32, 2.2f32, 0.85f32);
        let bus = Obb::vehicle(
            DVec2::new(0.0, (extent.0 + gap) as f64 + 6.0),
            0.0,
            6.0,
            6.0,
            1.25,
        );
        let mut obstacles = vec![bus];
        if alongside {
            // a car parked on the left, level with the bus's rear
            obstacles.push(Obb::vehicle(
                DVec2::new(-5.6, (extent.0 + gap) as f64 + 1.0),
                0.0,
                2.1,
                2.2,
                0.85,
            ));
        }
        let ramps = pull_out_ramps(gap, extent.0, false);
        t.line(&format!("ramps [{}]", list32(&ramps)));
        for &ramp in &ramps {
            let way = straight_pull_out(3.3, ramp);
            let mut body = AiBody::new(&def, MotionKind::Road);
            body.place(&way, None, None, 0.0);
            t.line(&format!(
                "ramp {} sweep {}",
                n32(ramp),
                n64(body.sweep_clearance(
                    &way,
                    0.0,
                    PULL_OUT_WAIT,
                    PULL_OUT_ACCEL,
                    8.0,
                    gap + 6.0,
                    extent,
                    &obstacles
                ))
            ));
        }
        let ramp = ramps[0];
        let way = straight_pull_out(3.3, ramp);
        let mut body = AiBody::new(&def, MotionKind::Road);
        body.place(&way, None, None, 0.0);
        let dt = 1.0 / 20.0;
        let (mut x, mut speed) = (0.0f32, 0.0f32);
        for k in 0..240 {
            let at = x;
            let shifted = |d: f32| way(at + d);
            let sweep = body.sweep_clearance(
                &shifted,
                speed,
                PULL_OUT_WAIT,
                PULL_OUT_ACCEL,
                8.0,
                gap + 6.0 - x,
                extent,
                &obstacles,
            );
            let me = Obb::vehicle(
                body.position.truncate(),
                body.heading,
                extent.0 as f64,
                extent.1 as f64,
                extent.2 as f64,
            );
            let gaps: Vec<String> = obstacles.iter().map(|o| n64(me.separation(o))).collect();
            t.frame(dt, &format!("speed {} x {}", n32(speed), n32(x)));
            t.line(&format!(
                "  sweep {} separation [{}]",
                n64(sweep),
                gaps.join(" ")
            ));
            // the driver turns the wheels for `PULL_OUT_WAIT`, then edges out
            if k as f32 * dt >= PULL_OUT_WAIT {
                speed = (speed + PULL_OUT_ACCEL * dt).min(5.0);
            }
            x += speed * dt;
            let at = x;
            body.step(dt, speed, &|d| way(at + d), None, None);
            t.body(&body);
        }
        t.text
    }

    pub fn p03_pull_out_round_a_standing_bus() -> String {
        pull_out(3.0, false)
    }

    pub fn p04_pull_out_with_a_car_alongside() -> String {
        pull_out(3.0, true)
    }

    /// A car behind a bus that pulls away, and a car crossing ahead: every frame the clearance
    /// of a lane change from where the car is, against where they are now.
    pub fn p05_clearance_from_moving_obstacles() -> String {
        let mut t = Trace::default();
        let def = lorry();
        let extent = (4.6f32, 3.4f32, 1.25f32);
        let way = |s: f64| lane(Way::LaneChange, Slope::Flat, s);
        let mut body = AiBody::new(&def, MotionKind::Road);
        body.place(&|d| way(d as f64), None, None, 4.0);
        let dt = 1.0 / 30.0;
        let (mut s, speed) = (0.0f64, 4.0f32);
        for k in 0..450 {
            let time = k as f64 * dt as f64;
            let mut bus = Obb::vehicle(
                DVec2::new(0.0, 24.0 + 1.5 * time * time.min(4.0) / 4.0),
                0.0,
                6.0,
                6.0,
                1.25,
            );
            bus.velocity = DVec2::new(0.0, 0.75 * time.min(4.0));
            let crossing = Obb::vehicle(DVec2::new(-20.0 + 5.0 * time, 45.0), 90.0, 2.1, 2.2, 0.85);
            let obstacles = [bus, crossing];
            let at = s;
            let ahead = |d: f32| way(at + d as f64);
            let sweep = body.sweep_clearance(
                &ahead,
                speed,
                PULL_OUT_WAIT,
                PULL_OUT_ACCEL,
                12.0,
                40.0,
                extent,
                &obstacles,
            );
            t.frame(
                dt,
                &format!(
                    "speed {} s {} bus {} crossing {}",
                    n32(speed),
                    n64(s),
                    n64(obstacles[0].center.y),
                    n64(obstacles[1].center.x)
                ),
            );
            t.line(&format!("  sweep {}", n64(sweep)));
            s += (speed * dt) as f64;
            let at = s;
            body.step(dt, speed, &|d| way(at + d as f64), None, None);
            t.body(&body);
        }
        t.text
    }

    // -- stopping and moving off --

    /// Up to 50 km/h, braking to a stop line at 120 m, three seconds standing, off again.
    fn stop_and_go(kind: Kind, slope: Slope) -> String {
        let mut t = Trace::default();
        let (mut v, mut body, names) = ai_vehicle(kind);
        let contact = probe(slope);
        v.contact = Some(Arc::new(contact) as Arc<dyn Ground>);
        body.place(
            &|d| lane(Way::Straight, slope, d as f64),
            None,
            Some(&contact),
            0.0,
        );
        let dt = 1.0 / 60.0;
        let (mut s, mut speed, mut stood) = (0.0f64, 0.0f32, 0usize);
        let mut stop = 120.0f64;
        for _ in 0..2400 {
            if speed == 0.0 && s > 100.0 && stop < 1000.0 {
                stood += 1;
                if stood > 180 {
                    stop = 10_000.0;
                }
            }
            let (sp, brake) = planner_speed(speed, s, 50.0 / 3.6, stop, dt);
            speed = sp;
            s += (speed * dt) as f64;
            let at = s;
            let standing = speed == 0.0 && stop < 1000.0 && s > 100.0;
            let frame = AiFrame {
                speed,
                odometer: s as f32,
                brake: brake || standing,
                lights: false,
                ..Default::default()
            };
            ai_step(
                &mut t,
                &mut body,
                &mut v,
                &names,
                dt,
                speed,
                s,
                &|d| lane(Way::Straight, slope, at + d as f64),
                None,
                Some(&contact),
                frame,
            );
        }
        t.text
    }

    pub fn s01_stops_at_a_line_and_moves_off() -> String {
        stop_and_go(Kind::Car, Slope::Flat)
    }

    pub fn s02_articulated_stops_on_a_grade_and_moves_off() -> String {
        stop_and_go(Kind::Articulated, Slope::Grade)
    }

    // -- rail and air --

    fn rail(pieces: &'static [(f64, f64)], rise: f64) -> String {
        let mut t = Trace::default();
        let ty = vehicle_type(tram(), 2, Vec::new());
        let mut body = AiBody::new(&ty.def, MotionKind::Rail);
        let mut v = instance(ty);
        let names = var_names(2);
        let track = move |s: f64| {
            let p = along(pieces, s + 20.0);
            DVec3::new(p.x, p.y, rise * (s + 20.0))
        };
        body.place(&|d| track(d as f64), None, None, 0.0);
        t.body(&body);
        let dt = 1.0 / 60.0;
        let (mut s, mut speed) = (0.0f64, 0.0f32);
        for _ in 0..1500 {
            let (sp, brake) = planner_speed(speed, s, 30.0 / 3.6, 110.0, dt);
            speed = sp;
            s += (speed * dt) as f64;
            let at = s;
            let frame = AiFrame {
                speed,
                odometer: s as f32,
                brake,
                lights: true,
                ..Default::default()
            };
            ai_step(
                &mut t,
                &mut body,
                &mut v,
                &names,
                dt,
                speed,
                s,
                &|d| track(at + d as f64),
                None,
                None,
                frame,
            );
        }
        t.text
    }

    /// Two outer axles 9 m apart through a 25 m curve, climbing 2 %.
    pub fn r01_rail_through_a_curve() -> String {
        rail(
            &[
                (40.0, 0.0),
                (25.0 * std::f64::consts::FRAC_PI_2, 1.0 / 25.0),
            ],
            0.02,
        )
    }

    /// Over to the parallel track 4 m to the right through a crossover (two 190 m arcs).
    pub fn r02_rail_through_a_crossover() -> String {
        let a = (1.0f64 - 2.0 / 190.0).acos();
        let pieces: &'static [(f64, f64)] = Box::leak(Box::new([
            (40.0, 0.0),
            (190.0 * a, 1.0 / 190.0),
            (190.0 * a, -1.0 / 190.0),
        ]));
        rail(pieces, 0.0)
    }

    /// An aircraft along a climbing turn of 800 m radius, then level.
    pub fn r03_aircraft_turns_and_climbs() -> String {
        let mut t = Trace::default();
        let def = Vehicle {
            axles: vec![axle(0.0, 4.0, 0.0, 0.0, false)],
            ..car()
        };
        let mut body = AiBody::new(&def, MotionKind::Air);
        let path = |s: f64| {
            let p = along(&[(300.0, 0.0), (800.0 * 1.2, 1.0 / 800.0)], s);
            DVec3::new(p.x, p.y, 300.0 + 0.08 * (s - 300.0).clamp(0.0, 900.0))
        };
        body.place(&|d| path(d as f64), None, None, 70.0);
        t.body(&body);
        let dt = 1.0 / 30.0;
        let mut s = 0.0f64;
        for k in 0..900 {
            // (a frame of no time now and then, as the traffic hands one after a pause)
            let dt = if k % 300 == 299 { 0.0 } else { dt };
            s += (70.0 * dt) as f64;
            let at = s;
            t.frame(dt, &format!("speed 70 s {}", n64(s)));
            body.step(dt, 70.0, &|d| path(at + d as f64), None, None);
            t.body(&body);
        }
        t.text
    }

    // -- the rotation point --

    /// `rotation_point` for the files' `[rot_pnt_long]` against their axles.
    pub fn q01_rotation_point_of_the_files() -> String {
        let mut t = Trace::default();
        let two = |rot: f32| Vehicle {
            rot_pnt_long: rot,
            ..car()
        };
        let cases: Vec<(&str, Vehicle)> = vec![
            ("car", car()),
            ("lorry", lorry()),
            (
                "absent",
                Vehicle {
                    rot_pnt_long: Vehicle::default().rot_pnt_long,
                    ..car()
                },
            ),
            ("zero", two(0.0)),
            ("rear_axle", two(-1.304)),
            ("behind_the_rear_axle", two(-2.5)),
            ("half_a_metre_short_of_the_front_axle", two(0.71)),
            ("just_inside", two(0.709)),
            ("on_the_front_axle", two(1.21)),
            ("ahead_of_the_front_axle", two(3.0)),
            (
                "one_axle",
                Vehicle {
                    axles: vec![axle(0.5, 1.8, 100.0, 5.0, true)],
                    rot_pnt_long: 0.0,
                    ..car()
                },
            ),
            (
                "no_axle",
                Vehicle {
                    axles: Vec::new(),
                    ..car()
                },
            ),
            (
                "short",
                Vehicle {
                    axles: vec![
                        axle(0.3, 1.0, 50.0, 2.0, true),
                        axle(-0.2, 1.0, 50.0, 2.0, false),
                    ],
                    rot_pnt_long: -0.2,
                    ..car()
                },
            ),
        ];
        for (name, def) in cases {
            let (rot, wheelbase) = rotation_point(&def);
            let km = KinematicModel::from_definition(&def);
            // (a file without axles gives the kinematic model an infinite wheelbase: written
            // down as it is, on a line of its own)
            let known = if km.wheelbase.is_finite() {
                ""
            } else {
                "known "
            };
            t.line(&format!(
                "{known}rotation_point {name} -> {} {} kinematic wheelbase {} max_steer {} mass {}",
                n32(rot),
                n32(wheelbase),
                n32(km.wheelbase),
                n32(km.max_steer_deg),
                n32(km.mass_kg)
            ));
        }
        t.text
    }

    /// The car with `[rot_pnt_long]` at `rot` through the tight bend: the body turns about
    /// that line, and `update_ai` turns every axle ahead of it.
    fn turning_about(rot: f32) -> String {
        let mut t = Trace::default();
        let ty = vehicle_type(
            Vehicle {
                rot_pnt_long: rot,
                ..car()
            },
            2,
            vec![0, 1],
        );
        let mut body = AiBody::new(&ty.def, MotionKind::Road);
        let mut v = instance(ty);
        let names = var_names(2);
        let contact = probe(Slope::Flat);
        body.place(
            &|d| lane(Way::TightBend, Slope::Flat, d as f64),
            None,
            Some(&contact),
            0.0,
        );
        let dt = 1.0 / 60.0;
        let (mut s, mut speed) = (0.0f64, 0.0f32);
        for _ in 0..1200 {
            let (sp, brake) = planner_speed(speed, s, 18.0 / 3.6, 60.0, dt);
            speed = sp;
            s += (speed * dt) as f64;
            let at = s;
            let frame = AiFrame {
                speed,
                odometer: s as f32,
                brake,
                ..Default::default()
            };
            ai_step(
                &mut t,
                &mut body,
                &mut v,
                &names,
                dt,
                speed,
                s,
                &|d| lane(Way::TightBend, Slope::Flat, at + d as f64),
                None,
                Some(&contact),
                frame,
            );
        }
        t.text
    }

    pub fn q02_rotation_point_absent() -> String {
        turning_about(Vehicle::default().rot_pnt_long)
    }

    pub fn q03_rotation_point_zero() -> String {
        turning_about(0.0)
    }

    pub fn q04_rotation_point_on_the_rear_axle() -> String {
        turning_about(-1.304)
    }

    pub fn q05_rotation_point_ahead_of_the_front_axle() -> String {
        turning_about(3.0)
    }

    // -- the static pose of an AI copy --

    /// `ai_rest_offset` (`ai_lift` and `[ai_deltaheight]`) with and without the axles the
    /// model moves with `Axle_Suspension`, then standing and rolling on its springs.
    pub fn h01_rest_height() -> String {
        let mut t = Trace::default();
        let no_springs = Vehicle {
            axles: bus()
                .axles
                .into_iter()
                .map(|a| Axle { spring: 0.0, ..a })
                .collect(),
            ..bus()
        };
        let cases: Vec<(&str, Vehicle, Vec<usize>)> = vec![
            ("none", bus(), Vec::new()),
            ("all", bus(), vec![0, 1]),
            ("rear", bus(), vec![1]),
            (
                "no_delta_all",
                Vehicle {
                    ai_delta_height: 0.0,
                    ..bus()
                },
                vec![0, 1],
            ),
            ("no_springs_none", no_springs.clone(), Vec::new()),
            ("no_springs_all", no_springs, vec![0, 1]),
            ("car_all", car(), vec![0, 1]),
        ];
        for (name, def, axles) in cases {
            let ty = vehicle_type(def, 2, axles);
            let mut body = AiBody::new(&ty.def, MotionKind::Road);
            let mut v = instance(ty);
            let (lift, delta) = v.ai_rest_offset();
            t.line(&format!(
                "rest {name} lift {} delta {}",
                n32(lift),
                n32(delta)
            ));
            let names = var_names(2);
            let contact = probe(Slope::Flat);
            body.place(
                &|d| lane(Way::Straight, Slope::Flat, d as f64),
                None,
                Some(&contact),
                0.0,
            );
            let dt = 1.0 / 60.0;
            let (mut s, mut speed) = (0.0f64, 0.0f32);
            for k in 0..240 {
                let (sp, brake) = if k < 60 {
                    (0.0, true)
                } else {
                    planner_speed(speed, s, 30.0 / 3.6, 30.0, dt)
                };
                speed = sp;
                s += (speed * dt) as f64;
                let at = s;
                let frame = AiFrame {
                    speed,
                    odometer: s as f32,
                    brake,
                    ..Default::default()
                };
                ai_step(
                    &mut t,
                    &mut body,
                    &mut v,
                    &names,
                    dt,
                    speed,
                    s,
                    &|d| lane(Way::Straight, Slope::Flat, at + d as f64),
                    None,
                    Some(&contact),
                    frame,
                );
            }
        }
        t.text
    }

    // -- the kinematic model --

    /// The model on its own: frame times over its 0.1 s limit, frames of no time, and the
    /// ordinary ones, driving, steering, braking and backing.
    pub fn k01_kinematic_model_frame_times() -> String {
        let mut t = Trace::default();
        let def = bus();
        let km = KinematicModel::from_definition(&def);
        t.line(&format!(
            "model mass {} rolling {} wheelbase {} max_steer {}",
            n32(km.mass_kg),
            n32(km.rolling_resistance),
            n32(km.wheelbase),
            n32(km.max_steer_deg)
        ));
        let mut m = VehicleMotion::at_rest(&def);
        let dts = [
            1.0 / 60.0,
            0.0,
            0.25,
            1.0 / 60.0,
            0.1,
            0.5,
            0.0,
            1.0 / 30.0,
            2.0,
            1.0 / 60.0,
        ];
        for k in 0..600usize {
            let dt = dts[k % dts.len()];
            let (torque, brake, steer) = match k {
                0..=199 => (12_000.0f32, 0.0f32, 0.0f32),
                200..=349 => (4_000.0, 0.0, 0.6),
                350..=449 => (0.0, 6_000.0, -0.3),
                450..=519 => (-8_000.0, 0.0, 1.5),
                _ => (0.0, 40_000.0, 0.0),
            };
            t.frame(
                dt,
                &format!(
                    "torque {} brake {} steer {}",
                    n32(torque),
                    n32(brake),
                    n32(steer)
                ),
            );
            let (ds, dh) = km.step(
                &mut m,
                steer,
                dt,
                torque,
                &[brake * 0.5, brake * 0.5, brake * 0.5, brake * 0.5],
            );
            let wheels: Vec<String> = m
                .wheels
                .iter()
                .flat_map(|a| a.iter())
                .map(|w| format!("{} {}", n32(w.rotation_deg), n32(w.rpm)))
                .collect();
            t.line(&format!(
                "  motion ds {} dheading {} speed {} steer {} accel {} wheels [{}]",
                n32(ds),
                n32(dh),
                n32(m.speed),
                n32(m.steer_deg),
                v3(m.accel),
                wheels.join(", ")
            ));
        }
        t.text
    }

    /// The vehicle on the kinematic model (`physics_traces.rs`' B4) through the same frame
    /// times: over 0.1 s and none at all.
    pub fn k02_kinematic_vehicle_frame_times() -> String {
        let mut t = Trace::default();
        let ty = vehicle_type(bus(), 2, Vec::new());
        let mut v = instance(ty);
        let names: Vec<String> = ["M_Wheel", "Brakeforce"]
            .map(String::from)
            .into_iter()
            .chain(var_names(2))
            .collect();
        let rise = |_x: f64, y: f64| Some(if y > 40.0 { (y - 40.0) * 0.04 } else { 0.0 });
        v.ground = Some(Arc::new(rise));
        let dts = [
            1.0 / 60.0,
            0.0,
            0.25,
            1.0 / 60.0,
            0.1,
            0.5,
            0.0,
            1.0 / 30.0,
            1.0 / 60.0,
        ];
        for k in 0..540usize {
            let dt = dts[k % dts.len()];
            let (m_wheel, brake, steering) = match k {
                0..=59 => (0.0f32, 20_000.0f32, 0.0f32),
                60..=299 => (7_000.0, 0.0, 0.0),
                300..=449 => (2_000.0, 0.0, 0.5),
                _ => (0.0, 30_000.0, 0.0),
            };
            v.set_var("M_Wheel", m_wheel);
            v.set_var("Brakeforce", brake);
            v.set_controls(Controls {
                steering,
                ..Default::default()
            });
            t.frame(
                dt,
                &format!(
                    "M_Wheel {} Brakeforce {} steering {}",
                    n32(m_wheel),
                    n32(brake),
                    n32(steering)
                ),
            );
            v.update(dt);
            t.vehicle(&v, &names);
        }
        t.text
    }

    // -- the AI frame --

    /// The bus up to a stop with its blinker and brake, standing there (doors on one side,
    /// then the other, then both; the engine off), told to go, on with its hazard lights and
    /// the warning of a vehicle with right of way, backing, a jump of its odometer, a wheel
    /// angle beyond the axles' limit, a frame of no time, and frames without its visuals.
    pub fn u01_ai_frame_at_a_stop() -> String {
        let mut t = Trace::default();
        let front = vehicle_type(section(&[(3.2, false), (-2.6, true)], 8.0), 3, vec![0, 1]);
        let rear = vehicle_type(section(&[(-0.4, false)], 7.2), 3, vec![0]);
        let mut v = instance(front);
        v.attach_trailer(rear);
        let names = var_names(3);
        let mut odometer = 0.0f32;
        let mut speed = 10.0f32;
        for k in 0..900usize {
            let mut dt = 1.0 / 60.0;
            let mut f = AiFrame {
                lights: true,
                ..Default::default()
            };
            match k {
                0..=239 => {
                    speed = (10.0 - 2.5 * k as f32 / 60.0).max(0.0);
                    f.blinker = 2;
                    f.brake = true;
                    f.steer_deg = 2.0;
                }
                240..=299 => {
                    speed = 0.0;
                    f.at_station = 1;
                    f.brake = true;
                }
                300..=359 => {
                    f.at_station = 1;
                    f.at_station_side = 1.0;
                    f.engine_off = true;
                    f.brake = true;
                }
                360..=419 => {
                    f.at_station = 1;
                    f.at_station_side = 2.0;
                    f.engine_off = k < 400;
                    f.brake = true;
                }
                420..=449 => f.at_station = -1,
                450..=599 => {
                    speed = ((k - 450) as f32 / 60.0 * 1.2).min(8.0);
                    f.blinker = 3;
                    f.priority_warning = k % 40 < 20;
                    f.steer_deg = -6.0;
                }
                600..=659 => {
                    speed = -1.5;
                    f.steer_deg = 12.0;
                }
                660 => {
                    speed = 5.0;
                    odometer += 80.0;
                }
                661..=719 => {
                    speed = 5.0;
                    // (beyond the axles' 1.2 rad)
                    f.steer_deg = 85.0;
                }
                720 => {
                    speed = 5.0;
                    dt = 0.0;
                }
                _ => {
                    speed = 6.0;
                    f.steer_deg = -20.0;
                }
            }
            odometer += speed * dt;
            if k == 700 {
                // an odometer that went back a long way (a vehicle moved for reuse)
                odometer -= 120.0;
            }
            v.ai_visuals = !(780..840).contains(&k);
            f.speed = speed;
            f.odometer = odometer;
            t.frame(dt, &format!("speed {} odometer {} steer {} blinker {} brake {} at_station {} side {} engine_off {} priority {} visuals {}", n32(f.speed), n32(f.odometer), n32(f.steer_deg), f.blinker, f.brake, f.at_station, n32(f.at_station_side), f.engine_off, f.priority_warning, v.ai_visuals));
            v.update_ai(dt, &f);
            t.vehicle(&v, &names);
            t.line(&format!("  station_released {}", v.station_released()));
        }
        t.text
    }
}

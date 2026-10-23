use crate::physics::cases::run;
use crate::support::ground::road;
use crate::support::vehicles::{bus, gn92};
use glam::{DVec3, Vec3};
use omsi_sim::testing::physics::domain::{BodyInputs, GroundProbe, Joint, RigidBody};
use omsi_sim::testing::physics::implementation::train::*;
use omsi_sim::testing::physics::RigidBodyInternals;
use omsi_vehicle::Vehicle;

struct Train {
    bodies: Vec<RigidBody>,
    joints: Vec<Joint>,
    /// The largest gap at the joint so far, and the angles at the end of the last step.
    gap: f32,
    alpha: f32,
    beta: f32,
}

impl Train {
    fn new(front: &Vehicle, rear: &Vehicle) -> Train {
        let mut bodies = vec![
            RigidBody::from_definition(front, &[]),
            RigidBody::from_definition(rear, &[]),
        ];
        let back = Vec3::from(front.coupling_back.as_ref().unwrap().pos);
        let at = Vec3::from(rear.coupling_front.as_ref().unwrap().pos);
        let joints = vec![Joint::new(0, 1, rear, back, at)];
        support_rest_loads(&mut bodies, &joints, &[front, rear]);
        place_train(&mut bodies, &joints, DVec3::ZERO, &[0.0, 0.0]);
        Train {
            bodies,
            joints,
            gap: 0.0,
            alpha: 0.0,
            beta: 0.0,
        }
    }

    /// `secs` at 60 frames a second: `torque` from the speed it is at, brakes per wheel of
    /// the front and the rear section, steering from the time.
    fn run(
        &mut self,
        secs: f32,
        torque: &dyn Fn(f32) -> f32,
        brake: [f32; 2],
        steer: &dyn Fn(f32) -> f32,
        g: &dyn Fn(f64, f64, f64) -> GroundProbe,
    ) {
        let (b0, b1) = (
            vec![brake[0]; self.bodies[0].wheels().len()],
            vec![brake[1]; self.bodies[1].wheels().len()],
        );
        for k in 0..(secs * 60.0).round() as usize {
            let t = k as f32 / 60.0;
            let m = torque(self.bodies[0].forward_speed());
            let inputs = [
                BodyInputs {
                    brake: &b0,
                    steer: Some(steer(t)),
                    air: true,
                },
                BodyInputs {
                    brake: &b1,
                    steer: None,
                    air: false,
                },
            ];
            step_train(&mut self.bodies, &self.joints, 1.0 / 60.0, m, &inputs, g);
            assert!(self
                .bodies
                .iter()
                .all(|b| b.position().is_finite() && b.velocity().is_finite()));
            self.gap = self.gap.max(self.joints[0].gap(&self.bodies));
            (self.alpha, self.beta) = self.joints[0].angles(&self.bodies);
        }
    }

    fn front(&self) -> &RigidBody {
        &self.bodies[0]
    }
}

/// The torque (N m at a 0.47 m wheel) that keeps a train of `mass` kg near `v` m/s.
fn cruise(v: f32, mass: f32) -> impl Fn(f32) -> f32 {
    move |at: f32| ((v - at) * mass * 1.5 + 0.012 * mass * 9.81 * v.signum()) * 0.47
}

fn none(_: f32) -> f32 {
    0.0
}

/// Humps 6 cm high and 0.6 m long every 7 m.
fn humps(_x: f64, y: f64, top: f64) -> GroundProbe {
    let z = if y > 10.0 && (y % 7.0) < 0.6 {
        0.06
    } else {
        0.0
    };
    if z <= top {
        GroundProbe {
            below: Some(z),
            above: None,
        }
    } else {
        GroundProbe {
            below: Some(0.0),
            above: Some(z),
        }
    }
}

/// A grade of `s` from y = 20 on.
fn ramp(s: f64) -> impl Fn(f64, f64, f64) -> GroundProbe {
    move |_x, y, top| {
        let z = (y - 20.0).max(0.0) * s;
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

/// Two bodies of a train without a joint, both steered, both in the air's way, step exactly
/// as each would alone: the train takes nothing from the calculation of a body but the
/// source of its curvature and of the air (the drive is shared by the wheels of both).
#[test]
fn a_follower_without_a_joint_is_the_leader_again() {
    let mut lone = RigidBody::from_definition(&bus(), &[]);
    lone.place(DVec3::ZERO, 0.0);
    let mut pair = vec![lone.clone(), lone.clone()];
    let g = road(30.0, 0.05);
    for k in 0..900 {
        let t = k as f32 / 60.0;
        let torque = if t < 6.0 { 6000.0 } else { 0.0 };
        let brake = [if t > 10.0 { 15_000.0 } else { 0.0 }; 4];
        let steer = (t * 0.8).sin() * 0.7;
        lone.step(1.0 / 60.0, torque, &brake, steer, &g);
        let inputs = [BodyInputs {
            brake: &brake,
            steer: Some(steer),
            air: true,
        }; 2];
        step_train(&mut pair, &[], 1.0 / 60.0, 2.0 * torque, &inputs, &g);
        for b in &pair {
            assert_eq!(
                (b.position(), b.orientation(), b.velocity(), b.omega()),
                (
                    lone.position(),
                    lone.orientation(),
                    lone.velocity(),
                    lone.omega()
                ),
                "at {t} s"
            );
            assert!(b
                .wheels()
                .iter()
                .zip(lone.wheels())
                .all(|(a, b)| a.compression() == b.compression() && a.spin() == b.spin()));
        }
    }
    assert!(lone.origin().y > 30.0, "{:?}", lone.origin());
}

/// The joint holds: weaving at 40 km/h, braking hard from there and over a run of humps
/// at 30 km/h, the two balls stay within 2 cm of each other.
#[test]
fn the_joint_stays_closed() {
    let (front, rear) = gn92(false);
    let mass = 15_200.0;
    let flat = road(1e9, 0.0);
    let mut t = Train::new(&front, &rear);
    t.run(1.0, &none, [5000.0; 2], &none, &flat);
    t.gap = 0.0;
    t.run(12.0, &cruise(40.0 / 3.6, mass), [0.0; 2], &none, &flat);
    t.run(
        20.0,
        &cruise(40.0 / 3.6, mass),
        [0.0; 2],
        &|s| (s * 0.7).sin() * 0.5,
        &flat,
    );
    assert!(
        t.front().forward_speed() > 9.0,
        "{}",
        t.front().forward_speed()
    );
    assert!(t.gap < 0.02, "weaving: the joint opened {} m", t.gap);
    t.run(5.0, &none, [20_000.0; 2], &none, &flat);
    assert!(
        t.front().forward_speed().abs() < 0.1,
        "{}",
        t.front().forward_speed()
    );
    assert!(t.gap < 0.02, "braking: the joint opened {} m", t.gap);
    let mut t = Train::new(&front, &rear);
    t.run(1.0, &none, [5000.0; 2], &none, &humps);
    t.gap = 0.0;
    t.run(15.0, &cruise(30.0 / 3.6, mass), [0.0; 2], &none, &humps);
    assert!(t.front().origin().y > 60.0, "{:?}", t.front().origin());
    assert!(t.gap < 0.02, "humps: the joint opened {} m", t.gap);
}

/// Standing, the front section's rear axle carries the share of the rear section the
/// joint holds (some 6 kN of the GN92's 71 kN, the rest on its one axle), which lifts the
/// front axle a little and pitches the front section nose up.
#[test]
fn the_joint_weighs_on_the_front_sections_rear_axle() {
    let (front, rear) = gn92(false);
    let flat = road(1e9, 0.0);
    let mut t = Train::new(&front, &rear);
    t.run(4.0, &none, [5000.0; 2], &none, &flat);
    let mut lone = RigidBody::from_definition(&front, &[]);
    lone.place(DVec3::ZERO, 0.0);
    run(&mut lone, 4.0, 0.0, 5000.0, &flat);
    let axle = |b: &RigidBody, a: usize| {
        b.wheels()
            .iter()
            .zip(b.wheel_axle())
            .filter(|(_, x)| **x == a)
            .map(|(w, _)| w.load())
            .sum::<f32>()
    };
    let (gain_rear, gain_front) = (
        axle(t.front(), 1) - axle(&lone, 1),
        axle(t.front(), 0) - axle(&lone, 0),
    );
    eprintln!("the joint on the front section: rear axle {gain_rear:+.0} N, front axle {gain_front:+.0} N");
    assert!(
        gain_rear > 5000.0 && gain_rear < 11_000.0,
        "rear axle took {gain_rear} N more"
    );
    assert!(gain_front < 0.0, "front axle took {gain_front} N more");
    let carried = axle(&t.bodies[1], 0);
    let weight = 7200.0 * 9.81;
    assert!(
        (weight - carried - (gain_rear + gain_front)).abs() < 1000.0,
        "rear axle {carried} N, front section {} N",
        gain_rear + gain_front
    );
    let (_, pitch, _) = t.front().heading_pitch_bank();
    let (_, lone_pitch, _) = lone.heading_pitch_bank();
    assert!(
        pitch > lone_pitch + 0.03,
        "pitch {pitch} alone {lone_pitch}"
    );
    assert!(t.gap < 0.02, "{}", t.gap);
}

/// The rear section's mass comes back to the front through the joint: braking on the
/// front section's wheels alone, the train slows much less than the front section alone
/// and dips differently; a heavier rear section with more inertia turns and rolls the
/// front differently in the same bend, and so does a joint that lets it roll on its own.
#[test]
fn the_rear_sections_mass_and_inertia_reach_the_front() {
    let (front, rear) = gn92(false);
    let flat = road(1e9, 0.0);
    let start = |rear: &Vehicle| {
        let mut t = Train::new(&front, rear);
        t.run(1.0, &none, [5000.0; 2], &none, &flat);
        let m = t.bodies.iter().map(|b| b.mass()).sum::<f32>();
        t.run(10.0, &cruise(40.0 / 3.6, m), [0.0; 2], &none, &flat);
        t
    };
    // braking
    let mut t = start(&rear);
    let v0 = t.front().forward_speed();
    let mut dip = 0.0f32;
    for _ in 0..30 {
        t.run(1.0 / 60.0, &none, [12_000.0, 0.0], &none, &flat);
        dip = dip.min(t.front().heading_pitch_bank().1);
    }
    let train_decel = (v0 - t.front().forward_speed()) * 2.0;
    let mut lone = RigidBody::from_definition(&front, &[]);
    lone.place(DVec3::ZERO, 0.0);
    run(&mut lone, 1.0, 0.0, 5000.0, &flat);
    lone.set_velocity(Vec3::Y * v0);
    lone.wheels_mut()
        .iter_mut()
        .for_each(|w| w.set_spin(v0 / w.radius()));
    let mut lone_dip = 0.0f32;
    for _ in 0..30 {
        lone.step(1.0 / 60.0, 0.0, &[12_000.0; 4], 0.0, &flat);
        lone_dip = lone_dip.min(lone.heading_pitch_bank().1);
    }
    let lone_decel = (v0 - lone.forward_speed()) * 2.0;
    eprintln!("braking on the front: train {train_decel:.2} m/s², front alone {lone_decel:.2}; dip {dip:.2}° / {lone_dip:.2}°");
    assert!(
        train_decel < 0.7 * lone_decel,
        "train {train_decel}, alone {lone_decel}"
    );
    assert!((dip - lone_dip).abs() > 0.05, "dip {dip} alone {lone_dip}");
    assert!(t.gap < 0.02, "{}", t.gap);
    // a step of the steering at 40 km/h, rear sections of different mass and inertia
    let bend = |rear: &Vehicle, friction: f32| {
        let mut t = start(rear);
        t.bodies.iter_mut().for_each(|b| b.set_friction(friction));
        let (mut yaw, mut roll) = (0.0f32, 0.0f32);
        let m = t.bodies.iter().map(|b| b.mass()).sum::<f32>();
        for _ in 0..90 {
            t.run(
                1.0 / 60.0,
                &cruise(40.0 / 3.6, m),
                [0.0; 2],
                &|_| 0.4,
                &flat,
            );
            yaw = yaw.max(t.front().omega().z.abs());
            roll = roll.max(t.front().heading_pitch_bank().2.abs());
        }
        assert!(t.gap < 0.02, "{}", t.gap);
        (yaw, roll)
    };
    let light = Vehicle {
        mass: 3.0,
        moment_of_inertia: [60.0, 15.0, 60.0],
        ..rear.clone()
    };
    let heavy = Vehicle {
        mass: 12.0,
        moment_of_inertia: [250.0, 80.0, 250.0],
        ..rear.clone()
    };
    let ball = Vehicle {
        coupling_front_character: Some([52.5, -20.0, 20.0, 0.0]),
        ..rear.clone()
    };
    // (on a dry road the front follows its wheels, as OMSI 2's holds it: the yaw is the
    // steering's; on a wet one its tyres slide and the rear section's inertia shows)
    let ((_, rl), (_, rh)) = (bend(&light, 0.85), bend(&heavy, 0.85));
    let ((yl, _), (yh, _)) = (bend(&light, 0.25), bend(&heavy, 0.25));
    let ((_, ra), (_, rb)) = (bend(&rear, 0.85), bend(&ball, 0.85));
    eprintln!("step steer: roll {rl:.2}° / {rh:.2}°, sliding yaw rate {yl:.3} / {yh:.3} rad/s (light / heavy rear); roll with the turntable {ra:.2}°, with a ball {rb:.2}°");
    assert!(rh > 1.2 * rl, "roll {rl} {rh}");
    assert!((yl - yh).abs() > 0.05 * yl.max(yh), "yaw rate {yl} {yh}");
    assert!(
        ra > rb + 0.2,
        "roll with the turntable {ra}, with a ball {rb}"
    );
}

/// A pusher: the front section has no driven wheel and is pushed through the joint, the
/// train running straight; the brakes of the rear section alone stop it.
#[test]
fn a_pusher_pushes_its_front_section() {
    let (front, rear) = gn92(false);
    let flat = road(1e9, 0.0);
    let mut t = Train::new(&front, &rear);
    t.run(1.0, &none, [5000.0; 2], &none, &flat);
    t.run(8.0, &|_| 9000.0, [0.0; 2], &none, &flat);
    let v = t.front().forward_speed();
    let (heading, _, _) = t.front().heading_pitch_bank();
    assert!(v > 3.0, "pushed to {v} m/s");
    assert!(
        heading.min(360.0 - heading) < 0.5 && t.alpha.abs() < 0.01,
        "heading {heading}, alpha {}",
        t.alpha
    );
    t.run(4.0, &none, [0.0, 30_000.0], &none, &flat);
    assert!(
        t.front().forward_speed().abs() < 0.1,
        "{}",
        t.front().forward_speed()
    );
    assert!(t.gap < 0.02, "{}", t.gap);
}

/// Reversing on full lock folds a pusher up until its joint stops at
/// `[coupling_front_character]`'s 52.5 degrees; it goes no further, and the joint holds.
#[test]
fn the_joint_stops_at_its_max_alpha() {
    let (front, rear) = gn92(false);
    let flat = road(1e9, 0.0);
    let mut t = Train::new(&front, &rear);
    t.run(1.0, &none, [5000.0; 2], &none, &flat);
    let mut most = 0.0f32;
    for _ in 0..(25 * 4) {
        t.run(0.25, &cruise(-1.5, 15_200.0), [0.0; 2], &|_| 1.0, &flat);
        most = most.max(t.alpha.abs());
    }
    eprintln!(
        "reversing on full lock: alpha up to {:.1}°",
        most.to_degrees()
    );
    assert!(most.to_degrees() > 45.0, "alpha only {}", most.to_degrees());
    assert!(
        most.to_degrees() < 52.5 + 1.5,
        "alpha {}",
        most.to_degrees()
    );
    assert!(t.gap < 0.03, "{}", t.gap);
}

/// Beta stays within the file's range: with a joint that bends 2 degrees up and down the
/// rear section is held in line with the front as it drives onto a 10 % grade, its wheel
/// lifted rather than bending the joint.
#[test]
fn beta_stays_within_its_range() {
    let (front, rear) = gn92(false);
    let stiff = Vehicle {
        coupling_front_character: Some([52.5, -2.0, 2.0, 1.0]),
        ..rear.clone()
    };
    for (rear, limit) in [(&rear, 20.0f32), (&stiff, 2.0)] {
        let g = ramp(0.10);
        let mut t = Train::new(&front, rear);
        t.run(1.0, &none, [5000.0; 2], &none, &g);
        let mut most = 0.0f32;
        for _ in 0..(16 * 4) {
            t.run(0.25, &cruise(3.0, 15_200.0), [0.0; 2], &none, &g);
            most = most.max(t.beta.abs());
        }
        eprintln!(
            "onto a 10 % grade with ±{limit}°: beta up to {:.2}°",
            most.to_degrees()
        );
        assert!(t.front().origin().y > 35.0, "{:?}", t.front().origin());
        assert!(
            most.to_degrees() < limit + 0.5,
            "beta {}",
            most.to_degrees()
        );
        assert!(t.gap < 0.02, "{}", t.gap);
    }
}

/// A pusher's joint that pitches about a hinge 0.8 m ahead of the ball holds over humps.
#[test]
fn a_hinge_ahead_of_the_ball_holds() {
    let (front, rear) = gn92(false);
    let rear = Vehicle {
        coupling_pitch_offset: Some(0.8),
        ..rear
    };
    let mut t = Train::new(&front, &rear);
    t.run(1.0, &none, [5000.0; 2], &none, &humps);
    t.gap = 0.0;
    t.run(
        15.0,
        &cruise(30.0 / 3.6, 15_200.0),
        [0.0; 2],
        &|s| (s * 0.5).sin() * 0.3,
        &humps,
    );
    assert!(t.front().origin().y > 50.0, "{:?}", t.front().origin());
    assert!(t.gap < 0.02, "{}", t.gap);
}

/// A file without `[coupling_front_character]` gives the joint no limits and no turntable:
/// a ball free about every axis. A file's angles are taken as it gives them.
#[test]
fn a_joint_without_coupling_front_character_is_a_free_ball() {
    use omsi_sim::testing::physics::domain::JointKind;
    let (_, rear) = gn92(false);
    let without = Vehicle {
        coupling_front_character: None,
        ..rear.clone()
    };
    let j = Joint::new(0, 1, &without, Vec3::ZERO, Vec3::ZERO);
    assert_eq!(j.kind, JointKind::Ball);
    assert_eq!((j.alpha_max, j.beta_min, j.beta_max), (0.0, 0.0, 0.0));
    let stiff = Vehicle {
        coupling_front_character: Some([0.0, 5.0, 5.0, 1.0]),
        ..rear
    };
    let j = Joint::new(0, 1, &stiff, Vec3::ZERO, Vec3::ZERO);
    assert_eq!(j.kind, JointKind::Articulation);
    assert_eq!(j.alpha_max, 0.0);
    assert_eq!(j.beta_min, 5f32.to_radians());
}

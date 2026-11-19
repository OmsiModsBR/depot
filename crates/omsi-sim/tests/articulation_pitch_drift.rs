//! Parity case: a pusher's rear section must not drift in pitch against the front section as
//! the bus weaves (the joint's yaw working) and comes back to rest straight on flat ground.
//!
//! Observed in game (a Depot-physics articulated bus with a pusher joint): after weaving, the
//! rear section's pitch walks away from the front's, a little more each time, and stays there
//! until the game restarts. Black-box expectation: same flat ground, same straight rest, same
//! relative pitch (the sections' relative pitch at rest is fixed by the file's geometry).
//!
//! The case is made from the model alone (a synthetic bus), no reference material.

mod support;

use glam::{DVec3, Vec3};
use omsi_sim::physics::{
    place_train, step_train, support_rest_loads, BodyInputs, Engine, GroundProbe, Joint, RigidBody,
};
use omsi_vehicle::Vehicle;
use support::vehicles::gn92;

fn flat(_x: f64, _y: f64, _top: f64) -> GroundProbe {
    GroundProbe {
        below: Some(0.0),
        above: None,
    }
}

/// Relative pitch (deg) of the sections after each of `cycles` rounds of weaving at 30 km/h
/// for 14 s and standing braked on straight ground for 10 s; index 0 is the first rest.
fn beta_after_weaving(engine: Engine, pitch_offset: Option<f32>, cycles: usize) -> Vec<f32> {
    let (front, rear) = gn92(false);
    let rear = Vehicle {
        coupling_pitch_offset: pitch_offset,
        ..rear
    };
    let mut bodies = vec![
        RigidBody::for_engine(&front, &[], engine),
        RigidBody::for_engine(&rear, &[], engine),
    ];
    let back = Vec3::from(front.coupling_back.as_ref().unwrap().pos);
    let at = Vec3::from(rear.coupling_front.as_ref().unwrap().pos);
    let joints = vec![Joint::new(0, 1, &rear, back, at)];
    support_rest_loads(&mut bodies, &joints, &[&front, &rear]);
    place_train(&mut bodies, &joints, DVec3::ZERO, &[0.0, 0.0]);
    let none = [
        vec![0.0; bodies[0].wheels().len()],
        vec![0.0; bodies[1].wheels().len()],
    ];
    let held = [
        vec![5000.0; bodies[0].wheels().len()],
        vec![5000.0; bodies[1].wheels().len()],
    ];
    let mut t = 0.0f32;
    let mut stage = |bodies: &mut Vec<RigidBody>, secs: f32, weave: bool| {
        for _ in 0..(secs * 60.0) as usize {
            let v = bodies[0].forward_speed();
            let m = if weave {
                ((30.0 / 3.6 - v) * 15_200.0 * 1.5 + 0.012 * 15_200.0 * 9.81) * 0.47
            } else {
                0.0
            };
            let steer = if weave { (t * 0.5).sin() * 0.3 } else { 0.0 };
            let brake = if weave { &none } else { &held };
            let inputs = [
                BodyInputs {
                    brake: &brake[0],
                    steer: Some(steer),
                    air: true,
                },
                BodyInputs {
                    brake: &brake[1],
                    steer: None,
                    air: false,
                },
            ];
            step_train(bodies, &joints, 1.0 / 60.0, m, &inputs, &flat);
            t += 1.0 / 60.0;
        }
    };
    let beta = |bodies: &Vec<RigidBody>| joints[0].angles(bodies).1.to_degrees();
    let mut out = Vec::new();
    stage(&mut bodies, 6.0, false);
    out.push(beta(&bodies));
    for _ in 0..cycles {
        stage(&mut bodies, 14.0, true);
        stage(&mut bodies, 10.0, false);
        out.push(beta(&bodies));
    }
    out
}

/// A joint with no hinge offset (a puller's ball) keeps its rest pitch: the control.
#[test]
fn a_ball_joint_keeps_its_rest_pitch_after_weaving() {
    for engine in [Engine::Classic, Engine::Depot] {
        let b = beta_after_weaving(engine, None, 6);
        let drift = (b[b.len() - 1] - b[0]).abs();
        assert!(drift < 0.05, "{engine:?}: beta walked {drift} deg: {b:?}");
    }
}

/// A pusher (hinge ahead of the ball) must too. It used to fail: the rear section's pitch walked
/// some 0.03 deg per round of weaving, in both engines, and never comes back (fixed: the
/// pusher's link is held to the follower's pitch, see `Joint::ball`).
#[test]
fn a_pushers_joint_keeps_its_rest_pitch_after_weaving() {
    for engine in [Engine::Classic, Engine::Depot] {
        let b = beta_after_weaving(engine, Some(0.8), 12);
        let drift = (b[b.len() - 1] - b[0]).abs();
        assert!(drift < 0.05, "{engine:?}: beta walked {drift} deg: {b:?}");
    }
}

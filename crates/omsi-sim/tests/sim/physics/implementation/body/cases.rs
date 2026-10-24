/// The body's pitch and roll are damped as much a second at any frame rate: as OMSI
/// damps them at 30 frames a second (a 12 t bus on 240-280 kN/m springs).
#[test]
fn body_damping_does_not_depend_on_the_frame_rate() {
    let f = ((2.0 * 240.0 + 2.0 * 280.0) / 12.0f32).sqrt();
    let second = |fps: f32| {
        (0..fps as usize).fold(1.0f32, |k, _| {
            k * omsi_sim::testing::physics::implementation::body::body_damping(f, 1.0 / fps)
        })
    };
    let (a, b, c) = (second(30.0), second(60.0), second(144.0));
    assert!((a - b).abs() < 1e-3 && (a - c).abs() < 1e-3, "{a} {b} {c}");
    assert!(a > 0.2 && a < 0.5, "{a}");
}

use crate::physics::cases::run;
use crate::support::ground::flat;
use crate::support::vehicles::bus;
use glam::DVec3;
use omsi_sim::testing::physics::RigidBody;

/// A bus carrying `kg`, standing on flat ground for a second (its springs settled).
fn loaded(kg: f32) -> RigidBody {
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.set_payload(kg);
    rb.place(DVec3::ZERO, 0.0);
    run(&mut rb, 3.0, 0.0, 0.0, &flat);
    rb
}

/// What a body carries weighs as its own mass: the mass it moves with is the file's and the
/// payload's, and its pitch and roll frequency is that of the heavier body on the same
/// springs. Nothing aboard, or a payload that is no mass, leaves the empty body.
#[test]
fn a_payload_adds_to_the_body_mass() {
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    let (m0, f0) = (rb.mass(), rb.body_freq());
    assert_eq!(m0, 10_200.0);
    assert_eq!(rb.payload(), 0.0);
    rb.set_payload(3_000.0);
    assert_eq!(rb.payload(), 3_000.0);
    assert_eq!(rb.mass(), 13_200.0);
    let f = f0 * (m0 / 13_200.0f32).sqrt();
    assert!((rb.body_freq() - f).abs() < 1e-4, "{} {f}", rb.body_freq());
    // the people get off again: the payload is set, not added
    rb.set_payload(750.0);
    assert_eq!(rb.mass(), 10_950.0);
    for nothing in [0.0, -80.0, f32::NAN, f32::INFINITY] {
        rb.set_payload(nothing);
        assert_eq!((rb.mass(), rb.payload()), (m0, 0.0), "{nothing}");
        assert_eq!(rb.body_freq(), f0);
    }
}

/// A full bus sits lower on its springs: they carry the people's weight besides the bus's,
/// the more the more people.
#[test]
fn a_loaded_bus_sits_lower_on_its_springs() {
    let sag = |kg: f32| {
        let rb = loaded(kg);
        rb.wheels().iter().map(|w| w.compression()).sum::<f32>() / rb.wheels().len() as f32
    };
    let (empty, half, full) = (sag(0.0), sag(3_000.0), sag(6_000.0));
    assert!(half > empty && full > half, "{empty} {half} {full}");
    // the springs are linear: each 3 t press them about as much further
    let (a, b) = (half - empty, full - half);
    assert!(a > 0.0 && (a - b).abs() < 0.2 * a, "{a} {b}");
    // and as much as the 10.2 t of the bus would, in proportion
    let per_t = empty / 10.2;
    assert!((a / 3.0 - per_t).abs() < 0.2 * per_t, "{a} {per_t}");
}

/// A full bus gathers speed more slowly under the same torque, and the same brakes take
/// longer to stop it: it has more mass to move.
#[test]
fn a_loaded_bus_is_slower_to_speed_up_and_to_stop() {
    let speed_after = |kg: f32| {
        let mut rb = loaded(kg);
        run(&mut rb, 5.0, 4_000.0, 0.0, &flat);
        rb.forward_speed()
    };
    let (empty, full) = (speed_after(0.0), speed_after(5_000.0));
    assert!(full < 0.8 * empty, "empty {empty} m/s, full {full} m/s");
    let stopping = |kg: f32| {
        let mut rb = loaded(kg);
        rb.set_velocity(glam::Vec3::Y * 10.0);
        for w in rb.wheels_mut() {
            w.set_spin(10.0 / w.radius());
        }
        let y0 = rb.position().y;
        run(&mut rb, 20.0, 0.0, 3_000.0, &flat);
        assert!(
            rb.forward_speed().abs() < 0.05,
            "{kg} kg still at {}",
            rb.forward_speed()
        );
        rb.position().y - y0
    };
    let (empty, full) = (stopping(0.0), stopping(5_000.0));
    assert!(full > 1.2 * empty, "empty {empty} m, full {full} m");
}

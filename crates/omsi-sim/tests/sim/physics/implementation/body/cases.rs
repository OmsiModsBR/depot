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
use glam::{DVec3, Vec3};
use omsi_sim::testing::physics::{Engine, Payload, RigidBody};

/// `kg` carried at the model origin.
fn kg(kg: f32) -> Payload {
    Payload {
        kg,
        centre: Vec3::ZERO,
        spread: Vec3::ZERO,
    }
}

/// A bus carrying `kg`, standing on flat ground for a second (its springs settled).
fn loaded(mass: f32) -> RigidBody {
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.set_payload(kg(mass));
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
    rb.set_payload(kg(3_000.0));
    assert_eq!(rb.payload(), 3_000.0);
    assert_eq!(rb.mass(), 13_200.0);
    let f = f0 * (m0 / 13_200.0f32).sqrt();
    assert!((rb.body_freq() - f).abs() < 1e-4, "{} {f}", rb.body_freq());
    // the people get off again: the payload is set, not added
    rb.set_payload(kg(750.0));
    assert_eq!(rb.mass(), 10_950.0);
    for nothing in [0.0, -80.0, f32::NAN, f32::INFINITY] {
        rb.set_payload(kg(nothing));
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

/// `D-2`: a crowd of `kg` centred at `centre`, carried by a bus under `engine` standing on
/// flat ground for three seconds (its springs settled).
fn crowded(engine: Engine, kg: f32, centre: Vec3) -> RigidBody {
    let mut rb = RigidBody::for_engine(&bus(), &[], engine);
    rb.place(DVec3::ZERO, 0.0);
    rb.set_payload(Payload {
        kg,
        centre,
        spread: Vec3::ZERO,
    });
    run(&mut rb, 3.0, 0.0, 0.0, &flat);
    rb
}

/// The front and the rear axle's mean compression (m).
fn axle_compressions(rb: &RigidBody) -> (f32, f32) {
    let w = rb.wheels();
    (
        (w[0].compression() + w[1].compression()) / 2.0,
        (w[2].compression() + w[3].compression()) / 2.0,
    )
}

/// `D-2`: under Depot the people move the centre of gravity to the mass-weighted mean of the
/// empty body's (`[cog]`, `[schwerpunkt]`) and theirs, the model origin stays where it stood,
/// and nobody aboard gives the file's centre back exactly.
#[test]
fn depot_moves_the_centre_of_gravity_towards_the_people() {
    let mut rb = RigidBody::for_engine(&bus(), &[], Engine::Depot);
    rb.place(DVec3::new(10.0, 20.0, 0.0), 30.0);
    let (empty, origin) = (rb.cog(), rb.origin());
    rb.set_payload(Payload {
        kg: 3_000.0,
        centre: Vec3::new(-0.6, 4.0, 1.8),
        spread: Vec3::ZERO,
    });
    let want = Vec3::new(
        -0.6 * 3_000.0 / 13_200.0,
        4.0 * 3_000.0 / 13_200.0,
        (empty.z * 10_200.0 + 1.8 * 3_000.0) / 13_200.0,
    );
    assert!((rb.cog() - want).length() < 1e-4, "{} {want}", rb.cog());
    assert_eq!(rb.empty_cog(), empty);
    assert!(
        (rb.origin() - origin).length() < 1e-4,
        "{} {origin}",
        rb.origin()
    );
    for nothing in [0.0, -80.0, f32::NAN] {
        rb.set_payload(Payload {
            kg: nothing,
            centre: Vec3::new(0.0, 4.0, 1.0),
            spread: Vec3::ZERO,
        });
        assert_eq!(rb.cog(), empty, "{nothing}");
    }
    // a place that is no place moves nothing: the mass is carried at `[cog]`
    rb.set_payload(Payload {
        kg: 3_000.0,
        centre: Vec3::new(0.0, f32::NAN, 1.0),
        spread: Vec3::ZERO,
    });
    assert_eq!((rb.cog(), rb.mass()), (empty, 13_200.0));
}

/// `D-2`: moving the centre of gravity leaves the motion of the body's points as it was: a
/// body turning and rolling forward keeps its origin's velocity.
#[test]
fn moving_the_centre_of_gravity_keeps_the_body_moving_as_before() {
    let mut rb = RigidBody::for_engine(&bus(), &[], Engine::Depot);
    rb.place(DVec3::ZERO, 0.0);
    rb.set_velocity(Vec3::new(0.0, 8.0, 0.0));
    rb.set_omega(Vec3::new(0.0, 0.0, 0.3));
    let at_origin = |rb: &RigidBody| {
        rb.velocity()
            + rb.orientation()
                .mul_vec3(rb.omega())
                .cross(-rb.orientation().mul_vec3(rb.cog()))
    };
    let before = at_origin(&rb);
    rb.set_payload(Payload {
        kg: 4_000.0,
        centre: Vec3::new(0.5, -4.0, 1.0),
        spread: Vec3::ZERO,
    });
    assert!(
        (at_origin(&rb) - before).length() < 1e-4,
        "{} {before}",
        at_origin(&rb)
    );
}

/// `D-2`: under Depot a crowd at the front presses the front springs more than the same
/// crowd at the back, and the back springs less; under Classic where the people stand
/// changes nothing - they weigh as if at `[cog]`.
#[test]
fn where_the_people_stand_moves_the_load_between_the_axles_under_depot_only() {
    let front = Vec3::new(0.0, 4.0, 1.0);
    let back = Vec3::new(0.0, -4.5, 1.0);
    let (f_front, r_front) = axle_compressions(&crowded(Engine::Depot, 4_000.0, front));
    let (f_back, r_back) = axle_compressions(&crowded(Engine::Depot, 4_000.0, back));
    assert!(f_front > f_back + 0.02, "{f_front} {f_back}");
    assert!(r_front < r_back - 0.02, "{r_front} {r_back}");
    let classic_front = axle_compressions(&crowded(Engine::Classic, 4_000.0, front));
    let classic_back = axle_compressions(&crowded(Engine::Classic, 4_000.0, back));
    assert_eq!(classic_front, classic_back);
}

/// `D-2`: under Depot people on one side lean the bus towards them; under Classic it stands
/// level.
#[test]
fn people_on_one_side_lean_the_bus_under_depot_only() {
    let left = Vec3::new(-1.0, 0.0, 1.0);
    let lean = |engine| {
        let rb = crowded(engine, 3_000.0, left);
        let w = rb.wheels();
        // left wheels minus right wheels, both axles
        (w[0].compression() + w[2].compression()) - (w[1].compression() + w[3].compression())
    };
    assert!(lean(Engine::Depot) > 0.01, "{}", lean(Engine::Depot));
    assert!(
        lean(Engine::Classic).abs() < 1e-4,
        "{}",
        lean(Engine::Classic)
    );
}

/// `D-2`: under Depot the people add to the inertia about the new centre of gravity: the
/// empty body's and the people's own spread, each moved there (the parallel-axis rule); under
/// Classic the inertia stays the file's. Nobody aboard gives the file's inertia back exactly.
#[test]
fn depot_adds_the_people_to_the_inertia_by_the_parallel_axis_rule() {
    let (m0, kg) = (10_200.0f32, 3_000.0f32);
    let (centre, spread) = (Vec3::new(0.4, -3.0, 1.5), Vec3::new(500.0, 9_000.0, 200.0));
    let load = Payload { kg, centre, spread };
    let mut rb = RigidBody::for_engine(&bus(), &[], Engine::Depot);
    rb.place(DVec3::ZERO, 0.0);
    let (i0, c0) = (rb.inertia(), rb.cog());
    rb.set_payload(load);
    let c = rb.cog();
    let (d, e) = (c0 - c, centre - c);
    let sq = |v: Vec3, k: f32| v * v * k;
    let (body, people) = (sq(d, m0), sq(e, kg) + spread);
    let want = i0
        + Vec3::new(body.y + body.z, body.x + body.z, body.x + body.y)
        + Vec3::new(
            people.y + people.z,
            people.x + people.z,
            people.x + people.y,
        );
    assert!(
        (rb.inertia() - want).abs().max_element() < 1.0,
        "{} {want}",
        rb.inertia()
    );
    // a long crowd turns the bus harder about its yaw and pitch than about its roll
    let added = rb.inertia() - i0;
    assert!(added.z > added.y && added.x > added.y, "{added}");
    rb.set_payload(Payload::default());
    assert_eq!((rb.inertia(), rb.cog()), (i0, c0));
    assert_eq!(rb.empty_inertia(), i0);
    let mut classic = RigidBody::for_engine(&bus(), &[], Engine::Classic);
    classic.set_payload(load);
    assert_eq!(classic.inertia(), i0);
}

/// `D-2`: under Depot the same crowd standing higher in the bus leans it further out of a
/// bend: the centre of gravity is higher, so the bend's pull rolls the body harder.
#[test]
fn a_crowd_higher_up_leans_the_bus_further_in_a_bend_under_depot() {
    let roll = |z: f32| {
        let mut rb = RigidBody::for_engine(&bus(), &[], Engine::Depot);
        rb.place(DVec3::ZERO, 0.0);
        rb.set_payload(Payload {
            kg: 5_000.0,
            centre: Vec3::new(0.0, 0.0, z),
            spread: Vec3::ZERO,
        });
        rb.set_velocity(Vec3::new(0.0, 8.0, 0.0));
        for w in rb.wheels_mut() {
            let spin = 8.0 / w.radius();
            w.set_spin(spin);
        }
        let brakes = vec![0.0; rb.wheels().len()];
        for _ in 0..240 {
            let torque = (8.0 - rb.forward_speed()) * rb.mass() + rb.rolling_resistance();
            rb.step(1.0 / 60.0, torque * 0.47, &brakes, 0.3, &flat);
        }
        rb.heading_pitch_bank().2.abs()
    };
    let (low, high) = (roll(0.6), roll(2.0));
    assert!(high > low * 1.1, "low {low} deg, high {high} deg");
}

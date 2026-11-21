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
/// payload's. Nothing aboard, or a payload that is no mass, leaves the empty body.
#[test]
fn a_payload_adds_to_the_body_mass() {
    for engine in [Engine::Classic, Engine::Depot] {
        let mut rb = RigidBody::for_engine(&bus(), &[], engine);
        let (m0, f0) = (rb.mass(), rb.body_freq());
        assert_eq!(m0, 10_200.0);
        assert_eq!(rb.payload(), 0.0);
        rb.set_payload(kg(3_000.0));
        assert_eq!(rb.payload(), 3_000.0);
        assert_eq!(rb.mass(), 13_200.0);
        // the people get off again: the payload is set, not added
        rb.set_payload(kg(750.0));
        assert_eq!(rb.mass(), 10_950.0);
        for nothing in [0.0, -80.0, f32::NAN, f32::INFINITY] {
            rb.set_payload(kg(nothing));
            assert_eq!((rb.mass(), rb.payload()), (m0, 0.0), "{engine:?} {nothing}");
            assert_eq!(rb.body_freq(), f0);
        }
    }
}

/// `K-5`: under OMSI Classic the pitch and roll damping is the empty body's however many people
/// are aboard; under Depot it is that of the heavier body on the same springs.
#[test]
fn the_payload_damps_the_body_only_under_depot() {
    let loaded = |engine| {
        let mut rb = RigidBody::for_engine(&bus(), &[], engine);
        let f0 = rb.body_freq();
        rb.set_payload(kg(3_000.0));
        (f0, rb.body_freq())
    };
    let (f0, f) = loaded(Engine::Classic);
    assert_eq!(f, f0);
    let (f0, f) = loaded(Engine::Depot);
    let heavier = f0 * (10_200.0 / 13_200.0f32).sqrt();
    assert!((f - heavier).abs() < 1e-4, "{f} {heavier}");
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

/// A bus under `engine` rolling at `v` m/s, its yaw inertia raised by `yaw_spread` (kg m²) of
/// payload centred at its centre of gravity.
fn rolling(engine: Engine, v: f32, yaw_spread: f32) -> RigidBody {
    let mut rb = RigidBody::for_engine(&bus(), &[], engine);
    rb.place(DVec3::ZERO, 0.0);
    let centre = rb.cog();
    rb.set_payload(Payload {
        kg: 1.0,
        centre,
        spread: Vec3::new(yaw_spread / 2.0, yaw_spread / 2.0, 0.0),
    });
    rb.set_velocity(Vec3::new(0.0, v, 0.0));
    for w in rb.wheels_mut() {
        let spin = v / w.radius();
        w.set_spin(spin);
    }
    let brakes = vec![0.0; rb.wheels().len()];
    for _ in 0..60 {
        let torque = 0.47 * ((v - rb.forward_speed()) * rb.mass() + rb.rolling_resistance());
        rb.step(1.0 / 60.0, torque, &brakes, 0.0, &flat);
    }
    rb
}

/// Whether the tyres let go within two seconds of the steering swung to `steer` and back
/// (half a second each way) at `v` m/s.
fn lets_go(engine: Engine, v: f32, yaw_spread: f32, steer: f32) -> bool {
    let mut rb = rolling(engine, v, yaw_spread);
    let brakes = vec![0.0; rb.wheels().len()];
    for i in 0..120 {
        let t = i as f32 / 60.0;
        let s = if t < 0.5 {
            steer * t / 0.5
        } else if t < 1.0 {
            steer * (1.0 - t) / 0.5
        } else {
            0.0
        };
        let torque = 0.47 * ((v - rb.forward_speed()) * rb.mass() + rb.rolling_resistance());
        rb.step(1.0 / 60.0, torque, &brakes, s, &flat);
        if !rb.holding() {
            return true;
        }
    }
    false
}

/// The smallest steering swing (of 0.02 steps) that makes the tyres let go.
fn least_swing_that_lets_go(engine: Engine, v: f32, yaw_spread: f32) -> f32 {
    (1..=50)
        .map(|i| i as f32 * 0.02)
        .find(|&s| lets_go(engine, v, yaw_spread, s))
        .unwrap_or(f32::INFINITY)
}

/// `D-3`: under Depot, a bus that turns harder (a larger yaw inertia) lets go at a smaller
/// swing of the steering: its tyres must also start and stop it turning. Under Classic the
/// inertia changes nothing while the tyres hold.
#[test]
fn a_heavier_turning_bus_lets_go_sooner_in_a_quick_swing_under_depot_only() {
    let v = 14.0;
    let (light, heavy) = (0.0, 600_000.0);
    let depot = (
        least_swing_that_lets_go(Engine::Depot, v, light),
        least_swing_that_lets_go(Engine::Depot, v, heavy),
    );
    assert!(
        depot.1 < depot.0,
        "depot: light {} heavy {}",
        depot.0,
        depot.1
    );
    let classic = (
        least_swing_that_lets_go(Engine::Classic, v, light),
        least_swing_that_lets_go(Engine::Classic, v, heavy),
    );
    assert_eq!(classic.0, classic.1, "classic");
}

/// `D-3`: a steady bend lets go at the same speed under both engines: with the weight shared
/// about `[cog]`, each axle is asked the same share of its grip as all tyres together.
#[test]
fn a_steady_bend_lets_go_at_the_same_steering_under_both_engines() {
    let steady = |engine: Engine| {
        (1..=50)
            .map(|i| i as f32 * 0.02)
            .find(|&s| {
                let mut rb = rolling(engine, 12.0, 0.0);
                let brakes = vec![0.0; rb.wheels().len()];
                // eased in over three seconds, then held
                (0..360).any(|i| {
                    let k = (i as f32 / 180.0).min(1.0);
                    let torque =
                        0.47 * ((12.0 - rb.forward_speed()) * rb.mass() + rb.rolling_resistance());
                    rb.step(1.0 / 60.0, torque, &brakes, s * k, &flat);
                    !rb.holding()
                })
            })
            .unwrap_or(f32::INFINITY)
    };
    let (classic, depot) = (steady(Engine::Classic), steady(Engine::Depot));
    assert!(
        (classic - depot).abs() <= 0.02 + 1e-6,
        "classic {classic} depot {depot}"
    );
}

/// `D-3`: a steering key that sets a full lock in one frame at walking pace does not make the
/// tyres let go: the turning they must start follows their relaxation.
#[test]
fn a_steering_key_at_walking_pace_does_not_let_the_tyres_go() {
    let mut rb = rolling(Engine::Depot, 2.0, 0.0);
    let brakes = vec![0.0; rb.wheels().len()];
    for _ in 0..120 {
        let torque = 0.47 * ((2.0 - rb.forward_speed()) * rb.mass() + rb.rolling_resistance());
        rb.step(1.0 / 60.0, torque, &brakes, 1.0, &flat);
        assert!(rb.holding());
    }
}

/// A road at 0 whose faces carry `road` (their texture's ground byte) right of x = 0 and
/// the terrain (no byte) left of it.
struct Textured {
    road: u8,
}

impl omsi_sim::physics::Ground for Textured {
    fn probe(&self, x: f64, y: f64, top: f64) -> omsi_sim::physics::GroundProbe {
        flat(x, y, top)
    }

    fn surface(&self, x: f64, _y: f64, z: f64) -> Option<u8> {
        (x > 0.0 && z.abs() < 0.01).then_some(self.road)
    }
}

/// Each wheel standing on the ground takes the ground byte of the road face under it, and
/// `Axle_SurfaceID` its `[surface]` kind; a wheel on the terrain keeps the byte it had, as
/// OMSI's wheels do, and a body that has not stood on a road yet has 0.
#[test]
fn the_wheels_take_the_ground_of_the_road_under_them() {
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    run(&mut rb, 1.0, 0.0, 0.0, &flat);
    assert!(rb.wheels().iter().all(|w| w.surface() == 0));
    rb.read_surfaces(&Textured { road: 0x62 });
    for w in rb.wheels() {
        let right = w.attach().x > 0.0;
        assert_eq!(
            w.surface(),
            if right { 0x62 } else { 0 },
            "{:?}",
            w.attach()
        );
        assert_eq!(w.surface_id(), if right { 2.0 } else { 0.0 });
    }
    // the right wheels roll onto the terrain: they keep their road's byte
    rb.read_surfaces(&Textured { road: 0x21 });
    for w in rb.wheels().iter().filter(|w| w.attach().x > 0.0) {
        assert_eq!(w.surface(), 0x21);
    }
    rb.place(DVec3::new(-50.0, 0.0, 0.0), 0.0);
    run(&mut rb, 1.0, 0.0, 0.0, &flat);
    rb.read_surfaces(&Textured { road: 0x05 });
    for w in rb.wheels().iter().filter(|w| w.attach().x > 0.0) {
        assert_eq!(w.surface(), 0x21);
    }
}

/// A road rising `grade` (a fraction) towards +y, as a probe reads it.
fn ramp(grade: f64) -> impl Fn(f64, f64, f64) -> omsi_sim::physics::GroundProbe {
    move |_x, y, _top| omsi_sim::physics::GroundProbe {
        below: Some(y * grade),
        above: None,
    }
}

/// The road's grade is read from the ground under the axles, in percent, positive with the
/// front higher: 8 % facing up the ramp, -8 % facing down it, 0 on the flat. It is the
/// road's and not the body's pitch: braking hard, which tips the body forward, leaves it.
#[test]
fn the_road_grade_is_the_slope_under_the_axles() {
    for engine in [Engine::Classic, Engine::Depot] {
        for (heading, want) in [(0.0, 8.0), (180.0, -8.0)] {
            let mut rb = RigidBody::for_engine(&bus(), &[], engine);
            rb.place(DVec3::ZERO, heading);
            run(&mut rb, 3.0, 0.0, 50_000.0, &ramp(0.08));
            let grade = rb.road_grade().expect("the bus stands on the ramp");
            assert!((grade - want).abs() < 0.2, "{engine:?} {heading}: {grade}");
        }
        let mut rb = RigidBody::for_engine(&bus(), &[], engine);
        rb.place(DVec3::ZERO, 0.0);
        run(&mut rb, 3.0, 0.0, 0.0, &flat);
        assert!(rb.road_grade().unwrap().abs() < 0.05, "{engine:?} flat");
        // braking from 10 m/s pitches the body, not the road
        rb.set_velocity(Vec3::Y * 10.0);
        for w in rb.wheels_mut() {
            w.set_spin(10.0 / w.radius());
        }
        run(&mut rb, 0.5, 0.0, 3_000.0, &flat);
        assert!(rb.road_grade().unwrap().abs() < 0.05, "{engine:?} braking");
    }
}

/// Just placed, before a tyre has found the ground, the body has no grade to give.
#[test]
fn a_body_not_yet_on_the_ground_has_no_road_grade() {
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    assert_eq!(rb.road_grade(), None);
}

/// The body skids while its tyres slide across the road (they let go of it) or a tyre spins
/// or is locked by its brake; standing, cruising and braking gently it does not.
#[test]
fn the_body_skids_when_a_tyre_slides_spins_or_locks() {
    for engine in [Engine::Classic, Engine::Depot] {
        // standing still on its springs
        let mut rb = RigidBody::for_engine(&bus(), &[], engine);
        rb.place(DVec3::ZERO, 0.0);
        run(&mut rb, 3.0, 0.0, 0.0, &flat);
        assert!(!rb.skidding(), "{engine:?} standing");
        // cruising, then braking gently
        let mut rb = rolling(engine, 12.0, 0.0);
        assert!(!rb.skidding(), "{engine:?} cruising");
        let gentle = vec![500.0; rb.wheels().len()];
        rb.step(1.0 / 60.0, 0.0, &gentle, 0.0, &flat);
        assert!(!rb.skidding(), "{engine:?} braking gently");
        // a brake far past the grip locks the wheels
        let hard = vec![200_000.0; rb.wheels().len()];
        let locked = (0..30).any(|_| {
            rb.step(1.0 / 60.0, 0.0, &hard, 0.0, &flat);
            rb.skidding()
        });
        assert!(locked, "{engine:?} locked");
        // full lock at speed lets the tyres go across
        let mut rb = rolling(engine, 14.0, 0.0);
        let none = vec![0.0; rb.wheels().len()];
        let slid = (0..120).any(|i| {
            let s = if i < 30 { i as f32 / 30.0 } else { 1.0 };
            rb.step(1.0 / 60.0, 0.0, &none, s, &flat);
            !rb.holding()
        });
        assert!(slid && rb.skidding(), "{engine:?} sliding across");
    }
}

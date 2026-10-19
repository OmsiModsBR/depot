use crate::physics::cases::run;
use crate::support::ground::road;
use crate::support::vehicles::bus;
use glam::{DVec3, Vec3};
use omsi_sim::testing::collision::Obb;
use omsi_sim::testing::physics::domain::params::{CRASH_SPEED, RESTITUTION};
use omsi_sim::testing::physics::domain::RigidBody;
use omsi_sim::testing::physics::RigidBodyInternals;

/// Straight into a wall at 40 km/h: the bus stops at the wall, bounces off a little, and
/// drives away backwards straight after.
#[test]
fn crash_bounces_and_releases() {
    let def = bus();
    let bb = def.bounding_box.unwrap();
    let mut rb = RigidBody::from_definition(&def, &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    run(&mut rb, 1.0, 0.0, 5000.0, &g);
    rb.set_velocity(Vec3::new(0.0, 40.0 / 3.6, 0.0));
    let wall = [Obb::from_box(
        [20.0, 1.0, 3.0, 0.0, 0.0, 1.5],
        DVec3::new(0.0, 12.0, 0.0),
        0.0,
    )];
    let mut impacts = Vec::new();
    for _ in 0..90 {
        rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &g);
        impacts.extend(rb.collide(bb, &wall, &|_| false, 1.0 / 30.0));
    }
    assert_eq!(
        impacts.iter().filter(|i| i.speed > CRASH_SPEED).count(),
        1,
        "{impacts:?}"
    );
    let hit = impacts[0];
    assert!(hit.speed > 10.0 && hit.energy > 400_000.0, "{hit:?}");
    assert!(hit.point.y > 5.5, "{hit:?}");
    assert!(rb.forward_speed() < 0.0, "bounced: {}", rb.forward_speed());
    let front = rb.origin().y + 5.83;
    assert!(front < 11.5 + 0.01, "{front}");
    // reverse away at once
    let y0 = rb.origin().y;
    for _ in 0..60 {
        rb.step(1.0 / 30.0, -8000.0, &[0.0; 4], 0.0, &g);
        rb.collide(bb, &wall, &|_| false, 1.0 / 30.0);
    }
    assert!(rb.origin().y < y0 - 1.0, "{} -> {}", y0, rb.origin().y);
}

/// At 4 frames a second the bus answers the wall after each slice of the frame, with the
/// time the slice covered: it stops at the wall, as it does at 30, and the crash counts
/// once. (Answered once a frame, it ran 2.6 m into the wall before the wall pushed it out.)
#[test]
fn a_long_frame_meets_the_wall_slice_by_slice() {
    let def = bus();
    let bb = def.bounding_box.unwrap();
    let mut rb = RigidBody::from_definition(&def, &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    run(&mut rb, 1.0, 0.0, 5000.0, &g);
    rb.set_velocity(Vec3::new(0.0, 40.0 / 3.6, 0.0));
    let wall = [Obb::from_box(
        [20.0, 1.0, 3.0, 0.0, 0.0, 1.5],
        DVec3::new(0.0, 12.0, 0.0),
        0.0,
    )];
    let mut impacts = Vec::new();
    let mut deepest = f64::MIN;
    for _ in 0..12 {
        let inputs = [omsi_sim::physics::BodyInputs {
            brake: &[0.0; 4],
            steer: Some(0.0),
            air: true,
        }];
        omsi_sim::physics::step_train_with(
            std::slice::from_mut(&mut rb),
            &[],
            0.25,
            0.0,
            &inputs,
            &g,
            &mut |bodies, slice| {
                deepest = deepest.max(bodies[0].origin().y + 5.83 - 11.5);
                impacts.extend(bodies[0].collide(bb, &wall, &|_| false, slice));
            },
        );
    }
    assert_eq!(
        impacts.iter().filter(|i| i.speed > CRASH_SPEED).count(),
        1,
        "{impacts:?}"
    );
    assert!(deepest < 0.6, "the front went {deepest} m into the wall");
    assert!(rb.origin().y + 5.83 < 11.5 + 0.01);
}

/// A glancing blow on the front corner turns the bus away from the wall and lets it slide.
#[test]
fn glancing_blow_turns_the_bus() {
    let def = bus();
    let bb = def.bounding_box.unwrap();
    let mut rb = RigidBody::from_definition(&def, &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    run(&mut rb, 1.0, 0.0, 5000.0, &g);
    // heading 20° to the right towards a wall along x = 4
    rb.place(DVec3::ZERO, 20.0);
    run(&mut rb, 0.5, 0.0, 0.0, &g);
    rb.set_velocity(rb.orientation().mul_vec3(Vec3::Y) * 30.0 / 3.6);
    let wall = [Obb::from_box(
        [1.0, 60.0, 3.0, 0.0, 0.0, 1.5],
        DVec3::new(4.5, 10.0, 0.0),
        0.0,
    )];
    let mut hits = 0;
    for _ in 0..60 {
        rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &g);
        hits += rb.collide(bb, &wall, &|_| false, 1.0 / 30.0).len();
    }
    let (heading, _, _) = rb.heading_pitch_bank();
    assert!(hits > 0);
    assert!(
        heading < 15.0 || heading > 300.0,
        "turned away from the wall: {heading}"
    );
    assert!(
        rb.velocity().y > 1.0,
        "still sliding along: {:?}",
        rb.velocity()
    );
}

/// A car (1 t, kinematic: it keeps its speed) drives into the side of the standing bus
/// at 50 km/h: the bus gets a nudge and a crash, not a shove across the road.
#[test]
fn a_car_does_not_bulldoze_the_bus() {
    let def = bus();
    let bb = def.bounding_box.unwrap();
    let mut rb = RigidBody::from_definition(&def, &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    run(&mut rb, 1.0, 0.0, 5000.0, &g);
    let (x0, mut hits, mut energy) = (rb.origin().x, 0, 0.0f32);
    for k in 0..30 {
        // from the left, heading east
        let car = Obb::from_box(
            [1.7, 4.2, 1.4, 0.0, 0.0, 0.7],
            DVec3::new(-4.0 + 50.0 / 3.6 * k as f64 / 30.0, 0.0, 0.0),
            90.0,
        )
        .moving(glam::DVec2::new(50.0 / 3.6, 0.0), 1000.0, 7);
        rb.step(1.0 / 30.0, 0.0, &[5000.0; 4], 0.0, &g);
        for hit in rb.collide(bb, &[car], &|_| false, 1.0 / 30.0) {
            hits += 1;
            energy += hit.energy;
        }
    }
    run(&mut rb, 2.0, 0.0, 5000.0, &g);
    let (_, _, bank) = rb.heading_pitch_bank();
    assert!(hits > 0 && energy > 20_000.0, "{hits} hits, {energy} J");
    assert!(
        (rb.origin().x - x0).abs() < 1.0,
        "shoved {} m",
        rb.origin().x - x0
    );
    assert!(bank.abs() < 3.0, "banked {bank}");
}

/// A car driving into the standing bus is one crash per call, however long it stays
/// inside the bus: 0.5 v² (1 - e²) over the pair's inverse mass, not four times that.
#[test]
fn a_car_strike_counts_once() {
    let def = bus();
    let bb = def.bounding_box.unwrap();
    let mut rb = RigidBody::from_definition(&def, &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    run(&mut rb, 1.0, 0.0, 5000.0, &g);
    let v = 50.0f32 / 3.6;
    let car = Obb::from_box(
        [1.7, 4.2, 1.4, 0.0, 0.0, 0.7],
        DVec3::new(-2.0, 0.0, 0.0),
        90.0,
    )
    .moving(glam::DVec2::new(v as f64, 0.0), 1000.0, 7);
    let hits = rb.collide(bb, &[car], &|_| false, 1.0 / 30.0);
    assert_eq!(hits.len(), 1, "{hits:?}");
    let n = Vec3::X;
    let r = Vec3::new(-1.236, 0.0, 0.0);
    let k = rb.inv_mass_at(r, n) + 1.0 / 1000.0;
    let want = 0.5 * v * v * (1.0 - RESTITUTION * RESTITUTION) / k;
    assert!(
        (hits[0].energy - want).abs() < 0.15 * want,
        "{} J, want about {want}",
        hits[0].energy
    );
    assert!(hits[0].energy < 100_000.0, "{hits:?}");
}

/// The scripts hear where the bodies meet - low down at the bumpers - not the height of
/// the centre of gravity: reversing into a wall or a car is below 1.10 m and behind
/// -4.70 m, where the stock buses damage their engine; a high beam is hit high.
#[test]
fn impacts_are_reported_at_bumper_height() {
    let def = bus();
    let bb = def.bounding_box.unwrap();
    let hit = |obstacle: Obb, v: Vec3| {
        let mut rb = RigidBody::from_definition(&def, &[]);
        rb.place(DVec3::ZERO, 0.0);
        let g = road(1e9, 0.0);
        run(&mut rb, 1.0, 0.0, 5000.0, &g);
        rb.set_velocity(v);
        for _ in 0..60 {
            rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &g);
            if let Some(h) = rb
                .collide(bb, &[obstacle], &|_| false, 1.0 / 30.0)
                .into_iter()
                .find(|h| h.speed > CRASH_SPEED)
            {
                return h;
            }
        }
        panic!("no hit on {obstacle:?}");
    };
    let back = Vec3::new(0.0, -15.0 / 3.6, 0.0);
    let wall = hit(
        Obb::from_box(
            [20.0, 1.0, 5.0, 0.0, 0.0, 2.5],
            DVec3::new(0.0, -8.0, 0.0),
            0.0,
        ),
        back,
    );
    assert!(
        wall.point.z < 1.1 && wall.point.z > 0.5 && wall.point.y < -4.7,
        "{wall:?}"
    );
    let car = hit(
        Obb::from_box(
            [1.7, 4.2, 1.4, 0.0, 0.0, 0.7],
            DVec3::new(0.0, -9.0, 0.0),
            0.0,
        )
        .moving(glam::DVec2::ZERO, 1000.0, 3),
        back,
    );
    assert!(car.point.z < 1.1 && car.point.y < -4.7, "{car:?}");
    let front = hit(
        Obb::from_box(
            [20.0, 1.0, 5.0, 0.0, 0.0, 2.5],
            DVec3::new(0.0, 8.0, 0.0),
            0.0,
        ),
        -back,
    );
    assert!(front.point.z < 1.1 && front.point.y > 4.7, "{front:?}");
    let beam = hit(
        Obb::from_box(
            [20.0, 1.0, 1.0, 0.0, 0.0, 2.5],
            DVec3::new(0.0, 8.0, 0.0),
            0.0,
        ),
        -back,
    );
    assert!(beam.point.z > 2.0, "{beam:?}");
}

/// Into the back of a standing car at 30 km/h: the car (1 t, which the AI holds where it
/// is) stops the bus within a few metres instead of being driven through.
#[test]
fn rear_ends_a_standing_car() {
    let def = bus();
    let bb = def.bounding_box.unwrap();
    let mut rb = RigidBody::from_definition(&def, &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    run(&mut rb, 1.0, 0.0, 5000.0, &g);
    rb.set_velocity(Vec3::new(0.0, 30.0 / 3.6, 0.0));
    let car = [Obb::from_box(
        [1.7, 4.2, 1.4, 0.0, 0.0, 0.7],
        DVec3::new(0.0, 12.0, 0.0),
        0.0,
    )
    .moving(glam::DVec2::ZERO, 1000.0, 3)];
    let mut first = None;
    for k in 0..90 {
        rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &g);
        if !rb.collide(bb, &car, &|_| false, 1.0 / 30.0).is_empty() && first.is_none() {
            first = Some((k, rb.origin().y));
        }
    }
    let (_, y_hit) = first.expect("hit the car");
    let front = rb.origin().y + 5.83;
    assert!(
        rb.forward_speed().abs() < 0.3,
        "still at {} km/h",
        rb.forward_speed() * 3.6
    );
    assert!(
        front < 12.0 - 2.1 + 0.3,
        "drove into the car: front at {front}"
    );
    assert!(
        rb.origin().y - y_hit < 3.0,
        "pushed on {} m",
        rb.origin().y - y_hit
    );
}

/// A lamp post breaks off: the bus hardly slows down.
#[test]
fn a_post_breaks_off() {
    let def = bus();
    let bb = def.bounding_box.unwrap();
    let mut rb = RigidBody::from_definition(&def, &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    run(&mut rb, 1.0, 0.0, 5000.0, &g);
    rb.set_velocity(Vec3::new(0.0, 30.0 / 3.6, 0.0));
    let mut post = Obb::from_box(
        [0.2, 0.2, 6.0, 0.0, 0.0, 3.0],
        DVec3::new(0.5, 10.0, 0.0),
        0.0,
    );
    post.pole = Some((0.05, 0.5));
    let mut broke = false;
    for _ in 0..30 {
        rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &g);
        let hits = rb.collide(bb, &[post], &|_| broke, 1.0 / 30.0);
        broke |= hits.iter().any(|h| h.broke);
    }
    assert!(broke);
    assert!(
        rb.forward_speed() > 25.0 / 3.6,
        "{}",
        rb.forward_speed() * 3.6
    );
}

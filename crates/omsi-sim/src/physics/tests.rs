use super::domain::params::{HALF_WIDTH, LATTICE};
use super::*;
use glam::{DVec3, Vec3};
use omsi_vehicle::Vehicle;

use omsi_vehicle::vehicle::Axle;

pub(super) fn bus() -> Vehicle {
    let axle = |long: f32, spring: f32, max_force: f32, driven: bool| Axle { long, max_width: 2.4, min_width: 1.76, wheel_diameter: 0.94, spring, max_force, damper: 20.0, driven, inertia_inv: 0.0 };
    Vehicle { mass: 10.2, moment_of_inertia: [600.0, 90.0, 400.0], cog_height: 1.2, rolling_resistance: 1000.0, rot_pnt_long: -2.7, inv_min_turn_radius: 0.13, axles: vec![axle(3.238, 240.0, 90.0, false), axle(-2.637, 280.0, 116.0, true)], bounding_box: Some([2.472, 11.663, 2.491, 0.0, 0.001, 1.622]), ..Default::default() }
}

/// Flat road at height 0 with an optional step of `step` metres from y = `at` on.
pub(super) fn road(at: f64, step: f64) -> impl Fn(f64, f64, f64) -> GroundProbe {
    move |_x, y, z_top| {
        let z = if y >= at { step } else { 0.0 };
        if z <= z_top {
            GroundProbe { below: Some(z), above: None }
        } else {
            GroundProbe { below: Some(0.0), above: Some(z) }
        }
    }
}

pub(super) fn run(rb: &mut RigidBody, secs: f32, torque: f32, brake: f32, g: &dyn Fn(f64, f64, f64) -> GroundProbe) {
    let brakes = vec![brake; rb.wheels.len()];
    for _ in 0..(secs * 60.0) as usize {
        rb.step(1.0 / 60.0, torque, &brakes, 0.0, g);
    }
}

#[test]
fn driving_past_a_thin_roadside_triangle_has_no_false_wall() {
    let mut grid = omsi_geometry::DriveGrid::default();
    grid.push([Vec3::ZERO, Vec3::new(30.0, 0.0, 0.0), Vec3::new(0.0, 50.0, 0.0)]);
    grid.push([Vec3::new(30.0, 0.0, 0.0), Vec3::new(30.0, 50.0, 0.0), Vec3::new(0.0, 50.0, 0.0)]);
    // Entirely beside the bus's wheel track. A tolerance on the infinite
    // edge lines extended this face into the road and extrapolated its height.
    grid.push([Vec3::new(10.0, 10.0, 0.0), Vec3::new(12.0, 9.6, 0.8), Vec3::new(10.0, 9.999, 0.0)]);
    grid.build(300.0);
    let ground = |x: f64, y: f64, top: f64| {
        let p = grid.probe(x as f32, y as f32, top as f32);
        GroundProbe { below: p.below.map(f64::from), above: p.above.map(f64::from) }
    };
    for fps in [30, 60] {
        for kmh in [5.0, 40.0, 45.0, 60.0] {
            let mut rb = RigidBody::from_definition(&bus(), &[]);
            rb.place(DVec3::new(14.0, 4.0, 0.0), 0.0);
            run(&mut rb, 2.0, 0.0, 0.0, &ground);
            let speed = kmh / 3.6;
            rb.velocity = Vec3::Y * speed;
            for w in &mut rb.wheels { w.spin = speed / w.radius; }
            let radius = rb.wheels.iter().find(|w| w.driven).unwrap().radius;
            let dt = 1.0 / fps as f32;
            // Hold speed with drive torque so the slow case crosses too.
            for _ in 0..(18.0 / speed / dt).ceil() as usize {
                let torque = ((speed - rb.forward_speed()) * rb.mass * 2.0 + rb.rolling_resistance) * radius;
                rb.step(dt, torque, &[0.0; 4], 0.0, &ground);
                assert!(rb.wheel_impacts.is_empty(), "{kmh} km/h at {fps} fps: {:?}", rb.wheel_impacts);
                assert!(rb.wheels.iter().all(|w| w.walls.is_empty()), "{kmh} km/h at {fps} fps: false wheel wall");
            }
            assert!(rb.position.y > 20.0, "{kmh} km/h at {fps} fps: stopped at {:?}", rb.position);
            assert!(rb.forward_speed() > speed * 0.9, "{kmh} km/h at {fps} fps: lost speed");
        }
    }
}

/// `Axle_Steering_*` is the axle's angle, one for both sides, the same to the left as to
/// the right (#953): the tyres keep their own Ackermann angles for the physics.
#[test]
fn the_axle_steers_as_far_left_as_right() {
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    let mut at = |steer: f32| {
        for _ in 0..5 {
            rb.step(1.0 / 60.0, 0.0, &[0.0; 4], steer, &g);
        }
        (rb.axle_steer(0), rb.axle_steer(1), rb.wheels[0].steer, rb.wheels[1].steer)
    };
    let (l0, r0, tl0, tr0) = at(-1.0);
    let (l1, r1, tl1, tr1) = at(1.0);
    assert_eq!(l0, r0);
    assert_eq!(l1, r1);
    assert!((l0 + l1).abs() < 1e-6, "left {l0} right {l1}");
    // (the full lock of 0.13 1/m, 5.94 m ahead of the turning line)
    assert!((l1 - (5.938f32 * 0.13).atan()).abs() < 1e-3, "{l1}");
    // the inner tyre turns further than the outer
    assert!(tl0.abs() > tr0.abs() && tr1.abs() > tl1.abs(), "{tl0} {tr0} {tl1} {tr1}");
}

/// A brake stronger than the road takes locks the wheels (the ABS scripts see the
/// wheel stand while the bus still moves); a brake the road takes keeps them turning.
#[test]
fn a_brake_beyond_the_grip_locks_the_wheels() {
    for (brake, locks) in [(60_000.0, true), (8_000.0, false)] {
        let mut rb = RigidBody::from_definition(&bus(), &[]);
        rb.place(DVec3::ZERO, 0.0);
        let g = road(1e9, 0.0);
        run(&mut rb, 2.0, 0.0, 0.0, &g);
        rb.velocity = rb.orientation.mul_vec3(Vec3::Y) * 12.0;
        for w in rb.wheels.iter_mut() {
            w.spin = 12.0 / w.radius;
        }
        run(&mut rb, 0.6, 0.0, brake, &g);
        let v = rb.velocity.length();
        let locked = rb.wheels.iter().all(|w| w.spin.abs() * w.radius < 0.5);
        assert!(v > 5.0, "still moving: {v}");
        assert_eq!(locked, locks, "brake {brake}: spins {:?} at {v} m/s", rb.wheels.iter().map(|w| w.spin).collect::<Vec<_>>());
    }
}

/// The NL202 sits about 0.10 m below its unloaded height - its own `ai_deltaheight` -
/// with the wheels exactly on the road.
#[test]
fn sags_by_its_own_springs() {
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    run(&mut rb, 3.0, 0.0, 5000.0, &g);
    let origin = rb.origin();
    assert!((origin.z + 0.097).abs() < 0.01, "origin {origin:?}");
    for w in &rb.wheels {
        assert!((w.compression - 0.097).abs() < 0.015, "compression {}", w.compression);
        assert!(w.ground_z.abs() < 1e-6);
    }
    // a softer air spring lowers it further
    let mut soft = RigidBody::from_definition(&bus(), &[]);
    soft.wheels.iter_mut().for_each(|w| w.spring_factor = 0.5);
    soft.place(DVec3::ZERO, 0.0);
    run(&mut soft, 3.0, 0.0, 5000.0, &g);
    assert!(soft.origin().z < origin.z - 0.08, "{:?}", soft.origin());
}

/// A 6 cm hump 0.6 m long (a manhole cover's rim, a speed cushion's edge) crossed at
/// 40 km/h: the wheels ride over it without leaving the road for long, the body
/// hardly lifts, and a wheel's travel changes smoothly from frame to frame instead of
/// jumping into the arch and out to full droop.
#[test]
fn a_long_frame_does_not_bounce_the_body() {
    // driving on flat road, one frame held up for 60 ms (loading): the body keeps still
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    run(&mut rb, 3.0, 3000.0, 5000.0, &g);
    let before: Vec<f32> = rb.wheels.iter().map(|w| w.compression).collect();
    rb.step(0.1, 3000.0, &[0.0; 4], 0.0, &g);
    let mut worst = 0.0f32;
    for _ in 0..30 {
        rb.step(1.0 / 60.0, 3000.0, &[0.0; 4], 0.0, &g);
        for (w, b) in rb.wheels.iter().zip(&before) {
            worst = worst.max((w.compression - b).abs());
        }
    }
    assert!(worst < 0.02, "the suspension moved {worst} m after a long frame");
}

#[test]
fn rides_over_a_hump() {
    let hump = |_x: f64, y: f64, z_top: f64| {
        let z = if (20.0..20.6).contains(&y) { 0.06 } else { 0.0 };
        if z <= z_top {
            GroundProbe { below: Some(z), above: None }
        } else {
            GroundProbe { below: Some(0.0), above: Some(z) }
        }
    };
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    run(&mut rb, 2.0, 0.0, 5000.0, &hump);
    rb.velocity = rb.orientation.mul_vec3(Vec3::Y) * (40.0 / 3.6);
    let z0 = rb.position.z;
    let (mut lift, mut jump, mut air) = (0.0f64, 0.0f32, 0usize);
    let mut last: Vec<f32> = rb.wheels.iter().map(|w| w.compression).collect();
    for _ in 0..(3.0 * 60.0) as usize {
        rb.step(1.0 / 60.0, 0.0, &[0.0; 4], 0.0, &hump);
        lift = lift.max(rb.position.z - z0);
        for (k, w) in rb.wheels.iter().enumerate() {
            jump = jump.max((w.compression - last[k]).abs());
            last[k] = w.compression;
            if !w.on_ground {
                air += 1;
            }
        }
    }
    assert!(rb.origin().y > 30.0, "{:?}", rb.origin());
    assert!(lift < 0.05, "the body lifted {lift:.3} m");
    // (OMSI's wheel stands on the point under it: at the hump's edge its travel
    // takes the hump's height at once, no more)
    assert!(jump < 0.065, "a wheel's travel jumped {jump:.3} m in a frame");
    assert!(air < 16, "wheels off the road for {air} wheel-frames");
}

/// Drive at `torque` (N m) for `secs` from rest against the ground `g`; where the front
/// axle ends up.
fn climb(g: &dyn Fn(f64, f64, f64) -> GroundProbe, torque: f32, secs: f32) -> (RigidBody, f64) {
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    run(&mut rb, 1.0, 0.0, 5000.0, g);
    run(&mut rb, secs, torque, 0.0, g);
    let front = rb.origin().y + 3.238;
    (rb, front)
}

/// A 15 cm kerb takes a firm push from a standstill (full torque: about 20 kN at the
/// rear wheels) but not the idle creep of 3 kN; a lowered kerb of 3 cm the creep takes;
/// a 60 cm wall stops the bus whatever it does.
#[test]
fn climbs_a_kerb_but_not_a_wall() {
    let kerb = road(6.0, 0.15);
    let (rb, front) = climb(&kerb, 9500.0, 10.0);
    assert!(front > 9.0, "front axle at {front}");
    assert!((rb.wheels[0].ground_z - 0.15).abs() < 0.01, "front wheel on the kerb: {}", rb.wheels[0].ground_z);
    let (_, front) = climb(&kerb, 1400.0, 10.0);
    assert!(front < 6.0, "idle creep climbed the kerb: front axle at {front}");
    let lowered = road(6.0, 0.03);
    let (_, front) = climb(&lowered, 1400.0, 15.0);
    assert!(front > 7.0, "idle creep stuck at a lowered kerb: front axle at {front}");

    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    let wall = road(6.0, 0.6);
    run(&mut rb, 1.0, 0.0, 5000.0, &wall);
    run(&mut rb, 12.0, 6000.0, 0.0, &wall);
    let front = rb.origin().y + 3.238;
    assert!(front < 6.0 && front > 5.3, "front axle at {front}");
    // and it backs off at once
    let y0 = rb.origin().y;
    run(&mut rb, 3.0, -6000.0, 0.0, &wall);
    assert!(rb.origin().y < y0 - 1.0, "{} -> {}", y0, rb.origin().y);
}

/// A pusher's front section has no driven wheels: the push of the rear section's axle
/// comes through the joint and moves it like the same torque on its own axle, the towed
/// mass slows the start, and the brakes of either part hold the train.
#[test]
fn pusher_front_section_is_pushed_through_the_joint() {
    let mut def = bus();
    def.axles.iter_mut().for_each(|a| a.driven = false);
    let rear = CoupledPart { point: Vec3::new(0.0, -4.456, 0.503), dir: Vec3::Y, mass: 0.0, driven_wheels: 2, radius: 0.47, driven_load: 70_000.0, brake: 0.0, load: 70_000.0 };
    let g = road(1e9, 0.0);
    let start = |def: &Vehicle, parts: Vec<CoupledPart>| {
        let mut rb = RigidBody::from_definition(def, &[]);
        rb.coupled = parts;
        rb.place(DVec3::ZERO, 0.0);
        run(&mut rb, 1.0, 0.0, 5000.0, &g);
        rb
    };
    // held by the front section's brakes, then by the rear section's alone
    let mut rb = start(&def, vec![rear]);
    run(&mut rb, 2.0, 9000.0, 20_000.0, &g);
    assert!(rb.origin().y.abs() < 0.05, "crept to {:?}", rb.origin());
    rb.coupled[0].brake = 80_000.0;
    run(&mut rb, 2.0, 9000.0, 0.0, &g);
    assert!(rb.origin().y.abs() < 0.05, "crept to {:?}", rb.origin());
    rb.coupled[0].brake = 0.0;
    run(&mut rb, 5.0, 9000.0, 0.0, &g);
    let pushed = rb.origin().y;
    // the same torque on its own rear axle
    let mut own = start(&bus(), Vec::new());
    run(&mut own, 5.0, 9000.0, 0.0, &g);
    let driven = own.origin().y;
    assert!(pushed > 5.0 && (pushed - driven).abs() < 0.1 * driven, "pushed {pushed} m, driven {driven} m");
    let (heading, _, _) = rb.heading_pitch_bank();
    assert!(rb.origin().x.abs() < 0.05 && heading.min(360.0 - heading) < 0.5, "{:?} {heading}", rb.origin());
    // the rear section's brakes stop the train
    let v = rb.forward_speed();
    rb.coupled[0].brake = 30_000.0;
    run(&mut rb, 1.0, 0.0, 0.0, &g);
    assert!(rb.forward_speed() < v - 2.0, "{v} -> {}", rb.forward_speed());
    // and backwards
    rb.coupled[0].brake = 0.0;
    run(&mut rb, 1.0, 0.0, 20_000.0, &g);
    let y0 = rb.origin().y;
    run(&mut rb, 6.0, -9000.0, 0.0, &g);
    assert!(rb.origin().y < y0 - 1.0, "{} -> {}", y0, rb.origin().y);
    // a towed mass as heavy as the body halves the acceleration
    let mut heavy = start(&def, vec![CoupledPart { mass: rb.mass, ..rear }]);
    run(&mut heavy, 5.0, 9000.0, 0.0, &g);
    let ratio = heavy.origin().y / pushed;
    assert!((ratio - 0.5).abs() < 0.08, "heavy train went {} m, light {pushed} m", heavy.origin().y);
    // a joint bent by 30 degrees pushes with its share along the body and does not turn
    // it, standing against the brakes or driving
    let bent_dir = Vec3::new(0.5, 0.866, 0.0);
    let mut bent = start(&def, vec![CoupledPart { dir: bent_dir, brake: 30_000.0, ..rear }]);
    run(&mut bent, 5.0, 9000.0, 20_000.0, &g);
    let (heading, _, _) = bent.heading_pitch_bank();
    assert!(bent.origin().truncate().length() < 0.05 && heading.min(360.0 - heading) < 0.2, "{:?} {heading}", bent.origin());
    bent.coupled[0].brake = 0.0;
    run(&mut bent, 5.0, 9000.0, 0.0, &g);
    let (heading, _, _) = bent.heading_pitch_bank();
    let ratio = bent.origin().y / pushed;
    assert!((ratio - 0.866).abs() < 0.08 && heading.min(360.0 - heading) < 0.5, "{:?} {heading} ({ratio})", bent.origin());
}

/// A towed part on a grade pulls the body down it, and its brakes hold it there.
#[test]
fn towed_weight_on_a_grade() {
    let mut def = bus();
    def.axles.iter_mut().for_each(|a| a.driven = false);
    let g = ramp(0.08);
    let part = CoupledPart { point: Vec3::new(0.0, -4.456, 0.503), dir: Vec3::Y, mass: 7000.0, driven_wheels: 0, radius: 0.47, driven_load: 0.0, brake: 0.0, load: 70_000.0 };
    let mut rb = RigidBody::from_definition(&def, &[]);
    rb.coupled = vec![CoupledPart { brake: 40_000.0, ..part }];
    rb.place(DVec3::new(0.0, 50.0, 4.0), 0.0);
    run(&mut rb, 3.0, 0.0, 0.0, &g);
    let y0 = rb.origin().y;
    run(&mut rb, 3.0, 0.0, 0.0, &g);
    assert!((rb.origin().y - y0).abs() < 0.02, "rolled from {y0} to {}", rb.origin().y);
    rb.coupled[0].brake = 0.0;
    run(&mut rb, 3.0, 0.0, 0.0, &g);
    assert!(rb.origin().y < y0 - 1.0, "stayed at {}", rb.origin().y);
}

/// A ramp of grade `s` along y (from y = 0), steps over `CLIMB` radii read as faces.
pub(super) fn ramp(s: f64) -> impl Fn(f64, f64, f64) -> GroundProbe {
    move |_x, y, top| {
        let z = y * s;
        if z <= top { GroundProbe { below: Some(z), above: None } } else { GroundProbe { below: None, above: Some(z) } }
    }
}

/// A step asks the ground for a few points per tyre - the lattice and a look to either
/// side - on the flat and on any grade alike (the comb read every grade over 6.4 % as a
/// run of kerbs and asked for 73 per tyre).
#[test]
fn a_step_asks_for_few_points() {
    for s in [0.0, 0.05, 0.08, 0.12, 0.2] {
        let count = std::cell::Cell::new(0);
        let g = ramp(s);
        let probe = |x: f64, y: f64, top: f64| {
            count.set(count.get() + 1);
            g(x, y, top)
        };
        let mut rb = RigidBody::from_definition(&bus(), &[]);
        rb.place(DVec3::new(0.0, 10.0, 10.0 * s), 0.0);
        run(&mut rb, 1.0, 0.0, 30_000.0, &probe);
        count.set(0);
        rb.step(1.0 / 60.0, 0.0, &[30_000.0; 4], 0.0, &probe);
        // (and the point under the wheel OMSI's suspension stands on)
        assert!(count.get() <= 4 * rb.wheels.len() * (LATTICE + 6), "{s}: {} probes", count.get());
    }
}

/// Released on grades from 4 % to 16 %, the bus rolls down with gravity's full pull along
/// the slope less the rolling resistance.
#[test]
fn rolls_down_a_grade_at_its_own_pace() {
    for pct in [4.0f64, 8.0, 12.0, 16.0] {
        let s = pct / 100.0;
        let g = ramp(s);
        let mut rb = RigidBody::from_definition(&bus(), &[]);
        rb.place(DVec3::ZERO, 0.0);
        run(&mut rb, 3.0, 0.0, 30_000.0, &g);
        run(&mut rb, 0.5, 0.0, 0.0, &g);
        let v0 = rb.forward_speed();
        run(&mut rb, 2.0, 0.0, 0.0, &g);
        let accel = (rb.forward_speed() - v0) / 2.0;
        let want = -(9.81 * s / (1.0 + s * s).sqrt()) as f32 + 1000.0 / rb.mass;
        assert!((accel - want).abs() < 0.04 * want.abs(), "{pct} %: {accel} m/s², want {want}");
    }
}

/// A step close to the tyre's radius (0.45 m on 0.47 m, which the hub-and-5-cm limit
/// still climbed) is a wall, whatever the push or the run-up; one of 0.3 m is climbed
/// with a run-up.
#[test]
fn a_step_near_the_radius_is_a_wall() {
    let step = road(6.0, 0.45);
    let (rb, front) = climb(&step, 9500.0, 10.0);
    assert!(front < 6.0 - 0.4, "front axle at {front}");
    assert!(rb.wheels[0].ground_z.abs() < 0.01, "on the step: {}", rb.wheels[0].ground_z);
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    run(&mut rb, 1.0, 0.0, 5000.0, &step);
    rb.velocity = Vec3::new(0.0, 15.0 / 3.6, 0.0);
    run(&mut rb, 5.0, 9500.0, 0.0, &step);
    let front = rb.origin().y + 3.238;
    assert!(front < 6.0 - 0.4, "front axle at {front} after a run-up");
    let low = road(6.0, 0.3);
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    run(&mut rb, 1.0, 0.0, 5000.0, &low);
    rb.velocity = Vec3::new(0.0, 15.0 / 3.6, 0.0);
    run(&mut rb, 5.0, 9500.0, 0.0, &low);
    assert!(rb.origin().y > 6.0, "stuck at {} before a 0.3 m step", rb.origin().y);
    assert!(rb.wheels.iter().all(|w| (w.ground_z - 0.3).abs() < 0.01), "{:?}", rb.wheels.iter().map(|w| w.ground_z).collect::<Vec<_>>());
}

/// A platform's face stops the front wheels at any speed and frame rate (the body has no
/// collision box here: a spline's faces are not in the collision world), without
/// throwing the bus about.
#[test]
fn a_face_stops_the_wheels_at_any_speed() {
    let wall = road(6.0, 0.6);
    for fps in [60.0f32, 30.0, 20.0] {
        for kmh in [10.0f32, 30.0, 50.0, 80.0] {
            let mut rb = RigidBody::from_definition(&bus(), &[]);
            rb.place(DVec3::new(0.0, -10.0, 0.0), 0.0);
            run(&mut rb, 1.0, 0.0, 5000.0, &wall);
            rb.velocity = Vec3::new(0.0, kmh / 3.6, 0.0);
            let kinetic = rb.kinetic_energy();
            let (mut front, mut top, mut crash) = (f64::MIN, f64::MIN, 0.0);
            for _ in 0..(fps * 6.0) as usize {
                rb.step(1.0 / fps, 0.0, &[0.0; 4], 0.0, &wall);
                front = front.max(rb.origin().y + 3.238);
                top = top.max(rb.origin().z);
                for hit in rb.wheel_impacts.iter().filter(|h| h.speed > CRASH_SPEED && h.energy > 1000.0) {
                    assert!(hit.obstacle < 2 && hit.point.y > 3.238 && hit.point.z < 1.1, "{hit:?}");
                    crash += hit.energy;
                }
            }
            // the crash is the wheels' (the bus has no box here), and it took most of the
            // bus's motion
            assert!(crash > 0.5 * kinetic && crash < 1.05 * kinetic, "{kmh} km/h at {fps} fps: crash of {crash} J for {kinetic} J");
            assert!(front < 6.0 - 0.47 + 0.03, "{kmh} km/h at {fps} fps: front axle reached {front}");
            assert!(top < 0.3, "{kmh} km/h at {fps} fps: thrown up to {top}");
            // (a rebound of a metre or so: with the moments of inertia on OMSI's axes
            // the body's lower yaw inertia takes a little more of the blow back)
            assert!(rb.forward_speed().abs() < 1.5, "{kmh} km/h at {fps} fps: still at {}", rb.forward_speed());
        }
    }
}

/// Sliding sideways into a platform along the bus, the tyres stop at its face instead of
/// running under it.
#[test]
fn a_face_beside_the_tyres_stops_a_slide() {
    let platform = |x: f64, _y: f64, top: f64| {
        let z = if x >= 1.5 { 0.6 } else { 0.0 };
        if z <= top { GroundProbe { below: Some(z), above: None } } else { GroundProbe { below: Some(0.0), above: Some(z) } }
    };
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    run(&mut rb, 1.0, 0.0, 0.0, &platform);
    rb.velocity = Vec3::new(8.0, 0.0, 0.0);
    let mut right = f64::MIN;
    for _ in 0..120 {
        rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &platform);
        for w in rb.wheels.iter().filter(|w| w.attach.x > 0.0) {
            let hub = rb.position + rb.orientation.mul_vec3(w.attach - rb.cog).as_dvec3();
            right = right.max(hub.x);
        }
    }
    let limit = 1.5 - (HALF_WIDTH * 0.47) as f64;
    assert!(right < limit + 0.02, "a right hub reached {right} (face at 1.5)");
    assert!(rb.wheels.iter().filter(|w| w.attach.x > 0.0).all(|w| !w.walls.is_empty()), "{:?}", rb.wheels.iter().map(|w| &w.walls).collect::<Vec<_>>());
    let (_, _, bank) = rb.heading_pitch_bank();
    assert!(bank.abs() < 5.0, "banked {bank}");
}

/// On an 8 % slope the parking brake (rear wheels only) holds the bus; released, it rolls
/// back down; and a driven bus brakes to a stand on the flat.
#[test]
fn holds_on_a_slope_and_brakes_to_a_stand() {
    let slope = |_x: f64, y: f64, top: f64| {
        let z = y * 0.08;
        if z <= top { GroundProbe { below: Some(z), above: None } } else { GroundProbe { below: None, above: Some(z) } }
    };
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    let park = [0.0, 0.0, 20_000.0, 20_000.0];
    for _ in 0..180 {
        rb.step(1.0 / 60.0, 0.0, &park, 0.0, &slope);
    }
    let y0 = rb.origin().y;
    for _ in 0..600 {
        rb.step(1.0 / 60.0, 0.0, &park, 0.0, &slope);
    }
    let y1 = rb.origin().y;
    for _ in 0..600 {
        rb.step(1.0 / 60.0, 0.0, &park, 0.0, &slope);
    }
    eprintln!("parked on 8 %: {:+.4} m in the first 10 s, {:+.4} m in the next", y1 - y0, rb.origin().y - y1);
    assert!((rb.origin().y - y1).abs() < 0.005, "crept {} m", rb.origin().y - y1);
    for _ in 0..180 {
        rb.step(1.0 / 60.0, 0.0, &[0.0; 4], 0.0, &slope);
    }
    assert!(rb.origin().y < y0 - 1.0 && rb.forward_speed() < -0.5, "rolled back to {} at {}", rb.origin().y, rb.forward_speed());

    let flat = road(1e9, 0.0);
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    run(&mut rb, 0.5, 0.0, 0.0, &flat);
    rb.velocity = Vec3::new(0.0, 50.0 / 3.6, 0.0);
    let mut t = 0.0;
    while rb.forward_speed() > 0.01 && t < 10.0 {
        rb.step(1.0 / 60.0, 0.0, &[15_000.0; 4], 0.0, &flat);
        t += 1.0 / 60.0;
    }
    // 60 kN on 10.2 t: about 5.9 m/s², so 50 km/h are gone in some 2.4 s
    assert!(t > 2.0 && t < 3.0, "stopped after {t} s");
    let y = rb.origin().y;
    run(&mut rb, 3.0, 0.0, 15_000.0, &flat);
    assert!((rb.origin().y - y).abs() < 0.02, "moved {} m after stopping", rb.origin().y - y);
}

/// The same full braking from 50 km/h on a dry road and on black ice: the tyres cannot
/// pass the brakes' 5.9 m/s² on to the ice, and the bus slides about five times as far.
#[test]
fn brakes_slide_on_ice() {
    let stop = |mu: f32| {
        let flat = road(1e9, 0.0);
        let mut rb = RigidBody::from_definition(&bus(), &[]);
        rb.place(DVec3::ZERO, 0.0);
        run(&mut rb, 0.5, 0.0, 0.0, &flat);
        rb.friction = mu;
        rb.velocity = Vec3::new(0.0, 50.0 / 3.6, 0.0);
        let y0 = rb.origin().y;
        let mut t = 0.0;
        while rb.forward_speed() > 0.01 && t < 30.0 {
            rb.step(1.0 / 60.0, 0.0, &[15_000.0; 4], 0.0, &flat);
            t += 1.0 / 60.0;
        }
        rb.origin().y - y0
    };
    let (dry, ice) = (stop(crate::vehicle::road_grip(0.0, 15.0)), stop(crate::vehicle::road_grip(0.4, -6.0)));
    eprintln!("50 km/h to a stand: {dry:.1} m dry, {ice:.1} m on ice");
    assert!(dry > 12.0 && dry < 20.0, "dry {dry}");
    assert!(ice > dry * 4.0, "ice {ice} dry {dry}");
}

/// A drive torque, steering or brake force that is no number (a script's 0/0) is taken
/// as none: the body keeps its place and speed instead of turning to NaN (#1045).
#[test]
fn inputs_that_are_no_number_leave_the_body_whole() {
    for (torque, brake, steer) in [(f32::NAN, 0.0, 0.0), (3000.0, f32::INFINITY, 0.0), (3000.0, f32::NAN, 0.0), (3000.0, 0.0, f32::NAN), (f32::NEG_INFINITY, 0.0, 0.0)] {
        let mut rb = RigidBody::from_definition(&bus(), &[]);
        rb.place(DVec3::ZERO, 0.0);
        let g = road(1e9, 0.0);
        run(&mut rb, 1.0, 3000.0, 0.0, &g);
        for _ in 0..60 {
            rb.step(1.0 / 60.0, torque, &[brake; 4], steer, &g);
        }
        assert!(rb.position.is_finite() && rb.velocity.is_finite(), "{torque} {brake} {steer}: {:?} {:?}", rb.position, rb.velocity);
        assert!(rb.forward_speed() > 0.1, "{torque} {brake} {steer}: {}", rb.forward_speed());
        assert!(rb.wheels.iter().all(|w| w.compression.is_finite() && w.spin.is_finite()));
    }
    // a body whose own values are no number stays put
    let mut rb = RigidBody::from_definition(&bus(), &[]);
    rb.place(DVec3::ZERO, 0.0);
    let g = road(1e9, 0.0);
    run(&mut rb, 0.5, 0.0, 0.0, &g);
    let at = rb.position;
    rb.mass = f32::NAN;
    rb.step(1.0 / 60.0, 3000.0, &[0.0; 4], 0.0, &g);
    assert!(rb.position.is_finite() && (rb.position - at).length() < 1e-9, "{:?}", rb.position);
    assert_eq!(rb.velocity, Vec3::ZERO);
}

#[test]
fn the_mass_is_in_tonnes_whatever_the_number() {
    // as OMSI reads `[mass]`: a file that gives kilograms weighs a thousand times as much, a
    // light one keeps its weight, and a file without one weighs 1 t
    let kilograms = Vehicle { mass: 10_200.0, ..bus() };
    assert_eq!(mass_kg(&kilograms), 10_200_000.0);
    assert_eq!(mass_kg(&Vehicle { mass: 0.2, ..bus() }), 200.0);
    assert_eq!(mass_kg(&Vehicle { mass_missing: true, ..bus() }), 1000.0);
    assert_eq!(RigidBody::from_definition(&kilograms, &[]).mass(), 10_200_000.0);
    // the kinematic model (the AI's copies) reads it as it always has
    assert_eq!(kinematic_mass_kg(&kilograms), 10_200.0);
    assert_eq!(kinematic_mass_kg(&Vehicle { mass: 0.2, ..bus() }), 500.0);
}

#[test]
fn the_moments_of_inertia_are_in_tonne_square_metres() {
    // no unit guess, no minimum: a file's 6000 is 6000 t m², its 0.05 is 50 kg m², and a file
    // without the section gets OMSI's 100 t m² about each axis
    let i = |moi: [f32; 3]| PhysicsSpec::from(&Vehicle { moment_of_inertia: moi, ..bus() }).inertia;
    assert_eq!(i([6000.0, 90.0, 400.0]), Vec3::new(6_000_000.0, 90_000.0, 400_000.0));
    assert_eq!(i([0.05, 0.05, 0.05]), Vec3::splat(50.0));
    assert_eq!(i([0.0; 3]), Vec3::splat(100_000.0));
}

#[test]
fn an_axle_without_springs_carries_nothing() {
    // OMSI's `[newachse]` leaves `achse_feder`, `achse_daempfer` and `achse_maxforce` at zero
    // when the file names none: the strut pushes nothing and the body sinks to the ground
    let mut def = bus();
    for a in def.axles.iter_mut() {
        (a.spring, a.damper, a.max_force) = (0.0, 0.0, 0.0);
    }
    let spec = AxleSpec::of(&def.axles[0]);
    assert_eq!((spec.spring, spec.damper, spec.max_force), (0.0, 0.0, 0.0));
    assert_eq!(PhysicsSpec::from(&def).body_freq, 0.0);
    let mut rb = RigidBody::from_definition(&def, &[]);
    rb.place(DVec3::ZERO, 0.0);
    run(&mut rb, 1.0, 0.0, 0.0, &road(1e9, 0.0));
    assert!(rb.wheels.iter().all(|w| w.load() == 0.0), "{:?}", rb.wheels.iter().map(|w| w.load()).collect::<Vec<_>>());
    assert!(rb.position().z.is_finite());
}

#[test]
fn rolling_resistance_is_the_files_force() {
    // `[rollwiderstand]` is a force spread over the wheels, whatever the weight; a file
    // without one rolls without it, as in OMSI
    assert_eq!(PhysicsSpec::from(&bus()).rolling_resistance, 1000.0);
    assert_eq!(PhysicsSpec::from(&Vehicle { rolling_resistance: 0.0, ..bus() }).rolling_resistance, 0.0);
    assert_eq!(RigidBody::from_definition(&Vehicle { rolling_resistance: 0.0, ..bus() }, &[]).rolling_resistance(), 0.0);
}

#[test]
fn the_centre_of_gravity_is_as_high_as_schwerpunkt() {
    // `[schwerpunkt]` is the lever the weight shifts by; `[cog]`'s height places OMSI's
    // collision box, not the weight, and a file without either has its weight on the ground
    let cog = |def: Vehicle| PhysicsSpec::from(&def).cog.z;
    assert_eq!(cog(bus()), 1.2);
    assert_eq!(cog(Vehicle { cog_height: 0.0, cog: Some([0.0, 0.5, 0.9]), ..bus() }), 0.0);
    assert_eq!(cog(Vehicle { cog_height: 0.0, ..bus() }), 0.0);
}

#[test]
fn the_wheels_stand_where_the_axle_says() {
    // no minimum radius or width and no share of the outer width: the tyres stand halfway
    // between `achse_minwidth` and `achse_maxwidth`, the strut pushes at half the outer width
    let a = Axle { long: 1.0, max_width: 0.4, min_width: 0.0, wheel_diameter: 0.2, spring: 100.0, max_force: 10.0, damper: 1.0, driven: false, inertia_inv: 0.0 };
    let spec = AxleSpec::of(&a);
    assert_eq!((spec.radius, spec.outer, spec.track), (0.1, 0.2, 0.1));
    // the kinematic model's parts stand as they always have
    assert_eq!((spec.kinematic_radius, spec.kinematic_track), (0.15, 0.3 * 0.85));
}

#[test]
fn the_lock_is_the_files_turning_radius() {
    // `[inv_min_turnradius]` as it is: no range, and a file without one does not steer
    let spec = PhysicsSpec::from(&Vehicle { inv_min_turn_radius: 0.5, ..bus() });
    assert_eq!(spec.inv_min_turn_radius, 0.5);
    assert!(spec.max_steer_deg > 70.0, "{}", spec.max_steer_deg);
    let none = PhysicsSpec::from(&Vehicle { inv_min_turn_radius: 0.0, ..bus() });
    assert_eq!((none.inv_min_turn_radius, none.max_steer_deg), (0.0, 0.0));
    let mut rb = RigidBody::from_definition(&Vehicle { inv_min_turn_radius: 0.0, ..bus() }, &[]);
    rb.place(DVec3::ZERO, 0.0);
    for _ in 0..60 {
        rb.step(1.0 / 60.0, 0.0, &[0.0; 4], 1.0, &road(1e9, 0.0));
    }
    assert_eq!(rb.kappa(), 0.0);
    // the kinematic model keeps its lock
    assert_eq!((spec.kinematic_max_steer_deg, none.kinematic_max_steer_deg), (60.0, 10.0));
}

#[test]
fn a_wheel_far_from_the_weight_carries_nothing() {
    // the centre of gravity behind the rear axle: the front wheels carry nothing (an OMSI
    // strut only pushes), not a fifth of an even share
    let def = Vehicle { cog: Some([0.0, -4.0, 1.2]), ..bus() };
    let loads = wheel_rest_loads(&def);
    assert_eq!(loads[0], 0.0);
    let (loads, _) = wheel_rest_loads_supported(&Vehicle { cog: Some([0.0, -8.0, 1.2]), ..bus() }, 6.0);
    assert_eq!(loads[0], 0.0, "{loads:?}");
    // the kinematic model's parts keep their least share
    assert!(kinematic_wheel_rest_loads(&def)[0] > 0.0);
}

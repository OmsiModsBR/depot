use glam::{DVec2, DVec3};
use omsi_sim::testing::ai::*;
use omsi_vehicle::vehicle::Axle;
use omsi_vehicle::Vehicle;

#[test]
fn back_in_ramp_keeps_the_sideways_acceleration() {
    for (offset, kmh) in [(3.28f32, 40.0f32), (3.28, 44.0), (2.5, 34.0), (5.0, 50.0)] {
        let v = kmh / 3.6;
        let len = back_in_ramp(offset, v, BACK_IN_LAT_ACCEL);
        // the peak of the smoothstep's second derivative, at its ends
        let peak = 6.0 * offset * v * v / (len * len);
        assert!(
            peak <= BACK_IN_LAT_ACCEL + 1e-3,
            "{offset} m at {kmh} km/h: {len:.1} m, {peak:.2} m/s²"
        );
        assert!(
            len / v >= 2.0,
            "{offset} m at {kmh} km/h: {len:.1} m is {:.2} s",
            len / v
        );
    }
    // creeping round something: the old geometric length
    assert_eq!(back_in_ramp(3.0, 1.0, BACK_IN_LAT_ACCEL), 10.5);
}

fn golf() -> Vehicle {
    let mut v = Vehicle {
        mass: 1.0,
        moment_of_inertia: [1.49, 0.40, 1.56],
        cog_height: 0.4,
        rot_pnt_long: -1.304,
        inv_min_turn_radius: 0.1904,
        ..Default::default()
    };
    v.axles = vec![
        Axle {
            long: 1.21,
            max_width: 1.606,
            spring: 40.0,
            damper: 2.0,
            wheel_diameter: 0.503,
            ..Default::default()
        },
        Axle {
            long: -1.304,
            max_width: 1.606,
            spring: 50.0,
            damper: 2.0,
            wheel_diameter: 0.503,
            ..Default::default()
        },
    ];
    v
}

/// A way: straight north for 50 m, then a right turn of radius `r`.
fn bend(r: f64, s: f64) -> DVec3 {
    if s < 50.0 {
        DVec3::new(0.0, s, 10.0)
    } else {
        let a = (s - 50.0) / r;
        DVec3::new(r - r * a.cos(), 50.0 + r * a.sin(), 10.0)
    }
}

#[test]
fn follows_a_bend_smoothly() {
    let def = golf();
    let mut body = AiBody::new(&def, MotionKind::Road);
    let speed = 6.0f32;
    let dt = 1.0 / 30.0;
    let mut s = 20.0f64;
    body.place(&|d| bend(12.0, s + d as f64), None, None, speed);
    let mut last_yaw: Option<f32> = None;
    let mut worst_jump = 0.0f32;
    let mut worst_off = 0.0f64;
    for _ in 0..(12.0 / dt) as usize {
        s += (speed * dt) as f64;
        let at = s;
        body.step(dt, speed, &|d| bend(12.0, at + d as f64), None, None);
        if let Some(y) = last_yaw {
            worst_jump = worst_jump.max((body.yaw_rate() - y).abs().to_degrees());
        }
        last_yaw = Some(body.yaw_rate());
        // the rear axle stays on the way (a circle has no steady error; entering it costs a little)
        let rear = body.rear();
        let q = bend(12.0, at + def.rot_pnt_long as f64).truncate();
        worst_off = worst_off.max((rear - q).length());
    }
    // a quarter turn done at 6 m/s: the heading has turned right, the yaw rate never
    // jumps by more than a few degrees per second between frames, the car keeps to its lane
    assert!(
        body.heading > 60.0 && body.heading < 300.0,
        "heading {}",
        body.heading
    );
    assert!(
        worst_jump < 3.0,
        "yaw rate jumps by {worst_jump} deg/s in one frame"
    );
    assert!(worst_off < 0.6, "rear axle {worst_off} m off the way");
}

#[test]
fn brakes_nose_down_and_rolls_out_of_a_turn() {
    let def = golf();
    let mut body = AiBody::new(&def, MotionKind::Road);
    let flat = |_x: f64, _y: f64| Some(10.0);
    body.place(
        &|d| DVec3::new(0.0, d as f64, 10.0),
        Some(&flat),
        None,
        10.0,
    );
    let dt = 1.0 / 30.0;
    let mut speed = 10.0f32;
    let mut s = 0.0f64;
    let mut min_pitch = 0.0f32;
    for _ in 0..60 {
        speed = (speed - 3.0 * dt).max(0.0);
        s += (speed * dt) as f64;
        let at = s;
        body.step(
            dt,
            speed,
            &|d| DVec3::new(0.0, at + d as f64, 10.0),
            Some(&flat),
            None,
        );
        min_pitch = min_pitch.min(body.pitch_deg);
    }
    assert!(min_pitch < -0.1, "braking pitch {min_pitch}");
    assert!((body.position.z - 10.0).abs() < 0.1);
    // the front wheels come up in their arches while the nose dives
    assert!(
        body.suspension[0][0] < 0.0 && body.suspension[1][0] > 0.0,
        "{:?}",
        body.suspension
    );
}

/// A two-axle lorry: 4.5 m between the axles, a turning circle of 11 m radius.
fn lorry() -> Vehicle {
    let mut v = Vehicle {
        mass: 12.0,
        moment_of_inertia: [40.0, 10.0, 45.0],
        cog_height: 1.2,
        rot_pnt_long: -2.0,
        inv_min_turn_radius: 0.09,
        ..Default::default()
    };
    v.axles = vec![
        Axle {
            long: 2.5,
            max_width: 2.0,
            spring: 300.0,
            damper: 20.0,
            wheel_diameter: 1.0,
            ..Default::default()
        },
        Axle {
            long: -2.0,
            max_width: 1.8,
            spring: 400.0,
            damper: 25.0,
            wheel_diameter: 1.0,
            ..Default::default()
        },
    ];
    v
}

/// A standing bus `half_width` × 2 wide, `gap` metres ahead of a vehicle whose front is `front`
/// metres ahead of its origin at (0, 0) facing north.
fn bus_ahead(front: f32, gap: f32, half_width: f64) -> omsi_sim::collision::Obb {
    omsi_sim::collision::Obb::vehicle(
        DVec2::new(0.0, (front + gap) as f64 + 6.0),
        0.0,
        6.0,
        6.0,
        half_width,
    )
}

#[test]
fn a_car_too_close_behind_a_bus_cannot_steer_round_it() {
    let def = golf();
    let extent = (2.1, 2.2, 0.85);
    let best = |gap: f32| {
        pull_out_ramps(gap, extent.0, false)
            .into_iter()
            .map(|ramp| {
                let way = straight_pull_out(3.3, ramp);
                let mut body = AiBody::new(&def, MotionKind::Road);
                body.place(&way, None, None, 0.0);
                body.sweep_clearance(
                    &way,
                    0.0,
                    PULL_OUT_WAIT,
                    PULL_OUT_ACCEL,
                    8.0,
                    gap + 6.0,
                    extent,
                    &[bus_ahead(extent.0, gap, 1.25)],
                )
            })
            .fold(f64::MIN, f64::max)
    };
    // half a metre behind the bus no S-curve gets the front corner past (the lock
    // is 25°, the steering takes a moment to get there); six metres back it is easy
    let close = best(0.5);
    assert!(close < PULL_OUT_CLEARANCE, "clearance {close} from 0.5 m");
    let far = best(6.0);
    assert!(far >= PULL_OUT_CLEARANCE, "clearance {far} from 6 m");
    // and the swept body does leave the lane: after the pull-out it is 3.3 m over
    let way = straight_pull_out(3.3, 8.0);
    let mut body = AiBody::new(&def, MotionKind::Road);
    body.place(&way, None, None, 0.0);
    let mut x = 0.0f32;
    let dt = 1.0 / 20.0;
    for _ in 0..400 {
        x += 4.0 * dt;
        let at = x;
        body.step(dt, 4.0, &|d| way(at + d), None, None);
    }
    assert!(
        (body.position.x + 3.3).abs() < 0.2,
        "ended at x {}",
        body.position.x
    );
}

#[test]
fn room_to_pull_out_depends_on_the_steering() {
    let car = pull_out_room(&golf(), (2.1, 2.2, 0.85), 1.25, 3.3);
    let lorry = pull_out_room(&lorry(), (3.6, 3.4, 1.25), 1.25, 3.3);
    // a bus standing a little further over (or wider) needs more room
    let wide = pull_out_room(&golf(), (2.1, 2.2, 0.85), 1.6, 3.3);
    assert!(wide > car, "{wide} against {car}");
    assert!(car > 1.5 && car < 6.0, "car {car}");
    assert!(
        lorry > car + 0.5 && lorry < 12.5,
        "lorry {lorry}, car {car}"
    );
    // what it found works: from that room less the half metre of slack one of the
    // curves clears, from a metre less none does
    let clears = |gap: f32| {
        pull_out_ramps(gap, 2.1, false).into_iter().any(|ramp| {
            let way = straight_pull_out(3.3, ramp);
            let mut body = AiBody::new(&golf(), MotionKind::Road);
            body.place(&way, None, None, 0.0);
            body.sweep_clearance(
                &way,
                0.0,
                PULL_OUT_WAIT,
                PULL_OUT_ACCEL,
                8.0,
                gap + 6.0,
                (2.1, 2.2, 0.85),
                &[bus_ahead(2.1, gap, 1.25)],
            ) >= PULL_OUT_CLEARANCE
        })
    };
    assert!(clears(car - 0.5));
    assert!(!clears(car - 1.0));
}

#[test]
fn pull_out_curves_run_gentle_to_steep() {
    let r = pull_out_ramps(3.0, 2.0, false);
    assert!(r.windows(2).all(|w| w[0] > w[1]), "{r:?}");
    assert!(r.iter().all(|&x| (4.0..=12.0).contains(&x)), "{r:?}");
    let rolling = pull_out_ramps(30.0, 2.0, true);
    assert_eq!(rolling[0], 20.0);
    assert!(
        rolling.iter().all(|&x| (4.0..=20.0).contains(&x)),
        "{rolling:?}"
    );
}

/// Every wheel stands on the face drawn under it, as OMSI's ground query stands an
/// AI car's (the highest face under a ray from 3 m over the wheel): a road drawn 8 cm
/// over the lane and cambered across it carries each wheel at its own height, and a lane
/// laid 0.9 m under its road has the car on the road, not inside it.
#[test]
fn each_wheel_stands_on_the_face_under_it() {
    let def = golf();
    for (lift, camber) in [(0.08f64, 0.03f64), (0.9, 0.0), (0.08, -0.05)] {
        let road = move |x: f64, _y: f64| 10.0 + lift + camber * x;
        let ground = move |x: f64, y: f64, top: f64| {
            let z = road(x, y);
            if z <= top {
                omsi_sim::physics::GroundProbe {
                    below: Some(z),
                    above: None,
                }
            } else {
                omsi_sim::physics::GroundProbe {
                    below: Some(9.0),
                    above: Some(z),
                }
            }
        };
        let way = |d: f32| DVec3::new(0.0, d as f64, 10.0);
        let mut body = AiBody::new(&def, MotionKind::Road);
        body.place(&way, None, Some(&ground), 0.0);
        let mut x = 0.0f32;
        for _ in 0..90 {
            x += 0.2;
            let at = x;
            body.step(1.0 / 30.0, 6.0, &|d| way(at + d), None, Some(&ground));
        }
        for (w, z) in body.wheels().iter().zip(body.contact_z()) {
            let p = body.position.truncate() + DVec2::new(w.lat as f64, w.long as f64);
            assert!(
                (z - road(p.x, p.y)).abs() < 0.01,
                "lift {lift} camber {camber}: wheel at {z:.3}, road {:.3}",
                road(p.x, p.y)
            );
        }
        assert!(
            (body.position.z - (10.0 + lift)).abs() < 0.02,
            "lift {lift} camber {camber}: body at {:.3}",
            body.position.z
        );
        let lean = (-camber).atan().to_degrees() as f32;
        assert!(
            (body.bank_deg - lean).abs() < 0.3,
            "camber {camber}: bank {} against {lean}",
            body.bank_deg
        );
    }
}

/// A lane laid half a metre over its drawn road (a junction's paths): with no wheel
/// near the lane's height the car comes down onto the road instead of driving in the air;
/// a road far below (under a bridge the lane crosses) still does not pull it down.
#[test]
fn a_car_over_a_lower_road_comes_down_onto_it() {
    let def = golf();
    for (drop, expect) in [(0.5f64, 9.5f64), (6.0, 10.0)] {
        let ground = move |_x: f64, _y: f64, top: f64| {
            let z = 10.0 - drop;
            omsi_sim::physics::GroundProbe {
                below: (z <= top).then_some(z),
                above: None,
            }
        };
        let way = |d: f32| DVec3::new(0.0, d as f64, 10.0);
        let mut body = AiBody::new(&def, MotionKind::Road);
        body.place(&way, None, Some(&ground), 0.0);
        let mut x = 0.0f32;
        for _ in 0..60 {
            x += 0.2;
            let at = x;
            body.step(1.0 / 30.0, 6.0, &|d| way(at + d), None, Some(&ground));
        }
        assert!(
            (body.position.z - expect).abs() < 0.02,
            "drop {drop}: body at {:.3}",
            body.position.z
        );
    }
}

#[test]
fn aircraft_keep_their_height() {
    let mut def = golf();
    def.axles.truncate(1);
    let mut body = AiBody::new(&def, MotionKind::Air);
    let way = |d: f32| DVec3::new(0.0, d as f64, 300.0 - d as f64 * 0.05);
    body.place(&way, None, None, 70.0);
    body.step(1.0 / 30.0, 70.0, &way, None, None);
    assert!((body.position.z - 300.0).abs() < 1e-6);
    assert!(
        body.pitch_deg < -2.0 && body.pitch_deg > -4.0,
        "pitch {}",
        body.pitch_deg
    );
}

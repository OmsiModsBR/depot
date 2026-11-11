//! The OMSI Classic AI body (`KA-1`, `KA-4`) on the conformance vehicle V1 (a 12 m solo bus).

use glam::DVec3;
use omsi_sim::testing::ai::*;
use omsi_vehicle::vehicle::Axle;
use omsi_vehicle::Vehicle;

fn bus() -> Vehicle {
    let axle = |long: f32| Axle {
        long,
        max_width: 2.4,
        min_width: 1.76,
        wheel_diameter: 0.94,
        spring: 260.0,
        damper: 20.0,
        ..Default::default()
    };
    Vehicle {
        mass: 10.2,
        moment_of_inertia: [400.0, 90.0, 400.0],
        cog: Some([0.0, 0.5, 1.2]),
        cog_height: 1.2,
        rot_pnt_long: -2.6 - 0.5,
        inv_min_turn_radius: 0.12,
        axles: vec![axle(3.2), axle(-2.6)],
        ..Default::default()
    }
}

/// A way: straight north for 40 m, then a right turn of radius `r`, climbing at `grade`.
fn bend(r: f64, grade: f64, s: f64) -> DVec3 {
    if s < 40.0 {
        DVec3::new(0.0, s, 10.0 + grade * s)
    } else {
        let a = (s - 40.0) / r;
        DVec3::new(r - r * a.cos(), 40.0 + r * a.sin(), 10.0 + grade * s)
    }
}

/// KA-1: the rotation point stays on the lane through a bend, within 1 cm (SC-006).
#[test]
fn ka1_the_rotation_point_stays_on_the_lane() {
    let def = bus();
    let mut body = AiClassic::new(&def, MotionKind::Road);
    let (speed, dt, r) = (5.0f32, 1.0 / 30.0, 15.0);
    let mut s = 10.0f64;
    body.place(&|d| bend(r, 0.0, s + d as f64), None, None, speed);
    let mut worst = 0.0f64;
    for _ in 0..(14.0 / dt) as usize {
        s += (speed * dt) as f64;
        let at = s;
        body.step(dt, speed, &|d| bend(r, 0.0, at + d as f64), None, None);
        let on_lane = bend(r, 0.0, at + def.rot_pnt_long as f64).truncate();
        let origin = body.position.truncate();
        let heading = body.heading.to_radians();
        let rot = origin + glam::DVec2::new(heading.sin(), heading.cos()) * def.rot_pnt_long as f64;
        worst = worst.max((rot - on_lane).length());
    }
    assert!(worst < 0.01, "the rotation point is {worst} m off the lane");
}

/// KA-1: in a steady bend the body's axis lags the path's tangent so that `sin ψ_r = L κ`,
/// its front swung out of the bend, and the wheels point at the turning centre.
#[test]
fn ka1_the_axis_lags_the_path_in_a_steady_bend() {
    let def = bus();
    let mut body = AiClassic::new(&def, MotionKind::Road);
    let (speed, dt, r) = (5.0f32, 1.0 / 30.0, 25.0);
    let mut s = 30.0f64;
    body.place(&|d| bend(r, 0.0, s + d as f64), None, None, speed);
    for _ in 0..(8.0 / dt) as usize {
        s += (speed * dt) as f64;
        let at = s;
        body.step(dt, speed, &|d| bend(r, 0.0, at + d as f64), None, None);
    }
    let at = s;
    let tangent = {
        let a = bend(r, 0.0, at + def.rot_pnt_long as f64).truncate();
        let b = bend(r, 0.0, at + def.rot_pnt_long as f64 + 1.0).truncate();
        (b - a).x.atan2((b - a).y).to_degrees()
    };
    let length = (3.2 - def.rot_pnt_long) as f64;
    let want = (length / r).asin().to_degrees();
    let got = (tangent - body.heading + 540.0).rem_euclid(360.0) - 180.0;
    assert!((got - want).abs() < 0.5, "ψ_r {got}°, want {want}°");
    // the front is outside the bend: farther from its centre than the lane
    let h = body.heading.to_radians();
    let front = body.position.truncate() + glam::DVec2::new(h.sin(), h.cos()) * 3.2;
    let centre = glam::DVec2::new(r, 40.0);
    assert!(
        (front - centre).length() > r,
        "the front is inside the bend"
    );
    let delta = (((3.2 - def.rot_pnt_long) as f64) / r).atan().to_degrees();
    assert!(
        (body.steer as f64 - delta).abs() < 0.5,
        "front wheels {}°",
        body.steer
    );
}

/// KA-4: braking hard or cornering leans nothing: the pose comes from the path, the spring
/// travel is 0 and the bank is 0.
#[test]
fn ka4_the_pose_comes_from_the_path() {
    let def = bus();
    let mut body = AiClassic::new(&def, MotionKind::Road);
    let (dt, r, grade) = (1.0 / 30.0, 12.0, 0.05);
    let mut s = 30.0f64;
    body.place(&|d| bend(r, grade, s + d as f64), None, None, 8.0);
    for i in 0..300 {
        let speed = if i < 150 { 8.0 } else { 0.5 };
        s += (speed * dt) as f64;
        let at = s;
        body.step(dt, speed, &|d| bend(r, grade, at + d as f64), None, None);
        assert_eq!(body.bank_deg, 0.0);
        assert!(body.suspension.iter().all(|a| *a == [0.0, 0.0]));
        // the pitch is the path's grade between the axles
        assert!(
            (body.pitch_deg - (grade as f32).atan().to_degrees()).abs() < 0.2,
            "pitch {}°",
            body.pitch_deg
        );
        // the height of the origin is the path's
        let want = bend(r, grade, at).z;
        assert!((body.position.z - want).abs() < 1e-6);
    }
}

/// KA-2: standing, the resistance only holds the vehicle against the drive; rolling, it acts
/// against the motion and never turns the vehicle round.
#[test]
fn ka2_the_speed_comes_from_the_drive_and_the_brakes() {
    let f = |drive: f32, resist: f32| DriveForces {
        drive,
        resist,
        mass_kg: 10_000.0,
    };
    // held: the drive does not overcome the brakes
    assert_eq!(integrate(0.0, f(5_000.0, 8_000.0), 0.85, 0.1), 0.0);
    // pulls away with what is left: 2000 N on 10 t for 0.1 s
    assert!((integrate(0.0, f(10_000.0, 8_000.0), 0.85, 0.1) - 0.02).abs() < 1e-6);
    // rolling, the resistance slows it
    assert!((integrate(10.0, f(0.0, 20_000.0), 0.85, 0.1) - 9.8).abs() < 1e-5);
    // and stops it, not backwards
    assert_eq!(integrate(0.1, f(0.0, 200_000.0), 0.85, 0.1), 0.0);
    // a frame's change is bounded by g·μ (`KA-3`)
    assert!((integrate(0.0, f(1.0e7, 0.0), 0.12, 1.0) - 9.81 * 0.12).abs() < 1e-5);
}

/// KA-2: the driver's pedals from the ratio of the speed to the target speed.
#[test]
fn ka2_the_driver_sets_throttle_and_brake_from_the_target() {
    let mut p = Pedals::default();
    // well below the target: the throttle rises by 1 per second
    p.update(5.0, 10.0, 0.25);
    assert_eq!((p.throttle, p.brake), (0.25, 0.0));
    for _ in 0..8 {
        p.update(5.0, 10.0, 0.25);
    }
    assert_eq!(p.throttle, 1.0);
    // near the target the throttle eases off
    p.update(9.8, 10.0, 0.25);
    assert!((p.throttle - 0.2).abs() < 1e-5, "{p:?}");
    // above it the brake comes on, off the throttle
    p.update(13.0, 10.0, 0.25);
    assert!((p.brake - 0.5).abs() < 1e-5 && p.throttle == 0.0, "{p:?}");
    // the brake's ordinary limit, then full at twice the target
    p.update(19.0, 10.0, 0.25);
    assert!((p.brake - 0.7).abs() < 1e-6);
    p.update(21.0, 10.0, 0.25);
    assert_eq!(p.brake, 1.0);
    // asked to stop
    p.update(0.0, 0.0, 0.25);
    assert_eq!((p.throttle, p.brake), (0.0, 1.0));
}

/// KA-9 and KA-10: the regimes, and the detail radius about the camera.
#[test]
fn ka9_the_target_speed_beyond_the_detail_radius() {
    assert!(within_detail_radius((4, 7), (4, 7)));
    assert!(within_detail_radius((5, 6), (4, 7)));
    assert!(!within_detail_radius((6, 7), (4, 7)));
    assert_eq!(Regime::of(true, false), Regime::Physical);
    assert_eq!(Regime::of(false, true), Regime::Physical);
    assert_eq!(Regime::of(false, false), Regime::Target);
    let mut ai = AiClassic::new(&bus(), MotionKind::Road);
    let no_drive = DriveForces {
        drive: 0.0,
        resist: 0.0,
        mass_kg: 10_000.0,
    };
    assert_eq!(
        ai.speed_step(Regime::Target, 3.0, 12.5, no_drive, 0.85, 0.02),
        12.5
    );
    assert_eq!(
        ai.speed_step(Regime::Physical, 3.0, 12.5, no_drive, 0.85, 0.02),
        3.0
    );
    assert!(ai.pedals().throttle > 0.0);
}

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

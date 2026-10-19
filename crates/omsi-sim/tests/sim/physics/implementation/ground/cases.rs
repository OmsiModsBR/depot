use crate::physics::cases::ramp;
use glam::{DVec3, Vec3};
use omsi_sim::testing::physics::domain::params::{CLIMB, WALL_HEIGHT};
use omsi_sim::testing::physics::domain::params::{ENVELOPE, LATTICE};
use omsi_sim::testing::physics::domain::GroundProbe;
use omsi_sim::testing::physics::implementation::ground::*;

/// On a uniform grade of any steepness the tyre reports the grade itself and stands where
/// the envelope touches the ramp, with no more probes than on the flat. The old comb read
/// every grade over 6.4 % as a run of kerbs: 0.043 for 6.5-9 %, 0.13 for 10-15 %, and 73
/// probes a wheel.
#[test]
fn tyre_reads_any_grade_as_a_grade() {
    let r = 0.47f32;
    let a = (r * ENVELOPE) as f64;
    for pct in [
        0.0, 2.0, 5.0, 6.3, 6.5, 8.0, 9.0, 10.0, 12.0, 15.0, 20.0, 25.0,
    ] {
        let s = pct / 100.0;
        let count = std::cell::Cell::new(0);
        let g = ramp(s);
        let probe = |x: f64, y: f64, top: f64| {
            count.set(count.get() + 1);
            g(x, y, top)
        };
        for k in 0..40 {
            let y = 100.0 + k as f64 * 0.037;
            let hub = DVec3::new(0.0, y, y * s + r as f64);
            let z_top = y * s + (CLIMB * r) as f64 + 0.05;
            count.set(0);
            let line = tyre_line(&probe, hub, Vec3::Y, r, z_top, hub.z + WALL_HEIGHT, 0.0);
            let (need, slope, x) = line.ground.expect("ground");
            // the envelope touches where its own slope is the grade
            let t = s * a / r as f64;
            let xs = a * t / (1.0 + t * t).sqrt();
            let want = s * (y + xs) + r as f64 * (1.0 - (xs / a).powi(2)).sqrt();
            assert!(
                (slope as f64 - s).abs() < 1e-4,
                "{pct} %: slope {slope} at {y}"
            );
            assert!(
                (need - want).abs() < 1e-5,
                "{pct} %: need {need}, want {want}"
            );
            assert!(
                (x as f64 - xs).abs() < 1e-3,
                "{pct} %: contact at {x}, want {xs}"
            );
            assert!(line.faces == [None, None] && line.clear);
            assert!(count.get() <= LATTICE, "{pct} %: {} probes", count.get());
        }
    }
}

/// Walked over a step or a chamfered face (on the level or on a grade), the push the
/// slope gives takes exactly what the hub is lifted by (∫ slope dx = Δneed), and the hub
/// never jumps as the samples move on: the old corner slope gave back only 0.43 of the
/// load for 0.6 of it on a steep face and pumped the body up.
#[test]
fn climbing_a_face_costs_what_it_lifts() {
    let r = 0.47f32;
    for (grade, height, angle) in [
        (0.0, 0.15, 90.0f64),
        (0.0, 0.2, 72.0),
        (0.0, 0.2, 60.0),
        (0.0, 0.1, 45.0),
        (0.0, 0.3, 80.0),
        (0.0, 0.05, 20.0),
        (0.0, 0.37, 90.0),
        (0.0, 0.37, 70.0),
        (0.1, 0.15, 90.0),
        (-0.08, 0.2, 75.0),
    ] {
        let run = height / angle.to_radians().tan();
        let face = move |_x: f64, y: f64, top: f64| {
            let z = grade * y + ((y - 5.0) / run.max(1e-9)).clamp(0.0, 1.0) * height;
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
        };
        let need_at = |y: f64| {
            let hub = DVec3::new(0.3, y, r as f64 + grade * y);
            let top = hub.z - r as f64 + (CLIMB * r) as f64;
            tyre_line(
                &face,
                hub,
                Vec3::Y,
                r,
                top.max(grade * 5.0 + height),
                hub.z + WALL_HEIGHT,
                0.0,
            )
            .ground
            .expect("ground")
        };
        let (y0, y1, dy) = (2.5, 7.0, 0.0005);
        let (mut work, mut y, mut need) = (0.0, y0, need_at(y0).0);
        while y < y1 {
            let slope = need_at(y + dy / 2.0).1 as f64;
            work += slope * dy;
            y += dy;
            let next = need_at(y).0;
            assert!(
                (next - need - slope * dy).abs() < 1e-3,
                "{height} m at {angle}° on {grade}: the hub jumped by {} at {y}",
                next - need - slope * dy
            );
            need = next;
        }
        let lift = need_at(y).0 - need_at(y0).0;
        let want = height + grade * (y - y0);
        assert!(
            (lift - want).abs() < 1e-3,
            "{height} m at {angle}° on {grade}: lifted {lift}, want {want}"
        );
        assert!(
            (work - lift).abs() < 0.003 * height.max(0.1),
            "{height} m at {angle}° on {grade}: pushed back {work} for {lift}"
        );
    }
}

use crate::vehicle::cases::coupling_test_type;
use glam::DVec3;
use omsi_sim::host::VehicleHost;
use omsi_sim::physics::Controls;
use omsi_sim::physics::GroundProbe;
use omsi_sim::testing::physics::RigidBodyInternals;
use omsi_sim::testing::vehicle::vehicle_type::VehicleType;
use omsi_sim::testing::vehicle::{TrailerPartInternals, VehicleInstance, VehicleInstanceInternals};
use std::sync::Arc;

/// The two sections of an articulated bus: a front section on two free-rolling axles and a
/// pusher's rear section on one driven axle and the joint.
fn sections() -> (Arc<VehicleType>, Arc<VehicleType>) {
    let section = |axles: &[(f32, bool)], mass: f32| {
        let mut ty = coupling_test_type(None);
        let def = &mut Arc::get_mut(&mut ty).unwrap().def;
        def.mass = mass;
        def.moment_of_inertia = [300.0, 80.0, 300.0];
        def.cog_height = 1.2;
        def.inv_min_turn_radius = 0.13;
        def.rot_pnt_long = axles.iter().map(|a| a.0).fold(f32::MAX, f32::min);
        def.axles = axles
            .iter()
            .map(|&(long, driven)| omsi_vehicle::Axle {
                long,
                max_width: 2.4,
                min_width: 1.2,
                wheel_diameter: 0.94,
                spring: 280.0,
                max_force: 116.0,
                damper: 20.0,
                driven,
                inertia_inv: 0.0,
            })
            .collect();
        def.coupling_front_character = Some([52.5, -20.0, 20.0, 1.0]);
        ty
    };
    (
        section(&[(3.2, false), (-2.6, false)], 8.0),
        section(&[(-0.4, true)], 7.2),
    )
}

fn flat() -> Arc<dyn omsi_sim::physics::Ground> {
    Arc::new(|_x: f64, _y: f64, _top: f64| GroundProbe {
        below: Some(0.0),
        above: None,
    })
}

/// How far apart the two sections hold the ball.
fn gap(v: &VehicleInstance) -> f64 {
    let t = &v.trailers[0];
    let (_, front) = t.couplings();
    (t.coupling_point(v.position, v.body_rotation())
        - (t.position + t.body_rotation().transform_point3(front).as_dvec3()))
    .length()
}

fn frames(v: &mut VehicleInstance, secs: f32, steer: &dyn Fn(f32) -> f32) -> f64 {
    let mut worst = 0.0f64;
    for k in 0..(secs * 60.0) as usize {
        v.set_controls(Controls {
            steering: steer(k as f32 / 60.0),
            ..Default::default()
        });
        v.step_physics(1.0 / 60.0);
        v.update_visuals(1.0 / 60.0);
        worst = worst.max(gap(v));
    }
    worst
}

/// Behind the player's bus the rear section is a body of its own: it stands on its axle
/// and the joint, follows the front section through a bend on its own tyres, stops at
/// the joint's limit, and is hung on the joint again when the bus is put somewhere else.
#[test]
fn the_rear_section_is_a_body_of_its_own() {
    let (front, rear) = sections();
    let mut v = VehicleInstance::new(front, VehicleHost::new(Default::default()));
    v.contact = Some(flat());
    v.attach_trailer(rear);
    v.enable_rigid_body();
    let settle = frames(&mut v, 1.0, &|_| 0.0);
    let rb = v.trailers[0].rigid().expect("the rear section has a body");
    assert!((rb.mass() - 7200.0).abs() < 1.0);
    assert!(settle < 0.02, "the joint opened {settle} m standing");
    v.set_speed(6.0);
    let bend = frames(&mut v, 6.0, &|_| 0.8);
    let alpha = ((v.heading - v.trailers[0].heading + 540.0) % 360.0) - 180.0;
    assert!(bend < 0.02, "the joint opened {bend} m in the bend");
    assert!(alpha > 3.0 && alpha < 52.6, "alpha {alpha}");
    // put down elsewhere, turned round: the rear section is hung on the joint again
    let origin = DVec3::new(200.0, 50.0, 0.0);
    v.position = origin;
    v.heading = 180.0;
    if let Some(rb) = v.rigid_mut() {
        rb.place(origin, 180.0);
    }
    v.trailers[0].realign();
    frames(&mut v, 1.0 / 60.0, &|_| 0.0);
    assert!(gap(&v) < 0.02, "{}", gap(&v));
    assert!(
        (v.trailers[0].heading - 180.0).abs() < 0.5,
        "{}",
        v.trailers[0].heading
    );
    assert!(
        v.trailers[0].position.y > 50.0,
        "{:?}",
        v.trailers[0].position
    );
}

/// A rear section turns about its own `[rot_pnt_long]`, 0 as well: its body does not move
/// the line to its axle as the kinematic part does.
#[test]
fn the_rear_section_turns_about_its_own_line() {
    let (front, mut rear) = sections();
    Arc::get_mut(&mut rear).unwrap().def.rot_pnt_long = 0.0;
    let mut v = VehicleInstance::new(front, VehicleHost::new(Default::default()));
    v.contact = Some(flat());
    v.attach_trailer(rear);
    v.enable_rigid_body();
    frames(&mut v, 1.0 / 60.0, &|_| 0.0);
    assert_eq!(v.trailers[0].axle_long(), -0.4);
    assert_eq!(
        v.trailers[0]
            .rigid()
            .expect("the rear section has a body")
            .rot_pnt_long(),
        0.0
    );
}

/// Uncoupled, the front section carries its own weight on its own wheels again, not the
/// share the joint took of the part that is gone.
#[test]
fn an_uncoupled_part_leaves_no_load_on_the_wheels() {
    let run = |v: &mut VehicleInstance| {
        for _ in 0..12 {
            v.step_physics(1.0 / 60.0);
            v.update_visuals(1.0 / 60.0);
        }
    };
    let (front, rear) = sections();
    let loads = |v: &VehicleInstance| {
        v.rigid()
            .expect("body")
            .wheels()
            .iter()
            .map(|w| w.rest_load())
            .collect::<Vec<f32>>()
    };
    let mut alone = VehicleInstance::new(front.clone(), VehicleHost::new(Default::default()));
    alone.contact = Some(flat());
    alone.enable_rigid_body();
    run(&mut alone);
    let own = loads(&alone);
    let mut v = VehicleInstance::new(front, VehicleHost::new(Default::default()));
    v.contact = Some(flat());
    v.attach_trailer(rear);
    v.enable_rigid_body();
    run(&mut v);
    assert!(
        loads(&v).iter().zip(&own).any(|(a, b)| a > b),
        "the joint should carry load: {:?} vs {own:?}",
        loads(&v)
    );
    assert!(v.detach_last_trailer().is_some());
    run(&mut v);
    assert_eq!(loads(&v), own);
}

/// The stock GN92 weaving at 30 km/h on its own physics: the rear section's body follows
/// with the joint closed and `articulation_0_alpha` within the joint's 52.5 degrees.
#[test]
#[ignore = "needs the original OMSI 2 install (OMSI_ROOT)"]
fn the_gn92_weaves_on_two_bodies() {
    let root = omsi_cfg::env::var_os("OMSI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
    let bus = root.join("Vehicles/MAN_NL_NG/MAN_GN92_main.bus");
    let trail = root.join("Vehicles/MAN_NL_NG/MAN_GN92_trail.bus");
    for p in [&bus, &trail] {
        assert!(
            p.exists(),
            "this test needs {} (set OMSI_ROOT)",
            p.display()
        );
    }
    let ty = Arc::new(VehicleType::load(&root, &bus).expect("GN92"));
    let mut v = VehicleInstance::new(ty, VehicleHost::new(omsi_sim::SimClock::default()));
    v.contact = Some(flat());
    v.attach_trailer_ex(
        Arc::new(VehicleType::load(&root, &trail).expect("GN92 trail")),
        false,
    );
    v.enable_rigid_body();
    assert!(frames(&mut v, 1.0, &|_| 0.0) < 0.02);
    v.set_speed(30.0 / 3.6);
    let worst = frames(&mut v, 10.0, &|t| (t * 0.8).sin() * 0.6);
    let alpha = v.var("articulation_0_alpha").unwrap();
    eprintln!(
        "GN92 weaving at 30 km/h: joint open at most {:.1} mm, alpha {alpha:.1}°, at {:?}",
        worst * 1000.0,
        v.position
    );
    assert!(worst < 0.02, "the joint opened {worst} m");
    assert!(alpha.abs() < 52.6, "alpha {alpha}");
    assert!(v.trailers[0]
        .rigid()
        .is_some_and(|rb| (rb.mass() - 7200.0).abs() < 1.0));
}

/// Both sections of an articulated bus lying on one sloping plane, bent in a curve: the joint
/// is bent about the vertical axis only, so beta stays 0 and alpha is the bend - on a grade
/// taken straight up or across, and on a crossfall. Measured as headings and pitches against
/// the horizon, the bend alone made 2° of beta on a 6% grade.
#[test]
fn joint_angles_on_one_sloping_plane_bend_only_alpha() {
    use glam::{Mat4, Quat};
    use omsi_sim::vehicle::joint_angles;
    let yaw = |h: f64| Quat::from_rotation_z((-h).to_radians() as f32);
    let grade = |g: f32| Quat::from_rotation_x(g.atan());
    let cross = |g: f32| Quat::from_rotation_y(g.atan());
    for plane in [grade(0.06), grade(0.10), cross(0.025), cross(0.04)] {
        for own in [0.0, 90.0, 200.0] {
            for bend in [-45.0, 20.0, 35.0] {
                let lead = own + bend;
                let (alpha, beta) = joint_angles(
                    Mat4::from_quat(plane * yaw(lead)),
                    lead,
                    Mat4::from_quat(plane * yaw(own)),
                    own,
                    false,
                );
                assert!(beta.abs() < 1e-3, "beta {beta} at {own}° bent {bend}°");
                assert!((alpha - bend).abs() < 1e-3, "alpha {alpha} for {bend}°");
            }
        }
    }
}

/// Unbent, beta is the part in front's pitch less this one's (it grows as the part in front
/// noses up against the one behind); bent on level ground, alpha is its heading less this
/// one's. A model turned round (travelling backwards along its own axis) reads the same.
#[test]
fn joint_angles_on_level_ground_and_turned_models() {
    use omsi_sim::vehicle::{body_rotation, joint_angles};
    for reversed in [false, true] {
        for (lead_pitch, own_pitch) in [(3.0f32, 0.0f32), (0.0, -2.0), (-1.5, 1.0)] {
            let (alpha, beta) = joint_angles(
                body_rotation(10.0, lead_pitch, 0.0, false),
                10.0,
                body_rotation(10.0, own_pitch, 0.0, reversed),
                10.0,
                false,
            );
            let want = (lead_pitch - own_pitch) as f64;
            assert!(alpha.abs() < 1e-3, "alpha {alpha}");
            assert!(
                (beta - want).abs() < 1e-3,
                "beta {beta} for {want} ({reversed})"
            );
        }
        let (alpha, beta) = joint_angles(
            body_rotation(40.0, 0.0, 0.0, false),
            40.0,
            body_rotation(10.0, 0.0, 0.0, reversed),
            10.0,
            false,
        );
        assert!((alpha - 30.0).abs() < 1e-3, "alpha {alpha} ({reversed})");
        assert!(beta.abs() < 1e-3, "beta {beta} ({reversed})");
    }
}

/// Bent both ways at once - round a bend over a dip or a crest, on level ground or on a slope -
/// a puller's rear section is the part in front turned by beta about its own transverse axis
/// and then by alpha about the vertical of the part in front (`physics::Joint`). Its alpha and
/// beta are those two turns, so that the bellows' last ring, turned by them in the same order
/// (beta, then alpha), lands on the rear section. Read the pusher's way, beta came out short by
/// the cosine of alpha (3.28° for 4° at 35°): the last ring stood off the rear section by
/// several centimetres.
#[test]
fn joint_angles_bent_both_ways_rebuild_the_rear_section() {
    use glam::{Mat4, Quat, Vec3};
    use omsi_sim::vehicle::joint_angles;
    let yaw = |h: f64| Quat::from_rotation_z((-h).to_radians() as f32);
    let pitch = |p: f64| Quat::from_rotation_x(p.to_radians() as f32);
    for plane in [
        Quat::IDENTITY,
        Quat::from_rotation_x(0.06f32.atan()),
        Quat::from_rotation_y(0.04f32.atan()),
    ] {
        for lead_h in [0.0, 120.0, 250.0] {
            for (bend, rise) in [(35.0, 4.0), (-45.0, -3.0), (50.0, -6.0), (-20.0, 8.0)] {
                let lead = plane * yaw(lead_h);
                // (alpha grows as the rear section turns left of the front, beta as it
                // pitches down against it)
                let own = lead * yaw(-bend) * pitch(-rise);
                let own_h = lead_h - bend;
                let (alpha, beta) = joint_angles(
                    Mat4::from_quat(lead),
                    lead_h,
                    Mat4::from_quat(own),
                    own_h,
                    false,
                );
                assert!((alpha - bend).abs() < 1e-3, "alpha {alpha} for {bend}°");
                assert!(
                    (beta - rise).abs() < 1e-3,
                    "beta {beta} for {rise}° at {bend}°"
                );
                // the last ring, beta and then alpha about the ball, on the rear section
                let ring = lead * yaw(-alpha) * pitch(-beta);
                for corner in [Vec3::new(1.25, 0.0, 3.0), Vec3::new(-1.25, 0.0, 0.0)] {
                    let off = (ring * corner - own * corner).length();
                    assert!(off < 1e-4, "ring {off} m off at {bend}°/{rise}°");
                }
            }
        }
    }
}

/// A pusher (`[coupling_pitch_offset]`) yaws about its own vertical and pitches about the
/// transverse axis of the part in front: its beta is still read off the front's forward axis
/// in the rear section's frame.
#[test]
fn joint_angles_of_a_pusher_bent_both_ways() {
    use glam::{Mat4, Quat};
    use omsi_sim::vehicle::joint_angles;
    let yaw = |h: f64| Quat::from_rotation_z((-h).to_radians() as f32);
    let pitch = |p: f64| Quat::from_rotation_x(p.to_radians() as f32);
    for (bend, rise) in [(35.0, 4.0), (-45.0, -3.0)] {
        let lead = yaw(30.0);
        let own = lead * pitch(-rise) * yaw(-bend);
        let (alpha, beta) = joint_angles(
            Mat4::from_quat(lead),
            30.0,
            Mat4::from_quat(own),
            30.0 - bend,
            true,
        );
        assert!((alpha - bend).abs() < 1e-3, "alpha {alpha} for {bend}°");
        // (asin of the rise over the run: 4.01 for 4°)
        assert!((beta - rise).abs() < 0.02, "beta {beta} for {rise}°");
    }
}

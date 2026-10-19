use glam::Vec3;
use omsi_content::Human;
use omsi_sim::testing::human_omsi::*;

fn rig() -> OmsiRig {
    let def = Human {
        height: 1.77,
        feet_dist: 0.04,
        seat_height: 0.83,
        links: vec![
            0.09, 0.0, 0.92, 0.09, -0.03, 0.53, 0.02, 1.17, 0.18, -0.05, 1.43, 0.44, -0.04, 1.41,
            -0.02, 1.55, 0.69, -0.03, 1.43, 0.9, -0.03, 1.43,
        ],
        walk_param: [1.4, 80.0, 1.0, 1.0, 0.0],
        ..Default::default()
    };
    OmsiRig::new(&def)
}

#[test]
fn curves_as_set_up_in_sub_624820() {
    assert_eq!(curve(&THIGH_CURVE, 0.0), 0.75);
    assert!((curve(&THIGH_CURVE, 0.1) - 0.925).abs() < 1e-6);
    assert!((curve(&THIGH_CURVE, 0.5) - 0.0).abs() < 1e-6);
    assert_eq!(curve(&KNEE_CURVE, 0.5), 0.0);
    assert_eq!(curve(&KNEE_CURVE, 1.0), 1.0);
}

#[test]
fn d3dx_rotations_and_order() {
    // D3DX: RotationY(90°) takes +x to -z (left-handed, row vectors)
    let p = DMat::rot_y(std::f32::consts::FRAC_PI_2).point(Vec3::X);
    assert!((p - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-6, "{p:?}");
    // a translation then a rotation: the rotation acts last
    let m = DMat::translation(Vec3::X).mul(&DMat::rot_z(std::f32::consts::FRAC_PI_2));
    let p = m.point(Vec3::ZERO);
    assert!((p - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-6, "{p:?}");
}

#[test]
fn the_hand_reaches_down_to_the_cash_desk() {
    // targets in front of the right shoulder (D3D: x right, y up, z forward), below it as
    // the money tray and the ticket slot are: the fingers end there, the elbow under the
    // shoulder - not the arm raised up and out
    let r = rig();
    for target in [
        Vec3::new(0.25, 1.0, 0.4),
        Vec3::new(0.1, 1.2, 0.45),
        Vec3::new(0.35, 1.1, 0.2),
    ] {
        let mut an = OmsiAnim::default();
        an.advance(
            &r,
            &AnimInput {
                kind: 0,
                room_height: 50.0,
                dt_ms: 16.0,
                reach: Some(target),
                ..Default::default()
            },
        );
        let b = an.bones_d3d(&r);
        let finger = b[7].point(r.finger);
        let elbow = b[5].point(r.elbow);
        assert!(
            (finger - target).length() < 0.05,
            "{target:?}: the fingers at {finger:?}"
        );
        assert!(
            elbow.y < r.shoulder.y,
            "{target:?}: the elbow at {elbow:?} above the shoulder"
        );
    }
}

#[test]
fn standing_pose_keeps_the_feet_on_the_floor() {
    let r = rig();
    let mut an = OmsiAnim::default();
    an.advance(
        &r,
        &AnimInput {
            kind: 0,
            room_height: 50.0,
            dt_ms: 16.0,
            ..Default::default()
        },
    );
    let b = an.bones(&r);
    // the right ankle region (a point just above the floor under the knee) stays at its
    // height: the leg only turns in by feetdist / (2 x hip height) about the hip
    let foot = Vec3::new(0.09, -0.03, 0.05);
    let p = b[3].transform_point3(foot);
    assert!(
        (p.z - foot.z).abs() < 0.01 && (p.x - foot.x).abs() < 0.03,
        "{p:?}"
    );
    // the head is where the model has it (no turn)
    let head = Vec3::new(0.0, -0.02, 1.65);
    assert!((b[10].transform_point3(head) - head).length() < 1e-4);
}

#[test]
fn walking_swings_the_legs_and_plays_steps() {
    let r = rig();
    let mut an = OmsiAnim::default();
    let mut steps = 0;
    let mut max_thigh: f32 = 0.0;
    for _ in 0..200 {
        let ev = an.advance(
            &r,
            &AnimInput {
                kind: 1,
                speed: 1.2,
                moved: 1.2 * 0.016,
                room_height: 50.0,
                dt_ms: 16.0,
                ..Default::default()
            },
        );
        steps += ev.step as u32;
        max_thigh = max_thigh.max(an.angles[0].abs());
    }
    // 200 frames at 1.2 m/s = 3.84 m = 2.74 strides of 1.4 m: a step every half stride
    assert!((4..=7).contains(&steps), "{steps}");
    // the thigh swings by 1.1 x (1.4 / (4 x 0.92)) rad = 24 degrees
    assert!((max_thigh - 24.0).abs() < 1.5, "{max_thigh}");
}

#[test]
fn sitting_bends_hips_and_knees() {
    let r = rig();
    let mut an = OmsiAnim::default();
    an.advance(
        &r,
        &AnimInput {
            kind: 2,
            room_height: 50.0,
            seat_height: 0.45,
            dt_ms: 16.0,
            ..Default::default()
        },
    );
    assert!(an.angles[0] >= 60.0 && an.angles[4] >= 90.0);
    assert_eq!(an.angles[9], -20.0);
}

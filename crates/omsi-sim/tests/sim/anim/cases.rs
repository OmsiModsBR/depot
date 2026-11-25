use glam::{Mat4, Vec3, Vec4};
use omsi_model::MeshDef;
use omsi_sim::testing::anim::*;

fn animator(parent: Option<usize>) -> MeshAnimator {
    MeshAnimator {
        anims: Vec::new(),
        parent,
    }
}

#[test]
fn parents_compose_down_the_chain() {
    // 2 -> 0 -> 1 (the parent of 0 is listed after it)
    let mut a = vec![animator(Some(1)), animator(None), animator(Some(0))];
    let t0 = Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0));
    let t1 = Mat4::from_rotation_z(0.5);
    let t2 = Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0));
    let mut xf = vec![t0, t1, t2];
    apply_parents(&a, &mut xf);
    assert!(xf[1].abs_diff_eq(t1, 1e-6));
    assert!(xf[0].abs_diff_eq(t1 * t0, 1e-6));
    assert!(xf[2].abs_diff_eq(t1 * t0 * t2, 1e-6));
    // a loop is cut instead of hanging
    a[1].parent = Some(2);
    let mut xf = vec![t0, t1, t2];
    apply_parents(&a, &mut xf);
    assert!(xf.iter().all(|m| m.is_finite()));
}

#[test]
fn parents_found_by_mesh_ident() {
    let desk = MeshDef {
        mesh_ident: Some("Zahltisch".into()),
        ..Default::default()
    };
    let key = MeshDef {
        anim_parent: Some("Zahltisch".into()),
        ..Default::default()
    };
    let lost = MeshDef {
        anim_parent: Some("nobody".into()),
        ..Default::default()
    };
    let mut a = vec![animator(None), animator(None), animator(None)];
    link_parents(&mut a, &[&desk, &key, &lost]);
    assert_eq!(
        a.iter().map(|a| a.parent).collect::<Vec<_>>(),
        vec![None, Some(0), None]
    );
    // two door variants with the same arm names: each leaf hangs on the arm before it
    let arm_a = MeshDef {
        mesh_ident: Some("arm".into()),
        ..Default::default()
    };
    let leaf_a = MeshDef {
        anim_parent: Some("arm".into()),
        ..Default::default()
    };
    let arm_b = MeshDef {
        mesh_ident: Some("arm".into()),
        ..Default::default()
    };
    let leaf_b = MeshDef {
        anim_parent: Some("arm".into()),
        ..Default::default()
    };
    let mut a = vec![
        animator(None),
        animator(None),
        animator(None),
        animator(None),
    ];
    link_parents(&mut a, &[&arm_a, &leaf_a, &arm_b, &leaf_b]);
    assert_eq!(
        a.iter().map(|a| a.parent).collect::<Vec<_>>(),
        vec![None, Some(0), None, Some(2)]
    );
}

#[test]
fn a_badly_named_parent_is_not_mended() {
    let desk = MeshDef {
        mesh_ident: Some("Zahltisch".into()),
        ..Default::default()
    };
    // another case
    let other_case = MeshDef {
        anim_parent: Some("zahltisch".into()),
        ..Default::default()
    };
    // a space the ident does not have
    let spaced = MeshDef {
        anim_parent: Some(" Zahltisch".into()),
        ..Default::default()
    };
    // the parent is only listed after the child
    let early = MeshDef {
        anim_parent: Some("Zahltisch".into()),
        ..Default::default()
    };
    let mut a = vec![
        animator(None),
        animator(None),
        animator(None),
        animator(None),
    ];
    link_parents(&mut a, &[&early, &desk, &other_case, &spaced]);
    assert!(a.iter().all(|a| a.parent.is_none()));
}

/// How far the top of a wheel of radius 0.5 hung on `pivot` moves along the vehicle (+ =
/// forwards) when `Wheel_Rotation_0_L` is 0.35 rad - the wheel's own `[newanim]` being
/// `origin_from_mesh` / `anim_rot Wheel_Rotation_0_L 57.2958`, as the MEP buses have it.
fn wheel_top_moves(pivot: Mat4) -> f32 {
    let def = MeshDef {
        animations: vec![omsi_model::Animation {
            origins: vec![omsi_model::AnimOrigin::FromMesh],
            kind: Some(omsi_model::AnimKind::Rot),
            variable: "Wheel_Rotation_0_L".into(),
            factor: 57.295_78,
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut a = MeshAnimator::new(&def, pivot, |_| Some(0));
    let m = a.update(0.0, &[0.35]);
    let centre = pivot.w_axis.truncate();
    (m.transform_point3(centre + Vec3::Z * 0.5) - (centre + Vec3::Z * 0.5)).y
}

#[test]
fn a_wheel_rolls_forwards_whichever_way_its_mesh_matrix_is_mirrored() {
    let at = Vec4::new(-1.15, 3.288, 0.51, 1.0);
    // a plain pivot (the stock cars)
    let plain = Mat4::from_cols(Vec4::X, Vec4::Y, Vec4::Z, at);
    // the Millennium III's and the Apache VIP IV's right wheels: y and z swapped (det -1)
    let swapped = Mat4::from_cols(Vec4::X, Vec4::Z, Vec4::Y, at);
    // the Apache VIP IV's left wheels: x turned outwards with y and z turned (det -1)
    let outward = Mat4::from_cols(-Vec4::X, -Vec4::Z, Vec4::Y, at);
    for (name, pivot) in [("plain", plain), ("swapped", swapped), ("outward", outward)] {
        let moved = wheel_top_moves(pivot);
        assert!(
            moved > 0.1,
            "{name}: the top of the wheel moved {moved} m along the bus"
        );
    }
}

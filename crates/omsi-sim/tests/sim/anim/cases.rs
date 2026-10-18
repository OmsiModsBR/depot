use glam::{Mat4, Vec3};
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
        mesh_ident: Some("zahltisch".into()),
        ..Default::default()
    };
    let key = MeshDef {
        anim_parent: Some("Zahltisch ".into()),
        ..Default::default()
    };
    let lost = MeshDef {
        anim_parent: Some("nobody".into()),
        ..Default::default()
    };
    let mut a = vec![animator(None), animator(None), animator(None)];
    link_parents(&mut a, &[&key, &desk, &lost]);
    assert_eq!(
        a.iter().map(|a| a.parent).collect::<Vec<_>>(),
        vec![Some(1), None, None]
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

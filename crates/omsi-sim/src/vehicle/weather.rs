//! What the weather does to a vehicle: the air's humidity and the road's grip.


pub use omsi_content::weather::psychro::omsi::relative_humidity;

/// The tyres' friction coefficient on the road under them: `street_cond` as the weather sets
/// it (0 dry … 1 wet, 1 … 2 snow from packed to fresh) and the air temperature (°C), below
/// which a wet road freezes (the stock "Ueberfrierende Naesse" weather). Dry asphalt 0.85,
/// wet 0.6, snow 0.3, black ice 0.12 - textbook values for truck tyres; OMSI keeps its
/// friction inside ode.dll and only hands `StreetCond` to the scripts, so these are not
/// read off it.
pub fn road_grip(street_cond: f32, temperature: f32) -> f32 {
    const DRY: f32 = 0.85;
    const WET: f32 = 0.6;
    const SNOW: f32 = 0.3;
    const ICE: f32 = 0.12;
    let c = street_cond.clamp(0.0, 2.0);
    if c <= 1.0 {
        let wet = DRY + (WET - DRY) * c;
        // a wet road at or below freezing turns to ice as the film freezes; a damp one is
        // black ice all over (the stock "Ueberfrierende Naesse": -6 degC, groundwet 0.31)
        if temperature <= 0.0 && c > 0.05 {
            let frozen = ((0.5 - temperature) / 3.0).clamp(0.0, 1.0) * (c / 0.25).min(1.0);
            wet + (ICE - wet) * frozen
        } else {
            wet
        }
    } else {
        WET + (SNOW - WET) * (c - 1.0)
    }
}

#[cfg(test)]
mod grip_tests {
    use super::road_grip;

    #[test]
    fn wet_snowy_and_frozen_roads_hold_less() {
        assert_eq!(road_grip(0.0, 15.0), 0.85);
        assert!((road_grip(1.0, 15.0) - 0.6).abs() < 1e-6);
        assert!((road_grip(2.0, -5.0) - 0.3).abs() < 1e-6);
        assert!(road_grip(1.0, -3.0) < 0.2, "black ice");
        assert_eq!(road_grip(0.0, -10.0), 0.85, "a dry road does not freeze");
    }

    /// A `[smoothskin]` vertex hung partly on an o3d bone no `[setbone]` names keeps that
    /// share where it was modelled: the AA-FR Agora's retarder lever (`retarderhebel_2.o3d`)
    /// splits its vertices between `Bone` (unbound) and `Bone.001` (the animated dummy), and
    /// with the unbound share dropped the lever bent out of shape as it moved.
    #[test]
    fn unbound_bones_keep_their_share_of_a_vertex() {
        use super::super::*;
        use std::sync::Arc;
        let root = omsi_cfg::env::var_os("OMSI_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
        let bus = root.join("Vehicles/AA-FR_BusBundle/2002_Agora_S_2d.bus");
        if !bus.exists() {
            eprintln!("skipped: no {}", bus.display());
            return;
        }
        let ty = Arc::new(VehicleType::load(&root, &bus).expect("Agora S"));
        let i = (0..ty.meshes.len())
            .find(|&i| ty.model.meshes[ty.meshes[i].def_index].file.to_ascii_lowercase().contains("retarderhebel_2"))
            .expect("the Agora's retarder lever");
        let vm = &ty.meshes[i];
        let bound = vm.skin.iter().find(|b| b.def_index.is_some()).expect("Bone.001");
        assert!(vm.skin.iter().any(|b| b.def_index.is_none()), "the unbound Bone is kept");
        let k = ty.meshes.iter().position(|m| Some(m.def_index) == bound.def_index).unwrap();
        let mut v = VehicleInstance::new(ty.clone(), VehicleHost::new(crate::SimClock::default()));
        v.update_visuals(0.02);
        let rest = v.mesh_transforms.clone();
        assert!(v.set_var("cp_retarder_hebel", 4.0));
        for _ in 0..200 {
            v.update_visuals(0.05);
        }
        let bone = v.mesh_transforms[k] * rest[k].inverse();
        assert!(bone.abs_diff_eq(Mat4::IDENTITY, 1e-3) == false, "the lever's dummy did not move");
        let (pos, _) = v.skinned(i).expect("skinned lever");
        let mut checked = 0;
        for &(vi, w) in &bound.weights {
            let vi = vi as usize;
            if !(0.2..0.8).contains(&w) {
                continue;
            }
            let p = vm.data.positions[vi];
            // (1 - w) where it was modelled, w with the bone, in the mesh's own frame
            let want = rest[i].inverse() * (Mat4::IDENTITY * (1.0 - w) + bone * w);
            let want = want.transform_point3(p);
            assert!((pos[vi] - want).length() < 1e-4, "vertex {vi} (weight {w}): {:?} for {:?}", pos[vi], want);
            checked += 1;
        }
        assert!(checked > 0, "no vertex shared between the two bones");
    }

    /// The joint's vertical angle turns the Agora L's arch (`anim_rot articulation_0_beta
    /// 0.5`) and the bone its bellows' far ring hangs on half-way towards the rear section,
    /// whichever way the front section pitches.
    #[test]
    fn articulation_beta_tilts_the_joint_towards_the_rear_section() {
        use super::super::*;
        use std::sync::Arc;
        let root = omsi_cfg::env::var_os("OMSI_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
        let bus = root.join("Vehicles/AA-FR_BusBundle/2002_Agora_L_3d_main.bus");
        let trail = root.join("Vehicles/AA-FR_BusBundle/2002_Agora_L_3d_trail.bus");
        if !bus.exists() || !trail.exists() {
            eprintln!("skipped: no {}", bus.display());
            return;
        }
        let ty = Arc::new(VehicleType::load(&root, &bus).expect("Agora L"));
        let mut v = VehicleInstance::new(ty, VehicleHost::new(crate::SimClock::default()));
        v.attach_trailer_ex(Arc::new(VehicleType::load(&root, &trail).expect("Agora L trail")), false);
        v.update_visuals(0.02);
        let find = |v: &VehicleInstance, n: &str| {
            v.ty.meshes
                .iter()
                .position(|m| v.ty.model.meshes[m.def_index].file.to_ascii_lowercase().ends_with(n))
                .expect(n)
        };
        let (arch, bone_b) = (find(&v, "gelenk_arch.o3d"), find(&v, "bone_b.o3d"));
        for pitch in [4.0f32, -4.0] {
            v.pitch = pitch;
            for _ in 0..50 {
                v.update_visuals(0.05);
            }
            // where the rear section lies, in the front section's frame
            let rel = v.body_rotation().inverse() * v.trailers[0].body_rotation();
            let back = rel.transform_vector3(-Vec3::Y);
            assert!(back.z.abs() > 0.03, "the sections are not pitched apart ({back:?})");
            for k in [arch, bone_b] {
                let d = v.mesh_transforms[k].transform_vector3(-Vec3::Y);
                assert!(
                    d.z * back.z > 0.0 && d.z.abs() < back.z.abs(),
                    "pitch {pitch}: {} points {d:?}, the rear section {back:?}",
                    v.ty.model.meshes[v.ty.meshes[k].def_index].file
                );
            }
        }
    }
}

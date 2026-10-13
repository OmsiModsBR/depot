//! Skinned meshes: the bones the animations move and the vertices they carry.

use crate::anim::MeshAnimator;
use glam::{Mat4, Vec3};
use super::vehicle_type::VehicleType;
use super::VehicleInstance;

impl VehicleInstance {
    /// The `[smoothskin]` meshes (the bellows of an articulated bus).
    pub fn skinned_meshes(&self) -> Vec<usize> {
        (0..self.ty.meshes.len())
            .filter(|&i| !self.ty.meshes[i].skin.is_empty())
            .collect()
    }

    /// The transforms a skinned mesh's shape depends on (its own and its bones'): when they
    /// are unchanged, so is the shape.
    pub fn skin_key(&self, i: usize) -> Vec<Mat4> {
        skin_key(&self.ty, i, &self.mesh_transforms)
    }

    /// Skinned mesh `i` as its bones hold it now (see [`skin_vertices`]).
    pub fn skinned(&mut self, i: usize) -> Option<(Vec<Vec3>, Vec<Vec3>)> {
        if self.skin_rest.len() != self.animators.len() {
            self.skin_rest = rest_transforms(&self.animators, self.state.vars.len());
        }
        skin_vertices(&self.ty, i, &self.mesh_transforms, &self.skin_rest)
    }
}

pub(super) fn skin_key(ty: &VehicleType, i: usize, transforms: &[Mat4]) -> Vec<Mat4> {
    let Some(vm) = ty.meshes.get(i) else {
        return Vec::new();
    };
    let mut key = vec![transforms.get(i).copied().unwrap_or(Mat4::IDENTITY)];
    for b in &vm.skin {
        if let Some(k) = ty.meshes.iter().position(|m| Some(m.def_index) == b.def_index) {
            key.push(transforms.get(k).copied().unwrap_or(Mat4::IDENTITY));
        }
    }
    key
}

/// The transforms of every mesh with all variables at 0: the pose the model was built in
/// (what a skinned mesh's vertices are given in).
pub(super) fn rest_transforms(animators: &[MeshAnimator], n_vars: usize) -> Vec<Mat4> {
    let zeros = vec![0.0; n_vars];
    let mut probes = animators.to_vec();
    let mut out: Vec<Mat4> = probes
        .iter_mut()
        .map(|a| {
            for (_, _, st) in a.anims.iter_mut() {
                st.initialized = false;
            }
            a.update(0.0, &zeros)
        })
        .collect();
    crate::anim::apply_parents(&probes, &mut out);
    out
}

/// The vertices (positions, normals) of `[smoothskin]` mesh `i` with its bones where
/// `transforms` has them, in the mesh's own frame (the renderer puts `transforms[i]` on
/// top); `rest` are the transforms of the modelled pose. None for a mesh without bones or
/// vertices.
///
/// As in OMSI, the bones move the vertices first and the mesh's own motion (its
/// animations and its `[animparent]`) comes on top: the Agora L's rear half of the bellows
/// (`gelenk_B`) hangs on the arch that turns by half the joint's angle and takes the same
/// bones as the front half (a quarter and a half of it), so that its far ring ends up at
/// the whole angle, with the rear section. Put in place of the mesh's motion, the bones
/// held that ring at half the angle and the bellows fanned out across the bend.
///
/// A vertex no bone holds stays with the mesh, and so does the share of a vertex that an
/// unbound bone holds: dropping that share and making up the rest to 1, a vertex hung half
/// on the armature's fixed root moved all the way with the other bone - the Agora's retarder
/// lever bent out of shape.
pub fn skin_vertices(
    ty: &VehicleType,
    i: usize,
    transforms: &[Mat4],
    rest: &[Mat4],
) -> Option<(Vec<Vec3>, Vec<Vec3>)> {
    let vm = ty.meshes.get(i)?;
    let n = vm.data.positions.len();
    if vm.skin.is_empty()
        || n == 0
        || transforms.len() < ty.meshes.len()
        || rest.len() < ty.meshes.len()
    {
        return None;
    }
    let mut sum = vec![Mat4::ZERO; n];
    let mut total = vec![0.0f32; n];
    for b in &vm.skin {
        let bone = match b.def_index {
            None => Mat4::IDENTITY,
            Some(d) => {
                let Some(k) = ty.meshes.iter().position(|m| m.def_index == d) else {
                    continue;
                };
                transforms[k] * rest[k].inverse()
            }
        };
        for &(v, w) in &b.weights {
            let v = v as usize;
            if v < n && w.is_finite() && w > 0.0 {
                sum[v] += bone * w;
                total[v] += w;
            }
        }
    }
    // (the renderer puts the mesh's transform on top: in the modelled pose, the vertices
    // are where the file has them)
    let own_rest_inv = rest[i].inverse();
    let mut pos = Vec::with_capacity(n);
    let mut nrm = Vec::with_capacity(n);
    for v in 0..n {
        let p = vm.data.positions[v];
        let q = vm.data.normals.get(v).copied().unwrap_or(Vec3::Z);
        if total[v] < 1e-4 {
            pos.push(p);
            nrm.push(q);
            continue;
        }
        let m = own_rest_inv * (sum[v] * (1.0 / total[v]));
        pos.push(m.transform_point3(p));
        nrm.push(m.transform_vector3(q).normalize_or_zero());
    }
    Some((pos, nrm))
}

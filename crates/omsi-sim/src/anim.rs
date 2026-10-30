//! `[newanim]` evaluation: turns script variables into per-mesh transforms.
//!
//! All maths happens in the model frame (x right, y forward, z up). `anim_rot` rotates about
//! the local **x** axis of the origin frame, `anim_trans` translates along it (doors use
//! `origin_rot_y -90` to turn that axis up, wheels spin about it directly, a blind slides along
//! it); `origin_*` commands move/rotate that frame, `origin_from_mesh` takes the mesh's stored
//! pivot matrix whose first row is that axis. Angles are used with the sign the file gives
//! them, and the blocks of one mesh are composed in file order (the first listed acts on the
//! mesh first), which with column vectors means the product is built from the right.

use glam::{Mat4, Quat, Vec3};
use omsi_model::{AnimKind, AnimOrigin, Animation, MeshDef};
use omsi_script::VarId;

/// Runtime state of one animation (smoothed value).
#[derive(Debug, Clone)]
pub struct AnimState {
    pub var: Option<VarId>,
    pub value: f32,
    pub initialized: bool,
}

/// Precomputed animation chain for one mesh.
#[derive(Debug, Clone)]
pub struct MeshAnimator {
    pub anims: Vec<(Animation, Mat4, AnimState)>,
    pub parent: Option<usize>,
}

/// Convert an o3d pivot matrix (D3D frame, column-vector form) to the model frame.
pub fn pivot_from_mesh(m: &omsi_o3d::Mesh) -> Mat4 {
    let s = Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);
    s * m.transform * s
}

pub fn origin_matrix(origins: &[AnimOrigin], pivot: Mat4) -> Mat4 {
    let mut m = Mat4::IDENTITY;
    for o in origins {
        let op = match o {
            AnimOrigin::Trans(t) => Mat4::from_translation(Vec3::from_array(*t)),
            AnimOrigin::RotX(a) => Mat4::from_rotation_x(-a.to_radians()),
            AnimOrigin::RotY(a) => Mat4::from_rotation_y(-a.to_radians()),
            AnimOrigin::RotZ(a) => Mat4::from_rotation_z(-a.to_radians()),
            AnimOrigin::FromMesh => pivot,
        };
        m *= op;
    }
    m
}

impl MeshAnimator {
    pub fn new(def: &MeshDef, pivot: Mat4, resolve: impl Fn(&str) -> Option<VarId>) -> Self {
        let anims = def
            .animations
            .iter()
            .map(|a| {
                let var = resolve(&a.variable);
                if var.is_none() && !a.variable.is_empty() {
                    log::warn!("animation variable {} not found", a.variable);
                }
                (
                    a.clone(),
                    origin_matrix(&a.origins, pivot),
                    AnimState {
                        var,
                        value: 0.0,
                        initialized: false,
                    },
                )
            })
            .collect();
        Self {
            anims,
            parent: None,
        }
    }

    pub fn has_animations(&self) -> bool {
        !self.anims.is_empty() || self.parent.is_some()
    }

    /// Advance smoothing and return the mesh-space transform.
    pub fn update(&mut self, dt: f32, vars: &[f32]) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        for (a, origin, st) in &mut self.anims {
            let target = st.var.map(|v| vars[v as usize]).unwrap_or(0.0) * a.factor + a.offset;
            if !st.initialized {
                st.value = target;
                st.initialized = true;
            } else {
                let mut v = target;
                if a.delay > 0.0 {
                    // `delay` is a rate, not a time: the value closes the gap to its target
                    // at `delay` per second (the SD200's handbrake lever says `delay 10`,
                    // its doors 3 to 4). Read as a time constant it made the handbrake
                    // crawl for ten seconds.
                    let k = (dt * a.delay).min(1.0);
                    v = st.value + (target - st.value) * k;
                }
                if a.max_speed > 0.0 {
                    let max_step = a.max_speed * dt;
                    v = st.value + (v - st.value).clamp(-max_step, max_step);
                }
                st.value = v;
            }
            let local = match a.kind {
                Some(AnimKind::Rot) => {
                    Mat4::from_quat(Quat::from_rotation_x(-st.value.to_radians()))
                }
                Some(AnimKind::Trans) => Mat4::from_translation(Vec3::new(st.value, 0.0, 0.0)),
                None => Mat4::IDENTITY,
            };
            // The original multiplies its matrices the Direct3D way round (a row vector
            // times the matrix), so a mesh with several [newanim] blocks is transformed by
            // the first one first and the later ones act on the result; written with column
            // vectors that is the reverse product. The SD200's wiper blade proves it: its
            // four blocks are its own two rotations about the arm's tip followed by the
            // arm's own two about the arm's base, and composed the other way round the
            // blade leaves the arm 1.2 m behind. Angles keep the same sign as the origin
            // rotations they are measured in (both are read in the original's left-handed
            // frame, so both change sign here) - with only one of them flipped the wiper
            // sweeps down into the bonnet instead of across the glass.
            m = *origin * local * origin.inverse() * m;
        }
        m
    }
}

/// `[animparent]`: a mesh hangs on the mesh whose `[mesh_ident]` it names and moves with
/// it (the NL202's changer, ticket printer and IBIS keys ride on the cash desk, which
/// swings with the driver's door). Sets the parent of every mesh, `defs` being the
/// meshes in the order the animators have them. The parent is looked for among the
/// meshes before this one, the last of them that carries the name exactly as written
/// (capitals count): door variants that reuse their arms' names hang each variant's
/// leaves on its own arm (#348). The game does not mend a model: a name that matches
/// only in another case, or only a mesh listed after this one, leaves the mesh on its
/// own and the log says so (once per mesh and name, not for every copy of the vehicle),
/// so the mistake shows and can be fixed in the model.
pub fn link_parents(animators: &mut [MeshAnimator], defs: &[&MeshDef]) {
    // `OMSI_NO_ANIMPARENT=1`: every mesh on its own, for an A/B
    if omsi_cfg::flags::NO_ANIMPARENT.is_set() {
        return;
    }
    for (i, a) in animators.iter_mut().enumerate() {
        let Some(name) = defs.get(i).and_then(|d| d.anim_parent.as_deref()) else {
            a.parent = None;
            continue;
        };
        a.parent = defs[..i.min(defs.len())]
            .iter()
            .rposition(|d| d.mesh_ident.as_deref() == Some(name));
        if a.parent.is_none() {
            static WARNED: std::sync::Mutex<Vec<(String, String)>> =
                std::sync::Mutex::new(Vec::new());
            let key = (defs[i].file.clone(), name.to_string());
            let mut warned = WARNED.lock().unwrap_or_else(|e| e.into_inner());
            if !warned.contains(&key) {
                log::warn!(
                    "[animparent] {name:?} of mesh {}: no mesh before it has this [mesh_ident]",
                    key.0
                );
                warned.push(key);
            }
        }
    }
}

/// Put the parents' movement onto their children: a child's own animation happens in the
/// model frame first, then everything its parent chain does (the original multiplies the
/// child's matrix by the parent's, row-vector style). Parents may be listed after their
/// children and chains may be deep; a loop is cut where it closes.
pub fn apply_parents(animators: &[MeshAnimator], transforms: &mut [Mat4]) {
    if animators.iter().all(|a| a.parent.is_none()) {
        return;
    }
    let n = transforms.len().min(animators.len());
    let local: Vec<Mat4> = transforms[..n].to_vec();
    let mut done = vec![false; n];
    let mut chain: Vec<usize> = Vec::new();
    for start in 0..n {
        // walk up to the first mesh that is finished (or has no parent) ...
        chain.clear();
        let mut k = start;
        while !done[k] {
            if chain.contains(&k) {
                break;
            }
            chain.push(k);
            match animators[k].parent.filter(|p| *p < n) {
                Some(p) => k = p,
                None => break,
            }
        }
        // ... and come back down, each mesh taking its parent's final transform
        for &c in chain.iter().rev() {
            if done[c] {
                continue;
            }
            transforms[c] = match animators[c].parent.filter(|p| *p < n && done[*p]) {
                Some(p) => transforms[p] * local[c],
                None => local[c],
            };
            done[c] = true;
        }
    }
}

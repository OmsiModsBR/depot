//! Loading a human type from its `.hum`: the meshes, the textures, the leg joints fitted to
//! the mesh and the feet split off the shins.

use anyhow::{Context, Result};
use glam::Vec3;
use omsi_content::Human;
use omsi_geometry::util::smoothstep;
use omsi_geometry::{mesh_from_o3d, MeshData};
use omsi_model::Model;
use std::path::{Path, PathBuf};

use crate::people::rig::{
    bone_id_by_name, slot_of, HumanMesh, Influence, Joints, Rig, FOOT, MAIN, SHIN, THIGH, TOE,
};

pub struct HumanType {
    pub def: Human,
    /// The skeleton as OMSI animates it (see [`crate::human_omsi`]).
    pub omsi: crate::people::omsi_anim::OmsiRig,
    pub model: Model,
    pub model_dir: PathBuf,
    pub meshes: Vec<HumanMesh>,
    pub joints: Joints,
    pub rig: Rig,
    /// Clothing variants: the `[item]`s of the `.cti` files in the model's `[CTC]` folder
    /// (the stock people have two or three each), which replace the `[CTCTexture]`
    /// default. Variant 0 is the default texture, variant n the scheme n-1.
    pub variants: Vec<crate::vehicle::PaintScheme>,
    /// Folders the textures may lie in besides the usual ones: the `[CTC]` folders and
    /// the sub-folders of the human's own folder (see [`HumanType::texture_dirs`]).
    extra_dirs: Vec<PathBuf>,
}

impl HumanType {
    pub fn load(path: &Path) -> Result<HumanType> {
        let def = Human::load(path).with_context(|| format!("loading {}", path.display()))?;
        let dir = path.parent().unwrap_or(Path::new(""));
        let model_path = omsi_cfg::resolve_path(dir, &def.model);
        let model = Model::load(&model_path)
            .with_context(|| format!("loading {}", model_path.display()))?;
        let model_dir = model_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let mut meshes = Vec::new();
        if !model.lods.is_empty() {
            for md in model.lod_meshes(0) {
                let p = omsi_cfg::resolve_path(&model_dir, &md.file);
                let m =
                    omsi_o3d::load_mesh(&p).with_context(|| format!("loading {}", p.display()))?;
                let bones: Vec<(i32, Vec<(u32, f32)>)> = m
                    .bones
                    .iter()
                    .map(|b| {
                        let id = md
                            .bones
                            .iter()
                            .find(|(n, _)| n.eq_ignore_ascii_case(&b.name))
                            .map(|(_, id)| *id)
                            .or_else(|| bone_id_by_name(&b.name))
                            .unwrap_or(0);
                        (id, b.weights.iter().map(|w| (w.vertex, w.weight)).collect())
                    })
                    .collect();
                let data = mesh_from_o3d(&m);
                let skin = influences(&data, &bones);
                // `[matl] <texture> <n>` names the n-th material with that texture
                let mut seen: Vec<String> = Vec::new();
                let alpha = m
                    .materials
                    .iter()
                    .map(|mat| {
                        let name = mat.texture.trim().to_ascii_lowercase();
                        let n = seen.iter().filter(|t| **t == name).count() as i32;
                        seen.push(name);
                        md.materials
                            .iter()
                            .find(|d| {
                                d.texture.trim().eq_ignore_ascii_case(mat.texture.trim())
                                    && d.index == n
                            })
                            .map(|d| d.alpha)
                            .unwrap_or(0)
                    })
                    .collect();
                meshes.push(HumanMesh {
                    data,
                    materials: m.materials.clone(),
                    bones,
                    skin,
                    alpha,
                });
            }
        }
        let mut joints = Joints::from_links(&def.links);
        fit_leg_joints(&mut joints, &meshes, path);
        let rig = Rig::measure(&def, &joints, &meshes);
        for m in meshes.iter_mut() {
            split_feet(m, &rig);
        }
        // The `[CTC]` folder is relative to the .hum file's folder (`Texture\man02` is
        // Humans/Other/texture/man02). An add-on that put that folder straight into its own
        // folder instead (GSPNS: Humans/GSPNS/man02, with the default texture in it too) is
        // found by the folder's last name; OMSI would show that person untextured.
        let mut variants = Vec::new();
        let mut extra_dirs: Vec<PathBuf> = Vec::new();
        for c in &model.ctc {
            let mut d = omsi_cfg::resolve_path(dir, &c.path);
            if !omsi_cfg::vfs::is_dir(&d) {
                let last = c
                    .path
                    .trim()
                    .trim_end_matches(['\\', '/'])
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or("");
                if !last.is_empty() {
                    let alt = omsi_cfg::resolve_path(dir, last);
                    if omsi_cfg::vfs::is_dir(&alt) {
                        d = alt;
                    }
                }
            }
            variants.extend(crate::vehicle::load_paint_schemes(&d));
            extra_dirs.push(d);
        }
        // the sub-folders of the human's folder and of its texture folder, last
        for base in [dir.to_path_buf(), omsi_cfg::resolve_path(dir, "texture")] {
            for (n, is_dir) in omsi_cfg::vfs::list_dir(&base).unwrap_or_default() {
                if is_dir && !n.to_string_lossy().eq_ignore_ascii_case("model") {
                    let d = base.join(n);
                    if !extra_dirs.contains(&d) {
                        extra_dirs.push(d);
                    }
                }
            }
        }
        Ok(HumanType {
            omsi: crate::people::omsi_anim::OmsiRig::new(&def),
            joints,
            rig,
            def,
            model,
            model_dir,
            meshes,
            variants,
            extra_dirs,
        })
    }

    pub fn texture_dirs(&self, root: &Path) -> Vec<PathBuf> {
        let human_dir = self.model_dir.parent().unwrap_or(&self.model_dir);
        let hum_dir = self.def.path.parent().unwrap_or(human_dir);
        let mut dirs = vec![
            omsi_cfg::resolve_path(&self.model_dir, "texture"),
            self.model_dir.clone(),
            omsi_cfg::resolve_path(human_dir, "texture"),
            human_dir.to_path_buf(),
            omsi_cfg::resolve_path(hum_dir, "texture"),
            hum_dir.to_path_buf(),
            omsi_cfg::resolve_path(&self.model_dir, "..\\texture"),
            omsi_cfg::resolve_path(root, "Texture"),
        ];
        dirs.dedup();
        dirs.extend(self.extra_dirs.iter().cloned());
        dirs
    }

    /// The texture a material shows in clothing variant `variant` (0 = the default):
    /// the file name and the folder to look in first.
    pub fn variant_texture<'a>(
        &'a self,
        texture: &'a str,
        variant: usize,
    ) -> (&'a str, Option<&'a Path>) {
        let Some(s) = variant.checked_sub(1).and_then(|k| self.variants.get(k)) else {
            return (texture, None);
        };
        for (ctc_name, file) in &s.textures {
            for (name, default) in &self.model.ctc_textures {
                if name.eq_ignore_ascii_case(ctc_name)
                    && default.trim().eq_ignore_ascii_case(texture.trim())
                {
                    return (file.as_str(), Some(s.dir.as_path()));
                }
            }
        }
        (texture, None)
    }
}

/// A leg joint of `[links]` that lies outside the leg it belongs to is the author's slip:
/// the GSPNS man04 and man041 give the right hip at x = 0.6 m and the knee at 0.83 m, where
/// the mesh's thigh is at 0.08. OMSI's rotations about those points hardly show it (the
/// legs swing about the x axis, along which the error lies), but a leg rig measured from
/// them reaches its foot 0.8 m out to the side and crosses the legs. Such a coordinate is
/// taken from the mesh instead: the middle of the limb's vertices at the joint's height.
fn fit_leg_joints(j: &mut Joints, meshes: &[HumanMesh], path: &Path) {
    let verts = |slots: &[usize]| -> Vec<Vec3> {
        let mut out = Vec::new();
        for m in meshes {
            for (i, inf) in m.skin.iter().enumerate() {
                if (0..inf.n as usize)
                    .any(|k| slots.contains(&(inf.slot[k] as usize)) && inf.weight[k] > 0.5)
                {
                    out.push(m.data.positions[i]);
                }
            }
        }
        out
    };
    let thigh = verts(&[THIGH[1]]);
    let knee_region = verts(&[THIGH[1], SHIN[1]]);
    let fit = |joint: &mut Vec3, limb: &[Vec3], what: &str| {
        if limb.len() < 8 {
            return;
        }
        let near: Vec<&Vec3> = limb
            .iter()
            .filter(|p| (p.z - joint.z).abs() < 0.06)
            .collect();
        let near: Vec<&Vec3> = if near.len() >= 4 {
            near
        } else {
            limb.iter().collect()
        };
        let n = near.len() as f32;
        let (lo, hi) = limb
            .iter()
            .fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
        let mut fixed = false;
        if joint.x < lo.x - 0.05 || joint.x > hi.x + 0.05 {
            joint.x = near.iter().map(|p| p.x).sum::<f32>() / n;
            fixed = true;
        }
        if joint.y < lo.y - 0.05 || joint.y > hi.y + 0.05 {
            joint.y = near.iter().map(|p| p.y).sum::<f32>() / n;
            fixed = true;
        }
        if fixed {
            log::info!(
                "{}: the {what} of [links] lies outside the leg; using ({:.2}, {:.2}, {:.2}) from the mesh",
                path.display(),
                joint.x,
                joint.y,
                joint.z
            );
        }
    };
    fit(&mut j.hip, &thigh, "hip");
    fit(&mut j.knee, &knee_region, "knee");
}

/// Per-vertex influences from the file's bone lists: each bone counted once per vertex,
/// the strongest four kept, normalised. A vertex no bone claims follows the torso.
fn influences(data: &MeshData, bones: &[(i32, Vec<(u32, f32)>)]) -> Vec<Influence> {
    let n = data.positions.len();
    let mut per: Vec<Vec<(u8, f32)>> = vec![Vec::new(); n];
    for (id, weights) in bones {
        let Some(slot) = slot_of(*id) else { continue };
        for (v, w) in weights {
            let Some(list) = per.get_mut(*v as usize) else {
                continue;
            };
            if !w.is_finite() || *w <= 0.0 {
                continue;
            }
            match list.iter_mut().find(|e| e.0 as usize == slot) {
                Some(e) => e.1 = e.1.max(*w),
                None => list.push((slot as u8, *w)),
            }
        }
    }
    per.into_iter()
        .map(|mut list| {
            list.sort_by(|a, b| b.1.total_cmp(&a.1));
            list.truncate(4);
            let total: f32 = list.iter().map(|e| e.1).sum();
            if total < 1e-4 {
                return Influence {
                    n: 1,
                    slot: [MAIN as u8, 0, 0, 0],
                    weight: [1.0, 0.0, 0.0, 0.0],
                };
            }
            let mut inf = Influence {
                n: list.len() as u8,
                ..Default::default()
            };
            for (k, e) in list.iter().enumerate() {
                inf.slot[k] = e.0;
                inf.weight[k] = e.1 / total;
            }
            inf
        })
        .collect()
}

/// Move `share` of a vertex's weight on slot `from` to slot `to`.
fn move_weight(inf: &mut Influence, from: usize, to: usize, share: f32) {
    let Some(k) = (0..inf.n as usize).find(|&k| inf.slot[k] as usize == from) else {
        return;
    };
    let moved = inf.weight[k] * share;
    if moved < 1e-3 {
        return;
    }
    if let Some(e) = (0..inf.n as usize).find(|&e| inf.slot[e] as usize == to) {
        inf.weight[k] -= moved;
        inf.weight[e] += moved;
    } else if (inf.n as usize) < 4 {
        inf.weight[k] -= moved;
        let e = inf.n as usize;
        inf.slot[e] = to as u8;
        inf.weight[e] = moved;
        inf.n += 1;
    } else if share > 0.5 {
        // no room: all of it
        inf.slot[k] = to as u8;
    }
}

/// Give the feet and the toes bones of their own: shin weight below the ankle moves to the
/// foot and foot weight beyond the ball to the toes, with short blends around the joints.
/// A skirt is weighted to the thighs in the files and would fly up with a swinging leg like
/// a lap: its cloth (thigh vertices well outside the leg) follows the hips in part.
fn split_feet(m: &mut HumanMesh, rig: &Rig) {
    let top = rig.sole + rig.ankle_h + 0.025;
    let bottom = rig.sole + rig.ankle_h - 0.03;
    for (i, inf) in m.skin.iter_mut().enumerate() {
        let v = m.data.positions[i];
        if v.z >= top {
            continue;
        }
        let to_foot = smoothstep(top, bottom, v.z);
        for side in 0..2 {
            move_weight(inf, SHIN[side], FOOT[side], to_foot);
            let along = v.y - rig.ankle[side].y;
            let to_toe = smoothstep(rig.ball - 0.012, rig.ball + 0.02, along);
            if to_toe > 0.0 {
                move_weight(inf, FOOT[side], TOE[side], to_toe);
            }
        }
    }
}

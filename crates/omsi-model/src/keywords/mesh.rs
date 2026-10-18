//! `model.cfg` keywords: meshes and what hangs on them: items, variables, viewpoints, shadows,
//! mouse events, bones and animations.

use crate::{Animation, Lod, MeshDef, Model, ModelItem};
use omsi_cfg::CfgReader;

impl Model {
    pub(super) fn mesh_keyword(&mut self, k: &str, r: &mut CfgReader) -> bool {
        match k {
            "mesh" => {
                if self.lods.is_empty() {
                    self.lods.push(Lod {
                        min_size: 0.0,
                        first_mesh: 0,
                    });
                    self.implicit_lod = true;
                }
                let file = r.str().to_string();
                // OMSI: a new mesh takes the interior lights of the
                // mesh before it, the first one lights 0 to 3 - a mesh without its own
                // [illumination_interior] is lit like the one written before it (the GN92's
                // rear saloon, its walls and seats, has no line of its own and stayed dark)
                let illumination_interior = self
                    .meshes
                    .last()
                    .map(|m| m.illumination_interior.clone())
                    .unwrap_or_else(|| vec![0, 1, 2, 3]);
                self.meshes.push(MeshDef {
                    file,
                    lod: self.lods.len() - 1,
                    illumination_interior,
                    ..Default::default()
                });
            }
            "item" => {
                let name = r.str().to_string();
                let ctc = r.str().to_string();
                let texture = r.str().to_string();
                self.items.push(ModelItem {
                    name,
                    ctc,
                    texture,
                    set_vars: Vec::new(),
                });
            }
            "setvar" => {
                let n = r.str().to_string();
                let v = r.f32();
                match self.items.last_mut() {
                    Some(i) => i.set_vars.push((n, v)),
                    None => self.set_vars.push((n, v)),
                }
            }
            "mesh_ident" => {
                let s = r.str().to_string();
                if let Some(m) = self.cur_mesh() {
                    m.mesh_ident = Some(s);
                }
            }
            "viewpoint" => {
                let v = r.i32();
                if let Some(m) = self.cur_mesh() {
                    m.viewpoint = v;
                }
            }
            "shadow" => {
                if let Some(m) = self.cur_mesh() {
                    m.shadow = true;
                }
            }
            "isshadow" => {
                if let Some(m) = self.cur_mesh() {
                    m.is_shadow = true;
                }
            }
            "mouseevent" => {
                let s = r.str().to_string();
                if let Some(m) = self.cur_mesh() {
                    m.mouse_event = Some(s);
                }
            }
            "setbone" => {
                let n = r.str().to_string();
                let i = r.i32();
                if let Some(m) = self.cur_mesh() {
                    m.bones.push((n, i));
                }
            }
            "smoothskin" => {
                if let Some(m) = self.cur_mesh() {
                    m.smooth_skin = true;
                }
            }
            "animparent" => {
                let s = r.str().to_string();
                if let Some(m) = self.cur_mesh() {
                    m.anim_parent = Some(s);
                }
            }
            "newanim" => {
                if let Some(m) = self.cur_mesh() {
                    m.animations.push(Animation::default());
                }
            }
            "visible" => {
                let v = r.str().to_string();
                let val = r.f32();
                if let Some(m) = self.cur_mesh() {
                    m.visible = Some((v, val));
                }
            }
            _ => return false,
        }
        true
    }
}

//! The `model.cfg` vocabulary, one family of keywords per module. `.sco` files use the
//! same vocabulary inline and delegate to [`Model::handle_keyword`].

mod lights;
mod material;
mod mesh;
mod particles;
mod textures;

use crate::{Lod, Model};
use omsi_cfg::CfgReader;

impl Model {
    /// Handle one model.cfg keyword. Returns `false` when the keyword is not part of the
    /// model vocabulary (so the caller can try its own).
    pub fn handle_keyword(&mut self, k: &str, r: &mut CfgReader) -> bool {
        self.model_keyword(k, r)
            || self.mesh_keyword(k, r)
            || self.texture_keyword(k, r)
            || self.light_keyword(k, r)
            || self.material_keyword(k, r)
            || self.particle_keyword(k, r)
    }

    /// The whole model's settings: render type, surface, levels of detail and distances.
    fn model_keyword(&mut self, k: &str, r: &mut CfgReader) -> bool {
        match k {
            "rendertype" => self.render_type = Some(r.word().to_ascii_lowercase()),
            "surface" => {
                let value = r.word();
                self.surface = Some(value != "0");
            }
            "lod" => {
                let min_size = r.f32();
                // Meshes written before the first [LOD] belong to that first level: OMSI gives
                // them the level index its loader holds then, which no [LOD] has set yet. The
                // WH UK AI cars (Westcountry, London) put their shadow there: as a level of its
                // own the shadow was all an AI car had (only the first level is loaded for a
                // vehicle), and the traffic drove about as shadows and lamps.
                if self.implicit_lod && self.lods.len() == 1 {
                    self.lods[0].min_size = min_size;
                    self.implicit_lod = false;
                } else {
                    self.implicit_lod = false;
                    self.lods.push(Lod {
                        min_size,
                        first_mesh: self.meshes.len(),
                    });
                }
            }
            "vfdmaxmin" => self.vfd_max_min = Some(r.f32s::<6>()),
            "detail_factor" => self.detail_factor = r.f32(),
            "tex_detail_factor" => self.tex_detail_factor = r.f32(),
            "nodistancecheck" => {
                if let Some(m) = self.cur_mesh() {
                    m.no_distance_check = true;
                } else {
                    self.no_distance_check = true;
                }
            }
            "terrainhole" => {
                let f = r.str().to_string();
                // This declares an object-wide cutter, often before the first [mesh].
                // Keep every command; multiple cutters may follow a single render mesh.
                self.terrain_holes.push(f.clone());
                if let Some(m) = self.cur_mesh() {
                    m.terrain_hole = Some(f);
                }
            }
            _ => return false,
        }
        true
    }
}

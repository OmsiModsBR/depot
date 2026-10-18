//! `model.cfg` keywords: `[matl]` and the material commands that follow it.

use crate::{MaterialDef, Model, TexAddress};
use omsi_cfg::CfgReader;

impl Model {
    pub(super) fn material_keyword(&mut self, k: &str, r: &mut CfgReader) -> bool {
        match k {
            "matl" => {
                let texture = r.str().to_string();
                let index = r.i32();
                if let Some(m) = self.cur_mesh() {
                    // [matl] selects a material and what follows changes it: a second [matl] of
                    // the same one goes on with it (TH_Wald's chain barrier gives its slot an
                    // envmap in one block and [matl_alpha] 1 in the next - the second block was
                    // lost and the chain stood on a white band)
                    let same = |d: &MaterialDef| {
                        !d.item
                            && d.change.is_none()
                            && d.index == index
                            && d.texture.eq_ignore_ascii_case(&texture)
                    };
                    match m.materials.iter().position(same) {
                        Some(k) => {
                            let d = m.materials.remove(k);
                            m.materials.push(d);
                        }
                        None => m.materials.push(MaterialDef {
                            texture,
                            index,
                            ..Default::default()
                        }),
                    }
                }
            }
            "matl_change" => {
                let texture = r.str().to_string();
                let index = r.i32();
                let var = r.str().to_string();
                if let Some(m) = self.cur_mesh() {
                    m.materials.push(MaterialDef {
                        texture: texture.clone(),
                        index,
                        change: Some((texture, index, var)),
                        ..Default::default()
                    });
                }
            }
            "matl_item" => {
                // the variant of the preceding [matl_change]: starts as a copy of the base
                // material, the following matl_* keywords change the variant only
                if let Some(mesh) = self.cur_mesh() {
                    if let Some(change) = mesh
                        .materials
                        .last()
                        .filter(|m| m.change.is_some())
                        .cloned()
                    {
                        // inherit the plain [matl] of the same slot (alpha, transmap, ...)
                        let base = mesh
                            .materials
                            .iter()
                            .rev()
                            .find(|m| {
                                !m.item
                                    && m.change.is_none()
                                    && m.texture.eq_ignore_ascii_case(&change.texture)
                                    && m.index == change.index
                            })
                            .cloned();
                        // ... and what the [matl_change] (or the item before) set on top: OMSI
                        // copies the whole material record into the new item,
                        // its free texture, scrolling and border with it. Copied from
                        // the plain [matl] alone, the SD77's lit roller blind (`lights_stand`)
                        // lost its destination text and scrolling with the lights on (#1419).
                        let mut item = change.clone();
                        if let Some(base) = base {
                            item.inherit(&base);
                        }
                        item.item = true;
                        mesh.materials.push(item);
                    }
                }
            }
            "matl_raindropmap" => {
                let t = r.str().to_string();
                if let Some(m) = self.cur_matl() {
                    m.raindropmap = Some(t);
                }
            }
            "matl_texadress_mirror" => {
                if let Some(m) = self.cur_matl() {
                    m.tex_address = TexAddress::Mirror;
                }
            }
            "matl_texadress_clamp" => {
                if let Some(m) = self.cur_matl() {
                    m.tex_address = TexAddress::Clamp;
                }
            }
            "matl_texadress_border" => {
                let c = r.f32s::<4>();
                if let Some(m) = self.cur_matl() {
                    m.tex_address = TexAddress::Border;
                    m.border_color = c;
                }
            }
            "matl_texadress_mirroronce" => {
                if let Some(m) = self.cur_matl() {
                    m.tex_address = TexAddress::MirrorOnce;
                }
            }
            "texcoordtransx" => {
                let v = r.str().to_string();
                if let Some(m) = self.cur_matl() {
                    m.texcoord_trans_x = Some(v);
                }
            }
            "texcoordtransy" => {
                let v = r.str().to_string();
                if let Some(m) = self.cur_matl() {
                    m.texcoord_trans_y = Some(v);
                }
            }
            "usescripttexture" => {
                let v = r.i32();
                if let Some(m) = self.cur_matl() {
                    m.use_script_texture = Some(v);
                }
            }
            "usehtmltexture" => {
                let v = r.i32();
                let index = usize::try_from(v)
                    .ok()
                    .and_then(|n| self.html_textures.get(n))
                    .map(|d| d.script_index as i32);
                if let (Some(m), Some(i)) = (self.cur_matl(), index) {
                    m.use_script_texture = Some(i);
                }
            }
            "usetexttexture" => {
                let v = r.i32();
                if let Some(m) = self.cur_matl() {
                    m.use_text_texture = Some(v);
                }
            }
            "alphascale" => {
                let v = r.str().to_string();
                if let Some(m) = self.cur_matl() {
                    m.alphascale = Some(v);
                }
            }
            "matl_freetex" => {
                let t = r.str().to_string();
                let v = r.str().to_string();
                if let Some(m) = self.cur_matl() {
                    m.freetex = Some((t, v));
                }
            }
            "matl_lightmap" => {
                let t = r.str().to_string();
                let v = r.str().to_string();
                if let Some(m) = self.cur_matl() {
                    m.lightmaps.push((t.clone(), v.clone()));
                    m.lightmap = Some((t, v));
                }
            }
            "matl_nightmap" => {
                let t = r.str().to_string();
                if let Some(m) = self.cur_matl() {
                    m.nightmap = Some(t);
                }
            }
            "matl_allcolor" => {
                let v = r.f32s::<14>();
                if let Some(m) = self.cur_matl() {
                    m.allcolor = Some(v);
                }
            }
            "matl_alpha" => {
                let v = r.i32();
                if let Some(m) = self.cur_matl() {
                    m.alpha = v;
                    m.alpha_set = true;
                }
            }
            "matl_nozwrite" => {
                if let Some(m) = self.cur_matl() {
                    m.no_z_write = true;
                }
            }
            "matl_nozcheck" => {
                if let Some(m) = self.cur_matl() {
                    m.no_z_check = true;
                }
            }
            "matl_zbias" => {
                let v = r.i32();
                if let Some(m) = self.cur_matl() {
                    m.z_bias = v;
                }
            }
            "matl_envmap" => {
                let t = r.str().to_string();
                let f = r.f32();
                if let Some(m) = self.cur_matl() {
                    m.envmap = Some((t, f));
                }
            }
            "matl_envmaprealtime" => {
                if let Some(m) = self.cur_matl() {
                    m.envmap_realtime = true;
                }
            }
            "matl_bumpmap" => {
                let t = r.str().to_string();
                let f = r.f32();
                if let Some(m) = self.cur_matl() {
                    m.bumpmap = Some((t, f));
                }
            }
            "matl_envmap_mask" => {
                let t = r.str().to_string();
                if let Some(m) = self.cur_matl() {
                    m.envmap_mask = Some(t);
                }
            }
            "matl_transmap" => {
                let t = r.str().to_string();
                if let Some(m) = self.cur_matl() {
                    m.transmap = Some(t);
                }
            }
            _ => return false,
        }
        true
    }
}

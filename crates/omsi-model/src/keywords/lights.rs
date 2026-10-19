//! `model.cfg` keywords: illumination, lights, spotlights and interior lights.

use crate::{InteriorLight, LightEnh, LightEnh2, Model, Spotlight, Spotlight2};
use omsi_cfg::CfgReader;

impl Model {
    pub(super) fn light_keyword(&mut self, k: &str, r: &mut CfgReader) -> bool {
        match k {
            "illumination" => {
                let v: Vec<i32> = (0..4).map(|_| r.i32()).collect();
                if let Some(m) = self.cur_mesh() {
                    m.illumination = v;
                }
            }
            // (OMSI's four lamps, and openOMSI takes as many more as the lines that follow
            // give)
            "illumination_interior" => {
                let v: Vec<i32> = r.i32_list(4);
                if let Some(m) = self.cur_mesh() {
                    m.illumination_interior = v;
                }
            }
            "light" => {
                let v: Vec<String> = (0..7).map(|_| r.str().to_string()).collect();
                self.lights.push(v);
            }
            "light_enh" => {
                let pos = r.f32s::<3>();
                let color = r.f32s::<3>();
                let size = r.f32();
                let variable = r.str().to_string();
                let values: Vec<f32> = (0..4).map(|_| r.f32()).collect();
                let mut texture = None;
                // optional texture line (not a keyword, not empty, not numeric)
                let save = r.pos();
                let t = r.str();
                if !t.trim().is_empty()
                    && omsi_cfg::keyword_of(t).is_none()
                    && t.trim().parse::<f64>().is_err()
                    && t.contains('.')
                {
                    texture = Some(t.to_string());
                } else {
                    r.seek(save);
                }
                if let Some(m) = self.cur_mesh() {
                    m.light_enh.push(LightEnh {
                        pos,
                        color,
                        size,
                        variable,
                        values,
                        texture,
                    });
                }
            }
            "light_enh_2" => {
                let pos = r.f32s::<3>();
                let dir = r.f32s::<3>();
                let up = r.f32s::<3>();
                let omni = r.bool();
                let rotating = r.i32();
                let color = r.f32s::<3>();
                let size = r.f32();
                let cone_inner = r.f32();
                let cone_outer = r.f32();
                let variable = r.str().to_string();
                let factor = r.f32();
                let z_offset = r.f32();
                let values: Vec<String> = (0..3).map(|_| r.str().to_string()).collect();
                // the effect bitmap follows on the next line - when it names a picture (an
                // empty line keeps the standard one)
                let mut bitmap = None;
                {
                    let mut ahead = r.clone();
                    let l = ahead.str().trim().to_string();
                    let lower = l.to_ascii_lowercase();
                    if [".bmp", ".tga", ".dds", ".png", ".jpg"]
                        .iter()
                        .any(|e| lower.ends_with(e))
                    {
                        bitmap = Some(l);
                        *r = ahead;
                    }
                }
                let cone = values
                    .get(1)
                    .map(|v| omsi_cfg::parse_f32(v) >= 0.5)
                    .unwrap_or(false);
                let time_const = values
                    .get(2)
                    .map(|v| omsi_cfg::parse_f32(v).max(0.0))
                    .unwrap_or(0.0);
                if let Some(m) = self.cur_mesh() {
                    m.light_enh_2.push(LightEnh2 {
                        pos,
                        dir,
                        up,
                        omni,
                        rotating,
                        color,
                        size,
                        cone_inner,
                        cone_outer,
                        variable,
                        factor,
                        z_offset,
                        values,
                        cone,
                        time_const,
                        bitmap,
                    });
                }
            }
            "spotlight" => self.spotlights.push(Spotlight {
                values: r.f32s::<12>(),
            }),
            "spotlight_2" => {
                let values = r.f32s::<12>();
                let variable = r.str().to_string();
                // (the flag may be left out: a keyword next is not it)
                let mut ahead = r.clone();
                let flag = ahead.line();
                let mirrored = if omsi_cfg::keyword_of(flag).is_some() {
                    true
                } else {
                    *r = ahead;
                    omsi_cfg::parse_f32(flag) < 0.5
                };
                self.spotlights_2.push(Spotlight2 {
                    values,
                    variable,
                    mirrored,
                });
            }
            "interiorlight" => {
                let variable = r.str().to_string();
                let range = r.f32();
                let color = r.f32s::<3>();
                let pos = r.f32s::<3>();
                self.interior_lights.push(InteriorLight {
                    variable,
                    range,
                    color,
                    pos,
                });
            }
            _ => return false,
        }
        true
    }
}

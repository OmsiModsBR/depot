//! `model.cfg` keywords: the textures a script, an HTML page or a text fills at run time, and
//! the texture changes (repaints).

use crate::{Ctc, HtmlTextureDef, Model, TextTexture};
use omsi_cfg::CfgReader;

impl Model {
    pub(super) fn texture_keyword(&mut self, k: &str, r: &mut CfgReader) -> bool {
        match k {
            "ctc" => {
                let variable = r.str().to_string();
                let path = r.str().to_string();
                let value = r.i32();
                self.ctc.push(Ctc {
                    variable,
                    path,
                    value,
                });
            }
            "ctctexture" => {
                let a = r.str().to_string();
                let b = r.str().to_string();
                self.ctc_textures.push((a, b));
            }
            "scripttexture" => {
                let w = omsi_cfg::limits::clamp_dynamic_dim(r.i32());
                let h = omsi_cfg::limits::clamp_dynamic_dim(r.i32());
                self.script_textures.push((w, h));
            }
            "htmltexture" => {
                let width = omsi_cfg::limits::clamp_dynamic_dim(r.i32());
                let height = omsi_cfg::limits::clamp_dynamic_dim(r.i32());
                let path = r.str().to_string();
                let script_index = self.script_textures.len();
                self.script_textures.push((width, height));
                self.html_textures.push(HtmlTextureDef {
                    script_index,
                    width,
                    height,
                    path,
                });
            }
            "texttexture" => {
                let variable = r.str().to_string();
                let font = r.str().to_string();
                let width = omsi_cfg::limits::clamp_dynamic_dim(r.i32());
                let height = omsi_cfg::limits::clamp_dynamic_dim(r.i32());
                let full_color = r.bool();
                let color = r.f32s::<3>();
                self.text_textures.push(TextTexture {
                    variable,
                    font,
                    width,
                    height,
                    full_color,
                    color,
                    orientation: 0,
                    grid: 1,
                });
            }
            "texttexture_enh" => {
                // The same first eight fields as [texttexture] and two more: orientation
                // and grid (the SDK's own list in the stock model files; all 190 stock and
                // installed uses follow it). It takes a slot in the same index space
                // [useTextTexture] counts through - the D86's number plate is texture 0 and
                // its matrix texts 3..5; without the slot every later index was off by one
                // and the destination display showed the temperature's 7-segment digits.
                let variable = r.str().to_string();
                let font = r.str().to_string();
                let width = omsi_cfg::limits::clamp_dynamic_dim(r.i32());
                let height = omsi_cfg::limits::clamp_dynamic_dim(r.i32());
                let full_color = r.bool();
                let color = r.f32s::<3>();
                let extra: Vec<String> = (0..2).map(|_| r.str().to_string()).collect();
                let orientation = omsi_cfg::parse_i32(&extra[0]);
                let grid = omsi_cfg::parse_i32(&extra[1]).max(1);
                self.text_textures.push(TextTexture {
                    variable: variable.clone(),
                    font: font.clone(),
                    width,
                    height,
                    full_color,
                    color,
                    orientation,
                    grid,
                });
                let mut lines = vec![variable, font, width.to_string(), height.to_string()];
                lines.extend(extra);
                self.text_textures_enh.push(lines);
            }
            "texchanges" => self.texchanges.push(r.str().to_string()),
            _ => return false,
        }
        true
    }
}

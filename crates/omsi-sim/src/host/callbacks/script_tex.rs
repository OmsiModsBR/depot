//! Script callbacks of the script tex family, dispatched from [`Host::callback`](super::super::VehicleHost).

use crate::host::{arg_i32, arg_idx, debug_text, st_load, VehicleHost};
use omsi_script::Stacks;

impl VehicleHost {
    /// The `callback`s of the script tex family; `false` when the name is not one of them.
    pub(in crate::host) fn cb_script_tex(&mut self, lname: &str, stacks: &mut Stacks) -> bool {
        match lname {
            // --- script textures (matrix displays)
            "stnewtex" => {
                let i = arg_idx(stacks.pop());
                if let Some(t) = self.script_textures.get_mut(i) {
                    t.renew();
                }
            }
            "stlock" => {
                let i = arg_idx(stacks.pop());
                if let Some(t) = self.script_textures.get_mut(i) {
                    t.locked = true;
                }
            }
            "stunlock" => {
                let i = arg_idx(stacks.pop());
                if let Some(t) = self.script_textures.get_mut(i) {
                    t.unlock();
                }
            }
            "stfilter" => {
                // OMSI creates the mip chain only when the script asks it to, normally
                // immediately after STUnlock. Remember that request for the renderer; it
                // must also refresh the chain when this matrix changes again.
                let i = arg_idx(stacks.pop());
                if let Some(t) = self.script_textures.get_mut(i) {
                    if !t.mipmaps {
                        t.mipmaps = true;
                        t.dirty = true;
                    }
                }
            }
            "stsetcolor" => {
                let b = stacks.pop();
                let g = stacks.pop();
                let r = stacks.pop();
                let a = stacks.pop();
                let i = arg_idx(stacks.pop());
                if let Some(t) = self.script_textures.get_mut(i) {
                    t.color = [r, g, b, a].map(|c| arg_i32(c).clamp(0, 255) as u8);
                }
            }
            "stdrawpixel" => {
                let y = arg_i32(stacks.pop());
                let x = arg_i32(stacks.pop());
                let i = arg_idx(stacks.pop());
                if let Some(t) = self.script_textures.get_mut(i) {
                    let c = t.color;
                    t.put(x, y, c);
                }
            }
            "stdrawrect" => {
                let y2 = arg_i32(stacks.pop());
                let x2 = arg_i32(stacks.pop());
                let y1 = arg_i32(stacks.pop());
                let x1 = arg_i32(stacks.pop());
                let i = arg_idx(stacks.pop());
                if let Some(t) = self.script_textures.get_mut(i) {
                    t.rect(x1, y1, x2, y2);
                }
            }
            "sttextout" => {
                let text = stacks.pop_str();
                let spacing = arg_i32(stacks.pop());
                let mode = stacks.pop();
                let font = arg_i32(stacks.pop());
                let y = arg_i32(stacks.pop());
                let x = arg_i32(stacks.pop());
                let i = arg_idx(stacks.pop());
                let atlas = self.font_atlas(font);
                if debug_text() {
                    log::info!("STTextOut(tex {i}, x {x}, y {y}, font {font}, spacing {spacing}, {text:?}) atlas {:?} from {:?} bitmap {:?} {}x{} E {:?}", atlas.as_ref().map(|a| a.font.name.clone()), atlas.as_ref().map(|a| a.font.path.clone()), atlas.as_ref().map(|a| a.font.alpha.clone()), atlas.as_ref().map(|a| a.width).unwrap_or(0), atlas.as_ref().map(|a| a.height).unwrap_or(0), atlas.as_ref().and_then(|a| a.font.glyph('E').map(|g| (g.x0, g.x1, g.y))));
                }
                if let (Some(t), Some(a)) = (self.script_textures.get_mut(i), atlas) {
                    t.text_out(&a, x, y, spacing, arg_i32(mode) as u8, &text);
                }
            }
            "streadpixel" => {
                let y = arg_i32(stacks.pop());
                let x = arg_i32(stacks.pop());
                let i = arg_idx(stacks.pop());
                // `STReadPixel` makes the selected texture's current ST colour the
                // pixel it read.  RHLib then asks `STGet*` of either that source texture
                // or a different target texture: the latter must retain its own draw
                // colour while the source keeps changing beneath the scaler.
                if let Some(t) = self.script_textures.get_mut(i) {
                    let color = t.get(x, y);
                    t.color = color;
                }
            }
            "stgetr" | "stgetg" | "stgetb" | "stgeta" => {
                let i = arg_idx(stacks.pop());
                let k = match lname {
                    "stgetr" => 0,
                    "stgetg" => 1,
                    "stgetb" => 2,
                    _ => 3,
                };
                stacks.push(
                    self.script_textures
                        .get(i)
                        .map(|t| t.color[k] as f32)
                        .unwrap_or(0.0),
                );
            }
            "stcopycolor" => {
                // colour of texture a → texture b
                let b = arg_idx(stacks.pop());
                let a = arg_idx(stacks.pop());
                if let Some(c) = self.script_textures.get(a).map(|t| t.color) {
                    if let Some(t) = self.script_textures.get_mut(b) {
                        t.color = c;
                    }
                }
            }
            "stloadtex" => {
                let i = arg_idx(stacks.pop());
                let path = stacks.pop_str();
                // relative to the vehicle's texture folder, like every texture name of the
                // vehicle: the Krüger matrix loads `..\..\Anzeigen\Krueger\<bitmap>`,
                // which is `Vehicles\Anzeigen\...` seen from `Vehicles\<bus>\Texture`
                // (and found in whichever content root has it)
                let mut full = omsi_cfg::resolve_path(
                    &omsi_cfg::resolve_path(&self.content_dir, "Texture"),
                    &path,
                );
                // (then the global `Texture` folder, as OMSI looks)
                if !omsi_cfg::vfs::is_file(&full) {
                    if let Some((_, p)) =
                        omsi_cfg::find_in_roots(&format!("Texture/{}", path.replace('\\', "/")))
                    {
                        full = p;
                    }
                }
                // (OMSI pushes nothing: a file it cannot find goes to its log only)
                match st_load(&full) {
                    Ok(img) => {
                        if let Some(t) = self.script_textures.get_mut(i) {
                            t.load(img.width, img.height, &img.rgba);
                        }
                    }
                    Err(e) => log::warn!(
                        "STLoadTex: did not find texture file {}: {e}",
                        full.display()
                    ),
                }
            }
            _ => return false,
        }
        true
    }
}

//! Script callbacks of the fonts family, dispatched from [`Host::callback`](super::super::VehicleHost).

use crate::host::{arg_i32, debug_text, VehicleHost};
use omsi_script::Stacks;

impl VehicleHost {
    /// The `callback`s of the fonts family; `false` when the name is not one of them.
    pub(in crate::host) fn cb_fonts(&mut self, lname: &str, stacks: &mut Stacks) -> bool {
        match lname {
            "getfontindex" => {
                let font = stacks.pop_str();
                let idx = match self
                    .fonts
                    .entries
                    .iter()
                    .position(|f| f.0.eq_ignore_ascii_case(&font))
                {
                    Some(i) => i,
                    None => {
                        // an empty name or a depot string that is no font (the Krüger
                        // matrix asks for the custom fonts of termini 14991.. this way) is
                        // looked up once and remembered as missing
                        let atlas = if font.trim().is_empty() {
                            None
                        } else {
                            self.font_lib.as_ref().and_then(|l| l.lock().load(&font))
                        };
                        self.fonts.entries.push((font, atlas));
                        self.fonts.entries.len() - 1
                    }
                };
                // OMSI answers -1 for a font it does not have, and scripts rely on it: the
                // Krüger matrix falls back from a missing custom font to the one before with
                // `1 + 31416 * l1 1 + max 31415 % 1 -`, which only works for -1. An index for
                // a font that cannot draw left the destination display blank.
                let found = self.fonts.entries[idx].1.is_some();
                stacks.push(if found { idx as f32 } else { -1.0 });
            }
            "textlength" => {
                let font = arg_i32(stacks.pop());
                let text = stacks.pop_str();
                // as OMSI measures it: the glyphs and the gaps between them
                let w = self
                    .font_atlas(font)
                    .map(|a| a.font.text_width(&text) as f32)
                    .unwrap_or(0.0);
                if debug_text() {
                    log::info!(
                        "TextLength(font {font} = {:?}, {text:?}) = {w}",
                        self.font_atlas(font).map(|a| a.font.name.clone())
                    );
                }
                stacks.push(w);
            }
            _ => return false,
        }
        true
    }
}

//! The HUD: what is drawn over the picture outside the menus - the time and speed in OMSI's
//! `.oft` fonts, the chat, notes and notifications, the information bar, the tooltip, the
//! name tags and the old game menu - as the frame hands it over in a [`Frame`].
//!
//! The package has three parts:
//!
//! - `domain`: the objects a frame hands over (the [`Frame`] and its parts, the information
//!   bar, the notifications), with no drawing.
//! - `interface`: what the game uses, and the only way in: `input` is what the frame hands
//!   over and calls (and the rectangles it reads back for the mouse), `output` what the HUD
//!   asks of the game (the renderer).
//! - `implementation`: how it is drawn. `legacy` is the old interface, drawn on the CPU into
//!   textures, until its parts move to `omsi-ui`.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub use interface::*;

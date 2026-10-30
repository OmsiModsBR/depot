//! The bus preview of the launcher and of the game's "Add vehicle" window, drawn by a helper
//! process.
//!
//! The launcher's window has no 3D renderer (see `launcher::gfx`): the chosen bus is drawn by the game
//! binary started again as `depot --preview-helper`. The helper opens the game's renderer
//! without a window, reads the bus with the showroom (`Showroom`), and sends each picture
//! back over its standard output, while the launcher tells it what to show (the bus and its
//! light, the size, the mouse turning it) over its standard input. It runs only while a page
//! shows the preview, ends a moment after the page is left, and ends with the launcher (its
//! input closes then). The last settled picture of each bus and paint is kept in
//! `<data>/previews` and shown at once the next time that bus is chosen, until the helper
//! has drawn the live one. On a phone the helper runs on a thread of the launcher instead
//! (an app does not start processes of its own).
//!
//! `domain` holds the bus shown, the picture's shape and the messages, `implementation` the
//! showroom, the helper, the link and the kept pictures, `interface` what the launcher and the
//! game call (`input`) and what it reads from the rest of the game (`output`).

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub use interface::input::*;

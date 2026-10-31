//! The launcher's pages, drawn from here one by one as they move out of the game's window
//! (`omsi-app::launcher`), and the parts every page shares (its heading, its tabs, a card's
//! title).
//!
//! `domain` holds what a page is drawn with (`Frame`), `implementation` the headings and the
//! pages, `interface` what the window calls (`input`) and what the pages read from the rest of
//! the crate (`output`).

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub use interface::input::*;

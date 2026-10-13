//! The entry: the one package the game sees. Every `pub fn` on `Renderer` lives here, split
//! by subject (`renderer`, `scene`, `assets`, `lights`), and `types` re-exports the public
//! types of the domains. Nothing in the crate imports from `api`.

mod assets;
mod lights;
mod renderer;
mod scene;
mod surface;
mod types;

pub use types::*;

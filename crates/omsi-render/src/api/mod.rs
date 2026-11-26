//! The entry: the one package the game sees. Every `pub fn` on `Renderer` lives here, split
//! by subject (`renderer`, `scene`, `assets`, `lights`), and `types` re-exports the public
//! types of the domains. Nothing in the crate imports from `api`.

pub mod assets;
pub mod lights;
pub mod mirror;
pub mod renderer;
pub mod scene;
pub mod surface;
pub mod types;

pub use types::*;

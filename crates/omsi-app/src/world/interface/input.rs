//! What the rest of the game reaches of the world, by subject.

pub(crate) use super::super::implementation::editor;
pub(crate) use super::super::implementation::export;
pub(crate) use super::super::implementation::map_surface;
pub(crate) use super::super::implementation::placing;
pub(crate) use super::super::implementation::tiles;
pub(crate) use super::super::implementation::world_load;

/// The scene: its types (`domain/scene.rs`) and its procedures (`implementation/scene.rs`).
pub(crate) mod scene {
    pub use crate::world::domain::scene::*;
    pub use crate::world::implementation::scene::*;
}

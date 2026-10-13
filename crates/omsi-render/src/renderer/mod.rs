//! The renderer: its state, opening it, building everything it draws with, and the frame:
//! preparing it and recording its passes, ending by handing it to an `output` destination.

mod assets;
mod build;
mod encode;
mod frame;
mod lights;
mod lists;
mod options;
mod prepare;
mod sky;
mod state;

pub use options::{RenderOptions, AUTO_SCALE_PIXELS};
pub use state::{Renderer, LAMP_CODE_STRIDE, MAX_LAMPS_PER_MESH, SHADOW_SIZE};

pub(crate) use options::scene_scale_for;

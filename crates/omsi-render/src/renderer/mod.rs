//! The renderer: its state, opening it, building everything it draws with, and the frame:
//! preparing it and recording its passes, ending by handing it to an `output` destination.

mod assets;
mod build;
mod effects;
mod encode;
mod frame;
mod lights;
mod lists;
mod options;
mod output;
mod passes;
mod prepare;
mod reflections;
mod sky;
mod state;
mod targets;
mod upload;

pub use options::{RenderOptions, AUTO_SCALE_PIXELS};
pub use state::{Renderer, LAMP_CODE_STRIDE, MAX_LAMPS_PER_MESH, SHADOW_SIZE};

pub(crate) use options::scene_scale_for;

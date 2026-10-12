//! The renderer: its state, opening it, building everything it draws with, and the frame:
//! preparing it and recording its passes, ending by handing it to an `output` destination.

mod build;
mod frame;
mod open;
mod options;
mod prepare;
mod state;

pub use options::{RenderOptions, AUTO_SCALE_PIXELS};
pub use state::{
    Renderer, DEPTH_FORMAT, LAMP_CODE_STRIDE, MAX_LAMPS_PER_MESH, MSAA, SHADOW_RANGE,
    SHADOW_RANGE_CLOSE, SHADOW_RANGE_FAR, SHADOW_SIZE,
};

#[cfg(test)]
pub(crate) use options::scene_scale_for;
pub(crate) use state::{
    rt_gbuf, AUX_FORMAT, GBUF_FORMAT, HDR_FORMAT, MASK_FORMAT, SHADOW_CLOSE_MAX,
};

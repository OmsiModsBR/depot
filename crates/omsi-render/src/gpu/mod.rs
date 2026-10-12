//! The device and what it can do: opening it (and the reduced renderers remembered per
//! adapter), its memory and limits, how the per-draw arrays reach the shaders, GPU timers,
//! and what to do when the driver fails. Knows nothing of the scene or of a frame.

mod arrays;
mod caps;
mod errors;
mod open;
mod timers;

pub use caps::{gl_backend, wait_gpu, ADAPTER_TEXTURE_MB, ADAPTER_VRAM_MB};
pub use errors::{catch, catching};

#[cfg(test)]
pub(crate) use arrays::array_tex_spans;
pub(crate) use arrays::{
    array_layout_entry, array_layout_entry_on, array_path, ArrayPath, GpuArray, ARRAY_PATH,
    ARRAY_TEX_WIDTH,
};
pub(crate) use caps::{
    dedicated_vram_mb, gl_worker_turn, sixteen_texture_units, vulkan_vram_mb, GL_BACKEND,
};
pub(crate) use errors::{gpu_error_text, CATCHING};
pub(crate) use open::{
    basic_pipelines, fallback_load, fallback_store, DevicePoller, BASIC_PIPELINES,
};
pub(crate) use timers::{pass_timer, GpuTimers};

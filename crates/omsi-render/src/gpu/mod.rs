//! The device and what it can do: opening it (and the reduced renderers remembered per
//! adapter), its memory and limits, how the per-draw arrays reach the shaders, GPU timers,
//! and what to do when the driver fails. Knows nothing of the scene or of a frame.

mod arrays;
mod buffers;
mod caps;
mod context;
mod errors;
mod formats;
mod lost;
mod open;
mod timers;

pub use caps::{gl_backend, poll_gpu, wait_gpu, ADAPTER_TEXTURE_MB, ADAPTER_VRAM_MB};
pub use errors::{catch, catching};
pub use formats::{DEPTH_FORMAT, MSAA};
#[cfg(test)]
pub(crate) use lost::mark_lost;
pub use lost::GpuWaitError;

#[cfg(test)]
pub(crate) use arrays::array_tex_spans;
pub(crate) use arrays::{
    array_layout_entry, array_layout_entry_on, array_path, float_tex, ArrayPath, GpuArray,
    ARRAY_PATH, ARRAY_TEX_WIDTH,
};
pub(crate) use buffers::buffer_init;
pub(crate) use caps::{
    dedicated_vram_mb, gl_worker_turn, sixteen_texture_units, vulkan_vram_mb, GL_BACKEND,
};
pub(crate) use context::{GpuContext, Samplers};
pub(crate) use errors::{gpu_error_text, CATCHING};
pub(crate) use formats::{
    color_targets, rt_gbuf, AUX_FORMAT, GBUF_FORMAT, HDR_FORMAT, MASK_FORMAT, RT_GBUF,
};
pub(crate) use lost::{
    fake_due, fake_poll_loss, guarded_poll, let_go, lost_why, read_buffer_with, wait_poll,
    wait_with,
};
pub(crate) use open::{basic_pipelines, fallback_load, fallback_store, BASIC_PIPELINES};
pub(crate) use timers::{pass_timer, GPU_TIMER_PASSES};

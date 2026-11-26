//! The device and what it can do: opening it (and the reduced renderers remembered per
//! adapter), its memory and limits, how the per-draw arrays reach the shaders, GPU timers,
//! and what to do when the driver fails. Knows nothing of the scene or of a frame.

pub mod arrays;
pub mod buffers;
pub mod caps;
pub mod context;
pub mod debug;
pub mod errors;
pub mod fault;
pub mod formats;
pub mod lost;
pub mod open;
pub mod timers;
pub mod util;

pub use caps::{gl_backend, note_adapter, poll_gpu, wait_gpu, ADAPTER_TEXTURE_MB, ADAPTER_VRAM_MB};
pub use errors::{catch, catching};
pub use formats::{DEPTH_FORMAT, MSAA};
#[cfg(feature = "test-support")]
pub use lost::mark_lost;
pub use lost::GpuWaitError;

#[cfg(feature = "test-support")]
pub use arrays::array_tex_spans;
pub use arrays::{
    array_layout_entry, array_layout_entry_on, array_path, float_tex, ArrayPath, GpuArray,
    ARRAY_PATH, ARRAY_TEX_WIDTH,
};
pub use buffers::buffer_init;
pub use caps::{
    dedicated_vram_mb, gl_worker_turn, sixteen_texture_units, vulkan_vram_mb, GL_BACKEND,
};
pub use context::{GpuContext, Samplers};
pub use debug::debug;
pub use errors::{gpu_error_text, CATCHING};
pub use fault::{due as fault_due, is as fault_is, Hook as FaultHook};
pub use formats::{color_targets, HDR_FORMAT, MASK_FORMAT};
pub use lost::{guarded_poll, let_go, lost_why, read_buffer_with, wait_poll, wait_with};
pub use open::{basic_pipelines, fallback_load, fallback_store, BASIC_PIPELINES};
pub use timers::{pass_timer, GPU_TIMER_PASSES};
pub use util::{
    depth_texture_entry, fullscreen_pipeline, mip_levels, sampler_entry, target_2d, texture_entry,
    uniform_entry,
};

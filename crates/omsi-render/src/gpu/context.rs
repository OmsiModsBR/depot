//! What the renderer keeps of the device besides the device itself: the window's format, the
//! error flags, the encoding workers and the poller, and the layouts and samplers the shaders
//! share.

use super::open::DevicePoller;
use std::sync::Arc;

/// The bind group layouts the scene's shaders share.
pub(crate) struct Layouts {
    pub(crate) camera_layout: wgpu::BindGroupLayout,
    pub(crate) material_layout: wgpu::BindGroupLayout,
    pub(crate) sky_layout: wgpu::BindGroupLayout,
}

/// The samplers the materials and the lighting share.
pub(crate) struct Samplers {
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) lin_sampler: wgpu::Sampler,
    /// `[matl_texadress_clamp]` (and border, mirror-once) and `[matl_texadress_mirror]`.
    pub(crate) clamp_sampler: wgpu::Sampler,
    pub(crate) mirror_sampler: wgpu::Sampler,
}

/// The device's surroundings: the window's format, what went wrong with it, the encoding
/// workers, and its poller (the renderer's last field, so that it is dropped last).
pub(crate) struct GpuContext {
    pub(crate) format: wgpu::TextureFormat,
    /// Set by the device's error handler when something failed with multisampling on.
    pub(crate) gpu_error: Arc<std::sync::atomic::AtomicBool>,
    /// A GPU error while Enhanced+ traces rays: its ray tracing is left out from the next
    /// frame on (a frame whose commands the device refused shows nothing at all - left to
    /// go on, the picture stood still on the loading screen while the game ran behind it).
    pub(crate) rt_error: Arc<std::sync::atomic::AtomicBool>,
    /// The card ran out of memory since the last `take_out_of_memory`.
    pub(crate) out_of_memory: Arc<std::sync::atomic::AtomicBool>,
    /// Why the graphics device was lost (a driver reset, the card removed), once it was.
    pub(crate) device_lost: Arc<std::sync::Mutex<Option<String>>>,
    /// Reuse a small set of encoding workers instead of creating OS threads for each
    /// main/mirror picture. Keep these separate from simulation's worker queue.
    pub(crate) encoding_pool: Option<rayon::ThreadPool>,
    pub(crate) _device_poller: Option<DevicePoller>,
}

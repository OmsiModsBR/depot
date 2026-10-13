//! What the renderer keeps of the device besides the device itself: the window's format, the
//! error flags, the encoding workers and the poller, and the samplers the shaders share.

use super::errors::gpu_error_text;
use super::open::DevicePoller;
use std::sync::Arc;

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

impl GpuContext {
    /// Sets the device's error and loss handlers (see `Renderer::build`), the encoding workers and
    /// the poller.
    pub(crate) fn open(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        msaa: u32,
        ray_tracing: bool,
    ) -> Self {
        // A GPU error while multisampling is on is logged and switches multisampling off
        // at the next frame (`render_inner`). Otherwise it is logged and the game goes on:
        // wgpu's own handler ends the process, and one call a driver refused (a limit of
        // that card, a bigger map than the last) closed the game a few seconds into the
        // drive - a wrong picture for a frame is better than no game. The first errors and
        // then every thousandth reach the log.
        let gpu_error = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let rt_error = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let out_of_memory = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let device_lost: Arc<std::sync::Mutex<Option<String>>> = Default::default();
        {
            let lost = device_lost.clone();
            device.set_device_lost_callback(move |reason, message| {
                // (dropping the device at the end also calls this: that is no loss)
                if matches!(reason, wgpu::DeviceLostReason::Destroyed) {
                    return;
                }
                log::error!("the graphics device was lost ({reason:?}): {message}");
                *lost.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(format!("{reason:?}: {message}"));
            });
        }
        {
            let flag = gpu_error.clone();
            let rt_flag = rt_error.clone();
            let rt_on = ray_tracing;
            let oom = out_of_memory.clone();
            let count = Arc::new(std::sync::atomic::AtomicU64::new(0));
            device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
                if matches!(e, wgpu::Error::OutOfMemory { .. }) {
                    oom.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                let n = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if rt_on {
                    // (the ray tracing goes first: the likelier cause, and the dearer feature)
                    if !rt_flag.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        log::error!("GPU error with ray tracing (Enhanced+ draws as Enhanced from now on): {}", gpu_error_text(&e));
                    }
                } else if msaa > 1 && !flag.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    log::error!("GPU error with {msaa}x MSAA (drawing without it from now on): {}", gpu_error_text(&e));
                } else if n < 20 || n % 1000 == 0 {
                    log::error!("GPU error #{} (the game goes on): {}", n + 1, gpu_error_text(&e));
                }
            }));
        }
        Self {
            _device_poller: DevicePoller::start(device),
            format,
            gpu_error,
            rt_error,
            out_of_memory,
            device_lost,
            encoding_pool: if omsi_cfg::env::var_os("OMSI_NO_RENDER_POOL").is_some() {
                None
            } else {
                let workers = std::thread::available_parallelism()
                    .map(|n| n.get() / 2)
                    .unwrap_or(2)
                    .clamp(2, 8);
                rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .thread_name(|i| format!("omsi-render-{i}"))
                    .build()
                    .ok()
            },
        }
    }
}

impl Samplers {
    /// The shared samplers (`anisotropy`: `RenderOptions::anisotropy`).
    pub(crate) fn new(device: &wgpu::Device, anisotropy: u16) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: anisotropy,
            ..Default::default()
        });
        // [matl_texadress_clamp]: a number plate is a small quad whose texture must not
        // repeat beyond its edge - repeated, the plate text tiled the whole rear of the bus
        let clamp_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: anisotropy,
            ..Default::default()
        });
        let mirror_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            address_mode_w: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: anisotropy,
            ..Default::default()
        });
        let lin_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self {
            clamp_sampler,
            mirror_sampler,
            lin_sampler,
            sampler,
        }
    }
}

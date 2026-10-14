//! What the renderer keeps of the device besides the device itself: the window's format, the
//! error flags, the encoding workers and the poller, and the samplers the shaders share.

use super::errors::GpuErrors;
use super::lost::{lost_why, mark_lost, LostFlag};
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
    pub(crate) device_lost: LostFlag,
    /// The device's error handler (the `oom` test hook goes through it as a real error).
    pub(crate) errors: Arc<GpuErrors>,
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
        // (what an error does: see `GpuErrors`)
        let errors = Arc::new(GpuErrors::new(msaa, ray_tracing));
        let device_lost = LostFlag::default();
        {
            let lost = device_lost.clone();
            device.set_device_lost_callback(move |reason, message| {
                on_device_lost(&lost, reason, &message);
            });
        }
        {
            let errors = errors.clone();
            device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
                errors.on_error(&e);
            }));
        }
        let (gpu_error, rt_error, out_of_memory) = (
            errors.gpu_error.clone(),
            errors.rt_error.clone(),
            errors.out_of_memory.clone(),
        );
        Self {
            _device_poller: DevicePoller::start(device, device_lost.clone()),
            errors,
            format,
            gpu_error,
            rt_error,
            out_of_memory,
            device_lost,
            encoding_pool: if omsi_cfg::flags::NO_RENDER_POOL.is_set() {
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

impl GpuContext {
    /// What this context, made anew for the same device (the fallbacks without
    /// multisampling or ray tracing), takes over from `old`: an "out of memory" not yet
    /// taken (the error that switched the multisampling off was often that one, and the
    /// game's textures were then never cut) and a loss already known.
    pub(crate) fn inherit(&self, old: &GpuContext) {
        if old.out_of_memory.load(std::sync::atomic::Ordering::Relaxed) {
            self.out_of_memory
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        // (and the `oom` test hook, sent once for the run)
        if old
            .errors
            .fake_oom_sent
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.errors
                .fake_oom_sent
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if let Some(why) = lost_why(&old.device_lost) {
            mark_lost(&self.device_lost, why);
        }
    }
}

/// The device's loss handler: the loss marked with its reason. Whether it was a loss.
pub(crate) fn on_device_lost(
    lost: &LostFlag,
    reason: wgpu::DeviceLostReason,
    message: &str,
) -> bool {
    // (dropping the device at the end also calls this: that is no loss)
    if matches!(reason, wgpu::DeviceLostReason::Destroyed) {
        return false;
    }
    mark_lost(lost, format!("{reason:?}: {message}"));
    true
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn noop_device() -> wgpu::Device {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::NOOP;
        descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        pollster::block_on(adapter.request_device(&Default::default()))
            .unwrap()
            .0
    }

    #[test]
    fn a_context_made_anew_takes_over_a_shortage_and_a_loss() {
        let device = noop_device();
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let old = GpuContext::open(&device, format, 4, false);
        old.out_of_memory.store(true, Ordering::Relaxed);
        mark_lost(&old.device_lost, "Unknown: before the fallback");
        let new = GpuContext::open(&device, format, 1, false);
        old.errors.fake_out_of_memory_once();
        new.inherit(&old);
        assert!(new.out_of_memory.load(Ordering::Relaxed));
        assert!(
            !new.errors.fake_out_of_memory_once(),
            "the test hook fires once a run"
        );
        assert_eq!(
            lost_why(&new.device_lost).as_deref(),
            Some("Unknown: before the fallback")
        );
    }

    #[test]
    fn a_context_made_anew_after_nothing_went_wrong_starts_clean() {
        let device = noop_device();
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let old = GpuContext::open(&device, format, 4, false);
        let new = GpuContext::open(&device, format, 1, false);
        new.inherit(&old);
        assert!(!new.out_of_memory.load(Ordering::Relaxed));
        assert!(!new.errors.fake_oom_sent.load(Ordering::Relaxed));
        assert_eq!(lost_why(&new.device_lost), None);
    }

    #[test]
    fn the_handler_of_the_device_marks_what_reaches_it() {
        // the error handler set on the device is the context's: an "out of memory" from
        // the device is noted
        let device = noop_device();
        let ctx = GpuContext::open(&device, wgpu::TextureFormat::Rgba8UnormSrgb, 1, false);
        assert!(ctx.errors.fake_out_of_memory_once());
        assert!(ctx.out_of_memory.load(Ordering::Relaxed));
    }

    #[test]
    fn a_device_let_go_of_is_no_loss() {
        let lost = LostFlag::default();
        assert!(!on_device_lost(
            &lost,
            wgpu::DeviceLostReason::Destroyed,
            "dropped"
        ));
        assert_eq!(lost_why(&lost), None);
    }

    #[test]
    fn a_device_the_driver_gave_up_is_marked_with_the_reason() {
        let lost = LostFlag::default();
        assert!(on_device_lost(
            &lost,
            wgpu::DeviceLostReason::Unknown,
            "DXGI_ERROR_DEVICE_REMOVED"
        ));
        assert_eq!(
            lost_why(&lost).as_deref(),
            Some("Unknown: DXGI_ERROR_DEVICE_REMOVED")
        );
    }
}

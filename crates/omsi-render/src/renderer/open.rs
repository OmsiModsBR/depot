//! Opening the renderer: choosing the adapter, opening the device with its fallbacks, and
//! what the device reports afterwards (memory, loss).

use super::options::RenderOptions;
use super::state::{Renderer, DEPTH_FORMAT, MASK_FORMAT, MSAA};
use crate::assets::RT_BUFFERS;
use crate::gpu::{
    basic_pipelines, dedicated_vram_mb, fallback_load, fallback_store, gpu_error_text,
    vulkan_vram_mb, ArrayPath, ADAPTER_TEXTURE_MB, ADAPTER_VRAM_MB, ARRAY_PATH, BASIC_PIPELINES,
    GL_BACKEND,
};
use anyhow::anyhow;
use anyhow::{Context, Result};

impl Renderer {
    /// Create a renderer with the default options. `surface` is used to pick a compatible
    /// adapter and format.
    pub async fn new(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        format: Option<wgpu::TextureFormat>,
    ) -> Result<Renderer> {
        Self::new_with(instance, surface, format, RenderOptions::default()).await
    }

    /// Create a renderer with the given graphics settings.
    pub async fn new_with(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        format: Option<wgpu::TextureFormat>,
        options: RenderOptions,
    ) -> Result<Renderer> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: surface, force_fallback_adapter: false, ..Default::default() })
            .await
            .map_err(|e| anyhow!("no graphics adapter that can draw the game was found (Metal, Vulkan, DirectX 12 or OpenGL 3.3 or later); updating the graphics driver often helps: {e}"))?;
        Self::new_on(adapter, surface, format, options).await
    }

    /// The adapters of `instance` that can show `surface`, the ones worth trying first first:
    /// a graphics card of its own, then the processor's graphics, then anything else (a
    /// software renderer last).
    pub fn adapters_for(
        instance: &wgpu::Instance,
        surface: &wgpu::Surface<'_>,
    ) -> Vec<wgpu::Adapter> {
        let mut v: Vec<wgpu::Adapter> =
            pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
                .into_iter()
                .filter(|a| a.is_surface_supported(surface))
                .collect();
        let rank = |a: &wgpu::Adapter| match a.get_info().device_type {
            wgpu::DeviceType::DiscreteGpu => 0,
            wgpu::DeviceType::IntegratedGpu => 1,
            wgpu::DeviceType::VirtualGpu | wgpu::DeviceType::Other => 2,
            wgpu::DeviceType::Cpu => 3,
        };
        v.sort_by_key(rank);
        v
    }

    /// Create a renderer on this adapter.
    pub async fn new_on(
        adapter: wgpu::Adapter,
        surface: Option<&wgpu::Surface<'_>>,
        format: Option<wgpu::TextureFormat>,
        options: RenderOptions,
    ) -> Result<Renderer> {
        let info = adapter.get_info();
        let (asked_format, asked_options) = (format, options);
        // test hooks for an adapter that cannot be opened: an error, or wgpu going down
        match omsi_cfg::env::var("OMSI_FAKE_GPU_ERROR").as_deref() {
            Ok("open") => {
                return Err(anyhow!(
                    "test: {} refused (OMSI_FAKE_GPU_ERROR=open)",
                    info.name
                ))
            }
            Ok("open-panic") => panic!(
                "test: {} went down while being opened (OMSI_FAKE_GPU_ERROR=open-panic)",
                info.name
            ),
            _ => {}
        }
        // What the textures may take on this adapter (wgpu does not tell a card's memory):
        // a discrete card is taken for one of 2-4 GB, whose rest the pictures (the render
        // targets, the shadow maps) and the driver need; an integrated one shares the
        // system's memory, Apple's generously
        let vram = dedicated_vram_mb(&info).or_else(|| vulkan_vram_mb(&adapter));
        let guess_mb: u64 = match info.device_type {
            // (a card of 2 or 3 GB, where Windows says: half of it - 1600 MB of a GTX 1050's
            // 2 GB left too little for the rest, and its Vulkan device was lost at the start;
            // a card of 2 GB a third of it - with half, 4x MSAA, SSAO and the shadows its
            // DirectX 12 device still ran out of memory on Grundorf within seconds, #114)
            wgpu::DeviceType::DiscreteGpu => vram.filter(|v| *v >= 512).map_or(1600, |v| {
                if v <= 2560 {
                    v * 35 / 100
                } else if v <= 6144 {
                    (v / 2).min(1600)
                } else {
                    v * 3 / 10
                }
            }),
            wgpu::DeviceType::IntegratedGpu if info.backend == wgpu::Backend::Metal => 3000,
            wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::VirtualGpu => 1000,
            _ => 800,
        };
        ADAPTER_TEXTURE_MB.store(guess_mb, std::sync::atomic::Ordering::Relaxed);
        let discrete_vram = vram
            .filter(|_| info.device_type == wgpu::DeviceType::DiscreteGpu)
            .unwrap_or(0);
        ADAPTER_VRAM_MB.store(discrete_vram, std::sync::atomic::Ordering::Relaxed);
        log::info!(
            "graphics adapter: {} ({:?}, {:?}{}), texture memory taken for it: {guess_mb} MB",
            info.name,
            info.device_type,
            info.backend,
            vram.map(|v| format!(", {v} MB of its own"))
                .unwrap_or_default()
        );
        // The legacy Intel Windows Vulkan branch has repeatedly crashed inside igvk64.dll
        // while compiling the larger multisampled/SSAO pipeline set. This is a driver access
        // violation, so wgpu cannot turn it into a recoverable error. Start those adapters
        // with the conservative feature set instead. OMSI_INTEL_FULL_GPU=1 is useful for
        // retesting after a driver update without rebuilding the game.
        let intel_vulkan_safe = cfg!(windows)
            && info.backend == wgpu::Backend::Vulkan
            && info.vendor == 0x8086
            && omsi_cfg::env::var_os("OMSI_INTEL_FULL_GPU").is_none();
        let options = if intel_vulkan_safe {
            log::warn!(
                "Intel Vulkan adapter detected ({}): using the stable driver profile (1x MSAA, 1x anisotropy, SSAO and runtime texture compression off); set OMSI_INTEL_FULL_GPU=1 after updating the Intel driver to retry the requested settings",
                info.name
            );
            RenderOptions {
                msaa: 1,
                anisotropy: 1,
                ssao: false,
                compress_textures: false,
                ..options
            }
        } else {
            options
        };
        // A small or shared graphics chip (the processor's graphics outside a Mac, a phone,
        // a card of up to 2.5 GB, anything on OpenGL) gets a lighter picture whatever the
        // settings ask: no SSAO and no multisampling, smaller shadow maps; a card of up to
        // 4 GB no SSAO and at most 2x. (The settings' "High" on such a machine ran out of
        // memory or at a dozen frames a second.) OMSI_FULL_GPU=1 asks for the settings as
        // they are.
        GL_BACKEND.store(
            info.backend == wgpu::Backend::Gl,
            std::sync::atomic::Ordering::Relaxed,
        );
        let full = omsi_cfg::env::var_os("OMSI_FULL_GPU").is_some();
        let weak = !full
            && (info.backend == wgpu::Backend::Gl
                // (a phone's chip, whatever type its driver reports: some say "other")
                || cfg!(target_os = "android")
                || (info.device_type == wgpu::DeviceType::IntegratedGpu && info.backend != wgpu::Backend::Metal)
                || vram.is_some_and(|v| v <= 2560));
        let modest = !full && !weak && vram.is_some_and(|v| v <= 4200);
        let options = if weak {
            log::warn!("{}: a small or shared graphics chip - no SSAO, no MSAA, shadow maps of at most 1024 (OMSI_FULL_GPU=1 keeps the settings)", info.name);
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: options.shadow_size.min(1024),
                ..options
            }
        } else if modest {
            log::info!("{}: {} MB of its own - no SSAO, at most 2x MSAA and 2048 shadow maps (OMSI_FULL_GPU=1 keeps the settings)", info.name, vram.unwrap_or(0));
            RenderOptions {
                msaa: options.msaa.min(2),
                ssao: false,
                shadow_size: options.shadow_size.min(2048),
                ..options
            }
        } else {
            options
        };
        let shadow_size = options
            .shadow_size
            .clamp(512, if intel_vulkan_safe { 2048 } else { 8192 });
        let mut limits = wgpu::Limits::default().using_resolution(adapter.limits());
        if intel_vulkan_safe {
            // Do not request every large limit the Intel driver advertises. In particular,
            // asking for its maximum storage-buffer and buffer sizes makes 31.0.101.2141
            // crash in vkCreateDevice instead of returning a VkResult. The WebGPU defaults
            // are ample for the renderer and stay on the driver's well-tested path.
            limits = wgpu::Limits::default().using_resolution(adapter.limits());
        } else {
            limits.max_storage_buffer_binding_size =
                adapter.limits().max_storage_buffer_binding_size;
            limits.max_buffer_size = adapter.limits().max_buffer_size;
        }
        // an older or smaller graphics chip (an OpenGL one, a GT 530) does not reach the
        // WebGPU defaults: asked for them anyway, the device was never opened
        if !limits.check_limits(&adapter.limits()) {
            log::warn!(
                "{}: below the standard limits; using what it has",
                info.name
            );
            limits = adapter.limits();
        }
        // OMSI_GPU_LIMITS=default|downlevel: the WebGPU defaults (or the downlevel ones) and
        // nothing more, whatever this machine could do - to find what a stricter driver refuses
        match omsi_cfg::env::var("OMSI_GPU_LIMITS").as_deref() {
            Ok("default") => limits = wgpu::Limits::default(),
            Ok("downlevel") => limits = wgpu::Limits::downlevel_defaults(),
            _ => {}
        }
        // the shadow atlas is two maps wide: no wider than the card draws
        let shadow_size = shadow_size
            .min(limits.max_texture_dimension_2d / 2)
            .max(256);
        let format = format
            .or_else(|| {
                surface.map(|s| {
                    let formats = s.get_capabilities(&adapter).formats;
                    // (an Android driver lists the plain RGBA8 first: the picture, written in
                    // linear light for an sRGB target, came out dark and flat)
                    if cfg!(target_os = "android") {
                        if let Some(f) = formats.iter().find(|f| f.is_srgb()) {
                            return *f;
                        }
                    }
                    formats[0]
                })
            })
            .unwrap_or(wgpu::TextureFormat::Rgba8UnormSrgb);
        // Every target the scene is drawn into with multisampling (the swap chain or mirror
        // format, the HDR target of the enhanced path and its screen mask, the depth buffer)
        // must take the sample count, and the colour targets must resolve. A device judges
        // that by the WebGPU table, which promises only 1x and 4x, unless it was asked for
        // the adapter's own table: the launcher's "2x MSAA" (which Apple GPUs do support)
        // was a fatal validation error before the first frame because the adapter's table
        // said yes and the device's said no. So the adapter's table is asked for when the
        // wanted count needs it (2x, 8x), and the count is checked against the table the
        // device will use.
        let wanted = match options.msaa {
            1 | 2 | 4 | 8 => options.msaa,
            _ => MSAA,
        };
        let targets = [
            format,
            wgpu::TextureFormat::Rgba16Float,
            DEPTH_FORMAT,
            MASK_FORMAT,
        ];
        let takes = |flags: wgpu::TextureFormatFeatureFlags, f: wgpu::TextureFormat, n: u32| {
            flags.sample_count_supported(n)
                && (n == 1
                    || f.is_depth_stencil_format()
                    || flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE))
        };
        let adapter_table_needed = !targets.iter().all(|&f| {
            takes(
                f.guaranteed_format_features(wgpu::Features::empty()).flags,
                f,
                wanted,
            )
        });
        let mut required_features = if adapter_table_needed && info.backend != wgpu::Backend::Noop {
            adapter.features() & wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
        } else {
            wgpu::Features::empty()
        };
        if omsi_cfg::env::var_os("OMSI_GPU_TIMERS").is_some() {
            required_features |= adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
        }
        // DXT textures stay compressed on the GPU where it takes them (Apple silicon does);
        // OMSI_NO_BC=1 uploads everything as RGBA (the old way, for comparisons)
        if omsi_cfg::env::var_os("OMSI_NO_BC").is_none() {
            required_features |= adapter.features() & wgpu::Features::TEXTURE_COMPRESSION_BC;
        }
        if intel_vulkan_safe {
            // Keep vkCreateDevice entirely free of optional extensions. Compressed source
            // textures are decoded to RGBA by upload_texture on this device.
            required_features = wgpu::Features::empty();
        }
        // Enhanced+: hardware ray queries where the device has them (Apple silicon from the
        // M3/A17 on, RTX and RDNA 2 cards and newer through Vulkan and Direct3D 12 - with DXC,
        // which the Windows build ships beside the game); OMSI_NO_RT=1 leaves them out. Should
        // the device refuse its work all the same, it falls back to Enhanced (`rt_error`).
        let ray_query = options.ray_tracing
            && !intel_vulkan_safe
            && info.backend != wgpu::Backend::Noop
            && adapter
                .features()
                .contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY)
            && omsi_cfg::env::var_os("OMSI_NO_RT").is_none();
        if ray_query {
            required_features |= wgpu::Features::EXPERIMENTAL_RAY_QUERY;
            limits = limits.using_acceleration_structure_values(adapter.limits());
        } else if options.ray_tracing {
            log::warn!(
                "{}: no hardware ray queries; Enhanced+ is drawn as Enhanced",
                info.name
            );
        }
        // the per-draw arrays as storage buffers where the device reads them in a vertex
        // shader (three there, two lights arrays in a fragment shader), else as textures;
        // OMSI_GPU_ARRAYS=textures|nostorage takes those paths on any device
        let downlevel = adapter.get_downlevel_capabilities().flags;
        let storage = limits.max_storage_buffers_per_shader_stage;
        let path = match omsi_cfg::env::var("OMSI_GPU_ARRAYS").as_deref() {
            Ok("textures") => ArrayPath::VertexTextures,
            Ok("nostorage") => ArrayPath::NoStorage,
            _ if !downlevel.contains(wgpu::DownlevelFlags::FRAGMENT_STORAGE) || storage < 2 => {
                ArrayPath::NoStorage
            }
            _ if !downlevel.contains(wgpu::DownlevelFlags::VERTEX_STORAGE) || storage < 3 => {
                ArrayPath::VertexTextures
            }
            _ => ArrayPath::Storage,
        };
        ARRAY_PATH.store(path as u8, std::sync::atomic::Ordering::Relaxed);
        match path {
            ArrayPath::Storage => {}
            ArrayPath::VertexTextures => log::warn!("{}: no storage buffers in vertex shaders; the scene's arrays are read from textures", info.name),
            ArrayPath::NoStorage => log::warn!("{}: no storage buffers; the scene's arrays are read from textures and the lamps light no pixels of their own", info.name),
        }
        log::info!("opening graphics device: {} ({:?}, vendor {:#06x}, device {:#06x}), features {:?}, max buffer {} MB, max storage binding {} MB", info.name, info.backend, info.vendor, info.device, required_features, limits.max_buffer_size / 1_000_000, limits.max_storage_buffer_binding_size as u64 / 1_000_000);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("omsi"),
                required_features,
                required_limits: limits,
                // (a card of up to 4 GB gets the allocator's small blocks: the large ones
                // left hundreds of MB reserved and unused, and 2 GB cards lost the device to
                // "Out of memory" in the first frames with the textures well under budget,
                // #332, #295)
                memory_hints: if weak || modest || vram.is_some_and(|v| v <= 4200) {
                    wgpu::MemoryHints::MemoryUsage
                } else {
                    wgpu::MemoryHints::Performance
                },
                // (the ray queries are still an experimental feature of wgpu)
                experimental_features: if ray_query {
                    unsafe { wgpu::ExperimentalFeatures::enabled() }
                } else {
                    wgpu::ExperimentalFeatures::disabled()
                },
                ..Default::default()
            })
            .await
            .context("request_device")?;
        log::info!("graphics device opened; compiling renderer pipelines");
        // the same choice wgpu-core makes when it validates a texture or a pipeline
        let adapter_table = device
            .features()
            .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
            || !adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT);
        let flags_of = |f: wgpu::TextureFormat| {
            if adapter_table {
                adapter.get_texture_format_features(f).flags
            } else {
                f.guaranteed_format_features(device.features()).flags
            }
        };
        let supported = |n: u32| targets.iter().all(|&f| takes(flags_of(f), f, n));
        let msaa = [wanted, 8, 4, 2, 1]
            .into_iter()
            .filter(|&n| n <= wanted)
            .find(|&n| supported(n))
            .unwrap_or(1);
        if msaa != wanted {
            log::warn!("{}x MSAA is not supported by {} (the device takes {:?} samples for {:?}, {:?} for the HDR target, {:?} for depth); using {}x", wanted, info.name, flags_of(format).supported_sample_counts(), format, flags_of(targets[1]).supported_sample_counts(), flags_of(DEPTH_FORMAT).supported_sample_counts(), msaa);
        }
        let options = RenderOptions {
            msaa,
            shadow_size,
            // (16x was held at 8x once, for lane-line dashes far down a road seen to alias
            // into two streaks running apart like an arrow; Spandau's Heerstrasse at 8x and
            // 16x showed none of it - 16x kept the far dashes narrow where 8x smeared them
            // sideways - so 16x is the player's choice again, 8x the default)
            anisotropy: options.anisotropy.clamp(1, 16),
            ray_tracing: ray_query,
            ..options
        };
        RT_BUFFERS.store(ray_query, std::sync::atomic::Ordering::Relaxed);
        let bc = device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let compress = bc
            && options.compress_textures
            && omsi_cfg::env::var_os("OMSI_NO_TEXCOMPRESS").is_none();
        omsi_texture::set_gpu_options(omsi_texture::GpuOptions { bc, compress });
        log::info!("renderer: {} ({:?}), {:?}, {}x MSAA{}, anisotropy {}, shadow map {}, SSAO {}, render scale {}, textures {}", info.name, info.backend, format, options.msaa, if adapter_table { " (adapter format table)" } else { "" }, options.anisotropy, options.shadow_size, options.ssao, if options.render_scale > 0.0 { format!("{:.2}", options.render_scale.clamp(0.5, 1.0)) } else { "auto".to_string() }, match (bc, compress) { (false, _) => "RGBA (no BC on this device)", (true, false) => "DXT as blocks, others RGBA", (true, true) => "DXT as blocks, others compressed where close" });
        // Every pipeline is built under an error scope of every kind and the renderer built
        // again with less where one fails: without multisampling (a driver whose table
        // promises more than it takes), then without the pipelines a picture can do without
        // (`basic_pipelines`), then both. A phone driver whose shader compiler gives up on a
        // pipeline reports it as an *internal* error, not a validation error: caught as
        // validation errors only, it went by, the pipeline stayed invalid and the first frame
        // stopped on "RenderPipeline with 'omsi' label is invalid" (#1663, #1664 - Adreno
        // 740/830 since 0.2.0), and the retry without multisampling was not checked at all.
        let mesh_pages = adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::BASE_VERTEX);
        let name = format!("{} ({:?})", info.name, info.backend);
        // As few builds as can be: each one compiles every shader again, and four of them in
        // a row - a phone whose driver failed the first went through them all - kept the
        // launcher from opening for a minute until Android closed it (#1708, since 0.2.4).
        // What worked on this adapter before is remembered (`fallback_store`) and tried first;
        // after a failure the one most likely to work (no multisampling, basic pipelines).
        let remembered = fallback_load(&name);
        let mut attempts: Vec<(u32, bool)> = match remembered {
            Some((m, b)) => vec![(m.min(options.msaa).max(1), b), (1, true)],
            // (a phone starts with the basic set: the full one failed on Adreno and the
            // second build after it took long enough for Android to close the app)
            None if cfg!(any(target_os = "android", target_os = "ios")) => {
                vec![(options.msaa, true), (1, true)]
            }
            None => vec![(options.msaa, false), (1, false), (1, true)],
        };
        attempts.dedup();
        let mut made: Result<Renderer> = Err(anyhow!("renderer pipelines: not built"));
        let mut tried: Vec<String> = Vec::new();
        for (msaa, basic) in attempts {
            if tried.iter().any(|t| t == &format!("{msaa}{basic}")) || (!basic && basic_pipelines())
            {
                continue;
            }
            tried.push(format!("{msaa}{basic}"));
            if basic {
                BASIC_PIPELINES.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
            let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
            let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let renderer = Self::build(
                device.clone(),
                queue.clone(),
                name.clone(),
                format,
                RenderOptions { msaa, ..options },
            );
            // (test hook: OMSI_FAKE_GPU_ERROR=pipeline fails a pipeline until the basic ones)
            if !basic && omsi_cfg::env::var("OMSI_FAKE_GPU_ERROR").as_deref() == Ok("pipeline") {
                let _ = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("test"),
                    source: wgpu::ShaderSource::Wgsl("fn broken( {".into()),
                });
            }
            let errors = [
                validation.pop().await,
                memory.pop().await,
                internal.pop().await,
            ];
            match errors.iter().flatten().next() {
                None => {
                    if msaa != options.msaa || basic {
                        log::warn!(
                            "{}: drawing with {}x MSAA{}",
                            info.name,
                            msaa,
                            if basic {
                                ", without the snowfall, the lamps in the fog and the street lamps' shadows"
                            } else {
                                ""
                            }
                        );
                    }
                    // (a fallback that worked is the first try next time; the full set
                    // working again forgets it)
                    if remembered != Some((msaa, basic))
                        && (msaa != options.msaa || basic || remembered.is_some())
                    {
                        fallback_store(
                            &name,
                            (msaa != options.msaa || basic).then_some((msaa, basic)),
                        );
                    }
                    made = Ok(Renderer {
                        mesh_pages,
                        ..renderer
                    });
                    break;
                }
                Some(e) => {
                    log::error!(
                        "{}: the renderer's pipelines failed ({}x MSAA{}): {}",
                        info.name,
                        msaa,
                        if basic { ", basic pipelines" } else { "" },
                        gpu_error_text(e)
                    );
                    made = Err(anyhow!("renderer pipelines: {}", gpu_error_text(e)));
                    drop(renderer);
                }
            }
        }
        // A driver whose shader compiler fails on a pipeline answers "out of memory" or an
        // unknown error, and wgpu takes that as the device lost: on phones (Adreno, Mali)
        // one of 0.2's new pipelines did so, the renderer was made all the same and every
        // frame came out black. The device is opened again and the pipelines made without
        // the ones a picture can do without (`basic_pipelines`); lost even so, the next
        // graphics interface is tried.
        // (test hook: OMSI_FAKE_GPU_ERROR=lost-build loses the first device so)
        if let (Ok(r), Ok("lost-build")) = (
            made.as_ref(),
            omsi_cfg::env::var("OMSI_FAKE_GPU_ERROR").as_deref(),
        ) {
            if !basic_pipelines() {
                *r.device_lost.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some("test (OMSI_FAKE_GPU_ERROR=lost-build)".into());
            }
        }
        if let Some(why) = made.as_ref().ok().and_then(|r| r.device_lost()) {
            drop(made);
            if basic_pipelines() {
                return Err(anyhow!(
                    "the graphics device was lost while the pipelines were made: {why}"
                ));
            }
            log::warn!("{}: the graphics device was lost while the pipelines were made ({why}); opening it again without the snowfall, the lamps in the fog and the street lamps' shadows", info.name);
            BASIC_PIPELINES.store(true, std::sync::atomic::Ordering::Relaxed);
            // (and so from the start next time, see `fallback_load`)
            fallback_store(&name, Some((1, true)));
            return Box::pin(Self::new_on(adapter, surface, asked_format, asked_options)).await;
        }
        made
    }

    /// Whether the card ran out of memory since the last call (the game then keeps fewer
    /// textures, before the driver gives up the device).
    pub fn take_out_of_memory(&self) -> bool {
        self.out_of_memory
            .swap(false, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn device_lost(&self) -> Option<String> {
        self.device_lost
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

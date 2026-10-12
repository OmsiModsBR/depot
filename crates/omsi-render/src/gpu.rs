//! The adapter and its compatibility paths: video memory, the OpenGL backend's limits,
//! and how the scene's per-draw arrays reach the shaders (storage buffers or textures).

use super::*;

/// Texture memory (MB) the adapter is taken to have room for (0 = no adapter yet), see
/// `Renderer::new`.
pub static ADAPTER_TEXTURE_MB: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The discrete card's own memory (MB) where the system tells it (0 = not known, or not a
/// discrete card), see `Renderer::new`.
pub static ADAPTER_VRAM_MB: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The device runs on OpenGL (set in `Renderer::new`).
pub(crate) static GL_BACKEND: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Whether the device draws on OpenGL (known once a renderer is made).
pub fn gl_backend() -> bool {
    GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed)
}

/// How the per-draw arrays (the model matrices, the instance parameters, the draw list) and
/// the point lights reach the scene shader on this device (set in `Renderer::new_on`).
/// Older OpenGL chips cannot read a storage buffer in a vertex shader - an Intel HD 2500, a
/// Mali on GLES ("Downlevel flags VERTEX_STORAGE are required but not supported", the
/// renderer was never made, #770, #316) - or have no storage buffers at all (OpenGL below
/// 4.3, GLES 3.0): for them the arrays are textures the vertex shader reads texel by texel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ArrayPath {
    /// Storage buffers everywhere (every Metal, Vulkan and DirectX 12 device).
    Storage,
    /// The vertex shader's arrays are textures; the lights stay storage buffers.
    VertexTextures,
    /// No storage buffers at all: the arrays are textures, and the point lights (the street
    /// lamps, headlights and interior lamps lit per pixel) are left out - the sixteen
    /// textures a fragment shader may read here are all taken.
    NoStorage,
}

pub(crate) static ARRAY_PATH: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub(crate) fn array_path() -> ArrayPath {
    match ARRAY_PATH.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ArrayPath::VertexTextures,
        2 => ArrayPath::NoStorage,
        _ => ArrayPath::Storage,
    }
}

/// Texels per row of an array kept as a texture (within every device's 2048).
pub(crate) const ARRAY_TEX_WIDTH: u32 = 2048;

/// A read-only array of the scene shader: a storage buffer, or where the device cannot
/// read one (see `ArrayPath`) a texture of one value per texel - four floats
/// (`Rgba32Float`, 16 bytes) or one `u32` (`R32Uint`) - in rows of `ARRAY_TEX_WIDTH`.
pub(crate) enum GpuArray {
    Buffer(wgpu::Buffer),
    Texture {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        texel: u32,
    },
    /// Not bound on this device (the lights under `ArrayPath::NoStorage`): takes any write.
    Unused,
}

impl GpuArray {
    /// An array of `size` bytes of `texel`-byte values (16 or 4), a texture where the
    /// vertex shader reads it on a device without vertex storage (`vertex`), or without any.
    pub(crate) fn new(
        device: &wgpu::Device,
        label: &str,
        size: u64,
        texel: u32,
        vertex: bool,
    ) -> GpuArray {
        let path = array_path();
        if path == ArrayPath::Storage || (!vertex && path == ArrayPath::VertexTextures) {
            return GpuArray::Buffer(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16).next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if !vertex {
            return GpuArray::Unused;
        }
        let texels = size.div_ceil(texel as u64).max(1);
        let max_rows = device.limits().max_texture_dimension_2d;
        let rows = (texels.div_ceil(ARRAY_TEX_WIDTH as u64) as u32).clamp(1, max_rows);
        if rows == max_rows {
            log::warn!("{label}: {texels} values do not fit a texture of {ARRAY_TEX_WIDTH} x {max_rows}; the rest are not drawn");
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: ARRAY_TEX_WIDTH,
                height: rows,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if texel == 16 {
                wgpu::TextureFormat::Rgba32Float
            } else {
                wgpu::TextureFormat::R32Uint
            },
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        GpuArray::Texture {
            texture,
            view,
            texel,
        }
    }

    /// Its room in bytes.
    pub(crate) fn size(&self) -> u64 {
        match self {
            GpuArray::Buffer(b) => b.size(),
            GpuArray::Texture { texture, texel, .. } => {
                texture.width() as u64 * texture.height() as u64 * *texel as u64
            }
            GpuArray::Unused => u64::MAX,
        }
    }

    /// Bytes it takes on the GPU (for the memory summary).
    pub(crate) fn gpu_bytes(&self) -> u64 {
        match self {
            GpuArray::Unused => 0,
            a => a.size(),
        }
    }

    /// Write `data` at byte `offset` (both whole values); what runs past its end is dropped.
    pub(crate) fn write(&self, queue: &wgpu::Queue, offset: u64, data: &[u8]) {
        match self {
            GpuArray::Buffer(b) => queue.write_buffer(b, offset, data),
            GpuArray::Texture { texture, texel, .. } => {
                let t = *texel as usize;
                let total = texture.width() as u64 * texture.height() as u64;
                let mut data = data;
                for (x, y, width, rows) in
                    array_tex_spans(offset / t as u64, (data.len() / t) as u64, total)
                {
                    let bytes = (width * rows) as usize * t;
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d { x, y, z: 0 },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &data[..bytes],
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(width * t as u32),
                            rows_per_image: None,
                        },
                        wgpu::Extent3d {
                            width,
                            height: rows,
                            depth_or_array_layers: 1,
                        },
                    );
                    data = &data[bytes..];
                }
            }
            GpuArray::Unused => {}
        }
    }

    pub(crate) fn binding(&self) -> wgpu::BindingResource<'_> {
        match self {
            GpuArray::Buffer(b) => b.as_entire_binding(),
            GpuArray::Texture { view, .. } => wgpu::BindingResource::TextureView(view),
            GpuArray::Unused => unreachable!("an unused array is not bound"),
        }
    }
}

/// The rectangles (x, y, width, rows) of an array texture `ARRAY_TEX_WIDTH` wide and
/// `total` texels big that `n` values from value `at` on fill, in order: the rest of a row,
/// whole rows, the start of the last row. What runs past the end is left out.
pub(crate) fn array_tex_spans(mut at: u64, n: u64, total: u64) -> Vec<(u32, u32, u32, u32)> {
    let w = ARRAY_TEX_WIDTH as u64;
    let mut left = n.min(total.saturating_sub(at));
    let mut out = Vec::new();
    while left > 0 {
        let (x, y) = (at % w, at / w);
        let (width, rows) = if x == 0 && left >= w {
            (w, left / w)
        } else {
            ((w - x).min(left), 1)
        };
        out.push((x as u32, y as u32, width as u32, rows as u32));
        at += width * rows;
        left -= width * rows;
    }
    out
}

/// The bind group layout entry of a scene array read in `stage` (see `GpuArray`):
/// `float4` for vec4 values, else u32.
pub(crate) fn array_layout_entry(
    binding: u32,
    stage: wgpu::ShaderStages,
    float4: bool,
) -> wgpu::BindGroupLayoutEntry {
    array_layout_entry_on(array_path(), binding, stage, float4)
}

/// The same on a device whose arrays take `path`.
pub(crate) fn array_layout_entry_on(
    path: ArrayPath,
    binding: u32,
    stage: wgpu::ShaderStages,
    float4: bool,
) -> wgpu::BindGroupLayoutEntry {
    let texture = match path {
        ArrayPath::Storage => false,
        ArrayPath::VertexTextures => stage.contains(wgpu::ShaderStages::VERTEX),
        ArrayPath::NoStorage => true,
    };
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: stage,
        ty: if texture {
            wgpu::BindingType::Texture {
                sample_type: if float4 {
                    wgpu::TextureSampleType::Float { filterable: false }
                } else {
                    wgpu::TextureSampleType::Uint
                },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            }
        } else {
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            }
        },
        count: None,
    }
}

/// wgpu's OpenGL backend has sixteen texture units for a pipeline - for all its groups and
/// both its shaders together (wgpu-hal's `MAX_TEXTURE_SLOTS`, whatever the chip has) - while
/// it lets through sixteen for each shader alone. With the per-draw arrays as textures (see
/// `ArrayPath`) the scene's camera and material groups held nineteen, and the renderer went
/// down making its first pipeline ("index out of bounds: the len is 16 but the index is 16"
/// in wgpu-hal's gles/device.rs, on the phones of #1133, #1154, #1162, #1176: OpenGL was
/// passed over for Vulkan and the panic reported as the game's end). There the camera group
/// leaves out the enhanced path's three textures, and the enhanced path is not made
/// (vanilla and vanilla+ draw as everywhere). OMSI_GL_TEXTURE_UNITS=1 takes this layout on
/// any device (with OMSI_GPU_ARRAYS, to try it without such a chip).
pub(crate) fn sixteen_texture_units() -> bool {
    array_path() != ArrayPath::Storage
        && (gl_backend() || omsi_cfg::env::var_os("OMSI_GL_TEXTURE_UNITS").is_some())
}

/// Wait until the GPU has done `submission` (None: everything submitted so far).
///
/// On OpenGL wgpu holds the one GL context for the whole of a wait, and every other thread
/// that wants it meanwhile (a worker making a bus's textures) gives up after
/// a second with a panic - "Could not lock adapter context. This is most-likely a deadlock."
/// (wgpu-hal's WGL lock; #843: a slow chip took longer than that for a frame). There the
/// wait is made of short ones, and the context is free between them.
pub fn wait_gpu(
    device: &wgpu::Device,
    submission: Option<wgpu::SubmissionIndex>,
) -> Result<(), wgpu::PollError> {
    if !gl_backend() {
        return device
            .poll(wgpu::PollType::Wait {
                submission_index: submission,
                timeout: None,
            })
            .map(|_| ());
    }
    loop {
        match device.poll(wgpu::PollType::Wait {
            submission_index: submission.clone(),
            timeout: Some(GL_WAIT_SLICE),
        }) {
            Err(wgpu::PollError::Timeout) => std::thread::yield_now(),
            r => return r.map(|_| ()),
        }
    }
}

/// The longest a single wait for the GPU holds the GL context (see [`wait_gpu`]).
pub(crate) const GL_WAIT_SLICE: std::time::Duration = std::time::Duration::from_millis(20);

/// On OpenGL, the GPU work of worker threads (textures and meshes of a bus made while the
/// world loads) goes one thread at a time: a dozen of them queueing for the GL context left
/// the last one waiting past wgpu's one second (#843). Elsewhere the device takes them all.
pub(crate) fn gl_worker_turn() -> Option<std::sync::MutexGuard<'static, ()>> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    gl_backend().then(|| TURN.lock().unwrap_or_else(|e| e.into_inner()))
}

/// The card's own memory in MB where the system tells it: Windows, through DXGI, for
/// whichever backend draws; Linux, through the DRM driver's sysfs (amdgpu; not
/// NVIDIA's own driver, whose memory [`vulkan_vram_mb`] reads from Vulkan instead).
pub(crate) fn dedicated_vram_mb(info: &wgpu::AdapterInfo) -> Option<u64> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
        let f: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut i = 0;
        while let Ok(a) = f.EnumAdapters1(i) {
            i += 1;
            let Ok(d) = a.GetDesc1() else { continue };
            if d.VendorId == info.vendor && d.DeviceId == info.device {
                return Some(d.DedicatedVideoMemory as u64 >> 20);
            }
        }
        None
    }
    #[cfg(target_os = "linux")]
    {
        let hex = |p: std::path::PathBuf| {
            let t = std::fs::read_to_string(p).ok()?;
            u32::from_str_radix(t.trim().trim_start_matches("0x"), 16).ok()
        };
        for e in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
            // (card0, card1, ...; not their connectors, card1-DP-1)
            let name = e.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("card") || name.contains('-') {
                continue;
            }
            let dev = e.path().join("device");
            if hex(dev.join("vendor")) != Some(info.vendor)
                || hex(dev.join("device")) != Some(info.device)
            {
                continue;
            }
            let bytes = std::fs::read_to_string(dev.join("mem_info_vram_total"))
                .ok()
                .and_then(|t| t.trim().parse::<u64>().ok());
            if let Some(b) = bytes.filter(|b| *b > 0) {
                return Some(b >> 20);
            }
        }
        None
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = info;
        None
    }
}

/// The largest device-local memory heap of a Vulkan adapter (MB).
#[cfg(target_os = "linux")]
pub(crate) fn vulkan_vram_mb(adapter: &wgpu::Adapter) -> Option<u64> {
    // SAFETY: the adapter outlives the borrow, and only its memory properties are read
    let hal = unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }?;
    // SAFETY: the physical device belongs to this instance
    let props = unsafe {
        hal.shared_instance()
            .raw_instance()
            .get_physical_device_memory_properties(hal.raw_physical_device())
    };
    props.memory_heaps[..props.memory_heap_count as usize]
        .iter()
        .filter(|h| h.flags.contains(ash::vk::MemoryHeapFlags::DEVICE_LOCAL))
        .map(|h| h.size >> 20)
        .max()
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn vulkan_vram_mb(_adapter: &wgpu::Adapter) -> Option<u64> {
    None
}

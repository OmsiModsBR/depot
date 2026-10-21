//! Textures: the GPU texture slot, preparing and fitting images, uploads with their mip
//! chains, and replacing or shrinking textures already in the scene.

use crate::gpu::{
    fullscreen_pipeline, gl_worker_turn, mip_levels, sampler_entry, target_2d, texture_entry,
};

/// A texture on the GPU, made on a worker thread; [`Renderer::add_prepared_texture`] puts it
/// into a scene.
pub struct PreparedTexture(
    pub(crate) GpuTexture,
    /// The levels as uploaded, kept while a renderer mirrors its assets.
    pub(crate) Option<Box<omsi_texture::TextureData>>,
);

impl PreparedTexture {
    pub fn bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// Make a texture that carries its levels (blocks, or RGBA that wants no GPU-made chain) on
/// any thread; None for anything else (an RGBA picture whose chain the GPU makes, a block
/// format the device does not take).
pub fn prepare_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &omsi_texture::TextureData,
) -> Option<PreparedTexture> {
    if let Some(small) = fit_texture(data, device.limits().max_texture_dimension_2d) {
        return prepare_texture(device, queue, &small);
    }
    let _turn = gl_worker_turn();
    use omsi_texture::PixelFormat;
    let format = match data.format {
        PixelFormat::Rgba8 => wgpu::TextureFormat::Rgba8UnormSrgb,
        PixelFormat::Bc1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        PixelFormat::Bc2 => wgpu::TextureFormat::Bc2RgbaUnormSrgb,
        PixelFormat::Bc3 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
    };
    let (w, h) = (data.width.max(1), data.height.max(1));
    if data.levels.is_empty()
        || (data.format == PixelFormat::Rgba8
            && data.levels.len() == 1
            && data.gpu_mips
            && w > 1
            && h > 1)
    {
        return None;
    }
    if data.format.is_compressed()
        && (!device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
            || w % 4 != 0
            || h % 4 != 0)
    {
        return None;
    }
    let levels = data.levels.len() as u32;
    let size = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let (bw, bh) = format.block_dimensions();
    let block = format.block_copy_size(None).unwrap_or(4);
    for (l, bytes) in data.levels.iter().enumerate() {
        let (lw, lh) = ((w >> l).max(1), (h >> l).max(1));
        // compressed levels are written whole blocks at a time, also below 4x4
        let phys = wgpu::Extent3d {
            width: lw.div_ceil(bw) * bw,
            height: lh.div_ceil(bh) * bh,
            depth_or_array_layers: 1,
        };
        let need = ((phys.width / bw) * (phys.height / bh) * block) as usize;
        if bytes.len() < need {
            log::warn!(
                "texture level {l} of {w}x{h} {:?} has {} bytes, not {need}",
                data.format,
                bytes.len()
            );
            break;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: l as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes[..need],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((phys.width / bw) * block),
                rows_per_image: Some(phys.height / bh),
            },
            phys,
        );
    }
    Some(PreparedTexture(
        GpuTexture::new(texture, (w, h), texture_bytes(format, w, h, levels)),
        super::keep_cpu_copies().then(|| Box::new(data.clone())),
    ))
}

pub(crate) fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    img: &omsi_texture::Image,
    mipmaps: bool,
) -> GpuTexture {
    let mip_count = if mipmaps {
        mip_levels(img.width, img.height)
    } else {
        1
    };
    let size = wgpu::Extent3d {
        width: img.width,
        height: img.height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    // CPU box-filter mip chain
    let mut level: Vec<u8> = img.rgba.clone();
    let (mut w, mut h) = (img.width, img.height);
    for mip in 0..mip_count {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        if mip + 1 == mip_count {
            break;
        }
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut sum = 0u32;
                    let mut n = 0;
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let sx = (x * 2 + dx).min(w - 1);
                            let sy = (y * 2 + dy).min(h - 1);
                            sum += level[((sy * w + sx) * 4 + c) as usize] as u32;
                            n += 1;
                        }
                    }
                    next[((y * nw + x) * 4 + c) as usize] = (sum / n) as u8;
                }
            }
        }
        level = next;
        w = nw;
        h = nh;
    }
    let view = texture.create_view(&Default::default());
    GpuTexture {
        texture,
        view,
        size: (img.width, img.height),
        bytes: texture_bytes(
            wgpu::TextureFormat::Rgba8UnormSrgb,
            img.width,
            img.height,
            mip_count,
        ),
        gen: next_gen(),
    }
}

/// A texture bigger than the graphics chip takes (`max` texels a side - 16384 on most, 2048
/// or 4096 on older ones) made to fit: its smaller levels when it has them, else the picture
/// halved until it fits. None when it fits as it is. A too big texture used to be a device
/// error, and the material that used it one too.
pub fn fit_texture(
    data: &omsi_texture::TextureData,
    max: u32,
) -> Option<omsi_texture::TextureData> {
    use omsi_texture::PixelFormat;
    let (w, h) = (data.width.max(1), data.height.max(1));
    if w <= max && h <= max {
        return None;
    }
    let mut k = 0u32;
    while (w >> k).max(1) > max || (h >> k).max(1) > max {
        k += 1;
    }
    if (k as usize) < data.levels.len() {
        return Some(omsi_texture::TextureData {
            width: (w >> k).max(1),
            height: (h >> k).max(1),
            levels: data.levels[k as usize..].to_vec(),
            ..data.clone()
        });
    }
    // one level only: decode it and halve it
    let mut rgba = match (data.format, data.levels.first()) {
        (PixelFormat::Rgba8, Some(l)) => l.clone(),
        (f, Some(l)) => omsi_texture::bc::decode(
            l,
            w,
            h,
            match f {
                PixelFormat::Bc1 => omsi_texture::bc::Bc::Bc1 { punch: true },
                PixelFormat::Bc2 => omsi_texture::bc::Bc::Bc2,
                _ => omsi_texture::bc::Bc::Bc3,
            },
        ),
        _ => return None,
    };
    let (mut cw, mut ch) = (w, h);
    for _ in 0..k {
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let at = |xx: u32, yy: u32| {
                        rgba[((yy.min(ch - 1) * cw + xx.min(cw - 1)) * 4 + c) as usize] as u32
                    };
                    let v = at(2 * x, 2 * y)
                        + at(2 * x + 1, 2 * y)
                        + at(2 * x, 2 * y + 1)
                        + at(2 * x + 1, 2 * y + 1);
                    next[((y * nw + x) * 4 + c) as usize] = (v / 4) as u8;
                }
            }
        }
        rgba = next;
        (cw, ch) = (nw, nh);
    }
    Some(omsi_texture::TextureData {
        width: cw,
        height: ch,
        format: PixelFormat::Rgba8,
        levels: vec![rgba],
        has_alpha: data.has_alpha,
        gpu_mips: true,
    })
}

/// A size halved (both sides, as `fit_texture` halves a picture) until neither passes `max`.
pub(crate) fn fit_size(w: u32, h: u32, max: u32) -> (u32, u32) {
    let (mut w, mut h) = (w, h);
    while w > max || h > max {
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
    }
    (w, h)
}

/// An RGBA picture of the game's own (a script's or a sign's text, an HTML display) larger
/// than the graphics chip takes, halved until it fits (None when it fits as it is). Made at
/// its full size, the texture was a device error, and so was every frame's write into it.
pub(crate) fn fit_image(img: &omsi_texture::Image, max: u32) -> Option<omsi_texture::Image> {
    if img.width <= max && img.height <= max {
        return None;
    }
    let data = omsi_texture::TextureData {
        width: img.width,
        height: img.height,
        format: omsi_texture::PixelFormat::Rgba8,
        levels: vec![img.rgba.clone()],
        has_alpha: img.has_alpha,
        gpu_mips: true,
    };
    let small = fit_texture(&data, max)?;
    Some(omsi_texture::Image {
        width: small.width,
        height: small.height,
        rgba: small.levels.into_iter().next().unwrap_or_default(),
        has_alpha: img.has_alpha,
    })
}

#[cfg(test)]
pub(crate) mod fit_tests {
    #[test]
    fn a_big_picture_is_halved_until_it_fits() {
        let data = omsi_texture::TextureData {
            width: 8,
            height: 4,
            format: omsi_texture::PixelFormat::Rgba8,
            levels: vec![vec![200; 8 * 4 * 4]],
            has_alpha: false,
            gpu_mips: true,
        };
        let small = super::fit_texture(&data, 2).unwrap();
        assert_eq!((small.width, small.height), (2, 1));
        assert_eq!(small.levels[0].len(), 2 * 4);
        assert!(super::fit_texture(&data, 8).is_none());
        // a picture of the game's own, and a size, the same way
        let img = omsi_texture::Image {
            width: 5000,
            height: 300,
            rgba: vec![9; 5000 * 300 * 4],
            has_alpha: true,
        };
        let small = super::fit_image(&img, 2048).unwrap();
        assert_eq!((small.width, small.height), (1250, 75));
        assert_eq!(small.rgba.len(), 1250 * 75 * 4);
        assert_eq!(super::fit_size(5000, 300, 2048), (1250, 75));
        assert_eq!(super::fit_size(2048, 16, 2048), (2048, 16));
        assert!(super::fit_image(&small, 2048).is_none());
    }
}

/// An RGBA picture borrowed for an upload.
pub(crate) struct RgbaRef<'a> {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: &'a [u8],
}

pub struct GpuTexture {
    #[allow(dead_code)]
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) size: (u32, u32),
    /// Bytes the texture takes on the GPU, all levels (0 for a freed slot's placeholder).
    pub(crate) bytes: u64,
    /// Which texture this is (a slot is reused and a texture replaced): part of the
    /// key of a material bind group made this frame (see `Scene::bind_groups`).
    pub(crate) gen: u64,
}

pub(crate) static TEXTURE_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(crate) fn next_gen() -> u64 {
    TEXTURE_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl GpuTexture {
    /// A slot showing `view` (a picture the renderer draws: the one behind the rain on the
    /// glass), with `texture` a small stand-in the slot's bookkeeping asks about.
    pub(crate) fn showing(
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        size: (u32, u32),
    ) -> GpuTexture {
        GpuTexture {
            texture,
            view,
            size,
            bytes: 0,
            gen: next_gen(),
        }
    }

    pub(crate) fn new(texture: wgpu::Texture, size: (u32, u32), bytes: u64) -> GpuTexture {
        let view = texture.create_view(&Default::default());
        GpuTexture {
            texture,
            view,
            size,
            bytes,
            gen: next_gen(),
        }
    }
}

/// Bytes of a texture of `format` with `levels` mip levels.
pub(crate) fn texture_bytes(format: wgpu::TextureFormat, w: u32, h: u32, levels: u32) -> u64 {
    let (bw, bh) = format.block_dimensions();
    let block = format.block_copy_size(None).unwrap_or(4) as u64;
    (0..levels)
        .map(|l| ((w >> l).max(1).div_ceil(bw) * (h >> l).max(1).div_ceil(bh)) as u64 * block)
        .sum()
}

/// Texture `t` without its `n` finest mip levels: the coarser levels copied into a smaller
/// texture on the GPU. None when it cannot shrink so far (a block format needs sides that
/// are multiples of four).
pub(crate) fn drop_top_levels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    t: &GpuTexture,
    n: u32,
) -> Option<GpuTexture> {
    let (w, h) = t.size;
    let levels = t.texture.mip_level_count();
    let format = t.texture.format();
    if n == 0 || n >= levels || !t.texture.usage().contains(wgpu::TextureUsages::COPY_SRC) {
        return None;
    }
    let (nw, nh) = ((w >> n).max(1), (h >> n).max(1));
    let (bw, bh) = format.block_dimensions();
    if nw % bw != 0 || nh % bh != 0 {
        return None;
    }
    let new_levels = levels - n;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: nw,
            height: nh,
            depth_or_array_layers: 1,
        },
        mip_level_count: new_levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("drop levels"),
    });
    for l in 0..new_levels {
        let (lw, lh) = ((nw >> l).max(1), (nh >> l).max(1));
        let size = wgpu::Extent3d {
            width: lw.div_ceil(bw) * bw,
            height: lh.div_ceil(bh) * bh,
            depth_or_array_layers: 1,
        };
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &t.texture,
                mip_level: l + n,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: l,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            size,
        );
    }
    queue.submit([encoder.finish()]);
    Some(GpuTexture::new(
        texture,
        (nw, nh),
        texture_bytes(format, nw, nh, new_levels),
    ))
}

/// How fast textures go up to the GPU, in MB/s: four 1024x1024 RGBA uploads timed.
pub(crate) fn upload_speed_mb_s(device: &wgpu::Device, queue: &wgpu::Queue) -> f64 {
    let n = 1024u32;
    let size = wgpu::Extent3d {
        width: n,
        height: n,
        depth_or_array_layers: 1,
    };
    let texture = target_2d(
        device,
        "upload check",
        size,
        1,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::COPY_DST,
    );
    let data = vec![0u8; (n * n * 4) as usize];
    let rounds = 4;
    let t = std::time::Instant::now();
    for _ in 0..rounds {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(n * 4),
                rows_per_image: Some(n),
            },
            size,
        );
    }
    let secs = t.elapsed().as_secs_f64().max(1e-6);
    queue.submit([]);
    texture.destroy();
    (data.len() * rounds) as f64 / 1e6 / secs
}

/// The mip map generator (mip.wgsl): its bind group layout, pipeline and sampler.
pub(crate) fn mip_pipeline(
    device: &wgpu::Device,
) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline, wgpu::Sampler) {
    // --- mipmaps on the GPU: the CPU box filter took up to a second per bus spawn
    log::info!("renderer: compiling the mip maps shaders");
    let mip_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mip"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/assets/mip.wgsl").into()),
    });
    let mip_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mip"),
        entries: &[texture_entry(0), sampler_entry(1)],
    });
    let mip_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mip"),
        bind_group_layouts: &[Some(&mip_layout)],
        immediate_size: 0,
    });
    let mip_pipeline = fullscreen_pipeline(
        device,
        "mip",
        &mip_pl,
        &mip_shader,
        "vs_main",
        "fs_main",
        wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        },
    );
    let mip_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    (mip_layout, mip_pipeline, mip_sampler)
}

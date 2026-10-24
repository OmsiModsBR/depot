//! The launcher drawn into a picture, with no window (for screenshots and checks).

use crate::look::scale_for;
use crate::Shell;
use anyhow::{anyhow, Result};
use glam::Vec2;
use omsi_ui::widgets::Ui;
use omsi_ui::{Draw, Gpu};
use std::path::Path;

const LOGO: &[u8] = include_bytes!("../../../assets/logos/wordmark-on-dark.png");

/// The logo is shrunk to this width before it goes to the GPU (about twice what the launcher
/// draws at its widest scale).
const LOGO_WIDTH: u32 = 640;

#[cfg(test)]
mod logo_tests {
    #[test]
    fn the_logo_is_clear_round_its_letters_and_wears_the_accent() {
        let blue = omsi_ui::theme::Accent::Blue.color();
        let img = super::logo_pixels(blue).unwrap();
        // (the wordmark's black background is gone)
        assert_eq!(img.get_pixel(0, 0).0[3], 0);
        let opaque: Vec<_> = img.pixels().filter(|p| p.0[3] == 255).collect();
        assert!(opaque
            .iter()
            .any(|p| p.0[0] > 250 && p.0[1] > 250 && p.0[2] > 250));
        // (no orange is left: the coloured letters are the accent's)
        assert!(!opaque.iter().any(|p| p.0[0] > 200 && p.0[2] < 60));
        assert!(opaque.iter().any(|p| p.0[2] > 230 && p.0[0] < 110));
    }
}

/// The adapter and device, for the picture.
pub fn device(instance: &wgpu::Instance) -> Result<(wgpu::Device, wgpu::Queue)> {
    let (_, device, queue) = adapter_device(instance, None)?;
    Ok((device, queue))
}

/// The adapter and the device made from it (the window needs the adapter too: the surface's
/// capabilities are asked of the adapter the device came from, not of a second one).
pub fn adapter_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: surface,
        ..Default::default()
    }))
    .map_err(|e| anyhow!("no graphics adapter: {e}"))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|e| anyhow!("no graphics device: {e}"))?;
    Ok((adapter, device, queue))
}

/// The logo into `gpu`, in the accent in use: its texture and its height over its width.
pub fn load_logo(
    gpu: &mut Gpu,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<(usize, f32)> {
    let img = logo_pixels(omsi_ui::theme::accent())?;
    let tex = gpu.add_image(device, queue, img.width(), img.height(), img.as_raw());
    Some((tex, img.height() as f32 / img.width() as f32))
}

/// The logo's texture `tex` drawn again in the accent in use (after the player chose another).
pub fn tint_logo(gpu: &mut Gpu, device: &wgpu::Device, queue: &wgpu::Queue, tex: usize) {
    if let Some(img) = logo_pixels(omsi_ui::theme::accent()) {
        gpu.update_image(device, queue, tex, img.width(), img.height(), img.as_raw());
    }
}

/// The wordmark (white "Project", coloured "Depot" on black) on a clear background, its
/// coloured part in `accent`: how bright a pixel is becomes how opaque it is, how coloured it
/// is how much of the accent it takes.
pub fn logo_pixels(accent: omsi_ui::Color) -> Option<image::RgbaImage> {
    // (the picture is far bigger than the screens draw it, and the sampler has no mip levels:
    // shrunk by the sampler alone its edges are jagged, so it is shrunk here with a good
    // filter, before the colours are worked out)
    let src = image::load_from_memory(LOGO).ok()?;
    let h = (src.height() as u64 * LOGO_WIDTH as u64 / src.width().max(1) as u64).max(1) as u32;
    let mut img = src
        .resize_exact(LOGO_WIDTH, h, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let a = accent.0;
    for p in img.pixels_mut() {
        let [r, g, b, alpha] = p.0;
        let mx = r.max(g).max(b) as f32;
        let mn = r.min(g).min(b) as f32;
        if mx <= 0.0 {
            p.0 = [0, 0, 0, 0];
            continue;
        }
        // (the wordmark's orange is about 0.95 saturated)
        let k = ((mx - mn) / mx / 0.9).min(1.0);
        let c = |i: usize| ((1.0 - k + k * a[i]) * 255.0).round() as u8;
        p.0 = [c(0), c(1), c(2), (mx * alpha as f32 / 255.0).round() as u8];
    }
    Some(img)
}

#[allow(clippy::too_many_arguments)]
/// Draw one frame of `shell` at the pixel size `w` x `h` onto `view`.
pub fn draw_into(
    shell: &mut Shell,
    ui: &mut Ui,
    gpu: &mut Gpu,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    (w, h): (u32, u32),
    dt: f32,
) {
    let scale = scale_for(w, h);
    ui.begin(Vec2::new(w as f32, h as f32) / scale, scale, dt);
    shell.draw(ui);
    let (layers, verts, ranges) = ui.finish();
    let draws: Vec<Draw> = ranges
        .iter()
        .enumerate()
        .map(|(k, (r, tex))| Draw {
            buffer: 0,
            range: r.clone(),
            layer: k,
            texture: *tex,
        })
        .collect();
    gpu.upload(device, queue, 0, &verts);
    gpu.upload_atlas(queue, &mut ui.atlas);
    let mut enc = device.create_command_encoder(&Default::default());
    gpu.render(
        device,
        queue,
        &mut enc,
        view,
        (w, h),
        Some(wgpu::Color::BLACK),
        &layers,
        &draws,
    );
    queue.submit([enc.finish()]);
}

/// Write the launcher on `shell`'s page as a PNG of `w` x `h` pixels.
pub fn shot(shell: &mut Shell, (w, h): (u32, u32), out: &Path) -> Result<()> {
    let instance = wgpu::Instance::default();
    let (device, queue) = device(&instance)?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut ui = Ui::new();
    let mut gpu = Gpu::new(&device, format, 4, ui.atlas.size);
    shell.logo = load_logo(&mut gpu, &device, &queue);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("launcher picture"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    // (twice: the first frame fills the glyph atlas)
    for _ in 0..2 {
        draw_into(
            shell,
            &mut ui,
            &mut gpu,
            &device,
            &queue,
            &view,
            (w, h),
            1.0 / 60.0,
        );
    }
    let stride = (w * 4).next_multiple_of(256);
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (stride * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let data = slice.get_mapped_range();
    let mut img = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let i = (y * stride + x * 4) as usize;
            img.put_pixel(x, y, image::Rgb([data[i], data[i + 1], data[i + 2]]));
        }
    }
    img.save(out)?;
    Ok(())
}

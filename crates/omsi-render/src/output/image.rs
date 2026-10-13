//! A frame delivered as an image or into a texture: reading the pixels back.

use crate::gpu::wait_gpu;
use anyhow::anyhow;
use anyhow::{Context, Result};

/// A texture's pixels read back (rows without the copy's padding), the GPU waited for.
pub(crate) fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    aspect: wgpu::TextureAspect,
) -> Result<Vec<u8>> {
    let (width, height) = (texture.width(), texture.height());
    let row_bytes = width
        * texture
            .format()
            .block_copy_size(Some(aspect))
            .context("unsupported readback format")?;
    let bpr = row_bytes.div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (bpr * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bpr),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let idx = queue.submit([enc.finish()]);
    let slice = buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    wait_gpu(device, Some(idx)).map_err(|e| anyhow!("poll: {e:?}"))?;
    rx.recv()
        .context("map")?
        .map_err(|e| anyhow!("map: {e:?}"))?;
    let data = slice.get_mapped_range();
    let mut out = Vec::with_capacity((row_bytes * height) as usize);
    for row in 0..height {
        let start = (row * bpr) as usize;
        out.extend_from_slice(&data[start..start + row_bytes as usize]);
    }
    drop(data);
    buf.unmap();
    Ok(out)
}

//! What the demo examples share: a device, a texture to draw into, and writing it as PNG.
use glam::Vec2;
use omsi_ui::widgets::Ui;
use omsi_ui::{Draw, Gpu};

pub struct Offscreen {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub gpu: Gpu,
    pub ui: Ui,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    pub size: (u32, u32),
}

impl Offscreen {
    pub fn new(size: (u32, u32)) -> Offscreen {
        let instance = wgpu::Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("device");
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
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
        let ui = Ui::new();
        let gpu = Gpu::new(&device, format, 4, ui.atlas.size);
        Offscreen {
            device,
            queue,
            gpu,
            ui,
            target,
            view,
            size,
        }
    }

    /// Draw `body` into the picture (twice: the first frame fills the glyph atlas) and write
    /// it to `out`.
    pub fn shot(&mut self, out: &str, mut body: impl FnMut(&mut Ui)) {
        let (w, h) = self.size;
        for _ in 0..2 {
            self.ui
                .begin(Vec2::new(w as f32, h as f32), 1.0, 1.0 / 60.0);
            body(&mut self.ui);
            let (layers, verts, ranges) = self.ui.finish();
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
            self.gpu.upload(&self.device, &self.queue, 0, &verts);
            self.gpu.upload_atlas(&self.queue, &mut self.ui.atlas);
            let mut enc = self.device.create_command_encoder(&Default::default());
            self.gpu.render(
                &self.device,
                &self.queue,
                &mut enc,
                &self.view,
                (w, h),
                Some(wgpu::Color::BLACK),
                &layers,
                &draws,
            );
            self.queue.submit([enc.finish()]);
        }
        let stride = (w * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (stride * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok();
        let data = buf.slice(..).get_mapped_range();
        let mut img = image::RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let i = (y * stride + x * 4) as usize;
                img.put_pixel(x, y, image::Rgb([data[i], data[i + 1], data[i + 2]]));
            }
        }
        img.save(out).unwrap();
        println!("wrote {out}");
    }
}

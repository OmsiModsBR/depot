//! The loading screen drawn offscreen: with no screenshots (the default background), and
//! with three made-up ones in the middle of a fade: `cargo run -p omsi-ui --example
//! loading_demo -- out_dir`
use glam::Vec2;
use omsi_ui::loading::{self, Card, Slide, Slideshow, View, FADE_SECONDS, HOLD_SECONDS};
use omsi_ui::widgets::Ui;
use omsi_ui::{Draw, Gpu};

/// A made-up screenshot: a vertical blend of two colours, `w` x `h`.
fn picture(w: u32, h: u32, top: [u8; 3], bottom: [u8; 3]) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let t = y as f32 / h as f32;
        for _ in 0..w {
            for k in 0..3 {
                px.push((top[k] as f32 * (1.0 - t) + bottom[k] as f32 * t) as u8);
            }
            px.push(255);
        }
    }
    px
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(".".into());
    std::fs::create_dir_all(&dir).unwrap();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("device");
    let (w, h) = (1672u32, 941u32);
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
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
    let mut ui = Ui::new();
    let mut gpu = Gpu::new(&device, format, 4, ui.atlas.size);
    let colours = [
        ([20, 60, 120], [200, 140, 60]),
        ([30, 100, 60], [220, 220, 120]),
        ([90, 30, 90], [240, 120, 120]),
    ];
    let slides: Vec<Slide> = colours
        .iter()
        .map(|(a, b)| Slide {
            tex: gpu.add_image(&device, &queue, 300, 200, &picture(300, 200, *a, *b)),
            aspect: 1.5,
        })
        .collect();
    for (name, shown, t) in [
        ("default", 0usize, 0.0f32),
        ("slideshow-mid-fade", 3, HOLD_SECONDS + FADE_SECONDS * 0.5),
    ] {
        let mut show = Slideshow::new(shown);
        show.update(t);
        let v = View {
            version: "example version",
            heading: "Example map \u{b7} Line 50 \u{b7} 08:00",
            stage: "Loading map tiles (first area) \u{b7} 62 of 100",
            progress: 0.62,
            logo: None,
            bus: Some(Card {
                title: "Example bus A",
                sub: "Example Maker \u{b7} v1.2 \u{b7} Default paint",
                description:
                    "Author's description, first lines as written by the author (example text).",
            }),
            map: Some(Card {
                title: "Example map",
                sub: "Example City",
                description: "Map description from the map folder (example text, a few lines).",
            }),
            slides: &slides[..shown],
        };
        for _ in 0..2 {
            ui.begin(Vec2::new(w as f32, h as f32), 1.0, 1.0 / 60.0);
            loading::draw(&mut ui, &v, &show);
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
            gpu.upload(&device, &queue, 0, &verts);
            gpu.upload_atlas(&queue, &mut ui.atlas);
            let mut enc = device.create_command_encoder(&Default::default());
            gpu.render(
                &device,
                &queue,
                &mut enc,
                &view,
                (w, h),
                Some(wgpu::Color::BLACK),
                &layers,
                &draws,
            );
            queue.submit([enc.finish()]);
        }
        let stride = (w * 4).div_ceil(256) * 256;
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
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).ok();
        let data = buf.slice(..).get_mapped_range();
        let mut img = image::RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let i = (y * stride + x * 4) as usize;
                img.put_pixel(x, y, image::Rgb([data[i], data[i + 1], data[i + 2]]));
            }
        }
        let out = format!("{dir}/loading-{name}.png");
        img.save(&out).unwrap();
        println!("wrote {out}");
    }
}

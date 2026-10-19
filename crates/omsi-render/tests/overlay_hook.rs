//! The overlay hook: a layer the renderer knows nothing about is drawn over the window's
//! finished picture, at its full size, and taking the hook away puts the picture back.

use glam::DVec3;
use omsi_render::{Camera, Lighting, Renderer};
use std::sync::{Arc, Mutex};

fn camera() -> Camera {
    Camera {
        position: DVec3::new(0.0, 0.0, 2.0),
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 70.0,
        near: 0.1,
        far: 1000.0,
    }
}

#[test]
fn the_hook_draws_over_the_picture_at_full_size_and_can_be_taken_away() {
    // (the game runs on a main thread with an 8 MB stack, see .cargo/config.toml; a test
    // thread has 2 MB, which a frame's setup does not fit on Windows)
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(hook_draws_and_goes_away)
        .unwrap()
        .join()
        .unwrap();
}

fn hook_draws_and_goes_away() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let Ok(mut renderer) = pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        Default::default(),
    )) else {
        eprintln!("no graphics adapter: skipped");
        return;
    };
    let mut scene = renderer.new_scene();
    let (w, h) = (96, 54);
    let before = renderer
        .render_to_image(&mut scene, w, h, &camera(), &Lighting::default())
        .expect("frame");

    let calls: Arc<Mutex<Vec<(u32, u32)>>> = Arc::default();
    let seen = calls.clone();
    renderer.set_overlay_hook(Some(Box::new(move |_, _, encoder, target, size| {
        seen.lock().unwrap().push(size);
        // (a layer that covers the whole picture)
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("test overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.0,
                        g: 0.0,
                        b: 1.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    })));
    let covered = renderer
        .render_to_image(&mut scene, w, h, &camera(), &Lighting::default())
        .expect("frame");
    assert_eq!(*calls.lock().unwrap(), vec![(w, h)]);
    assert!(covered.chunks(4).all(|p| p[..3] == [255, 0, 255]));

    renderer.set_overlay_hook(None);
    let after = renderer
        .render_to_image(&mut scene, w, h, &camera(), &Lighting::default())
        .expect("frame");
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(after, before);
}

//! Tests moved out of `src/api/renderer.rs`.

use omsi_render::testing::internals::*;

fn noop_renderer() -> Renderer {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::NOOP;
    descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
    let instance = wgpu::Instance::new(descriptor);
    pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            shadow_size: 1024,
            ..Default::default()
        },
    ))
    .expect("noop renderer")
}

fn readback(r: &Renderer) -> wgpu::Buffer {
    let buf = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    r.queue.write_buffer(&buf, 0, &[7; 16]);
    r.queue.submit([]);
    buf
}

#[test]
fn a_working_device_is_waited_for_polled_and_read_back() {
    let r = noop_renderer();
    assert_eq!(r.wait_gpu(None), Ok(()));
    assert_eq!(r.poll_gpu(), Ok(()));
    let buf = readback(&r);
    assert_eq!(r.read_buffer(&buf).map(|d| d.len()), Ok(16));
    assert_eq!(r.device_lost(), None);
    assert!(r.close(), "a working device closes in order");
}

#[test]
fn a_lost_device_is_neither_waited_for_nor_polled_nor_read() {
    let r = noop_renderer();
    let buf = readback(&r);
    omsi_render::testing::internals::mark_lost(&r.gpu.device_lost, "Unknown: test");
    assert_eq!(r.device_lost().as_deref(), Some("Unknown: test"));
    assert_eq!(r.wait_gpu(None), Err(GpuWaitError::Lost));
    assert_eq!(r.poll_gpu(), Err(GpuWaitError::Lost));
    assert_eq!(r.read_buffer(&buf), Err(GpuWaitError::Lost));
    // (and letting it go after the loss goes in order too)
    assert!(r.close());
}

#[test]
fn a_device_destroyed_on_purpose_is_not_lost() {
    let r = noop_renderer();
    // (`destroy` is the device let go of on purpose: no loss)
    r.device.destroy();
    let _ = r.device.poll(wgpu::PollType::Poll);
    assert_eq!(r.device_lost(), None);
}

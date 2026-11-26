//! Tests moved out of `src/gpu/context.rs`.

use omsi_render::testing::internals::gpu::context::*;
use omsi_render::testing::internals::gpu::lost::*;
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
    let old = GpuContext::open(&device, format, 4);
    old.out_of_memory.store(true, Ordering::Relaxed);
    mark_lost(&old.device_lost, "Unknown: before the fallback");
    let new = GpuContext::open(&device, format, 1);
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
    let old = GpuContext::open(&device, format, 4);
    let new = GpuContext::open(&device, format, 1);
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
    let ctx = GpuContext::open(&device, wgpu::TextureFormat::Rgba8UnormSrgb, 1);
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

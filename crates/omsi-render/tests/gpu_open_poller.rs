//! Tests moved out of `src/gpu/open.rs`.

use omsi_render::testing::internals::gpu::lost::*;
use omsi_render::testing::internals::gpu::open::*;
use omsi_render::testing::internals::lost::mark_lost;
use std::sync::atomic::{AtomicBool, Ordering};

const NO_PAUSE: std::time::Duration = std::time::Duration::ZERO;

#[test]
fn a_stopped_poller_polls_no_more() {
    let stop = AtomicBool::new(true);
    let n = poll_until(&stop, &LostFlag::default(), NO_PAUSE, || true);
    assert_eq!(n, 0);
}

#[test]
fn a_lost_device_is_not_polled() {
    let lost = LostFlag::default();
    mark_lost(&lost, "before");
    let n = poll_until(&AtomicBool::new(false), &lost, NO_PAUSE, || {
        unreachable!("a lost device was polled")
    });
    assert_eq!(n, 0);
}

#[test]
fn the_poller_stops_at_the_poll_that_finds_the_device_lost() {
    let lost = LostFlag::default();
    let mut k = 0;
    let n = poll_until(&AtomicBool::new(false), &lost, NO_PAUSE, || {
        k += 1;
        k < 3
    });
    assert_eq!(n, 3);
}

#[test]
fn the_poller_stops_once_the_loss_handler_marked_the_device() {
    let lost = LostFlag::default();
    let mut k = 0;
    let n = poll_until(&AtomicBool::new(false), &lost, NO_PAUSE, || {
        k += 1;
        if k == 2 {
            mark_lost(&lost, "the handler, between two polls");
        }
        true
    });
    assert_eq!(n, 2);
}

#[test]
fn the_poller_stops_when_asked() {
    let stop = AtomicBool::new(false);
    let n = poll_until(&stop, &LostFlag::default(), NO_PAUSE, || {
        stop.store(true, Ordering::Relaxed);
        true
    });
    assert_eq!(n, 1);
}

#[test]
fn the_poller_thread_runs_and_stops_on_a_real_device() {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::NOOP;
    descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, _queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let lost = LostFlag::default();
    let poller = DevicePoller::start(&device, lost.clone()).expect("a poller off OpenGL");
    std::thread::sleep(std::time::Duration::from_millis(5));
    // the device lost: the thread ends by itself, before it is asked to
    mark_lost(&lost, "test");
    let mut poller = poller;
    let t = poller.thread.take().unwrap();
    t.join().expect("the poller thread ended without a panic");
}

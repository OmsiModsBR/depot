//! Tests moved out of `src/gpu/lost.rs`.

use omsi_render::testing::internals::gpu::caps::*;
use omsi_render::testing::internals::gpu::lost::*;

fn noop_device() -> (wgpu::Device, wgpu::Queue) {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::NOOP;
    descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
    let instance = wgpu::Instance::new(descriptor);
    let adapter =
        pollster::block_on(instance.request_adapter(&Default::default())).expect("noop adapter");
    pollster::block_on(adapter.request_device(&Default::default())).expect("noop device")
}

fn read_buffer(device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) -> wgpu::Buffer {
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes.len() as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buf, 0, bytes);
    queue.submit([]);
    buf
}

fn device_wait(device: &wgpu::Device) -> Result<(), GpuWaitError> {
    guarded_poll(None, false, || device.poll(wait_poll(None, None))).map(|_| ())
}

#[test]
fn the_first_cause_of_a_loss_is_kept() {
    let flag = LostFlag::default();
    assert_eq!(lost_why(&flag), None);
    assert!(mark_lost(&flag, "first"));
    assert!(!mark_lost(&flag, "second"));
    assert_eq!(lost_why(&flag).as_deref(), Some("first"));
}

#[test]
fn a_poisoned_flag_is_still_read() {
    let flag = LostFlag::default();
    let f = flag.clone();
    let _ = std::thread::spawn(move || {
        let _g = f.lock().unwrap();
        panic!("poison the flag");
    })
    .join();
    assert!(flag.is_poisoned());
    assert!(mark_lost(&flag, "after the poison"));
    assert_eq!(lost_why(&flag).as_deref(), Some("after the poison"));
}

#[test]
fn the_lost_poll_hook_waits_three_seconds_and_only_for_its_mode() {
    use omsi_render::testing::internals::gpu::fault::{due, is_due, Hook};
    assert!(!is_due(None, Hook::LostPoll, 10.0));
    assert!(!is_due(Some("lost"), Hook::LostPoll, 10.0));
    assert!(!is_due(Some("lost-poll"), Hook::LostPoll, 2.0));
    assert!(is_due(Some("lost-poll"), Hook::LostPoll, 3.5));
    // (nothing set in the test's environment: the hook is off)
    assert!(!due(Hook::LostPoll, std::time::Instant::now()));
    assert!(is_due(Some("oom"), Hook::Oom, 3.5));
    assert!(!is_due(Some("oom"), Hook::LostPoll, 3.5));
}

#[test]
fn a_poll_that_works_gives_its_status() {
    let flag = LostFlag::default();
    let r = guarded_poll(Some(&flag), false, || Ok(wgpu::PollStatus::QueueEmpty));
    assert_eq!(r, Ok(wgpu::PollStatus::QueueEmpty));
    assert_eq!(lost_why(&flag), None);
}

#[test]
fn wgpus_own_poll_errors_are_passed_on_and_lose_nothing() {
    let flag = LostFlag::default();
    let r = guarded_poll(Some(&flag), false, || Err(wgpu::PollError::Timeout));
    assert_eq!(r, Err(GpuWaitError::Poll(wgpu::PollError::Timeout)));
    assert_eq!(lost_why(&flag), None);
}

#[test]
fn a_panic_in_the_poll_is_the_device_lost() {
    let flag = LostFlag::default();
    let r = guarded_poll(Some(&flag), false, || {
        panic!("Error in Device::poll: Parent device is lost")
    });
    assert_eq!(r, Err(GpuWaitError::Lost));
    assert!(lost_why(&flag).is_some_and(|w| w.contains("polled")));
    // (and the panic was the renderer's to catch: the thread goes on as before)
    assert!(!omsi_render::testing::internals::catching());
}

#[test]
fn a_panic_in_a_poll_without_a_flag_is_still_the_device_lost() {
    let r = guarded_poll(None, false, || panic!("lost"));
    assert_eq!(r, Err(GpuWaitError::Lost));
}

#[test]
fn a_device_known_lost_is_not_polled_again() {
    let flag = LostFlag::default();
    mark_lost(&flag, "before");
    let r = guarded_poll(
        Some(&flag),
        false,
        || -> Result<wgpu::PollStatus, wgpu::PollError> {
            unreachable!("a lost device was polled")
        },
    );
    assert_eq!(r, Err(GpuWaitError::Lost));
    assert_eq!(lost_why(&flag).as_deref(), Some("before"));
}

#[test]
fn the_lost_poll_hook_loses_the_device_as_wgpu_does() {
    let flag = LostFlag::default();
    let r = guarded_poll(Some(&flag), true, || Ok(wgpu::PollStatus::QueueEmpty));
    assert_eq!(r, Err(GpuWaitError::Lost));
    // (the loss handler's word, not the guard's: the handler runs first in wgpu)
    assert!(lost_why(&flag).is_some_and(|w| w.contains("lost-poll")));
    // without a flag the hook still panics into the guard
    assert_eq!(
        guarded_poll(None, true, || Ok(wgpu::PollStatus::QueueEmpty)),
        Err(GpuWaitError::Lost)
    );
}

#[test]
fn a_wait_off_opengl_is_one_wait_without_a_timeout() {
    let mut timeouts = vec![];
    let r = wait_with(false, |t| {
        timeouts.push(t);
        Ok(wgpu::PollStatus::QueueEmpty)
    });
    assert_eq!(r, Ok(()));
    assert_eq!(timeouts, vec![None]);
    // its error is passed on as it is
    assert_eq!(
        wait_with(false, |_| Err(GpuWaitError::Lost)),
        Err(GpuWaitError::Lost)
    );
}

#[test]
fn a_wait_on_opengl_is_made_of_short_ones_until_done() {
    let mut n = 0;
    let r = wait_with(true, |t| {
        assert_eq!(t, Some(GL_WAIT_SLICE));
        n += 1;
        if n < 3 {
            Err(GpuWaitError::Poll(wgpu::PollError::Timeout))
        } else {
            Ok(wgpu::PollStatus::WaitSucceeded)
        }
    });
    assert_eq!(r, Ok(()));
    assert_eq!(n, 3);
}

#[test]
fn a_wait_on_opengl_stops_at_a_lost_device() {
    let mut n = 0;
    let r = wait_with(true, |_| {
        n += 1;
        Err(GpuWaitError::Lost)
    });
    assert_eq!(r, Err(GpuWaitError::Lost));
    assert_eq!(n, 1);
}

#[test]
fn the_wait_poll_waits_for_the_submission_given() {
    let wgpu::PollType::Wait {
        submission_index,
        timeout,
    } = wait_poll(None, Some(GL_WAIT_SLICE))
    else {
        panic!("not a wait")
    };
    assert!(submission_index.is_none());
    assert_eq!(timeout, Some(GL_WAIT_SLICE));
}

#[test]
fn a_buffer_is_read_back_whole() {
    let (device, queue) = noop_device();
    let buf = read_buffer(&device, &queue, &[1, 2, 3, 4, 5, 6, 7, 8]);
    let data = read_buffer_with(&buf, || device_wait(&device)).expect("read back");
    assert_eq!(data.len(), 8);
    // unmapped again: it can be read a second time
    assert!(read_buffer_with(&buf, || device_wait(&device)).is_ok());
}

#[test]
fn a_wait_that_fails_is_the_reads_error() {
    let (device, queue) = noop_device();
    let buf = read_buffer(&device, &queue, &[0; 8]);
    assert_eq!(
        read_buffer_with(&buf, || Err(GpuWaitError::Lost)),
        Err(GpuWaitError::Lost)
    );
}

#[test]
fn a_mapping_never_answered_is_no_read() {
    let (device, queue) = noop_device();
    let buf = read_buffer(&device, &queue, &[0; 8]);
    // (no poll: the mapping's answer never comes)
    assert_eq!(read_buffer_with(&buf, || Ok(())), Err(GpuWaitError::Map));
}

#[test]
fn a_mapping_refused_is_no_read() {
    let (device, queue) = noop_device();
    let buf = read_buffer(&device, &queue, &[0; 8]);
    // a buffer destroyed before its mapping (as the device's loss destroys them all):
    // wgpu answers the mapping with an error
    buf.destroy();
    let r = read_buffer_with(&buf, || device_wait(&device));
    assert!(
        matches!(r, Err(GpuWaitError::Map) | Err(GpuWaitError::Lost)),
        "{r:?}"
    );
}

#[test]
fn a_mapping_gone_after_its_answer_is_the_device_lost() {
    let (device, queue) = noop_device();
    let buf = read_buffer(&device, &queue, &[0; 8]);
    // the mapping is answered, then taken away before it is read (as a loss in between
    // does): wgpu's `get_mapped_range` panics, and the read gives the device lost
    let r = read_buffer_with(&buf, || {
        device_wait(&device)?;
        buf.unmap();
        Ok(())
    });
    assert_eq!(r, Err(GpuWaitError::Lost));
}

#[test]
fn the_errors_say_what_happened() {
    assert_eq!(
        GpuWaitError::Lost.to_string(),
        "the graphics device was lost"
    );
    assert_eq!(
        GpuWaitError::Map.to_string(),
        "the buffer could not be read back"
    );
    assert!(!GpuWaitError::Poll(wgpu::PollError::Timeout)
        .to_string()
        .is_empty());
}

#[test]
fn letting_go_of_a_hung_device_is_no_crash() {
    struct Hangs;
    impl Drop for Hangs {
        fn drop(&mut self) {
            // (wgpu-core's panic when the queue's last wait times out)
            panic!("We timed out while waiting on the last successful submission to complete!");
        }
    }
    assert!(!let_go(Hangs));
    assert!(!omsi_render::testing::internals::catching());
    assert!(let_go(String::from("an ordinary renderer")));
}

#[test]
fn errors_compare_by_kind() {
    let timeout = || GpuWaitError::Poll(wgpu::PollError::Timeout);
    assert_eq!(timeout(), timeout());
    assert_ne!(
        timeout(),
        GpuWaitError::Poll(wgpu::PollError::WrongSubmissionIndex(2, 1))
    );
    assert_ne!(timeout(), GpuWaitError::Lost);
    assert_ne!(GpuWaitError::Lost, GpuWaitError::Map);
    assert_eq!(GpuWaitError::Map, GpuWaitError::Map);
}

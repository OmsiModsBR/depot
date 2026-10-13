//! A lost device (a driver reset, the card removed, the card out of memory) without the end
//! of the game. wgpu 29 lets most calls on a lost device fail as errors, but a few panic
//! instead, whatever the device's error handler: `Device::poll` (a wait or a check of the
//! device's fence that the driver answers "lost": on Vulkan every poll after the loss),
//! `Buffer::get_mapped_range` (a read-back whose mapping the loss cancelled) and the queue's
//! last wait when it is let go of (a card that hangs instead of giving up). The game reached
//! one of these in the middle of a frame - before the next frame saw the device as lost and
//! ended the session in order - and ended there (#48). Here they are made without a panic
//! reaching the game: a panic is caught and taken for what it is, the device lost.

use super::caps::GL_WAIT_SLICE;
use super::errors::catch;
use std::sync::{Arc, Mutex};

/// Why the device was lost, once it was (set by the device's loss handler, or here when wgpu
/// panicked over the loss before the handler ran).
pub(crate) type LostFlag = Arc<Mutex<Option<String>>>;

/// Why the device was lost (None while it works).
pub(crate) fn lost_why(flag: &LostFlag) -> Option<String> {
    flag.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Take the device as lost for `why`, unless it was already (the first cause is kept).
/// Whether this call was the one that marked it.
pub(crate) fn mark_lost(flag: &LostFlag, why: impl Into<String>) -> bool {
    let mut lost = flag.lock().unwrap_or_else(|e| e.into_inner());
    if lost.is_some() {
        return false;
    }
    let why = why.into();
    log::error!("the graphics device was lost ({why})");
    *lost = Some(why);
    true
}

/// Let `x` (a renderer, its device and queue) go at the end. Letting the device's queue go
/// waits for the GPU, and wgpu panics when a card that hangs does not answer in time: the
/// game had ended in order, and that panic still went to the log and the launcher as its
/// crash. Whether it went in order.
pub(crate) fn let_go<T>(x: T) -> bool {
    let closed = catch(move || drop(x)).is_some();
    if !closed {
        log::warn!("the graphics device did not close in order (the game had ended already)");
    }
    closed
}

/// Why waiting for the device (or reading a buffer back from it) gave nothing.
#[derive(Debug)]
pub enum GpuWaitError {
    /// wgpu's own answer: a wait that timed out, or a submission it does not know.
    Poll(wgpu::PollError),
    /// The device is gone (wgpu panicked over it, or it was known lost before the call).
    Lost,
    /// The buffer could not be mapped (its mapping was refused or cancelled).
    Map,
}

impl PartialEq for GpuWaitError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // (wgpu's poll error has no comparison of its own; its text tells the kinds apart)
            (Self::Poll(a), Self::Poll(b)) => a.to_string() == b.to_string(),
            (Self::Lost, Self::Lost) | (Self::Map, Self::Map) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for GpuWaitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Poll(e) => write!(f, "{e}"),
            Self::Lost => f.write_str("the graphics device was lost"),
            Self::Map => f.write_str("the buffer could not be read back"),
        }
    }
}

impl std::error::Error for GpuWaitError {}

/// The test hook `OMSI_FAKE_GPU_ERROR=lost-poll`: from three seconds after the renderer was
/// made (`started`), the device is lost the way a driver loses it while the game runs - its
/// loss handler is told, and the poll of it panics inside wgpu as on Vulkan (at the start of
/// a frame, or in the game's own waits for the device). Unlike `lost` (which only raises the
/// flag), this goes through the panic that ended the game.
pub(crate) fn fake_poll_loss(started: std::time::Instant) -> bool {
    fake_poll_loss_for(
        omsi_cfg::flags::FAKE_GPU_ERROR.text().as_deref(),
        started.elapsed().as_secs_f32(),
    )
}

/// `fake_poll_loss` for a given mode and the seconds since the renderer was made.
pub(crate) fn fake_poll_loss_for(mode: Option<&str>, seconds: f32) -> bool {
    fake_due(mode, "lost-poll", seconds)
}

/// Whether the `OMSI_FAKE_GPU_ERROR` test hook `mode` is `name`, three seconds in (the
/// session has drawn its first frames by then).
pub(crate) fn fake_due(mode: Option<&str>, name: &str, seconds: f32) -> bool {
    mode == Some(name) && seconds > 3.0
}

/// One poll of the device (`poll`), guarded: a panic in it is taken as the device lost and
/// marked on `flag` when there is one. A device already known lost is not polled at all
/// (on Vulkan that poll is the panic). `fake`: the `lost-poll` test hook loses it now.
pub(crate) fn guarded_poll(
    flag: Option<&LostFlag>,
    fake: bool,
    poll: impl FnOnce() -> Result<wgpu::PollStatus, wgpu::PollError>,
) -> Result<wgpu::PollStatus, GpuWaitError> {
    if flag.is_some_and(|f| lost_why(f).is_some()) {
        return Err(GpuWaitError::Lost);
    }
    let r = catch(|| {
        if fake {
            // what wgpu does: the loss handler first, then the panic of `Device::poll`
            if let Some(f) = flag {
                mark_lost(f, "Unknown: test (OMSI_FAKE_GPU_ERROR=lost-poll)");
            }
            panic!("Error in Device::poll: Parent device is lost (test: OMSI_FAKE_GPU_ERROR=lost-poll)");
        }
        poll()
    });
    match r {
        Some(Ok(status)) => Ok(status),
        Some(Err(e)) => Err(GpuWaitError::Poll(e)),
        None => {
            if let Some(f) = flag {
                mark_lost(f, "Unknown: wgpu gave the device up while it was polled");
            }
            Err(GpuWaitError::Lost)
        }
    }
}

/// Wait until the GPU is done, by `poll(timeout)`: one wait without a timeout, or on OpenGL
/// (`gl`) short ones until it is done (see `wait_gpu`).
pub(crate) fn wait_with(
    gl: bool,
    mut poll: impl FnMut(Option<std::time::Duration>) -> Result<wgpu::PollStatus, GpuWaitError>,
) -> Result<(), GpuWaitError> {
    if !gl {
        return poll(None).map(|_| ());
    }
    loop {
        match poll(Some(GL_WAIT_SLICE)) {
            Err(GpuWaitError::Poll(wgpu::PollError::Timeout)) => std::thread::yield_now(),
            r => return r.map(|_| ()),
        }
    }
}

/// The poll of a wait for `submission` (None: everything submitted so far).
pub(crate) fn wait_poll(
    submission: Option<wgpu::SubmissionIndex>,
    timeout: Option<std::time::Duration>,
) -> wgpu::PollType {
    wgpu::PollType::Wait {
        submission_index: submission,
        timeout,
    }
}

/// Read `buf` (made with `MAP_READ`, its copy already submitted) back whole: mapped, waited
/// for by `wait`, copied out and unmapped. A mapping the device refused or cancelled, or a
/// device lost on the way, is an error instead of wgpu's panic in `get_mapped_range`.
pub(crate) fn read_buffer_with(
    buf: &wgpu::Buffer,
    wait: impl FnOnce() -> Result<(), GpuWaitError>,
) -> Result<Vec<u8>, GpuWaitError> {
    let (tx, rx) = std::sync::mpsc::channel();
    let slice = buf.slice(..);
    // (a device already lost may refuse the call itself)
    catch(|| {
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        })
    })
    .ok_or(GpuWaitError::Lost)?;
    wait()?;
    match rx.try_recv() {
        Ok(Ok(())) => {}
        // (refused, or never answered: the device went before the copy was done)
        Ok(Err(_)) | Err(_) => return Err(GpuWaitError::Map),
    }
    let data = catch(|| slice.get_mapped_range().to_vec()).ok_or(GpuWaitError::Lost)?;
    catch(|| buf.unmap());
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_device() -> (wgpu::Device, wgpu::Queue) {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::NOOP;
        descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
            .expect("noop adapter");
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
        assert!(!fake_poll_loss_for(None, 10.0));
        assert!(!fake_poll_loss_for(Some("lost"), 10.0));
        assert!(!fake_poll_loss_for(Some("lost-poll"), 2.0));
        assert!(fake_poll_loss_for(Some("lost-poll"), 3.5));
        // (nothing set in the test's environment: the hook is off)
        assert!(!fake_poll_loss(std::time::Instant::now()));
        assert!(fake_due(Some("oom"), "oom", 3.5));
        assert!(!fake_due(Some("oom"), "lost-poll", 3.5));
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
        assert!(!crate::gpu::catching());
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
        assert!(!crate::gpu::catching());
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
}

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
pub type LostFlag = Arc<Mutex<Option<String>>>;

/// Why the device was lost (None while it works).
pub fn lost_why(flag: &LostFlag) -> Option<String> {
    flag.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Take the device as lost for `why`, unless it was already (the first cause is kept).
/// Whether this call was the one that marked it.
pub fn mark_lost(flag: &LostFlag, why: impl Into<String>) -> bool {
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
pub fn let_go<T>(x: T) -> bool {
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

/// One poll of the device (`poll`), guarded: a panic in it is taken as the device lost and
/// marked on `flag` when there is one. A device already known lost is not polled at all
/// (on Vulkan that poll is the panic). `fake`: the `lost-poll` test hook loses it now.
pub fn guarded_poll(
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
pub fn wait_with(
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
pub fn wait_poll(
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
pub fn read_buffer_with(
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

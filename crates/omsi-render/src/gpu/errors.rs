//! What goes wrong with the device: the text of a GPU error, what the game does with one,
//! and panics in a driver caught so they do not end the game.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// A GPU error with what the validation layer said, not just its kind.
pub fn gpu_error_text(e: &wgpu::Error) -> String {
    match e {
        wgpu::Error::Validation { description, .. } | wgpu::Error::Internal { description, .. } => {
            description.trim().replace('\n', " ")
        }
        other => other.to_string(),
    }
}

thread_local! {
    /// A panic on this thread now is caught and handled (see [`catching`]).
    pub static CATCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a panic on this thread is being caught by the renderer (the game's panic hook
/// does not report it as the end of the game).
pub fn catching() -> bool {
    CATCHING.with(|c| c.get())
}

/// Run `f`; a panic in it (a driver wgpu cannot use, taking it down in a way it does not
/// turn into an error) is caught and gives None, and is not reported as the end of the game.
pub fn catch<R>(f: impl FnOnce() -> R) -> Option<R> {
    let was = CATCHING.with(|c| c.replace(true));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    CATCHING.with(|c| c.set(was));
    r.ok()
}

/// What the device's error handler does with an error wgpu did not hand to anyone else
/// (`GpuContext::open`): a GPU error while multisampling is on is logged and switches
/// multisampling off at the next frame (`render_inner`); otherwise it is logged and the game goes on: wgpu's own handler ends the
/// process, and one call a driver refused (a limit of that card, a bigger map than the last)
/// closed the game a few seconds into the drive - a wrong picture for a frame is better than
/// no game. The first errors and then every thousandth reach the log. Running out of memory
/// is also noted (`out_of_memory`), so that the game keeps fewer textures.
pub struct GpuErrors {
    pub gpu_error: Arc<AtomicBool>,
    pub out_of_memory: Arc<AtomicBool>,
    pub msaa: u32,
    count: AtomicU64,
    /// The `oom` test hook sent its error.
    pub fake_oom_sent: AtomicBool,
}

/// What `GpuErrors::on_error` did with an error (for the tests).
#[derive(Debug, PartialEq, Eq)]
pub enum ErrorOutcome {
    /// The first with multisampling on: it is switched off.
    MsaaOff,
    /// Logged; the game goes on.
    Logged,
    /// Not logged (one of the many between two thousandths).
    Quiet,
}

impl GpuErrors {
    pub fn new(msaa: u32) -> Self {
        Self {
            gpu_error: Default::default(),
            out_of_memory: Default::default(),
            msaa,
            count: AtomicU64::new(0),
            fake_oom_sent: AtomicBool::new(false),
        }
    }

    /// The `oom` test hook: the device reports once that it is out of memory, as for a
    /// texture it could not make, through the same handler. Whether it was sent now.
    pub fn fake_out_of_memory_once(&self) -> bool {
        if self.fake_oom_sent.swap(true, Ordering::Relaxed) {
            return false;
        }
        log::error!("the graphics card ran out of memory (test): OMSI_FAKE_GPU_ERROR=oom");
        self.on_error(&fake_out_of_memory());
        true
    }

    pub fn on_error(&self, e: &wgpu::Error) -> ErrorOutcome {
        if matches!(e, wgpu::Error::OutOfMemory { .. }) {
            self.out_of_memory.store(true, Ordering::Relaxed);
        }
        let n = self.count.fetch_add(1, Ordering::Relaxed);
        if self.msaa > 1 && !self.gpu_error.swap(true, Ordering::Relaxed) {
            log::error!(
                "GPU error with {}x MSAA (drawing without it from now on): {}",
                self.msaa,
                gpu_error_text(e)
            );
            return ErrorOutcome::MsaaOff;
        }
        if n < 20 || n.is_multiple_of(1000) {
            log::error!(
                "GPU error #{} (the game goes on): {}",
                n + 1,
                gpu_error_text(e)
            );
            return ErrorOutcome::Logged;
        }
        ErrorOutcome::Quiet
    }
}

/// An "out of memory" error as the device reports it (the `oom` test hook).
pub fn fake_out_of_memory() -> wgpu::Error {
    wgpu::Error::OutOfMemory {
        source: Box::new(std::io::Error::other("test (OMSI_FAKE_GPU_ERROR=oom)")),
    }
}

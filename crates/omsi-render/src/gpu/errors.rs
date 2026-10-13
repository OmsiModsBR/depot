//! What goes wrong with the device: the text of a GPU error, what the game does with one,
//! and panics in a driver caught so they do not end the game.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// A GPU error with what the validation layer said, not just its kind.
pub(crate) fn gpu_error_text(e: &wgpu::Error) -> String {
    match e {
        wgpu::Error::Validation { description, .. } | wgpu::Error::Internal { description, .. } => {
            description.trim().replace('\n', " ")
        }
        other => other.to_string(),
    }
}

thread_local! {
    /// A panic on this thread now is caught and handled (see [`catching`]).
    pub(crate) static CATCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
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
/// multisampling off at the next frame (`render_inner`); one with ray tracing on switches the
/// ray tracing off; otherwise it is logged and the game goes on: wgpu's own handler ends the
/// process, and one call a driver refused (a limit of that card, a bigger map than the last)
/// closed the game a few seconds into the drive - a wrong picture for a frame is better than
/// no game. The first errors and then every thousandth reach the log. Running out of memory
/// is also noted (`out_of_memory`), so that the game keeps fewer textures.
pub(crate) struct GpuErrors {
    pub(crate) gpu_error: Arc<AtomicBool>,
    pub(crate) rt_error: Arc<AtomicBool>,
    pub(crate) out_of_memory: Arc<AtomicBool>,
    pub(crate) msaa: u32,
    pub(crate) ray_tracing: bool,
    count: AtomicU64,
    /// The `oom` test hook sent its error.
    pub(crate) fake_oom_sent: AtomicBool,
}

/// What `GpuErrors::on_error` did with an error (for the tests).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ErrorOutcome {
    /// The first with ray tracing on: it is switched off.
    RayTracingOff,
    /// The first with multisampling on: it is switched off.
    MsaaOff,
    /// Logged; the game goes on.
    Logged,
    /// Not logged (one of the many between two thousandths).
    Quiet,
}

impl GpuErrors {
    pub(crate) fn new(msaa: u32, ray_tracing: bool) -> Self {
        Self {
            gpu_error: Default::default(),
            rt_error: Default::default(),
            out_of_memory: Default::default(),
            msaa,
            ray_tracing,
            count: AtomicU64::new(0),
            fake_oom_sent: AtomicBool::new(false),
        }
    }

    /// The `oom` test hook: the device reports once that it is out of memory, as for a
    /// texture it could not make, through the same handler. Whether it was sent now.
    pub(crate) fn fake_out_of_memory_once(&self) -> bool {
        if self.fake_oom_sent.swap(true, Ordering::Relaxed) {
            return false;
        }
        log::error!("the graphics card ran out of memory (test): OMSI_FAKE_GPU_ERROR=oom");
        self.on_error(&fake_out_of_memory());
        true
    }

    pub(crate) fn on_error(&self, e: &wgpu::Error) -> ErrorOutcome {
        if matches!(e, wgpu::Error::OutOfMemory { .. }) {
            self.out_of_memory.store(true, Ordering::Relaxed);
        }
        let n = self.count.fetch_add(1, Ordering::Relaxed);
        if self.ray_tracing {
            // (the ray tracing goes first: the likelier cause, and the dearer feature)
            if !self.rt_error.swap(true, Ordering::Relaxed) {
                log::error!(
                    "GPU error with ray tracing (Enhanced+ draws as Enhanced from now on): {}",
                    gpu_error_text(e)
                );
                return ErrorOutcome::RayTracingOff;
            }
            return ErrorOutcome::Quiet;
        }
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
pub(crate) fn fake_out_of_memory() -> wgpu::Error {
    wgpu::Error::OutOfMemory {
        source: Box::new(std::io::Error::other("test (OMSI_FAKE_GPU_ERROR=oom)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validation() -> wgpu::Error {
        wgpu::Error::Validation {
            source: Box::new(std::io::Error::other("bad")),
            description: "a refused call\nover two lines ".into(),
        }
    }

    #[test]
    fn a_validation_errors_text_is_its_description_on_one_line() {
        assert_eq!(
            gpu_error_text(&validation()),
            "a refused call over two lines"
        );
        assert_eq!(gpu_error_text(&fake_out_of_memory()), "Out of Memory");
    }

    #[test]
    fn running_out_of_memory_is_noted_for_the_textures() {
        let errors = GpuErrors::new(1, false);
        assert_eq!(errors.on_error(&fake_out_of_memory()), ErrorOutcome::Logged);
        assert!(errors.out_of_memory.load(Ordering::Relaxed));
        // (an error of another kind does not say so)
        let errors = GpuErrors::new(1, false);
        errors.on_error(&validation());
        assert!(!errors.out_of_memory.load(Ordering::Relaxed));
    }

    #[test]
    fn the_oom_hook_reports_once_through_the_handler() {
        let errors = GpuErrors::new(1, false);
        assert!(errors.fake_out_of_memory_once());
        assert!(errors.out_of_memory.swap(false, Ordering::Relaxed));
        assert!(!errors.fake_out_of_memory_once());
        assert!(!errors.out_of_memory.load(Ordering::Relaxed));
    }

    #[test]
    fn with_ray_tracing_the_first_error_switches_it_off_and_the_others_are_quiet() {
        let errors = GpuErrors::new(4, true);
        assert_eq!(errors.on_error(&validation()), ErrorOutcome::RayTracingOff);
        assert!(errors.rt_error.load(Ordering::Relaxed));
        assert_eq!(errors.on_error(&validation()), ErrorOutcome::Quiet);
        // (multisampling stays: the ray tracing was the likelier cause)
        assert!(!errors.gpu_error.load(Ordering::Relaxed));
    }

    #[test]
    fn with_msaa_the_first_error_switches_it_off_then_errors_are_logged() {
        let errors = GpuErrors::new(4, false);
        assert_eq!(errors.on_error(&validation()), ErrorOutcome::MsaaOff);
        assert!(errors.gpu_error.load(Ordering::Relaxed));
        assert_eq!(errors.on_error(&validation()), ErrorOutcome::Logged);
    }

    #[test]
    fn without_msaa_the_first_twenty_and_every_thousandth_are_logged() {
        let errors = GpuErrors::new(1, false);
        let outcomes: Vec<ErrorOutcome> =
            (0..1001).map(|_| errors.on_error(&validation())).collect();
        assert!(outcomes[..20].iter().all(|o| *o == ErrorOutcome::Logged));
        assert!(outcomes[20..1000].iter().all(|o| *o == ErrorOutcome::Quiet));
        assert_eq!(outcomes[1000], ErrorOutcome::Logged);
        assert!(!errors.gpu_error.load(Ordering::Relaxed));
    }

    #[test]
    fn a_panic_caught_gives_none_and_restores_the_catching_state() {
        assert!(!catching());
        assert_eq!(catch(|| 7), Some(7));
        assert_eq!(catch(|| -> u8 { panic!("a driver went down") }), None);
        assert!(!catching());
        // (a catch inside a catch leaves the outer one catching)
        let inner = catch(|| {
            let r = catch(|| -> u8 { panic!("inner") });
            (r, catching())
        });
        assert_eq!(inner, Some((None, true)));
        assert!(!catching());
    }
}

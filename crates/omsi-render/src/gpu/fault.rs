//! The test hooks of `OMSI_FAKE_GPU_ERROR=<name>`: a device that cannot be opened, a pipeline
//! the driver refuses, a device that is lost, a card out of memory. They let the answers to
//! these failures (the reduced renderer, the next graphics interface, the session ending in
//! order) be tried without a card that fails. The hooks that need the device or the frame
//! (what they do once due) stay where they fire; what says which one is on, and when, is here.

use omsi_cfg::flags::FAKE_GPU_ERROR;
use std::sync::OnceLock;
use std::time::Instant;

/// How long after the renderer was made a hook that fires in a running session waits (the
/// session has drawn its first frames by then).
const SESSION_SECONDS: f32 = 3.0;

/// One test hook, by the value that turns it on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    /// `open`: opening the adapter is refused.
    Open,
    /// `open-panic`: wgpu goes down while the adapter is opened.
    OpenPanic,
    /// `pipeline`: a pipeline fails until the basic ones are made.
    Pipeline,
    /// `lost-build`: the first device is lost once the renderer is made.
    LostBuild,
    /// `build`: a sample count no device takes, when the renderer is made.
    Build,
    /// `lost`: the device is taken away in a running session.
    Lost,
    /// `lost-poll`: the poll of the device panics, as wgpu's does on Vulkan.
    LostPoll,
    /// `oom`: the card runs out of memory once.
    Oom,
    /// `frame`: a frame's target is refused, for the fallback of the frame.
    Frame,
}

impl Hook {
    fn value(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::OpenPanic => "open-panic",
            Self::Pipeline => "pipeline",
            Self::LostBuild => "lost-build",
            Self::Build => "build",
            Self::Lost => "lost",
            Self::LostPoll => "lost-poll",
            Self::Oom => "oom",
            Self::Frame => "frame",
        }
    }
}

/// Which hook the environment asks for (read once).
fn asked() -> Option<&'static str> {
    static ASKED: OnceLock<Option<String>> = OnceLock::new();
    ASKED.get_or_init(|| FAKE_GPU_ERROR.text()).as_deref()
}

/// Whether `hook` is the one asked for.
pub fn is(hook: Hook) -> bool {
    asked() == Some(hook.value())
}

/// Whether `hook` is asked for and the renderer made at `started` has run for a few seconds
/// (the hooks that fire in a session, not while it starts).
pub fn due(hook: Hook, started: Instant) -> bool {
    is_due(asked(), hook, started.elapsed().as_secs_f32())
}

/// `due` for a given request and the seconds the renderer has run.
pub fn is_due(asked: Option<&str>, hook: Hook, seconds: f32) -> bool {
    asked == Some(hook.value()) && seconds > SESSION_SECONDS
}

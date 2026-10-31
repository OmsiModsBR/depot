//! The original OMSI 2 mode: whether it is offered, what is installed and the steps of its
//! install.

use super::super::interface::output::oe;
use oe::detect::{EngineStatus, Integrity};
use oe::install::{InstallPlan, ReleaseInfo, RemovePlan};
use oe::{Engine, OriginalEngineConfig};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;
use std::time::Instant;

/// Whether the feature is released. It is switched on by the last pull request of the
/// rollout (#459, OL-31) once the pieces before it are in; until then only
/// `DEPOT_ORIGINAL_ENGINE=1` shows it (for developers and the rollout's tests). Owner: the
/// maintainers; removed with that pull request.
const RELEASED: bool = false;

/// The feature exists on Windows computers only (OmsiLaunch needs Windows 10+ x64); it is
/// absent on every other system, whatever the config file says.
pub fn available() -> bool {
    if super::super::implementation::text::mobile() {
        return false;
    }
    (RELEASED && cfg!(windows))
        || std::env::var_os("DEPOT_ORIGINAL_ENGINE").is_some_and(|v| v == "1")
}

/// A background job's answer.
pub enum Event {
    Status(Box<EngineStatus>),
    /// (attempt, answer): an answer of an attempt the player left is ignored
    Offer(u64, Result<(ReleaseInfo, String), String>),
    Downloaded(u64, Result<(Arc<Vec<u8>>, InstallPlan), String>),
    Installed(Result<String, String>),
    Removed(Result<RemovePlan, String>),
    Latest(Result<ReleaseInfo, String>),
    /// The answer of a start: the running game, or why there is none.
    Started(Result<oe::session::Running, oe::start::StartError>),
}

/// Where the install dialog is.
pub enum Flow {
    Idle,
    /// Asking the official repository for the newest release.
    Looking,
    /// The release and its checksum: the player decides whether to download.
    Offer {
        release: ReleaseInfo,
        sha: String,
    },
    Downloading {
        release: ReleaseInfo,
    },
    /// Downloaded and verified: exactly which files would be added.
    Review {
        release: ReleaseInfo,
        sha: String,
        zip: Arc<Vec<u8>>,
        plan: InstallPlan,
    },
    Installing,
    /// Removal needs a confirmation too.
    ConfirmRemove(RemovePlan),
    Removing,
    Failed(String),
}

pub struct Original {
    pub status: Option<EngineStatus>,
    pub asked: Option<Instant>,
    pub flow: Flow,
    pub done: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
    pub cancel: Arc<AtomicBool>,
    /// Counts the lookups and downloads started or left, so a late answer can be told apart.
    pub attempt: u64,
    /// The newest release seen by "Check for updates" (Diagnostics).
    pub latest: Option<Result<ReleaseInfo, String>>,
    pub checking: bool,
    /// A start is being planned and made.
    pub starting: bool,
    /// The original game this launcher started, while it runs.
    pub run: Option<oe::session::Running>,
}

impl Default for Original {
    fn default() -> Self {
        Original {
            status: None,
            asked: None,
            flow: Flow::Idle,
            done: Arc::new(AtomicU64::new(0)),
            total: Arc::new(AtomicU64::new(0)),
            cancel: Arc::new(AtomicBool::new(false)),
            attempt: 0,
            latest: None,
            checking: false,
            starting: false,
            run: None,
        }
    }
}

impl Original {
    /// OmsiLaunch is installed, whole and of a version Depot drives.
    pub fn ready(&self) -> bool {
        self.status.as_ref().is_some_and(|s| {
            s.omsilaunch.installed
                && matches!(s.omsilaunch.integrity, Integrity::Intact)
                && (s.omsilaunch.control_protocol.is_none()
                    || oe::install::protocol_supported(s.omsilaunch.control_protocol.as_deref()))
        })
    }

    /// The flag is on and OmsiLaunch is ready: the Depot | OMSI 2 switch is shown.
    pub fn switch_shown(&self, cfg: &OriginalEngineConfig) -> bool {
        available() && cfg.enabled && self.ready()
    }

    /// The engine games start with.
    pub fn engine(&self, cfg: &OriginalEngineConfig) -> Engine {
        if available() {
            cfg.effective(self.ready())
        } else {
            Engine::Depot
        }
    }

    pub fn busy(&self) -> bool {
        matches!(
            self.flow,
            Flow::Looking | Flow::Downloading { .. } | Flow::Installing | Flow::Removing
        )
    }
}

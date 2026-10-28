//! Supervising a started session: the one running game, how it ended, and the rule that
//! only one game runs at a time (a Depot game or the original, never both).

use super::errors::{explain, shim_problem, Fix};
use std::path::PathBuf;
use std::process::Child;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// OMSI 2 ran and was closed; its files are back as they were.
    Completed,
    /// The session did not complete; the sentence says why as far as the exit code tells.
    Failed {
        code: Option<i32>,
        message: &'static str,
        fix: Fix,
    },
}

/// What an exit code of `OmsiLaunchW.exe` means for the player. The reason itself (the
/// `OL_E_` code) is in the OmsiLaunch log; the code alone is enough for a first sentence.
pub fn outcome_of(code: Option<i32>) -> Outcome {
    match code {
        Some(0) => Outcome::Completed,
        Some(c) if shim_problem(c).is_some() => {
            let (message, fix) = shim_problem(c).unwrap();
            Outcome::Failed { code, message, fix }
        }
        Some(7) => {
            let (message, fix) = explain("OL_E_INSTALLATION_BUSY");
            Outcome::Failed { code, message, fix }
        }
        Some(8) => {
            let (message, fix) = explain("OL_E_RESTORE_FAILED");
            Outcome::Failed { code, message, fix }
        }
        Some(3) => {
            let (message, fix) = explain("OL_E_UNSUPPORTED_BUILD");
            Outcome::Failed { code, message, fix }
        }
        _ => {
            let (message, fix) = explain("OL_E_PLAN_NOT_RUNNABLE");
            Outcome::Failed { code, message, fix }
        }
    }
}

/// The original game this launcher started.
pub struct Running {
    child: Child,
    pub started: Instant,
    /// The run's launch description, removed when the session ends.
    spec_file: PathBuf,
}

impl Running {
    pub fn new(child: Child, spec_file: PathBuf) -> Self {
        Running {
            child,
            started: Instant::now(),
            spec_file,
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// `None` while it runs; the outcome once it has ended (and its files are cleaned up).
    pub fn poll(&mut self) -> Option<Outcome> {
        match self.child.try_wait() {
            Ok(None) => None,
            Ok(Some(status)) => {
                super::config::remove_launch_file(&self.spec_file);
                Some(outcome_of(status.code()))
            }
            Err(_) => {
                super::config::remove_launch_file(&self.spec_file);
                Some(outcome_of(None))
            }
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // the launcher closing does not end the game (OmsiLaunch owns it). While the child
        // runs it may not have read its launch description yet, so the file stays then
        // (the next start of the same session writes it again); once it has ended it goes.
        if matches!(self.child.try_wait(), Ok(Some(_)) | Err(_)) {
            super::config::remove_launch_file(&self.spec_file);
        }
    }
}

/// Why a start is refused right now, if it is: one game at a time.
pub fn refusal(depot_game_running: bool, original_running: bool) -> Option<&'static str> {
    (depot_game_running || original_running).then_some(
        "A game is running already. Close it before starting another one (its window may be in the tray).",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_become_outcomes() {
        assert_eq!(outcome_of(Some(0)), Outcome::Completed);
        assert!(matches!(
            outcome_of(Some(102)),
            Outcome::Failed {
                fix: Fix::InstallDotnet,
                ..
            }
        ));
        assert!(matches!(
            outcome_of(Some(7)),
            Outcome::Failed {
                fix: Fix::CloseOtherSession,
                ..
            }
        ));
        assert!(matches!(
            outcome_of(Some(8)),
            Outcome::Failed {
                fix: Fix::RecoverSession,
                ..
            }
        ));
        assert!(matches!(
            outcome_of(Some(1)),
            Outcome::Failed { code: Some(1), .. }
        ));
        assert!(matches!(
            outcome_of(None),
            Outcome::Failed { code: None, .. }
        ));
    }

    #[test]
    fn only_one_game_at_a_time() {
        assert!(refusal(false, false).is_none());
        assert!(refusal(true, false).is_some());
        assert!(refusal(false, true).is_some());
    }
}

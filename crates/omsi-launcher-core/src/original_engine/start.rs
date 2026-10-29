//! Starting the original game: the launch description and command line, OmsiLaunch's own
//! plan of the session (nothing is started when it is not runnable), then the session.

use super::client::{Client, Problem};
use super::errors::explain;
use super::launch::{self, World};
use super::runner::OriginalEngineRunner;
use super::{config, session::Running};
use serde_json::{Map, Value};
use std::path::Path;

/// Why a start did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// What Depot would send is not right (a path outside the OMSI 2 folder, a saved value
    /// OMSI 2 does not take): the sentence says which.
    Invalid(String),
    /// OmsiLaunch's plan is not runnable: each reason, as a sentence and a fix.
    NotRunnable(Vec<Problem>),
    /// The program could not be started.
    Spawn(String),
}

/// Builds the launch of run `id`, asks OmsiLaunch whether it can run, and starts it. The
/// launch description is removed again whenever nothing was started.
pub fn start<R: OriginalEngineRunner>(
    client: &Client<R>,
    data: &Path,
    root: &Path,
    id: &str,
    world: &World,
    settings: &Map<String, Value>,
    modules: &Map<String, Value>,
) -> Result<Running, StartError> {
    let launch = launch::build(data, root, id, world, settings, modules)
        .map_err(|e| StartError::Invalid(format!("{e:#}")))?;
    let gone = |e| {
        config::remove_launch_file(&launch.spec_file);
        e
    };
    let plan = client
        .plan(&launch.args)
        .map_err(|e| gone(StartError::Invalid(format!("{e:#}"))))?;
    if !plan.runnable {
        let mut problems = plan.problems;
        if problems.is_empty() {
            let (message, fix) = explain("OL_E_PLAN_NOT_RUNNABLE");
            problems.push(Problem {
                code: "OL_E_PLAN_NOT_RUNNABLE".into(),
                message,
                fix,
            });
        }
        return Err(gone(StartError::NotRunnable(problems)));
    }
    match client.start(&launch.args) {
        Ok(child) => Ok(Running::new(child, launch.spec_file)),
        Err(e) => Err(gone(StartError::Spawn(e.to_string()))),
    }
}

//! The original OMSI 2 mode's work: started, looked up, downloaded, installed and removed;
//! the logic is `omsi_launcher_lib::original_engine`, this is the part that needs the
//! window's messages and background jobs.

use super::super::domain::{msg::*, original::*, state::*};
use super::super::interface::output::{core, oe};
use oe::install::{InstallPlan, ReleaseInfo};
use oe::Engine;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Instant;

impl State {
    /// The original OMSI 2 is the engine games start with: the pages show what it has.
    pub fn omsi2_engine(&self) -> bool {
        self.original.engine(&self.config.original_engine) == Engine::Omsi2
    }

    /// The world a duty starts in: its map and the start point, as OMSI 2 presents them.
    pub fn world_of(d: &core::Duty) -> oe::launch::World {
        oe::launch::World::New {
            map: d.map.clone(),
            entrypoint: d.entry.unwrap_or(0).max(0) as u32,
        }
    }

    /// Starts the original game for `d`: the launch is built, OmsiLaunch plans it and, when
    /// it can run, starts it (all off the UI thread). Only the map and the start point are
    /// sent; the rest of the duty is the Depot game's.
    pub fn launch_original(&mut self, d: &core::Duty) {
        let root = std::path::PathBuf::from(&self.config.root);
        let data = core::data_dir();
        let world = Self::world_of(d);
        let id = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |t| t.as_millis())
        );
        self.original.starting = true;
        self.set_status("Starting the game…", false);
        self.spawn(move || {
            let settings = oe::config::load_settings_in(&data);
            let modules = oe::config::load_modules_in(&data);
            let runner = oe::runner::ProcessRunner;
            let r = match oe::client::Client::in_root(&runner, &root) {
                Ok(client) => {
                    oe::start::start(&client, &data, &root, &id, &world, &settings, &modules)
                }
                Err(e) => Err(oe::start::StartError::Invalid(e)),
            };
            Msg::Original(Event::Started(r))
        });
    }

    /// Sees whether the original game has ended (every frame; cheap).
    pub fn engine_poll(&mut self) {
        let Some(run) = self.original.run.as_mut() else {
            return;
        };
        let Some(outcome) = run.poll() else { return };
        self.original.run = None;
        if let oe::session::Outcome::Failed {
            code,
            message,
            fix: _,
        } = outcome
        {
            core::log_to_file(&format!(
                "ERROR OMSI 2 session ended with code {code:?}: {message}"
            ));
            self.set_status(omsi_ui::tr(message), true);
        }
        self.engine_refresh(true);
    }

    /// Reads what is installed (in the background); at most every few seconds.
    pub fn engine_refresh(&mut self, force: bool) {
        if !available() {
            return;
        }
        if !force
            && self
                .original
                .asked
                .is_some_and(|t| t.elapsed().as_secs_f32() < 4.0)
        {
            return;
        }
        self.original.asked = Some(Instant::now());
        let root = std::path::PathBuf::from(&self.config.root);
        self.spawn(move || {
            // (a day old: an install or a game of another Depot may be running)
            oe::config::sweep_leftovers(&core::data_dir(), std::time::Duration::from_secs(86_400));
            let have = !root.as_os_str().is_empty() && root.is_dir();
            let st =
                oe::detect::status_for(have.then_some(root.as_path()), oe::detect::dotnet_here());
            Msg::Original(Event::Status(Box::new(st)))
        });
    }

    /// The flag under Setup. Saved at once; turning it off falls back to Depot.
    pub fn engine_set_enabled(&mut self, on: bool) {
        if self.config.original_engine.enabled == on {
            return;
        }
        self.config.original_engine.enabled = on;
        if !on {
            self.config.original_engine.set_mode(Engine::Depot);
        }
        self.engine_save();
        self.engine_refresh(true);
    }

    /// The switch in the sidebar or the top bar. Refused while a game runs.
    pub fn engine_set_mode(&mut self, mode: Engine) {
        if self.in_game() || self.config.original_engine.mode == mode {
            return;
        }
        self.config.original_engine.set_mode(mode);
        self.engine_save();
    }

    fn engine_save(&mut self) {
        if let Err(e) = core::save_config(&self.config) {
            self.set_status(format!("{e:#}"), true);
        }
    }

    /// "Install OmsiLaunch": asks the official repository for the newest release and its
    /// checksum. Nothing else is downloaded before the player agrees.
    pub fn engine_look(&mut self) {
        if self.original.busy() {
            return;
        }
        self.original.attempt += 1;
        let n = self.original.attempt;
        self.original.flow = Flow::Looking;
        self.spawn(move || Msg::Original(Event::Offer(n, offer())));
    }

    /// The player agreed to download the package.
    pub fn engine_download(&mut self) {
        let Flow::Offer { release, sha } = std::mem::replace(&mut self.original.flow, Flow::Idle)
        else {
            return;
        };
        // each download has its own counters and cancel flag: a worker the player left
        // can neither be revived nor move the new one's bar
        self.original.attempt += 1;
        let n = self.original.attempt;
        self.original.done = Arc::new(AtomicU64::new(0));
        self.original.total = Arc::new(AtomicU64::new(0));
        self.original.cancel = Arc::new(AtomicBool::new(false));
        let (done, total, cancel) = (
            self.original.done.clone(),
            self.original.total.clone(),
            self.original.cancel.clone(),
        );
        let root = std::path::PathBuf::from(&self.config.root);
        self.original.flow = Flow::Downloading {
            release: release.clone(),
        };
        self.spawn(move || {
            Msg::Original(Event::Downloaded(
                n,
                fetch_package(&release, &sha, &root, &done, &total, &cancel),
            ))
        });
    }

    /// The player confirmed the file list: install.
    pub fn engine_install(&mut self) {
        let Flow::Review {
            release, sha, zip, ..
        } = std::mem::replace(&mut self.original.flow, Flow::Installing)
        else {
            return;
        };
        let root = std::path::PathBuf::from(&self.config.root);
        let data = core::data_dir();
        self.spawn(move || {
            Msg::Original(Event::Installed(
                oe::install::install(&root, &data, &zip, &sha, &release.version)
                    .map(|r| r.version)
                    .map_err(|e| format!("{e:#}")),
            ))
        });
    }

    pub fn engine_ask_remove(&mut self) {
        if self.original.busy() {
            return;
        }
        let plan =
            oe::install::plan_remove(std::path::Path::new(&self.config.root), &core::data_dir());
        self.original.flow = Flow::ConfirmRemove(plan);
    }

    pub fn engine_remove(&mut self) {
        let Flow::ConfirmRemove(_) = std::mem::replace(&mut self.original.flow, Flow::Removing)
        else {
            return;
        };
        let root = std::path::PathBuf::from(&self.config.root);
        let data = core::data_dir();
        self.spawn(move || {
            Msg::Original(Event::Removed(
                oe::install::remove(&root, &data).map_err(|e| format!("{e:#}")),
            ))
        });
    }

    /// "Check for updates" (only when asked).
    pub fn engine_check_update(&mut self) {
        if self.original.checking {
            return;
        }
        self.original.checking = true;
        self.spawn(move || Msg::Original(Event::Latest(latest())));
    }

    pub fn engine_cancel(&mut self) {
        self.original.cancel.store(true, Relaxed);
        if matches!(self.original.flow, Flow::Looking | Flow::Downloading { .. }) {
            self.original.attempt += 1;
        }
        if !self.original.busy() || matches!(self.original.flow, Flow::Looking) {
            self.original.flow = Flow::Idle;
        } else if !matches!(self.original.flow, Flow::Installing | Flow::Removing) {
            self.original.flow = Flow::Idle;
        }
    }

    pub fn engine_event(&mut self, e: Event) {
        match e {
            Event::Status(s) => {
                self.original.status = Some(*s);
            }
            Event::Started(r) => {
                self.original.starting = false;
                match r {
                    Ok(run) => {
                        core::log_to_file(&format!("OMSI 2 started (process {})", run.pid()));
                        self.original.run = Some(run);
                    }
                    Err(oe::start::StartError::NotRunnable(problems)) => {
                        for p in &problems {
                            core::log_to_file(&format!("ERROR OMSI 2 start refused: {}", p.code));
                        }
                        let message = problems.first().map_or("", |p| p.message);
                        self.set_status(omsi_ui::tr(message), true);
                    }
                    Err(oe::start::StartError::Spawn(e)) => {
                        core::log_to_file(&format!("ERROR OMSI 2 could not be started: {e}"));
                        let (message, _) = oe::errors::explain("OL_E_PROCESS_START_FAILED");
                        self.set_status(omsi_ui::tr(message), true);
                    }
                    Err(oe::start::StartError::Invalid(e)) => self.set_status(
                        omsi_ui::tr("OMSI 2 mode could not prepare the start: %{error}")
                            .replace("%{error}", &e),
                        true,
                    ),
                }
            }
            Event::Offer(n, r) => {
                // (a lookup the player cancelled meanwhile is dropped)
                if n == self.original.attempt && matches!(self.original.flow, Flow::Looking) {
                    self.original.flow = match r {
                        Ok((release, sha)) => Flow::Offer { release, sha },
                        Err(e) => Flow::Failed(e),
                    };
                }
            }
            Event::Downloaded(n, r) => {
                if n != self.original.attempt
                    || !matches!(self.original.flow, Flow::Downloading { .. })
                {
                    return;
                }
                let Flow::Downloading { release } =
                    std::mem::replace(&mut self.original.flow, Flow::Idle)
                else {
                    return;
                };
                self.original.flow = match r {
                    Ok((zip, plan)) => {
                        let sha = oe::install::sha256_hex(&zip);
                        Flow::Review {
                            release,
                            sha,
                            zip,
                            plan,
                        }
                    }
                    Err(e) if e.contains("cancelled") => Flow::Idle,
                    Err(e) => Flow::Failed(e),
                };
            }
            Event::Installed(r) => {
                self.original.flow = match r {
                    Ok(v) => {
                        core::log_to_file(&format!("OmsiLaunch {v} installed"));
                        Flow::Idle
                    }
                    Err(e) => Flow::Failed(e),
                };
                self.engine_refresh(true);
            }
            Event::Removed(r) => {
                self.original.flow = match r {
                    Ok(_) => {
                        // without OmsiLaunch the switch is gone and games start with Depot
                        self.config.original_engine.set_mode(Engine::Depot);
                        self.engine_save();
                        Flow::Idle
                    }
                    Err(e) => Flow::Failed(e),
                };
                self.engine_refresh(true);
            }
            Event::Latest(r) => {
                self.original.checking = false;
                self.original.latest = Some(r);
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
fn fetcher() -> impl oe::install::Fetch {
    oe::http::HttpsFetch
}

#[cfg(target_os = "android")]
fn fetcher() -> impl oe::install::Fetch {
    struct None;
    impl oe::install::Fetch for None {
        fn get(
            &self,
            _: &str,
            _: u64,
            _: &mut dyn FnMut(u64, Option<u64>) -> bool,
        ) -> anyhow::Result<Vec<u8>> {
            anyhow::bail!("not available on this system")
        }
    }
    None
}

fn latest() -> Result<ReleaseInfo, String> {
    oe::install::latest_release(&fetcher()).map_err(|e| format!("{e:#}"))
}

fn offer() -> Result<(ReleaseInfo, String), String> {
    let f = fetcher();
    let rel = oe::install::latest_release(&f).map_err(|e| format!("{e:#}"))?;
    if !oe::install::host_allowed(&rel.sha_url) {
        return Err("The release points outside the official download hosts.".into());
    }
    let body = oe::install::Fetch::get(&f, &rel.sha_url, 4096, &mut |_, _| true)
        .map_err(|e| format!("{e:#}"))?;
    let sha = oe::install::parse_sha256_file(&String::from_utf8_lossy(&body))
        .ok_or_else(|| "The checksum file is not a SHA-256.".to_string())?;
    Ok((rel, sha))
}

fn fetch_package(
    rel: &ReleaseInfo,
    sha: &str,
    root: &std::path::Path,
    done: &AtomicU64,
    total: &AtomicU64,
    cancel: &AtomicBool,
) -> Result<(Arc<Vec<u8>>, InstallPlan), String> {
    let f = fetcher();
    let (zip, got) = oe::install::download(&f, rel, &mut |d, t| {
        done.store(d, Relaxed);
        total.store(t.unwrap_or(0), Relaxed);
        !cancel.load(Relaxed)
    })
    .map_err(|e| format!("{e:#}"))?;
    // (the checksum the player saw is the one the package must have)
    if !got.eq_ignore_ascii_case(sha) {
        return Err("The package does not match the checksum that was shown.".into());
    }
    let plan =
        oe::install::plan_install(root, &core::data_dir(), &zip).map_err(|e| format!("{e:#}"))?;
    Ok((Arc::new(zip), plan))
}

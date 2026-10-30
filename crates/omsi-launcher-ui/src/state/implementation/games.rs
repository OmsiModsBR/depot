//! The games: started, listed, followed and stopped, and the saved situations to continue.

use super::super::domain::{msg::*, state::*};
use super::super::interface::output::core;

impl State {
    /// A game is about to start, starting (not in the list of games yet) or running.
    pub fn in_game(&self) -> bool {
        self.queued_launch.is_some()
            || self.original.starting
            || self.original.run.is_some()
            || self
                .launch_hold
                .is_some_and(|t| t.elapsed().as_secs_f32() < 15.0)
            || self.instances.iter().any(|i| i.running)
    }

    pub fn spawn_launch(&mut self, d: core::Duty) {
        self.launch_hold = Some(std::time::Instant::now());
        self.launched_pid = None;
        self.spawn(move || Msg::Launched(core::launch(&d).map_err(|e| format!("{e:#}"))));
    }
}

impl State {
    /// Without LAN play no host is looked up.
    pub fn check_join(&mut self) {}

    /// Without LAN play no host or server limits the buses.
    pub fn host_vehicles(&self) -> Option<Vec<String>> {
        None
    }

    /// Back to playing alone (the Drive page's "Leave Server").
    pub fn leave_server(&mut self) {
        self.joined_server = None;
        self.choice.lan_mode = "off".into();
        self.touched();
    }

    pub fn poll_now(&mut self) {
        if self.polling {
            return;
        }
        self.polling = true;
        self.spawn(|| Msg::Poll(core::poll().map_err(|e| format!("{e:#}"))));
    }

    pub fn log_tail(&mut self, pid: u32) {
        self.spawn(move || Msg::LogTail {
            pid,
            lines: core::log_tail(pid, 80).unwrap_or_default(),
        });
    }

    pub fn stop(&mut self, pid: u32) {
        self.stopping.insert(pid);
        self.spawn(move || Msg::Stopped {
            pid,
            result: core::stop_instance(pid).map_err(|e| format!("{e:#}")),
        });
    }
}

impl State {
    pub fn launch(&mut self) {
        self.launch_with(|_| {});
    }

    /// Only one game runs at a time: with one starting or running, a start says so and does
    /// nothing (its window may be in the tray).
    fn one_game_only(&mut self) -> bool {
        if !self.in_game() {
            return false;
        }
        self.set_status(
            "A game is running already. Close it before starting another one (its window may be in the tray).",
            true,
        );
        true
    }

    /// Start a session with the duty the choice makes, as `adjust` changes it (Maps: a start
    /// without a bus leaves the bus out of the duty, not out of the choice kept for the next).
    pub fn launch_with(&mut self, adjust: impl FnOnce(&mut core::Duty)) {
        if self.one_game_only() || !self.save_pending_settings() {
            return;
        }
        if !omsi_cfg::missing_original_essentials(std::path::Path::new(&self.config.root))
            .is_empty()
        {
            self.set_status(
                "A session needs the original OMSI 2: choose its folder under Setup first.",
                true,
            );
            return;
        }
        let mut d = self.duty();
        adjust(&mut d);
        // (with OMSI 2 chosen the game is the original one, started through OmsiLaunch)
        if self.omsi2_engine() {
            self.launch_original(&d);
            return;
        }
        self.set_status("Starting the game…", false);
        self.queued_launch = Some(d);
    }

    /// The duty as the backend takes it.
    pub fn duty(&self) -> core::Duty {
        let hof = if self.choice.hof_manual {
            self.choice.hof.clone()
        } else {
            self.default_hof()
        };
        self.choice
            .to_duty(hof, self.picked_trip(), &self.config.profile)
    }

    /// The situations to continue on the chosen map: the last one and the save slots
    /// (looked up at most every two seconds: the page asks every frame).
    pub fn saved_situations(&mut self) -> &[core::SavedSituation] {
        let fresh = self
            .last_sit
            .as_ref()
            .is_some_and(|(m, _, t)| *m == self.choice.map && t.elapsed().as_secs_f32() < 2.0);
        if !fresh {
            if self
                .last_sit
                .as_ref()
                .is_some_and(|(m, _, _)| *m != self.choice.map)
            {
                self.save_pick = 0;
            }
            let list = core::saved_situations(&self.choice.map);
            self.save_pick = self.save_pick.min(list.len().saturating_sub(1));
            self.last_sit = Some((self.choice.map.clone(), list, std::time::Instant::now()));
        }
        self.last_sit
            .as_ref()
            .map(|x| x.1.as_slice())
            .unwrap_or(&[])
    }

    /// Whether a situation to continue lies on the chosen map.
    pub fn has_last_situation(&mut self) -> bool {
        !self.saved_situations().is_empty()
    }

    /// Continue the situation chosen of the map's (`laststn.osn`, or a save slot, #341).
    pub fn launch_last_situation(&mut self) {
        if self.one_game_only() || !self.save_pending_settings() {
            return;
        }
        if self.omsi2_engine() {
            self.set_status("This start option is not available in OMSI 2 mode.", true);
            return;
        }
        let pick = self.save_pick;
        let list = self.saved_situations();
        let Some(file) = list
            .get(pick)
            .or_else(|| list.first())
            .map(|s| s.file.clone())
        else {
            self.set_status("No situation left on this map yet", true);
            return;
        };
        let mut d = self.duty();
        d.situation = Some(file.to_string_lossy().to_string());
        d.lan = Some("off".into());
        self.set_status("Continuing where you left off…", false);
        self.queued_launch = Some(d);
    }

    /// Start one of OMSI's tutorials (1..4).
    pub fn launch_tutorial(&mut self, n: usize) {
        if self.one_game_only() || !self.save_pending_settings() {
            return;
        }
        if self.omsi2_engine() {
            self.set_status("This start option is not available in OMSI 2 mode.", true);
            return;
        }
        let mut d = self.duty();
        d.tutorial = Some(n);
        d.lan = Some("off".into());
        self.set_status("Starting the tutorial…", false);
        self.queued_launch = Some(d);
    }
}

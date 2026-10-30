//! The state made, the background work started and its results taken each frame, the status
//! line and the clock.

use super::super::domain::{msg::*, squeeze::*, state::*};
use super::super::interface::output::{core, Host};
use super::settings::read_settings_file;
use super::text::*;
use std::sync::mpsc::channel;
use std::time::Instant;

impl State {
    pub fn new(host: &dyn Host) -> State {
        let (tx, rx) = channel();
        let config = core::load_config();
        let settings = core::get_settings().unwrap_or_else(|_| core::settings_from_text(None));
        host.set_language(
            settings
                .get("language")
                .and_then(|x| x.as_str())
                .unwrap_or("ENG"),
        );
        let keybindings = core::get_keybindings().unwrap_or(serde_json::Value::Null);
        let mut choice = Choice::load(mobile());
        // (at the real time, a trip picked in an earlier session has most likely left)
        if settings
            .get("use_real_time")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            choice.start_trip = None;
        }
        let mut s = State {
            config,
            maps: Vec::new(),
            vehicles: Vec::new(),
            weathers: Vec::new(),
            lines: Vec::new(),
            lines_for: (String::new(), String::new()),
            loading_content: false,
            content_first: false,
            reload_content: false,
            loading_lines: false,
            choice,
            choice_dirty: 0.0,
            last_sit: None,
            save_pick: 0,
            profiles: Vec::new(),
            profile: None,
            settings,
            settings_dirty: 0.0,
            settings_file: read_settings_file(),
            keybindings,
            keybindings_error: String::new(),
            instances: Vec::new(),
            queued_launch: None,
            launch_hold: None,
            launched_pid: None,
            started_here: None,
            crash: None,
            disconnected: None,
            jobs: Vec::new(),
            mods: None,
            mods_asked: false,
            usage: None,
            usage_asked: false,
            usage_gen: 0,
            squeeze: Squeeze::default(),
            original: Default::default(),
            content_gen: 0,
            mod_info: None,
            mod_path: String::new(),
            ibis: None,
            cmdline: String::new(),
            join: (true, String::new()),
            logs: Default::default(),
            open_logs: Default::default(),
            stopping: Default::default(),
            fresh: Default::default(),
            status: (String::new(), false, Instant::now()),
            stamp: None,
            poll_t: 0.0,
            polling: false,
            joined_server: None,
            tx,
            rx,
        };
        s.load_content();
        s.load_profiles();
        s.poll_now();
        if s.config.original_engine.enabled {
            s.engine_refresh(true);
        }
        // (a phone runs the game in the launcher's process: a crash took both, and the
        // launcher learns of it from the previous run's log)
        if let Some(c) = host.previous_run_crash() {
            s.crash = Some(c);
        }
        s
    }

    pub fn set_status(&mut self, text: impl Into<String>, err: bool) {
        let t = text.into();
        if err {
            core::log_to_file(&format!("ERROR {t}"));
        }
        self.status = (t, err, Instant::now());
    }
}

impl State {
    pub(super) fn spawn(&self, f: impl FnOnce() -> Msg + Send + 'static) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            // (a job that panics - an odd file of some mod - sent nothing, and the page
            // it was loading for said "loading" for ever: it says what went wrong instead)
            let m = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|e| {
                let why = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "an unknown error".into());
                Msg::Crashed(why)
            });
            let _ = tx.send(m);
        });
    }
}

impl State {
    /// Something of the duty changed: remember it (soon) and refresh what depends on it.
    pub fn touched(&mut self) {
        self.choice_dirty = 0.4;
    }

    /// Work done each frame: results of background work, the regular poll, saving.
    fn follow_clock(&mut self) {
        let on = |k: &str| {
            self.settings
                .get(k)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        };
        let (time, date, year) = (
            on("use_real_time"),
            on("use_real_date"),
            on("use_real_year"),
        );
        if !time && !date {
            return;
        }
        let Some((y, mo, d, h, m)) = core::local_now() else {
            return;
        };
        if time {
            self.choice.time = h * 60 + m;
        }
        if date {
            let y = if year {
                y
            } else {
                self.choice
                    .date
                    .get(..4)
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(y)
            };
            let today = format!("{y:04}-{mo:02}-{d:02}");
            if self.choice.date != today {
                self.choice.date = today;
                self.load_lines();
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        while let Ok(m) = self.rx.try_recv() {
            self.handle(m);
        }
        self.follow_clock();
        self.engine_poll();
        self.poll_t -= dt;
        if self.poll_t <= 0.0 {
            self.poll_t = 2.5;
            self.poll_now();
            self.reload_changed_settings();
        }
        if self.choice_dirty > 0.0 {
            self.choice_dirty -= dt;
            if self.choice_dirty <= 0.0 {
                self.choice.save();
                self.load_args();
                self.load_ibis();
            }
        }
        if self.settings_dirty > 0.0 {
            self.settings_dirty -= dt;
            if self.settings_dirty <= 0.0 {
                match core::save_settings(&self.settings) {
                    Ok(()) => {
                        self.settings_file = read_settings_file();
                        self.set_status("Settings saved.", false)
                    }
                    Err(e) => self.set_status(format!("{e:#}"), true),
                }
            }
        }
        if self.choice.lan_mode == "join" {
            self.check_join();
        }
        let now = Instant::now();
        self.fresh
            .retain(|_, t| now.duration_since(*t).as_secs() < 600);
    }
}

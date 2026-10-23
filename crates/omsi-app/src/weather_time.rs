//! Changing the weather (by hand, the cycle, METAR) and the clock (real-time sync, jumps).

use crate::app::App;
use crate::weather_setup::load_weather;

impl App {
    /// Change the weather in force by hand: `f` changes a copy of it, which takes the place of
    /// the weather at once (a change on its way and the weather cycle stop: this is the
    /// weather now). The sky's clouds are made again when their type changed.
    pub(crate) fn edit_weather(&mut self, f: impl FnOnce(&mut omsi_content::weather::Weather)) {
        if self.is_lan_client() {
            self.service_msg = Some(("In a LAN session the host sets the weather".into(), 3.0));
            return;
        }
        if self.metar_locked() {
            self.service_msg = Some((
                "The weather cannot be changed while the METAR sync is on".into(),
                3.0,
            ));
            return;
        }
        let mut w = self.weather.clone().unwrap_or_default();
        if w.precip.len() < 5 {
            w.precip.resize(5, 0.0);
        }
        f(&mut w);
        let brightness = crate::weather_setup::custom_weather(self.args.weather.as_deref())
            .map(|c| c.brightness)
            .unwrap_or(1.0);
        let custom =
            crate::weather_setup::CustomWeather::from_weather(&w, brightness, self.wetness);
        self.set_custom_weather(custom);
    }

    pub(crate) fn set_custom_weather(&mut self, mut custom: crate::weather_setup::CustomWeather) {
        if self.is_lan_client() {
            self.service_msg = Some(("In a LAN session the host sets the weather".into(), 3.0));
            return;
        }
        if self.metar_locked() {
            self.service_msg = Some((
                "The weather cannot be changed while the METAR sync is on".into(),
                3.0,
            ));
            return;
        }
        custom.normalize();
        self.metar_rx = None;
        self.metar_once = false;
        let spec = custom.encode();
        let to = custom.to_weather();
        let clouds_changed = self
            .weather
            .as_ref()
            .is_none_or(|w| w.clouds.0.trim() != to.clouds.0.trim());
        self.args.weather = Some(spec.clone());
        self.weather_blend = None;
        self.weather_cycle = None;
        self.wetness = custom.road_wetness;
        crate::scene::SNOW_WEATHER.store(to.snow, std::sync::atomic::Ordering::Relaxed);
        omsi_sim::host::set_ambient_weather(to.temp.0, to.temp.1);
        self.weather = Some(to);
        if clouds_changed {
            if let (Some(r), Some(scene)) = (self.renderer.as_ref(), self.scene.as_mut()) {
                crate::weather_setup::setup_sky(
                    &self.args,
                    r,
                    scene,
                    self.envir.as_ref(),
                    self.weather.as_ref(),
                );
            }
        }
        self.follow_date();
        self.share_weather_as_host(&spec);
        self.service_msg = Some(("Weather: Custom weather".into(), 2.0));
    }

    /// OMSI's weather dialog, the short way: the next weather of the Weather folder, in
    /// force at once (the roads keep their wetness until the rain changes it).
    /// How fast the clock runs: the session's in LAN play (the host's, which its time speed
    /// setting or its administration set), else the settings'.
    pub(crate) fn time_speed(&self) -> f64 {
        if self.real_time_locked() {
            return 1.0;
        }
        #[cfg(feature = "lan")]
        if let Some(l) = self.online.lan.as_ref() {
            return l.clock_speed;
        }
        self.settings.time_speed.clamp(1.0, 30.0)
    }

    pub(crate) fn next_weather(&mut self) {
        self.step_weather();
    }

    /// The next (`dir` 1) or previous (-1) weather of the Weather folder, round the ends.
    pub(crate) fn step_weather(&mut self) {
        if self.is_lan_client() {
            self.service_msg = Some(("In a LAN session the host sets the weather".into(), 3.0));
            return;
        }
        let mut files: Vec<String> = omsi_cfg::read_dir_merged("Weather")
            .into_iter()
            .filter(|p| {
                p.extension()
                    .map(|e| e.eq_ignore_ascii_case("owt"))
                    .unwrap_or(false)
            })
            .filter_map(|p| {
                p.file_name()
                    .map(|n| format!("Weather/{}", n.to_string_lossy()))
            })
            .collect();
        files.sort();
        files.dedup();
        if files.is_empty() {
            return;
        }
        let cur = self
            .args
            .weather
            .clone()
            .unwrap_or_default()
            .replace('\\', "/")
            .to_ascii_lowercase();
        let i = files
            .iter()
            .position(|f| f.to_ascii_lowercase() == cur)
            .map(|i| (i + 1) % files.len())
            .unwrap_or(0);
        self.change_weather(Some(files[i].clone()), true, 1.0);
    }

    /// Go over to weather `file` (None: the map's default) in `secs` of the day (see
    /// `weather_cycle`); a host tells the others (`share`), who come over to it the same way.
    /// The player's own choice comes at once, as in OMSI (the weather dialog loads the
    /// .owt and applies it straight away); the cycle blends it in.
    pub(crate) fn change_weather(&mut self, file: Option<String>, share: bool, secs: f32) {
        if self.metar_locked() {
            self.service_msg = Some((
                "The weather cannot be changed while the METAR sync is on".into(),
                3.0,
            ));
            return;
        }
        self.metar_rx = None;
        self.metar_once = false;
        let from = self.weather.clone().unwrap_or_default();
        self.args.weather = file.clone();
        let to = load_weather(&self.args);
        let name = to.name.clone();
        self.weather_blend = Some(crate::weather_cycle::Blend::new(from, to, secs));
        if share {
            // (a host: the others take it up with its next clock message)
            #[cfg(feature = "lan")]
            if let (Some(l), Some(f)) = (self.online.lan.as_mut(), file.as_ref()) {
                l.set_weather(f);
            }
        }
        log::info!("weather: going over to {file:?} ({name})");
        self.service_msg = Some((
            omsi_ui::tr("Weather: %{name}").replace("%{name}", &name),
            4.0,
        ));
    }

    /// The weather this frame: a change coming in, and the cycle's next one (`secs` of the
    /// day went by; in LAN play only the host's cycle runs, the others follow it).
    pub(crate) fn tick_weather(&mut self, secs: f32) {
        if let Some(b) = self.weather_blend.as_mut() {
            let (w, clouds_changed, done) = b.step(secs);
            self.weather = Some(w);
            if done {
                self.weather_blend = None;
            }
            if clouds_changed {
                if let (Some(r), Some(scene)) = (self.renderer.as_ref(), self.scene.as_mut()) {
                    crate::weather_setup::setup_sky(
                        &self.args,
                        r,
                        scene,
                        self.envir.as_ref(),
                        self.weather.as_ref(),
                    );
                }
            }
        }
        // the physical model goes on with the clock (unless a change is coming in)
        if self.weather_blend.is_none() {
            if let Some(w) = crate::weather_model::refresh(&self.clock) {
                let kind_changed = self
                    .weather
                    .as_ref()
                    .is_none_or(|old| old.clouds.0 != w.clouds.0);
                self.weather = Some(w);
                if kind_changed {
                    if let (Some(r), Some(scene)) = (self.renderer.as_ref(), self.scene.as_mut()) {
                        crate::weather_setup::setup_sky(
                            &self.args,
                            r,
                            scene,
                            self.envir.as_ref(),
                            self.weather.as_ref(),
                        );
                    }
                }
            }
        }
        if let Some(w) = self.weather.as_ref() {
            crate::weather_setup::cloud_drift_step(&mut self.cloud_drift, w, secs as f64);
        }
        let follows = self.is_lan_client();
        if follows || self.weather_blend.is_some() {
            return;
        }
        if self.metar_locked() {
            return;
        }
        let Some(c) = self.weather_cycle.as_mut() else {
            return;
        };
        c.next_in -= secs as f64;
        if c.next_in > 0.0 {
            return;
        }
        c.next_in = c.interval();
        let r = c.rand();
        let all = crate::weather_cycle::installed();
        let now = self.weather.clone().unwrap_or_default();
        let now_file = self.args.weather.clone().unwrap_or_default();
        if let Some(next) =
            crate::weather_cycle::pick(&all, &now, &now_file, self.clock.day_month().1, r)
        {
            self.change_weather(Some(next), true, 240.0);
        }
    }

    /// The weather follows the METAR report and cannot be changed (the `metar_sync` setting).
    /// In a LAN session as a client the host's weather counts: the host syncs, not us.
    pub(crate) fn metar_locked(&self) -> bool {
        self.settings.metar_sync && !self.is_lan_client()
    }

    /// The airport whose report the sync follows: the one chosen, else the one of the weather
    /// in force, else the one nearest the map.
    pub(crate) fn metar_station(&self) -> String {
        if !self.settings.metar_station.is_empty() {
            return self.settings.metar_station.to_ascii_uppercase();
        }
        match self
            .args
            .weather
            .as_deref()
            .and_then(|w| w.strip_prefix("metar:"))
        {
            Some(code) if !code.trim().is_empty() => code.trim().to_ascii_uppercase(),
            _ => crate::launcher::drive::nearest_airport(
                &self.args.root.to_string_lossy(),
                &self.args.map,
            ),
        }
    }

    /// Fetch the selected station once, without turning the ten-minute METAR sync on.
    pub(crate) fn load_metar_once(&mut self) {
        if self.is_lan_client() {
            self.service_msg = Some(("In a LAN session the host sets the weather".into(), 3.0));
            return;
        }
        let icao = self.metar_station();
        self.metar_rx = None;
        self.metar_once = true;
        let (tx, rx) = std::sync::mpsc::channel();
        self.metar_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(crate::weather_setup::try_metar(&icao));
        });
        self.service_msg = Some((
            format!("Weather: loading METAR for {}", self.metar_station()),
            4.0,
        ));
    }

    /// Ask the continuous METAR sync to fetch its selected station immediately.
    pub(crate) fn refresh_metar_now(&mut self) {
        if !self.metar_locked() {
            self.load_metar_once();
            return;
        }
        self.metar_rx = None;
        self.metar_once = false;
        self.metar_next = 0.0;
        self.service_msg = Some((
            format!("Weather: refreshing METAR for {}", self.metar_station()),
            4.0,
        ));
    }

    /// Freeze the weather currently in force into an editable custom state.
    pub(crate) fn current_weather_as_custom(&mut self) {
        if self.metar_locked() {
            self.service_msg = Some((
                "Turn METAR sync off before editing its current weather".into(),
                3.0,
            ));
            return;
        }
        let Some(w) = self.weather.as_ref() else {
            return;
        };
        let brightness = crate::weather_setup::custom_weather(self.args.weather.as_deref())
            .map(|c| c.brightness)
            .unwrap_or(1.0);
        let c = crate::weather_setup::CustomWeather::from_weather(w, brightness, self.wetness);
        self.set_custom_weather(c);
    }

    /// The METAR sync: with it on, the report is downloaded in the background (at once, then
    /// every ten minutes) and the weather goes over to it; `dt` is real seconds.
    pub(crate) fn tick_metar(&mut self, dt: f32) {
        self.share_start_metar();
        if let Some(rx) = self.metar_rx.as_ref() {
            match rx.try_recv() {
                Ok(report) => {
                    let once = self.metar_once;
                    self.metar_rx = None;
                    self.metar_once = false;
                    match report {
                        Some(w) => self.apply_metar(w),
                        None => {
                            self.service_msg =
                                Some(("Weather: no METAR report could be loaded".into(), 4.0))
                        }
                    }
                    if once {
                        return;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    let once = self.metar_once;
                    self.metar_rx = None;
                    self.metar_once = false;
                    if once {
                        self.service_msg = Some(("Weather: METAR request failed".into(), 4.0));
                        return;
                    }
                }
            }
        }
        if !self.metar_locked() {
            self.metar_next = 0.0;
            return;
        }
        self.metar_next -= dt as f64;
        if self.metar_next > 0.0 {
            return;
        }
        // (a failed download is tried again in a minute)
        self.metar_next = 60.0;
        let icao = self.metar_station();
        let (tx, rx) = std::sync::mpsc::channel();
        self.metar_rx = Some(rx);
        self.metar_once = false;
        std::thread::spawn(move || {
            let _ = tx.send(crate::weather_setup::try_metar(&icao));
        });
    }

    /// A host that started on `metar:<ICAO>` tells the players the report's values as soon as
    /// they are there (they cannot download it by the station's name: only the host syncs).
    #[cfg(feature = "lan")]
    pub(crate) fn share_start_metar(&mut self) {
        let Some(l) = self
            .online
            .lan
            .as_mut()
            .filter(|l| l.role == omsi_net::Role::Host)
        else {
            return;
        };
        if !l.weather().to_ascii_lowercase().starts_with("metar:") {
            return;
        }
        if let Some(wire) = self
            .weather
            .as_ref()
            .and_then(crate::weather_setup::report_wire)
        {
            l.set_weather(&wire);
        }
    }

    /// Go over to the weather of a METAR report that came in.
    pub(crate) fn apply_metar(&mut self, to: omsi_content::weather::Weather) {
        self.metar_next = 600.0;
        let file = to.path.to_string_lossy().to_string();
        // (what the players are told: the report's values, which they make the weather from)
        let wire = crate::weather_setup::report_wire(&to).unwrap_or_else(|| file.clone());
        let name = to.name.clone();
        let from = self.weather.clone().unwrap_or_default();
        crate::scene::SNOW_WEATHER.store(to.snow, std::sync::atomic::Ordering::Relaxed);
        omsi_sim::host::set_ambient_weather(to.temp.0, to.temp.1);
        self.args.weather = Some(file.clone());
        self.weather_cycle = None;
        self.weather_blend = Some(crate::weather_cycle::Blend::new(from, to, 60.0));
        self.share_weather_as_host(&wire);
        log::info!("weather: METAR sync, going over to {file} ({name})");
        self.service_msg = Some((
            omsi_ui::tr("Weather: %{name}").replace("%{name}", &name),
            4.0,
        ));
    }

    /// The clock follows the real time and cannot be changed (the `time_sync` setting). In a
    /// LAN session as a client the host's clock counts: the host or the server syncs, not us.
    pub(crate) fn real_time_locked(&self) -> bool {
        self.settings.time_sync && !self.is_lan_client()
    }

    /// With the real-time sync on: hold the clock to this device's date and time (a second
    /// off at most; a bigger gap - the game was paused - is jumped, the traffic's clock with it).
    pub(crate) fn sync_real_time(&mut self) {
        if !self.real_time_locked() {
            return;
        }
        let Some(real) = crate::real_time::clock_now(&self.clock) else {
            return;
        };
        let gap = crate::real_time::gap(&self.clock, &real);
        if gap.abs() < 0.25 {
            return;
        }
        self.clock.year = real.year;
        self.clock.day_of_year = real.day_of_year;
        self.clock.time = real.time;
        if let Some(tr) = self.traffic.as_mut() {
            tr.day_time += gap;
        }
        if let Some(p) = self.player.as_mut() {
            p.vehicle.host.clock = self.clock.clone();
        }
    }

    /// Move the clock by `secs` (the traffic's clock with it), as OMSI's time dialog does.
    pub(crate) fn shift_clock(&mut self, secs: f64) {
        if self.real_time_locked() {
            self.service_msg = Some((
                "The time cannot be changed while the real-time sync is on".into(),
                3.0,
            ));
            return;
        }
        let mut t = self.clock.time + secs;
        while t < 0.0 {
            t += 86400.0;
            self.clock.day_of_year = if self.clock.day_of_year > 1 {
                self.clock.day_of_year - 1
            } else {
                omsi_sim::clock::days_in_year(self.clock.year - 1)
            };
        }
        while t >= 86400.0 {
            t -= 86400.0;
            self.clock.day_of_year =
                self.clock.day_of_year % omsi_sim::clock::days_in_year(self.clock.year) + 1;
        }
        self.clock.time = t;
        if let Some(tr) = self.traffic.as_mut() {
            tr.day_time += secs;
        }
        if let Some(p) = self.player.as_mut() {
            p.vehicle.host.clock = self.clock.clone();
        }
        let h = (t / 3600.0) as u32;
        self.service_msg = Some((
            omsi_ui::tr("Clock: %{time}").replace(
                "%{time}",
                &format!("{h:02}:{:02}", ((t / 60.0) as u32) % 60),
            ),
            3.0,
        ));
        self.clock_jump += secs;
        // (held Page Up/Down: once they are let go)
        if self.clock_hold == 0.0 {
            self.timetable_after_clock_jump();
        }
    }

    /// After the clock was set by more than two minutes: the timetable's buses put out again
    /// for the new time (`Schedule::restart`).
    pub(crate) fn timetable_after_clock_jump(&mut self) {
        let jump = std::mem::take(&mut self.clock_jump);
        if jump.abs() < 120.0 {
            return;
        }
        if let (Some(s), Some(w), Some(t), Some(r), Some(scene)) = (
            self.schedule.as_mut(),
            self.world.as_ref(),
            self.traffic.as_mut(),
            self.renderer.as_ref(),
            self.scene.as_mut(),
        ) {
            let day_time = t.day_time;
            s.restart(w, t, r, scene, day_time);
        }
    }
}

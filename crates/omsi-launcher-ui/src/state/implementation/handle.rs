//! The results of the background work taken in.

use super::super::domain::{msg::*, state::*};
use super::super::interface::output::core;
use super::text::*;
use std::time::Instant;

impl State {
    pub(super) fn handle(&mut self, m: Msg) {
        match m {
            Msg::Crashed(why) => {
                log::error!("launcher: a background job stopped: {why}");
                self.loading_content = false;
                self.loading_lines = false;
                self.set_status(
                    omsi_ui::tr("Reading the content stopped on an error: %{error}")
                        .replace("%{error}", &why.to_string()),
                    true,
                );
            }
            Msg::ContentEarly { maps, weathers } => {
                if !self.content_first {
                    return;
                }
                self.maps = maps;
                self.weathers = weathers;
                self.pick_map();
                self.load_lines();
                self.set_status(
                    omsi_ui::tr("%{maps} maps - reading the buses…")
                        .replace("%{maps}", &self.maps.len().to_string()),
                    false,
                );
            }
            Msg::VehiclesRead { batch, done, total } => {
                if !self.content_first {
                    return;
                }
                self.vehicles.extend(batch);
                self.set_status(omsi_ui::tr("%{maps} maps, %{buses} buses - reading the vehicle folders: %{done} of %{total}").replace("%{maps}", &self.maps.len().to_string()).replace("%{buses}", &self.vehicles.len().to_string()).replace("%{done}", &done.to_string()).replace("%{total}", &total.to_string()), false);
            }
            Msg::Content(Ok((maps, vehicles, weathers))) => {
                let known: std::collections::HashSet<String> = self
                    .maps
                    .iter()
                    .map(|m| m.file.clone())
                    .chain(self.vehicles.iter().map(|v| v.file.clone()))
                    .chain(self.weathers.iter().map(|w| w.file.clone()))
                    .collect();
                if !known.is_empty() {
                    for f in maps
                        .iter()
                        .map(|m| &m.file)
                        .chain(vehicles.iter().map(|v| &v.file))
                        .chain(weathers.iter().map(|w| &w.file))
                    {
                        if !known.contains(f) {
                            self.fresh.insert(f.clone(), Instant::now());
                        }
                    }
                }
                self.set_status(
                    omsi_ui::tr("%{maps} maps, %{buses} buses, %{weathers} weathers")
                        .replace("%{maps}", &maps.len().to_string())
                        .replace("%{buses}", &vehicles.len().to_string())
                        .replace("%{weathers}", &weathers.len().to_string()),
                    false,
                );
                // (other files than before - another content folder chosen under Setup: the
                // Maps and Vehicles pages take the new lists even when as many came)
                let same = self
                    .maps
                    .iter()
                    .map(|m| &m.file)
                    .eq(maps.iter().map(|m| &m.file))
                    && self
                        .vehicles
                        .iter()
                        .map(|v| &v.file)
                        .eq(vehicles.iter().map(|v| &v.file));
                if !same {
                    self.content_gen += 1;
                }
                self.maps = maps;
                self.vehicles = vehicles;
                self.weathers = weathers;
                self.loading_content = false;
                // what was chosen last time, where it still exists (the phone's Drive page always
                // has a bus; on a computer none is preselected unless Vehicles picked one)
                if !self.vehicles.iter().any(|v| v.file == self.choice.bus) {
                    if !mobile() {
                        if !self.choice.bus.is_empty() {
                            self.choice.bus.clear();
                            self.choice.paint.clear();
                            self.choice.number.clear();
                            self.choice.plate.clear();
                            self.choice.hof_manual = false;
                            self.choice.hof = self.default_hof();
                        }
                    } else if let Some(v) = self.vehicles.first() {
                        self.choice.bus = v.file.clone();
                        self.choice.paint.clear();
                        self.choice.hof = self.default_hof();
                    }
                }
                self.pick_map();
                // (the first reading asked for the lines with the maps already)
                if !std::mem::take(&mut self.content_first)
                    || self.lines_for != (self.choice.map.clone(), self.choice.date.clone())
                {
                    self.load_lines();
                }
                self.load_args();
                self.load_ibis();
                self.content_done();
            }
            Msg::Content(Err(e)) => {
                self.loading_content = false;
                self.content_first = false;
                self.content_done();
                if omsi_cfg::missing_original_essentials(std::path::Path::new(&self.config.root))
                    .is_empty()
                {
                    self.set_status(
                        omsi_ui::tr("%{error}\nSet the OMSI 2 folder under Setup.")
                            .replace("%{error}", &e.to_string()),
                        true,
                    );
                } else {
                    self.set_status(root_problem(&self.config.root), true);
                }
            }
            Msg::Lines { map, date, lines } => {
                if (map, date) != self.lines_for {
                    return;
                }
                self.loading_lines = false;
                match lines {
                    Ok(l) => {
                        self.lines = l;
                        let mut note = String::new();
                        if let Some(line) = self.choice.line.clone() {
                            match self.lines.iter().find(|x| x.name == line) {
                                None => {
                                    note = omsi_ui::tr(" - line %{line} does not run on %{date}")
                                        .replace("%{line}", &line.to_string())
                                        .replace("%{date}", &date_shown(&self.choice.date));
                                    self.choice.line = None;
                                    self.choice.tour = None;
                                }
                                Some(l) => {
                                    if let Some(t) = &self.choice.tour {
                                        match l.tours.iter().find(|x| &x.number == t) {
                                            None => self.choice.tour = None,
                                            Some(t) if !t.runs => note = omsi_ui::tr(" - tour %{tour} of line %{line} does not run that day (%{days})").replace("%{tour}", &t.number).replace("%{line}", &line.to_string()).replace("%{days}", &days_text(&t.days)),
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                        self.set_status(
                            omsi_ui::tr("%{n} lines on %{date}%{note}")
                                .replace("%{n}", &self.lines.len().to_string())
                                .replace("%{date}", &date_shown(&self.choice.date))
                                .replace("%{note}", &note),
                            !note.is_empty(),
                        );
                    }
                    Err(e) => {
                        self.lines.clear();
                        self.set_status(e, true);
                    }
                }
                self.load_ibis();
            }
            Msg::Poll(r) => {
                self.polling = false;
                match r {
                    Ok(p) => {
                        for s in &p.started {
                            core::log_to_file(&format!("inbox: installing {s}"));
                        }
                        if !p.started.is_empty() {
                            self.set_status(
                                omsi_ui::tr("Installing from the Mods folder: %{list}").replace(
                                    "%{list}",
                                    &p.started
                                        .iter()
                                        .map(|x| x.rsplit('/').next().unwrap_or(x))
                                        .collect::<Vec<_>>()
                                        .join(", "),
                                ),
                                false,
                            );
                        }
                        let mut installed = false;
                        for j in &p.jobs {
                            let was = self
                                .jobs
                                .iter()
                                .find(|x| x.id == j.id)
                                .map(|x| x.finished.is_some())
                                .unwrap_or(false);
                            if j.finished.is_some() && !was && self.stamp.is_some() {
                                if j.state == "done" {
                                    installed = true;
                                    self.set_status(j.message.clone(), false);
                                } else if j.state == "failed" {
                                    self.set_status(format!("{}: {}", j.name, j.message), true);
                                }
                            }
                        }
                        self.jobs = p.jobs;
                        // a game that was running and is not any more: did it end on an error?
                        for old in self.instances.iter().filter(|i| i.running) {
                            let still = p.instances.iter().any(|n| n.pid == old.pid && n.running);
                            if !still && !self.stopping.contains(&old.pid) {
                                if let Some(why) = disconnect_of(std::path::Path::new(&old.log)) {
                                    core::log_to_file(&format!(
                                        "game {} was sent away by its server: {why}",
                                        old.pid
                                    ));
                                    self.disconnected = Some(why);
                                } else if let Some(c) = crash_of(std::path::Path::new(&old.log)) {
                                    core::log_to_file(&format!(
                                        "game {} ended on an error: {}",
                                        old.pid, c.0
                                    ));
                                    self.crash = Some(c);
                                }
                            }
                        }
                        self.instances = p.instances;
                        // the game started from here is in the list: whether it runs is known
                        // (one that ended at once left the launcher blank until the 15 s were out)
                        if game_listed(self.launched_pid, &self.instances) {
                            self.launch_hold = None;
                            self.launched_pid = None;
                        }
                        for i in &self.instances {
                            if !i.running {
                                self.stopping.remove(&i.pid);
                            }
                        }
                        // (while the lists are read the stamp moves by itself - the cache gains
                        // the folders each bus depends on - and every poll started the whole
                        // reading over on top of the one going: a big installation never
                        // finished. A change then is taken up when the reading is done.)
                        let changed = self.stamp.as_ref().map(|s| *s != p.stamp).unwrap_or(false);
                        self.stamp = if self.loading_content {
                            None
                        } else {
                            Some(p.stamp)
                        };
                        if changed || installed {
                            if self.loading_content {
                                self.reload_content = true;
                            } else {
                                self.load_content();
                            }
                            self.load_mods();
                        }
                        for pid in self.open_logs.clone() {
                            self.log_tail(pid);
                        }
                    }
                    Err(e) => core::log_to_file(&format!("poll: {e}")),
                }
            }
            Msg::Profile(Ok(p)) => {
                // A read started before another driver was selected must not put the
                // old profile back on screen after deletion.
                if p.name.eq_ignore_ascii_case(&self.config.profile) {
                    self.profile = Some(p);
                }
            }
            Msg::Profile(Err(e)) => {
                self.profile = None;
                core::log_to_file(&format!("profile: {e}"));
            }
            Msg::Profiles(p) => {
                self.profiles = p;
                if !self.profiles.contains(&self.config.profile) {
                    self.config.profile = self.profiles.first().cloned().unwrap_or_default();
                    self.profile = None;
                    let _ = core::save_config(&self.config);
                }
                if !self.config.profile.is_empty() {
                    self.load_profile();
                }
            }
            Msg::Ibis { key, info } => self.ibis = Some((key, info)),
            Msg::Args(r) => {
                self.cmdline = match r {
                    Ok(a) => format!(
                        "omsi {}",
                        a.iter()
                            .map(|x| if x.contains(' ') {
                                format!("\"{x}\"")
                            } else {
                                x.clone()
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    ),
                    Err(e) => e,
                }
            }
            Msg::Launched(Ok(l)) => {
                core::log_to_file(&format!(
                    "launched pid {} ({} other game(s) running): {}",
                    l.pid, l.others, l.command
                ));
                self.launched_pid = Some(l.pid);
                self.started_here = Some(l.pid);
                self.set_status(
                    omsi_ui::tr("Game started (process %{pid}), log %{log}%{others}")
                        .replace("%{pid}", &l.pid.to_string())
                        .replace("%{log}", &l.log.to_string())
                        .replace(
                            "%{others}",
                            &if l.others > 0 {
                                omsi_ui::tr(" - %{n} other game(s) keep running")
                                    .replace("%{n}", &l.others.to_string())
                            } else {
                                String::new()
                            },
                        ),
                    false,
                );
                self.poll_now();
            }
            Msg::Launched(Err(e)) => {
                self.launch_hold = None;
                self.set_status(e, true)
            }
            Msg::Stopped { pid, result } => {
                self.stopping.remove(&pid);
                match result {
                    Ok(true) => self.set_status(omsi_ui::tr("Game %{pid} ended by itself.").replace("%{pid}", &pid.to_string()), false),
                    Ok(false) => self.set_status(omsi_ui::tr("Game %{pid} did not end by itself and was killed - this run is not saved.").replace("%{pid}", &pid.to_string()), true),
                    Err(e) => self.set_status(e, true),
                }
                self.poll_now();
            }
            Msg::LogTail { pid, lines } => {
                self.logs.insert(pid, lines);
            }
            Msg::Mods(Ok(m)) => {
                for c in &m.cleaned {
                    core::log_to_file(&format!("cleanup: {c}"));
                }
                self.jobs = m.jobs.clone();
                self.mods = Some(m);
            }
            Msg::Mods(Err(e)) => self.set_status(e, true),
            Msg::Usage(gen, u) if gen == self.usage_gen => self.usage = Some(u),
            Msg::Usage(..) => {}
            Msg::Squeeze(gen, folders) if gen == self.squeeze.gen => {
                if !self.squeeze.chosen_set {
                    // (the Depot's own content is ticked, the original OMSI 2 is not)
                    self.squeeze.chosen_set = true;
                    self.squeeze.chosen = [true, false];
                }
                self.squeeze.folders = Some(folders);
            }
            Msg::Squeeze(..) => {}
            Msg::SqueezeDone(r) => {
                self.squeeze.job = None;
                self.squeeze.confirm = None;
                self.squeeze.done = Some(r);
                // (what was read before the job is not what is on the disk now)
                self.squeeze.folders = None;
                self.load_squeeze();
            }
            Msg::ModInfo(r) => {
                self.mod_info = Some(r);
            }
            Msg::Installed(Ok(j)) => {
                core::log_to_file(&format!("install started: {}", j.source));
                self.jobs.retain(|x| x.id != j.id);
                self.jobs.insert(0, j);
                self.poll_now();
            }
            Msg::Installed(Err(e)) => self.set_status(e, true),
            Msg::Original(e) => self.engine_event(e),
        }
    }
}

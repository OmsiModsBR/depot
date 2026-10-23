//! Typing one duty into the IBIS, key by key, watching what the scripts do.

use super::plan::*;
use super::sim::*;
use super::unit::*;
use super::*;
use crate::VehicleInstance;
use omsi_script::Vm;

/// Digit layouts to try for an entry that takes up to `room` digits.
pub(crate) fn layouts(t: &Target, room: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let ls = t.line * 100 + t.suffix;
    let mut add = |s: String| {
        if s.len() <= room && !out.contains(&s) {
            out.push(s);
        }
    };
    if let Some(r) = t.route {
        // line, suffix, course and route in one number (ALMEX L/S/K/R)
        if room >= 9 {
            add(format!("{:03}{:02}00{:02}", t.line, t.suffix, r));
            add(format!("{:03}{:02}00{:03}", t.line, t.suffix, r));
        }
    }
    add(format!("{ls:03}"));
    if let Some(r) = t.route {
        add(format!("{r:02}"));
    }
    if let Some(z) = t.terminus_code {
        add(format!("{z:03}"));
    }
    add(format!("{ls:05}"));
    // units that take the numbers as they are (the Procity's line field: "76")
    add(format!("{}", t.line));
    if let Some(r) = t.route {
        add(format!("{r}"));
    }
    if let Some(z) = t.terminus_code {
        add(format!("{z}"));
    }
    out
}

/// A running typing job: the plan found for the bus, played at a driver's pace.
pub struct Typist {
    pub(super) target: Target,
    pub(super) unit: Unit,
    pub(super) plan: Option<Plan>,
    /// Seconds into the plan, how far the plan was held back to wait for the unit, the
    /// next press and the keys to let go.
    pub(super) t: f32,
    pub(super) slip: f32,
    pub(super) waited: f32,
    pub(super) next: usize,
    pub(super) releases: Vec<(f32, String)>,
    pub(super) outcome: Option<Result<String, String>>,
    /// The plan still being looked for on a worker thread.
    pub(super) planning: Option<std::sync::mpsc::Receiver<Result<Plan, String>>>,
}

impl Typist {
    /// Learn the unit and look for a plan for `target` on a copy of the bus's state, then
    /// type it. `operable` says whether a trigger is a key the driver can reach. The trial
    /// runs take from a tenth of a second (the stock IBIS) to a second and a half (a unit
    /// that boots, wants a log-in and asks twice): in `background` they run on a worker
    /// thread and the typing starts when they are done - the keys come a little later than
    /// the trial had them, which only finds the unit further along.
    pub fn new(
        v: &VehicleInstance,
        target: Target,
        operable: &dyn Fn(&str) -> bool,
        background: bool,
    ) -> Typist {
        let p = v.ty.program.clone();
        let unit = Unit::learn(&p, operable);
        if debug() {
            log::info!("IBIS unit: {}; target {target:?}", unit.describe(&p));
        }
        let base = Sim {
            program: p.clone(),
            state: v.state.clone(),
            host: v.host.scratch(),
            vm: Vm::new(),
            t: 0.0,
            presses: Vec::new(),
            mode: unit.mode,
        };
        let mut typist = Typist {
            target: target.clone(),
            unit: unit.clone(),
            plan: None,
            t: 0.0,
            slip: 0.0,
            waited: 0.0,
            next: 0,
            releases: Vec::new(),
            outcome: None,
            planning: None,
        };
        let search = move || {
            let t0 = std::time::Instant::now();
            DEADLINE.with(|d| d.set(Some(t0 + trial_budget())));
            let found = find_plan(&unit, &target, base);
            DEADLINE.with(|d| d.set(None));
            let found = found.filter(|_| t0.elapsed() < trial_budget());
            match found {
                Some(pl) => {
                    log::info!(
                        "IBIS: {} ({} keys over {:.1} s, found in {:.0} ms)",
                        pl.summary,
                        pl.presses.len(),
                        pl.length,
                        t0.elapsed().as_secs_f64() * 1000.0
                    );
                    Ok(pl)
                }
                None => Err(format!(
                    "no way to type the duty found ({}; {:.0} ms of trials)",
                    unit.describe(&p),
                    t0.elapsed().as_secs_f64() * 1000.0
                )),
            }
        };
        if background {
            let (tx, rx) = std::sync::mpsc::channel();
            let spawned = std::thread::Builder::new()
                .name("ibis-typist".into())
                .spawn(move || {
                    let _ = tx.send(search());
                });
            match spawned {
                Ok(_) => typist.planning = Some(rx),
                Err(e) => typist.outcome = Some(Err(format!("no thread for the trials: {e}"))),
            }
        } else {
            typist.take(search());
        }
        typist
    }

    pub(super) fn take(&mut self, found: Result<Plan, String>) {
        match found {
            Ok(plan) => self.plan = Some(plan),
            Err(e) => self.outcome = Some(Err(e)),
        }
    }

    /// Whether the IBIS variables show the target.
    pub fn shows(&self, v: &VehicleInstance) -> bool {
        goal_met(&self.unit, &self.target, &|id| {
            v.state.vars.get(id as usize).copied()
        })
    }

    /// The result once the typing is over: what was typed, or why it could not be.
    pub fn outcome(&self) -> Option<&Result<String, String>> {
        self.outcome.as_ref()
    }

    /// Play the plan on the bus; false once it is over.
    pub fn tick(&mut self, v: &mut VehicleInstance, dt: f32) -> bool {
        if let Some(rx) = self.planning.as_ref() {
            match rx.try_recv() {
                Ok(found) => {
                    self.planning = None;
                    self.take(found);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.planning = None;
                    self.outcome = Some(Err("the trials ended without a result".into()));
                }
            }
        }
        if self.outcome.is_some() {
            return false;
        }
        let Some(plan) = self.plan.as_ref() else {
            return false;
        };
        self.t += dt;
        let now = self.t - self.slip;
        let mut i = 0;
        while i < self.releases.len() {
            if self.releases[i].0 <= self.t {
                let (_, key) = self.releases.swap_remove(i);
                v.trigger(&format!("{key}_off"));
            } else {
                i += 1;
            }
        }
        if let Some(press) = plan.presses.get(self.next) {
            if now >= press.at {
                let mode = self
                    .unit
                    .mode
                    .and_then(|m| v.state.vars.get(m as usize).copied());
                if press.mode.is_none() || mode == press.mode {
                    let announced = v.host.fired_file_triggers.len();
                    v.trigger(&press.key);
                    if press.quiet {
                        v.host.fired_file_triggers.truncate(announced);
                    }
                    self.releases.push((self.t + HOLD, press.key.clone()));
                    self.next += 1;
                    self.waited = 0.0;
                } else {
                    // the unit is not there yet (a real frame is not a trial frame): wait
                    self.slip += dt;
                    self.waited += dt;
                    if self.waited > SLIP {
                        let msg = format!(
                            "gave up at key {} ({}): the unit is in mode {:?}, the trial had {:?}",
                            self.next + 1,
                            press.key,
                            mode,
                            press.mode
                        );
                        self.finish(v, Err(msg));
                        return false;
                    }
                }
            }
            return true;
        }
        if now < plan.length || !self.releases.is_empty() {
            return true;
        }
        let summary = plan.summary.clone();
        if self.shows(v) {
            self.finish(v, Ok(summary));
        } else {
            let got = [self.unit.line, self.unit.route, self.unit.terminus]
                .map(|x| x.and_then(|m| v.state.vars.get(m as usize).copied()));
            self.finish(
                v,
                Err(format!(
                    "typed, but the IBIS shows line/route/terminus {got:?}"
                )),
            );
        }
        false
    }

    /// Stop typing (a new entry replaces this one): the keys still held are let go.
    pub fn abandon(&mut self, v: &mut VehicleInstance) {
        if self.outcome.is_none() {
            self.finish(v, Err("replaced by a new entry".into()));
        }
    }

    pub(super) fn finish(&mut self, v: &mut VehicleInstance, outcome: Result<String, String>) {
        for (_, key) in self.releases.drain(..) {
            v.trigger(&format!("{key}_off"));
        }
        self.outcome = Some(outcome);
    }
}

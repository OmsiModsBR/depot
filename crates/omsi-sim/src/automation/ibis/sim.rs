//! A copy of the bus's script state that the planner presses keys on.

use super::*;
use crate::VehicleHost;
use omsi_script::{State, VarId, Vm};
use std::sync::Arc;

/// A trial run on a copy of the bus's script state.
pub(super) struct Sim {
    pub(super) program: Arc<Program>,
    pub(super) state: State,
    pub(super) host: VehicleHost,
    pub(super) vm: Vm,
    pub(super) t: f32,
    pub(super) presses: Vec<Press>,
    pub(super) mode: Option<VarId>,
}

impl Clone for Sim {
    fn clone(&self) -> Sim {
        Sim {
            program: self.program.clone(),
            state: self.state.clone(),
            host: self.host.scratch(),
            vm: Vm::new(),
            t: self.t,
            presses: self.presses.clone(),
            mode: self.mode,
        }
    }
}

thread_local! {
    /// When the trials of this thread give up (see `TRIAL_BUDGET`): past it the copies of
    /// the bus stand still, so every trial fails at once and the search ends.
    pub(super) static DEADLINE: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
}

/// How long the search for a way to type may take. The stock units need a tenth of a
/// second, a unit that boots and wants a log-in a second or two; the Atron RBL of the
/// Citaro C2 (Ahlheim) - a menu the trials cannot find their way through - kept a worker
/// busy for two and a half minutes, and the displays stayed blank until the duty was set
/// directly after that. `OMSI_IBIS_BUDGET` (seconds) changes it.
pub(super) fn trial_budget() -> std::time::Duration {
    let s = omsi_cfg::flags::IBIS_BUDGET.number::<f64>().unwrap_or(10.0);
    std::time::Duration::from_secs_f64(s.max(0.1))
}

pub(super) fn out_of_time() -> bool {
    DEADLINE.with(|d| d.get().is_some_and(|t| std::time::Instant::now() >= t))
}

impl Sim {
    pub(super) fn frames(&mut self, secs: f32) {
        if out_of_time() {
            self.t += secs;
            return;
        }
        let n = (secs / STEP).round().max(1.0) as usize;
        for _ in 0..n {
            self.host.clock.advance(STEP);
            let p = self.program.clone();
            self.vm.run_frame(&p, &mut self.state, &mut self.host);
            self.t += STEP;
        }
        self.host.fired_triggers.clear();
        self.host.fired_trigger_vars.clear();
        self.host.fired_file_triggers.clear();
        self.host.messages.clear();
    }

    pub(super) fn var(&self, v: Option<VarId>) -> Option<f32> {
        v.and_then(|v| self.state.vars.get(v as usize).copied())
    }

    pub(super) fn press(&mut self, key: &str) {
        let mode = self.var(self.mode);
        let p = self.program.clone();
        self.host.fired_file_triggers.clear();
        let at = self.t;
        self.vm
            .run_trigger(&p, key, &mut self.state, &mut self.host);
        let announces = !self.host.fired_file_triggers.is_empty();
        self.presses.push(Press {
            at,
            key: key.to_string(),
            mode,
            announces,
            quiet: false,
        });
        self.frames(HOLD);
        self.vm
            .run_trigger(&p, &format!("{key}_off"), &mut self.state, &mut self.host);
        self.frames(GAP);
    }

    /// Press `key` once it does something (`effect` compares the pressed copy with one that
    /// was left alone for as long), waiting up to `wait` seconds for the unit to take it.
    pub(super) fn press_when(
        &mut self,
        key: &str,
        wait: f32,
        effect: &dyn Fn(&Sim, &Sim) -> bool,
    ) -> bool {
        let mut waited = 0.0;
        loop {
            let mut pressed = self.clone();
            pressed.press(key);
            let mut idle = self.clone();
            idle.frames(HOLD + GAP);
            if effect(&pressed, &idle) {
                *self = pressed;
                return true;
            }
            if waited >= wait {
                return false;
            }
            self.frames(0.25);
            waited += 0.25;
        }
    }
}

/// Whether two trial states differ in the given variables - in any variable when none are
/// given: a unit whose keys only store a key code (`IBIS_Taste`) that its frame macro reads
/// (Krüger's IBIS, the Procity's) says nothing statically about what typing moves.
pub(super) fn differs(a: &Sim, b: &Sim, vars: &[VarId], strs: &[u32]) -> bool {
    if vars.is_empty() && strs.is_empty() {
        return a.state.vars != b.state.vars || a.state.str_vars != b.state.str_vars;
    }
    vars.iter()
        .any(|v| a.state.vars.get(*v as usize) != b.state.vars.get(*v as usize))
        || strs
            .iter()
            .any(|v| a.state.str_vars.get(*v as usize) != b.state.str_vars.get(*v as usize))
}

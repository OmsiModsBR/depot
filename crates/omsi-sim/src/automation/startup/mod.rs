//! Putting a vehicle into service by itself (Shift+U, `--autostart`), the way a driver does
//! it: the electrics on, then the starter held until the engine runs.
//!
//! A fixed list of trigger names only fits the stock buses. Mods keep the variables of the
//! stock scripts (`elec_busbar_main_sw`, `engine_starter`, `engine_on`) but wire them to
//! their own keys: the O530 Citaro turns its ignition key one notch per press of E
//! (`cp_batterietrennschalter_toggle`: key in, electrics, ignition, starter while held) and
//! M turns it back, so the stock sequence (E, then M held) pulled the key out again. The
//! start-up therefore asks the compiled scripts which triggers can write those variables,
//! presses them one at a time and watches what happens - a toggle is pressed until the
//! electrics are on, a starter is held once it cranks and let go when the engine runs.

mod displays;
mod probe;

use displays::*;
use probe::*;

pub(crate) use probe::{engine_caught, starter_engaged};
pub use probe::{engine_rpm, engine_running, power_on};

use crate::VehicleInstance;

/// Seconds a key is held for an ordinary press, and between two presses.
pub(crate) const PRESS: f32 = 0.2;

const GAP: f32 = 0.5;

/// Presses of one trigger before the next candidate is tried.
const PRESSES_PER_TRIGGER: u32 = 4;

/// How long the starter is held at most (the NL202's cold engine needs a good 3.5 s).
pub(crate) const MAX_CRANK: f32 = 8.0;

/// LECIP/DDU scripts normally accept login after their power-up timer expires.
const DDU_LOGIN_DELAY: f32 = 7.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Power,
    /// Vehicle systems such as the Hong Kong LECIP DDU must be logged in before cranking.
    Login,
    /// Return selector-style gear controls to neutral before the starter is pressed.
    Neutral,
    Ignition,
    Crank,
    /// The starter was let go with the engine running: watch it for a moment, a cold
    /// engine that only fired a few times dies again (and is cranked once more).
    Verify,
    /// The displays that have a power switch of their own (the Procity's destination
    /// display, `sw_afisaj`): each such switch is pressed once.
    Displays,
    /// Displays still blank after that: the power buttons named for a display (the LiAZ's
    /// IBIS wants `IBIS_POWER` held while its script's frame reads the button), each held
    /// like a finger would, until the display shows something.
    DisplayPower,
    Shutdown,
    Done,
}

/// A running start-up; `tick` it every frame until it returns false.
#[derive(Debug, Clone)]
pub struct StartUp {
    pub(crate) step: Step,
    /// Seconds since the current press began.
    t: f32,
    /// The trigger being pressed and whether its `_off` was fired already.
    pressed: Option<(String, bool)>,
    /// Holding the starter (crank time so far).
    cranking: Option<f32>,
    candidates: Vec<String>,
    candidate: usize,
    presses: u32,
    cranks: u32,
    /// `DisplayPower`: the text displays still blank.
    blank: Vec<String>,
    /// `Shutdown`: the first this many candidates stop the engine (held until it dies).
    stop_n: usize,
    /// What was done, for the log and the HUD.
    pub report: Vec<String>,
}

impl StartUp {
    /// `bound` are the vehicle actions of `Inputs/keyboard.cfg` (the keys a driver presses).
    pub fn new(v: &VehicleInstance, bound: &[String]) -> StartUp {
        let mut s = StartUp {
            step: Step::Power,
            t: 0.0,
            pressed: None,
            cranking: None,
            candidates: Vec::new(),
            candidate: 0,
            presses: 0,
            cranks: 0,
            blank: Vec::new(),
            stop_n: 0,
            report: Vec::new(),
        };
        // (a bus under power whose engine has died - a crash stalled it - is started again:
        // taken for a running bus, it was "shut down" by pressing its switches round and
        // round, and every Shift+U after said "shutting down" for good)
        if engine_running(v) {
            s.enter(v, bound, Step::Shutdown);
        } else {
            s.enter(v, bound, Step::Power);
        }
        s
    }

    fn enter(&mut self, v: &VehicleInstance, bound: &[String], step: Step) {
        self.step = step;
        self.t = 0.0;
        self.pressed = None;
        self.candidate = 0;
        self.presses = 0;
        self.candidates = match step {
            Step::Power if !power_on(v) => candidates(
                v,
                POWER_VARIABLES,
                POWER_NAMES,
                bound,
                &[
                    "batter",
                    "battery",
                    "hauptschalter",
                    "main_switch",
                    "mainswitch",
                    "mainpower",
                    "master_switch",
                    "masterpower",
                ],
            ),
            Step::Ignition => semantic_triggers(
                &v.ty.program,
                bound,
                &[
                    "zuendung",
                    "zündung",
                    "zundung",
                    "ignition",
                    "schluessel",
                    "schlüssel",
                ],
            ),
            Step::Crank if !engine_running(v) => candidates(
                v,
                STARTER_VARIABLES,
                STARTER_NAMES,
                bound,
                &[
                    "engine_start",
                    "enginestart",
                    "engine_starter",
                    "starter",
                    "anlass",
                    "anlasser",
                    "motor_start",
                    "motorstart",
                    "start_stop",
                ],
            ),
            // Shutting down: what stops the engine (a stop button, where a bus has one),
            // then what switches the electrics off - the very toggles that switched them on
            // (the stock buses and most mods stop the engine with the battery switch or the
            // key) - then an ignition switch. Pressing the starter's `_off` half, as before,
            // only let go of a starter nobody held: the engine ran on, and the next Shift+U
            // said "shutting down" again.
            Step::Shutdown => {
                let mut list = semantic_triggers(
                    &v.ty.program,
                    bound,
                    // (the stock `kw_m_engineshutdown`, "Motorabstellung", held down)
                    &[
                        "enginestop",
                        "engine_stop",
                        "motorstop",
                        "motor_stop",
                        "stop_engine",
                        "motor_aus",
                        "motoraus",
                        "engine_kill",
                        "engineshutdown",
                        "engine_shutdown",
                        "motorabstell",
                        "engine_off",
                        "motor_off",
                    ],
                );
                self.stop_n = list.len();
                for n in candidates(
                    v,
                    POWER_VARIABLES,
                    POWER_NAMES,
                    bound,
                    &[
                        "batter",
                        "battery",
                        "hauptschalter",
                        "main_switch",
                        "mainswitch",
                        "mainpower",
                        "master_switch",
                        "masterpower",
                    ],
                )
                .into_iter()
                .chain(semantic_triggers(
                    &v.ty.program,
                    bound,
                    &[
                        "zuendung",
                        "zündung",
                        "zundung",
                        "ignition",
                        "schluessel",
                        "schlüssel",
                    ],
                )) {
                    if !list.contains(&n) {
                        list.push(n);
                    }
                }
                list
            }
            _ => Vec::new(),
        };
        if omsi_cfg::flags::DEBUG_STARTUP.is_set() {
            log::info!("start-up {step:?}: candidates {:?}", self.candidates);
        }
    }

    pub fn running(&self) -> bool {
        self.step != Step::Done
    }

    /// Switching the vehicle off (rather than putting it into service).
    pub fn shutting_down(&self) -> bool {
        self.step == Step::Shutdown
    }

    fn release(&mut self, v: &mut VehicleInstance) {
        if let Some((name, released)) = self.pressed.as_mut() {
            if !*released {
                if !name.ends_with("_off") {
                    v.trigger(&format!("{name}_off"));
                }
                *released = true;
            }
        }
    }

    /// Advance by `dt`; false once the start-up is over.
    pub fn tick(&mut self, v: &mut VehicleInstance, bound: &[String], dt: f32) -> bool {
        self.t += dt;
        match self.step {
            Step::Done => return false,
            Step::Power => self.step_power(v, bound),
            Step::Login => self.step_login(v, bound),
            Step::Neutral => self.step_neutral(v, bound),
            Step::Ignition => self.step_ignition(v, bound),
            Step::Crank => self.step_crank(v, bound),
            Step::Verify => self.step_verify(v),
            Step::Displays => self.step_displays(v),
            Step::DisplayPower => self.step_display_power(v),
            Step::Shutdown => self.step_shutdown(v),
        }
        if self.step == Step::Done {
            self.release(v);
            return false;
        }
        true
    }

    /// The `Power` step of [`StartUp::tick`].
    fn step_power(&mut self, v: &mut VehicleInstance, bound: &[String]) {
        if power_on(v) && self.pressed.as_ref().map(|p| p.1).unwrap_or(true) {
            if self.presses > 0 {
                self.report.push(format!(
                    "electrics on ({} x {})",
                    self.presses, self.candidates[self.candidate]
                ));
            }
            self.enter(v, bound, Step::Login);
        } else {
            self.press_loop(v, bound, Step::Ignition, "electrics");
        }
    }

    /// The `Login` step of [`StartUp::tick`].
    fn step_login(&mut self, v: &mut VehicleInstance, bound: &[String]) {
        let needs_login = v.var("ddu_login").is_some_and(|n| n < 0.5)
            && v.ty.program.trigger("ddu_login").is_some();
        if !needs_login {
            self.enter(v, bound, Step::Neutral);
        } else {
            let timer_ready = v
                .var("ddu_power_timer")
                .is_none_or(|n| n >= DDU_LOGIN_DELAY)
                || self.t >= DDU_LOGIN_DELAY + 1.0;
            if timer_ready {
                v.trigger("ddu_login");
                v.trigger("ddu_login_off");
                self.report.push("DDU login".to_string());
                self.enter(v, bound, Step::Neutral);
            }
        }
    }

    /// The `Neutral` step of [`StartUp::tick`].
    fn step_neutral(&mut self, v: &mut VehicleInstance, bound: &[String]) {
        if let Some(n) = ["automatic_N"]
            .into_iter()
            .find(|n| v.ty.program.trigger(n).is_some())
        {
            v.trigger(n);
            v.trigger(&format!("{n}_off"));
            self.report.push(format!("neutral ({n})"));
        }
        self.enter(v, bound, Step::Ignition);
    }

    /// The `Ignition` step of [`StartUp::tick`].
    fn step_ignition(&mut self, v: &mut VehicleInstance, bound: &[String]) {
        // the optional ignition switches of some mods: one press each
        if self.t >= GAP || self.candidates.is_empty() {
            if let Some(n) = self.candidates.get(self.candidate).cloned() {
                v.trigger(&n);
                v.trigger(&format!("{n}_off"));
                self.report.push(format!("ignition ({n})"));
                self.candidate += 1;
                self.t = 0.0;
            } else {
                self.enter(v, bound, Step::Crank);
            }
        }
    }

    /// The `Crank` step of [`StartUp::tick`].
    fn step_crank(&mut self, v: &mut VehicleInstance, bound: &[String]) {
        // A script may engage the starter several frames after the press. Check
        // before press_loop can release it, and include the pending start delay.
        if self.cranking.is_none()
            && self.pressed.as_ref().is_some_and(|(_, released)| !released)
            && starter_engaged(v)
        {
            self.cranking = Some(self.t);
        }
        if let Some(held) = self.cranking.as_mut() {
            *held = self.t;
            let held = *held;
            // An unknown electrical-state variable is not evidence that the power
            // went away.  Releasing immediately here was the reason a custom bus
            // whose only electrical state was `battery_on` produced one starter
            // sound and then nothing.  Stop early only when a known power state
            // explicitly dropped out.
            if engine_caught(v) || held > MAX_CRANK || matches!(power_state(v), Some(false)) {
                self.release(v);
                self.cranking = None;
                self.cranks += 1;
                let name = self
                    .candidates
                    .get(self.candidate)
                    .cloned()
                    .unwrap_or_default();
                if engine_running(v) {
                    self.report
                        .push(format!("engine started ({name} held {held:.1} s)"));
                    self.step = Step::Verify;
                    self.t = 0.0;
                } else if self.cranks >= 3 {
                    self.report.push(format!(
                        "the engine did not start ({name} held {held:.1} s, {} tries)",
                        self.cranks
                    ));
                    self.step = Step::Done;
                } else {
                    // crank again after a pause, as a driver would
                    self.t = -1.0;
                    self.pressed = None;
                }
            }
        } else if engine_running(v) && self.pressed.as_ref().map(|p| p.1).unwrap_or(true) {
            self.step = Step::Displays;
        } else {
            self.press_loop(v, bound, Step::Done, "starter");
            // Also observe an immediate start request from a newly pressed key.
            if self.pressed.as_ref().is_some_and(|(_, released)| !released) && starter_engaged(v) {
                self.cranking = Some(self.t);
            }
        }
    }

    /// The `Verify` step of [`StartUp::tick`].
    fn step_verify(&mut self, v: &mut VehicleInstance) {
        if !engine_running(v) {
            if self.cranks >= 3 {
                self.report
                    .push("the engine died again after starting".to_string());
                self.step = Step::Done;
            } else {
                self.report
                    .push("the engine died again: cranking once more".to_string());
                self.step = Step::Crank;
                self.t = -1.0;
                self.pressed = None;
                self.cranking = None;
            }
        } else if self.t > 2.0 {
            self.step = Step::Displays;
            self.t = 0.0;
        }
    }

    /// The `Displays` step of [`StartUp::tick`].
    fn step_displays(&mut self, v: &mut VehicleInstance) {
        // an automatic gearbox into D, the foot on the brake as the ZF scripts want it
        // (`(L.L.Brake) 0 >` for D: the GX7767 E500 MMC stayed in N)
        if v.ty.program.trigger("automatic_D").is_some()
            && v.var("antrieb_getr_gangvorwahl").is_none_or(|g| g < 3.5)
        {
            let brake = v.var("Brake");
            v.set_var("Brake", 1.0);
            v.trigger("automatic_D");
            v.trigger("automatic_D_off");
            if let Some(b) = brake {
                v.set_var("Brake", b);
            }
            self.report.push("gear D".to_string());
        }
        for (name, var, want) in display_switches(v) {
            for _ in 0..3 {
                v.trigger(&name);
                v.trigger(&format!("{name}_off"));
                if v.var(&var).map(|x| (x - want).abs() < 1e-3).unwrap_or(true) {
                    self.report.push(format!("display switched on ({name})"));
                    break;
                }
            }
        }
        self.blank = blank_displays(v);
        self.candidates = if self.blank.is_empty() {
            Vec::new()
        } else {
            display_power_buttons(v)
        };
        self.candidate = 0;
        self.t = 0.0;
        // (the starter's press is over: a stale one was taken for this step's own)
        self.pressed = None;
        self.step = Step::DisplayPower;
    }

    /// The `DisplayPower` step of [`StartUp::tick`].
    fn step_display_power(&mut self, v: &mut VehicleInstance) {
        match (self.candidates.get(self.candidate).cloned(), &self.pressed) {
            (None, _) => self.step = Step::Done,
            // pressed and held: the script reads the button in its frame
            (Some(name), None) => {
                v.trigger(&name);
                self.pressed = Some((name, false));
                self.t = 0.0;
            }
            (Some(_), Some((_, false))) if self.t >= 0.4 => self.release(v),
            (Some(name), Some((_, true))) if self.t >= 1.5 => {
                self.pressed = None;
                let still = blank_displays(v);
                if still.len() < self.blank.len() {
                    self.report.push(format!("display switched on ({name})"));
                }
                self.blank = still;
                self.candidate += 1;
                if self.blank.is_empty() {
                    self.step = Step::Done;
                }
            }
            _ => {}
        }
    }

    /// The `Shutdown` step of [`StartUp::tick`].
    fn step_shutdown(&mut self, v: &mut VehicleInstance) {
        let off = !power_on(v) && engine_dead(v);
        let n = self.candidates.len().max(1);
        let stopping = self.candidate % n < self.stop_n;

        match &self.pressed {
            _ if off => {
                self.release(v);
                self.report.push("switched off".to_string());
                self.step = Step::Done;
            }
            // an engine stop button is held until the engine has died (a few seconds)
            Some((_, false)) if stopping && (self.t >= 8.0 || engine_dead(v)) => self.release(v),
            Some((_, false)) if !stopping && self.t >= PRESS => self.release(v),
            // (the engine stopped already: no stop button pressed for nothing)
            None if stopping && engine_dead(v) => self.candidate += 1,
            // (a second to see what the press did: a dying engine runs down)
            Some((_, true)) if self.t >= 1.2 => {
                self.pressed = None;
                self.candidate += 1;
            }
            None => {
                // (the list twice over: the key of some mods turns one notch a press)
                let n = self.candidates.len();
                if n == 0 || self.candidate >= n * 2 {
                    self.report.push(format!(
                        "could not switch off ({} tried)",
                        self.candidates.join(", ")
                    ));
                    self.step = Step::Done;
                } else {
                    let name = self.candidates[self.candidate % n].clone();
                    v.trigger(&name);
                    self.presses += 1;
                    self.report.push(format!("pressed {name}"));
                    self.pressed = Some((name, false));
                    self.t = 0.0;
                }
            }
            _ => {}
        }
    }

    /// Press the current candidate, release it after `PRESS`, press again after `GAP`;
    /// move on to the next candidate after a few presses and to `next` when all failed.
    fn press_loop(&mut self, v: &mut VehicleInstance, bound: &[String], next: Step, what: &str) {
        if self.cranking.is_some() {
            return;
        }
        match &self.pressed {
            Some((_, false)) if self.t >= PRESS => self.release(v),
            Some((_, true)) | None if self.t >= GAP || self.pressed.is_none() && self.t >= 0.0 => {
                // (the starter longer: a bus's self-test after the key may hold it back for a
                // few seconds - the GX7767 E500 MMC's lamp test takes four - and a driver
                // presses it again once the lamps are out)
                let per_trigger = if what == "starter" {
                    PRESSES_PER_TRIGGER * 4
                } else {
                    PRESSES_PER_TRIGGER
                };
                if self.presses >= per_trigger {
                    self.candidate += 1;
                    self.presses = 0;
                }
                match self.candidates.get(self.candidate).cloned() {
                    Some(name) => {
                        v.trigger(&name);
                        self.presses += 1;
                        self.pressed = Some((name, false));
                        self.t = 0.0;
                    }
                    None => {
                        if !self.candidates.is_empty() {
                            self.report.push(format!(
                                "could not work the {what} ({} tried)",
                                self.candidates.join(", ")
                            ));
                        }
                        self.enter(v, bound, next);
                    }
                }
            }
            _ => {}
        }
    }
}

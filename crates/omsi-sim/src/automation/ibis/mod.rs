//! Typing a duty into a bus's IBIS the way a driver does it, whatever unit the bus has.
//!
//! The stock IBIS takes the line after its line key, the route after its route key, and
//! each after the entry key. Mods build very different units on the same variables: the
//! FloFix "Atron" wants the driver's PIN first and then walks from the line to the route
//! and the destination by itself; the BVG Citaro's ALMEX ticket machine boots for 17 s,
//! wants the driver's card, number and PIN, takes line, suffix, course and route as one
//! number after its L/S/K key (modes 102/103), makes the driver wait (104) and confirm the
//! destination (105), the first stop (106) and then the whole new route twice before the
//! IBIS shows it - typed the stock way, nothing of that happened and the destination had to
//! be written into the variables directly.
//!
//! So nothing here knows a unit. The compiled scripts say which triggers are the number
//! keys (`<prefix>_0` … `<prefix>_9`), which the entry key (it reads the typed number),
//! which switch the unit's mode (`IBIS_mode`), which log the driver in (the constfile
//! numbers the entry key compares with) and which advance the stop; the model and the
//! keyboard say which of them a driver can reach. A [`Plan`] is then found by trying the
//! keys out on copies of the bus's script state with the scripts' own frames running in
//! between: a key is pressed when it has an effect (a unit that is still booting or shows
//! "please wait" is waited for), the typed codes are tried in the layouts the digit
//! keys take, and entries are confirmed for as long as the unit asks - until the IBIS
//! variables show the duty. The plan is then played on the bus at a driver's pace,
//! checking on the way that the unit is where the trial run was.

mod plan;
mod program;
mod sim;
mod typist;
mod unit;

pub(crate) use typist::layouts;
pub use typist::Typist;

use omsi_script::Program;

/// What the IBIS should show when the typing is done.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Target {
    /// The IBIS line number (the depot file's route code without its last two digits).
    pub line: u32,
    pub suffix: u32,
    /// The route within the line (the route code's last two digits).
    pub route: Option<u32>,
    /// The destination code, typed when the depot file has no route for the trip.
    pub terminus_code: Option<u32>,
    /// `IBIS_RouteIndex` the route leads to (None: not checked).
    pub route_index: Option<i32>,
    /// `IBIS_TerminusIndex` the entry leads to.
    pub terminus_index: i32,
    /// The stop of the route the IBIS should stand at (the duty's next stop).
    pub stop: usize,
}

/// Seconds between two frames of a trial run.
const STEP: f32 = 0.05;

/// A key is held this long, and the next one pressed this long after it is let go.
const HOLD: f32 = 0.15;

const GAP: f32 = 0.2;

/// How long a trial waits for a unit to take a key (a boot, a "please wait" screen).
const BOOT_WAIT: f32 = 40.0;

const KEY_WAIT: f32 = 12.0;

/// How long a next-stop key is given before the next one is tried: the unit has just taken
/// the whole entry, so a key that does nothing now does nothing (the O530 Facelift's
/// mute key never moves the stop, and waiting 12 s for it at every stop took 54 s).
const STOP_KEY_WAIT: f32 = 1.0;

/// Confirmations a unit may ask for after an entry.
const MAX_CONFIRM: usize = 8;

/// How long the real run waits for the unit to reach the state the trial had before a key.
const SLIP: f32 = 6.0;

/// One key press of a plan.
#[derive(Debug, Clone, PartialEq)]
pub struct Press {
    /// Seconds after the start of the plan.
    pub at: f32,
    pub key: String,
    /// `IBIS_mode` just before the press in the trial run.
    pub mode: Option<f32>,
    /// The press announced a stop in the trial run.
    pub announces: bool,
    /// A stop passed over on the way to the duty's stop: its announcement is not played.
    pub quiet: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    pub presses: Vec<Press>,
    /// Seconds from the start to the end of the trial run.
    pub length: f32,
    /// What was done, for the log.
    pub summary: String,
}

//! The Controls page's game controllers tab: the devices `gamectrler.cfg` sets up and the ones
//! connected now (read by the game, through `Pads`), the set-up assistant and what it finds,
//! and the game's own actions a button can be given.

use super::super::interface::output::core;
use core::controllers::{Calibration, Connected};
use omsi_content::input::gamectrler::{DeviceCfg, Func};

/// The game's controllers, as the Controls page uses them: the game reads the devices (DirectInput,
/// gilrs, the system's own interfaces) and keeps them open between frames; the page asks it
/// through this trait, which the game implements.
pub trait Pads {
    /// Take the devices (for the force feedback test too: `feedback`), giving up any taken.
    fn open(&mut self, feedback: bool);
    /// Give the devices up.
    fn close(&mut self);
    /// Whether the devices are taken.
    fn is_open(&self) -> bool;
    /// The buttons pressed and let go since the last frame: (device, button, pressed).
    fn poll(&mut self) -> Vec<(String, usize, bool)>;
    /// The devices connected now.
    fn connected(&self) -> Vec<Connected>;
    /// Whether the device has buttons only (a gear shifter, a button box).
    fn buttons_only(&self, name: &str) -> bool;
    /// A force feedback pulse of the test on the device's `axis`; false: none could be given.
    fn calibration_pulse(&mut self, name: &str, axis: usize, force: f32) -> bool;
    /// The devices `gamectrler.cfg` sets up (the content folder's first, as the game reads it).
    fn read_cfg(&self) -> Vec<DeviceCfg>;
    /// Whether a device's buttons hold H-pattern gears (OMSI's `_fest` actions).
    fn held_gears(&self, devices: &[DeviceCfg]) -> bool;
}

/// The game controllers tab: the devices `gamectrler.cfg` sets up, the ones connected now,
/// the one shown, a button being waited for.
#[derive(Default)]
pub struct PadsView {
    /// The set-up assistant, while it runs.
    pub wizard: Option<Wizard>,
    pub feedback_test: bool,
    pub devices: Option<Vec<DeviceCfg>>,
    /// OMSI 2 mode: the devices of the OMSI 2 folder's own file, only shown.
    pub shown_devices: Option<Vec<DeviceCfg>>,
    /// When the Controls page was last drawn (OMSI 2 mode reads its files again after a gap).
    pub controls_seen: Option<std::time::Instant>,
    pub selected: usize,
    /// Waiting for a button of the shown device to be pressed (to add its binding).
    pub capturing: bool,
    /// A button found through "Add a button", kept visible even past the highlight.
    pub revealed_button: Option<usize>,
    pub dirty: bool,
    /// The button last pressed on the shown device and when: its line is lit, so that one
    /// sees which it is and what it does, and can give it an action there.
    pub last_pressed: Option<(usize, std::time::Instant)>,
    /// "Remove this device" clicked once, and when: a second click removes it.
    pub(crate) confirm_remove: Option<std::time::Instant>,
}

/// The set-up assistant of a device: the player lets go of everything, then turns the wheel
/// to the left and presses each pedal in turn; what moved most each time is that control
/// (and which way it runs), as OMSI's options dialog has the player choose by hand.
pub struct Wizard {
    pub step: usize,
    /// Where each axis rests, and where it stood at each step (left, throttle, brake,
    /// clutch).
    pub rest: [Option<f32>; 8],
    pub at: Vec<[Option<f32>; 8]>,
    pub error: Option<String>,
    pub calibration: Option<(std::time::Instant, Calibration)>,
    pub ff_choice: Option<bool>,
    pub test_strength: f32,
}

impl PadsView {
    pub fn cancel_feedback_test(&mut self, pads: &mut dyn Pads) {
        release_feedback(pads, &mut self.feedback_test);
        if let Some((_, test)) = self.wizard.as_mut().and_then(|w| w.calibration.as_mut()) {
            if test.result.is_none() {
                test.fail("The test was interrupted. Please try again.");
            }
        }
    }
}

/// The devices taken for the force feedback test are given back (opened again without it by
/// the next frame of the tab).
pub(crate) fn release_feedback(pads: &mut dyn Pads, active: &mut bool) {
    if *active {
        pads.close();
        *active = false;
    }
}

/// The game's own actions a controller's button can be given, besides the bus's: the doors
/// and gears of any bus, looking round while held, the bus radio while held, the cameras
/// and the views - both of OMSI's view resets, the one view's (C) and every view's (Space),
/// which a controller could not bring back to the first camera (#1167).
pub const PAD_GAME_ACTIONS: [&str; 26] = [
    "doors_all",
    "door_4",
    "door_3",
    "door_2",
    "door_1",
    "gear_up",
    "gear_down",
    "view_look_left",
    "view_look_right",
    "view_look_up",
    "view_look_down",
    "view_reset_direction",
    "view_reset_all_directions",
    "view_interiorcam_plus",
    "view_interiorcam_minus",
    "view_toggle_viewpoint",
    "view_toggle_interior",
    "view_set_driver",
    "view_set_passenger",
    "view_set_outside",
    "sim_pause",
    "screenshot",
    "quicksave",
    "toggel_mouse_ctrl",
    "toggel_ctrler",
    "voice_radio",
];

/// Hide empty slots beyond the physical buttons without changing the saved controller file.
pub fn shown_button_count(
    buttons: &[(String, String)],
    physical: usize,
    revealed: Option<usize>,
) -> usize {
    physical
        .max(
            buttons
                .iter()
                .rposition(|(action, _)| !action.trim().is_empty())
                .map(|i| i + 1)
                .unwrap_or(0),
        )
        .max(revealed.map(|i| i + 1).unwrap_or(0))
}

/// Take the device shown (`selected`) out of the list; the one below it (or the last) is
/// shown next. Its name.
pub fn remove_device(devices: &mut Vec<DeviceCfg>, selected: &mut usize) -> String {
    let name = devices.remove(*selected).name;
    *selected = (*selected).min(devices.len().saturating_sub(1));
    name
}

/// The steps of the set-up assistant (see `Wizard`): what the player is asked each time.
pub const WIZARD_STEPS: [(&str, &str); 5] = [
    ("Let go of everything", "Take your hands off the wheel and your feet off the pedals (the wheel in the middle), then press Next."),
    ("Steering", "Turn the wheel (or move the stick) all the way to the LEFT and hold it there, then press Next."),
    ("Throttle", "Press the throttle pedal all the way down and hold it, then press Next. No pedals: Skip."),
    ("Brake", "Press the brake pedal all the way down and hold it, then press Next. No brake pedal: Skip."),
    ("Clutch", "Press the clutch pedal all the way down and hold it, then press Next. No clutch: Skip."),
];

/// The axes the assistant found: `rest` where everything rested, `at` where the axes stood
/// with the wheel turned left, the throttle, the brake and the clutch pressed (all None: that
/// step skipped).
pub fn wizard_result(
    rest: &[Option<f32>; 8],
    at: &[[Option<f32>; 8]],
) -> [Option<(Func, bool)>; 8] {
    use Func;
    let mut axes: [Option<(Func, bool)>; 8] = [None; 8];
    let steer = at.first().and_then(|a| moved_most(rest, a, &[]));
    if let Some((k, delta)) = steer {
        // turned left the value falls: else the axis runs the other way
        axes[k] = Some((Func::Steering, delta > 0.0));
    }
    let taken: Vec<usize> = steer.map(|s| vec![s.0]).unwrap_or_default();
    let pedal = |i: usize, ex: &[usize]| at.get(i).and_then(|a| moved_most(rest, a, ex));
    let throttle = pedal(1, &taken);
    let brake = pedal(2, &taken);
    match (throttle, brake) {
        // one axis for both (pedals on a single axis): the throttle towards the raw maximum
        // (as in OMSI), else the axis is reversed
        (Some((kt, dt)), Some((kb, db))) if kt == kb && dt * db < 0.0 => {
            axes[kt] = Some((Func::ThrottleBrake, dt < 0.0))
        }
        _ => {
            // a pedal pressed goes towards 1
            if let Some((k, dl)) = throttle {
                axes[k] = Some((Func::Throttle, dl < 0.0));
            }
            if let Some((k, dl)) = brake.filter(|b| Some(b.0) != throttle.map(|t| t.0)) {
                axes[k] = Some((Func::Brake, dl < 0.0));
            }
        }
    }
    let mut ex = taken.clone();
    ex.extend(throttle.map(|t| t.0));
    ex.extend(brake.map(|t| t.0));
    if let Some((k, dl)) = pedal(3, &ex) {
        axes[k] = Some((Func::Clutch, dl < 0.0));
    }
    axes
}

/// The axis that moved most from `rest` to `now` (at least a sixth of its travel), not one of
/// `exclude`: (slot, how far, signed).
pub(crate) fn moved_most(
    rest: &[Option<f32>; 8],
    now: &[Option<f32>; 8],
    exclude: &[usize],
) -> Option<(usize, f32)> {
    (0..8)
        .filter(|k| !exclude.contains(k))
        .filter_map(|k| Some((k, now[k]? - rest[k].unwrap_or(0.0))))
        .filter(|(_, d)| d.abs() > 0.33)
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
}

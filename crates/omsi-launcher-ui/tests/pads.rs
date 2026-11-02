//! The Controls page's game controllers tab without a device: the set-up assistant's findings,
//! its force feedback step, a device taken out of the list and the buttons shown.

use glam::Vec2;
use omsi_content::input::gamectrler::{parse as parse_cfg, to_text as cfg_text, DeviceCfg, Func};
use omsi_launcher_lib::controllers::{Calibration, Connected, PULSE_FORCE};
use omsi_launcher_ui::window::*;
use omsi_ui::widgets::{id_of, Ui};
use omsi_ui::Rect;
/// No device at all: what the tab asked of the game is only counted.
#[derive(Default)]
struct Quiet {
    open: bool,
    opened: usize,
}

impl Pads for Quiet {
    fn open(&mut self, _feedback: bool) {
        self.open = true;
        self.opened += 1;
    }
    fn close(&mut self) {
        self.open = false;
    }
    fn is_open(&self) -> bool {
        self.open
    }
    fn poll(&mut self) -> Vec<(String, usize, bool)> {
        Vec::new()
    }
    fn connected(&self) -> Vec<Connected> {
        Vec::new()
    }
    fn buttons_only(&self, _name: &str) -> bool {
        false
    }
    fn calibration_pulse(&mut self, _name: &str, _axis: usize, _force: f32) -> bool {
        false
    }
    fn read_cfg(&self) -> Vec<DeviceCfg> {
        Vec::new()
    }
    fn held_gears(&self, _devices: &[DeviceCfg]) -> bool {
        false
    }
}

fn feedback_wizard() -> Wizard {
    Wizard {
        step: WIZARD_STEPS.len(),
        rest: [Some(0.0); 8],
        at: vec![
            [Some(-1.0), None, None, None, None, None, None, None],
            [None; 8],
            [None; 8],
            [None; 8],
        ],
        error: None,
        calibration: None,
        ff_choice: None,
        test_strength: PULSE_FORCE,
    }
}

fn click_feedback(name: &str, w: &mut Wizard, d: &mut DeviceCfg) -> Option<bool> {
    let mut ui = Ui::new();
    let size = Vec2::new(900.0, 700.0);
    let area = Rect::new(20.0, 20.0, 700.0, 600.0);
    let mut pads = Quiet::default();
    let mut active = false;
    ui.begin(size, 1.0, 0.016);
    feedback_setup(
        &mut ui,
        area,
        w,
        d,
        &[],
        None,
        &mut pads,
        &mut active,
        false,
    );
    let rect = ui.drawn[&id_of(name)];
    ui.input.mouse = rect.center();
    ui.input.pressed = true;
    ui.input.down = true;
    ui.begin(size, 1.0, 0.016);
    feedback_setup(
        &mut ui,
        area,
        w,
        d,
        &[],
        None,
        &mut pads,
        &mut active,
        false,
    );
    ui.input.pressed = false;
    ui.input.down = false;
    ui.input.released = true;
    ui.begin(size, 1.0, 0.016);
    let done = feedback_setup(
        &mut ui,
        area,
        w,
        d,
        &[],
        None,
        &mut pads,
        &mut active,
        false,
    );
    assert!(!active);
    assert!(!pads.is_open());
    done
}

#[test]
fn manual_direction_is_only_applied_on_finish_and_cancel_preserves_the_device() {
    let original = DeviceCfg {
        ff_invert: Some(true),
        ..Default::default()
    };
    let mut device = original.clone();
    let mut w = feedback_wizard();
    assert_eq!(click_feedback("wiz-ff-manual", &mut w, &mut device), None);
    assert_eq!(w.ff_choice, Some(false));
    assert_eq!(device, original);
    assert_eq!(
        click_feedback("wiz-cancel", &mut w, &mut device),
        Some(false)
    );
    assert_eq!(device, original);
    assert_eq!(
        click_feedback("wiz-ff-finish", &mut w, &mut device),
        Some(true)
    );
    assert_eq!(device.ff_invert, Some(false));
    assert_eq!(device.axes[0], Some((Func::Steering, false)));
}

#[test]
fn disconnected_wheel_cannot_start_a_hardware_test() {
    let mut device = DeviceCfg::default();
    let mut w = feedback_wizard();
    assert_eq!(click_feedback("wiz-ff-test", &mut w, &mut device), None);
    assert!(w.error.is_some());
    assert!(w.calibration.is_none());
}

#[test]
fn cancelling_feedback_releases_io_and_invalidates_the_test() {
    let mut pads = PadsView::default();
    pads.feedback_test = true;
    pads.wizard = Some(Wizard {
        step: WIZARD_STEPS.len(),
        rest: [None; 8],
        at: Vec::new(),
        error: None,
        calibration: Some((std::time::Instant::now(), Calibration::new(PULSE_FORCE))),
        ff_choice: None,
        test_strength: PULSE_FORCE,
    });
    let mut io = Quiet {
        open: true,
        ..Default::default()
    };
    pads.cancel_feedback_test(&mut io);
    assert!(!pads.feedback_test);
    assert!(!io.is_open());
    let test = &pads
        .wizard
        .as_ref()
        .unwrap()
        .calibration
        .as_ref()
        .unwrap()
        .1;
    assert!(test.result.unwrap().is_err());
    assert_eq!(pads.wizard.as_ref().unwrap().ff_choice, None);
}

#[test]
fn empty_saved_button_slots_do_not_fill_the_controller_list() {
    let mut buttons = vec![(String::new(), "0".to_string()); 131];
    buttons[10].0 = "horn".to_string();
    assert_eq!(shown_button_count(&buttons, 18, None), 18);
    assert_eq!(shown_button_count(&buttons, 18, Some(128)), 129);
}

#[test]
fn a_wheel_with_three_pedals() {
    // X the wheel; Y throttle, Z brake, Rz clutch - pedals reading 1 up, -1 down (as the
    // G25's run, "reversed")
    let rest = [
        Some(0.0),
        Some(1.0),
        Some(1.0),
        None,
        None,
        Some(1.0),
        None,
        None,
    ];
    let mut left = rest;
    left[0] = Some(-1.0);
    let mut thr = rest;
    thr[1] = Some(-1.0);
    let mut brk = rest;
    brk[2] = Some(-1.0);
    let mut clu = rest;
    clu[5] = Some(-1.0);
    let a = wizard_result(&rest, &[left, thr, brk, clu]);
    assert_eq!(a[0], Some((Func::Steering, false)));
    assert_eq!(a[1], Some((Func::Throttle, true)));
    assert_eq!(a[2], Some((Func::Brake, true)));
    assert_eq!(a[5], Some((Func::Clutch, true)));
}

#[test]
fn pedals_on_one_axis_and_a_wheel_the_other_way() {
    let rest = [Some(0.0), Some(0.0), None, None, None, None, None, None];
    let a = wizard_result(
        &rest,
        &[
            [Some(0.9), Some(0.0), None, None, None, None, None, None],
            [Some(0.0), Some(-1.0), None, None, None, None, None, None],
            [Some(0.0), Some(1.0), None, None, None, None, None, None],
            [None; 8],
        ],
    );
    assert_eq!(a[0], Some((Func::Steering, true)));
    assert_eq!(a[1], Some((Func::ThrottleBrake, true)));
}

/// A device taken out of the list is gone from the file Save writes, and the next one is
/// shown - the one below it, or above it when it was the last (#636).
#[test]
fn a_removed_device_leaves_the_file() {
    let mut devices = parse_cfg("[ctrl]\r\nSideWinder Joystick\r\n0\r\n\r\n[ctrl]\r\nLogitech G25 Racing Wheel USB\r\n1\r\n\r\n[ctrl]\r\nMOZA R3 Base\r\n0\r\n");
    let mut sel = 1;
    assert_eq!(
        remove_device(&mut devices, &mut sel),
        "Logitech G25 Racing Wheel USB"
    );
    assert_eq!(sel, 1);
    let names = |text: &str| {
        parse_cfg(text)
            .into_iter()
            .map(|d| d.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(&cfg_text(&devices)),
        ["SideWinder Joystick", "MOZA R3 Base"]
    );
    assert_eq!(remove_device(&mut devices, &mut sel), "MOZA R3 Base");
    assert_eq!(sel, 0);
    assert_eq!(remove_device(&mut devices, &mut sel), "SideWinder Joystick");
    assert_eq!(sel, 0);
    assert!(names(&cfg_text(&devices)).is_empty());
}

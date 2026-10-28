use glam::Vec2;
use omsi_content::input::gamectrler::DeviceCfg;
use omsi_content::input::gamectrler::Func;
use omsi_ui::Rect;
use openomsi_game::testing::launcher::ui::id_of;
use openomsi_game::testing::launcher::ui::Ui;
fn feedback_wizard() -> openomsi_game::testing::launcher::pages::Wizard {
    openomsi_game::testing::launcher::pages::Wizard {
        step: openomsi_game::testing::launcher::pages::WIZARD_STEPS.len(),
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
        test_strength: openomsi_game::testing::ffb_calibration::PULSE_FORCE,
    }
}

fn click_feedback(
    name: &str,
    w: &mut openomsi_game::testing::launcher::pages::Wizard,
    d: &mut DeviceCfg,
) -> Option<bool> {
    use openomsi_game::testing::launcher::pages::*;
    let mut ui = Ui::new();
    let size = Vec2::new(900.0, 700.0);
    let area = Rect::new(20.0, 20.0, 700.0, 600.0);
    let mut io = None;
    let mut active = false;
    ui.begin(size, 1.0, 0.016);
    feedback_setup(
        &mut ui,
        area,
        w,
        d,
        &[],
        None,
        &mut io,
        &mut active,
        None,
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
        &mut io,
        &mut active,
        None,
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
        &mut io,
        &mut active,
        None,
        false,
    );
    assert!(!active);
    assert!(io.is_none());
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
    let mut pads = openomsi_game::testing::launcher::pages::PadsView::default();
    pads.feedback_test = true;
    pads.wizard = Some(openomsi_game::testing::launcher::pages::Wizard {
        step: openomsi_game::testing::launcher::pages::WIZARD_STEPS.len(),
        rest: [None; 8],
        at: Vec::new(),
        error: None,
        calibration: Some((
            std::time::Instant::now(),
            openomsi_game::testing::ffb_calibration::Calibration::new(
                openomsi_game::testing::ffb_calibration::PULSE_FORCE,
            ),
        )),
        ff_choice: None,
        test_strength: openomsi_game::testing::ffb_calibration::PULSE_FORCE,
    });
    pads.cancel_feedback_test();
    assert!(!pads.feedback_test);
    assert!(pads.io.is_none());
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
fn h_pattern_gears_have_a_clear_name() {
    assert_eq!(
        openomsi_game::testing::launcher::pages::known_action("kw_s_1_fest").as_deref(),
        Some("Gear 1 (H-pattern)")
    );
    assert_eq!(
        openomsi_game::testing::launcher::pages::known_action("kw_s_R_fest").as_deref(),
        Some("Gear R (H-pattern)")
    );
}

#[test]
fn empty_saved_button_slots_do_not_fill_the_controller_list() {
    let mut buttons = vec![(String::new(), "0".to_string()); 131];
    buttons[10].0 = "horn".to_string();
    assert_eq!(
        openomsi_game::testing::launcher::pages::shown_button_count(&buttons, 18, None),
        18
    );
    assert_eq!(
        openomsi_game::testing::launcher::pages::shown_button_count(&buttons, 18, Some(128)),
        129
    );
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
    let a = openomsi_game::testing::launcher::pages::wizard_result(&rest, &[left, thr, brk, clu]);
    assert_eq!(a[0], Some((Func::Steering, false)));
    assert_eq!(a[1], Some((Func::Throttle, true)));
    assert_eq!(a[2], Some((Func::Brake, true)));
    assert_eq!(a[5], Some((Func::Clutch, true)));
}

#[test]
fn pedals_on_one_axis_and_a_wheel_the_other_way() {
    let rest = [Some(0.0), Some(0.0), None, None, None, None, None, None];
    let a = openomsi_game::testing::launcher::pages::wizard_result(
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

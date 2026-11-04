mod button;
mod cfg;
mod device_kind;
mod hot_reload;
mod right_stick_look;
mod slot;
mod stick_steering;

use omsi_content::input::gamectrler::Func;
#[test]
fn a_gilrs_event_that_panics_is_skipped() {
    let mut queue = vec![Some(2), None, Some(1)];
    let mut next = || match queue.pop().unwrap() {
        None => panic!("index out of bounds: the len is 0 but the index is 0"),
        ev => ev,
    };
    assert_eq!(
        openomsi_game::testing::controllers::next_event_caught(&mut next),
        Some(1)
    );
    assert_eq!(
        openomsi_game::testing::controllers::next_event_caught(&mut next),
        Some(2)
    );
}

#[test]
fn latching_switches_switch_back_and_stay_in_the_file() {
    assert_eq!(
        openomsi_game::testing::controllers::latch_release("blinker_left_set"),
        "blinker_off"
    );
    assert_eq!(
        openomsi_game::testing::controllers::latch_release("Blinker_Right_Set"),
        "blinker_off"
    );
    assert_eq!(
        openomsi_game::testing::controllers::latch_release("parking_brake_set"),
        "parking_brake_release"
    );
    assert_eq!(
        openomsi_game::testing::controllers::latch_release("blinker_warn_toggle"),
        "blinker_warn_toggle"
    );
    let text = "[ctrl]\r\nAER0 Truck Simulator Gear\r\n0\r\n\r\n[buttons]\r\n2\r\nblinker_warn_toggle\r\n0\r\n\r\n0\r\n\r\n[openOMSI.Latching]\r\n1\r\n";
    let devices = omsi_content::input::gamectrler::parse(text);
    assert_eq!(devices[0].latching, vec![0]);
    let again =
        omsi_content::input::gamectrler::parse(&omsi_content::input::gamectrler::to_text(&devices));
    assert_eq!(again[0].latching, vec![0]);
    assert_eq!(again[0].buttons, devices[0].buttons);
    assert!(
        omsi_content::input::gamectrler::to_text(&devices).contains("[openOMSI.Latching]\r\n1\r\n")
    );
}

#[test]
fn look_axes_are_kept_in_the_file_and_rest_at_the_centre() {
    // (openOMSI's own numbers after OMSI's five, written back as read)
    for f in [Func::LookX, Func::LookY] {
        assert_eq!(Func::from_code(Func::code(Some(f))), Some(f));
    }
    assert_eq!(Func::LABELS.len() as i32, Func::code(Some(Func::LookY)) + 2);
    assert_eq!(openomsi_game::testing::controllers::look_axis(0.1), 0.0);
    assert_eq!(openomsi_game::testing::controllers::look_axis(1.0), 1.0);
    assert_eq!(openomsi_game::testing::controllers::look_axis(-1.0), -1.0);
    assert!(
        openomsi_game::testing::controllers::look_axis(0.5) > 0.4
            && openomsi_game::testing::controllers::look_axis(0.5) < 0.5
    );
}

#[test]
fn a_stick_leaves_its_dead_zone_from_nothing() {
    use openomsi_game::testing::controllers::stick_deadzone;
    assert_eq!(stick_deadzone(0.05), 0.0);
    assert_eq!(stick_deadzone(-0.08), 0.0);
    assert!(stick_deadzone(0.0801) < 0.001);
    assert!((stick_deadzone(1.0) - 1.0).abs() < 1e-6);
    assert!((stick_deadzone(-1.0) + 1.0).abs() < 1e-6);
    assert!(stick_deadzone(-0.5) < 0.0);
}

#[test]
fn stick_steering_is_smoothed_at_any_frame_rate() {
    use openomsi_game::testing::controllers::smooth_toward;
    assert_eq!(smooth_toward(0.0, 0.7, 0.016, 0.0), 0.7);
    // never past the target
    assert!(smooth_toward(0.0, 0.5, 1.0, 0.12) <= 0.5);
    // two 8 ms frames land where one 16 ms frame does
    let twice = smooth_toward(smooth_toward(0.0, 1.0, 0.008, 0.12), 1.0, 0.008, 0.12);
    assert!((twice - smooth_toward(0.0, 1.0, 0.016, 0.12)).abs() < 1e-5);
    // a stick wobbling 0.03 either side of 0.2 every frame: the wheel holds near 0.2
    let mut v = 0.2;
    for i in 0..240 {
        v = smooth_toward(v, if i % 2 == 0 { 0.23 } else { 0.17 }, 1.0 / 60.0, 0.12);
    }
    assert!((v - 0.2).abs() < 0.01, "{v}");
}

#[test]
fn names() {
    assert!(openomsi_game::testing::controllers::names_match(
        "Logitech G25 Racing Wheel USB",
        "Logitech G25 Racing Wheel"
    ));
    assert!(!openomsi_game::testing::controllers::names_match("", "x"));
    assert!(openomsi_game::testing::controllers::names_match(
        "Кнопочная панель",
        "кнопочная  панель"
    ));
    assert!(!openomsi_game::testing::controllers::names_match(
        "Кнопочная панель",
        "Руль"
    ));
}

#[test]
fn an_xbox_named_ff_wheel_stays_visible_in_the_launcher() {
    assert!(
        openomsi_game::testing::controllers::include_direct_input_device(
            "G920 Driving Force Racing Wheel for Xbox One",
            true,
            true
        )
    );
    assert!(
        !openomsi_game::testing::controllers::include_direct_input_device(
            "Controller (Xbox One)",
            false,
            true
        )
    );
}

#[test]
fn xbox_gamepad_with_direct_input_twin_is_not_claimed_by_direct_input() {
    let xinput_pads = true;
    let is_claimed_by_di = |name: &str, ff_capable: bool| {
        openomsi_game::testing::controllers::include_direct_input_device(
            name,
            ff_capable,
            xinput_pads,
        )
    };
    assert!(!is_claimed_by_di(
        "Controller (XBOX 360 For Windows)",
        false
    ));
    assert!(!is_claimed_by_di("Xbox Wireless Controller", false));
    assert!(is_claimed_by_di(
        "Logitech G29 Driving Force Racing Wheel",
        true
    ));
    assert!(is_claimed_by_di("HORI Racing Wheel APEX", false));
    assert!(is_claimed_by_di(
        "G920 Driving Force Racing Wheel for Xbox One",
        true
    ));
}

#[test]
fn a_dualsense_on_macos_is_one_device() {
    let pad = ["PS5 Controller", "DualSense Wireless Controller"];
    let id = Some((0x054c, 0x0ce6));
    // the HID device under the system's name, or with the same ids, is gilrs's pad
    assert!(openomsi_game::testing::controllers::same_hid_device(
        "DualSense Wireless Controller",
        None,
        pad,
        id
    ));
    assert!(openomsi_game::testing::controllers::same_hid_device(
        "Wireless Controller",
        id,
        ["PS5 Controller", "PS5 Controller"],
        id
    ));
    assert!(!openomsi_game::testing::controllers::same_hid_device(
        "HORI Truck Control System",
        Some((0x0f0d, 0x01b7)),
        pad,
        id
    ));
    assert!(!openomsi_game::testing::controllers::same_hid_device(
        "Thrustmaster T.16000M",
        None,
        pad,
        None
    ));
}

#[test]
fn analog_triggers_are_no_buttons_on_macos() {
    // L2 and R2 (Rx, Ry), sliders, the simulation page's pedals and the hat
    for code in [0x10033, 0x10034, 0x10036, 0x10039, 0x200C4, 0x200C5] {
        assert!(
            openomsi_game::testing::controllers::mac_axis_code(code),
            "{code:#x}"
        );
    }
    // the button page, and a generic desktop D-pad button
    for code in [0x90001, 0x9000e, 0x10090] {
        assert!(
            !openomsi_game::testing::controllers::mac_axis_code(code),
            "{code:#x}"
        );
    }
}

#[test]
fn a_hat_switch_is_read_with_up_negative() {
    // macOS: the hat's two axes; an SDL mapping's D-pad Y comes turned round
    assert_eq!(
        openomsi_game::testing::controllers::hat_axis(0x10039, 1.0, false),
        Some((0, 1.0))
    );
    assert_eq!(
        openomsi_game::testing::controllers::hat_axis(0x1003A, 1.0, true),
        Some((1, -1.0))
    );
    assert_eq!(
        openomsi_game::testing::controllers::hat_axis(0x1003A, 1.0, false),
        Some((1, 1.0))
    );
    // Linux: ABS_HAT0X .. ABS_HAT3Y
    assert_eq!(
        openomsi_game::testing::controllers::hat_axis(0x30010, -1.0, false),
        Some((0, -1.0))
    );
    assert_eq!(
        openomsi_game::testing::controllers::hat_axis(0x30017, 1.0, false),
        Some((7, 1.0))
    );
    assert_eq!(
        openomsi_game::testing::controllers::hat_axis(0x30018, 1.0, false),
        None
    );
    assert_eq!(
        openomsi_game::testing::controllers::hat_axis(0x10030, 1.0, false),
        None
    );
}

#[test]
fn system_gamepad_buttons_work_alongside_direct_input_wheels() {
    assert!(openomsi_game::testing::controllers::use_gilrs_buttons(
        true, true
    ));
    assert!(!openomsi_game::testing::controllers::use_gilrs_buttons(
        true, false
    ));
    assert!(openomsi_game::testing::controllers::use_gilrs_buttons(
        false, false
    ));
}

#[test]
fn wheels_are_not_treated_as_system_gamepads() {
    assert!(!openomsi_game::testing::controllers::is_system_gamepad(
        "Logitech G29 Driving Force Racing Wheel",
        true
    ));
    assert!(!openomsi_game::testing::controllers::is_system_gamepad(
        "Logitech Driving Force GT",
        true
    ));
    assert!(!openomsi_game::testing::controllers::is_system_gamepad(
        "Logitech Driving Force GT",
        false
    ));
    assert!(!openomsi_game::testing::controllers::is_system_gamepad(
        "Thrustmaster T300RS",
        true
    ));
    assert!(!openomsi_game::testing::controllers::is_system_gamepad(
        "Thrustmaster T300RS",
        false
    ));

    assert!(openomsi_game::testing::controllers::is_system_gamepad(
        "Xbox 360 Controller",
        false
    ));
    assert!(openomsi_game::testing::controllers::is_system_gamepad(
        "Controller (Xbox One)",
        false
    ));
    assert!(openomsi_game::testing::controllers::is_system_gamepad(
        "Xbox Series X Controller",
        false
    ));
    assert!(!openomsi_game::testing::controllers::is_system_gamepad(
        "G920 Driving Force Racing Wheel for Xbox One",
        true
    ));
}

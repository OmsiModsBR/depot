use omsi_content::input::gamectrler::{DeviceCfg, Func};
use openomsi_game::testing;
use openomsi_game::testing::game_controller_menu::*;
use openomsi_game::testing::game_lists::{ListKind, Move};

#[test]
fn controller_menu_steps_clamp_and_wrap() {
    assert_eq!(step(0, 8, Move::Dec), 0);
    assert_eq!(step(7, 8, Move::Inc), 7);
    assert_eq!(step(7, 8, Move::Next), 0);
    assert_eq!(step(0, 41, Move::To(1.0)), 40);
    assert_eq!(button_label(0), "Button 1");
    assert_eq!(button_label(128), "Hat 1 up");
    assert!(is_controller_list(Some(&ListKind::ControllerCapture(
        "Wheel".into()
    ))));
    assert!(!is_controller_list(Some(&ListKind::Events)));
}

#[test]
fn axes_overview_keeps_eight_distinct_editors_and_live_readings() {
    let mut d = DeviceCfg::default();
    d.axes[0] = Some((Func::Steering, true));
    d.axes[5] = Some((Func::Brake, false));
    let live = testing::controllers::Connected {
        name: "Wheel".into(),
        hardware_id: None,
        axes: vec![(0, 0.25)],
        gamepad: false,
        ff: false,
        ff_capable: false,
        buttons: 0,
    };
    let rows = axis_rows(&d, Some(&live));
    assert_eq!(rows.len(), 8);
    assert_eq!(
        rows.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(),
        [
            "edit_axis 0",
            "edit_axis 1",
            "edit_axis 2",
            "edit_axis 3",
            "edit_axis 4",
            "edit_axis 5",
            "edit_axis 6",
            "edit_axis 7"
        ]
    );
    assert!(rows[0].0.starts_with("X axis: Steering · +0.25"));
    assert!(rows[5].0.starts_with("Z rotation: Brake"));
    assert!(axis_rows(&d, None)[0].0.starts_with("X axis: Steering"));
}

#[test]
fn buttons_overview_hides_empty_slots_without_renumbering_assignments() {
    let mut d = DeviceCfg::default();
    d.buttons.resize(144, (String::new(), "0".into()));
    d.buttons[7].0 = "horn".into();
    d.buttons[131].0 = "view_look_left".into();
    d.latching = vec![129];
    let names = testing::describe::ControlNames::load(std::path::Path::new("."), "ENG");
    let rows = button_rows(&d, &names);
    assert_eq!(
        rows.iter()
            .filter_map(|r| r.1.strip_prefix("button "))
            .collect::<Vec<_>>(),
        ["7", "129", "131"]
    );
    assert_eq!(rows.len(), 6);
    assert!(rows[3].0.starts_with("Button 8:"));
    assert!(rows[4].0.starts_with("Hat 1 right:"));
    assert!(rows[5].0.starts_with("Hat 1 left:"));
    assert_eq!(button_rows(&DeviceCfg::default(), &names).len(), 4);
}

#[test]
fn editor_back_navigation_preserves_the_device_and_parent_tab() {
    let wheel = "Wheel".to_string();
    assert_eq!(
        parent(&ListKind::ControllerAxis(wheel.clone(), 5)),
        Some(ListKind::Controller(wheel.clone(), 1))
    );
    for kind in [
        ListKind::ControllerCapture(wheel.clone()),
        ListKind::ControllerButtons(wheel.clone()),
        ListKind::ControllerButtonSettings(wheel.clone(), 131),
    ] {
        assert_eq!(parent(&kind), Some(ListKind::Controller(wheel.clone(), 2)));
    }
    assert_eq!(
        parent(&ListKind::ControllerButton(wheel.clone(), 131)),
        Some(ListKind::ControllerButtonSettings(wheel.clone(), 131))
    );
    assert_eq!(
        parent(&ListKind::Controller(wheel, 3)),
        Some(ListKind::ControllerDevices(0))
    );
    assert_eq!(
        parent(&ListKind::ControllerDevices(2)),
        Some(ListKind::Controls)
    );
    assert_eq!(parent(&ListKind::Events), None);
}

#[test]
fn identically_named_actions_remain_distinguishable_in_the_picker() {
    let rows = action_rows(vec![
        ("horn".into(), "Horn".into()),
        ("door_front".into(), "Door".into()),
        ("door_rear".into(), "Door".into()),
    ]);
    assert_eq!(
        rows,
        vec![
            ("Horn".into(), "bind horn".into()),
            ("Door · door_front".into(), "bind door_front".into()),
            ("Door · door_rear".into(), "bind door_rear".into())
        ]
    );
}

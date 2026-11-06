use openomsi_game::testing::player::{auto_drag_click, repair_roller_blind};
use openomsi_game::testing::script_test_vehicle;

fn blinds() -> omsi_sim::VehicleInstance {
    // Synthetic controls with both numbering conventions, deliberately different
    // from their event names. The simple frame stands in for a retaining ratchet.
    script_test_vehicle(
        "{trigger:cp_rollo_drag}\n\
            (L.L.cp_rollo_pos) (L.S.mouse_y) 100 / + (S.L.cp_rollo_pos)\n\
         {end}\n\
         {trigger:cp_rollo1_drag}\n(M.L.side)\n{end}\n\
         {macro:side}\n\
            (L.L.cp_rollo_pos1) (L.S.mouse_y) 100 / + (S.L.cp_rollo_pos1)\n\
         {end}\n\
         {trigger:cp_rollo2_drag}\n\
            (L.L.cp_rollo2_pos) (L.S.mouse_y) 100 / + (S.L.cp_rollo2_pos)\n\
         {end}\n\
         {trigger:window_drag}\n\
            (L.L.window_pos) (L.S.mouse_y) 100 / + (S.L.window_pos)\n\
         {end}\n\
         {frame}\n\
            (L.L.cp_rollo_rastpos) (L.L.cp_rollo_pos) max (S.L.cp_rollo_pos)\n\
            (L.L.cp_rollo_rastpos1) (L.L.cp_rollo_pos1) max (S.L.cp_rollo_pos1)\n\
            (L.L.cp_rollo2_rastpos) (L.L.cp_rollo2_pos) max (S.L.cp_rollo2_pos)\n\
         {end}\n",
        "cp_rollo_pos\ncp_rollo_rastpos\ncp_rollo_pos1\ncp_rollo_rastpos1\n\
         cp_rollo2_pos\ncp_rollo2_rastpos\nwindow_pos\n",
        "",
    )
}

fn drag(vehicle: &mut omsi_sim::VehicleInstance, event: &str, dy: f32) {
    vehicle.host.mouse = (0.0, dy);
    assert!(vehicle.trigger(event));
    repair_roller_blind(vehicle, event);
    vehicle.host.mouse = (0.0, 0.0);
}

#[test]
fn numbered_blind_keeps_partial_position_without_moving_another_blind() {
    for (event, pos, ratchet) in [
        ("CP_ROLLO1_DRAG", "cp_rollo_pos1", "cp_rollo_rastpos1"),
        ("cp_rollo2_drag", "cp_rollo2_pos", "cp_rollo2_rastpos"),
    ] {
        let mut vehicle = blinds();
        vehicle.set_var("cp_rollo_pos", 0.4);
        vehicle.set_var("cp_rollo_rastpos", 0.7);
        vehicle.set_var(pos, 0.8);
        vehicle.set_var(ratchet, 0.8);
        drag(&mut vehicle, event, -30.0);
        assert!((vehicle.var(pos).unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(vehicle.var(ratchet), vehicle.var(pos));
        assert_eq!(vehicle.var("cp_rollo_pos"), Some(0.4));
        assert_eq!(vehicle.var("cp_rollo_rastpos"), Some(0.7));
        // Pausing the hand delivers another drag with no movement.
        drag(&mut vehicle, event, 0.0);
        vehicle.update_scripts_only(0.0);
        assert!((vehicle.var(pos).unwrap() - 0.5).abs() < 1e-6);
    }
}

#[test]
fn canonical_blind_still_keeps_its_position() {
    let mut vehicle = blinds();
    vehicle.set_var("cp_rollo_pos", 0.8);
    vehicle.set_var("cp_rollo_rastpos", 0.8);
    vehicle.set_var("cp_rollo_pos1", 0.3);
    vehicle.set_var("cp_rollo_rastpos1", 0.6);
    drag(&mut vehicle, "cp_rollo_drag", -30.0);
    assert_eq!(vehicle.var("cp_rollo_rastpos"), vehicle.var("cp_rollo_pos"));
    assert_eq!(vehicle.var("cp_rollo_rastpos1"), Some(0.6));
    vehicle.update_scripts_only(0.0);
    assert!((vehicle.var("cp_rollo_pos").unwrap() - 0.5).abs() < 1e-6);
}

#[test]
fn retract_and_unrelated_events_leave_ratchets_to_their_scripts() {
    let mut vehicle = blinds();
    vehicle.set_var("cp_rollo_pos", 0.2);
    vehicle.set_var("cp_rollo_rastpos", 0.6);
    vehicle.set_var("cp_rollo_rastpos1", 0.8);
    for event in [
        "cp_rollo_retract",
        "cp_rollo_retract1",
        "cp_rollo1_retract",
        "cp_rollo1_retract_off",
        "cp_rollo1_off",
        "missing_rollo_drag",
    ] {
        repair_roller_blind(&mut vehicle, event);
        assert_eq!(vehicle.var("cp_rollo_rastpos"), Some(0.6));
        assert_eq!(vehicle.var("cp_rollo_rastpos1"), Some(0.8));
    }
    drag(&mut vehicle, "window_drag", 20.0);
    assert_eq!(vehicle.var("window_pos"), Some(0.2));
    assert_eq!(vehicle.var("cp_rollo_rastpos"), Some(0.6));
    assert_eq!(vehicle.var("cp_rollo_rastpos1"), Some(0.8));
}

#[test]
fn a_short_blind_gesture_does_not_become_full_travel() {
    for event in [
        "cp_rollo",
        "cp_rollo1",
        "CP_ROLLO2",
        "cp_sun_blind",
        "cp_Sun-Blind1-1",
        "C2_CP_Sonnenblende_Seite",
    ] {
        assert!(!auto_drag_click(event, false, 0.0));
        assert!(!auto_drag_click(event, false, 3.9));
    }
    assert!(auto_drag_click("cp_Fahrertuer", false, 0.0));
    assert!(!auto_drag_click("cp_Fahrertuer", false, 4.0));
    assert!(!auto_drag_click("cp_Fahrertuer", true, 0.0));
    assert!(auto_drag_click("C2_CP_Stop_Blind", false, 0.0));
}

#[test]
fn original_blinds_keep_small_upward_adjustments_after_release() {
    let Some(root) = omsi_cfg::flags::ROOT.os().map(std::path::PathBuf::from) else {
        eprintln!("skipped: OMSI_ROOT is not set");
        return;
    };
    let buses: [(&str, &[(&str, &str, &str)]); 3] = [
        (
            "Vehicles/Urbino_II/SU_18_V.bus",
            &[
                ("cp_rollo1", "cp_rollo_pos1", "cp_rollo_rastpos1"),
                ("cp_rollo2", "cp_rollo_pos2", "cp_rollo_rastpos2"),
            ],
        ),
        (
            "Vehicles/Solaris BVG/Urbino 18 Main.bus",
            &[
                ("cp_rollo", "cp_rollo_pos", "cp_rollo_rastpos"),
                ("cp_rollo1", "cp_rollo1_pos", "cp_rollo1_rastpos"),
            ],
        ),
        (
            "Vehicles/MAN_NL_NG/MAN_EN92_main.bus",
            &[("cp_rollo", "cp_rollo_pos", "cp_rollo_rastpos")],
        ),
    ];
    for (path, blinds) in buses {
        let bus = root.join(path);
        if !bus.exists() {
            eprintln!("skipped: no {}", bus.display());
            continue;
        }
        let ty = std::sync::Arc::new(omsi_sim::VehicleType::load(&root, &bus).expect(path));
        let mut vehicle =
            omsi_sim::VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()));
        vehicle.host.clock.timegap = 1.0 / 60.0;
        let program = vehicle.ty.program.clone();
        for &(blind, pos, ratchet) in blinds {
            for &(_, other_pos, other_ratchet) in blinds {
                assert!(vehicle.set_var(other_pos, 0.3));
                assert!(vehicle.set_var(other_ratchet, 0.3));
            }
            assert!(vehicle.set_var(pos, 0.6));
            assert!(vehicle.set_var(ratchet, 0.6));
            let event = format!("{blind}_drag");
            drag(&mut vehicle, &event, -2.0);
            let adjusted = vehicle.var(pos).unwrap();
            assert!(adjusted < 0.6 && adjusted > 0.59, "{path}: {blind}");
            drag(&mut vehicle, &event, 0.0);
            assert!(vehicle.trigger(&format!("{blind}_off")));
            for _ in 0..60 {
                vehicle
                    .vm
                    .run_frame(&program, &mut vehicle.state, &mut vehicle.host);
            }
            assert!(
                (vehicle.var(pos).unwrap() - adjusted).abs() < 1e-6,
                "{path}: {blind}"
            );
            assert!(
                (vehicle.var(ratchet).unwrap() - adjusted).abs() < 1e-6,
                "{path}: {blind}"
            );
            for &(other, other_pos, other_ratchet) in blinds {
                if other != blind {
                    assert!((vehicle.var(other_pos).unwrap() - 0.3).abs() < 1e-6);
                    assert!((vehicle.var(other_ratchet).unwrap() - 0.3).abs() < 1e-6);
                }
            }
            assert!(!auto_drag_click(blind, false, 2.0));
        }
    }
}

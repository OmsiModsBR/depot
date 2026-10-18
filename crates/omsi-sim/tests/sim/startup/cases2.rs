use omsi_sim::testing::startup::*;
use omsi_sim::VehicleInstance;
use omsi_sim::{SimClock, VehicleHost, VehicleType};
use std::sync::Arc;

/// A mod need not name its electrical controls like the stock buses.  This is the
/// failure mode behind the one-second starter sound: the custom starter is held until
/// its script turns `motor_on` on, rather than being released because there is no
/// `elec_busbar_main_sw` variable.
#[test]
fn starts_a_bus_with_custom_battery_and_starter_names() {
    let dir = std::env::temp_dir().join(format!(
        "omsi-startup-custom-controls-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("custom.bus"),
        "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
    )
    .unwrap();
    std::fs::write(dir.join("model.cfg"), "").unwrap();
    std::fs::write(
        dir.join("vars.txt"),
        "battery_on\nmotor_anlasser\nmotor_on\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.osc"),
        "{init}\n0 (S.L.battery_on) 0 (S.L.motor_anlasser) 0 (S.L.motor_on)\n{end}\n\
         {trigger:master_battery}\n1 (S.L.battery_on)\n{end}\n\
         {trigger:motor_anlasser}\n1 (S.L.motor_anlasser)\n{end}\n\
         {frame}\n(L.L.battery_on) (L.L.motor_anlasser) && (S.L.motor_on)\n{end}\n",
    )
    .unwrap();

    let ty = Arc::new(VehicleType::load(&dir, &dir.join("custom.bus")).unwrap());
    assert_eq!(
        ty.program.trigger_names(),
        vec!["master_battery".to_string(), "motor_anlasser".to_string()],
        "{:?}",
        ty.program.errors
    );
    let mut v = VehicleInstance::new(ty, VehicleHost::new(SimClock::default()));
    let mut start = StartUp::new(&v, &[]);
    for _ in 0..300 {
        let running = start.tick(&mut v, &[], 1.0 / 30.0);
        v.update(1.0 / 30.0);
        if !running {
            break;
        }
    }
    assert!(power_on(&v), "{:?}", start.report);
    assert!(engine_running(&v), "{:?}", start.report);
    assert!(
        start.report.iter().any(|s| s.contains("engine started")),
        "{:?}",
        start.report
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn logs_in_and_selects_neutral_before_starting() {
    let dir =
        std::env::temp_dir().join(format!("omsi-startup-login-neutral-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("login.bus"),
        "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
    )
    .unwrap();
    std::fs::write(dir.join("model.cfg"), "").unwrap();
    std::fs::write(
        dir.join("vars.txt"),
        "battery_on\nddu_login\nddu_power_timer\ncockpit_gangN\nengine_on\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.osc"),
        "{init}\n0 (S.L.battery_on) 0 (S.L.ddu_login) 0 (S.L.ddu_power_timer) 0.7 (S.L.cockpit_gangN) 0 (S.L.engine_on)\n{end}\n\\
         {trigger:master_battery}\n1 (S.L.battery_on)\n{end}\n\\
         {trigger:ddu_login}\n(L.L.ddu_power_timer) 1 > {if} 1 (S.L.ddu_login) {endif}\n{end}\n\\
         {trigger:automatic_N}\n0 (S.L.cockpit_gangN)\n{end}\n\\
         {trigger:kw_m_enginestart}\n(L.L.ddu_login) (L.L.cockpit_gangN) 0 = && {if} 1 (S.L.engine_on) {endif}\n{end}\n\\
         {frame}\n(L.L.battery_on) 1 = {if} (L.L.ddu_power_timer) (L.S.Timegap) + (S.L.ddu_power_timer) {endif}\n{end}\n",
    )
    .unwrap();

    let ty = Arc::new(VehicleType::load(&dir, &dir.join("login.bus")).unwrap());
    let mut v = VehicleInstance::new(ty, VehicleHost::new(SimClock::default()));
    let mut start = StartUp::new(&v, &[]);
    for _ in 0..900 {
        let running = start.tick(&mut v, &[], 1.0 / 30.0);
        v.update(1.0 / 30.0);
        if !running {
            break;
        }
    }
    assert!(engine_running(&v), "{:?}", start.report);
    assert!(
        start.report.iter().any(|s| s.contains("DDU login")),
        "{:?}",
        start.report
    );
    assert!(
        start.report.iter().any(|s| s.contains("neutral")),
        "{:?}",
        start.report
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn treats_real_main_as_power_before_two_stage_starter() {
    let dir = std::env::temp_dir().join(format!("omsi-startup-real-main-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("real.bus"),
        "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
    )
    .unwrap();
    std::fs::write(dir.join("model.cfg"), "").unwrap();
    std::fs::write(
        dir.join("vars.txt"),
        "elec_real_main\nelec_busbar_main\nengine_on\n",
    )
    .unwrap();
    std::fs::write(dir.join("main.osc"), "{init}\n0 (S.L.elec_real_main) 0 (S.L.elec_busbar_main) 0 (S.L.engine_on)\n{end}\n\\
         {trigger:master_battery}\n1 (S.L.elec_real_main)\n{end}\n\\
         {trigger:kw_m_enginestart}\n(L.L.elec_busbar_main) 0 = {if} 1 (S.L.elec_busbar_main) {else} 1 (S.L.engine_on) {endif}\n{end}\n\\
         {frame}\n{end}\n").unwrap();
    let ty = Arc::new(VehicleType::load(&dir, &dir.join("real.bus")).unwrap());
    let mut v = VehicleInstance::new(ty, VehicleHost::new(SimClock::default()));
    let mut start = StartUp::new(&v, &[]);
    for _ in 0..300 {
        if !start.tick(&mut v, &[], 1.0 / 30.0) {
            break;
        }
        v.update(1.0 / 30.0);
    }
    assert!(engine_running(&v), "{:?}", start.report);
    std::fs::remove_dir_all(&dir).ok();
}

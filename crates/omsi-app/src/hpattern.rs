//! H-pattern gate actions, including vehicles exposing only the held (`_fest`) variant.

pub(crate) fn is_gate(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let Some(gate) = name.strip_prefix("kw_s_") else {
        return false;
    };
    let gate = gate.strip_suffix("_fest").unwrap_or(gate);
    gate == "r" || gate == "n" || gate.parse::<u32>().is_ok()
}

pub fn has_held_bindings(cfg: &[crate::controllers::DeviceCfg]) -> bool {
    cfg.iter().any(|d| {
        d.buttons
            .iter()
            .any(|(name, _)| is_gate(name) && name.to_ascii_lowercase().ends_with("_fest"))
    })
}

pub(crate) fn resolve(program: &omsi_script::Program, name: &str) -> Option<String> {
    if !is_gate(name) {
        return None;
    }
    if program.trigger(name).is_some() {
        return Some(name.to_string());
    }
    let lower = name.to_ascii_lowercase();
    let other = match lower.strip_suffix("_fest") {
        Some(base) => base.to_string(),
        None => format!("{lower}_fest"),
    };
    program.trigger(&other).map(|_| other)
}

/// Returns None for non-manual actions, so automatic gearboxes keep their own scripts.
/// Release the exact supported gate first; neutral is an extra action only when enabled.
pub fn action(
    vehicle: &mut omsi_sim::VehicleInstance,
    name: &str,
    pressed: bool,
    momentary: bool,
) -> Option<bool> {
    if !is_gate(name) || !vehicle.ty.program.manual_gearbox() {
        return None;
    }
    let name = resolve(&vehicle.ty.program, name)?;
    let done = vehicle.trigger(&if pressed {
        name.clone()
    } else {
        format!("{name}_off")
    });
    if pressed || !momentary {
        return Some(done || vehicle.ty.program.trigger(&name).is_some());
    }
    for neutral in ["kw_s_N", "kw_s_N_fest"] {
        if vehicle.ty.program.trigger(neutral).is_none() {
            continue;
        }
        // A neutral trigger can require the clutch just like a numbered gate. Operate it
        // briefly when automatic clutch is enabled; leave a physical clutch untouched.
        let clutch = vehicle.var("Clutch");
        let assist = vehicle.host.auto_clutch >= 0.5
            && !vehicle
                .ty
                .program
                .reads_sys(omsi_script::SysVar::AutoClutch);
        if assist {
            vehicle.set_var("Clutch", 1.0);
        }
        let selected = vehicle.trigger(neutral);
        if selected {
            vehicle.trigger(&format!("{neutral}_off"));
        }
        if assist {
            if let Some(clutch) = clutch {
                vehicle.set_var("Clutch", clutch);
            }
        }
        if selected {
            return Some(true);
        }
    }
    Some(done)
}

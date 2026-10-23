//! The displays of the bus: which are dark, and the buttons that switch them on.

use crate::VehicleInstance;

/// Switches that do nothing but turn a display on: a clickable trigger whose whole job is
/// toggling the variable a display mesh (one showing a script or text texture) is
/// `[visible]` by, while that display is dark. The Procity's destination display has its
/// own button on the dashboard (`sw_afisaj`), which a driver presses when taking the bus
/// over; Shift+U does it too. Menus, modding switches and anything that is not a plain
/// on/off toggle of that one variable are left alone.
pub(super) fn display_switches(v: &VehicleInstance) -> Vec<(String, String, f32)> {
    let p = &v.ty.program;
    let clickable: Vec<String> =
        v.ty.model
            .meshes
            .iter()
            .filter_map(|m| {
                m.mouse_event
                    .as_ref()
                    .map(|e| e.trim().to_ascii_lowercase())
            })
            .collect();
    let mut out: Vec<(String, String, f32)> = Vec::new();
    // `(L.L.x) ! (S.L.x)`: a plain toggle of `x`
    let toggles_var = |t: &str, id: omsi_script::VarId| {
        p.trigger(t).map(|b| {
            p.blocks[b as usize].ops.windows(3).any(|w| {
                matches!(w, [omsi_script::Op::Load(a), omsi_script::Op::Not, omsi_script::Op::Store(c)] if *a == id && *c == id)
            })
        }).unwrap_or(false)
    };
    // Displays whose script only writes them while a switch is on: the LiAZ's Annax matrix
    // wants `matrix_power` (its "Matrix" switch) and the 12 V circuit (`elec_12v`). The
    // switches are the clickable plain toggles among the variables read by the blocks that
    // write a display's string (a `[texttexture]`) or draw on a script texture.
    let display_strings: Vec<u32> =
        v.ty.model
            .text_textures
            .iter()
            .filter_map(|t| p.text_texture_var(&t.variable))
            .map(|id| id as u32)
            .collect();
    let mut read_by_displays: Vec<omsi_script::VarId> = Vec::new();
    for b in &p.blocks {
        let writes_display = b.ops.iter().any(|op| match op {
            omsi_script::Op::StoreStr(id) => display_strings.contains(&(*id as u32)),
            omsi_script::Op::Callback(n) => p.name(*n).to_ascii_lowercase().starts_with("st"),
            _ => false,
        });
        if writes_display {
            for op in &b.ops {
                if let omsi_script::Op::Load(id) = op {
                    if !read_by_displays.contains(id) {
                        read_by_displays.push(*id);
                    }
                }
            }
        }
    }
    // the dashboard displays read every switch they show the state of; only a switch named
    // as the display's power (or the 12 V circuit displays hang on) is one to turn on
    let display_power = ["matrix", "anzeige", "display", "afisaj", "tablo", "12v"];
    for id in read_by_displays {
        let var = p.var_names[id as usize].clone();
        let lower = var.to_ascii_lowercase();
        if !display_power.iter().any(|w| lower.contains(w))
            || v.var(&var).map(|x| x.abs() > 1e-3).unwrap_or(true)
        {
            continue;
        }
        for t in p.triggers_setting(&var) {
            if clickable.contains(&t) && toggles_var(&t, id) && !out.iter().any(|(n, _, _)| *n == t)
            {
                out.push((t, var.clone(), 1.0));
            }
        }
    }
    for m in &v.ty.model.meshes {
        let Some((var, want)) = m.visible.as_ref() else {
            continue;
        };
        if (*want - 1.0).abs() > 1e-3 || m.mouse_event.is_some() {
            continue;
        }
        let shows = m.materials.iter().any(|mat| {
            mat.use_script_texture.is_some()
                || mat.use_text_texture.is_some()
                || mat
                    .transmap
                    .as_deref()
                    .map(|t| t.trim().starts_with("\\S:"))
                    .unwrap_or(false)
                || mat.texture.trim().starts_with("\\S:")
        });
        if !shows || v.var(var).map(|x| (x - want).abs() < 1e-3).unwrap_or(true) {
            continue;
        }
        // a variable that also shows something else at another value chooses between
        // alternatives (the C2's small or big ATRON, an odometer's modes): not a power switch
        let selects = v.ty.model.meshes.iter().any(|o| {
            o.visible
                .as_ref()
                .map(|(n, x)| n.eq_ignore_ascii_case(var) && (*x - want).abs() > 1e-3)
                .unwrap_or(false)
        });
        if selects {
            continue;
        }
        let Some(id) = p.var(var) else { continue };
        for t in p.triggers_setting(var) {
            if !clickable.contains(&t) || out.iter().any(|(n, _, _)| *n == t) {
                continue;
            }
            // `(L.L.x) ! (S.L.x)`: a plain toggle of the display's variable
            let Some(b) = p.trigger(&t) else { continue };
            let ops = &p.blocks[b as usize].ops;
            let toggles = ops.windows(3).any(|w| {
                matches!(w, [omsi_script::Op::Load(a), omsi_script::Op::Not, omsi_script::Op::Store(c)] if *a == id && *c == id)
            });
            if toggles {
                out.push((t, var.clone(), *want));
            }
        }
    }
    out
}

/// The `[texttexture]` strings of the model that show nothing.
pub(super) fn blank_displays(v: &VehicleInstance) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in &v.ty.model.text_textures {
        // the field may be a built-in string's number (see `Program::text_texture_var`); the
        // variable it names is what a caller has to look at, not the number in the file
        let Some(id) = v.ty.program.text_texture_var(&t.variable) else {
            continue;
        };
        let Some(name) = v.ty.program.str_var_names.get(id as usize).cloned() else {
            continue;
        };
        if v.str_var(&name).trim().is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Clickable buttons named as a display's power: `IBIS_POWER`, `matrix_on`, `anzeige_ein` ...
pub(super) fn display_power_buttons(v: &VehicleInstance) -> Vec<String> {
    let p = &v.ty.program;
    let displays = [
        "ibis",
        "matrix",
        "anzeige",
        "display",
        "afisaj",
        "tablo",
        "informator",
        "annax",
        "zza",
        "lcd",
    ];
    let power = [
        "power", "pwr", "strom", "_on", "_ein", "onoff", "on_off", "an_aus", "toggle", "_sw",
    ];
    let mut out: Vec<String> = Vec::new();
    for m in &v.ty.model.meshes {
        let Some(e) = m.mouse_event.as_ref().map(|e| e.trim().to_string()) else {
            continue;
        };
        let lower = e.to_ascii_lowercase();
        if lower.ends_with("_off")
            || !displays.iter().any(|w| lower.contains(w))
            || !power.iter().any(|w| lower.contains(w))
            || p.trigger(&e).is_none()
            || out.iter().any(|o| o.eq_ignore_ascii_case(&e))
        {
            continue;
        }
        out.push(e);
    }
    out.sort();
    out
}

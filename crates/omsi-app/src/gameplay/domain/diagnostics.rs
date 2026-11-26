//! Why a bus does not move, and the key a player has rebound for an action: the rules the
//! window's HUD and the offscreen log both read.

/// The player's key for `action` when `bindings` (the vehicle's `Inputs/keyboard.cfg`) bind it
/// to another key than OMSI's own file does; `None` for the stock key or no key at all.
pub fn rebound_key(bindings: &[omsi_content::KeyBinding], action: &str) -> Option<String> {
    use omsi_content::input::KEY_HOLD;
    let b = bindings
        .iter()
        .find(|b| b.action.eq_ignore_ascii_case(action) && b.scan_code != 0)?;
    let stock = omsi_content::input::stock_keys::STOCK_KEYS
        .iter()
        .any(|(a, s, m)| {
            a.eq_ignore_ascii_case(action)
                && *s == b.scan_code
                && (m & !KEY_HOLD) == (b.modifier & !KEY_HOLD)
        });
    (!stock).then(|| crate::input::keys::key_name(b.scan_code as i64, b.modifier as i64))
}

/// Why the bus is not moving although the throttle is pressed: the things a driver checks
/// first (empty when it moves or nothing is pressed). The window's HUD shows them, an
/// offscreen run logs them. `key(action)` names the player's own key for an action of
/// `Inputs/keyboard.cfg` when it is not the stock one (see [`rebound_key`]).
pub(crate) fn standing_reasons(
    v: &omsi_sim::VehicleInstance,
    key: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut lines = Vec::new();
    let throttle = v.var("throttle").unwrap_or(0.0) > 0.05;
    let slow = v.motion().velocity_kmh().abs() < 3.0;
    if !(throttle && slow) {
        return lines;
    }
    if !omsi_sim::startup::engine_running(v) {
        let m = key("kw_m_enginestart").unwrap_or_else(|| "M".into());
        // (the electrics' key named as the player set it too, not always E - #461)
        let e = key("cp_batterietrennschalter_toggle").unwrap_or_else(|| "E".into());
        lines.push(omsi_ui::tr("The engine is off  (%{electrics} electrics, %{starter} starter; mod buses with an ignition key turn it with %{electrics}: press again and hold. Shift+U does it all)").replace("%{electrics}", &e).replace("%{starter}", &m));
    } else if v
        .var("antrieb_getr_gangwahl")
        .map(|g| (g - 1.0).abs() < 0.1)
        .or_else(|| {
            v.var("cockpit_gangwahltaster")
                .map(|g| (g - 1.0).abs() < 0.1)
        })
        .unwrap_or(false)
    {
        // gear position 1 is N in the stock gearboxes and the mods built on them
        let d = key("automatic_D")
            .unwrap_or_else(|| omsi_ui::tr("D (Shift+D when W A S D drive)").into_owned());
        lines.push(omsi_ui::tr("The gearbox is in N: press %{key}, or click it. Buses like the Citaro take D only with the brake held").replace("%{key}", &d));
    } else if v.var("antrieb_getr_gangwahl").is_none()
        && v.var("cockpit_gangwahltaster").is_none()
        && v.var("antrieb_getr_gang")
            .map(|g| g.abs() < 0.1)
            .unwrap_or(false)
    {
        // a manual gearbox (the stock F90 and T3, the Sprinters, PAZ and LAZ mods): gear 0 is
        // neutral, and the keyboard's `kw_s_1` … are the digit keys
        lines.push("Manual gearbox in neutral: 1-5 select a gear (R reverse, N neutral); the clutch works itself unless auto_clutch=0".to_string());
    }
    let parking = v
        .var("bremse_feststell")
        .or_else(|| v.var("parking_brake"))
        .unwrap_or(0.0)
        > 0.5;
    if parking {
        let k = key("parking_brake_toggle").unwrap_or_else(|| ".".into());
        lines.push(omsi_ui::tr("Parking brake is on  (%{key} releases it)").replace("%{key}", &k));
    }
    // a bus that stood long enough to lose its air (the stock scripts start with 4-9 bar):
    // the spring brake is released by air (`bremse_p_Brzyl_FBA`, absolute; the stock
    // brake curve still holds below 6.5 bar), so it holds until the compressor has
    // filled the tanks, whatever the parking brake lever says
    if !parking && lines.is_empty() {
        let fba_val = v
            .var("bremse_p_Brzyl_FBA")
            .or_else(|| v.var("spring_brake_pressure"));
        if let Some(fba) = fba_val.filter(|p| *p < 6.0e5) {
            let tanks: Vec<f32> = (1..=4)
                .filter_map(|i| {
                    v.var(&format!("bremse_p_Tank0{i}"))
                        .or_else(|| v.var(&format!("air_tank_{i}")))
                })
                .collect();
            let low = tanks.iter().copied().fold(f32::MAX, f32::min);
            // (with the tanks full the spring brake is held by the bus's own parking brake,
            // whatever its script calls it, not by a want of air)
            let full = !tanks.is_empty() && low >= 6.0e5;
            if !full {
                let spring = format!("{:.1}", fba / 1e5);
                lines.push(if tanks.is_empty() {
                    omsi_ui::tr("Air pressure is low (spring brake %{spring} bar): the spring brake holds until the compressor has filled the tanks - keep the engine running").replace("%{spring}", &spring)
                } else {
                    omsi_ui::tr("Air pressure is low (%{tanks} bar in the tanks, spring brake %{spring} bar): the spring brake holds until the compressor has filled the tanks - keep the engine running")
                        .replace("%{tanks}", &format!("{:.1}", low / 1e5))
                        .replace("%{spring}", &spring)
                });
            }
        }
    }
    if v.var("bremse_halte_sw").unwrap_or(0.0) > 0.5
        || v.var("bremse_halte").unwrap_or(0.0) > 0.5
        || v.var("bus_stop_brake").unwrap_or(0.0) > 0.5
    {
        lines.push(
            "Stop brake / door release is on: the bus stands until it is switched off".to_string(),
        );
    }
    // what the passengers are told is open (OMSI's `PAX_Entry<n>_Open` / `PAX_Exit<n>_Open`);
    // the door leaves' `door_<n>` only where a bus has none of those - mods put other things
    // in `door_<n>`, and a Hong Kong bus with its doors shut said they were open
    let pax: Vec<f32> = (0..8)
        .flat_map(|i| {
            let e = format!("PAX_Entry{i}_Open");
            let x = format!("PAX_Exit{i}_Open");
            [
                v.has_script_var(&e).then(|| v.var(&e)).flatten(),
                v.has_script_var(&x).then(|| v.var(&x)).flatten(),
            ]
        })
        .flatten()
        .collect();
    let open = if pax.is_empty() {
        (0..4).any(|i| v.var(&format!("door_{i}")).unwrap_or(0.0) > 0.05)
    } else {
        pax.iter().any(|x| *x > 0.5)
            || (0..4).any(|i| {
                let e = format!("PAX_Entry{i}_Open");
                let x = format!("PAX_Exit{i}_Open");
                (!v.has_script_var(&e) && !v.has_script_var(&x))
                    && v.var(&format!("door_{i}")).unwrap_or(0.0) > 0.05
            })
    };
    if open {
        lines.push("Doors are open".to_string());
    }
    lines
}

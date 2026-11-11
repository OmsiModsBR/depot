//! Turning a registry entry into a declaration, and finding the entry for a vehicle.

use glam::DVec3;

use crate::domain::convention::{Convention, ConventionError, ConventionParams, RegistryEntry};
use crate::domain::declaration::{PoleGeometry, PoleParams, TrolleyDeclaration};

/// Frames per second the parameter file's frame counts are read at (see the plan's R8).
const FRAMES_PER_SECOND: f64 = 60.0;

/// The declaration of `entry` with the global `params`.
pub fn declaration_from(
    entry: &RegistryEntry,
    params: &ConventionParams,
) -> Result<TrolleyDeclaration, ConventionError> {
    let section = format!("vehicle:{}", entry.name);
    let raw = |key: &str| -> Result<&str, ConventionError> {
        entry.get(key).ok_or_else(|| ConventionError::Missing {
            section: section.clone(),
            key: key.into(),
        })
    };
    let num = |key: &str| -> Result<f64, ConventionError> {
        let v = raw(key)?;
        number(v).ok_or_else(|| ConventionError::NotANumber {
            section: section.clone(),
            key: key.into(),
            value: v.into(),
        })
    };
    // a value the parameter file must give (no value of Depot's own is made up for it)
    let global = |s: &str, k: &str| -> Result<f64, ConventionError> {
        let v = params.get(s, k).ok_or_else(|| ConventionError::Missing {
            section: s.into(),
            key: k.into(),
        })?;
        number(v).ok_or_else(|| ConventionError::NotANumber {
            section: s.into(),
            key: k.into(),
            value: v.into(),
        })
    };
    // the entry's own key, else the parameter file's
    let own_or_global = |key: &str| -> Result<f64, ConventionError> {
        match entry.get(key) {
            Some(_) => num(key),
            None => global("global", key),
        }
    };
    // the entry's own key, else the parameter file's [physics] one
    let own_or_physics = |key: &str| -> Result<f64, ConventionError> {
        match entry.get(key) {
            Some(_) => num(key),
            None => global("physics", key),
        }
    };
    // the entry's own scale or offset, else none (1 or 0: the value passes unchanged)
    let own_or = |key: &str, neutral: f64| -> Result<f64, ConventionError> {
        match entry.get(key) {
            Some(_) => num(key),
            None => Ok(neutral),
        }
    };
    let base_forward = num("pole_base_forward")?;
    let base_height = num("pole_base_height")?;
    let contact_forward = num("contact_forward")?;
    let contact_height = num("contact_height")?;
    let pole = |base_x: &str, contact_x: &str| -> Result<PoleGeometry, ConventionError> {
        Ok(PoleGeometry {
            base: DVec3::new(num(base_x)?, base_forward, base_height),
            rest_shoe: DVec3::new(num(contact_x)?, contact_forward, contact_height),
        })
    };
    Ok(TrolleyDeclaration {
        entry: entry.name.clone(),
        bus_file: raw("bus_file")?.to_string(),
        carrier_bus_file: entry
            .get("pole_carrier_bus_file")
            .filter(|f| !f.trim().is_empty())
            .map(str::to_string),
        poles: [
            pole("pole_base_left_x", "contact_left_x")?,
            pole("pole_base_right_x", "contact_right_x")?,
        ],
        params: PoleParams {
            capture_distance: own_or_global("manual_capture_distance")?,
            raise_duration: global("physics", "pole_raise_duration")?,
            lower_duration: global("physics", "pole_lower_duration")?,
            yaw_factor: [
                own_or("yaw_factor_left", 1.0)?,
                own_or("yaw_factor_right", 1.0)?,
            ],
            pitch_factor: own_or("pitch_factor", 1.0)?,
            shoe_factor: [
                own_or("shoe_factor_left", 1.0)?,
                own_or("shoe_factor_right", 1.0)?,
            ],
            shoe_height_offset: match entry.get("shoe_contact_height_offset") {
                Some(_) => num("shoe_contact_height_offset")?,
                None => params
                    .get("global", "shoe_contact_height_offset")
                    .and_then(number)
                    .unwrap_or(0.0),
            },
            spring_nominal_force: global("physics", "spring_nominal_force_n")?,
            spring_reference_elevation: global("physics", "spring_reference_elevation_deg")?,
            spring_gain_per_deg: global("physics", "spring_gain_n_per_deg")?,
            contact_force_max: global("physics", "contact_force_max_n")?,
            stall_reset: global("global", "lost_reset_frames")? / FRAMES_PER_SECOND,
            reach_release_error: own_or_physics("contact_reach_release_error_m")?,
            reach_release_time: own_or_physics("contact_reach_release_time")?,
            force_dewire: global("physics", "contact_force_dewire_n")?,
            force_dewire_time: global("physics", "contact_force_dewire_time")?,
            side_force_dewire: global("physics", "dewire_lateral_force_n")?,
            side_force_time: global("physics", "dewire_lateral_force_time")?,
            yaw_limit: global("physics", "dewire_yaw_limit_deg")?,
            free_rise_rate: global("physics", "dewire_free_rise_deg_s")?,
            max_pitch: global("global", "max_pitch")?,
            catcher_delay: global("physics", "dewire_catcher_delay")?,
            catcher_lower_rate: global("physics", "dewire_catcher_lower_deg_s")?,
            switch_threshold: global("global", "switch_throttle_threshold")?,
        },
    })
}

fn number(v: &str) -> Option<f64> {
    v.trim().parse::<f64>().ok().filter(|x| x.is_finite())
}

/// Does the path `file` (as the game found it) name the registry's `wanted` (a path
/// relative to the OMSI folder)? Compared from the end, folder by folder, without case.
pub fn same_vehicle_file(file: &str, wanted: &str) -> bool {
    let norm = |s: &str| {
        s.trim()
            .replace('/', "\\")
            .trim_start_matches(".\\")
            .to_ascii_lowercase()
    };
    let (f, w) = (norm(file), norm(wanted));
    !w.is_empty() && (f == w || f.ends_with(&format!("\\{w}")))
}

/// The declaration for a vehicle whose front part is `bus_file` and whose coupled parts are
/// `parts` (front to back), and the index of the part that carries the poles (0: the front
/// part, n: the n-th coupled part). The first enabled entry that names the vehicle wins; an
/// entry that names it but cannot be used is returned as its error.
pub fn find_declaration(
    convention: &Convention,
    bus_file: &str,
    parts: &[&str],
) -> Option<Result<(TrolleyDeclaration, usize), ConventionError>> {
    let entry = convention.entries.iter().find(|e| {
        e.enabled
            && e.get("bus_file")
                .is_some_and(|w| same_vehicle_file(bus_file, w))
    })?;
    Some(declaration_from(entry, &convention.params).and_then(|d| {
        let carrier = match &d.carrier_bus_file {
            None => 0,
            Some(c) => match parts.iter().position(|p| same_vehicle_file(p, c)) {
                Some(i) => i + 1,
                None => {
                    return Err(ConventionError::Missing {
                        section: format!("vehicle:{}", d.entry),
                        key: format!("a coupled part {c} (pole_carrier_bus_file)"),
                    })
                }
            },
        };
        Ok((d, carrier))
    }))
}

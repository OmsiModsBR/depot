//! Turning a registry entry into a declaration, and finding the entry for a vehicle.

use glam::DVec3;

use super::convention::{number, read_number, required};
use crate::domain::convention::{
    Convention, ConventionError, ConventionParams, RegistryEntry, POLE_SIDES, POLE_TURN,
    POLE_VARIABLE_PREFIX, SHOE_TURN,
};
use crate::domain::declaration::{PoleGeometry, PoleParams, TrolleyDeclaration};
use crate::domain::sound::{SoundParams, SparkParams};

/// Frames per second the parameter file's frame counts are read at (see the plan's R8).
const FRAMES_PER_SECOND: f64 = 60.0;

/// The keys of an entry that place its poles.
const POLE_KEYS: [&str; 8] = [
    "pole_base_left_x",
    "pole_base_right_x",
    "pole_base_forward",
    "pole_base_height",
    "contact_left_x",
    "contact_right_x",
    "contact_forward",
    "contact_height",
];

/// The declaration of `entry` with the global `params`; the entry places the poles.
pub fn declaration_from(
    entry: &RegistryEntry,
    params: &ConventionParams,
) -> Result<TrolleyDeclaration, ConventionError> {
    declaration_with_model(entry, params, || None)
}

/// The declaration of `entry` with the global `params`. Its poles stand where the entry
/// places them; an entry with none of the keys that place them takes them from the model
/// (`model`: each pole's foot and shoe, as [`poles_in_model`] finds them), and is an error
/// only if the model has none either. An entry that places them in part is an error.
pub fn declaration_with_model(
    entry: &RegistryEntry,
    params: &ConventionParams,
    model: impl FnOnce() -> Option<[(DVec3, DVec3); 2]>,
) -> Result<TrolleyDeclaration, ConventionError> {
    let section = format!("vehicle:{}", entry.name);
    let num = |key: &str| read_number(&section, key, entry.get(key));
    // a value the parameter file must give (no value of Depot's own is made up for it)
    let global = |s: &str, k: &str| value(params, s, k);
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
    let declared = POLE_KEYS.iter().any(|k| entry.get(k).is_some());
    let from_model = if declared { None } else { model() };
    let poles = match from_model {
        Some(found) => found.map(|(base, rest_shoe)| PoleGeometry { base, rest_shoe }),
        None => {
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
            [
                pole("pole_base_left_x", "contact_left_x")?,
                pole("pole_base_right_x", "contact_right_x")?,
            ]
        }
    };
    Ok(TrolleyDeclaration {
        entry: entry.name.clone(),
        bus_file: required(&section, "bus_file", entry.get("bus_file"))?.to_string(),
        carrier_bus_file: entry
            .get("pole_carrier_bus_file")
            .filter(|f| !f.trim().is_empty())
            .map(str::to_string),
        poles,
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
            complex_speed_limit: global("physics", "complex_speed_limit_kmh")?,
            complex_overspeed_grace: global("physics", "complex_overspeed_grace")?,
        },
        sound: optional(sound_params(params)),
        sparks: optional(spark_params(params)),
    })
}

/// A section that is not there is none; one that cannot be read is none too, with a
/// warning (the poles work without it).
fn optional<T>(r: Result<Option<T>, ConventionError>) -> Option<T> {
    r.unwrap_or_else(|e| {
        log::warn!("trolleybus: {e}");
        None
    })
}

/// A value of `section` the parameter file must give once the section is there.
fn value(params: &ConventionParams, section: &str, key: &str) -> Result<f64, ConventionError> {
    read_number(section, key, params.get(section, key))
}

fn has_section(params: &ConventionParams, section: &str) -> bool {
    let prefix = format!("{section}.");
    params.values.keys().any(|k| k.starts_with(&prefix))
}

/// The `[sound]` section (None when the file has none).
pub(crate) fn sound_params(
    params: &ConventionParams,
) -> Result<Option<SoundParams>, ConventionError> {
    if !has_section(params, "sound") {
        return Ok(None);
    }
    let v = |k: &str| value(params, "sound", k);
    Ok(Some(SoundParams {
        cross_trigger_radius: v("cross_trigger_radius")?,
        min_distance: v("min_distance")?,
        max_distance: v("max_distance")?,
        volume_db: v("volume_db")?,
        interior_source: DVec3::new(
            v("interior_source_x")?,
            v("interior_source_y")?,
            v("interior_source_z")?,
        ),
        interior_cab_gain_db: v("interior_cab_gain_db")?,
        interior_salon_gain_db: v("interior_salon_gain_db")?,
        spark_volume_db: v("spark_volume_db")?,
        dewire_volume_db: v("dewire_volume_db")?,
    }))
}

/// The `[sparks]` section (None when the file has none).
pub(crate) fn spark_params(
    params: &ConventionParams,
) -> Result<Option<SparkParams>, ConventionError> {
    if !has_section(params, "sparks") {
        return Ok(None);
    }
    let v = |k: &str| value(params, "sparks", k);
    Ok(Some(SparkParams {
        enabled: v("enabled")? > 0.5,
        min_speed_kmh: v("min_speed_kmh")?,
        min_throttle: v("min_throttle")?,
        interval: [
            v("interval_min_ms")? / 1000.0,
            v("interval_max_ms")? / 1000.0,
        ],
        duration: [
            v("duration_min_ms")? / 1000.0,
            v("duration_max_ms")? / 1000.0,
        ],
        rain_gain: v("rain_frequency_gain")?,
        snow_gain: v("snow_frequency_gain")?,
        ice_gain: v("ice_frequency_gain")?,
    }))
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
/// entry that names it but cannot be used is returned as its error. An entry that does not
/// place the poles takes them from the carrier's model (`model`, given the carrier's index;
/// see [`declaration_with_model`]).
pub fn find_declaration(
    convention: &Convention,
    bus_file: &str,
    parts: &[&str],
    model: impl FnOnce(usize) -> Option<[(DVec3, DVec3); 2]>,
) -> Option<Result<(TrolleyDeclaration, usize), ConventionError>> {
    let entry = convention.entries.iter().find(|e| {
        e.enabled
            && e.get("bus_file")
                .is_some_and(|w| same_vehicle_file(bus_file, w))
    })?;
    let carrier = match entry
        .get("pole_carrier_bus_file")
        .filter(|f| !f.trim().is_empty())
    {
        None => 0,
        Some(c) => match parts.iter().position(|p| same_vehicle_file(p, c)) {
            Some(i) => i + 1,
            None => {
                return Some(Err(ConventionError::Missing {
                    section: format!("vehicle:{}", entry.name),
                    key: format!("a coupled part {c} (pole_carrier_bus_file)"),
                }))
            }
        },
    };
    Some(declaration_with_model(entry, &convention.params, || model(carrier)).map(|d| (d, carrier)))
}

/// The convention's pole variable `var` (any case, the scripts' `_v` copy counts too):
/// which pole (0 left, 1 right) and whether it turns the shoe rather than the pole.
pub fn pole_variable(var: &str) -> Option<(usize, bool)> {
    let v = var.trim().to_ascii_lowercase();
    let v = v.strip_suffix("_v").unwrap_or(&v);
    let rest = v.strip_prefix(POLE_VARIABLE_PREFIX)?;
    let (side, what) = rest.split_once('_')?;
    let i = POLE_SIDES.iter().position(|s| *s == side)?;
    match what {
        POLE_TURN => Some((i, false)),
        SHOE_TURN => Some((i, true)),
        _ => None,
    }
}

/// Where a model's poles stand, from the animations the convention's pole variables drive:
/// for each pole (left, right) its foot - the pivot its turn (`shtanga_lev_rot`) is drawn
/// about - and its shoe at rest - the pivot of the shoe's turn (`shtanga_lev_shoe_rot`).
/// `anims` gives each animation's variable and the place of its first origin (`origin_trans`,
/// the model's frame). None: the model has no working poles, it is no trolleybus.
pub fn poles_in_model<'a>(
    anims: impl IntoIterator<Item = (&'a str, [f64; 3])>,
) -> Option<[(DVec3, DVec3); 2]> {
    let mut foot: [Option<DVec3>; 2] = [None, None];
    let mut shoe: [Option<DVec3>; 2] = [None, None];
    for (var, at) in anims {
        let Some((i, is_shoe)) = pole_variable(var) else {
            continue;
        };
        let slot = if is_shoe { &mut shoe[i] } else { &mut foot[i] };
        slot.get_or_insert(DVec3::from(at));
    }
    Some([(foot[0]?, shoe[0]?), (foot[1]?, shoe[1]?)])
}

/// The declaration of a vehicle the registry does not name, found a trolleybus by its model
/// (`poles` from [`poles_in_model`]): the poles where the model draws them, the parameter
/// file's values for everything else, as an entry named `name` for `bus_file` that places
/// no poles would give it.
pub fn declaration_from_model(
    name: &str,
    bus_file: &str,
    poles: [(DVec3, DVec3); 2],
    params: &ConventionParams,
) -> Result<TrolleyDeclaration, ConventionError> {
    let entry = RegistryEntry {
        name: name.to_string(),
        enabled: true,
        values: [("bus_file".to_string(), bus_file.to_string())].into(),
    };
    declaration_with_model(&entry, params, || Some(poles))
}

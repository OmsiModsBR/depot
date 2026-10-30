//! The vehicles of the lists: names, sorting, the place list's choices, liveries, depots and fleet numbers.

use crate::core::app::App;

/// A vehicle file of the menu's list (`Vehicles/...`) as its definition.
pub(in crate::hud) fn bus_def(app: &App, bus: &str) -> Option<omsi_vehicle::Vehicle> {
    let path = crate::spawn::player_bus_path(&app.args.root, bus).ok()?;
    omsi_vehicle::Vehicle::load(&path).ok()
}

/// Names in older packs often use underscores as spaces.
pub(in crate::hud) fn bus_label(name: &str) -> String {
    name.replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Numbers inside names sort as numbers (DL9 before DL10), case does not matter.
pub(in crate::hud) fn bus_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let x: String = std::iter::from_fn(|| a.next_if(|c| c.is_ascii_digit())).collect();
                let y: String = std::iter::from_fn(|| b.next_if(|c| c.is_ascii_digit())).collect();
                let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let order = x.len().cmp(&y.len()).then_with(|| x.cmp(y));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.cmp(&y);
                if order != Ordering::Equal {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// The vehicles of the place list as (manufacturer's key, manufacturer, type, path): on a
/// server only those it offers (#1183).
pub(in crate::hud) fn place_vehicles(
    app: &App,
    unknown: &str,
) -> Vec<(String, String, String, String)> {
    let offered = crate::lan::server_offers();
    app.vehicle_list
        .iter()
        .filter(|(_, path)| {
            offered
                .as_deref()
                .is_none_or(|o| crate::lan::offers(o, path))
        })
        .map(|(name, path)| {
            let (maker, ty) = app.vehicle_meta.get(path).cloned().unwrap_or_default();
            let maker = bus_label(&maker);
            let ty = if ty.trim().is_empty() {
                bus_label(name)
            } else {
                bus_label(&ty)
            };
            let shown = if maker.is_empty() {
                unknown.to_string()
            } else {
                maker.clone()
            };
            (maker.to_lowercase(), shown, ty, path.clone())
        })
        .collect()
}

/// Types as (name, path), sorted, as the place list shows them: a type name used twice gets
/// its pack's folder, then its file.
pub(in crate::hud) fn place_types(mut types: Vec<(String, String)>) -> Vec<(String, String)> {
    types.sort_by(|a, b| bus_cmp(&a.0, &b.0).then_with(|| a.1.cmp(&b.1)));
    let same = |t: &[(String, String)], n: &str| {
        t.iter()
            .filter(|x| x.0.to_lowercase() == n.to_lowercase())
            .count()
    };
    let counts: Vec<usize> = types.iter().map(|t| same(&types, &t.0)).collect();
    types
        .iter()
        .zip(counts)
        .map(|(t, n)| {
            let mut label = t.0.clone();
            if n > 1 {
                let parts: Vec<&str> = t.1.split('/').collect();
                let folder = parts.get(1).copied().unwrap_or_default();
                let file = parts
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .rsplit_once('.')
                    .map(|x| x.0)
                    .unwrap_or_default();
                label = format!("{label}  ·  {}  ·  {}", bus_label(folder), bus_label(file));
            }
            (label, t.1.clone())
        })
        .collect()
}

/// The place list's vehicles by manufacturer as (manufacturer, its models as (type, path)),
/// the manufacturers and the models each sorted as the place list sorts them: the new
/// interface's "Add vehicle" window lists the manufacturers, then the chosen one's models.
pub(crate) fn place_choices(app: &App) -> Vec<(String, Vec<(String, String)>)> {
    let all = place_vehicles(app, "Unknown manufacturer");
    let mut makers: Vec<(String, String)> = Vec::new();
    for v in &all {
        if !makers.iter().any(|m| m.0 == v.0) {
            makers.push((v.0.clone(), v.1.clone()));
        }
    }
    makers.sort_by(|a, b| bus_cmp(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
    makers
        .into_iter()
        .map(|(key, name)| {
            let types = all
                .iter()
                .filter(|v| v.0 == key)
                .map(|v| (v.2.clone(), v.3.clone()))
                .collect();
            (name, place_types(types))
        })
        .collect()
}

/// A vehicle's liveries by name, alphabetical (the place list puts "Random livery" first).
pub(crate) fn place_liveries(app: &App, bus: &str) -> Vec<String> {
    let mut names = bus_def(app, bus).map(|d| liveries(&d)).unwrap_or_default();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
}

/// A vehicle's fleet numbers (its `[number]` list), as the "Add vehicle" window steps
/// through them.
pub(crate) fn place_numbers(app: &App, bus: &str) -> Vec<String> {
    bus_def(app, bus)
        .map(|d| {
            d.numbers_with_plates()
                .into_iter()
                .map(|(n, _)| n)
                .collect()
        })
        .unwrap_or_default()
}

/// A vehicle's description as its author wrote it: its description file in the game's
/// language (`<bus>_ENG.dsc`), else the `[description]` of the vehicle file.
pub(crate) fn place_description(app: &App, bus: &str) -> String {
    let Ok(path) = crate::spawn::player_bus_path(&app.args.root, bus) else {
        return String::new();
    };
    let lang = crate::describe::language_code(&app.settings.language);
    omsi_content::Dsc::find(&path, &lang)
        .map(|d| d.description)
        .filter(|t| !t.trim().is_empty())
        .or_else(|| {
            omsi_vehicle::Vehicle::load(&path)
                .ok()
                .map(|v| v.description.trim().to_string())
        })
        .unwrap_or_default()
}

/// A vehicle's depot files as (label, file name), alphabetical (the place list puts the
/// map's depot file first).
pub(crate) fn place_depots(app: &App, bus: &str) -> Vec<(String, String)> {
    let Some(d) = bus_def(app, bus) else {
        return Vec::new();
    };
    let mut files: Vec<(String, String)> = omsi_vehicle::hof::depot_files(d.dir())
        .into_iter()
        .map(|f| {
            (
                hof_label(&f),
                f.file_name()
                    .map(|x| x.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
        })
        .collect();
    files.sort_by_key(|(label, _)| label.to_lowercase());
    files
}

/// The paint schemes of a vehicle file, by name (without loading its meshes).
pub(in crate::hud) fn liveries(def: &omsi_vehicle::Vehicle) -> Vec<String> {
    let Some(m) = def.model.as_ref() else {
        return Vec::new();
    };
    let mp = omsi_cfg::resolve_path(def.dir(), m);
    let Ok(model) = omsi_model::Model::load(&mp) else {
        return Vec::new();
    };
    let mut names: Vec<String> = model
        .ctc
        .iter()
        .flat_map(|c| {
            omsi_sim::vehicle::load_paint_schemes(&omsi_cfg::resolve_path(def.dir(), &c.path))
        })
        .map(|s| s.name)
        .collect();
    names.dedup();
    names
}

/// A depot file's name for the lists: its `[name]`, and the file.
pub(in crate::hud) fn hof_label(p: &std::path::Path) -> String {
    let file = p
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    match omsi_vehicle::Hof::load(p)
        .ok()
        .map(|h| h.name)
        .filter(|n| !n.trim().is_empty())
    {
        Some(n)
            if !file
                .to_ascii_lowercase()
                .starts_with(&n.trim().to_ascii_lowercase()) =>
        {
            format!("{}  ({file})", n.trim())
        }
        _ => file,
    }
}

/// The fleet numbers of the bus's `[number]` list with their registrations.
pub(in crate::hud) fn fleet_numbers(v: &omsi_sim::VehicleInstance) -> Vec<(String, String)> {
    let def = &v.ty.def;
    def.numbers_with_plates()
        .into_iter()
        .map(|(n, _)| {
            let reg = if def.registration_mode == 1 {
                String::new()
            } else {
                def.chosen_plate_of_number(&n)
            };
            (n, reg)
        })
        .collect()
}

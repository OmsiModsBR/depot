//! What the Drive page keeps between frames (`DriveView`) and the bus list it offers: the
//! manufacturers and their types as OMSI groups them, how they sort and what a search finds;
//! and which trip of a tour a search or the clock points at.

use super::super::interface::output::core;
use core::start::trip_index_at;
use core::{display_bus_name, vehicle_type_label};

#[derive(Clone)]
pub struct BusVariant {
    pub file: String,
    pub(crate) name: String,
    pub variant: String,
    pub(crate) fresh: bool,
    pub(crate) installed: bool,
    pub(crate) paints: usize,
    pub(crate) incomplete: bool,
    /// Its file has the `[depot_physics]` key (see [`physics_notice`]).
    pub depot_physics: bool,
}

#[derive(Clone)]
pub struct BusManufacturer {
    pub(crate) key: String,
    pub name: String,
    pub variants: Vec<BusVariant>,
}

#[derive(Default)]
pub struct DriveView {
    /// The tab: 0 the vehicle and the environment, 1 the map and the duty.
    pub tab: usize,
    pub bus_filter: String,
    /// The original OMSI manufacturer/type hierarchy, cached between content updates.
    pub(crate) bus_manufacturers: std::sync::Arc<Vec<BusManufacturer>>,
    pub(crate) bus_manufacturers_key: (usize, u64, usize),
    pub(crate) expanded_manufacturer: Option<String>,
    pub(crate) bus_list_initialized: bool,
    pub(crate) vehicle_settings_open: bool,
    pub line_filter: String,
    /// The tour list's search (tour number, trip name, line, stop) and whether tours whose
    /// last trip has left are listed.
    pub tour_filter: String,
    pub show_ended: bool,
    /// The line list was put where the chosen line is (once: the first time it has rows).
    pub(crate) line_scrolled: bool,
    /// The roadbook's sidebar: open or shut, and whether the player shut it by hand (which
    /// is what stops a chosen tour from opening it again).
    pub book_open: bool,
    pub(crate) book_shut: bool,
    /// The duty (line, tour, first trip) the roadbook was last scrolled to: it opens on the
    /// player's first trip, not on the morning's trips before it.
    pub(crate) book_scrolled: Option<(String, String, usize)>,
    /// The buses marked with a star (#524), by file (lower case, '/'), read once from
    /// `~/.depot/favourite-buses.txt`; and whether the list shows only them.
    pub(crate) favourites: Option<std::collections::BTreeSet<String>>,
    pub(crate) only_favourites: bool,
}

/// Whether all of a tour's trips have left at `now`, and the trip a start then takes: the one
/// under way or the next, or with a search (`q`, lower case) the first still to come that
/// matches it by name, line or stop (a tour whose number matches keeps the usual one).
pub fn tour_trip(t: &core::TourInfo, now: f64, q: &str) -> (bool, Option<usize>) {
    let ended = t.runs && t.trips.iter().all(|x| x.departure < now - 120.0);
    let first = trip_index_at(t, now);
    let has = |s: &str| s.to_lowercase().contains(q);
    if q.is_empty() || has(&t.number) {
        return (ended, first);
    }
    let from = if ended { 0 } else { first.unwrap_or(0) };
    let hit = t.trips.iter().enumerate().skip(from).find(|(_, x)| {
        has(&x.name)
            || has(&x.line)
            || has(&x.from)
            || has(&x.terminus)
            || x.stops.iter().any(|s| has(&s.name))
    });
    (ended, hit.map(|(k, _)| k))
}

/// OMSI takes the manufacturer and the complete type from [friendlyname]. The
/// vehicle folder and rendering configuration do not define this hierarchy.
pub fn build_bus_manufacturers(
    vehicles: &[core::VehicleInfo],
    allowed: Option<&std::collections::HashSet<String>>,
    fresh: &std::collections::HashSet<String>,
) -> Vec<BusManufacturer> {
    let mut grouped = std::collections::BTreeMap::<String, BusManufacturer>::new();
    for vehicle in vehicles {
        if !allowed
            .map(|a| a.contains(&vehicle.file.replace('\\', "/").to_lowercase()))
            .unwrap_or(true)
        {
            continue;
        }
        let maker = vehicle.manufacturer.trim();
        let key = maker.to_lowercase();
        let group = grouped
            .entry(key.clone())
            .or_insert_with(|| BusManufacturer {
                key,
                name: if maker.is_empty() {
                    "Unknown manufacturer".into()
                } else {
                    display_bus_name(maker)
                },
                variants: Vec::new(),
            });
        let type_name = vehicle_type_label(&vehicle.type_name, std::path::Path::new(&vehicle.file));
        group.variants.push(BusVariant {
            file: vehicle.file.clone(),
            name: display_bus_name(&vehicle.name),
            variant: type_name,
            fresh: fresh.contains(&vehicle.file),
            installed: vehicle.installed,
            paints: vehicle.paints.len(),
            incomplete: !vehicle.missing_packs.is_empty(),
            depot_physics: vehicle.depot_physics,
        });
    }
    let mut manufacturers: Vec<BusManufacturer> = grouped.into_values().collect();
    for maker in &mut manufacturers {
        // Distinct .bus files remain selectable even when add-ons repeat a friendly
        // type name. Show the pack, and the file only if the pack also repeats it.
        let mut counts = std::collections::HashMap::<String, usize>::new();
        for variant in &maker.variants {
            *counts.entry(variant.variant.to_lowercase()).or_default() += 1;
        }
        for variant in &mut maker.variants {
            if counts[&variant.variant.to_lowercase()] > 1 {
                let folder = variant
                    .file
                    .replace('\\', "/")
                    .split('/')
                    .nth(1)
                    .unwrap_or_default()
                    .to_string();
                variant.variant = format!("{} · {}", variant.variant, display_bus_name(&folder));
            }
        }
        let mut counts = std::collections::HashMap::<String, usize>::new();
        for variant in &maker.variants {
            *counts.entry(variant.variant.to_lowercase()).or_default() += 1;
        }
        for variant in &mut maker.variants {
            if counts[&variant.variant.to_lowercase()] > 1 {
                let stem = std::path::Path::new(&variant.file)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy();
                variant.variant = format!("{} · {}", variant.variant, display_bus_name(&stem));
            }
        }
        maker
            .variants
            .sort_by(|a, b| bus_name_cmp(&a.variant, &b.variant).then_with(|| a.file.cmp(&b.file)));
    }
    manufacturers.sort_by(|a, b| bus_name_cmp(&a.name, &b.name).then_with(|| a.key.cmp(&b.key)));
    manufacturers
}

/// Sort numeric runs wherever they occur: DL9 precedes DL10; case does not change
/// a manufacturer's position.
pub fn bus_name_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let x: String = std::iter::from_fn(|| a.next_if(|c| c.is_ascii_digit())).collect();
                let y: String = std::iter::from_fn(|| b.next_if(|c| c.is_ascii_digit())).collect();
                let x = x.trim_start_matches('0');
                let y = y.trim_start_matches('0');
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

pub fn default_livery_label(vehicle: &core::VehicleInfo) -> &str {
    if vehicle.default_paint.trim().is_empty() {
        "Default paint"
    } else {
        vehicle.default_paint.trim()
    }
}

/// How many liveries a bus type with `repaints` has, as its Livery list counts them (its own
/// paint and the repaints): beside each type in the list, as a family says how many types it
/// has (#717).
pub fn liveries_text(repaints: usize) -> String {
    let n = repaints + 1;
    format!(
        "{n} {}",
        omsi_ui::tr(if n == 1 { "livery" } else { "liveries" })
    )
}

/// The badge and the notice the bus list shows for a type whose file has the `[depot_physics]`
/// key (spec 002, FR-008a): it runs Project Depot Physics when the player drives it.
pub fn physics_notice(variant: &BusVariant) -> Option<(&'static str, &'static str)> {
    depot_physics_notice(variant.depot_physics)
}

/// The badge and the notice for a bus whose file has the `[depot_physics]` key
/// (`depot_physics`), wherever the launcher lists it: the Drive page's bus list (the phone launcher) and the
/// Vehicles page (a computer's).
pub fn depot_physics_notice(depot_physics: bool) -> Option<(&'static str, &'static str)> {
    depot_physics.then_some((
        "DEPOT PHYSICS",
        "Runs Project Depot Physics when you drive it ([depot_physics] in its file), whatever the Physics setting says",
    ))
}

/// A type of a bus family in its dropdown: the type and its liveries.
pub fn variant_option(variant: &BusVariant) -> String {
    format!("{} · {}", variant.variant, liveries_text(variant.paints))
}

pub fn variant_matches(variant: &BusVariant, q: &str) -> bool {
    q.is_empty()
        || variant.name.to_lowercase().contains(q)
        || variant.variant.to_lowercase().contains(q)
        || display_bus_name(&variant.file).to_lowercase().contains(q)
}

pub fn manufacturer_matches(model: &BusManufacturer, q: &str) -> bool {
    q.is_empty()
        || model.name.to_lowercase().contains(q)
        || model.variants.iter().any(|v| variant_matches(v, q))
}

pub fn natural(s: &str) -> (u64, String) {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    (digits.parse().unwrap_or(u64::MAX), s.to_string())
}

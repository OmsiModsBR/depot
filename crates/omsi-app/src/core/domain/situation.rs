//! The situation's rules, with no file access: the start clock, the saved riders and the
//! names of their files, the checks of a saved height and odometer, the script texture
//! names and the situation's own arithmetic. The files are read and written in
//! `implementation/situation.rs`.

use crate::core::cli::{parse_time, Args};
use std::path::{Path, PathBuf};

pub(crate) fn start_clock(args: &Args) -> omsi_sim::SimClock {
    let mut c = omsi_sim::SimClock::default();
    if let Some(d) = &args.date {
        let v: Vec<i32> = d.split('-').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() == 3 {
            c.set_date(v[0], v[1], v[2]);
        }
    }
    if let Some(doy) = args.day_of_year {
        c.day_of_year = doy.max(1);
    }
    c.time = parse_time(&args.time);
    c
}

/// How the parts coupled behind vehicle `lead` of a situation stood against it: each part's
/// heading less the vehicle's (degrees, -180..180), from the front. The parts are the
/// vehicles listed after it whose `[coupledWith]` names the one before them (the number is
/// where that one is in the list, counted from the part itself: -1).
pub fn coupled_bends(
    vehicles: &[omsi_content::situation::SituationVehicle],
    lead: usize,
) -> Vec<f64> {
    let Some(front) = vehicles.get(lead) else {
        return Vec::new();
    };
    let heading = situation_heading(&front.orientation);
    vehicles
        .iter()
        .enumerate()
        .skip(lead + 1)
        .take_while(|(j, v)| v.coupled_with.map(|c| *j as i64 + c as i64) == Some(*j as i64 - 1))
        .map(|(_, v)| {
            (situation_heading(&v.orientation) - heading + 540.0).rem_euclid(360.0) - 180.0
        })
        .collect()
}

/// The files of a vehicle's `[scripttexture]` images saved with a situation, as OMSI
/// names them: the situation file's whole name, then `_<vehicle>_<texture>.dds`
/// (`Slot 1.osn_0_2.dds`), the vehicle counted in the order the file lists them. Without
/// `_<texture>.dds`; None for a situation not read from a file.
pub(crate) fn script_texture_prefix(osn: &Path, vehicle: usize) -> Option<PathBuf> {
    let name = osn.file_name()?.to_string_lossy();
    Some(osn.with_file_name(format!("{name}_{vehicle}")))
}

pub(crate) fn script_texture_file(prefix: &Path, texture: usize) -> PathBuf {
    let name = prefix
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    prefix.with_file_name(format!("{name}_{texture}.dds"))
}

/// A passenger riding in the player's bus, as a situation keeps them (#613).
///
/// A deliberate improvement over the reference game, whose situations bring the timetable
/// and the trip back but an empty bus. Kept: the place each rider holds (the index of the
/// bus's `[passpos]` list: the same seat or standing place again), where they are going and
/// how much of their ride is left. Not kept: who they look like, their ticket, their mood.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SavedRider {
    /// The place held; none for somebody already on the way to the door (they gave their
    /// place up), who is seated at any free one and asks for the stop again.
    pub seat: Option<usize>,
    pub dest: Option<String>,
    pub alt: Option<String>,
    pub alt_seen: bool,
    pub alt_m: f32,
    /// Kilometres still to ride (counts only for a rider without a destination, or past the
    /// stop their line leaves the known route at).
    pub left_km: f32,
    /// Where between departure and the approach they ask to get off (0..1).
    pub request_random: f64,
}

impl SavedRider {
    /// The rider a passenger sitting or standing in a bus is. `odometer_km`: the kilometres
    /// the riders' clock of that bus reads now. None for somebody not aboard with a place.
    pub fn from_pax(x: &crate::gameplay::humans::pax::Pax, odometer_km: f64) -> Option<SavedRider> {
        x.inside?;
        let seat = x.seat;
        if seat.is_none() && x.task != crate::gameplay::humans::pax::Task::InBusToExit {
            return None;
        }
        Some(SavedRider {
            seat,
            dest: x.dest.clone(),
            alt: x.alt.clone(),
            alt_seen: x.alt_seen,
            alt_m: x.alt_m,
            left_km: (x.km_start + x.ride_km as f64 - odometer_km).max(0.0) as f32,
            request_random: x.stop_request_random,
        })
    }

    /// The passenger again, settled at their place: `odometer_km` is the riders' clock now,
    /// so what is left of the ride counts from it.
    pub fn to_pax(&self, walk_speed: f32, odometer_km: f64) -> crate::gameplay::humans::pax::Pax {
        use crate::gameplay::humans::pax::{Pax, Task};
        let mut p = Pax::new(walk_speed, self.request_random.clamp(0.0, 1.0));
        p.task = Task::SittingInBus;
        p.seat = self.seat;
        p.dest = self.dest.clone();
        p.alt = self.alt.clone();
        p.alt_seen = self.alt_seen;
        p.alt_m = self.alt_m;
        p.km_start = odometer_km;
        p.ride_km = self.left_km;
        p
    }
}

/// The file the riders of a situation are kept in, next to it: `Slot 1.osn.riders`. A file
/// of its own so the `.osn` stays exactly what the reference game writes and reads; a
/// situation without the file (every older one, every one the reference saved) simply
/// loads an empty bus, as before.
pub fn riders_file(osn: &Path) -> Option<PathBuf> {
    let name = osn.file_name()?.to_string_lossy().into_owned();
    Some(osn.with_file_name(format!("{name}.riders")))
}

pub(crate) const RIDERS_HEADER: &str = "depot-riders 1";

/// Upper bounds for the numbers read back, so a damaged file cannot keep a rider aboard for
/// ever or make the ride arithmetic overflow.
pub(crate) const MAX_RIDE_KM: f32 = 1.0e5;

pub(crate) const MAX_ALT_M: f32 = 1.0e6;

/// More riders than any bus holds; a larger file is damaged or hostile and is not read.
pub(crate) const MAX_RIDERS: usize = 1024;

pub(crate) const MAX_RIDERS_FILE: u64 = 1 << 20;

pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\t' => out.push_str("%09"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn unescape(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The lowest height (m) a saved vehicle can plausibly stand at. A vehicle that fell through
/// an unloaded tile came back from a save thousands of kilometres under the map (#33).
pub(crate) const LOWEST_PLAUSIBLE_HEIGHT: f64 = -10_000.0;

/// Whether a vehicle's height is one a map can have: finite and not far under the world.
pub fn plausible_height(z: f64) -> bool {
    z.is_finite() && z > LOWEST_PLAUSIBLE_HEIGHT
}

/// The `--spawn` text `x,y,heading[,z]` of a saved vehicle. An implausible saved height is
/// left out, so the vehicle is put on the ground of its place.
pub fn spawn_arg(x: f64, y: f64, heading: f64, z: f64) -> String {
    if plausible_height(z) {
        format!("{x},{y},{heading},{z}")
    } else {
        log::warn!(
            "situation: a vehicle saved at height {z} is put on the ground at ({x:.1}, {y:.1})"
        );
        format!("{x},{y},{heading}")
    }
}

/// The place `x,y` of a `--spawn` text that carries no height (`spawn_arg` left it out because
/// the saved one was implausible).
pub fn spawn_without_height(spawn: &str) -> Option<(f64, f64)> {
    let v: Vec<f64> = spawn
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    (v.len() == 3).then(|| (v[0], v[1]))
}

/// Prefer OMSI's saved total; old openOMSI only kept it in the script variables.
/// A new bus starts at least at 8 km, so smaller header values
/// are placeholders. The old writer's ordinal pattern also excludes its 8, 9, 10...
/// ids; rejecting every reading below 10 would discard legitimate OMSI situations.
pub fn saved_odometer_km(
    v: &omsi_content::situation::SituationVehicle,
    legacy_ids: bool,
) -> Option<f64> {
    if !legacy_ids && v.odometer_km.is_finite() && v.odometer_km >= 8.0 {
        return Some(v.odometer_km);
    }
    let var = |name: &str| {
        v.vars
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, x)| *x)
    };
    let km = var("kmcounter_km")?;
    let metres = var("kmcounter_m").unwrap_or(0.0);
    let total = km + metres / 1000.0;
    (km.is_finite()
        && metres.is_finite()
        && km >= 0.0
        && (0.0..1000.0).contains(&metres)
        && total.is_finite()
        && total >= 8.0)
        .then_some(total)
}

/// The heading (degrees clockwise from north) of a situation vehicle's rotation quaternion
/// (x, y, z, w with y up, as OMSI writes it).
pub(crate) fn situation_heading(q: &[f64; 9]) -> f64 {
    (2.0 * q[1].atan2(q[3]).to_degrees()).rem_euclid(360.0)
}

/// A content path saved whole (a map or bus from an archive or the content folder:
/// `/Users/…/Archives/x.zip/Maps/Novi Sad/global.cfg`) from its `folder` (`maps`,
/// `vehicles`) on, as the game names content; went after the root, it closed the game.
pub(crate) fn content_relative(path: &str, folder: &str) -> String {
    let p = path.replace('\\', "/");
    if !(p.starts_with('/') || p.get(1..2) == Some(":")) {
        return p;
    }
    match p.to_ascii_lowercase().rfind(&format!("/{folder}/")) {
        Some(i) => p[i + 1..].to_string(),
        None => p,
    }
}

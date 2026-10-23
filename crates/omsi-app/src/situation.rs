//! Where a session starts from: the clock, the depot (`.hof`), a saved situation and writing one.

use crate::cli::{parse_time, Args};
use crate::player::Player;
use crate::scene::{self, World};
use anyhow::{anyhow, Result};
use omsi_render::Camera;
use std::path::{Path, PathBuf};

/// The depot file of a vehicle for this map: `--hof`, else the hof named by the map's
/// `[aigroup_depot]`, else the first .hof next to the .bus file.
pub(crate) fn find_hof(
    args: &Args,
    world: &World,
    vt: &omsi_sim::VehicleType,
) -> Option<std::sync::Arc<omsi_vehicle::Hof>> {
    let dir = vt.def.dir();
    let mut names: Vec<String> = Vec::new();
    if let Some(h) = &args.hof {
        names.push(h.clone());
    }
    names.extend(world.ailists.groups.iter().filter_map(|g| g.hof.clone()));
    // every wanted name in order: the bus's own depot files first (by file name, then by
    // the [name] inside), then the map's depot as another vehicle folder has it (a mod bus
    // brings only the depot of its own map: see `omsi_vehicle::hof::depot_anywhere`)
    for n in &names {
        if let Some(h) = omsi_vehicle::hof::depot_in(dir, n) {
            log::info!("using depot file {} (name {n})", h.path.display());
            return Some(std::sync::Arc::new(h));
        }
    }
    // the bus's own depot of the same place under another name (its Spandau 2019 where the
    // map's buses use Spandau 1986: its displays know its own codes and pictures, #896)
    let wanted: Vec<&str> = names.iter().map(|n| n.as_str()).collect();
    if let Some(h) = omsi_vehicle::hof::depot_like(dir, &wanted) {
        log::info!(
            "using depot file {} (the bus's own of {wanted:?})",
            h.path.display()
        );
        return Some(std::sync::Arc::new(h));
    }
    for n in &names {
        if let Some(h) = omsi_vehicle::hof::depot_anywhere(n) {
            log::info!(
                "{} has no depot file '{n}'; using {}",
                dir.display(),
                h.path.display()
            );
            return Some(std::sync::Arc::new(h));
        }
    }
    // a map without a depot of its own: the bus's depot named like the map (#896)
    let folder = world
        .map_dir
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let hints = [
        world.global.name.as_str(),
        world.global.friendly_name.as_str(),
        folder.as_str(),
    ];
    if let Some(h) = omsi_vehicle::hof::depot_like(dir, &hints) {
        log::info!("using depot file {} (named like the map)", h.path.display());
        return Some(std::sync::Arc::new(h));
    }
    let p = omsi_vehicle::hof::depot_files(dir).into_iter().next()?;
    log::info!("using depot file {}", p.display());
    omsi_vehicle::Hof::load(&p).ok().map(std::sync::Arc::new)
}

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

/// Fill the arguments from a situation file (`.osn`): the original's saved game state.
pub(crate) fn apply_situation(args: &mut Args) -> Result<()> {
    let Some(rel) = args.situation.clone() else {
        return Ok(());
    };
    // (an absolute path is the file itself: a situation saved elsewhere, a test's)
    let path = if Path::new(&rel).is_absolute() {
        PathBuf::from(&rel)
    } else {
        omsi_cfg::resolve_path(&args.root, &rel)
    };
    let sit = omsi_content::Situation::load(&path)
        .map_err(|e| anyhow!("loading {}: {e}", path.display()))?;
    log::info!(
        "situation \"{}\": map {} time {:?} vehicles {}",
        sit.name,
        sit.map,
        sit.time,
        sit.vehicles.len()
    );
    apply_situation_parsed(&sit, args);
    Ok(())
}

pub(crate) fn apply_situation_parsed(sit: &omsi_content::situation::Situation, args: &mut Args) {
    args.situation_odometer_km = None;
    args.situation_script_textures = None;
    args.map = sit.map.replace('\\', "/");
    // A map from an archive was saved as its whole path (`\Users\…\Archives\x.zip\Maps\
    // Novi Sad\global.cfg`), which then went after the root and the game closed at once:
    // from its `maps/` folder on, as every map is named.
    args.map = content_relative(&args.map, "maps");
    args.date = Some(format!("{}-01-01", sit.time.0));
    args.day_of_year = Some(sit.time.1);
    args.time = format!("{:02}:{:02}:{}", sit.time.2, sit.time.3, sit.time.4 as i32);
    if let Some(w) = &sit.weather {
        args.weather = Some(w.clone());
    }
    args.schedule = true;
    args.no_menu = true;
    // the player's vehicle: [ismyVehicle] or [myvehicle] index
    let mine = sit
        .vehicles
        .iter()
        .position(|v| v.is_my_vehicle)
        .or_else(|| {
            sit.vehicles
                .get(sit.my_vehicle.max(0) as usize)
                .map(|_| sit.my_vehicle as usize)
        });
    // Older openOMSI wrote ordinal ids here, not mileage: 1 for the player, then 2+k
    // for placed buses (or 2+k alone without a player). Recognize the complete old
    // writer's pattern, including its description, so genuine OMSI starts at 8..10 km
    // survive and even the tenth legacy bus does not acquire a 10 km reading.
    let legacy_odometer_ids = sit.description.starts_with("Saved by the openOMSI at ")
        && sit
            .vehicles
            .iter()
            .enumerate()
            .all(|(i, v)| v.odometer_km == (i + if mine == Some(0) { 1 } else { 2 }) as f64);
    // the map's tile grid first: a position is a tile and a place in it, and a
    // `[worldcoordinates]` map's tiles are ~372 m and scaled onto the grid
    let global_path = omsi_cfg::resolve_path(&args.root, &args.map);
    match omsi_map::GlobalCfg::load(&global_path) {
        Ok(g) => omsi_map::configure_grid(&g),
        Err(e) => log::warn!("situation: map {}: {e}", global_path.display()),
    }
    if let Some(i) = mine {
        let v = &sit.vehicles[i];
        args.bus = Some(content_relative(&v.file.replace('\\', "/"), "vehicles"));
        // tile-local x, height, y, then the body's rotation as a Direct3D quaternion
        // (x, y, z, w; y is up): a bus standing level has (0, sin a/2, 0, cos a/2) with a
        // its heading. "Linie 5" (0, 0.976, 0, -0.219) is 205.3 degrees, along the road
        // there; reading the pair as sin/cos of the heading itself turned it by half.
        let (x, y) = omsi_map::tile_local_to_world(v.tile.0, v.tile.1, v.pos[0], v.pos[2]);
        let heading = situation_heading(&v.orientation);
        // (with its height: a bus saved on a bridge stays on it)
        args.spawn = Some(spawn_arg(x, y, heading, v.pos[1]));
        if !v.paint.trim().is_empty() {
            args.hof = Some(v.paint.clone());
        }
        args.situation_next_stop = None;
        if v.timetable.len() >= 2 {
            args.line = Some(v.timetable[0].clone());
            args.tour = Some(v.timetable[1].clone());
            args.situation_next_stop = v.timetable.get(3).and_then(|s| s.trim().parse().ok());
            // the third value is the trip of the tour under way (0 = the first); the duty
            // goes on from there with the rest of the tour, as it was driven (taken as a
            // picked trip, it was the whole duty, and the next save wrote it as trip 0 of
            // a one-trip duty: the game after that started at the tour's first trip, #653)
            if let Some(t) = v
                .timetable
                .get(2)
                .and_then(|t| t.trim().parse::<usize>().ok())
            {
                args.trip = Some((t + 1).to_string());
                args.whole_tour = true;
            }
        }
        args.situation_vars = v.vars.iter().map(|(n, x)| (n.clone(), *x as f32)).collect();
        args.situation_odometer_km = saved_odometer_km(v, legacy_odometer_ids);
        // the livery it was driven in: the scheme's index is the `Colorscheme` variable (the
        // bus came back in its default paint - the variable alone repaints nothing)
        if let Some((_, c)) = v
            .vars
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("Colorscheme"))
            .filter(|(_, c)| *c >= 0.0)
        {
            args.paint = Some(format!("{}", *c as i64));
        }
        args.situation_strvars = v.string_vars.clone();
        args.situation_script_textures = script_texture_prefix(&sit.path, i);
        log::info!(
            "situation: player {} at {:?} line {:?} tour {:?} hof {:?}, {} vars",
            v.file,
            args.spawn,
            args.line,
            args.tour,
            args.hof,
            v.vars.len()
        );
    } else {
        log::warn!("situation: no player vehicle found");
    }
    // the vehicles placed besides the one driven: they stand where they were saved, in the
    // state their variables say (lights, doors, the engine), as OMSI loads them
    args.situation_others = sit
        .vehicles
        .iter()
        .enumerate()
        .filter(|(i, v)| Some(*i) != mine && v.coupled_with.is_none())
        .map(|(i, v)| {
            let (x, y) = omsi_map::tile_local_to_world(v.tile.0, v.tile.1, v.pos[0], v.pos[2]);
            let paint = v
                .vars
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case("Colorscheme"))
                .filter(|(_, c)| *c >= 0.0)
                .map(|(_, c)| format!("{}", *c as i64));
            crate::cli::SituationOther {
                bus: content_relative(&v.file.replace('\\', "/"), "vehicles"),
                spawn: spawn_arg(x, y, situation_heading(&v.orientation), v.pos[1]),
                hof: Some(v.paint.clone()).filter(|p| !p.trim().is_empty()),
                paint,
                vars: v.vars.iter().map(|(n, x)| (n.clone(), *x as f32)).collect(),
                strvars: v.string_vars.clone(),
                odometer_km: saved_odometer_km(v, legacy_odometer_ids),
                script_textures: script_texture_prefix(&sit.path, i),
            }
        })
        .collect();
    if !args.situation_others.is_empty() {
        log::info!(
            "situation: {} more vehicle(s) placed",
            args.situation_others.len()
        );
    }
}

/// The files of a vehicle's `[scripttexture]` images saved with a situation, as OMSI
/// names them: the situation file's whole name, then `_<vehicle>_<texture>.dds`
/// (`Slot 1.osn_0_2.dds`), the vehicle counted in the order the file lists them. Without
/// `_<texture>.dds`; None for a situation not read from a file.
fn script_texture_prefix(osn: &Path, vehicle: usize) -> Option<PathBuf> {
    let name = osn.file_name()?.to_string_lossy();
    Some(osn.with_file_name(format!("{name}_{vehicle}")))
}

fn script_texture_file(prefix: &Path, texture: usize) -> PathBuf {
    let name = prefix
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    prefix.with_file_name(format!("{name}_{texture}.dds"))
}

/// Write the `[scripttexture]` images of the situation's vehicles (in the order
/// `build_situation` wrote them) next to `osn`, as OMSI does: what the displays show
/// (destination signs, the IBIS, the ticket printer) is drawn by the scripts only when
/// something changes, so the variables alone bring them back empty (#34). The images of an
/// earlier save under the same name go first: a later bus does not get another's. The
/// number of images written.
pub(crate) fn save_script_textures(osn: &Path, vehicles: &[&omsi_sim::VehicleInstance]) -> usize {
    if let (Some(dir), Some(name)) = (
        osn.parent(),
        osn.file_name().map(|n| n.to_string_lossy().into_owned()),
    ) {
        let stale = |f: &str| {
            f.strip_prefix(name.as_str())
                .and_then(|r| r.strip_prefix('_'))
                .and_then(|r| r.strip_suffix(".dds"))
                .and_then(|r| r.split_once('_'))
                .is_some_and(|(a, b)| {
                    [a, b]
                        .iter()
                        .all(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
                })
        };
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if stale(&e.file_name().to_string_lossy()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let mut written = 0;
    for (v, vehicle) in vehicles.iter().enumerate() {
        let Some(prefix) = script_texture_prefix(osn, v) else {
            continue;
        };
        for (i, st) in vehicle.host.script_textures.iter().enumerate() {
            let file = script_texture_file(&prefix, i);
            let dds = omsi_texture::dds::encode_a8r8g8b8(st.width, st.height, st.shown());
            match std::fs::write(&file, dds) {
                Ok(()) => written += 1,
                Err(e) => log::warn!("situation: script texture {}: {e}", file.display()),
            }
        }
    }
    written
}

/// Draw a vehicle's saved `[scripttexture]` images back into it, as OMSI loads them: each
/// file there takes its texture's place at the file's own size (`ScriptTexture::load`); a
/// texture without a file stays as the scripts left it, and a file that cannot be read
/// leaves it empty, with a line in the log. The number restored.
pub(crate) fn restore_script_textures(
    vehicle: &mut omsi_sim::VehicleInstance,
    prefix: &Path,
) -> usize {
    let mut restored = 0;
    for (i, st) in vehicle.host.script_textures.iter_mut().enumerate() {
        let file = script_texture_file(prefix, i);
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        match omsi_texture::dds::decode(&bytes) {
            Ok(img)
                if img.width <= omsi_sim::scripttex::MAX_SCRIPT_TEXTURE_SIDE
                    && img.height <= omsi_sim::scripttex::MAX_SCRIPT_TEXTURE_SIDE =>
            {
                st.load(img.width, img.height, &img.rgba);
                restored += 1;
            }
            Ok(img) => {
                log::warn!(
                    "situation: script texture {} is {}x{}, above {}; left empty",
                    file.display(),
                    img.width,
                    img.height,
                    omsi_sim::scripttex::MAX_SCRIPT_TEXTURE_SIDE
                );
                st.clear();
            }
            Err(e) => {
                log::warn!(
                    "situation: script texture {} could not be read ({e}); left empty",
                    file.display()
                );
                st.clear();
            }
        }
    }
    restored
}

/// The lowest height (m) a saved vehicle can plausibly stand at. A vehicle that fell through
/// an unloaded tile came back from a save thousands of kilometres under the map (#33).
const LOWEST_PLAUSIBLE_HEIGHT: f64 = -10_000.0;

/// Whether a vehicle's height is one a map can have: finite and not far under the world.
pub(crate) fn plausible_height(z: f64) -> bool {
    z.is_finite() && z > LOWEST_PLAUSIBLE_HEIGHT
}

/// The `--spawn` text `x,y,heading[,z]` of a saved vehicle. An implausible saved height is
/// left out, so the vehicle is put on the ground of its place.
fn spawn_arg(x: f64, y: f64, heading: f64, z: f64) -> String {
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
pub(crate) fn spawn_without_height(spawn: &str) -> Option<(f64, f64)> {
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
fn saved_odometer_km(
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

/// Build a `.osn` situation from the running world: map, clock, weather and every vehicle
/// with its script variables (the player's marked as `[ismyVehicle]`). Like OMSI, only
/// the vehicles the player placed are written - never the AI traffic, which a loaded
/// situation brings back by itself (every stock situation holds exactly one vehicle; the
/// AI cars within 900 m written before came back in OMSI as placed, standing vehicles).
pub(crate) fn build_situation(
    args: &Args,
    world: &scene::World,
    clock: &omsi_sim::SimClock,
    weather: Option<&str>,
    player: Option<&Player>,
    placed: &[Player],
    camera: &Camera,
    duty: Option<&crate::schedule::PlayerDuty>,
    name: &str,
) -> omsi_content::situation::Situation {
    use omsi_content::situation::{Situation, SituationVehicle};
    let rel = |p: &std::path::Path| -> String {
        p.strip_prefix(&args.root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('/', "\\")
    };
    let vehicle_record = |v: &omsi_sim::VehicleInstance, mine: bool| -> SituationVehicle {
        let (tile, (lx, ly)) = omsi_map::world_to_tile_local(v.position.x, v.position.y);
        // the body's rotation as OMSI writes it: a quaternion (x, y, z, w) about the up axis,
        // then three more numbers (zero for a standing vehicle)
        let h = v.heading.to_radians() / 2.0;
        let orientation = [0.0, h.sin(), 0.0, h.cos(), 0.0, 0.0, 0.0, 0.0, 0.0];
        let vars: Vec<(String, f64)> =
            v.ty.program
                .var_names
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    (
                        n.clone(),
                        v.state.vars.get(i).copied().unwrap_or(0.0) as f64,
                    )
                })
                .collect();
        let string_vars: Vec<(String, String)> =
            v.ty.program
                .str_var_names
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    (
                        n.clone(),
                        v.state.str_vars.get(i).cloned().unwrap_or_default(),
                    )
                })
                .collect();
        // (a height under the world - the vehicle fell through an unloaded tile - is not
        // saved: the ground of the place is, #33)
        let z = if plausible_height(v.position.z) {
            v.position.z
        } else {
            world
                .ground_height(v.position.x, v.position.y)
                .unwrap_or(0.0)
        };
        SituationVehicle {
            file: rel(&v.ty.def.path),
            pos: [lx, z, ly],
            orientation,
            tile,
            odometer_km: v.odometer_km(),
            paint: v
                .host
                .hof
                .as_ref()
                .map(|h| h.name.clone())
                .unwrap_or_default(),
            coupled_with: None,
            is_my_vehicle: mine,
            vars,
            string_vars,
            timetable: Vec::new(),
        }
    };
    let mut vehicles = Vec::new();
    let mut my = -1;
    if let Some(p) = player {
        my = 0;
        let mut rec = vehicle_record(&p.vehicle, true);
        // the duty: line, tour, the trip under way and its next stop (OMSI writes two more
        // numbers whose meaning is not settled; they are left at 0 and not read back)
        if let Some(d) = duty {
            rec.timetable = vec![
                d.line.clone(),
                d.tour.clone(),
                (d.first_trip + d.trip_index).to_string(),
                d.next_stop.to_string(),
                "0".into(),
                "0".into(),
            ];
        }
        vehicles.push(rec);
    }
    // and the vehicles placed besides it, standing where they are (only the driven one was
    // written: a session continued with the last one alone, #139)
    for q in placed {
        vehicles.push(vehicle_record(&q.vehicle, false));
    }
    let (cam_tile, cam_local) = omsi_map::world_to_tile_local(camera.position.x, camera.position.y);
    Situation {
        path: Default::default(),
        name: name.to_string(),
        description: format!(
            "Saved by Project Depot at {:02}:{:02}",
            (clock.time / 3600.0) as i32,
            ((clock.time % 3600.0) / 60.0) as i32
        ),
        map: rel(&world.global.path),
        weather: weather.map(|w| w.to_string()),
        time: (
            clock.year,
            clock.day_of_year,
            (clock.time / 3600.0) as i32,
            ((clock.time % 3600.0) / 60.0) as i32,
            (clock.time % 60.0) as f64,
        ),
        // the map camera is written in the centre tile's frame
        center_tile: cam_tile,
        map_cam: vec![
            cam_local.0,
            camera.position.z,
            cam_local.1,
            camera.yaw as f64,
            camera.pitch as f64,
            25.0,
        ],
        ego_pos: vec![10.0, 0.0, 10.0, 0.0, 0.0],
        timetable_active: args.schedule,
        my_vehicle: my,
        view: 0,
        vehicles,
    }
}

/// The heading (degrees clockwise from north) of a situation vehicle's rotation quaternion
/// (x, y, z, w with y up, as OMSI writes it).
pub(crate) fn situation_heading(q: &[f64; 9]) -> f64 {
    (2.0 * q[1].atan2(q[3]).to_degrees()).rem_euclid(360.0)
}

/// The key `Inputs/keyboard.cfg` gives `ticket_give`, as the key names file spells it.
pub(crate) fn ticket_key_name(root: &Path, bindings: &[omsi_content::KeyBinding]) -> String {
    let Some(b) = bindings
        .iter()
        .find(|b| b.action.eq_ignore_ascii_case("ticket_give"))
    else {
        return "T".into();
    };
    let names =
        omsi_content::input::load_key_names(&root.join("Inputs/ENG.kyb")).unwrap_or_default();
    let key = names
        .iter()
        .find(|(c, _)| *c == b.scan_code)
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| format!("key {}", b.scan_code));
    let mut out = String::new();
    for (bit, name) in [
        (omsi_content::input::KEY_SHIFT, "Shift+"),
        (omsi_content::input::KEY_CTRL, "Ctrl+"),
        (omsi_content::input::KEY_ALT, "Alt+"),
    ] {
        if b.modifier & bit != 0 {
            out.push_str(name);
        }
    }
    out + &key
}

/// A content path saved whole (a map or bus from an archive or the content folder:
/// `/Users/…/Archives/x.zip/Maps/Novi Sad/global.cfg`) from its `folder` (`maps`,
/// `vehicles`) on, as the game names content; went after the root, it closed the game.
fn content_relative(path: &str, folder: &str) -> String {
    let p = path.replace('\\', "/");
    if !(p.starts_with('/') || p.get(1..2) == Some(":")) {
        return p;
    }
    match p.to_ascii_lowercase().rfind(&format!("/{folder}/")) {
        Some(i) => p[i + 1..].to_string(),
        None => p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// A vehicle that fell through the map is neither saved nor spawned at its fallen height.
    #[test]
    fn a_fallen_height_is_not_a_spawn_height() {
        assert!(plausible_height(0.0) && plausible_height(-400.0) && plausible_height(2500.0));
        for z in [
            -119_841.0,
            -2_470_000.0,
            f64::NAN,
            f64::NEG_INFINITY,
            f64::INFINITY,
        ] {
            assert!(!plausible_height(z), "{z}");
        }
        assert_eq!(spawn_arg(1.0, 2.0, 90.0, 12.5), "1,2,90,12.5");
        assert_eq!(spawn_arg(1.0, 2.0, 90.0, -2_470_000.0), "1,2,90");
        assert_eq!(spawn_arg(1.0, 2.0, 90.0, f64::NAN), "1,2,90");
        assert_eq!(spawn_without_height("1,2,90"), Some((1.0, 2.0)));
        assert_eq!(spawn_without_height("1,2,90,12.5"), None);
    }

    #[test]
    fn saved_odometer_prefers_the_header_and_recovers_legacy_variables() {
        use omsi_content::situation::SituationVehicle;
        let mut v = SituationVehicle {
            odometer_km: 75556.639,
            vars: vec![
                ("KMCOUNTER_KM".into(), 12345.0),
                ("kmcounter_m".into(), 678.0),
            ],
            ..Default::default()
        };
        assert_eq!(saved_odometer_km(&v, false), Some(75556.639));
        for placeholder in [1.0, 2.0, 8.0, 9.0, 10.0] {
            v.odometer_km = placeholder;
            assert_eq!(saved_odometer_km(&v, true), Some(12345.678));
        }
        v.odometer_km = 1.0;
        assert_eq!(saved_odometer_km(&v, false), Some(12345.678));
        v.vars.clear();
        assert_eq!(saved_odometer_km(&v, true), None);
        for actual in [8.0, 8.5, 9.9] {
            v.odometer_km = actual;
            assert_eq!(saved_odometer_km(&v, false), Some(actual));
        }
        v.odometer_km = f64::NAN;
        v.vars = vec![("kmcounter_km".into(), f64::INFINITY)];
        assert_eq!(saved_odometer_km(&v, false), None);
    }

    #[test]
    fn legacy_odometer_ids_fall_back_for_player_and_placed_buses() {
        use omsi_content::situation::{Situation, SituationVehicle};
        let mut sit = Situation {
            description: "Saved by the openOMSI at 08:45".into(),
            vehicles: (1..=10)
                .map(|id| SituationVehicle {
                    odometer_km: id as f64,
                    is_my_vehicle: id == 1,
                    vars: vec![
                        ("kmcounter_km".into(), 12000.0 + id as f64),
                        ("kmcounter_m".into(), 125.0),
                    ],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let mut args = Args::parse_from(["depot"]);
        apply_situation_parsed(&sit, &mut args);
        assert_eq!(args.situation_odometer_km, Some(12001.125));
        assert_eq!(args.situation_others[0].odometer_km, Some(12002.125));
        assert_eq!(args.situation_others[8].odometer_km, Some(12010.125));
        sit.vehicles[0].vars.clear();
        apply_situation_parsed(&sit, &mut args);
        assert_eq!(args.situation_odometer_km, None);
    }

    #[test]
    fn situation_others_preserve_their_colorscheme() {
        let sit = omsi_content::situation::Situation {
            map: "maps/Berlin/global.cfg".into(),
            vehicles: vec![
                omsi_content::situation::SituationVehicle {
                    file: "Vehicles/MAN_SD200/MAN_SD77.bus".into(),
                    is_my_vehicle: true,
                    odometer_km: 10001.125,
                    vars: vec![("Colorscheme".into(), 1.0)],
                    ..Default::default()
                },
                omsi_content::situation::SituationVehicle {
                    file: "Vehicles/MAN_NL_NG/MAN_EN92.bus".into(),
                    is_my_vehicle: false,
                    odometer_km: 20002.375,
                    vars: vec![("Colorscheme".into(), 4.0)],
                    string_vars: vec![("destination".into(), "  Manual destination  ".into())],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut args = crate::cli::Args::parse_from(["depot"]);
        apply_situation_parsed(&sit, &mut args);
        assert_eq!(args.paint.as_deref(), Some("1"));
        assert_eq!(args.situation_odometer_km, Some(10001.125));
        assert_eq!(args.situation_others.len(), 1);
        assert_eq!(args.situation_others[0].paint.as_deref(), Some("4"));
        assert_eq!(args.situation_others[0].odometer_km, Some(20002.375));
        assert_eq!(
            args.situation_others[0].strvars,
            vec![("destination".into(), "  Manual destination  ".into())]
        );
    }

    /// #653: a saved duty goes on at the trip of the tour it was saved on, with the rest of
    /// the tour after it.
    #[test]
    fn a_saved_duty_goes_on_at_its_trip() {
        let sit = omsi_content::situation::Situation {
            map: "maps/Berlin/global.cfg".into(),
            vehicles: vec![omsi_content::situation::SituationVehicle {
                file: "Vehicles/MAN_SD200/MAN_SD77.bus".into(),
                is_my_vehicle: true,
                timetable: ["137", "4", "3", "2", "0", "0"].map(String::from).to_vec(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut args = crate::cli::Args::parse_from(["depot"]);
        apply_situation_parsed(&sit, &mut args);
        assert_eq!(
            (
                args.line.as_deref(),
                args.tour.as_deref(),
                args.trip.as_deref()
            ),
            (Some("137"), Some("4"), Some("4"))
        );
        assert!(args.whole_tour);
        assert_eq!(args.situation_next_stop, Some(2));
    }

    #[test]
    fn legacy_and_invalid_saved_stop_fields_keep_legacy_selection() {
        let mut args = crate::cli::Args::parse_from(["depot"]);
        for fields in [
            vec!["109", "65104", "16"],
            vec!["109", "65104", "16", "-1"],
            vec!["109", "65104", "16", "invalid"],
            vec![],
        ] {
            args.situation_next_stop = Some(9);
            let sit = omsi_content::situation::Situation {
                vehicles: vec![omsi_content::situation::SituationVehicle {
                    is_my_vehicle: true,
                    timetable: fields.into_iter().map(String::from).collect(),
                    ..Default::default()
                }],
                ..Default::default()
            };
            apply_situation_parsed(&sit, &mut args);
            assert_eq!(args.situation_next_stop, None);
        }
    }

    #[test]
    fn a_string_only_snapshot_is_a_resume() {
        let mut args = crate::cli::Args::parse_from(["depot"]);
        assert!(!args.is_resuming());
        args.situation_strvars
            .push(("destination".into(), "Manual".into()));
        assert!(args.is_resuming());
    }

    #[test]
    fn an_odometer_only_snapshot_is_a_resume() {
        let mut args = Args::parse_from(["depot"]);
        args.situation_odometer_km = Some(75556.639);
        assert!(args.is_resuming());
    }

    /// A vehicle with `[scripttexture]`s of these sizes and no scripts.
    fn with_script_textures(sizes: &[(i32, i32)]) -> omsi_sim::VehicleInstance {
        let mut model = omsi_model::Model::default();
        model.script_textures = sizes.to_vec();
        let ty = std::sync::Arc::new(omsi_sim::VehicleType {
            def: Default::default(),
            model,
            model_dir: PathBuf::new(),
            program: Default::default(),
            meshes: Vec::new(),
            paint_schemes: Vec::new(),
            texchanges: Vec::new(),
            wheel_meshes: Vec::new(),
            suspension_axles: Vec::new(),
            missing_packs: Vec::new(),
            mesh_bounds: Vec::new(),
            mesh_boxes: Vec::new(),
        });
        omsi_sim::VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()))
    }

    /// #34: what the scripts drew (an IBIS, a destination sign) comes back with the
    /// situation, in OMSI's files next to it, for each vehicle saved; a missing or damaged
    /// file leaves its texture empty.
    #[test]
    fn script_textures_come_back_with_the_situation() {
        let dir = std::env::temp_dir().join(format!("depot_sit_scripttex_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let osn = dir.join("Slot 1.osn");
        let mut player = with_script_textures(&[(4, 2), (8, 8)]);
        let mut placed = with_script_textures(&[(2, 2)]);
        player.host.script_textures[0].color = [200, 100, 50, 255];
        player.host.script_textures[0].rect(1, 0, 2, 1);
        placed.host.script_textures[0].color = [1, 2, 3, 4];
        placed.host.script_textures[0].rect(0, 0, 1, 1);
        let drawn = player.host.script_textures[0].rgba.clone();
        // a file of an earlier save under the same name, for a texture this save has not
        std::fs::write(dir.join("Slot 1.osn_0_7.dds"), b"old").unwrap();
        assert_eq!(save_script_textures(&osn, &[&player, &placed]), 3);
        assert!(dir.join("Slot 1.osn_0_0.dds").is_file());
        assert!(dir.join("Slot 1.osn_1_0.dds").is_file());
        assert!(!dir.join("Slot 1.osn_0_7.dds").exists());

        // the situation as read back: each vehicle finds its own files
        let mut args = Args::parse_from(["depot"]);
        let sit = omsi_content::situation::Situation {
            path: osn.clone(),
            vehicles: vec![
                omsi_content::situation::SituationVehicle {
                    is_my_vehicle: true,
                    ..Default::default()
                },
                omsi_content::situation::SituationVehicle::default(),
            ],
            ..Default::default()
        };
        apply_situation_parsed(&sit, &mut args);
        let mine = args.situation_script_textures.clone().unwrap();
        let other = args.situation_others[0].script_textures.clone().unwrap();

        let mut back = with_script_textures(&[(4, 2), (8, 8)]);
        assert_eq!(restore_script_textures(&mut back, &mine), 2);
        assert_eq!(back.host.script_textures[0].rgba, drawn);
        assert!(
            back.host.script_textures[0].take_upload().is_some(),
            "goes up to the GPU"
        );
        let mut back_placed = with_script_textures(&[(2, 2)]);
        assert_eq!(restore_script_textures(&mut back_placed, &other), 1);
        assert_eq!(back_placed.host.script_textures[0].get(1, 1), [1, 2, 3, 4]);

        // a damaged file: an empty texture and a line in the log, no panic
        std::fs::write(dir.join("Slot 1.osn_0_0.dds"), b"DDS broken").unwrap();
        std::fs::remove_file(dir.join("Slot 1.osn_0_1.dds")).unwrap();
        let mut damaged = with_script_textures(&[(4, 2), (8, 8)]);
        damaged.host.script_textures[0].color = [9, 9, 9, 9];
        damaged.host.script_textures[0].rect(0, 0, 3, 1);
        assert_eq!(restore_script_textures(&mut damaged, &mine), 0);
        assert!(damaged.host.script_textures[0].rgba.iter().all(|b| *b == 0));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A buffer the script keeps locked saves the image it last released, not its
    /// half-drawn next one.
    #[test]
    fn a_locked_texture_saves_what_it_shows() {
        let mut st = omsi_sim::scripttex::ScriptTexture::new(2, 1);
        st.put(0, 0, [1, 1, 1, 1]);
        st.unlock();
        st.locked = true;
        st.put(1, 0, [2, 2, 2, 2]);
        assert_eq!(st.shown(), &[1, 1, 1, 1, 0, 0, 0, 0]);
    }
}

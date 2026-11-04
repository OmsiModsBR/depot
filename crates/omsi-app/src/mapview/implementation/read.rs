//! Reading a map off the interface thread: the roads the game's city map draws, the entry
//! points, the objects and the chosen trip's route.

use super::view::Look;
use crate::mapview::domain::roads::{Entry, Roads};
use glam::DVec2;
use hashbrown::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What a worker read.
pub(crate) struct Reply {
    pub(crate) look: Look,
    pub(crate) roads: Option<Arc<Roads>>,
    pub(crate) trip: omsi_launcher_lib::TripPath,
    /// What went wrong, when nothing could be read at all.
    pub(crate) error: Option<String>,
}

/// Read a map the way the map needs it: the roads the game's city map draws, the entry
/// points, the objects - and the chosen trip's route with them.
pub(crate) fn read(look: Look, have: Option<Arc<Roads>>) -> Reply {
    let mut reply = Reply {
        look,
        roads: None,
        trip: Default::default(),
        error: None,
    };
    if let Some(r) = have {
        reply.trip = trip_of(&reply.look);
        reply.roads = Some(r);
        return reply;
    }
    let roads = read_map(&reply.look);
    if roads.roads.is_empty() && roads.objects.is_empty() {
        reply.error = Some(format!("{}", reply.look.global.display()));
    }
    reply.trip = trip_of(&reply.look);
    reply.roads = Some(Arc::new(roads));
    reply
}

/// The route of the chosen trip (nothing when no trip is chosen or the map has a timetable
/// that does not know it).
fn trip_of(look: &Look) -> omsi_launcher_lib::TripPath {
    if look.trip.trim().is_empty() {
        return Default::default();
    }
    match omsi_launcher_lib::trip_path(&look.map, &look.date, &look.trip) {
        Ok(t) => t,
        Err(e) => {
            log::debug!("trip path for {}: {e}", look.trip);
            Default::default()
        }
    }
}

/// The map's own data, read the way the game reads it for its city map: every tile's paths
/// (splines and the crossings' objects), linked, grouped into carriageways with their real
/// widths. `look.global` is the map's `global.cfg`; the tiles stand beside it and the assets
/// the map names under the installation it belongs to.
fn read_map(look: &Look) -> Roads {
    let Ok(global) = omsi_map::GlobalCfg::load(&look.global) else {
        return Roads::default();
    };
    // everything that turns a tile index and a place in the tile into world metres needs
    // this first (`[worldcoordinates]` maps use another grid)
    omsi_map::configure_grid(&global);
    let Some(map_dir) = look.global.parent().map(Path::to_path_buf) else {
        return Roads::default();
    };
    let chrono = omsi_map::date_code(&look.date)
        .map(|c| omsi_map::active_chrono_dirs(&map_dir, c))
        .unwrap_or_default();
    let size = omsi_map::tile_size();
    // the installation the map belongs to: what its `.sli` and `.sco` files are named from.
    // A map is a folder under a content root's `maps` (that is where the launcher's list came
    // from), so the root that holds it is the one its assets are named from; a map outside
    // every root falls back to the folder two steps up, as `<root>/maps/<name>`.
    let root = omsi_cfg::content_roots()
        .into_iter()
        .find(|r| map_dir.starts_with(r))
        .or_else(|| {
            map_dir
                .parent()
                .and_then(|p| p.parent())
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| map_dir.clone());
    let tiles: Vec<(i32, i32, PathBuf)> = global
        .tiles
        .iter()
        .map(|t| (t.x, t.y, omsi_cfg::resolve_path(&map_dir, &t.file)))
        .filter(|t| omsi_cfg::vfs::is_file(&t.2))
        .collect();
    let t0 = std::time::Instant::now();
    let crate::world::scene::NavigationMap {
        lanes,
        road_surfaces,
        places,
        signs,
    } = crate::world::scene::navigation_map_of(&root, &tiles, &chrono);
    let (roads, net) = crate::hud::nav::city_roads(lanes, &road_surfaces);
    // the lanes the timetable can name, and the lanes of every spline behind them
    let mut by_path = HashMap::new();
    let mut by_spline: HashMap<(i32, i32, i64), Vec<usize>> = HashMap::new();
    for (i, l) in net.lanes.iter().enumerate() {
        let Some(key) = l.key else { continue };
        by_path.insert((key.tile.0, key.tile.1, key.id, key.path), i);
        by_spline
            .entry((key.tile.0, key.tile.1, key.id))
            .or_default()
            .push(i);
    }
    // the entry points: on the tile the record names, else at their object (`World::
    // entry_point_place` reads them the same way)
    let mut entries = Vec::new();
    for (index, ep) in global.entry_points.iter().enumerate() {
        let p = usize::try_from(ep.group)
            .ok()
            .and_then(|i| global.raw_tiles.get(i))
            .map(|t| DVec2::new(t.0 as f64 * size + ep.pos[0], t.1 as f64 * size + ep.pos[1]))
            .or_else(|| places.get(&ep.object_id).map(|q| q.truncate()));
        // the marker carries where it stands in `global.cfg`'s list, so one left out here
        // (no tile, no object) does not shift the ones after it
        if let Some(p) = p {
            entries.push(Entry {
                at: p,
                name: ep.name.clone(),
                index,
            });
        }
    }
    let objects: HashMap<i64, DVec2> = places.iter().map(|(k, v)| (*k, v.truncate())).collect();
    // where the roads are, and where the picture is measured from
    let (mut min, mut max) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
    for r in &roads {
        for q in &r.points {
            min = min.min(q.truncate());
            max = max.max(q.truncate());
        }
    }
    if min.x == f64::MAX {
        min = DVec2::ZERO;
        max = DVec2::ZERO;
    }
    let origin = (min + max) * 0.5;
    // Where the roads really are: one spline left in a far corner (an editor's marker, a
    // piece of the next town) would shrink everything else to a dot, so the map opens on the
    // 1 - 99 % box of what the roads cover instead of the extremes (the far ones are still
    // there to be dragged to).
    let mut xs: Vec<f32> = roads
        .iter()
        .flat_map(|r| r.points.iter())
        .map(|q| q.x as f32)
        .collect();
    let mut ys: Vec<f32> = roads
        .iter()
        .flat_map(|r| r.points.iter())
        .map(|q| q.y as f32)
        .collect();
    xs.sort_by(f32::total_cmp);
    ys.sort_by(f32::total_cmp);
    let (lo, hi) = if xs.is_empty() {
        (min, max)
    } else {
        let at = |v: &Vec<f32>, t: f64| v[((v.len() - 1) as f64 * t) as usize] as f64;
        (
            DVec2::new(at(&xs, 0.01), at(&ys, 0.01)),
            DVec2::new(at(&xs, 0.99), at(&ys, 0.99)),
        )
    };
    log::info!(
        "map: {} roads from {} lanes in {} tiles, {} objects, {} entry points, read in {:.2} s; roads cover ({:.0}, {:.0}) - ({:.0}, {:.0}), opened at the 1 - 99 % box ({:.0}, {:.0}) - ({:.0}, {:.0})",
        roads.len(),
        net.lanes.len(),
        tiles.len(),
        objects.len(),
        entries.len(),
        t0.elapsed().as_secs_f64(),
        min.x, min.y, max.x, max.y,
        lo.x, lo.y, hi.x, hi.y
    );
    let mut streets: Vec<(String, DVec2)> = Vec::new();
    for (at, _, name) in signs {
        if !streets.iter().any(|s| s.0.eq_ignore_ascii_case(&name)) {
            streets.push((name, at.truncate()));
        }
    }
    streets.sort_by_key(|s| s.0.to_lowercase());
    Roads {
        roads,
        net: Arc::new(net),
        lanes: by_path,
        splines: by_spline,
        origin,
        lo,
        hi,
        entries,
        objects,
        streets,
        // (after `configure_grid`: the package is put onto this game's grid)
        surface: crate::world::map_surface::MapSurface::open(&map_dir).map(Arc::new),
    }
}

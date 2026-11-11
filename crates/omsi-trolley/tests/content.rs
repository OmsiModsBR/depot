//! The installed trolleybus content: needs `OMSI_ROOT` with the convention's parameter file
//! and a map with wire (`OMSI_TROLLEY_MAP`, a folder under `maps`), and skips without them.

use std::path::PathBuf;

use glam::DVec2;
use omsi_trolley::{build_tile_wires, parse_params, parse_registry, WireNetwork};

fn content() -> Option<(PathBuf, String)> {
    let root = PathBuf::from(std::env::var_os("OMSI_ROOT")?);
    let map = std::env::var("OMSI_TROLLEY_MAP").ok()?;
    Some((root, map))
}

#[test]
fn an_installed_wired_map_gives_a_joined_network() {
    let Some((root, map)) = content() else {
        eprintln!("skipped: set OMSI_ROOT and OMSI_TROLLEY_MAP");
        return;
    };
    let text = |p: PathBuf| omsi_cfg::decode_text(&std::fs::read(p).expect("readable"));
    let params = parse_params(&text(root.join("Plugins/TrolleyWirePlugin.cfg"))).expect("params");
    if let Ok(b) = std::fs::read(root.join("Plugins/TrolleyWireVehicles.ini")) {
        assert!(!parse_registry(&omsi_cfg::decode_text(&b)).is_empty());
    }
    let map_dir = root.join("maps").join(map);
    let global = omsi_map::GlobalCfg::load(&map_dir.join("global.cfg")).expect("global.cfg");
    omsi_map::configure_grid(&global);
    let size = omsi_map::tile_size();
    let mut net = WireNetwork::default();
    for t in &global.tiles {
        let Ok(mut tile) = omsi_map::tile::Tile::load(&map_dir.join(&t.file)) else {
            continue;
        };
        tile.fit_to_world_grid(t.y);
        let origin = DVec2::new(t.x as f64 * size, t.y as f64 * size);
        net.insert_tile(build_tile_wires((t.x, t.y), origin, &tile, &params));
    }
    eprintln!(
        "{} spans, {} joints, {} wire objects",
        net.span_count(),
        net.joints().len(),
        net.objects().count()
    );
    assert!(net.span_count() > 0, "the map has wire splines");
    assert!(!net.joints().is_empty(), "its spans join");
}

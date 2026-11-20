//! The wire spans of an installed map, printed for debugging a map's wire: the splines
//! `OMSI_TROLLEY_DUMP` names (ids, comma separated), and with `OMSI_TROLLEY_DUMP_RADIUS` also
//! every span with an end that near the first one's ends. Needs `OMSI_ROOT`,
//! `OMSI_TROLLEY_MAP` and `OMSI_TROLLEY_PLUGINS` as the switch probe does.
//!
//! `cargo run --release -p omsi-trolley --example wire_dump`

use std::path::PathBuf;

use glam::{DVec2, DVec3};
use omsi_trolley::{build_tile_wires, parse_params, WireNetwork};

fn main() {
    let (Some(root), Ok(map), Some(plugins), Ok(ids)) = (
        std::env::var_os("OMSI_ROOT").map(PathBuf::from),
        std::env::var("OMSI_TROLLEY_MAP"),
        std::env::var_os("OMSI_TROLLEY_PLUGINS").map(PathBuf::from),
        std::env::var("OMSI_TROLLEY_DUMP"),
    ) else {
        eprintln!("set OMSI_ROOT, OMSI_TROLLEY_MAP, OMSI_TROLLEY_PLUGINS and OMSI_TROLLEY_DUMP");
        std::process::exit(2);
    };
    let mut ids: Vec<i64> = ids
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let text = |p: PathBuf| omsi_cfg::decode_text(&std::fs::read(p).expect("readable"));
    let params = parse_params(&text(plugins.join("TrolleyWirePlugin.cfg"))).expect("params");
    let map_dir = root.join("maps").join(map);
    // (`OMSI_TROLLEY_DUMP_RADIUS`: also every span with an end that near the first's ends)
    let radius: Option<f64> = std::env::var("OMSI_TROLLEY_DUMP_RADIUS")
        .ok()
        .and_then(|s| s.parse().ok());
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
    let first = net
        .tiles()
        .flat_map(|(_, t)| t.spans.iter())
        .find(|s| s.spline_id == ids[0])
        .map(|s| s.ends)
        .unwrap();
    let o = first[0].point;
    if let Some(r) = radius {
        let more: Vec<i64> = net
            .tiles()
            .flat_map(|(_, t)| t.spans.iter())
            .filter(|s| {
                s.ends.iter().any(|e| {
                    first
                        .iter()
                        .any(|f| (e.point - f.point).truncate().length() <= r)
                })
            })
            .map(|s| s.spline_id)
            .collect();
        ids.extend(more);
    }
    let rel = |p: DVec3| format!("({:.2}, {:.2})", p.x - o.x, p.y - o.y);
    for (k, (_, t)) in net.tiles().enumerate() {
        for (i, s) in t.spans.iter().enumerate() {
            if !ids.contains(&s.spline_id) {
                continue;
            }
            eprintln!(
                "spline {} ({}), {:.1} m: start {} out {:.2?}, end {} out {:.2?}; L {}..{} R {}..{}",
                s.spline_id,
                s.file,
                s.length,
                rel(s.ends[0].point),
                s.ends[0].outward.truncate(),
                rel(s.ends[1].point),
                s.ends[1].outward.truncate(),
                rel(s.wires[0][0]),
                rel(*s.wires[0].last().unwrap()),
                rel(s.wires[1][0]),
                rel(*s.wires[1].last().unwrap()),
            );
            let _ = (k, i);
        }
    }
    for j in net.joints() {
        let a = net.span(j.a.0).unwrap().spline_id;
        let b = net.span(j.b.0).unwrap().spline_id;
        if ids.contains(&a) || ids.contains(&b) {
            eprintln!(
                "joint {a}:{} - {b}:{} gap {:.2} reversed {} via {:?}",
                j.a.1, j.b.1, j.gap, j.reversed, j.via
            );
        }
    }
    for ob in net.objects() {
        if (ob.position - o).truncate().length() < 40.0 {
            eprintln!(
                "object {:?} at {} heading {:.1}",
                ob.kind,
                rel(ob.position),
                ob.heading_deg
            );
        }
    }
}

//! What the wire's sounds cost each step on an installed map: the poles are driven through
//! every switch of the map, once with the declaration's sounds and once without, and the time
//! of a step is printed for both, with the time the map's wire took to load.
//!
//! Needs `OMSI_ROOT` (the OMSI folder with the map), `OMSI_TROLLEY_MAP` (a folder under
//! `maps`) and `OMSI_TROLLEY_PLUGINS` (the folder with the convention's two files);
//! `OMSI_TROLLEY_VEHICLE` picks the registry entry (else its first).
//!
//! `cargo run --release -p omsi-trolley --example wire_sound_cost`

use std::path::PathBuf;
use std::time::{Duration, Instant};

use glam::{DMat3, DVec2, DVec3};
use omsi_trolley::{
    build_tile_wires, declaration_from, parse_params, parse_registry, BodyPose, PoleInputs,
    PolePhase, PoleSim, TrolleyDeclaration, WireNetwork,
};

/// Metres of wire driven on each side of a switch.
const RUN: f64 = 30.0;

fn main() {
    let (Some(root), Ok(map), Some(plugins)) = (
        std::env::var_os("OMSI_ROOT").map(PathBuf::from),
        std::env::var("OMSI_TROLLEY_MAP"),
        std::env::var_os("OMSI_TROLLEY_PLUGINS").map(PathBuf::from),
    ) else {
        eprintln!("set OMSI_ROOT, OMSI_TROLLEY_MAP and OMSI_TROLLEY_PLUGINS");
        std::process::exit(2);
    };
    let text = |p: PathBuf| omsi_cfg::decode_text(&std::fs::read(p).expect("readable"));
    let params = parse_params(&text(plugins.join("TrolleyWirePlugin.cfg"))).expect("params");
    let entries = parse_registry(&text(plugins.join("TrolleyWireVehicles.ini")));
    let wanted = std::env::var("OMSI_TROLLEY_VEHICLE").unwrap_or_default();
    let entry = entries
        .iter()
        .find(|e| e.name == wanted)
        .unwrap_or(&entries[0]);
    let decl = declaration_from(entry, &params).expect("declaration");
    let map_dir = root.join("maps").join(map);
    let global = omsi_map::GlobalCfg::load(&map_dir.join("global.cfg")).expect("global.cfg");
    omsi_map::configure_grid(&global);
    let size = omsi_map::tile_size();
    let mut wires = Vec::new();
    for t in &global.tiles {
        let Ok(mut tile) = omsi_map::tile::Tile::load(&map_dir.join(&t.file)) else {
            continue;
        };
        tile.fit_to_world_grid(t.y);
        let origin = DVec2::new(t.x as f64 * size, t.y as f64 * size);
        wires.push(build_tile_wires((t.x, t.y), origin, &tile, &params));
    }
    let start = Instant::now();
    let mut net = WireNetwork::default();
    for w in wires {
        net.insert_tile(w);
    }
    let load = start.elapsed();
    let objects = net.objects().count();
    let paths = switch_paths(&net);
    eprintln!(
        "{} tiles, {} spans, {objects} wire objects, {} drives through switches",
        global.tiles.len(),
        net.span_count(),
        paths.len()
    );
    eprintln!("wire put together: {:.1} ms", load.as_secs_f64() * 1e3);
    let silent = TrolleyDeclaration {
        sound: None,
        ..decl.clone()
    };
    // (twice each, the first to warm up)
    for _ in 0..2 {
        let (with, steps) = drive_all(&decl, &net, &paths, params.wire_height);
        let (without, _) = drive_all(&silent, &net, &paths, params.wire_height);
        let per = |d: Duration| d.as_secs_f64() * 1e6 / steps.max(1) as f64;
        eprintln!(
            "{steps} steps: {:.2} us a step with the sounds, {:.2} us without",
            per(with),
            per(without)
        );
    }
}

/// For every joint a switch bridges, a path along the wire through it.
fn switch_paths(net: &WireNetwork) -> Vec<Vec<DVec3>> {
    let centre = |span| {
        let s = net.span(span).unwrap();
        let (l, r): (&Vec<DVec3>, &Vec<DVec3>) = (&s.wires[0], &s.wires[1]);
        (0..l.len().min(r.len()))
            .map(|k| (l[k] + r[k]) / 2.0)
            .collect::<Vec<_>>()
    };
    let first = |pts: &[DVec3], len: f64| {
        let mut out = vec![pts[0]];
        let mut d = 0.0;
        for w in pts.windows(2) {
            d += w[0].distance(w[1]);
            out.push(w[1]);
            if d >= len {
                break;
            }
        }
        out
    };
    let mut paths = Vec::new();
    for j in net
        .joints()
        .iter()
        .filter(|j| j.via.is_some_and(|k| k.is_switch()))
    {
        let (from, to) = (j.a, j.b);
        let mut a = centre(from.0);
        if from.1 == 1 {
            a.reverse();
        }
        let mut a = first(&a, RUN);
        a.reverse();
        let mut b = centre(to.0);
        if to.1 == 1 {
            b.reverse();
        }
        let path: Vec<DVec3> = a.into_iter().chain(first(&b, RUN)).collect();
        if path.len() >= 3 && path[0].distance(path[1]) > 1e-6 {
            paths.push(path);
        }
    }
    paths
}

/// The time the steps of driving every path took, and how many there were.
fn drive_all(
    decl: &TrolleyDeclaration,
    net: &WireNetwork,
    paths: &[Vec<DVec3>],
    wire_height: f64,
) -> (Duration, usize) {
    let pose = |p: DVec3, dir: DVec3| BodyPose {
        position: p - DVec3::Z * wire_height,
        rotation: DMat3::from_rotation_z(-dir.x.atan2(dir.y)),
    };
    let dt = 1.0 / 60.0;
    let step = 20.0 / 3.6 * dt;
    let (mut time, mut steps) = (Duration::ZERO, 0);
    for path in paths {
        let mut sim = PoleSim::default();
        let at = pose(path[0], path[1] - path[0]);
        let press = PoleInputs {
            grab: [true, true],
            ..Default::default()
        };
        sim.step(decl, net, &at, press, dt);
        for _ in 0..150 {
            sim.step(decl, net, &at, PoleInputs::default(), dt);
        }
        if !sim.poles.iter().all(|p| p.phase == PolePhase::OnWire) {
            continue;
        }
        for w in path.windows(2) {
            let len = w[0].distance(w[1]);
            let mut s = 0.0;
            while s < len {
                s = (s + step).min(len);
                let body = pose(w[0].lerp(w[1], s / len), w[1] - w[0]);
                let inputs = PoleInputs {
                    speed_kmh: 20.0,
                    ..Default::default()
                };
                let t = Instant::now();
                sim.step(decl, net, &body, inputs, dt);
                time += t.elapsed();
                steps += 1;
            }
        }
    }
    (time, steps)
}

//! The switches of an installed map, driven through: needs `OMSI_ROOT` (the OMSI folder with
//! the map), `OMSI_TROLLEY_MAP` (a folder under `maps`) and `OMSI_TROLLEY_PLUGINS` (the folder
//! with the convention's two files), and skips without them. For every joint a switch
//! bridges, a vehicle (the registry's `OMSI_TROLLEY_VEHICLE` entry, else its first) drives
//! along the wire from one side of the joint to the other, both ways, and the shoes must keep
//! their sides.

use std::path::PathBuf;

use glam::{DMat3, DVec2, DVec3};
use omsi_trolley::{
    build_tile_wires, declaration_from, parse_params, parse_registry, BodyPose, PoleInputs,
    PolePhase, PoleSim, SpanId, WireNetwork,
};

/// Metres of wire driven on each side of the joint.
const RUN: f64 = 30.0;

fn centre(net: &WireNetwork, span: SpanId) -> Vec<DVec3> {
    let s = net.span(span).unwrap();
    let (l, r) = (&s.wires[0], &s.wires[1]);
    (0..l.len().min(r.len()))
        .map(|k| (l[k] + r[k]) / 2.0)
        .collect()
}

/// The points of `pts` within `len` metres of its start.
fn first(pts: &[DVec3], len: f64) -> Vec<DVec3> {
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
}

#[test]
fn every_switch_of_the_map_keeps_the_shoes_on_their_sides() {
    let (Some(root), Ok(map), Some(plugins)) = (
        std::env::var_os("OMSI_ROOT").map(PathBuf::from),
        std::env::var("OMSI_TROLLEY_MAP"),
        std::env::var_os("OMSI_TROLLEY_PLUGINS").map(PathBuf::from),
    ) else {
        eprintln!("skipped: set OMSI_ROOT, OMSI_TROLLEY_MAP and OMSI_TROLLEY_PLUGINS");
        return;
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
    let mut net = WireNetwork::default();
    for t in &global.tiles {
        let Ok(mut tile) = omsi_map::tile::Tile::load(&map_dir.join(&t.file)) else {
            continue;
        };
        tile.fit_to_world_grid(t.y);
        let origin = DVec2::new(t.x as f64 * size, t.y as f64 * size);
        net.insert_tile(build_tile_wires((t.x, t.y), origin, &tile, &params));
    }
    let wire_height = params.wire_height;
    let joints: Vec<_> = net
        .joints()
        .iter()
        .filter(|j| j.via.is_some_and(|k| k.is_switch()))
        .cloned()
        .collect();
    eprintln!("{} joints bridged by switches", joints.len());
    let (mut runs, mut swaps) = (0, 0);
    for j in &joints {
        for (from, to) in [(j.a, j.b), (j.b, j.a)] {
            // along `from` towards its joined end, then along `to` away from its own
            let mut a = centre(&net, from.0);
            if from.1 == 0 {
                a.reverse();
            }
            a.reverse();
            let mut a = first(&a, RUN);
            a.reverse();
            let mut b = centre(&net, to.0);
            if to.1 == 1 {
                b.reverse();
            }
            let b = first(&b, RUN);
            let mut path: Vec<DVec3> = a.into_iter().chain(b).collect();
            // (the shoes trail the body: it starts some way along, with wire under them)
            let mut d = 0.0;
            let mut k = 0;
            while k + 1 < path.len() && d < 9.0 {
                d += path[k].distance(path[k + 1]);
                k += 1;
            }
            path.drain(..k);
            if path.len() < 3 || path[0].distance(path[1]) < 1e-6 {
                continue;
            }
            let ids = (
                net.span(from.0).unwrap().spline_id,
                net.span(to.0).unwrap().spline_id,
            );
            for (offset, demand) in [(0.0, 0.0), (1.2, 0.0), (-1.2, 0.0), (1.2, 1.0), (-1.2, 1.0)] {
                let pose = |p: DVec3, dir: DVec3, off: f64| {
                    let side = DVec3::new(dir.y, -dir.x, 0.0).normalize_or_zero() * off;
                    BodyPose {
                        position: p - DVec3::Z * wire_height + side,
                        rotation: DMat3::from_rotation_z(-dir.x.atan2(dir.y)),
                    }
                };
                let mut sim = PoleSim::default();
                let at = pose(path[0], path[1] - path[0], 0.0);
                let press = PoleInputs {
                    grab: [true, true],
                    ..Default::default()
                };
                sim.step(&decl, &net, &at, press, 1.0 / 60.0);
                for _ in 0..150 {
                    sim.step(&decl, &net, &at, PoleInputs::default(), 1.0 / 60.0);
                }
                if !sim.poles.iter().all(|p| p.phase == PolePhase::OnWire) {
                    if offset == 0.0 {
                        eprintln!("  {ids:?}: the poles did not take the wire at the start");
                    }
                    continue;
                }
                runs += 1;
                let step = 10.0 / 3.6 / 60.0;
                let mut swapped = None;
                let mut driven = 0.0;
                'drive: for w in path.windows(2) {
                    let len = w[0].distance(w[1]);
                    if len < 1e-9 {
                        continue;
                    }
                    let mut s = 0.0;
                    while s < len {
                        s = (s + step).min(len);
                        driven += step;
                        let p = w[0].lerp(w[1], s / len);
                        let o = sim.step(
                            &decl,
                            &net,
                            &pose(p, w[1] - w[0], offset * (driven / 6.0).min(1.0)),
                            PoleInputs {
                                speed_kmh: 10.0,
                                switch_demand: demand,
                                ..Default::default()
                            },
                            1.0 / 60.0,
                        );
                        if o.wire_present && o.poles[0].shoe_pos.x >= o.poles[1].shoe_pos.x {
                            swapped = Some(p);
                            break 'drive;
                        }
                    }
                }
                if let Some(p) = swapped {
                    swaps += 1;
                    eprintln!(
                    "  SWAP: spline {} -> {} via {:?} (gap {:.2}, reversed {}), offset {offset}, demand {demand}, at ({:.1}, {:.1})",
                    ids.0, ids.1, j.via, j.gap, j.reversed, p.x, p.y
                );
                }
            }
        }
    }
    eprintln!("{runs} drives, {swaps} with the shoes swapped");
    assert_eq!(swaps, 0);
}

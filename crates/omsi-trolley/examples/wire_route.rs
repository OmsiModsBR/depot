//! One route of an installed map driven along its wire, for debugging a map's wire: the
//! splines `OMSI_TROLLEY_ROUTE` names (ids in order, comma separated), and how far each shoe
//! ever is from the nearest drawn wire, and where. Needs `OMSI_ROOT`, `OMSI_TROLLEY_MAP` and
//! `OMSI_TROLLEY_PLUGINS` as the switch probe does; `OMSI_TROLLEY_VEHICLE` picks the entry.
//!
//! `cargo run --release -p omsi-trolley --example wire_route`

use std::path::PathBuf;

use glam::{DMat3, DVec2, DVec3};
use omsi_trolley::{
    build_tile_wires, declaration_from, parse_params, parse_registry, BodyPose, PoleInputs,
    PoleSim, SpanId, WireNetwork,
};

fn centre(net: &WireNetwork, span: SpanId) -> Vec<DVec3> {
    let s = net.span(span).unwrap();
    let (l, r) = (&s.wires[0], &s.wires[1]);
    (0..l.len().min(r.len()))
        .map(|k| (l[k] + r[k]) / 2.0)
        .collect()
}

fn main() {
    let (Some(root), Ok(map), Some(plugins), Ok(route)) = (
        std::env::var_os("OMSI_ROOT").map(PathBuf::from),
        std::env::var("OMSI_TROLLEY_MAP"),
        std::env::var_os("OMSI_TROLLEY_PLUGINS").map(PathBuf::from),
        std::env::var("OMSI_TROLLEY_ROUTE"),
    ) else {
        eprintln!("set OMSI_ROOT, OMSI_TROLLEY_MAP, OMSI_TROLLEY_PLUGINS and OMSI_TROLLEY_ROUTE");
        std::process::exit(2);
    };
    let route: Vec<i64> = route
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
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
    let find = |id: i64| {
        net.tiles()
            .flat_map(|(k, t)| t.spans.iter().enumerate().map(move |(i, s)| ((k, i), s)))
            .find(|(_, s)| s.spline_id == id)
            .map(|(k, _)| k)
            .unwrap()
    };
    // the centre line along the route, each span the way it is driven
    let mut path: Vec<DVec3> = Vec::new();
    for (n, id) in route.iter().enumerate() {
        let mut c = centre(&net, find(*id));
        let flip = match (path.last(), n + 1 < route.len()) {
            (Some(last), _) => c[0].distance(*last) > c[c.len() - 1].distance(*last),
            (None, true) => {
                let next = centre(&net, find(route[n + 1]));
                let near = |p: DVec3| p.distance(next[0]).min(p.distance(next[next.len() - 1]));
                near(c[0]) < near(c[c.len() - 1])
            }
            (None, false) => false,
        };
        if flip {
            c.reverse();
        }
        path.extend(c);
    }
    path.dedup_by(|a, b| a.distance(*b) < 1e-6);
    let o = path[0];
    let wire_height = params.wire_height;
    let demand: f64 = std::env::var("OMSI_TROLLEY_DEMAND")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    for offset in [0.0, 1.2, -1.2] {
        let pose = |p: DVec3, dir: DVec3, off: f64| {
            let side = DVec3::new(dir.y, -dir.x, 0.0).normalize_or_zero() * off;
            BodyPose {
                position: p - DVec3::Z * wire_height + side,
                rotation: DMat3::from_rotation_z(-dir.x.atan2(dir.y)),
            }
        };
        // start some metres in
        let mut k = 0;
        let mut d = 0.0;
        while k + 1 < path.len() && d < 9.0 {
            d += path[k].distance(path[k + 1]);
            k += 1;
        }
        let path = &path[k..];
        let mut sim = PoleSim::default();
        let at = pose(path[0], path[1] - path[0], 0.0);
        sim.step(
            &decl,
            &net,
            &at,
            PoleInputs {
                grab: [true, true],
                ..Default::default()
            },
            1.0 / 60.0,
        );
        for _ in 0..150 {
            sim.step(&decl, &net, &at, PoleInputs::default(), 1.0 / 60.0);
        }
        eprintln!(
            "offset {offset}: raised {:?}, path {:.0} m",
            sim.poles.map(|p| p.phase),
            path.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>()
        );
        let step = 10.0 / 3.6 / 60.0;
        let mut driven = 0.0;
        let mut worst: [(f64, DVec3, String); 2] = Default::default();
        let mut before: Option<[(f64, f64, f64); 2]> = None;
        let mut jump: [(f64, String); 2] = Default::default();
        for w in path.windows(2) {
            let len = w[0].distance(w[1]);
            if len < 1e-9 {
                continue;
            }
            let mut s = 0.0;
            while s < len {
                s = (s + step).min(len);
                driven += step;
                let p = w[0].lerp(w[1], s / len);
                let body = pose(p, w[1] - w[0], offset * (driven / 6.0).min(1.0));
                let out = sim.step(
                    &decl,
                    &net,
                    &body,
                    PoleInputs {
                        speed_kmh: 10.0,
                        switch_demand: demand,
                        ..Default::default()
                    },
                    1.0 / 60.0,
                );
                for e in &sim.events {
                    if matches!(
                        e,
                        omsi_trolley::PoleEvent::WireLost { .. }
                            | omsi_trolley::PoleEvent::Dewired { .. }
                    ) {
                        eprintln!("  forward: {e:?}");
                    }
                }
                let now = out.poles.map(|q| (q.yaw, q.pitch, q.shoe_rot));
                if let Some(b) = before {
                    for i in 0..2 {
                        let d = (now[i].0 - b[i].0)
                            .abs()
                            .max((now[i].1 - b[i].1).abs())
                            .max((now[i].2 - b[i].2).abs());
                        if d > jump[i].0 {
                            let on = sim.poles[i]
                                .cursor
                                .map(|c| (net.span(c.span).unwrap().spline_id, c.side));
                            jump[i] = (
                                d,
                                format!(
                                    "{:?} -> {:?} on {on:?} at ({:.1}, {:.1}), events {:?}",
                                    b[i],
                                    now[i],
                                    p.x - o.x,
                                    p.y - o.y,
                                    sim.events
                                ),
                            );
                        }
                    }
                }
                before = Some(now);
                for i in 0..2 {
                    if !out.poles[i].on_wire {
                        continue;
                    }
                    let shoe = body.position + body.rotation * out.poles[i].shoe_pos
                        - body.rotation * DVec3::Z * decl.params.shoe_height_offset;
                    let off = net
                        .segments_near(shoe, 3.0)
                        .first()
                        .and_then(|(r, _)| {
                            let (a, b) = net.segment(*r)?;
                            let ab = b - a;
                            let u = ((shoe - a).dot(ab) / ab.length_squared().max(1e-12))
                                .clamp(0.0, 1.0);
                            Some((a.lerp(b, u) - shoe).length())
                        })
                        .unwrap_or(9.9);
                    // (or from the wire a switch or crossing draws across its gap: the
                    // straight between the joined wires' tips, either side)
                    let tip = |sp: SpanId, e: usize, side: usize| {
                        let w = &net.span(sp).unwrap().wires[side];
                        if e == 0 {
                            w[0]
                        } else {
                            w[w.len() - 1]
                        }
                    };
                    let mut off = off;
                    for j in net.joints() {
                        for (sa, sb) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                            let (a, b) = (tip(j.a.0, j.a.1, sa), tip(j.b.0, j.b.1, sb));
                            if (a - shoe).length() > 8.0 {
                                continue;
                            }
                            let ab = b - a;
                            let u = ((shoe - a).dot(ab) / ab.length_squared().max(1e-12))
                                .clamp(0.0, 1.0);
                            off = off.min((a.lerp(b, u) - shoe).length());
                        }
                    }
                    if off > worst[i].0 {
                        let on = sim.poles[i]
                            .cursor
                            .map(|c| (net.span(c.span).unwrap().spline_id, c.side));
                        worst[i] = (off, shoe - o, format!("{on:?}"));
                    }
                }
            }
        }
        // then back the way it came, 15 m (the poles trail ahead: as a bus reversing)
        let mut back = 0.0;
        'reverse: for w in path.windows(2).rev() {
            let len = w[0].distance(w[1]);
            if len < 1e-9 {
                continue;
            }
            let mut s = len;
            while s > 0.0 {
                s = (s - step).max(0.0);
                back += step;
                if back > 15.0 {
                    break 'reverse;
                }
                let p = w[0].lerp(w[1], s / len);
                let body = pose(p, w[1] - w[0], offset);
                let out = sim.step(
                    &decl,
                    &net,
                    &body,
                    PoleInputs {
                        speed_kmh: -5.0,
                        ..Default::default()
                    },
                    1.0 / 60.0,
                );
                for e in &sim.events {
                    if !matches!(e, omsi_trolley::PoleEvent::Joint { .. }) {
                        eprintln!("  reversing {back:.1} m: {e:?}");
                    }
                }
                let _ = out;
            }
        }
        eprintln!(
            "offset {offset}: after reversing, {:?}",
            sim.poles.map(|p| p.phase)
        );
        for (i, j) in jump.iter().enumerate() {
            eprintln!(
                "offset {offset}: {} pole's largest change in a frame {:.1} deg: {}",
                ["left", "right"][i],
                j.0,
                j.1
            );
        }
        for (i, w) in worst.iter().enumerate() {
            eprintln!(
                "offset {offset}: {} shoe at most {:.2} m from a wire, at ({:.1}, {:.1}) on {}",
                ["left", "right"][i],
                w.0,
                w.1.x,
                w.1.y,
                w.2
            );
        }
    }
}

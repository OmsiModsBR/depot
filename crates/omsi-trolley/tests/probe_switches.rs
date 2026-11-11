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
                // (slowly: a shoe stands in each gap for a while)
                let step = 1.0 / 3.6 / 60.0;
                let mut crossings = [0usize; 2];
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
                        for e in &sim.events {
                            if let omsi_trolley::PoleEvent::Joint { pole, .. } = e {
                                crossings[*pole] += 1;
                            }
                        }
                        if o.wire_present && o.poles[0].shoe_pos.x >= o.poles[1].shoe_pos.x {
                            let on = sim.poles.map(|q| {
                                q.cursor.map(|c| {
                                    (net.span(c.span).unwrap().spline_id, c.side, c.seg, c.t)
                                })
                            });
                            eprintln!(
                            "    shoes {:?} / {:?}, shoe x {:.2} / {:.2}, yaw {:.1} / {:.1}, events {:?}",
                            on[0],
                            on[1],
                            o.poles[0].shoe_pos.x,
                            o.poles[1].shoe_pos.x,
                            o.poles[0].yaw,
                            o.poles[1].yaw,
                            sim.events
                        );
                            swapped = Some(p);
                            break 'drive;
                        }
                    }
                }
                if crossings.iter().any(|c| *c > 20) {
                    swaps += 1;
                    eprintln!(
                        "  BACK AND FORTH: spline {} -> {} via {:?}: joints crossed {crossings:?}",
                        ids.0, ids.1, j.via
                    );
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

#[test]
fn dump_spans() {
    let (Some(root), Ok(map), Some(plugins), Ok(ids)) = (
        std::env::var_os("OMSI_ROOT").map(PathBuf::from),
        std::env::var("OMSI_TROLLEY_MAP"),
        std::env::var_os("OMSI_TROLLEY_PLUGINS").map(PathBuf::from),
        std::env::var("OMSI_TROLLEY_DUMP"),
    ) else {
        return;
    };
    let ids: Vec<i64> = ids
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let text = |p: PathBuf| omsi_cfg::decode_text(&std::fs::read(p).expect("readable"));
    let params = parse_params(&text(plugins.join("TrolleyWirePlugin.cfg"))).expect("params");
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
    let o = net
        .tiles()
        .flat_map(|(_, t)| t.spans.iter())
        .find(|s| ids.contains(&s.spline_id))
        .map(|s| s.ends[0].point)
        .unwrap();
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

/// One route of an installed map (`OMSI_TROLLEY_ROUTE`: spline ids in order), driven along
/// its wire: how far each shoe ever is from the nearest drawn wire, and where.
#[test]
fn drive_route() {
    let (Some(root), Ok(map), Some(plugins), Ok(route)) = (
        std::env::var_os("OMSI_ROOT").map(PathBuf::from),
        std::env::var("OMSI_TROLLEY_MAP"),
        std::env::var_os("OMSI_TROLLEY_PLUGINS").map(PathBuf::from),
        std::env::var("OMSI_TROLLEY_ROUTE"),
    ) else {
        return;
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

//! The corner map: following the bus and its route, the traffic on the roads, and the
//! panel with its header (`picture` draws it).

use super::draw::level;
use super::geometry::{confirm_road_surfaces, congestion_on};
use super::route::probe_lanes;
use super::streets::build_streets;
use super::{
    angle_diff, ease, map_samples, placed_at, CityMap, NavFrame, NavStop, Navigator, Route,
    NAV_REDRAW_S, OFF_ROUTE_AFTER, REROUTE_EVERY,
};
use glam::{DVec2, DVec3};
use hashbrown::HashMap;
use omsi_render::{Renderer, Scene};
use omsi_sim::traffic::{LaneKind, Network};
use omsi_ui::{Atlas, Fonts, Gpu};

pub(crate) mod picture;

impl Navigator {
    /// The maps' Gpu (the small map's and the city map's), handing its draws to the
    /// D3D9Ex mirror when there is one.
    pub(super) fn new_gpu(&mut self, renderer: &Renderer) -> Gpu {
        let mut gpu = Gpu::new(
            &renderer.device,
            renderer.format(),
            map_samples(renderer.format()),
            self.atlas.size,
        );
        gpu.set_mirror(self.ui_mirror.take());
        gpu
    }

    pub fn new(enabled: bool, opacity: f32, corner: &str) -> Navigator {
        Navigator {
            panel_overlay: None,
            cockpit_display: false,
            drawn_at: f32::MIN,
            enabled,
            schedule: false,
            speed_avg: 8.0,
            opacity: opacity.clamp(0.2, 1.0),
            corner: corner.to_string(),
            at: placed_at(corner),
            panel_drag: None,
            panel_room: [0.0; 2],
            city: CityMap::default(),
            panel_rect: [0.0; 4],
            origin_x: 0.0,
            gpu: None,
            ui_mirror: None,
            fonts: Fonts::new(),
            atlas: Atlas::new(1024),
            target: None,
            own_net: None,
            global: None,
            stop_pos: Default::default(),
            streets: None,
            surface: None,
            surface_labels: Vec::new(),
            surface_version: 0,
            show_surface: true,
            show_contours: false,
            orient: 0,
            view_top: false,
            icon_tex: None,
            loc_city: None,
            loc_street: None,
            loc_limit: (0, 0),
            loc_alt: None,
            light: false,
            surface_mesh: None,
            building: None,
            global_version: 0,
            roads: None,
            route: Route::default(),
            route_mesh: (u64::MAX, 0, DVec2::ZERO, 0, 0),
            congestion: HashMap::new(),
            route_jam: HashMap::new(),
            jam_version: 0,
            congestion_t: 0.0,
            zoom: 120.0,
            cam_heading: 0.0,
            time: 0.0,
            next_dist: None,
            arrows: false,
            show_ai: true,
            shown: if enabled { 1.0 } else { 0.0 },
            stop_spots: Vec::new(),
            bus_at: DVec3::ZERO,
            bus_marker: None,
            next_turn: None,
            street_here: None,
            dist_t: 0.0,
            jam_cost: 0.0,
            first: true,
            day_clock: (0.0, 0),
            date: 0,
            chrono: Vec::new(),
        }
    }

    /// Read the whole map's road network on a worker (once per session).
    pub fn start_map(&mut self, world: std::sync::Arc<crate::scene::World>) {
        if self.building.is_some() || self.global.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("navigator map".into())
            .spawn(move || {
                let surface = crate::map_surface::MapSurface::open(&world.map_dir);
                let m = world.navigation_map();
                let mut net = Network {
                    lanes: m.lanes,
                    ..Default::default()
                };
                net.link(1.5);
                confirm_road_surfaces(&mut net, &m.road_surfaces);
                probe_lanes(&net);
                let streets = build_streets(&net, &m.signs);
                let _ = tx.send((
                    net,
                    m.places,
                    streets,
                    surface,
                    world.chrono_dirs.read().clone(),
                ));
            })
            .ok();
        self.building = Some(rx);
    }

    /// The map's network given at once (an offscreen picture reads it on its own thread).
    pub fn set_map(&mut self, map: crate::scene::NavigationMap) {
        let mut net = Network {
            lanes: map.lanes,
            ..Default::default()
        };
        net.link(1.5);
        confirm_road_surfaces(&mut net, &map.road_surfaces);
        probe_lanes(&net);
        self.streets = Some(std::sync::Arc::new(build_streets(&net, &map.signs)));
        let global = std::sync::Arc::new(net);
        self.global = Some(global);
        self.stop_pos = std::sync::Arc::new(map.places);
        self.global_version += 1;
    }

    /// The eras in force, given at once (an offscreen picture).
    pub fn set_chrono(&mut self, dirs: Vec<std::path::PathBuf>) {
        self.chrono = dirs;
    }

    /// The map's package, given at once (an offscreen picture reads it on its own thread).
    pub fn set_surface(&mut self, surface: Option<crate::map_surface::MapSurface>) {
        if let Some(s) = surface {
            let mut labels = s.labels();
            labels.sort_by(|a, b| b.rank.cmp(&a.rank));
            self.surface_labels = labels;
            self.surface = Some(std::sync::Arc::new(s));
            self.surface_version += 1;
        }
    }

    /// The places of every object on the map (stops beyond the loaded tiles), once read.
    pub fn places(&self) -> Option<&HashMap<i64, DVec3>> {
        self.global.as_ref().map(|_| &*self.stop_pos)
    }

    /// The whole map's network, once read.
    pub fn map_net(&self) -> Option<&Network> {
        self.global.as_deref()
    }

    /// The map's lanes, for a session without a traffic system (added as tiles stream in).
    pub fn add_lanes(&mut self, lanes: Vec<omsi_sim::traffic::Lane>) {
        if lanes.is_empty() {
            return;
        }
        match self.own_net.as_mut() {
            Some(n) => {
                // (only `draw` borrows it, for the length of a frame)
                if let Some(n) = std::sync::Arc::get_mut(n) {
                    n.extend(lanes, 1.5);
                    n.build_grid();
                }
            }
            None => {
                let mut n = Network {
                    lanes,
                    ..Default::default()
                };
                n.link(1.5);
                n.build_grid();
                self.own_net = Some(std::sync::Arc::new(n));
            }
        }
    }

    /// Does the route of trip `key` need (re)building? (a new trip, or tiles brought the
    /// lanes a partial route was missing)
    pub fn wants_route(&self, key: &str, generation: u64) -> bool {
        if self.global.is_some() {
            return self.route.key != key
                || self.route.generation != self.global_version + (1 << 40);
        }
        self.route.key != key || (!self.route.complete && self.route.generation != generation)
    }

    /// The route of trip `key`: its lanes as the timetable drives them.
    pub fn set_route(&mut self, key: &str, lanes: Vec<usize>, complete: bool, generation: u64) {
        let same_trip = self.route.key == key;
        self.route.key = key.to_string();
        self.route.complete = complete;
        self.route.generation = generation;
        if same_trip
            && !self.route.lanes.is_empty()
            && !self.route.on_route
            && !self.route.provisional
        {
            // a partial route grew while the driver was off it: keep the way back
            return;
        }
        if self.route.provisional && lanes.is_empty() {
            return;
        }
        self.route.provisional = false;
        self.route.lanes = lanes;
        if !same_trip {
            self.route.progress = 0;
            self.route.on_route = false;
            self.route.off_for = 0.0;
        }
        self.route.version += 1;
    }

    /// No duty: nothing to follow.
    pub fn clear_route(&mut self) {
        if !self.route.key.is_empty() || !self.route.lanes.is_empty() {
            self.route = Route {
                version: self.route.version + 1,
                ..Route::default()
            };
        }
    }

    /// Where the bus is along the route, and a way back when it left it.
    fn follow(&mut self, f: &NavFrame) {
        let global = self.global.clone();
        let Some(net) = global.as_deref().or(f.traffic.map(|t| &t.net)) else {
            return;
        };
        let r = &mut self.route;
        if r.lanes.is_empty() {
            // no route (yet): the way to the next stop, looked for now and then
            let Some(stop) = f.stops.first() else { return };
            r.retry_in -= f.dt;
            if r.retry_in > 0.0 {
                return;
            }
            r.retry_in = REROUTE_EVERY * 2.0;
            let targets: Vec<usize> = net
                .lanes_near(stop.position.truncate(), 40.0)
                .into_iter()
                .filter(|&i| {
                    net.lanes[i].kind == LaneKind::Street
                        && net.lanes[i]
                            .nearest_point(stop.position)
                            .map(|p| p.1 < 20.0)
                            .unwrap_or(false)
                })
                .collect();
            if let Some((mut path, k)) =
                net.shortest_path_to_any(f.bus, f.heading, &targets, 30_000.0)
            {
                path.push(targets[k]);
                log::info!(
                    "navigator: no route of the trip here yet; {} lanes to the next stop '{}'",
                    path.len(),
                    stop.name.trim()
                );
                r.lanes = path;
                r.progress = 0;
                r.s = 0.0;
                r.version += 1;
                r.provisional = true;
            }
            return;
        }
        r.note = (r.note - f.dt).max(0.0);
        // where the next stop is on the route: the way to the route must not skip it
        let stop_at = f.stops.first().and_then(|st| {
            (r.progress..r.lanes.len().min(r.progress + 1500)).find(|&k| {
                net.lanes
                    .get(r.lanes[k])
                    .and_then(|l| l.nearest_point(st.position))
                    .map(|p| p.1 < 25.0)
                    .unwrap_or(false)
            })
        });
        // the nearest route lane near the bus that runs its way, a little back to a lot ahead
        // (before the bus first reached the route: only the stretch up to the next stop)
        let (from, to) = match (r.joined, stop_at) {
            (false, Some(k)) if r.approach => {
                (r.progress.saturating_sub(3), (k + 2).min(r.lanes.len()))
            }
            (false, Some(k)) => (
                k.saturating_sub(40).max(r.progress),
                (k + 2).min(r.lanes.len()),
            ),
            _ => (
                r.progress.saturating_sub(3),
                (r.progress + 60).min(r.lanes.len()),
            ),
        };
        let mut best: Option<(usize, f32, f64)> = None;
        for k in from..to {
            let Some(l) = net.lanes.get(r.lanes[k]) else {
                continue;
            };
            let Some((s, d)) = l.nearest_point(f.bus) else {
                continue;
            };
            // (a wide road, a stop bay, a lane change or a turn in progress: the bus is still
            // on its route well beyond the lane's middle; at 11 m and 75 deg it counted as off
            // and the navigator recalculated for ever)
            if d > 16.0 {
                continue;
            }
            let (_, h) = l.at(s);
            if angle_diff(f.heading, h as f64).abs() > 100.0 {
                continue;
            }
            // (ahead of where the bus was preferred: a route crossing itself)
            let score = d
                + if k < r.progress { 4.0 } else { 0.0 }
                + (k.saturating_sub(r.progress) as f64) * 0.05;
            if best.map(|b| score < b.2).unwrap_or(true) {
                best = Some((k, s, score));
            }
        }
        match best {
            Some((k, s, _)) => {
                if k != r.progress {
                    r.version += 1;
                }
                r.progress = k;
                r.s = s;
                r.on_route = true;
                r.joined = true;
                r.off_for = 0.0;
            }
            None => {
                r.on_route = false;
                r.off_for += f.dt;
            }
        }
        // the way to the route: at once before the bus first reached it (from the depot or
        // wherever it starts), after a moment off it once driving it
        if r.on_route || (r.joined && r.off_for < OFF_ROUTE_AFTER) {
            return;
        }
        r.retry_in -= f.dt;
        if r.retry_in > 0.0 {
            return;
        }
        r.retry_in = REROUTE_EVERY;
        let base = r.progress.min(r.lanes.len() - 1);
        // targets: the route from where the bus left it up to the next stop (joining it
        // later would skip that stop), or on for 120 lanes when no stop is ahead
        let (lo, hi) = match stop_at {
            Some(k) => (k.saturating_sub(150).max(base), k + 1),
            None => (base, (base + 120).min(r.lanes.len())),
        };
        // (from the depot to the first stop may be a long way; back onto the route is not)
        let way = net.shortest_path_to_any(
            f.bus,
            f.heading,
            &r.lanes[lo..hi],
            if r.joined { 6000.0 } else { 30_000.0 },
        );
        if way.is_none() && omsi_cfg::flags::DEBUG_NAV.is_set() {
            let info: Vec<_> = r.lanes[lo..hi]
                .iter()
                .map(|&l| {
                    (
                        l,
                        net.lanes[l].kind,
                        net.lanes[l].name.clone(),
                        net.lanes.iter().filter(|x| x.next.contains(&l)).count(),
                        net.lanes[l].start(),
                    )
                })
                .collect();
            log::info!(
                "navigator: off the route for {:.1} s and no way back found; targets {info:?}",
                r.off_for
            );
        }
        // the route's own lanes before a stop may be cut off from the road network (the
        // map links nothing into the start of line 31's trip at Maulbeerallee): then a
        // road past the stop running the route's way, else the route just after the stop
        let max = if r.joined { 6000.0 } else { 30_000.0 };
        let way = way.map(|(p, j)| (p, lo + j)).or_else(|| {
            let k = stop_at?;
            let st = f.stops.first()?;
            let (_, h) = net.lanes[r.lanes[k]].nearest_point(st.position).map(|(s, _)| net.lanes[r.lanes[k]].at(s))?;
            let near: Vec<usize> = net.lanes_near(st.position.truncate(), 40.0)
                .into_iter()
                .filter(|&i| {
                    let l = &net.lanes[i];
                    l.kind == LaneKind::Street
                        && l.nearest_point(st.position).map(|(s, d)| d < 20.0 && angle_diff(l.at(s).1 as f64, h as f64).abs() < 60.0).unwrap_or(false)
                })
                .collect();
            let (mut path, j) = net.shortest_path_to_any(f.bus, f.heading, &near, max)?;
            path.push(near[j]);
            log::info!("navigator: the route before stop '{}' cannot be reached; led onto the road past it", st.name.trim());
            Some((path, k + 1))
        })
            .or_else(|| {
                let k = stop_at?;
                let hi = (k + 120).min(r.lanes.len());
                (k + 1 < hi).then_some(())?;
                net.shortest_path_to_any(f.bus, f.heading, &r.lanes[k + 1..hi], max).map(|(p, j)| (p, k + 1 + j))
            });
        if let Some((path, join)) = way {
            let rest = r.lanes[join.min(r.lanes.len())..].to_vec();
            log::info!(
                "navigator: {} of {} lanes joins the route {} lanes on{}",
                if r.joined {
                    "a way back"
                } else {
                    "the way to the route"
                },
                path.len(),
                join,
                stop_at
                    .map(|k| format!(" (the next stop is on route lane {k})"))
                    .unwrap_or_default()
            );
            let mut lanes = path;
            lanes.extend(rest);
            // (drawn from where the bus is on its first lane, not from that lane's start)
            r.s = lanes
                .first()
                .and_then(|&l| net.lanes.get(l))
                .and_then(|l| l.nearest_point(f.bus))
                .map(|p| p.0)
                .unwrap_or(0.0);
            r.lanes = lanes;
            r.progress = 0;
            r.version += 1;
            if r.joined {
                r.note = 4.0;
                r.on_route = true;
                r.off_for = 0.0;
            } else {
                r.approach = true;
            }
        }
    }

    /// How congested the lanes near the bus are: a lane with cars on it is light traffic,
    /// more so the fuller it is; cars crawling or standing in a row make it heavy or a jam
    /// (one car waiting at a light is not a jam). Smoothed over seconds.
    fn update_congestion(&mut self, f: &NavFrame) {
        self.congestion_t -= f.dt;
        if self.congestion_t > 0.0 {
            return;
        }
        let step = 0.25f32;
        self.congestion_t = step;
        let Some(t) = f.traffic else { return };
        let mut by_lane: HashMap<usize, (f32, u32)> = HashMap::new();
        for c in &t.cars {
            if c.gone || (c.vehicle.position - f.bus).truncate().length() > 1200.0 {
                continue;
            }
            let e = by_lane.entry(c.state.lane).or_insert((0.0, 0));
            e.0 += c.state.speed.max(0.0);
            e.1 += 1;
        }
        let k = ease(step, 4.0) as f32;
        for v in self.congestion.values_mut() {
            *v -= *v * k;
        }
        for (lane, (sum, n)) in by_lane {
            let Some(l) = t.net.lanes.get(lane) else {
                continue;
            };
            let expected = (l.speed_limit_kmh.clamp(20.0, 70.0) / 3.6) * 0.75;
            let slow = (1.0 - (sum / n as f32) / expected).clamp(0.0, 1.0);
            let full = (n as f32 * 7.5 / l.length().max(25.0)).min(1.0);
            let light = 0.22 + 0.33 * full;
            let jam = slow * (n as f32 / 3.0).min(1.0);
            let score = if jam > 0.2 {
                light.max(0.3 + 0.7 * jam)
            } else {
                light
            };
            let v = self.congestion.entry(lane).or_insert(0.0);
            *v += (score - *v) * k;
        }
        self.congestion.retain(|_, v| *v > 0.03);
        // the same for the route ahead, in the route's own network
        let global = self.global.clone();
        let jam = match global.as_deref() {
            Some(g) => congestion_on(g, &t.net, &self.congestion),
            None => self.congestion.clone(),
        };
        let net = global.as_deref().unwrap_or(&t.net);
        let r = &self.route;
        let mut route_jam = HashMap::new();
        let mut cost = 0.0;
        for &l in r.lanes.iter().skip(r.progress).take(600) {
            let Some(&c) = jam.get(&l) else { continue };
            route_jam.insert(l, c);
            // the time the jams ahead cost
            if let Some(lane) = net.lanes.get(l) {
                if c > 0.6 {
                    let v_free = lane.speed_limit_kmh.clamp(20.0, 70.0) / 3.6;
                    let v = (v_free * (1.0 - c)).max(1.2);
                    cost += lane.length() / v - lane.length() / v_free;
                }
            }
        }
        self.jam_cost = cost;
        let changed = route_jam.len() != self.route_jam.len()
            || route_jam.iter().any(|(l, c)| {
                self.route_jam
                    .get(l)
                    .map(|o| level(*o) != level(*c))
                    .unwrap_or(true)
            });
        self.route_jam = route_jam;
        if changed {
            self.jam_version += 1;
        }
    }

    /// The first real turn on the route within 1.5 km: a lane whose heading changes by more
    /// than 35 degrees from its start to its end (a junction's curve), or a sharp bend
    /// between two lanes. Gentle curves of a road are not turns.
    fn turn_ahead(&self, net: &Network) -> Option<(i32, f32, f64, Option<String>)> {
        let r = &self.route;
        if !r.on_route {
            return None;
        }
        let mut acc = -(r.s as f64);
        let mut prev_end: Option<f32> = None;
        for (j, &l) in r.lanes.iter().enumerate().skip(r.progress) {
            let lane = net.lanes.get(l)?;
            let len = lane.length();
            if acc > 1500.0 {
                break;
            }
            let (h0, h1) = (lane.start_heading(), lane.end_heading());
            let mut d = omsi_sim::traffic::wrap_deg(h1 - h0);
            if let Some(pe) = prev_end {
                d += omsi_sim::traffic::wrap_deg(h0 - pe);
            }
            // (a short junction lane turns within a few metres; a long road curve does not count)
            if d.abs() > 35.0 && (len < 60.0 || d.abs() > 70.0) && acc + len as f64 > 0.0 {
                let dir = if d.abs() > 150.0 {
                    2
                } else if d > 0.0 {
                    1
                } else {
                    -1
                };
                // the street it turns into: the first named lane after the turn
                let street = r
                    .lanes
                    .iter()
                    .skip(j + 1)
                    .take(4)
                    .chain(std::iter::once(&l))
                    .find_map(|&x| self.street_of(x))
                    .map(str::to_string);
                return Some((dir, d.abs(), acc.max(0.0), street));
            }
            prev_end = Some(h1);
            acc += len as f64;
        }
        None
    }

    /// The street a lane of the route's network belongs to (the map's network only).
    pub(super) fn street_of(&self, lane: usize) -> Option<&str> {
        self.global.as_ref()?;
        let st = self.streets.as_deref()?;
        let i = *st.of_lane.get(lane)?;
        st.names.get(i as usize).map(String::as_str)
    }

    /// Distance along the route to `stop` (m).
    fn route_distance(&self, net: &Network, stop: DVec3) -> Option<f64> {
        let r = &self.route;
        let mut acc = -(r.s as f64);
        for &l in r.lanes.iter().skip(r.progress) {
            let lane = net.lanes.get(l)?;
            if let Some((s, d)) = lane.nearest_point(stop) {
                if d < 25.0 && (acc + s as f64) >= -5.0 {
                    return Some((acc + s as f64).max(0.0));
                }
            }
            acc += lane.length() as f64;
            if acc > 30_000.0 {
                break;
            }
        }
        None
    }

    /// Advance, draw into the texture and put it on the screen.
    pub fn frame_at(
        &mut self,
        renderer: &Renderer,
        scene: &mut Scene,
        f: &NavFrame,
        origin_x: f32,
    ) {
        for rect in [&mut self.panel_rect, &mut self.city.rect] {
            rect[0] -= self.origin_x;
            rect[2] -= self.origin_x;
        }
        if self.origin_x != origin_x {
            self.panel_drag = None;
            self.city.drag = None;
        }
        let start = scene.overlays.len();
        self.frame(renderer, scene, f);
        crate::hud::shift_overlays(scene, start, origin_x);
        for rect in [&mut self.panel_rect, &mut self.city.rect] {
            rect[0] += origin_x;
            rect[2] += origin_x;
        }
        self.origin_x = origin_x;
    }

    pub fn frame(&mut self, renderer: &Renderer, scene: &mut Scene, f: &NavFrame) {
        self.panel_overlay = None;
        // (with the navigator off the route is still followed for OMSI 2's arrows)
        if !self.enabled && !self.city.open && !self.arrows && self.shown < 0.01 {
            return;
        }
        let target = if self.enabled { 1.0 } else { 0.0 };
        self.shown += (target - self.shown) * (1.0 - (-f.dt * 8.0).exp());
        if (self.shown - target).abs() < 0.005 {
            self.shown = target;
        }
        self.time += f.dt;
        self.day_clock = (f.time, f.weekday);
        self.date = f.date;
        if let Some(rx) = self.building.as_ref() {
            if let Ok((net, pos, streets, surface, chrono)) = rx.try_recv() {
                self.chrono = chrono;
                log::info!(
                    "navigator: the map's road network is there ({} lanes, {} streets named)",
                    net.lanes.len(),
                    streets.names.len()
                );
                self.streets = Some(std::sync::Arc::new(streets));
                self.set_surface(surface);
                self.global = Some(std::sync::Arc::new(net));
                self.stop_pos = std::sync::Arc::new(pos);
                self.global_version += 1;
                self.building = None;
                // routes and roads again on the whole map
                self.roads = None;
                self.route = Route {
                    version: self.route.version + 1,
                    ..Route::default()
                };
                self.route_mesh.0 = u64::MAX;
                self.route_jam.clear();
            }
        }
        // stops whose tiles are not loaded: their place from the map
        if omsi_cfg::flags::DEBUG_NAV.is_set() && self.time < 0.15 {
            log::info!(
                "navigator: stops {:?}",
                f.stops
                    .iter()
                    .map(|s| (
                        s.name.clone(),
                        s.object_id,
                        s.position != DVec3::ZERO,
                        self.stop_pos.contains_key(&s.object_id)
                    ))
                    .collect::<Vec<_>>()
            );
        }
        let f2;
        let f = if f.stops.iter().any(|s| s.position == DVec3::ZERO) {
            f2 = NavFrame {
                stops: f
                    .stops
                    .iter()
                    .filter_map(|s| {
                        if s.position == DVec3::ZERO {
                            self.stop_pos.get(&s.object_id).map(|p| NavStop {
                                position: *p,
                                ..s.clone()
                            })
                        } else {
                            Some(s.clone())
                        }
                    })
                    .collect(),
                ..f.clone_ref()
            };
            &f2
        } else {
            f
        };
        // (the route goes on following the bus; the maps follow the walker or the camera)
        self.follow(f);
        // (the stops' helpers in the world stay with the bus too)
        self.bus_at = f.bus;
        self.stop_spots = f
            .stops
            .iter()
            .take(3)
            .map(|st| (st.position, st.name.clone(), f.heading, st.object_id))
            .collect();
        let f3;
        let f = match f.focus {
            Some(focus) => {
                self.bus_marker = Some((f.bus, f.heading));
                f3 = NavFrame {
                    bus: focus.at,
                    heading: focus.heading,
                    ..f.clone_ref()
                };
                &f3
            }
            None => {
                self.bus_marker = None;
                f
            }
        };
        if omsi_cfg::flags::DEBUG_NAV.is_set() && (self.time % 1.0) < f.dt {
            log::info!("navigator: route {} lanes (complete {}, provisional {}, at {}, on it {}, off for {:.1} s), {} stops ahead, next {:?}, key {:?}", self.route.lanes.len(), self.route.complete, self.route.provisional, self.route.progress, self.route.on_route, self.route.off_for, f.stops.len(), f.stops.first().map(|s| (s.name.clone(), s.position.x.round(), s.position.y.round())), self.route.key);
        }
        self.update_congestion(f);
        // camera: zoomed out with speed, turned with the bus (both eased)
        let want = (110.0 + f.speed_kmh as f64 * 2.2).clamp(110.0, 280.0);
        if self.first {
            self.zoom = want;
            self.cam_heading = f.heading;
        }
        self.zoom += (want - self.zoom) * ease(f.dt, 1.8);
        self.cam_heading += angle_diff(self.cam_heading, f.heading) * ease(f.dt, 0.3);
        let v = f.speed_kmh.abs() / 3.6;
        self.speed_avg += (v - self.speed_avg) * ease(f.dt, 20.0) as f32;
        self.dist_t -= f.dt;
        if self.dist_t <= 0.0 || self.first {
            self.dist_t = 0.4;
            let global = self.global.clone();
            let net = global.as_deref().or(f.traffic.map(|t| &t.net));
            self.next_dist = match (f.stops.first(), net) {
                (Some(s), Some(n)) if !self.route.lanes.is_empty() => self
                    .route_distance(n, s.position)
                    .or_else(|| Some((s.position - self.bus_at).truncate().length())),
                (Some(s), _) => Some((s.position - self.bus_at).truncate().length()),
                _ => None,
            };
            self.next_turn = net.and_then(|n| self.turn_ahead(n));
            self.street_here = if self.route.on_route {
                self.route
                    .lanes
                    .get(self.route.progress)
                    .and_then(|&l| self.street_of(l))
            } else {
                global
                    .as_deref()
                    .and_then(|g| g.nearest_lane_near(self.bus_at, LaneKind::Street))
                    .filter(|l| l.2 < 10.0)
                    .and_then(|l| self.street_of(l.0))
            }
            .map(str::to_string);
            (self.loc_city, self.loc_street) = match self
                .surface
                .as_ref()
                .filter(|s| self.show_surface && s.has_location())
            {
                Some(sf) => sf.locate(f.bus.truncate()),
                None => (None, None),
            };
            self.loc_alt = self
                .surface
                .as_ref()
                .filter(|_| self.show_surface && self.show_contours)
                .and_then(|sf| sf.altitude_at(f.bus.truncate()));
            self.loc_limit = self
                .surface
                .as_ref()
                .filter(|s| self.show_surface && s.has_location())
                .and_then(|sf| sf.street_info(self.bus_at.truncate(), 30.0))
                .map_or((0, 0), |s| (s.speed, s.heavy));
        }
        self.first = false;
        if !self.enabled && self.shown < 0.01 {
            self.panel_rect = [0.0; 4];
            // (followed for the route arrows alone, with the city map shut: nothing to draw)
            if self.city.open {
                self.city(renderer, scene, f);
            }
            return;
        }

        // --- size and place on the screen: small, a corner of its own
        let (sw, sh) = f.screen;
        // (a third of the window's height however tall it is - held to 480 px, it was a
        // sixth of a 4K screen's - but 300 px at least, where its smallest texts were 8 px
        // high on a 720p window; made larger, still short enough to fit the window with its
        // schedule)
        let base = (sh * 0.33).max(300.0);
        let base = if f.follow_window {
            base
        } else {
            base.min(480.0)
        };
        let pw = (base * f.ui_scale).min((sh * 0.7).max(300.0)).round();
        let map_h = (pw * 0.62).round();
        let s = pw / 360.0;
        let bars = (34.0 + 46.0) * s;
        let sched = if self.schedule {
            (f.stops.len().clamp(1, 5) as f32 * 22.0 + 12.0) * s
        } else {
            0.0
        };
        // a row under the next stop: the municipality and the street, when the map's package says them
        let loc_on = self.show_surface && self.surface.as_ref().is_some_and(|sf| sf.has_location());
        let loc_h = if loc_on { 22.0 * s } else { 0.0 };
        let ph = (map_h + bars + loc_h + sched).round();
        let (w, h) = (pw as u32, ph as u32);
        let (x0, y0) = self.panel_origin(
            (sw, sh),
            (pw, ph),
            crate::core::platform::touch_controls(),
            f.info_rect,
        );

        if self.gpu.is_none() {
            self.gpu = Some(self.new_gpu(renderer));
        }
        let resized = self.target.map(|t| (t.1, t.2) != (w, h)).unwrap_or(true);
        if resized {
            if let Some((t, _, _)) = self.target.take() {
                renderer.free_texture(scene, t);
                scene.premultiplied.remove(&t);
            }
            let t = renderer.add_render_texture(scene, w, h);
            scene.premultiplied.insert(t);
            self.target = Some((t, w, h));
        }
        let (tex, _, _) = self.target.unwrap();
        let Some(view) = renderer.texture_view(scene, tex) else {
            return;
        };
        if !self.city.open && (resized || self.time - self.drawn_at >= NAV_REDRAW_S) {
            self.drawn_at = self.time;
            self.draw(renderer, &view, (w, h), map_h, f);
        }
        // (the small navigator steps aside while the city map is open)
        if !self.city.open {
            self.panel_overlay = Some(scene.overlays.len());
            scene.overlays.push((tex, [x0, y0, x0 + pw, y0 + ph]));
        }
        self.panel_rect = [x0, y0, x0 + pw, y0 + ph];
        if self.city.open {
            self.city(renderer, scene, f);
        }
    }
}

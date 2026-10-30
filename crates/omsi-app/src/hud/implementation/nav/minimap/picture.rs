//! The corner map's picture: the roads, the route and its arrows, the vehicles, the stops
//! and the header, drawn into the navigator's texture.

use super::super::draw::{
    arrow, focus_marker, level, player_triangle, rects_overlap, spaced_markers, stop_request_icon,
    traffic_kind, vehicle_icon, EARLY, LATE, LEVEL, ON_TIME, STREET, TEXT, TEXT_DIM, WARN,
};
use super::super::geometry::build_roads;
use super::super::route::{build_route, lane_from_right, RouteStyle};
use super::super::{
    angle_diff, project, words, NavFrame, Navigator, Roads, BAR, DOT, FOV, LINE_TEXT,
    OFF_ROUTE_AFTER, PANEL, PITCH, PLAYER, ROAD_RADIUS, ROUTE,
};
use glam::{DVec2, DVec3, Mat4, Vec2, Vec3};
use omsi_render::Renderer;
use omsi_sim::traffic::LaneKind;
use omsi_ui::paint::Align;
use omsi_ui::{Color, Draw, Layer, Painter, Rect, Vertex, Weight};

impl Navigator {
    pub(super) fn draw(
        &mut self,
        renderer: &Renderer,
        target: &wgpu::TextureView,
        size: (u32, u32),
        map_h: f32,
        f: &NavFrame,
    ) {
        let (pw, ph) = (size.0 as f32, size.1 as f32);
        let s = pw / 360.0;
        let wd = words(f.language);
        self.atlas.begin_frame();
        let panel = Rect::new(0.0, 0.0, pw, ph);
        let radius = 4.0 * s;
        let top_h = (34.0 * s).round();
        let map = Rect::new(0.0, top_h, pw, map_h);
        let vp = [map.x, map.y, map.w, map.h];

        // --- roads (buffer 0): rebuilt when the bus went far or tiles brought lanes
        let own = self.own_net.clone();
        let global = self.global.clone();
        let net = global
            .as_deref()
            .or(f.traffic.map(|t| &t.net))
            .or(own.as_deref());
        let lanes_now = net.map(|n| n.lanes.len()).unwrap_or(0);
        let rebuild = match &self.roads {
            None => lanes_now > 0,
            Some(r) => {
                r.light != self.light
                    || (r.anchor - f.bus.truncate()).length() > ROAD_RADIUS * 0.45
                    || (r.lanes_seen != lanes_now && self.time - r.built_at > 1.5)
            }
        };
        let mut road_verts = None;
        if rebuild {
            if let Some(net) = net {
                let anchor = f.bus.truncate();
                let mut p = Painter::new();
                let t0 = std::time::Instant::now();
                build_roads(&mut p, net, anchor, self.light);
                if omsi_cfg::flags::DEBUG_NAV.is_set() {
                    log::info!(
                        "navigator: roads around ({:.0}, {:.0}) in {:.1} ms: {} vertices",
                        anchor.x,
                        anchor.y,
                        t0.elapsed().as_secs_f64() * 1000.0,
                        p.verts.len()
                    );
                }
                road_verts = Some(p.verts);
                self.roads = Some(Roads {
                    light: self.light,
                    anchor,
                    lanes_seen: lanes_now,
                    verts: 0,
                    built_at: self.time,
                });
            }
        }

        // --- the map camera (world coordinates relative to the anchor)
        let anchor = self
            .roads
            .as_ref()
            .map(|r| r.anchor)
            .unwrap_or(f.bus.truncate());
        // --- the map's surface (buffer 6): built again when it arrives or the anchor moved
        let mut surface_verts = None;
        if let (true, Some(sf)) = (self.show_surface, self.surface.clone()) {
            let key = self.surface_version * 2 + self.light as u64;
            if self
                .surface_mesh
                .map(|m| m.0 != key || m.1 != anchor)
                .unwrap_or(true)
            {
                let mut p = Painter::new();
                sf.build(
                    &mut p,
                    anchor,
                    Some((anchor, ROAD_RADIUS * 1.4)),
                    self.light,
                    0.0,
                );
                self.surface_mesh = Some((key, anchor, p.len()));
                surface_verts = Some(p.verts);
            }
        }
        let surface_n = if self.show_surface {
            self.surface_mesh.map(|m| m.2).unwrap_or(0)
        } else {
            0
        };
        let rel = |p: DVec3| Vec3::new((p.x - anchor.x) as f32, (p.y - anchor.y) as f32, 0.0);
        let hd = self.cam_heading.to_radians();
        let fwd = DVec2::new(hd.sin(), hd.cos());
        let look_at = f.bus.truncate() - anchor + fwd * self.zoom * 0.28;
        let view = map_view(look_at, fwd, self.zoom, self.view_top);
        let clip_panel = [0.0, 0.0, pw, ph];
        let map_layer = Layer::world(
            view,
            FOV.to_radians(),
            vp,
            [map.x, map.y, map.right(), map.bottom()],
            0.0,
            1.0,
        );
        let vpm = map_layer.view_proj;

        // --- the route (buffer 1): rebuilt when it or the lane the bus is on changed
        let mut route_verts = None;
        let route_net = global.as_deref().or(f.traffic.map(|t| &t.net));
        if let (Some(rn), true) = (route_net, !self.route.lanes.is_empty()) {
            let first = self
                .route
                .lanes
                .get(
                    self.route
                        .progress
                        .min(self.route.lanes.len().saturating_sub(1)),
                )
                .copied();
            let bus_lane = first
                .map(|l| lane_from_right(rn, l, self.bus_at))
                .unwrap_or(0);
            if self.route_mesh.0 != self.route.version
                || self.route_mesh.1 != self.jam_version
                || self.route_mesh.2 != anchor
                || self.route_mesh.4 != bus_lane
            {
                let mut p = Painter::new();
                // (one lane wide: a little narrower than the lane, so that it never spills onto the next)
                let style = RouteStyle {
                    extra_m: -0.9,
                    min_px: 4.0,
                    arrows: Some((28.0, 900.0)),
                    max_len: 12_000.0,
                    near: Some((anchor, ROAD_RADIUS * 1.6)),
                };
                build_route(
                    &mut p,
                    rn,
                    &self.route.lanes[self.route.progress.min(self.route.lanes.len())..],
                    anchor,
                    self.route.s,
                    &self.route_jam,
                    &style,
                    bus_lane,
                );
                self.route_mesh = (
                    self.route.version,
                    self.jam_version,
                    anchor,
                    p.len(),
                    bus_lane,
                );
                route_verts = Some(p.verts);
            }
        } else if self.route_mesh.3 != 0 {
            route_verts = Some(Vec::new());
            self.route_mesh = (self.route.version, self.jam_version, anchor, 0, 0);
        }

        // --- background (half transparent), traffic, markers, text
        let mut bg = Painter::new();
        bg.rounded(
            panel,
            radius,
            if self.cockpit_display {
                Color::rgba(10, 10, 10, 1.0)
            } else {
                PANEL
            },
        );
        // the map's own ground under the roads (the panel's opacity setting dims it as it does the panel)
        if let (true, Some(sf)) = (self.show_surface, self.surface.as_ref()) {
            bg.rect(map, sf.palette(self.light).land);
        }
        let n_bg = bg.len();

        let mut dy = Painter::new();
        // other roads where the traffic is heavy or stands
        if let Some(net) = f.traffic.map(|t| &t.net) {
            for (&lane, &c) in &self.congestion {
                let lv = level(c);
                if lv < 3 {
                    continue;
                }
                let Some(l) = net.lanes.get(lane) else {
                    continue;
                };
                if (l.start() - f.bus).truncate().length() > 900.0 {
                    continue;
                }
                let pts: Vec<Vec3> = l.points.iter().map(|p| rel(*p)).collect();
                dy.ribbon(
                    &pts,
                    l.width.max(2.5) * 0.6,
                    2.0,
                    LEVEL[lv].alpha(0.8),
                    true,
                );
            }
        }
        let n_traffic = dy.len();
        // the other vehicles: blue dots, as the other drivers in ETS2; the public transport in
        // its own colours (trolleybus, bus, tram), a little bigger, its line beside it
        let mut lines: Vec<(DVec3, Color, String)> = Vec::new();
        if let Some(t) = f.traffic.filter(|_| self.show_ai) {
            for c in &t.cars {
                if c.gone
                    || (c.vehicle.position - f.bus).truncate().length() > self.zoom * 3.5 + 150.0
                {
                    continue;
                }
                let (color, line, icon) = traffic_kind(c, self.surface.as_deref(), f.car_line);
                if color == DOT {
                    dy.world_disc(rel(c.vehicle.position), 1.7, 3.6, Color::rgba(8, 8, 8, 0.9));
                    dy.world_disc(rel(c.vehicle.position), 1.2, 2.6, color);
                } else {
                    vehicle_icon(
                        &mut dy,
                        rel(c.vehicle.position),
                        c.vehicle.heading,
                        color,
                        icon,
                        7.0,
                    );
                }
                if let Some(l) = line {
                    lines.push((c.vehicle.position, color, l));
                }
            }
        }
        let n_world = dy.len();

        let mut ui = Painter::new();
        // the far end of the map fades into the panel
        ui.gradient(
            Rect::new(map.x, map.y, map.w, map.h * 0.3),
            Color::rgba(10, 10, 10, 0.75),
            Color::rgba(10, 10, 10, 0.0),
        );
        // the next turn: an arrow and how far, top left of the map
        if let Some((dir, angle, dist, street)) = self.next_turn.as_ref() {
            let icon = match *dir {
                2 => "u_turn_left",
                -1 if *angle < 60.0 => "turn_slight_left",
                1 if *angle < 60.0 => "turn_slight_right",
                -1 => "turn_left",
                _ => "turn_right",
            };
            let t = if *dist >= 1000.0 {
                format!("{:.1} km", dist / 1000.0)
            } else {
                format!("{:.0} m", ((dist / 10.0).round() * 10.0).max(10.0))
            };
            let tw = self.fonts.width(&t, 14.0 * s, Weight::Bold);
            // with the street it turns into, when the map names it
            let street = street.as_deref().map(|n| {
                self.fonts
                    .fit(n, 12.0 * s, Weight::Medium, map.w * 0.62 - 50.0 * s - tw)
            });
            let sw_ = street
                .as_deref()
                .map(|n| self.fonts.width(n, 12.0 * s, Weight::Medium) + 10.0 * s)
                .unwrap_or(0.0);
            let b = Rect::new(
                map.x + 8.0 * s,
                map.y + 8.0 * s,
                44.0 * s + tw + sw_,
                34.0 * s,
            );
            ui.rounded(b, 5.0 * s, Color::rgba(32, 33, 36, 0.92));
            ui.icon(
                &mut self.atlas,
                icon,
                Vec2::new(b.x + 18.0 * s, b.center().y),
                24.0 * s,
                TEXT,
            );
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &t,
                14.0 * s,
                Weight::Bold,
                Rect::new(b.x + 34.0 * s, b.y, tw + 4.0, b.h),
                Align::Left,
                TEXT,
            );
            if let Some(n) = street.as_deref() {
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    n,
                    12.0 * s,
                    Weight::Medium,
                    Rect::new(b.x + 42.0 * s + tw, b.y, sw_, b.h),
                    Align::Left,
                    TEXT_DIM,
                );
            }
        }
        // the street the bus is on, bottom middle of the map
        if let Some(n) = self.street_here.as_deref() {
            let px = 11.5 * s;
            let n = self.fonts.fit(n, px, Weight::Medium, map.w * 0.7);
            let w = self.fonts.width(&n, px, Weight::Medium) + 14.0 * s;
            let r = Rect::new(
                map.center().x - w * 0.5,
                map.bottom() - 24.0 * s,
                w,
                18.0 * s,
            );
            ui.rounded(r, 9.0 * s, Color::rgba(32, 33, 36, 0.9));
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &n,
                px,
                Weight::Medium,
                r,
                Align::Center,
                STREET,
            );
        }
        // Stops ahead use a bus badge so they read as stops, not generic route dots.
        let n_stops = f.stops.len();
        let markers = spaced_markers(
            f.stops.iter().enumerate().filter_map(|(k, st)| {
                project(vpm, vp, rel(st.position))
                    .filter(|p| map.contains(*p))
                    .map(|p| (k, p))
            }),
            20.0 * s,
        );
        for (k, sp) in markers.into_iter().rev() {
            if k + 1 == n_stops {
                ui.icon(
                    &mut self.atlas,
                    "sports_score",
                    sp - Vec2::new(0.0, 12.0 * s),
                    14.0 * s,
                    TEXT,
                );
            }
            let next = k == 0;
            let badge = if next { 8.0 } else { 6.5 } * s;
            ui.circle(sp, badge + 1.5 * s, Color::rgba(10, 10, 10, 0.95));
            let fill = if next {
                Color::rgba(45, 116, 205, 1.0)
            } else if k + 1 == n_stops {
                ROUTE
            } else {
                Color::rgba(76, 91, 112, 0.98)
            };
            ui.circle(sp, badge, fill);
            ui.icon(
                &mut self.atlas,
                "directions_bus",
                sp,
                if next { 12.5 } else { 10.5 } * s,
                TEXT,
            );
        }
        // the public transport's lines: a tag in its colour above its dot (the nearest first,
        // none on top of another)
        lines.sort_by(|a, b| (a.0 - f.bus).length().total_cmp(&(b.0 - f.bus).length()));
        let mut taken: Vec<Rect> = Vec::new();
        // the other players: an arrow each, their name above it (before the lines' tags)
        for pl in &f.players {
            let Some(p) = project(vpm, vp, rel(pl.position)).filter(|p| map.contains(*p)) else {
                continue;
            };
            let a = (angle_diff(self.cam_heading, pl.heading) as f32).to_radians();
            arrow(
                &mut ui,
                p,
                a,
                7.5 * s,
                1.25,
                Color::rgba(10, 10, 10, 0.8),
                PLAYER,
            );
            let px = 9.5 * s;
            let name = self.fonts.fit(&pl.name, px, Weight::Bold, 90.0 * s);
            let w = self.fonts.width(&name, px, Weight::Bold) + 8.0 * s;
            let r = Rect::new(p.x - w * 0.5, p.y - 23.0 * s, w, 13.0 * s);
            if taken.iter().any(|o| rects_overlap(o, &r)) {
                continue;
            }
            taken.push(r);
            ui.rounded(
                Rect::new(r.x - 1.0 * s, r.y - 1.0 * s, r.w + 2.0 * s, r.h + 2.0 * s),
                4.0 * s,
                Color::rgba(10, 10, 10, 0.9),
            );
            ui.rounded(r, 3.5 * s, PLAYER);
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &name,
                px,
                Weight::Bold,
                r,
                Align::Center,
                LINE_TEXT,
            );
        }
        for (pos, color, l) in &lines {
            let Some(p) = project(vpm, vp, rel(*pos)).filter(|p| map.contains(*p)) else {
                continue;
            };
            let px = 9.5 * s;
            let l = self.fonts.fit(l, px, Weight::Bold, 40.0 * s);
            let w = self.fonts.width(&l, px, Weight::Bold) + 8.0 * s;
            let r = Rect::new(p.x - w * 0.5, p.y - 19.0 * s, w, 13.0 * s);
            if taken
                .iter()
                .any(|o| o.x < r.right() && r.x < o.right() && o.y < r.bottom() && r.y < o.bottom())
            {
                continue;
            }
            taken.push(r);
            ui.rounded(
                Rect::new(r.x - 1.0 * s, r.y - 1.0 * s, r.w + 2.0 * s, r.h + 2.0 * s),
                4.0 * s,
                Color::rgba(10, 10, 10, 0.9),
            );
            ui.rounded(r, 3.5 * s, *color);
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &l,
                px,
                Weight::Bold,
                r,
                Align::Center,
                LINE_TEXT,
            );
        }
        // the bus: a plain white arrow (the map's own triangle, when its package draws the player so)
        let player = self
            .surface
            .as_ref()
            .filter(|_| self.show_surface)
            .and_then(|s| s.player_style(f.line.as_deref()));
        // (the bus left behind, while the map follows the walker or the free camera)
        if let (Some((at, heading)), Some(_)) = (self.bus_marker, f.focus) {
            if let Some(bp) = project(vpm, vp, rel(at)) {
                let a = (angle_diff(self.cam_heading, heading) as f32).to_radians();
                arrow(
                    &mut ui,
                    bp,
                    a,
                    7.0 * s,
                    1.25,
                    Color::rgba(10, 10, 10, 0.8),
                    TEXT_DIM,
                );
            }
        }
        if let (Some(focus), Some(bp)) = (f.focus, project(vpm, vp, rel(f.bus))) {
            let a = (angle_diff(self.cam_heading, f.heading) as f32).to_radians();
            focus_marker(&mut ui, focus.kind, bp, a, 12.0 * s);
        } else if let (Some(bp), Some((fill, ring))) = (project(vpm, vp, rel(f.bus)), player) {
            player_triangle(
                &mut ui,
                bp,
                (angle_diff(self.cam_heading, f.heading) as f32).to_radians(),
                22.0 * s,
                fill,
                ring,
            );
        } else if let Some(bp) = project(vpm, vp, rel(f.bus)) {
            let a = (angle_diff(self.cam_heading, f.heading) as f32).to_radians();
            arrow(
                &mut ui,
                bp,
                a,
                9.0 * s,
                1.25,
                Color::rgba(10, 10, 10, 0.8),
                TEXT,
            );
        }

        // the map's icons (terminals, stations, the garage): pictures of the package's sheet,
        // upright on the screen at the place they mark
        let mut icon_verts: Vec<Vertex> = Vec::new();
        if let (true, Some(sf)) = (self.show_surface, self.surface.as_ref()) {
            let reach = self.zoom * 3.5 + 300.0;
            for m in sf.markers() {
                if (m.at - f.bus.truncate()).length() > reach {
                    continue;
                }
                if let Some(p) =
                    project(vpm, vp, rel(m.at.extend(0.0))).filter(|p| map.contains(*p))
                {
                    sf.push_icon(m, p, s, &mut icon_verts);
                }
            }
        }

        // top bar: speed (and the limit) · line ……… game time
        let top = Rect::new(0.0, 0.0, pw, top_h);
        ui.rect(top, BAR);
        ui.rect(
            Rect::new(0.0, top.bottom() - 1.0, pw, 1.0),
            Color::WHITE.alpha(0.07),
        );
        let pad = 11.0 * s;
        let base = top.y + top.h * 0.5 + self.fonts.cap_height(17.0 * s, Weight::Bold) * 0.5;
        let mut x = pad;
        x += ui.text(
            &mut self.atlas,
            &self.fonts,
            &format!("{:.0}", f.speed_kmh.abs()),
            17.0 * s,
            Weight::Bold,
            Vec2::new(x, base),
            Align::Left,
            TEXT,
        );
        x += 4.0 * s;
        x += ui.text(
            &mut self.atlas,
            &self.fonts,
            wd.kmh,
            12.0 * s,
            Weight::Medium,
            Vec2::new(x, base),
            Align::Left,
            TEXT_DIM,
        );
        let limit = net.and_then(|n| {
            let lane = if self.route.on_route {
                self.route.lanes.get(self.route.progress).copied()
            } else {
                None
            };
            let lane = lane.or_else(|| {
                n.nearest_lane_near(self.bus_at, LaneKind::Street)
                    .filter(|l| l.2 < 8.0)
                    .map(|l| l.0)
            })?;
            let v = n.lanes.get(lane)?.speed_limit_kmh;
            (v > 1.0 && v < 200.0).then_some(v)
        });
        if let Some(v) = limit {
            x += 10.0 * s;
            let c = Vec2::new(x + 10.0 * s, top.center().y);
            ui.circle(c, 10.5 * s, Color::rgba(200, 40, 40, 1.0));
            ui.circle(c, 8.3 * s, Color::rgba(235, 235, 235, 1.0));
            let t = format!("{:.0}", (v / 5.0).round() * 5.0);
            let px = if t.len() > 2 { 7.5 } else { 9.0 } * s;
            ui.text(
                &mut self.atlas,
                &self.fonts,
                &t,
                px,
                Weight::Black,
                Vec2::new(c.x, c.y + self.fonts.cap_height(px, Weight::Black) * 0.5),
                Align::Center,
                Color::rgba(15, 15, 15, 1.0),
            );
        }
        let hh = (f.time / 3600.0) as i32 % 24;
        let mm = ((f.time % 3600.0) / 60.0) as i32;
        let time_text = format!("{hh:02}:{mm:02}");
        let day_text = wd.days[f.weekday.clamp(0, 6) as usize];
        let time_w = self.fonts.width(&time_text, 14.0 * s, Weight::Bold);
        let day_w = self.fonts.width(day_text, 12.0 * s, Weight::Medium);
        // (the day and the time in a chip, as the windows' headers hold theirs)
        let tr = pw - pad - 7.0 * s;
        let chip = Rect::new(
            tr - time_w - 5.0 * s - day_w - 9.0 * s,
            top.center().y - 11.0 * s,
            time_w + 5.0 * s + day_w + 18.0 * s,
            22.0 * s,
        );
        ui.rounded(chip, chip.h * 0.5, Color::WHITE.alpha(0.08));
        ui.text(
            &mut self.atlas,
            &self.fonts,
            &time_text,
            14.0 * s,
            Weight::Bold,
            Vec2::new(tr, base),
            Align::Right,
            TEXT,
        );
        ui.text(
            &mut self.atlas,
            &self.fonts,
            day_text,
            12.0 * s,
            Weight::Medium,
            Vec2::new(tr - time_w - 5.0 * s, base),
            Align::Right,
            TEXT_DIM,
        );

        // Temperatures stay in the navigator header on every bus. The simulator always keeps
        // Cabinair_Temp, while scripts that model heating/air conditioning can overwrite it.
        let mut temp = format!("EXT {:.0}°C · INT {:.0}°C", f.outside_temp, f.inside_temp);
        let mut left_edge = (if limit.is_some() { x + 24.0 * s } else { x }) + 6.0 * s;
        let right_edge = chip.x - 6.0 * s;
        // the line as a badge in the accent (when there is room for it)
        if let Some(line) = f.line.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
            let line = self.fonts.fit(line, 12.0 * s, Weight::Bold, 46.0 * s);
            let bw = self.fonts.width(&line, 12.0 * s, Weight::Bold) + 14.0 * s;
            if left_edge + bw + 70.0 * s < right_edge {
                let badge = Rect::new(left_edge, top.center().y - 10.0 * s, bw, 20.0 * s);
                ui.rounded(badge, 5.0 * s, omsi_ui::theme::accent());
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    &line,
                    12.0 * s,
                    Weight::Bold,
                    badge,
                    Align::Center,
                    omsi_ui::theme::on_accent(),
                );
                left_edge += bw + 6.0 * s;
            } else {
                // (no room for the badge: the line stays in the text, as it was)
                temp = format!("{temp} · {line}");
            }
        }
        if right_edge > left_edge {
            let rect = Rect::new(left_edge, top.y, right_edge - left_edge, top.h);
            let center = self.fonts.fit(&temp, 11.5 * s, Weight::Bold, rect.w);
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &center,
                11.5 * s,
                Weight::Bold,
                rect,
                Align::Center,
                TEXT_DIM,
            );
        }

        // bottom bar: the next stop; its distance, the time to it, the planned time and
        // whether the bus is early or late
        let bottom = Rect::new(0.0, map.bottom(), pw, 46.0 * s);
        // (the row under the next stop; `frame` made room for it)
        let loc_on = self.show_surface && self.surface.as_ref().is_some_and(|sf| sf.has_location());
        let loc_h = if loc_on { 22.0 * s } else { 0.0 };
        if loc_on {
            let row = Rect::new(0.0, bottom.bottom(), pw, loc_h);
            ui.rect(row, BAR);
            let text = match (&self.loc_city, &self.loc_street) {
                (Some(c), Some(st)) => format!("{c}  ·  {st}"),
                (Some(c), None) => c.clone(),
                (None, Some(st)) => st.clone(),
                _ => String::new(),
            };
            if !text.is_empty() {
                ui.icon(
                    &mut self.atlas,
                    "location_on",
                    Vec2::new(pad + 7.0 * s, row.center().y),
                    15.0 * s,
                    TEXT_DIM,
                );
                let text =
                    self.fonts
                        .fit(&text, 12.5 * s, Weight::Medium, pw - 2.0 * pad - 24.0 * s);
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    &text,
                    12.5 * s,
                    Weight::Medium,
                    Rect::new(pad + 22.0 * s, row.y, pw - 2.0 * pad - 22.0 * s, row.h),
                    Align::Left,
                    TEXT,
                );
            }
        }
        ui.rect(bottom, BAR);
        let stop_row = if f.stops.is_empty() {
            Rect::new(pad, bottom.y, pw - 2.0 * pad, bottom.h)
        } else {
            Rect::new(pad, bottom.y + 4.0 * s, pw - 2.0 * pad, 20.0 * s)
        };
        let stop_row = stop_request_icon(&mut ui, &mut self.atlas, f.stop_requested, stop_row, s);
        let note = if self.route.note > 0.0 {
            Some((wd.recalculated, ON_TIME))
        } else if self.route.joined
            && !self.route.lanes.is_empty()
            && !self.route.on_route
            && self.route.off_for > OFF_ROUTE_AFTER
        {
            // (no way back found after a while: it says so instead of recalculating for ever)
            Some((
                if self.route.off_for < OFF_ROUTE_AFTER + 20.0 {
                    wd.rerouting
                } else {
                    wd.off_route
                },
                WARN,
            ))
        } else {
            None
        };
        // what the traffic on the route ahead costs, as the module promises: a jam from a
        // minute on, slow traffic from half of one
        let jam_note = (self.jam_cost >= 30.0).then(|| {
            let what = if self.jam_cost >= 60.0 {
                wd.jam
            } else {
                wd.slow
            };
            (
                format!("{what} +{:.0} min", (self.jam_cost / 60.0).max(1.0).round()),
                if self.jam_cost >= 60.0 { LATE } else { WARN },
            )
        });
        match f.stops.first() {
            Some(st) => {
                let name = if n_stops == 1 {
                    format!("{} · {}", st.name.trim(), wd.last_stop)
                } else {
                    st.name.trim().to_string()
                };
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    &name,
                    13.5 * s,
                    Weight::Bold,
                    stop_row,
                    Align::Left,
                    TEXT,
                );
                let mut parts = Vec::new();
                if let Some(d) = self.next_dist {
                    parts.push(if d >= 1000.0 {
                        format!("{:.1} km", d / 1000.0)
                    } else {
                        format!("{:.0} m", (d / 10.0).round() * 10.0)
                    });
                    let secs = d / (self.speed_avg.max(5.0) as f64);
                    parts.push(if secs < 60.0 {
                        "<1 min".to_string()
                    } else {
                        format!("{:.0} min", (secs / 60.0).round())
                    });
                }
                parts.push(format!(
                    "{:02}:{:02}",
                    (st.arrival / 3600.0) as i32 % 24,
                    ((st.arrival % 3600.0) / 60.0) as i32
                ));
                let line2 = parts.join("  ·  ");
                let y2 = Rect::new(pad, bottom.y + 24.0 * s, pw - 2.0 * pad, 18.0 * s);
                match note {
                    Some((t, c)) => {
                        ui.text_in(
                            &mut self.atlas,
                            &self.fonts,
                            t,
                            12.5 * s,
                            Weight::Medium,
                            y2,
                            Align::Left,
                            c,
                        );
                    }
                    None => match &jam_note {
                        Some((t, c)) => {
                            ui.text_in(
                                &mut self.atlas,
                                &self.fonts,
                                &format!("{line2}  ·  {t}"),
                                12.5 * s,
                                Weight::Medium,
                                y2,
                                Align::Left,
                                *c,
                            );
                        }
                        None => {
                            ui.text_in(
                                &mut self.atlas,
                                &self.fonts,
                                &line2,
                                12.5 * s,
                                Weight::Medium,
                                y2,
                                Align::Left,
                                TEXT_DIM,
                            );
                        }
                    },
                }
                if let Some(d) = f.delay {
                    let (txt, c) = if d > 59.0 {
                        (
                            format!("+{}:{:02}", (d / 60.0) as i32, (d % 60.0) as i32),
                            LATE,
                        )
                    } else if d < -59.0 {
                        (
                            format!("−{}:{:02}", (-d / 60.0) as i32, (-d % 60.0) as i32),
                            EARLY,
                        )
                    } else {
                        (wd.on_time.to_string(), ON_TIME)
                    };
                    ui.text_in(
                        &mut self.atlas,
                        &self.fonts,
                        &txt,
                        12.5 * s,
                        Weight::Bold,
                        y2,
                        Align::Right,
                        c,
                    );
                }
            }
            None => {
                let t = f
                    .terminus
                    .clone()
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| wd.no_duty.to_string());
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    &t,
                    13.0 * s,
                    Weight::Medium,
                    stop_row,
                    Align::Left,
                    TEXT_DIM,
                );
            }
        }
        // the schedule: the next stops with their planned times
        if self.schedule && !f.stops.is_empty() {
            let mut y = bottom.bottom() + loc_h + 6.0 * s;
            ui.rect(
                Rect::new(pad, bottom.bottom() + loc_h, pw - 2.0 * pad, 1.0),
                Color::WHITE.alpha(0.06),
            );
            let late = f.delay.unwrap_or(0.0);
            for st in f.stops.iter().take(5) {
                let r = Rect::new(pad, y, pw - 2.0 * pad, 22.0 * s);
                let planned = format!(
                    "{:02}:{:02}",
                    (st.arrival / 3600.0) as i32 % 24,
                    ((st.arrival % 3600.0) / 60.0) as i32
                );
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    &planned,
                    12.5 * s,
                    Weight::Bold,
                    r,
                    Align::Left,
                    TEXT_DIM,
                );
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    st.name.trim(),
                    13.0 * s,
                    Weight::Medium,
                    Rect::new(r.x + 46.0 * s, r.y, r.w - 100.0 * s, r.h),
                    Align::Left,
                    TEXT,
                );
                // when the bus will be there at this lateness
                let exp = st.arrival + late;
                let e = format!(
                    "{:02}:{:02}",
                    (exp / 3600.0).rem_euclid(24.0) as i32,
                    ((exp.rem_euclid(3600.0)) / 60.0) as i32
                );
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    &e,
                    12.5 * s,
                    Weight::Medium,
                    r,
                    Align::Right,
                    if late > 59.0 {
                        LATE
                    } else if late < -59.0 {
                        EARLY
                    } else {
                        TEXT_DIM
                    },
                );
                y += 22.0 * s;
            }
        }

        // --- to the GPU
        let (Some(gpu), device, queue) = (self.gpu.as_mut(), &renderer.device, &renderer.queue)
        else {
            return;
        };
        if let Some(v) = road_verts {
            if let Some(r) = self.roads.as_mut() {
                r.verts = v.len();
            }
            gpu.upload(device, queue, 0, &v);
        }
        if let Some(v) = surface_verts {
            gpu.upload(device, queue, 6, &v);
        }
        if let Some(v) = route_verts {
            gpu.upload(device, queue, 1, &v);
        }
        let mut all = bg.verts;
        all.extend(dy.verts);
        let n_ui_start = all.len() as u32;
        all.extend(ui.verts);
        gpu.upload(device, queue, 2, &all);
        if self.icon_tex.is_none() {
            if let Some(sheet) = self.surface.as_ref().and_then(|s| s.sheet()) {
                self.icon_tex = Some(gpu.add_image(device, queue, sheet.w, sheet.h, &sheet.rgba));
            }
        }
        gpu.upload(device, queue, 8, &icon_verts);
        let (icon_n, icon_tex) = (icon_verts.len() as u32, self.icon_tex.unwrap_or(0));
        gpu.upload_atlas(queue, &mut self.atlas);
        // (the opacity setting is the background's: the map and the text stay solid)
        let flat = Layer::flat(clip_panel, radius, 1.0);
        let backdrop = Layer::flat(
            clip_panel,
            radius,
            if self.cockpit_display {
                self.opacity
            } else {
                crate::hud::backdrop(self.opacity).min(1.0)
            },
        );
        let mut layers = [flat, map_layer, backdrop];
        for l in layers.iter_mut() {
            l.opacity *= self.shown;
        }
        let roads_n = self.roads.as_ref().map(|r| r.verts as u32).unwrap_or(0);
        let draws = [
            Draw {
                buffer: 2,
                range: 0..n_bg,
                layer: 2,
                texture: 0,
            },
            Draw {
                buffer: 6,
                range: 0..surface_n,
                layer: 1,
                texture: 0,
            },
            Draw {
                buffer: 0,
                range: 0..roads_n,
                layer: 1,
                texture: 0,
            },
            Draw {
                buffer: 2,
                range: n_bg..n_bg + n_traffic,
                layer: 1,
                texture: 0,
            },
            Draw {
                buffer: 1,
                range: 0..self.route_mesh.3,
                layer: 1,
                texture: 0,
            },
            Draw {
                buffer: 2,
                range: n_bg + n_traffic..n_bg + n_world,
                layer: 1,
                texture: 0,
            },
            Draw {
                buffer: 8,
                range: 0..icon_n,
                layer: 0,
                texture: icon_tex,
            },
            Draw {
                buffer: 2,
                range: n_ui_start..all.len() as u32,
                layer: 0,
                texture: 0,
            },
        ];
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("navigator"),
        });
        // (drawn into the panel's texture, made in `frame` before: under D3D9Ex the device
        // draws into its own copy of it, which a device opened again does not keep; the
        // next redraw, a thirtieth of a second on, puts it back, and the city map is drawn
        // again every frame it is open)
        debug_assert!(
            self.target.is_some(),
            "the minimap draws without its texture"
        );
        gpu.set_mirror_target(self.target.map(|t| t.0));
        gpu.render(
            device,
            queue,
            &mut enc,
            target,
            size,
            Some(wgpu::Color::TRANSPARENT),
            &layers,
            &draws,
        );
        queue.submit([enc.finish()]);
    }
}

/// The minimap's camera: `zoom` metres from `look_at`, behind it along `fwd` and tilted
/// (`PITCH`), or straight above it with the heading up (`top`: Z cannot be the up vector of a
/// camera that looks straight down).
pub fn map_view(look_at: DVec2, fwd: DVec2, zoom: f64, top: bool) -> Mat4 {
    let pitch = if top { 90.0 } else { PITCH }.to_radians();
    let back = fwd * zoom * pitch.cos();
    let eye = DVec3::new(look_at.x - back.x, look_at.y - back.y, zoom * pitch.sin());
    let up = if top {
        Vec3::new(fwd.x as f32, fwd.y as f32, 0.0)
    } else {
        Vec3::Z
    };
    Mat4::look_at_rh(
        eye.as_vec3(),
        Vec3::new(look_at.x as f32, look_at.y as f32, 0.0),
        up,
    )
}

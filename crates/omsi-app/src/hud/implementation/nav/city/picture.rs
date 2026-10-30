//! The city map's picture: the roads, the surface, the route, the lines, the vehicles and
//! the stops, the header, the cards and the buttons, drawn into the window's texture.

use super::super::draw::{
    arrow, focus_marker, player_triangle, rects_overlap, rot, spaced_markers, stop_label_rect,
    traffic_kind, vehicle_icon, DRIVEN, STREET, TEXT, TEXT_DIM,
};
use super::super::geometry::{road_colors, road_geometry, simplify};
use super::super::lines::{line_color, lines_text};
use super::super::route::{build_route, lane_from_right, RouteStyle};
use super::super::{words, LinesView, NavFrame, Navigator, DOT, LINE_TEXT, PLAYER, ROUTE};
use super::*;
use glam::{DVec2, DVec3, Mat4, Vec2, Vec3};
use hashbrown::HashMap;
use omsi_render::{Renderer, Scene};
use omsi_ui::paint::Align;
use omsi_ui::theme::{v5, RADIUS};
use omsi_ui::{tr, Color, Draw, Layer, Painter, Rect, Vertex, Weight};

impl Navigator {
    /// Draw the city map and lay it over the picture.
    pub(in crate::hud) fn city(&mut self, renderer: &Renderer, scene: &mut Scene, f: &NavFrame) {
        self.atlas.begin_frame();
        let (sw, sh) = f.screen;
        let (w, h) = ((sw * 0.8).round(), (sh * 0.82).round());
        let (x0, y0) = (((sw - w) * 0.5).round(), ((sh - h) * 0.5).round());
        self.city.rect = [x0, y0, x0 + w, y0 + h];
        let (tw, th) = (w as u32, h as u32);
        if self.gpu.is_none() {
            self.gpu = Some(self.new_gpu(renderer));
        }
        if self
            .city
            .target
            .map(|t| (t.1, t.2) != (tw, th))
            .unwrap_or(true)
        {
            if let Some((t, _, _)) = self.city.target.take() {
                renderer.free_texture(scene, t);
                scene.premultiplied.remove(&t);
            }
            let t = renderer.add_render_texture(scene, tw, th);
            scene.premultiplied.insert(t);
            self.city.target = Some((t, tw, th));
        }
        if self.city.follow {
            self.city.center = f.bus.truncate();
        }
        let s = (h / 760.0).clamp(0.95, 2.0) * f.ui_scale;
        let global = self.global.clone();
        let net = global.as_deref().or(f.traffic.map(|t| &t.net));
        // roads of the whole map (buffer 3), once per map version
        let mut roads_verts = None;
        if let Some(n) = net {
            let version =
                (self.global_version * 1_000_000 + n.lanes.len() as u64) * 2 + self.light as u64;
            if self.city.roads.map(|r| r.0 != version).unwrap_or(true) {
                let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
                let road_lanes = road_geometry(n);
                for l in &road_lanes {
                    for p in &l.points {
                        lo = lo.min(p.truncate());
                        hi = hi.max(p.truncate());
                    }
                }
                if lo.x == f64::MAX {
                    lo = f.bus.truncate();
                    hi = lo;
                }
                let anchor = (lo + hi) * 0.5;
                self.city.extent = (lo, hi);
                let rel =
                    |q: DVec3| Vec3::new((q.x - anchor.x) as f32, (q.y - anchor.y) as f32, 0.0);
                let mut p = Painter::new();
                let (edge, road, main) = road_colors(self.light);
                let edge = if self.light {
                    edge
                } else {
                    Color::rgba(26, 26, 26, 1.0)
                };
                for pass in 0..2 {
                    for l in &road_lanes {
                        let pts =
                            simplify(&l.points.iter().map(|q| rel(*q)).collect::<Vec<_>>(), 0.12);
                        if pass == 0 {
                            p.ribbon(&pts, l.width + 2.0, 2.4, edge, true);
                        } else {
                            p.ribbon(&pts, l.width, 1.4, if l.main { main } else { road }, true);
                        }
                    }
                }
                self.city.roads = Some((version, p.len(), anchor));
                roads_verts = Some(p.verts);
            }
        }
        let anchor = self.city.roads.map(|r| r.2).unwrap_or(f.bus.truncate());
        // the map's surface (buffer 7), once per surface and anchor
        let mut surface_verts = None;
        if let (true, Some(sf)) = (self.show_surface, self.surface.clone()) {
            // (the contour lines are fainter the wider the view: the strength is part of the key)
            let strength = if self.show_contours {
                crate::map_surface::contour_strength(self.city.mpp)
            } else {
                0.0
            };
            let band = if strength >= 1.0 {
                0
            } else if strength >= 0.5 {
                1
            } else {
                2
            };
            let key =
                (self.surface_version * 4 + self.light as u64 + 2 * self.show_contours as u64) * 3
                    + band;
            if self
                .city
                .surface
                .map(|m| m.0 != key || m.2 != anchor)
                .unwrap_or(true)
            {
                let mut p = Painter::new();
                sf.build(&mut p, anchor, None, self.light, strength);
                self.city.surface = Some((key, p.len(), anchor));
                surface_verts = Some(p.verts);
            }
        }
        let surface_n = if self.show_surface {
            self.city.surface.map(|m| m.1).unwrap_or(0)
        } else {
            0
        };
        let rel = |q: DVec3| Vec3::new((q.x - anchor.x) as f32, (q.y - anchor.y) as f32, 0.0);
        // the lines panel: the view goes to a line just ticked, and the ticked ways are a mesh (buffer 10)
        let mut ln_verts = None;
        if let (true, Some(sf)) = (self.city.lines.open, self.surface.clone()) {
            let route_of = |st: &LinesView, i: usize| {
                sf.lines().get(i).and_then(|l| {
                    l.routes.get(
                        st.route
                            .get(&i)
                            .copied()
                            .unwrap_or(0)
                            .min(l.routes.len().saturating_sub(1)),
                    )
                })
            };
            if std::mem::take(&mut self.city.lines.fit) {
                if let Some(r) = self
                    .city
                    .lines
                    .focus
                    .or(self.city.lines.selected.last().copied())
                    .and_then(|i| route_of(&self.city.lines, i))
                {
                    // (the bounds as the screen sees them: in the turned view's own axes)
                    let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
                    for q in &r.pts {
                        let q = rot(*q, self.city.phi);
                        lo = lo.min(q);
                        hi = hi.max(q);
                    }
                    if lo.x <= hi.x {
                        let (free_w, free_h) = (
                            ((w - 332.0 * s) - 60.0 * s).max(300.0) as f64,
                            (h - HEAD_H * s - 60.0 * s).max(200.0) as f64,
                        );
                        let mpp = (((hi.x - lo.x) / free_w).max((hi.y - lo.y) / free_h) * 1.12)
                            .clamp(0.25, self.max_mpp());
                        self.city.mpp = mpp;
                        // (the panel takes the right side: the way is centred in what is left)
                        self.city.center = rot(
                            (lo + hi) * 0.5 + DVec2::new(166.0 * s as f64 * mpp, 0.0),
                            -self.city.phi,
                        );
                        self.city.follow = false;
                    }
                }
            }
            let key = (self.city.lines.version << 8) ^ self.surface_version;
            if self
                .city
                .lines
                .mesh
                .map(|m| m.0 != key || m.2 != anchor)
                .unwrap_or(true)
            {
                let mut p = Painter::new();
                for &i in &self.city.lines.selected {
                    let Some(l) = sf.lines().get(i) else { continue };
                    let c = line_color(i);
                    for r in &l.routes {
                        let v: Vec<Vec3> = r.pts.iter().map(|q| rel(q.extend(0.0))).collect();
                        p.ribbon(&v, 7.0, 3.4, c.alpha(0.62), false);
                        for q in &r.stops {
                            p.world_disc(rel(q.extend(0.0)), 4.5, 3.6, Color::rgba(8, 8, 8, 0.85));
                            p.world_disc(rel(q.extend(0.0)), 3.0, 2.4, Color::WHITE.alpha(0.95));
                        }
                    }
                }
                self.city.lines.mesh = Some((key, p.len(), anchor));
                ln_verts = Some(p.verts);
            }
        }
        let ln_n = if self.city.lines.open {
            self.city.lines.mesh.map(|m| m.1).unwrap_or(0)
        } else {
            0
        };
        // the whole route (buffer 4): the part driven grey, the rest by how busy it is,
        // with arrows about 90 pixels apart (built again when the zoom changes that much)
        let mut route_verts = None;
        let every = 2f64.powf((90.0 * s as f64 * self.city.mpp).log2().round()) as f32;
        let key = (
            self.route.version,
            self.jam_version,
            every.to_bits(),
            self.city.roads.map(|r| r.0).unwrap_or(0),
        );
        if self.city.route.0 != key {
            let mut p = Painter::new();
            if let Some(n) = net {
                let r = &self.route;
                let done = r.progress.min(r.lanes.len());
                for &l in &r.lanes[..done] {
                    let Some(lane) = n.lanes.get(l) else { continue };
                    let pts: Vec<Vec3> = lane.points.iter().map(|q| rel(*q)).collect();
                    p.ribbon(&pts, lane.width.max(3.0) + 2.0, 5.0, DRIVEN, true);
                }
                let style = RouteStyle {
                    extra_m: 0.0,
                    min_px: 6.0,
                    arrows: Some((every, f32::MAX)),
                    max_len: f32::MAX,
                    near: None,
                };
                let bus_lane = r
                    .lanes
                    .get(done)
                    .map(|&l| lane_from_right(n, l, self.bus_at))
                    .unwrap_or(0);
                build_route(
                    &mut p,
                    n,
                    &r.lanes[done..],
                    anchor,
                    r.s,
                    &self.route_jam,
                    &style,
                    bus_lane,
                );
            }
            self.city.route = (key, p.len());
            route_verts = Some(p.verts);
        }
        // the view: north up, `mpp` metres a pixel
        let c = self.city.center - anchor;
        let (hw, hh) = (
            w as f64 * 0.5 * self.city.mpp,
            h as f64 * 0.5 * self.city.mpp,
        );
        // the turn: as the map is drawn, real north up (where the package says which way it lies) or the heading up
        let north = self.surface.as_ref().map_or(0.0, |sf| sf.north() as f64);
        let target = match self.orient {
            1 => north,
            2 => f.heading,
            _ => 0.0,
        }
        .to_radians();
        // (the view turns to it the short way round instead of jumping)
        let phi = super::super::geometry::turn_towards(self.city.phi, target, f.dt as f64);
        self.city.phi = phi;
        let proj = Mat4::orthographic_rh(
            -hw as f32, hw as f32, -hh as f32, hh as f32, -1000.0, 1000.0,
        ) * Mat4::from_rotation_z(phi as f32)
            * Mat4::from_translation(Vec3::new(-c.x as f32, -c.y as f32, 0.0));
        let vp = [0.0, 0.0, w, h];
        let world = Layer {
            view_proj: proj,
            viewport: vp,
            clip: [0.0, 0.0, w, h],
            radius: RADIUS * s,
            opacity: 1.0,
            px_scale: self.city.mpp as f32,
        };
        let to_screen = |q: DVec3| -> Vec2 {
            let d = rot(q.truncate() - self.city.center, phi);
            Vec2::new(
                (w as f64 * 0.5 + d.x / self.city.mpp) as f32,
                (h as f64 * 0.5 - d.y / self.city.mpp) as f32,
            )
        };
        let win = Rect::new(0.0, 0.0, w, h);
        let mut bg = Painter::new();
        // (the opacity setting the map's ground too; the roads, names and header stay solid)
        let ground = match (self.show_surface, self.surface.as_ref()) {
            (true, Some(sf)) => sf
                .palette(self.light)
                .land
                .alpha(crate::hud::backdrop(self.opacity).min(1.0)),
            _ => v5::BAR.alpha(crate::hud::backdrop(self.opacity).min(1.0)),
        };
        bg.rounded(win, RADIUS * s, ground);
        let n_bg = bg.len();
        // traffic: blue dots, the public transport in its colours with its line (as on the
        // small map)
        let mut dots = Painter::new();
        let mut lines: Vec<(DVec3, Color, String)> = Vec::new();
        // the lines ticked on the lines panel: their numbers, and how many cars run on each
        let mut ticked: HashMap<String, (usize, Color)> = HashMap::new();
        let mut on_line: HashMap<usize, usize> = HashMap::new();
        if let (true, Some(sf)) = (self.city.lines.open, self.surface.as_ref()) {
            for &i in &self.city.lines.selected {
                if let Some(l) = sf.lines().get(i) {
                    ticked.insert(l.number.clone(), (i, line_color(i)));
                }
            }
        }
        if let Some(t) = f.traffic.filter(|_| self.show_ai) {
            for car in t.cars.iter().filter(|c| !c.gone) {
                let (mut color, line, icon) =
                    traffic_kind(car, self.surface.as_deref(), f.car_line);
                // a car on a ticked line: in that line's colour, with a ring round it
                if let Some(&(idx, c)) = line.as_deref().and_then(|l| ticked.get(l.trim())) {
                    color = c;
                    *on_line.entry(idx).or_insert(0) += 1;
                    dots.world_disc(rel(car.vehicle.position), 9.0, 11.0, c.alpha(0.30));
                }
                if color == DOT {
                    dots.world_disc(
                        rel(car.vehicle.position),
                        2.2,
                        3.4,
                        Color::rgba(8, 8, 8, 0.9),
                    );
                    dots.world_disc(rel(car.vehicle.position), 1.5, 2.3, color);
                } else {
                    vehicle_icon(
                        &mut dots,
                        rel(car.vehicle.position),
                        car.vehicle.heading,
                        color,
                        icon,
                        7.5,
                    );
                }
                if let Some(l) = line {
                    lines.push((car.vehicle.position, color, l));
                }
            }
        }
        let n_dots = dots.len();
        // stops with their names, the bus, the frame and the header
        let mut ui = Painter::new();
        let n_stops = f.stops.len();
        let lt = lines_text();
        let (head_h, pad) = (HEAD_H * s, 16.0 * s);
        let have_lines = self
            .surface
            .as_ref()
            .is_some_and(|sf| !sf.lines().is_empty());
        let lines_open = have_lines && self.city.lines.open;
        // the cards over the map: where the bus is (top left), the trip (top right, where the
        // lines panel is not) and what the colours mean (bottom right)
        let card_w = (260.0 * s).min(w * 0.3);
        let here = self.here_rows(&lt);
        let here_card = (!here.is_empty())
            .then(|| Rect::new(pad, head_h + 12.0 * s, card_w, card_h(here.len(), s)));
        let trip = if lines_open {
            Vec::new()
        } else {
            self.trip_rows(f, &lt)
        };
        let trip_card = (!trip.is_empty()).then(|| {
            Rect::new(
                w - pad - card_w,
                head_h + 12.0 * s,
                card_w,
                card_h(trip.len(), s),
            )
        });
        let you = match self
            .surface
            .as_ref()
            .filter(|_| self.show_surface)
            .and_then(|sf| sf.player_style(f.line.as_deref()))
        {
            Some((fill, _)) => fill,
            None => TEXT,
        };
        let legend = (!lines_open).then(|| self.legend_layout(&lt, you, w, h, s));
        // street names along their roads, where there is room (stops' names go first)
        let markers = spaced_markers(
            f.stops
                .iter()
                .enumerate()
                .map(|(k, st)| (k, to_screen(st.position)))
                .filter(|(_, p)| win.contains(*p) && p.y > 50.0 * s),
            20.0 * s,
        );
        let mut taken: Vec<Rect> = markers
            .iter()
            .map(|(_, p)| Rect::new(p.x - 9.0 * s, p.y - 9.0 * s, 18.0 * s, 18.0 * s))
            .collect();
        taken.push(Rect::new(0.0, 0.0, w, head_h + 6.0 * s));
        taken.extend(
            here_card
                .iter()
                .chain(&trip_card)
                .chain(legend.iter().map(|l| &l.0))
                .map(|r| r.pad(-6.0 * s, -6.0 * s)),
        );
        let mut stop_labels = Vec::new();
        for &(k, p) in &markers {
            if self.city.mpp >= 4.0 && k != 0 && k + 1 != n_stops {
                continue;
            }
            let st = &f.stops[k];
            let weight = if k == 0 { Weight::Bold } else { Weight::Medium };
            let time = clock_of(st.arrival);
            let tw = self.fonts.width(&time, 11.5 * s, Weight::Medium);
            let name = self.fonts.fit(
                st.name.trim(),
                12.5 * s,
                weight,
                (300.0 * s).min(w * 0.3) - tw,
            );
            let lw = self.fonts.width(&name, 12.5 * s, weight) + tw + 26.0 * s;
            if let Some(r) = stop_label_rect(p, lw, s, win, &taken) {
                taken.push(r);
                stop_labels.push((k, name, time, r));
            }
        }
        // the public transport's line tags, above their dots, where there is room
        for (pos, color, l) in &lines {
            let p = to_screen(*pos);
            if !win.contains(p) {
                continue;
            }
            let px = 11.0 * s;
            let l = self.fonts.fit(l, px, Weight::Bold, 50.0 * s);
            let tw = self.fonts.width(&l, px, Weight::Bold) + 9.0 * s;
            let r = Rect::new(p.x - tw * 0.5, p.y - 22.0 * s, tw, 15.0 * s);
            if taken
                .iter()
                .any(|o| o.x < r.right() && r.x < o.right() && o.y < r.bottom() && r.y < o.bottom())
            {
                continue;
            }
            taken.push(r);
            ui.rounded(
                Rect::new(r.x - 1.0 * s, r.y - 1.0 * s, r.w + 2.0 * s, r.h + 2.0 * s),
                4.5 * s,
                Color::rgba(10, 10, 10, 0.9),
            );
            ui.rounded(r, 4.0 * s, *color);
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
        if self.show_surface && self.surface.is_some() {
            // the municipalities' names as a watermark: big, faint, inside their outlines (the box
            // the package computed); out of sight when zoomed in, where the streets' names take over,
            // and when the view is turned (the names are written upright)
            if let Some(sf) = self
                .surface
                .clone()
                .filter(|_| !self.show_contours && phi.abs() < 1e-6)
            {
                let col = if self.light {
                    Color::rgba(70, 58, 44, 0.26)
                } else {
                    Color::WHITE.alpha(0.15)
                };
                for r in sf.regions().iter().filter(|r| r.box_w > 0.0) {
                    let p = to_screen(r.label.extend(0.0));
                    let (bw, bh) = (
                        (r.box_w / self.city.mpp) as f32,
                        (r.box_h / self.city.mpp) as f32,
                    );
                    if bh > 110.0 * s
                        || p.x + bw * 0.5 < 0.0
                        || p.x - bw * 0.5 > w
                        || p.y + bh < 0.0
                        || p.y - bh > h
                    {
                        continue;
                    }
                    let name = r.name.to_uppercase();
                    let mut px = bh;
                    let tw = self.fonts.width(&name, px, Weight::Bold);
                    if tw > bw {
                        px *= bw / tw;
                    }
                    // (in steps: every size is a set of glyphs in the atlas)
                    px = (px / 6.0).round() * 6.0;
                    if px < 12.0 * s {
                        continue;
                    }
                    ui.text(
                        &mut self.atlas,
                        &self.fonts,
                        &name,
                        px,
                        Weight::Bold,
                        p + Vec2::new(0.0, px * 0.36),
                        Align::Center,
                        col,
                    );
                }
            }
            // names of districts, terminals and towns, the more important ones from further away
            let min_rank = if self.city.mpp > 60.0 {
                200
            } else if self.city.mpp > 25.0 {
                140
            } else if self.city.mpp > 10.0 {
                100
            } else {
                0
            };
            let px = 13.0 * s;
            for l in &self.surface_labels {
                if l.rank < min_rank {
                    continue;
                }
                let p = to_screen(l.at.extend(0.0));
                if !win.pad(40.0 * s, 30.0 * s).contains(p) {
                    continue;
                }
                let weight = if l.kind == 0 {
                    Weight::Medium
                } else {
                    Weight::Bold
                };
                let name = self.fonts.fit(&l.name, px, weight, 220.0 * s);
                let tw = self.fonts.width(&name, px, weight);
                let bb = Rect::new(p.x - tw * 0.5, p.y - px * 0.7, tw, px * 1.3);
                if taken.iter().any(|t| rects_overlap(t, &bb)) {
                    continue;
                }
                taken.push(bb);
                let (halo, ink_dim, ink) = if self.light {
                    (
                        Color::hex(0xf4f1ea).alpha(0.9),
                        Color::hex(0x5b564c),
                        Color::hex(0x2d2a25),
                    )
                } else {
                    (Color::rgba(15, 15, 15, 0.9), TEXT_DIM, TEXT)
                };
                let at = p + Vec2::new(0.0, px * 0.35);
                for o in [
                    Vec2::new(1.0, 0.0),
                    Vec2::new(-1.0, 0.0),
                    Vec2::new(0.0, 1.0),
                    Vec2::new(0.0, -1.0),
                ] {
                    ui.text(
                        &mut self.atlas,
                        &self.fonts,
                        &name,
                        px,
                        weight,
                        at + o * s,
                        Align::Center,
                        halo,
                    );
                }
                ui.text(
                    &mut self.atlas,
                    &self.fonts,
                    &name,
                    px,
                    weight,
                    at,
                    Align::Center,
                    if l.kind == 0 { ink_dim } else { ink },
                );
            }
            // the terrain view: the heights written on the contour lines, semi-transparent
            if self.show_contours {
                if let Some(sf) = self.surface.clone() {
                    let px = 11.5 * s;
                    let col = if self.light {
                        Color::rgba(110, 72, 34, 0.62)
                    } else {
                        Color::rgba(236, 206, 156, 0.55)
                    };
                    let inside = win.pad(30.0 * s, 30.0 * s);
                    // (asking starts the worker once; until it is done the lines have no heights. This also covers a
                    // terrain view that was already on when the map opened)
                    sf.request_contour_marks();
                    let weight = |m: &crate::map_surface::ContourMark| {
                        if m.index {
                            Weight::Medium
                        } else {
                            Weight::Regular
                        }
                    };
                    let label_box = |p: Vec2, m: &crate::map_surface::ContourMark| {
                        let tw = self.fonts.width(&format!("{} m", m.height), px, weight(m));
                        Rect::new(p.x - tw * 0.5, p.y - px * 0.7, tw, px * 1.3)
                    };
                    let marks = crate::map_surface::marks_to_write(
                        sf.contour_marks().unwrap_or(&[]),
                        self.city.mpp,
                        s,
                        |at| to_screen(at.extend(0.0)),
                        |p| inside.contains(p),
                        &taken,
                        label_box,
                    );
                    for (p, m) in marks {
                        ui.text(
                            &mut self.atlas,
                            &self.fonts,
                            &format!("{} m", m.height),
                            px,
                            weight(m),
                            p + Vec2::new(0.0, px * 0.35),
                            Align::Center,
                            col,
                        );
                    }
                }
            }
            // the data's attribution (the map's licence asks for it)
            if let Some(sf) = self.surface.as_ref().filter(|s| !s.credit.is_empty()) {
                ui.text(
                    &mut self.atlas,
                    &self.fonts,
                    &sf.credit,
                    10.5 * s,
                    Weight::Medium,
                    Vec2::new(16.0 * s, h - 38.0 * s),
                    Align::Left,
                    if self.light {
                        Color::hex(0x5b564c).alpha(0.8)
                    } else {
                        TEXT_DIM.alpha(0.7)
                    },
                );
            }
        }
        // the compass: where real north lies on the screen; a click changes how the map is turned
        {
            let r = 24.0 * s;
            let c = match legend.as_ref() {
                Some((l, _)) => Vec2::new(l.right() - 12.0 * s - r, l.y - 30.0 * s - r),
                // (no legend means the lines panel is open: the compass stands left of it)
                None => compass_centre(w, h, s, true),
            };
            let red = v5::DANGER;
            let grey = Color::rgba(190, 196, 204, 1.0);
            ui.rounded(
                Rect::new(c.x - r, c.y - r, 2.0 * r, 2.0 * r),
                r,
                v5::CARD.alpha(0.94),
            );
            ui.rounded_border(
                Rect::new(c.x - r, c.y - r, 2.0 * r, 2.0 * r),
                r,
                1.0,
                v5::LINE,
            );
            let ang = (north.to_radians() - phi) as f32; // clockwise from the screen's up
            let dir = Vec2::new(ang.sin(), -ang.cos());
            let side = Vec2::new(-dir.y, dir.x);
            ui.tri(
                c + dir * (0.78 * r),
                c + side * (0.26 * r),
                c - side * (0.26 * r),
                red,
                red,
                red,
            );
            ui.tri(
                c - dir * (0.78 * r),
                c + side * (0.26 * r),
                c - side * (0.26 * r),
                grey,
                grey,
                grey,
            );
            ui.text(
                &mut self.atlas,
                &self.fonts,
                "N",
                10.5 * s,
                Weight::Bold,
                c + dir * (1.18 * r) + Vec2::new(0.0, 4.0 * s),
                Align::Center,
                TEXT,
            );
            ui.text(
                &mut self.atlas,
                &self.fonts,
                lines_text().orient[self.orient.min(2) as usize],
                10.5 * s,
                Weight::Medium,
                Vec2::new(c.x, c.y + r + 13.0 * s),
                Align::Center,
                v5::MUTED,
            );
            // (listed with the buttons on the next frame, after they are cleared)
            self.city.compass = Some(Rect::new(c.x - r, c.y - r, 2.0 * r, 2.0 * r + 16.0 * s));
        }
        if let (Some(st), true) = (
            self.streets.clone(),
            self.city.mpp < 3.2 && self.global.is_some(),
        ) {
            let px = 12.0 * s;
            for (q, a, id) in &st.labels {
                let p = to_screen(q.extend(0.0));
                if !win.pad(60.0 * s, 30.0 * s).contains(p) {
                    continue;
                }
                let name = &st.names[*id as usize];
                let tw = self.fonts.width(name, px, Weight::Medium);
                // (upright: turned at most a quarter either way)
                let mut ang = -(*a + phi as f32);
                // (the turn of the view can take it past half a turn: back to -pi..pi first)
                ang = (ang + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                if ang > std::f32::consts::FRAC_PI_2 {
                    ang -= std::f32::consts::PI;
                } else if ang <= -std::f32::consts::FRAC_PI_2 {
                    ang += std::f32::consts::PI;
                }
                // beside the road, not on it
                let side = Vec2::new(-ang.sin(), ang.cos()) * (px * 0.9 + 2.0 * s);
                let c = p + side;
                let (hx, hy) = (
                    (ang.cos() * tw * 0.5).abs() + (ang.sin() * px * 0.6).abs(),
                    (ang.sin() * tw * 0.5).abs() + (ang.cos() * px * 0.6).abs(),
                );
                let bb = Rect::new(c.x - hx, c.y - hy, hx * 2.0, hy * 2.0);
                if taken.iter().any(|t| rects_overlap(t, &bb)) {
                    continue;
                }
                taken.push(bb);
                let (halo, ink) = if self.light {
                    (Color::hex(0xf4f1ea).alpha(0.9), Color::hex(0x4a463f))
                } else {
                    (v5::BAR.alpha(0.9), STREET)
                };
                for o in [
                    Vec2::new(1.0, 0.0),
                    Vec2::new(-1.0, 0.0),
                    Vec2::new(0.0, 1.0),
                    Vec2::new(0.0, -1.0),
                ] {
                    ui.text_rotated(
                        &mut self.atlas,
                        &self.fonts,
                        name,
                        px,
                        Weight::Medium,
                        c + o * s,
                        ang,
                        halo,
                    );
                }
                ui.text_rotated(
                    &mut self.atlas,
                    &self.fonts,
                    name,
                    px,
                    Weight::Medium,
                    c,
                    ang,
                    ink,
                );
            }
        }
        for (k, p) in markers.into_iter().rev() {
            let next = k == 0;
            if self.city.mpp < 4.0 || next || k + 1 == n_stops {
                let badge = if next { 7.0 } else { 5.5 } * s;
                ui.circle(p, badge + 1.5 * s, Color::rgba(8, 8, 8, 0.95));
                let (fill, ink) = if next {
                    (omsi_ui::theme::accent(), omsi_ui::theme::on_accent())
                } else if k + 1 == n_stops {
                    (ROUTE, TEXT)
                } else {
                    (v5::LINE, v5::FG)
                };
                ui.circle(p, badge, fill);
                ui.icon(
                    &mut self.atlas,
                    "directions_bus",
                    p,
                    if next { 11.5 } else { 9.5 } * s,
                    ink,
                );
            } else {
                ui.circle(p, 5.0 * s, Color::rgba(12, 12, 12, 0.95));
                ui.circle(p, 3.2 * s, TEXT_DIM);
            }
        }
        // (pills: the name, and the time in a quieter ink)
        for (k, name, time, r) in stop_labels {
            ui.rounded(r, r.h * 0.5, v5::CARD.alpha(0.94));
            ui.rounded_border(
                r,
                r.h * 0.5,
                1.0,
                if k == 0 {
                    omsi_ui::theme::accent()
                } else {
                    v5::LINE
                },
            );
            let inner = r.pad(10.0 * s, 0.0);
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &name,
                12.5 * s,
                if k == 0 { Weight::Bold } else { Weight::Medium },
                inner,
                Align::Left,
                v5::FG,
            );
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &time,
                11.5 * s,
                Weight::Medium,
                inner,
                Align::Right,
                v5::MUTED,
            );
        }
        // the other players: an arrow the way they face and their name (as on the small map)
        for pl in &f.players {
            let p = to_screen(pl.position);
            if !win.contains(p) || p.y < head_h {
                continue;
            }
            arrow(
                &mut ui,
                p,
                (pl.heading.to_radians() - phi) as f32,
                8.5 * s,
                1.3,
                Color::rgba(10, 10, 10, 0.9),
                PLAYER,
            );
            let px = 11.0 * s;
            let name = self.fonts.fit(&pl.name, px, Weight::Bold, 140.0 * s);
            let tw = self.fonts.width(&name, px, Weight::Bold) + 9.0 * s;
            let r = Rect::new(p.x - tw * 0.5, p.y - 27.0 * s, tw, 15.0 * s);
            if taken.iter().any(|o| rects_overlap(o, &r)) {
                continue;
            }
            taken.push(r);
            ui.rounded(
                Rect::new(r.x - 1.0 * s, r.y - 1.0 * s, r.w + 2.0 * s, r.h + 2.0 * s),
                4.5 * s,
                Color::rgba(10, 10, 10, 0.9),
            );
            ui.rounded(r, 4.0 * s, PLAYER);
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
        if let Some((at, heading)) = self.bus_marker.filter(|_| f.focus.is_some()) {
            arrow(
                &mut ui,
                to_screen(at),
                (heading.to_radians() - phi) as f32,
                8.0 * s,
                1.3,
                Color::rgba(10, 10, 10, 0.9),
                TEXT_DIM,
            );
        }
        if let Some(focus) = f.focus {
            focus_marker(
                &mut ui,
                focus.kind,
                to_screen(f.bus),
                (f.heading.to_radians() - phi) as f32,
                13.0 * s,
            );
        } else if let Some((fill, ring)) = self
            .surface
            .as_ref()
            .filter(|_| self.show_surface)
            .and_then(|s| s.player_style(f.line.as_deref()))
        {
            player_triangle(
                &mut ui,
                to_screen(f.bus),
                (f.heading.to_radians() - phi) as f32,
                24.0 * s,
                fill,
                ring,
            );
        } else {
            arrow(
                &mut ui,
                to_screen(f.bus),
                (f.heading.to_radians() - phi) as f32,
                10.0 * s,
                1.3,
                Color::rgba(10, 10, 10, 0.9),
                TEXT,
            );
        }
        // the map's icons: terminals, BRT stations, the garage
        let mut icon_verts: Vec<Vertex> = Vec::new();
        if let (true, Some(sf)) = (self.show_surface, self.surface.as_ref()) {
            for m in sf.markers() {
                let p = to_screen(m.at.extend(0.0));
                if win.pad(30.0 * s, 30.0 * s).contains(p) {
                    sf.push_icon(m, p, s, &mut icon_verts);
                }
            }
        }
        // header: what this is, the line and where it goes, the next stop; buttons on the right
        // (opaque: the route and the stops showed through behind its text)
        let head = Rect::new(0.0, 0.0, w, head_h);
        ui.rect(head, WINDOW_BAR);
        ui.rect(Rect::new(0.0, head.bottom() - 1.0, w, 1.0), v5::LINE);
        self.city.buttons.clear();
        if let Some(r) = self.city.compass.take() {
            self.city.buttons.push((r, 6));
        }
        let bs = 32.0 * s;
        let by = head.y + (head.h - bs) * 0.5;
        let mut bx = w - pad - bs;
        for (icon, id) in [("close", 3u8), ("remove", 2), ("add", 1)] {
            let r = Rect::new(bx, by, bs, bs);
            let ink = outlined(&mut ui, r, s, false);
            ui.icon(&mut self.atlas, icon, r.center(), 18.0 * s, ink);
            self.city.buttons.push((r, id));
            bx -= bs + 8.0 * s;
        }
        // following the bus, the lines (only where the map's package has bus lines) and the
        // terrain view (only where it carries contour lines): lit in the accent while on
        let has_contours =
            self.show_surface && self.surface.as_ref().is_some_and(|sf| sf.has_contours());
        for (id, label, on) in [
            (0u8, lt.follow, self.city.follow),
            (4, lt.lines, self.city.lines.open),
            (5, lt.terrain, self.show_contours),
        ] {
            if (id == 4 && !have_lines) || (id == 5 && !has_contours) {
                continue;
            }
            let bw = self.fonts.width(label, 13.0 * s, Weight::Bold) + 44.0 * s;
            let r = Rect::new(bx + bs - bw, by, bw, bs);
            let ink = outlined(&mut ui, r, s, on);
            let (cx, cy) = (r.x + 17.0 * s, r.center().y);
            if id == 5 {
                // (a mountain pictogram)
                let cy = cy + 6.0 * s;
                ui.tri(
                    Vec2::new(cx - 10.0 * s, cy),
                    Vec2::new(cx - 1.0 * s, cy - 12.0 * s),
                    Vec2::new(cx + 6.0 * s, cy),
                    ink,
                    ink,
                    ink,
                );
                let back = ink.alpha(0.6);
                ui.tri(
                    Vec2::new(cx - 2.0 * s, cy),
                    Vec2::new(cx + 6.0 * s, cy - 8.0 * s),
                    Vec2::new(cx + 13.0 * s, cy),
                    back,
                    back,
                    back,
                );
            } else {
                ui.icon(
                    &mut self.atlas,
                    if id == 0 { "my_location" } else { "route" },
                    Vec2::new(cx, cy),
                    16.0 * s,
                    ink,
                );
            }
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                label,
                13.0 * s,
                Weight::Bold,
                Rect::new(r.x + 30.0 * s, r.y, r.w - 38.0 * s, r.h),
                Align::Center,
                ink,
            );
            self.city.buttons.push((r, id));
            bx = r.x - 8.0 * s - bs;
        }
        // (the texts keep clear of the buttons)
        let right = bx + bs - 16.0 * s;
        let mut tx = pad;
        // (the window's title, then the line as a badge in the accent and where it goes)
        tx += ui.text_in(
            &mut self.atlas,
            &self.fonts,
            &tr(lt.city_map),
            17.0 * s,
            Weight::Bold,
            Rect::new(tx, head.y, (right - tx).max(0.0), head.h),
            Align::Left,
            v5::FG,
        ) + 16.0 * s;
        let wd = words(f.language);
        match (&f.line, &f.terminus) {
            (Some(l), Some(t)) if right - tx >= 24.0 * s => {
                let num = l.trim();
                // (the badge never grows past the room the buttons leave, and is left out
                // when there is hardly any)
                let room = (right - tx).max(0.0);
                let bw = (self.fonts.width(num, 13.0 * s, Weight::Bold) + 16.0 * s).min(room);
                let badge = Rect::new(tx, head.center().y - 11.0 * s, bw, 22.0 * s);
                ui.rounded(badge, 5.0 * s, omsi_ui::theme::accent());
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    num,
                    13.0 * s,
                    Weight::Bold,
                    badge,
                    Align::Center,
                    omsi_ui::theme::on_accent(),
                );
                tx += bw + 10.0 * s;
                tx += ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    t.trim(),
                    15.0 * s,
                    Weight::Bold,
                    Rect::new(tx, head.y, (right - tx).max(0.0), head.h),
                    Align::Left,
                    v5::FG,
                ) + 22.0 * s;
            }
            // (a duty, but no room for it)
            (Some(_), Some(_)) => {}
            _ => {
                tx += ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    wd.no_duty,
                    15.0 * s,
                    Weight::Bold,
                    Rect::new(tx, head.y, (right - tx).max(0.0), head.h),
                    Align::Left,
                    v5::FG,
                ) + 22.0 * s;
            }
        }
        if let (Some(st), true) = (f.stops.first(), right - tx > 80.0 * s) {
            let parts = [
                format!("{}: {}", tr(lt.next_stop), st.name.trim()),
                self.next_dist.map(distance_text).unwrap_or_default(),
                clock_of(st.arrival),
            ];
            let t = parts
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &t,
                13.5 * s,
                Weight::Medium,
                Rect::new(tx, head.y, right - tx, head.h),
                Align::Left,
                v5::MUTED,
            );
        }
        // where the bus is: the municipality and the street, from the package
        if let Some(card) = here_card {
            self.map_card(&mut ui, card, s, lt.you_are_here, &here);
            // the street's limit as a road sign (and the heavy vehicles', smaller, when it is
            // another), beside its row
            let (limit, heavy) = self.loc_limit;
            let row = here
                .iter()
                .position(|r| r.0 == lt.street)
                .filter(|_| self.loc_street.is_some());
            if let Some(k) = row {
                let cy = card.y + (CARD_HEAD + CARD_ROW * (k as f32 + 0.5)) * s;
                let mut sx = card.right() + 8.0 * s;
                for (v, rad) in [(limit, 17.0 * s), (heavy, 12.5 * s)] {
                    if v == 0 {
                        continue;
                    }
                    let c = Vec2::new(sx + rad, cy);
                    ui.rounded(
                        Rect::new(c.x - rad, c.y - rad, 2.0 * rad, 2.0 * rad),
                        rad,
                        Color::rgba(205, 30, 36, 1.0),
                    );
                    ui.rounded(
                        Rect::new(c.x - rad * 0.74, c.y - rad * 0.74, 1.48 * rad, 1.48 * rad),
                        rad * 0.74,
                        Color::WHITE,
                    );
                    ui.text_in(
                        &mut self.atlas,
                        &self.fonts,
                        &v.to_string(),
                        rad * 0.95,
                        Weight::Bold,
                        Rect::new(c.x - rad, c.y - rad, 2.0 * rad, 2.0 * rad),
                        Align::Center,
                        Color::rgba(15, 15, 15, 1.0),
                    );
                    sx += 2.0 * rad + 6.0 * s;
                    if rad < 14.0 * s {
                        ui.text(
                            &mut self.atlas,
                            &self.fonts,
                            lt.heavy,
                            10.5 * s,
                            Weight::Medium,
                            Vec2::new(sx, c.y + 4.0 * s),
                            Align::Left,
                            v5::MUTED,
                        );
                    }
                }
            }
        }
        // the street under the pointer, with its limits (smaller), under the card
        if self.show_surface && self.surface.as_ref().is_some_and(|sf| sf.has_location()) {
            let y = here_card.map_or(head.bottom() + 12.0 * s, |c| c.bottom() + 8.0 * s);
            if let Some((st, alt)) = self.hover_street() {
                let mut parts: Vec<String> = Vec::new();
                if let Some(st) = &st {
                    parts.push(st.name.clone());
                    if st.speed > 0 {
                        parts.push(format!("{} km/h", st.speed));
                    }
                    if st.heavy > 0 {
                        parts.push(format!("{} {}", tr(lt.heavy), st.heavy));
                    }
                }
                if let Some(a) = alt {
                    parts.push(format!("{} {:.0} m", tr(lt.elev), a));
                }
                let text = parts.join("  ·  ");
                let t = self.fonts.fit(&text, 11.5 * s, Weight::Bold, w * 0.42);
                let tw = self.fonts.width(&t, 11.5 * s, Weight::Bold) + 20.0 * s;
                let r = Rect::new(pad, y, tw, 24.0 * s);
                ui.rounded(r, 4.0 * s, v5::CARD.alpha(0.94));
                ui.rounded_border(r, 4.0 * s, 1.0, omsi_ui::theme::accent().alpha(0.8));
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    &t,
                    11.5 * s,
                    Weight::Bold,
                    r,
                    Align::Center,
                    v5::FG,
                );
            }
        }
        // the trip: the next stop, how far and when, the stops still to come
        if let Some(card) = trip_card {
            self.map_card(&mut ui, card, s, lt.trip_card, &trip);
        }
        // what the colours mean, and how the map is moved
        if let Some((r, items)) = &legend {
            ui.rounded(*r, 8.0 * s, v5::CARD.alpha(0.94));
            ui.rounded_border(*r, 8.0 * s, 1.0, v5::LINE);
            let row = Rect::new(r.x + 12.0 * s, r.y + 6.0 * s, r.w - 24.0 * s, 20.0 * s);
            let mut x = row.x;
            for (label, c, bar) in items {
                let cy = row.center().y;
                if *bar {
                    ui.rounded(Rect::new(x, cy - 1.5 * s, 10.0 * s, 3.0 * s), 1.5 * s, *c);
                    x += 15.0 * s;
                } else {
                    ui.circle(Vec2::new(x + 3.0 * s, cy), 3.0 * s, *c);
                    x += 10.0 * s;
                }
                // (the driven part's grey is too dark to read: its name is muted)
                let ink = if *c == DRIVEN { v5::MUTED } else { *c };
                x += ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    label,
                    11.5 * s,
                    Weight::Medium,
                    Rect::new(x, row.y, row.right() - x, row.h),
                    Align::Left,
                    ink,
                ) + 12.0 * s;
            }
            let hint = Rect::new(row.x, row.bottom() + 2.0 * s, row.w, 20.0 * s);
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                lt.keys,
                11.5 * s,
                Weight::Medium,
                hint,
                Align::Left,
                v5::MUTED,
            );
        }
        if lines_open {
            if let (true, Some(sf), Some(i)) = (
                self.city.lines.sheet,
                self.surface.clone(),
                self.city.lines.focus,
            ) {
                let (cc, cm) = (self.city.center, self.city.mpp);
                let at = move |q: DVec3| {
                    let d = rot(q.truncate() - cc, phi);
                    Vec2::new(
                        (w as f64 * 0.5 + d.x / cm) as f32,
                        (h as f64 * 0.5 - d.y / cm) as f32,
                    )
                };
                self.sheet_labels(&mut ui, &sf, i, s, &at, (w, h));
            }
            self.lines_panel(&mut ui, w, h, s, head.h, &lt, &on_line);
        }
        // a scale bar, bottom left
        let nice = [
            10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0,
        ];
        let metres = nice
            .iter()
            .copied()
            .find(|m| m / self.city.mpp > 70.0 * s as f64)
            .unwrap_or(5000.0);
        let len = (metres / self.city.mpp) as f32;
        let by = h - 22.0 * s;
        ui.rect(Rect::new(pad, by, len, 2.0 * s), v5::MUTED);
        ui.text(
            &mut self.atlas,
            &self.fonts,
            &if metres >= 1000.0 {
                format!("{:.0} km", metres / 1000.0)
            } else {
                format!("{metres:.0} m")
            },
            12.0 * s,
            Weight::Medium,
            Vec2::new(pad + len + 8.0 * s, by + 4.0 * s),
            Align::Left,
            v5::MUTED,
        );
        ui.rounded_border(win, RADIUS * s, 1.0, v5::LINE);

        // --- to the GPU
        let (tex, _, _) = self.city.target.unwrap();
        let Some(view) = renderer.texture_view(scene, tex) else {
            return;
        };
        let (Some(gpu), device, queue) = (self.gpu.as_mut(), &renderer.device, &renderer.queue)
        else {
            return;
        };
        if let Some(v) = roads_verts {
            gpu.upload(device, queue, 3, &v);
        }
        if let Some(v) = surface_verts {
            gpu.upload(device, queue, 7, &v);
        }
        if let Some(v) = ln_verts {
            gpu.upload(device, queue, 10, &v);
        }
        if let Some(v) = route_verts {
            gpu.upload(device, queue, 4, &v);
        }
        let mut all = bg.verts;
        all.extend(dots.verts);
        let n_ui = all.len() as u32;
        all.extend(ui.verts);
        gpu.upload(device, queue, 5, &all);
        if self.icon_tex.is_none() {
            if let Some(sheet) = self.surface.as_ref().and_then(|s| s.sheet()) {
                self.icon_tex = Some(gpu.add_image(device, queue, sheet.w, sheet.h, &sheet.rgba));
            }
        }
        gpu.upload(device, queue, 9, &icon_verts);
        let (icon_n, icon_tex) = (icon_verts.len() as u32, self.icon_tex.unwrap_or(0));
        gpu.upload_atlas(queue, &mut self.atlas);
        let flat = Layer::flat([0.0, 0.0, w, h], RADIUS * s, 1.0);
        let layers = [flat, world];
        let roads_n = self.city.roads.map(|r| r.1).unwrap_or(0);
        let draws = [
            Draw {
                buffer: 5,
                range: 0..n_bg,
                layer: 0,
                texture: 0,
            },
            Draw {
                buffer: 7,
                range: 0..surface_n,
                layer: 1,
                texture: 0,
            },
            Draw {
                buffer: 3,
                range: 0..roads_n,
                layer: 1,
                texture: 0,
            },
            Draw {
                buffer: 4,
                range: 0..self.city.route.1,
                layer: 1,
                texture: 0,
            },
            Draw {
                buffer: 10,
                range: 0..ln_n,
                layer: 1,
                texture: 0,
            },
            Draw {
                buffer: 5,
                range: n_bg..n_bg + n_dots,
                layer: 1,
                texture: 0,
            },
            Draw {
                buffer: 9,
                range: 0..icon_n,
                layer: 0,
                texture: icon_tex,
            },
            Draw {
                buffer: 5,
                range: n_ui..all.len() as u32,
                layer: 0,
                texture: 0,
            },
        ];
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("city map"),
        });
        gpu.set_mirror_target(Some(tex));
        gpu.render(
            device,
            queue,
            &mut enc,
            &view,
            (tw, th),
            Some(wgpu::Color::TRANSPARENT),
            &layers,
            &draws,
        );
        queue.submit([enc.finish()]);
        let _ = n_dots;
        scene.overlays.push((tex, [x0, y0, x0 + w, y0 + h]));
    }
}

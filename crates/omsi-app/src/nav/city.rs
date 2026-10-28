use super::draw::{
    arrow, focus_marker, player_triangle, rects_overlap, rot, spaced_markers, stop_label_rect,
    traffic_kind, vehicle_icon, DRIVEN, LEVEL, STREET, TEXT, TEXT_DIM,
};
use super::geometry::{road_colors, road_geometry, simplify};
use super::lines::{
    decimal_comma, line_color, lines_text, LinesText, GROUP_BRT_TAB, GROUP_EXPRESS_TAB,
    GROUP_FEEDER_TAB,
};
use super::route::{build_route, lane_from_right, RouteStyle};
use super::{words, LinesHit, LinesView, NavFrame, Navigator, DOT, LINE_TEXT, PLAYER, ROUTE};
use crate::map_surface::{GROUP_BRT, GROUP_EXPRESS};
use glam::{DVec2, DVec3, Mat4, Vec2, Vec3};
use hashbrown::HashMap;
use omsi_render::{Renderer, Scene};
use omsi_sim::traffic::Network;
use omsi_ui::paint::Align;
use omsi_ui::theme::{v5, RADIUS};
use omsi_ui::{tr, Atlas, Color, Draw, Fonts, Layer, Painter, Rect, Vertex, Weight};

impl Navigator {
    /// Open or close the city map.
    /// Where OMSI 2's dynamic route arrows stand on the route ahead (the `nav_arrows`
    /// setting): every junction lane of the next `reach` metres with the way through it
    /// (`L`, `R`, or `dn` for straight on), and the stops of the trip ahead with their
    /// names (`busstop`). Each: a key that stays the same while it is ahead, the place,
    /// the heading, the kind and its text.
    /// `stop_pose` gives a stop object's place and heading where its tile is loaded.
    pub fn arrow_spots(
        &self,
        traffic: Option<&Network>,
        reach: f64,
        stop_pose: &dyn Fn(i64) -> Option<(DVec3, f64)>,
    ) -> Vec<(u64, DVec3, f64, &'static str, String)> {
        let mut out = Vec::new();
        if !self.arrows {
            return out;
        }
        let global = self.global.clone();
        let Some(net) = global.as_deref().or(traffic) else {
            return out;
        };
        let r = &self.route;
        if !r.on_route {
            return out;
        }
        let mut acc = -(r.s as f64);
        let mut prev_end: Option<f32> = None;
        for (j, &l) in r.lanes.iter().enumerate().skip(r.progress) {
            let Some(lane) = net.lanes.get(l) else { break };
            let len = lane.length();
            if acc > reach {
                break;
            }
            let (h0, h1) = (lane.start_heading(), lane.end_heading());
            let mut d = omsi_sim::traffic::wrap_deg(h1 - h0);
            if let Some(pe) = prev_end {
                d += omsi_sim::traffic::wrap_deg(h0 - pe);
            }
            let junction =
                net.crossings.get(l).map(|c| !c.is_empty()).unwrap_or(false) || lane.turn != 0;
            let turn = d.abs() > 35.0 && (len < 60.0 || d.abs() > 70.0);
            if (turn || junction) && acc + len as f64 > 5.0 {
                let kind = if !turn {
                    "dn"
                } else if d > 0.0 {
                    "R"
                } else {
                    "L"
                };
                // ten metres on from where the path begins, as OMSI puts it (the original:
                // the path's start moved by (0, 0, 10) in its own frame); the mesh hangs
                // 6-11 m over that point
                let (p, _) = lane.at(10.0f32.min(len * 0.7));
                // facing the driver coming in: the path's heading where it begins
                let h = h0;
                // the street the route goes on into (the junction's own lanes have no name)
                let text = r
                    .lanes
                    .iter()
                    .skip(j)
                    .take(5)
                    .find_map(|&x| self.street_of(x))
                    .map(str::to_string)
                    .unwrap_or_default();
                out.push((l as u64, p, h as f64, kind, text));
            }
            prev_end = Some(h1);
            acc += len as f64;
        }
        // The stop's helper: OMSI puts `routearrows_busstop.sco` on the stop object
        // itself, at its place and with its rotation (the station record's
        // position and quaternion) - where and how the mapper set the stop down,
        // not turned to the bus as it comes.
        for (k, (p, name, h, id)) in self.stop_spots.iter().enumerate() {
            let (p, h) = stop_pose(*id).unwrap_or((*p, *h));
            let d = (p - self.bus_at).truncate().length();
            if d < reach && k < 2 {
                out.push((
                    (1u64 << 40) + p.x.to_bits().rotate_left(7) ^ p.y.to_bits() ^ h.to_bits(),
                    p,
                    h,
                    "busstop",
                    name.clone(),
                ));
            }
        }
        out
    }

    pub fn toggle_map(&mut self) {
        self.city.open = !self.city.open;
        if self.city.open {
            self.city.follow = true;
            if self.city.mpp <= 0.0 {
                // For repeatable offscreen GPS comparisons at a chosen zoom.
                self.city.mpp = omsi_cfg::flags::NAV_MAP_MPP
                    .number::<f64>()
                    .filter(|v| v.is_finite() && (0.25..=20.0).contains(v))
                    .unwrap_or(2.5);
            }
        }
        self.city.drag = None;
    }

    /// Offscreen pictures (`OMSI_NAV_LINES=number[;direction;HH:MM]`): the lines panel on a line, its
    /// timetable and a departure picked.
    pub fn debug_lines(&mut self, spec: &str) {
        let mut it = spec.split(';');
        let n = it.next().unwrap_or("");
        let Some(i) = self
            .surface
            .as_ref()
            .and_then(|s| s.lines().iter().position(|l| l.number == n))
        else {
            return;
        };
        let st = &mut self.city.lines;
        (st.open, st.selected, st.focus, st.fit, st.version) =
            (true, vec![i], Some(i), true, st.version + 1);
        if let Some(d) = it.next() {
            (st.sheet, st.jump) = (true, true);
            st.route.insert(i, d.parse().unwrap_or(0));
            if let Some((h, m)) = it.next().and_then(|t| t.split_once(':')) {
                st.dep = Some(crate::line_times::DepKey {
                    t: h.parse::<u32>().unwrap_or(0) * 3600 + m.parse::<u32>().unwrap_or(0) * 60,
                    trip: None,
                    profile: None,
                });
            }
        }
    }

    pub fn map_open(&self) -> bool {
        self.city.open
    }

    /// Where the small navigator is on the screen, when it is shown.
    pub fn screen_rect(&self) -> Option<[f32; 4]> {
        let r = self.panel_rect;
        (self.enabled && !self.city.open && r[2] > r[0]).then_some(r)
    }

    /// The point (physical pixels) is on the small navigator.
    pub fn over_panel(&self, x: f32, y: f32) -> bool {
        let r = self.panel_rect;
        self.enabled && x >= r[0] && y >= r[1] && x < r[2] && y < r[3]
    }

    /// Where the small navigator (`size`) goes on a screen of `screen`: its corner, the top
    /// middle with the on-screen controls (`touch`), below the information bar (`info`) where
    /// they would lie over each other - unless it was dragged somewhere, which it keeps
    /// (kept inside the window), by the mouse or a finger.
    pub fn panel_origin(
        &mut self,
        screen: (f32, f32),
        size: (f32, f32),
        touch: bool,
        info: Option<[f32; 4]>,
    ) -> (f32, f32) {
        let ((sw, sh), (pw, ph)) = (screen, size);
        let margin = (sh * 0.018).max(10.0).round();
        // (with the on-screen controls the corners are theirs: the top middle)
        let right = self.corner.contains("right");
        let top = self.corner.contains("top") || touch;
        // ("top-center": a phone's, between its on-screen buttons)
        let x0 = if self.corner.contains("center") || touch {
            ((sw - pw) * 0.5).round()
        } else if right {
            sw - margin - pw
        } else {
            margin
        };
        let y0 = if top { margin } else { sh - margin - ph };
        // (below the information bar where they would lie over each other: in its rows on a
        // phone it took the navigator's place in the top middle, #1164)
        // (a bar along the bottom, the other way round: a navigator in a bottom corner stands
        // above it)
        let y0 = match info {
            Some(i) if x0 < i[2] && x0 + pw > i[0] => {
                if i[1] < sh * 0.5 {
                    if top {
                        y0.max((i[3] + margin * 0.5).round())
                    } else {
                        y0
                    }
                } else if top {
                    y0
                } else {
                    y0.min((i[1] - margin * 0.5 - ph).round())
                }
            }
            _ => y0,
        };
        // (dragged somewhere else: there - on a phone too, where it had to stay in the
        // middle it covered, #1138)
        let room = [(sw - pw).max(0.0), (sh - ph).max(0.0)];
        self.panel_room = room;
        match self.at {
            Some(a) => (
                (a[0].clamp(0.0, 1.0) * room[0]).round(),
                (a[1].clamp(0.0, 1.0) * room[1]).round(),
            ),
            None => (x0, y0),
        }
    }

    /// The mouse button went down on the small navigator: a click opens the city map, a
    /// drag moves the navigator (see [`Navigator::panel_move`]).
    pub fn panel_press(&mut self, x: f32, y: f32) {
        self.panel_drag = Some((
            [x, y],
            [self.panel_rect[0] - self.origin_x, self.panel_rect[1]],
            false,
        ));
    }

    /// The cursor moved with the button held on the navigator: past a few pixels it follows
    /// the cursor. True while it is being dragged.
    pub fn panel_move(&mut self, x: f32, y: f32) -> bool {
        let Some((from, rect, moved)) = self.panel_drag.as_mut() else {
            return false;
        };
        let (dx, dy) = (x - from[0], y - from[1]);
        if !*moved && dx.hypot(dy) < 6.0 {
            return false;
        }
        *moved = true;
        let room = self.panel_room;
        let share = |p: f32, r: f32| {
            if r > 0.0 {
                (p / r).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        self.at = Some([share(rect[0] + dx, room[0]), share(rect[1] + dy, room[1])]);
        true
    }

    /// The button came up after a press on the navigator: Some(true) when it was dragged
    /// (the place is then the setting's, [`Navigator::placement`]), Some(false) for a click.
    pub fn panel_release(&mut self) -> Option<bool> {
        self.panel_drag.take().map(|d| d.2)
    }

    /// The setting `navigator_corner` for where the navigator is now.
    pub fn placement(&self) -> String {
        match self.at {
            Some(a) => format!("at {:.3},{:.3}", a[0], a[1]),
            None => self.corner.clone(),
        }
    }

    fn map_hit(&self, x: f32, y: f32) -> bool {
        let r = self.city.rect;
        x >= r[0] && y >= r[1] && x < r[2] && y < r[3]
    }

    /// A click on the lines panel.
    fn lines_click(&mut self, h: LinesHit) {
        let n = self.surface.as_ref().map_or(0, |s| s.lines().len());
        let shown = if matches!(h, LinesHit::All) {
            self.shown_lines()
        } else {
            Vec::new()
        };
        let st = &mut self.city.lines;
        // (a click on a line leaves the list where it is: the player is scrolled down it)
        if matches!(h, LinesHit::Toggle(_) | LinesHit::Focus(_)) {
            (st.dep, st.jump) = (None, true);
        }
        match h {
            LinesHit::Sheet => {
                (st.sheet, st.dep, st.jump, st.scroll) = (!st.sheet, None, true, 0.0)
            }
            LinesHit::Day(d) => (st.day, st.dep, st.jump, st.scroll) = (Some(d), None, true, 0.0),
            LinesHit::Dep(k) => (st.dep, st.scroll) = (Some(k), 0.0),
            LinesHit::Back => (st.dep, st.jump, st.scroll) = (None, true, 0.0),
            LinesHit::All => {
                if shown.iter().all(|i| st.selected.contains(i)) {
                    st.selected.retain(|i| !shown.contains(i));
                } else {
                    let new: Vec<usize> = shown
                        .iter()
                        .copied()
                        .filter(|i| !st.selected.contains(i))
                        .collect();
                    st.selected.extend(&new);
                }
                st.focus = st.focus.filter(|f| st.selected.contains(f));
                st.version += 1;
            }
            LinesHit::Stop(id) => {
                if let Some(p) = self.stop_pos.get(&id) {
                    self.city.center = p.truncate();
                    self.city.follow = false;
                    self.city.mpp = self.city.mpp.min(3.0);
                }
            }
            LinesHit::Modal(k) => (st.modal, st.filter, st.scroll) = (k, 0, 0.0),
            LinesHit::Filter(k) => {
                st.filter = k;
                st.scroll = 0.0;
            }
            LinesHit::Toggle(i) if i < n => {
                if let Some(p) = st.selected.iter().position(|&x| x == i) {
                    st.selected.remove(p);
                    if st.focus == Some(i) {
                        st.focus = None;
                    }
                } else {
                    st.selected.push(i);
                    st.fit = true;
                }
                st.version += 1;
            }
            // a click on the name ticks the line (both its routes are drawn); a double click opens it
            LinesHit::Focus(i) if i < n => {
                let double = st
                    .last_click
                    .is_some_and(|(j, t)| j == i && self.time - t < 0.45);
                st.last_click = Some((i, self.time));
                if double {
                    if !st.selected.contains(&i) {
                        st.selected.push(i);
                    }
                    st.focus = Some(i);
                } else if let Some(p) = st.selected.iter().position(|&x| x == i) {
                    st.selected.remove(p);
                    if st.focus == Some(i) {
                        st.focus = None;
                    }
                } else {
                    st.selected.push(i);
                }
                st.fit = true;
                st.version += 1;
            }
            LinesHit::RoutePick(r) => {
                (st.dep, st.jump, st.scroll) = (None, true, 0.0);
                if let Some(i) = st.focus {
                    st.route.insert(i, r);
                    st.version += 1;
                    st.fit = true;
                }
            }
            _ => {}
        }
    }

    /// The lines panel on the right of the city map (after the package's `lines.lin`): the data of
    /// the line in focus above, the list of the map's lines below with a tick box each, filters
    /// and a scroll. `on_line` is how many vehicles each ticked line has on the road now.
    #[allow(clippy::too_many_arguments)]
    fn lines_panel(
        &mut self,
        ui: &mut Painter,
        w: f32,
        h: f32,
        s: f32,
        head_h: f32,
        lt: &LinesText,
        on_line: &HashMap<usize, usize>,
    ) {
        let Some(sf) = self.surface.clone() else {
            return;
        };
        let lines = sf.lines();
        let (x0, pw) = lines_panel_span(w, s);
        let top = head_h + 12.0 * s;
        let focus = self.city.lines.focus.filter(|&i| i < lines.len());
        let card_h = if focus.is_some() { 178.0 * s } else { 84.0 * s };
        let ink = v5::PANEL.alpha(0.95);
        let pad = 14.0 * s;
        let mut hits: Vec<(Rect, LinesHit)> = Vec::new();
        let num = |v: f32, dec: usize| {
            let t = format!("{v:.dec$}");
            if decimal_comma() {
                t.replace('.', ",")
            } else {
                t
            }
        };

        // --- the data card
        let card = Rect::new(x0, top, pw, card_h);
        ui.rounded(card, 4.0 * s, ink);
        ui.rounded_border(card, 4.0 * s, 1.0, v5::LINE);
        ui.text(
            &mut self.atlas,
            &self.fonts,
            lt.line_data,
            10.5 * s,
            Weight::Bold,
            Vec2::new(x0 + pad + 4.0 * s, top + 20.0 * s),
            Align::Left,
            v5::MUTED,
        );
        if focus.is_none() {
            ui.icon(
                &mut self.atlas,
                "touch_app",
                Vec2::new(x0 + pad + 14.0 * s, top + 54.0 * s),
                26.0 * s,
                omsi_ui::theme::accent(),
            );
            for (k, t) in lt.hint.iter().enumerate() {
                ui.text(
                    &mut self.atlas,
                    &self.fonts,
                    &self.fonts.fit(
                        t,
                        13.0 * s,
                        if k == 0 { Weight::Bold } else { Weight::Medium },
                        pw - 2.0 * pad - 40.0 * s,
                    ),
                    13.0 * s,
                    if k == 0 { Weight::Bold } else { Weight::Medium },
                    Vec2::new(x0 + pad + 36.0 * s, top + (50.0 + 18.0 * k as f32) * s),
                    Align::Left,
                    if k == 0 { TEXT } else { TEXT_DIM },
                );
            }
        }
        if let Some(i) = focus {
            let l = &lines[i];
            // (the button shows until the timetable has been read and the line is found not to have one; opening
            // a line reads nothing)
            if sf
                .times_read(self.date)
                .is_none_or(|t| t.line(&l.number).is_some())
            {
                let tw = self.fonts.width(lt.timetable, 11.5 * s, Weight::Bold) + 40.0 * s;
                let rr = Rect::new(x0 + pw - pad - tw, top + 8.0 * s, tw, 22.0 * s);
                chip(ui, rr, 4.0 * s, self.city.lines.sheet);
                ui.icon(
                    &mut self.atlas,
                    "schedule",
                    Vec2::new(rr.x + 15.0 * s, rr.center().y),
                    14.0 * s,
                    TEXT,
                );
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    lt.timetable,
                    11.5 * s,
                    Weight::Bold,
                    Rect::new(rr.x + 24.0 * s, rr.y, rr.w - 26.0 * s, rr.h),
                    Align::Center,
                    TEXT,
                );
                hits.push((rr, LinesHit::Sheet));
            }
            let ri = self
                .city
                .lines
                .route
                .get(&i)
                .copied()
                .unwrap_or(0)
                .min(l.routes.len().saturating_sub(1));
            let r = &l.routes[ri];
            ui.rect(
                Rect::new(x0, top + 6.0 * s, 4.0 * s, card_h - 12.0 * s),
                line_color(i),
            );
            let nw = ui.text(
                &mut self.atlas,
                &self.fonts,
                &l.number,
                18.0 * s,
                Weight::Bold,
                Vec2::new(x0 + pad + 4.0 * s, top + 48.0 * s),
                Align::Left,
                TEXT,
            );
            let name = self.fonts.fit(
                &l.name,
                16.0 * s,
                Weight::Bold,
                pw - 2.0 * pad - nw - 18.0 * s,
            );
            ui.text(
                &mut self.atlas,
                &self.fonts,
                &name,
                16.0 * s,
                Weight::Bold,
                Vec2::new(x0 + pad + 4.0 * s + nw + 10.0 * s, top + 48.0 * s),
                Align::Left,
                TEXT,
            );
            let itin = self.fonts.fit(
                &r.itinerary,
                11.5 * s,
                Weight::Medium,
                pw - 2.0 * pad - 4.0 * s,
            );
            ui.text(
                &mut self.atlas,
                &self.fonts,
                &itin,
                11.5 * s,
                Weight::Medium,
                Vec2::new(x0 + pad + 4.0 * s, top + 68.0 * s),
                Align::Left,
                TEXT_DIM,
            );
            // length, stops, trip time
            let colw = (pw - 2.0 * pad) / 3.0;
            let stats = [
                (lt.length, format!("{} km", num(r.km, 1))),
                (lt.stops, format!("{}", r.stops_n)),
                (lt.trip, format!("{} {}", r.minutes, tr(lt.minutes))),
            ];
            for (k, (label, value)) in stats.iter().enumerate() {
                let sx = x0 + pad + 4.0 * s + colw * k as f32;
                ui.text(
                    &mut self.atlas,
                    &self.fonts,
                    &self
                        .fonts
                        .fit(label, 10.0 * s, Weight::Medium, colw - 6.0 * s),
                    10.0 * s,
                    Weight::Medium,
                    Vec2::new(sx, top + 92.0 * s),
                    Align::Left,
                    TEXT_DIM,
                );
                ui.text(
                    &mut self.atlas,
                    &self.fonts,
                    value,
                    17.0 * s,
                    Weight::Bold,
                    Vec2::new(sx, top + 114.0 * s),
                    Align::Left,
                    TEXT,
                );
            }
            // the route shape is translated before the pieces are joined: the painter looks up whole strings only
            let shape = tr(if r.circular { lt.circular } else { lt.out_back });
            let meta = [r.days.as_str(), r.model.as_str(), shape.as_ref()]
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join("  ·  ");
            let meta = self
                .fonts
                .fit(&meta, 11.0 * s, Weight::Medium, pw - 2.0 * pad - 4.0 * s);
            ui.text(
                &mut self.atlas,
                &self.fonts,
                &meta,
                11.0 * s,
                Weight::Medium,
                Vec2::new(x0 + pad + 4.0 * s, top + 138.0 * s),
                Align::Left,
                TEXT_DIM,
            );
            let n = on_line.get(&i).copied().unwrap_or(0);
            ui.text(
                &mut self.atlas,
                &self.fonts,
                &format!("{}: {}", tr(lt.vehicles), n),
                12.0 * s,
                Weight::Bold,
                Vec2::new(x0 + pad + 4.0 * s, top + 162.0 * s),
                Align::Left,
                if n > 0 { v5::OK } else { TEXT_DIM },
            );
            if l.routes.len() > 1 {
                let mut tx = x0 + pw - pad;
                for k in (0..l.routes.len()).rev() {
                    let label = format!("{} {}", tr(lt.route), k + 1);
                    let tw = self.fonts.width(&label, 11.5 * s, Weight::Bold) + 16.0 * s;
                    let rr = Rect::new(tx - tw, top + 148.0 * s, tw, 22.0 * s);
                    chip(ui, rr, 4.0 * s, k == ri);
                    ui.text_in(
                        &mut self.atlas,
                        &self.fonts,
                        &label,
                        11.5 * s,
                        Weight::Bold,
                        rr,
                        Align::Center,
                        TEXT,
                    );
                    hits.push((rr, LinesHit::RoutePick(k)));
                    tx -= tw + 6.0 * s;
                }
            }
        }

        // --- the list of the map's lines
        let ly = top + card_h + 10.0 * s;
        let panel_h = (h - ly - 12.0 * s).max(60.0 * s);
        let list_panel = Rect::new(x0, ly, pw, panel_h);
        ui.rounded(list_panel, 4.0 * s, ink);
        ui.rounded_border(list_panel, 4.0 * s, 1.0, v5::LINE);
        if let Some(i) = focus.filter(|_| self.city.lines.sheet) {
            // the timetable is read only now that it is asked for
            let Some(timetable) = sf.times(&self.chrono, self.date) else {
                ui.text(
                    &mut self.atlas,
                    &self.fonts,
                    lt.loading,
                    13.0 * s,
                    Weight::Medium,
                    Vec2::new(x0 + pad, ly + 24.0 * s),
                    Align::Left,
                    TEXT_DIM,
                );
                self.city.lines.panel = Some(Rect::new(x0, top, pw, ly + panel_h - top));
                self.city.lines.list = None;
                self.city.lines.hits = hits;
                return;
            };
            if timetable.line(&lines[i].number).is_none() {
                // read, and this line has none: back to the list
                self.city.lines.sheet = false;
            } else {
                let rows = self.sheet_panel(ui, &sf, &timetable, i, list_panel, s, lt, &mut hits);
                self.city.lines.panel = Some(Rect::new(x0, top, pw, ly + panel_h - top));
                self.city.lines.list = Some(rows);
                self.city.lines.hits = hits;
                return;
            }
        }
        ui.text(
            &mut self.atlas,
            &self.fonts,
            lt.map_lines,
            10.5 * s,
            Weight::Bold,
            Vec2::new(x0 + pad, ly + 20.0 * s),
            Align::Left,
            v5::MUTED,
        );
        let dated: Vec<usize> = (0..lines.len())
            .filter(|&i| sf.line_on(i, self.date))
            .collect();
        let mut present = [false; 4];
        for &i in &dated {
            present[(lines[i].modal as usize).min(3)] = true;
        }
        if !present[self.city.lines.modal.min(3) as usize] {
            self.city.lines.modal = present.iter().position(|&p| p).unwrap_or(0) as u8;
        }
        let shown = self.shown_lines();
        ui.text(
            &mut self.atlas,
            &self.fonts,
            &format!("{} {}", shown.len(), tr(lt.count)),
            10.5 * s,
            Weight::Medium,
            Vec2::new(
                x0 + pad + self.fonts.width(lt.map_lines, 10.5 * s, Weight::Bold) + 10.0 * s,
                ly + 20.0 * s,
            ),
            Align::Left,
            TEXT_DIM,
        );
        let all_on =
            !shown.is_empty() && shown.iter().all(|i| self.city.lines.selected.contains(i));
        let label = if all_on { lt.tick_none } else { lt.tick_all };
        let bw = self.fonts.width(label, 11.5 * s, Weight::Bold) + 20.0 * s;
        let br = Rect::new(x0 + pw - pad - bw, ly + 6.0 * s, bw, 22.0 * s);
        chip(ui, br, 4.0 * s, all_on);
        ui.text_in(
            &mut self.atlas,
            &self.fonts,
            label,
            11.5 * s,
            Weight::Bold,
            br,
            Align::Center,
            TEXT,
        );
        hits.push((br, LinesHit::All));
        let mut ty = ly + 30.0 * s;
        // a tab for each kind of vehicle the map has lines of
        if present.iter().filter(|&&p| p).count() > 1 {
            let mut tx = x0 + pad;
            for k in (0..4u8).filter(|&k| present[k as usize]) {
                let label = lt.modals[k as usize];
                let tw = self.fonts.width(label, 12.0 * s, Weight::Bold) + 20.0 * s;
                let rr = Rect::new(tx, ty, tw, 24.0 * s);
                chip(ui, rr, 4.0 * s, self.city.lines.modal == k);
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    label,
                    12.0 * s,
                    Weight::Bold,
                    rr,
                    Align::Center,
                    TEXT,
                );
                hits.push((rr, LinesHit::Modal(k)));
                tx += tw + 6.0 * s;
            }
            ty += 30.0 * s;
        }
        // the groups of the buses
        if self.city.lines.modal == crate::map_surface::MODAL_BUS {
            let mut tx = x0 + pad;
            for (k, label) in [
                (0u8, lt.all),
                (GROUP_BRT_TAB, lt.brt),
                (GROUP_EXPRESS_TAB, lt.express),
                (GROUP_FEEDER_TAB, lt.feeders),
            ] {
                let tw = self.fonts.width(label, 11.5 * s, Weight::Bold) + 16.0 * s;
                let rr = Rect::new(tx, ty, tw, 22.0 * s);
                chip(ui, rr, 4.0 * s, self.city.lines.filter == k);
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    label,
                    11.5 * s,
                    Weight::Bold,
                    rr,
                    Align::Center,
                    TEXT,
                );
                hits.push((rr, LinesHit::Filter(k)));
                tx += tw + 6.0 * s;
            }
            ty += 28.0 * s;
        }
        let row_h = 27.0 * s;
        let rows = Rect::new(
            x0,
            ty + 2.0 * s,
            pw,
            (list_panel.bottom() - ty - 8.0 * s).max(row_h),
        );
        let vis = ((rows.h / row_h).floor() as usize).max(1);
        let max_scroll = shown.len().saturating_sub(vis) as f32;
        self.city.lines.scroll = self.city.lines.scroll.clamp(0.0, max_scroll);
        let first = self.city.lines.scroll.round() as usize;
        for (n, &i) in shown.iter().skip(first).take(vis).enumerate() {
            let l = &lines[i];
            let ry = rows.y + n as f32 * row_h;
            let row = Rect::new(x0 + 4.0 * s, ry, pw - 14.0 * s, row_h);
            let ticked = self.city.lines.selected.contains(&i);
            if self.city.lines.focus == Some(i) && ticked {
                ui.rounded(row, 4.0 * s, Color::WHITE.alpha(0.07));
            }
            let cb = Rect::new(x0 + pad, ry + (row_h - 14.0 * s) * 0.5, 14.0 * s, 14.0 * s);
            if ticked {
                ui.rounded(cb, 3.0 * s, line_color(i));
                ui.icon(
                    &mut self.atlas,
                    "check",
                    cb.center(),
                    12.0 * s,
                    Color::rgba(10, 10, 10, 1.0),
                );
            } else {
                ui.rounded_border(cb, 3.0 * s, 1.2, Color::WHITE.alpha(0.35));
            }
            hits.push((
                Rect::new(x0 + 4.0 * s, ry, 34.0 * s, row_h),
                LinesHit::Toggle(i),
            ));
            let nx = x0 + pad + 24.0 * s;
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &l.number,
                13.0 * s,
                Weight::Bold,
                Rect::new(nx, ry, 44.0 * s, row_h),
                Align::Left,
                TEXT,
            );
            let badge = match l.group {
                GROUP_BRT => Some(("BRT", Color::hex(0x2e8b45))),
                GROUP_EXPRESS => Some(("EXP", Color::hex(0x6b7480))),
                _ => None,
            };
            let name_w = pw - 2.0 * pad - 24.0 * s - 44.0 * s - 44.0 * s;
            let name = self.fonts.fit(&l.name, 12.0 * s, Weight::Medium, name_w);
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &name,
                12.0 * s,
                Weight::Medium,
                Rect::new(nx + 44.0 * s, ry, name_w, row_h),
                Align::Left,
                TEXT,
            );
            if let Some((label, c)) = badge {
                let bw = 32.0 * s;
                let br = Rect::new(
                    x0 + pw - pad - 6.0 * s - bw,
                    ry + (row_h - 15.0 * s) * 0.5,
                    bw,
                    15.0 * s,
                );
                ui.rounded(br, 3.0 * s, c);
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    label,
                    9.5 * s,
                    Weight::Bold,
                    br,
                    Align::Center,
                    Color::WHITE,
                );
            }
            hits.push((
                Rect::new(nx, ry, pw - 14.0 * s - (nx - x0), row_h),
                LinesHit::Focus(i),
            ));
        }
        // a thin scroll bar, when the list is longer than its room
        if max_scroll > 0.0 {
            let track = Rect::new(x0 + pw - 7.0 * s, rows.y, 3.0 * s, vis as f32 * row_h);
            ui.rounded(track, 1.5 * s, Color::WHITE.alpha(0.08));
            let (th, tp) = (
                track.h * vis as f32 / shown.len() as f32,
                self.city.lines.scroll / max_scroll,
            );
            ui.rounded(
                Rect::new(
                    track.x,
                    track.y + (track.h - th) * tp,
                    track.w,
                    th.max(14.0 * s),
                ),
                1.5 * s,
                Color::WHITE.alpha(0.35),
            );
        }
        self.city.lines.panel = Some(Rect::new(x0, top, pw, ly + panel_h - top));
        self.city.lines.list = Some(rows);
        self.city.lines.hits = hits;
    }

    /// The lines the list shows: those that hold on the session's date, of the kind of vehicle chosen and (buses)
    /// of the group chosen.
    fn shown_lines(&self) -> Vec<usize> {
        let Some(sf) = self.surface.as_ref() else {
            return Vec::new();
        };
        let (modal, filter) = (self.city.lines.modal, self.city.lines.filter);
        (0..sf.lines().len())
            .filter(|&i| {
                sf.line_on(i, self.date)
                    && sf.lines()[i].modal == modal
                    && (modal != crate::map_surface::MODAL_BUS
                        || filter == 0
                        || sf.lines()[i].group == filter)
            })
            .collect()
    }

    /// The day group the timetable shows: the picked one, else today's (Monday to Friday, Saturday, Sunday).
    fn sheet_day(&self) -> usize {
        self.city.lines.day.unwrap_or(if self.day_clock.1 <= 4 {
            0
        } else {
            self.day_clock.1 as usize
        })
    }

    /// The direction (in the sheet) of the route chosen for line `i`: route 1, route 2.
    fn sheet_dir(
        &self,
        sheet: &crate::line_times::Sheet,
        i: usize,
        sf: &crate::map_surface::MapSurface,
    ) -> usize {
        let order = sheet.order(
            sf.lines()[i]
                .routes
                .first()
                .map_or("", |r| r.itinerary.as_str()),
        );
        let r = self.city.lines.route.get(&i).copied().unwrap_or(0);
        order.get(r).or(order.last()).copied().unwrap_or(0)
    }

    /// The timetable of the line in focus in place of the list: its directions and day groups, the
    /// departures by the hour and - once one is picked - when its bus is at each stop (the same times are
    /// written on the map). Returns the scrolling area.
    #[allow(clippy::too_many_arguments)]
    fn sheet_panel(
        &mut self,
        ui: &mut Painter,
        sf: &crate::map_surface::MapSurface,
        timetable: &crate::line_times::LineTimes,
        i: usize,
        area: Rect,
        s: f32,
        lt: &LinesText,
        hits: &mut Vec<(Rect, LinesHit)>,
    ) -> Rect {
        use crate::line_times::clock;
        let sheet = timetable
            .line(&sf.lines()[i].number)
            .expect("the caller checked");
        let (x0, y0, pw, pad) = (area.x, area.y, area.w, 14.0 * s);
        let (day, dir) = (self.sheet_day(), self.sheet_dir(sheet, i, sf));
        let deps = sheet.departures(dir, day);
        let pick = self
            .city
            .lines
            .dep
            .and_then(|k| deps.iter().copied().find(|d| d.is(&k)));
        ui.text(
            &mut self.atlas,
            &self.fonts,
            &format!("{}  ·  {}", tr(lt.timetable), tr(lt.towards)).to_uppercase(),
            11.5 * s,
            Weight::Bold,
            Vec2::new(x0 + pad, y0 + 20.0 * s),
            Align::Left,
            v5::MUTED,
        );
        if pick.is_some() {
            let rr = Rect::new(x0 + pw - pad - 34.0 * s, y0 + 6.0 * s, 34.0 * s, 22.0 * s);
            chip(ui, rr, 4.0 * s, false);
            ui.icon(&mut self.atlas, "arrow_back", rr.center(), 16.0 * s, TEXT);
            hits.push((rr, LinesHit::Back));
        }
        // the route (the card's buttons choose it) and where it goes, then the day groups
        let r = self.city.lines.route.get(&i).copied().unwrap_or(0);
        let head = self.fonts.fit(
            &format!("{} {}  ·  {}", tr(lt.route), r + 1, sheet.dirs[dir].to),
            14.5 * s,
            Weight::Bold,
            pw - 2.0 * pad,
        );
        ui.text(
            &mut self.atlas,
            &self.fonts,
            &head,
            14.5 * s,
            Weight::Bold,
            Vec2::new(x0 + pad, y0 + 48.0 * s),
            Align::Left,
            TEXT,
        );
        let mut cx = x0 + pad;
        for (label, d) in lt.days.iter().zip([0usize, 5, 6]) {
            let cw = self.fonts.width(label, 13.0 * s, Weight::Bold) + 20.0 * s;
            let rr = Rect::new(cx, y0 + 60.0 * s, cw, 26.0 * s);
            chip(ui, rr, 4.0 * s, d == day);
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                label,
                13.0 * s,
                Weight::Bold,
                rr,
                Align::Center,
                TEXT,
            );
            hits.push((rr, LinesHit::Day(d)));
            cx += cw + 6.0 * s;
        }
        let rows = Rect::new(
            x0,
            y0 + 100.0 * s,
            pw,
            (area.bottom() - y0 - 106.0 * s - if pick.is_some() { 20.0 * s } else { 0.0 })
                .max(40.0 * s),
        );
        let scroll_bar =
            |ui: &mut Painter, scroll: f32, max: f32, vis: usize, total: usize, row_h: f32| {
                if max > 0.0 {
                    let track = Rect::new(x0 + pw - 7.0 * s, rows.y, 3.0 * s, vis as f32 * row_h);
                    ui.rounded(track, 1.5 * s, Color::WHITE.alpha(0.08));
                    let (th, tp) = (track.h * vis as f32 / total as f32, scroll / max);
                    ui.rounded(
                        Rect::new(
                            track.x,
                            track.y + (track.h - th) * tp,
                            track.w,
                            th.max(14.0 * s),
                        ),
                        1.5 * s,
                        Color::WHITE.alpha(0.35),
                    );
                }
            };
        if let Some(d) = pick {
            // the stops the bus calls at, with the time it is there
            let stops: Vec<_> = sheet.stops(d).into_iter().filter(|x| x.stops).collect();
            let row_h = 31.0 * s;
            let vis = ((rows.h / row_h).floor() as usize).max(1);
            let max = stops.len().saturating_sub(vis) as f32;
            self.city.lines.scroll = self.city.lines.scroll.clamp(0.0, max);
            for (n, (k, st)) in stops
                .iter()
                .enumerate()
                .skip(self.city.lines.scroll.round() as usize)
                .take(vis)
                .enumerate()
            {
                let rr = Rect::new(
                    x0 + 4.0 * s,
                    rows.y + n as f32 * row_h,
                    pw - 14.0 * s,
                    row_h,
                );
                let w = if k == 0 || k + 1 == stops.len() {
                    Weight::Bold
                } else {
                    Weight::Medium
                };
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    &clock(st.at),
                    14.5 * s,
                    Weight::Bold,
                    Rect::new(x0 + pad, rr.y, 58.0 * s, row_h),
                    Align::Left,
                    TEXT,
                );
                let nw = pw - 2.0 * pad - 64.0 * s;
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    &self.fonts.fit(st.name, 14.0 * s, w, nw),
                    14.0 * s,
                    w,
                    Rect::new(x0 + pad + 62.0 * s, rr.y, nw, row_h),
                    Align::Left,
                    TEXT,
                );
                hits.push((rr, LinesHit::Stop(st.id)));
            }
            scroll_bar(ui, self.city.lines.scroll, max, vis, stops.len(), row_h);
            ui.text(
                &mut self.atlas,
                &self.fonts,
                &self
                    .fonts
                    .fit(lt.predicted, 11.0 * s, Weight::Medium, pw - 2.0 * pad),
                11.0 * s,
                Weight::Medium,
                Vec2::new(x0 + pad, area.bottom() - 10.0 * s),
                Align::Left,
                TEXT_DIM,
            );
        } else {
            // the departures by the hour; the next one after the game's clock is lit
            let per = (((pw - 2.0 * pad - 44.0 * s) / (43.0 * s)).floor() as usize).max(1);
            let mut lines: Vec<(bool, i64, Vec<&crate::line_times::Dep>)> = Vec::new();
            for d in &deps {
                let h = (d.t / 3600.0) as i64;
                match lines.last_mut() {
                    Some((_, lh, v)) if *lh == h && v.len() < per => v.push(d),
                    _ => lines.push((lines.last().is_none_or(|l| l.1 != h), h, vec![d])),
                }
            }
            let next = deps
                .iter()
                .find(|d| d.t >= self.day_clock.0 as f32)
                .map(|d| d.t as u32);
            let row_h = 36.0 * s;
            let vis = ((rows.h / row_h).floor() as usize).max(1);
            let max = lines.len().saturating_sub(vis) as f32;
            if std::mem::take(&mut self.city.lines.jump) {
                self.city.lines.scroll = lines
                    .iter()
                    .position(|l| l.2.iter().any(|d| Some(d.t as u32) == next))
                    .unwrap_or(0) as f32;
            }
            self.city.lines.scroll = self.city.lines.scroll.clamp(0.0, max);
            if lines.is_empty() {
                ui.text_in(
                    &mut self.atlas,
                    &self.fonts,
                    "—",
                    16.0 * s,
                    Weight::Bold,
                    rows,
                    Align::Center,
                    TEXT_DIM,
                );
            }
            for (n, (first, h, v)) in lines
                .iter()
                .skip(self.city.lines.scroll.round() as usize)
                .take(vis)
                .enumerate()
            {
                let ry = rows.y + n as f32 * row_h;
                if *first {
                    ui.text_in(
                        &mut self.atlas,
                        &self.fonts,
                        &format!("{:02}", h % 24),
                        16.0 * s,
                        Weight::Bold,
                        Rect::new(x0 + pad, ry, 38.0 * s, row_h),
                        Align::Left,
                        TEXT,
                    );
                }
                for (k, d) in v.iter().enumerate() {
                    let rr = Rect::new(
                        x0 + pad + 42.0 * s + k as f32 * 43.0 * s,
                        ry + 3.0 * s,
                        39.0 * s,
                        row_h - 6.0 * s,
                    );
                    chip(ui, rr, 4.0 * s, Some(d.t as u32) == next);
                    ui.text_in(
                        &mut self.atlas,
                        &self.fonts,
                        &format!("{:02}", (d.t / 60.0).round() as i64 % 60),
                        14.5 * s,
                        Weight::Bold,
                        rr,
                        Align::Center,
                        TEXT,
                    );
                    hits.push((rr, LinesHit::Dep(d.key())));
                }
            }
            scroll_bar(ui, self.city.lines.scroll, max, vis, lines.len(), row_h);
        }
        rows
    }

    /// The departure picked, on the map: a dot and the time at each stop it calls at, as many as fit
    /// (the ends first); the stops are where the map's own stop objects stand.
    fn sheet_labels(
        &mut self,
        ui: &mut Painter,
        sf: &crate::map_surface::MapSurface,
        i: usize,
        s: f32,
        to_screen: &dyn Fn(DVec3) -> Vec2,
        view: (f32, f32),
    ) {
        let Some(timetable) = sf.times(&self.chrono, self.date) else {
            return;
        };
        let Some(sheet) = timetable.line(&sf.lines()[i].number) else {
            return;
        };
        let (day, dir) = (self.sheet_day(), self.sheet_dir(sheet, i, sf));
        let Some(d) = self
            .city
            .lines
            .dep
            .and_then(|k| sheet.departures(dir, day).into_iter().find(|d| d.is(&k)))
        else {
            return;
        };
        let stops: Vec<_> = sheet.stops(d).into_iter().filter(|x| x.stops).collect();
        let n = stops.len();
        let mut placed: Vec<Rect> = Vec::new();
        for k in [0, n.saturating_sub(1)]
            .into_iter()
            .chain(1..n.saturating_sub(1))
        {
            let Some(p) = stops.get(k).and_then(|st| self.stop_pos.get(&st.id)) else {
                continue;
            };
            let c = to_screen(*p);
            if c.x < 0.0 || c.y < 0.0 || c.x > view.0 || c.y > view.1 {
                continue;
            }
            let t = crate::line_times::clock(stops[k].at);
            let r = Rect::new(
                c.x + 8.0 * s,
                c.y - 26.0 * s,
                self.fonts.width(&t, 13.5 * s, Weight::Bold) + 14.0 * s,
                22.0 * s,
            );
            if placed.iter().any(|q| {
                r.x < q.right() + 3.0 * s
                    && q.x < r.right() + 3.0 * s
                    && r.y < q.bottom() + 3.0 * s
                    && q.y < r.bottom() + 3.0 * s
            }) {
                continue;
            }
            placed.push(r);
            ui.rounded(
                Rect::new(c.x - 5.0 * s, c.y - 5.0 * s, 10.0 * s, 10.0 * s),
                5.0 * s,
                Color::rgba(8, 8, 8, 0.9),
            );
            ui.rounded(
                Rect::new(c.x - 3.5 * s, c.y - 3.5 * s, 7.0 * s, 7.0 * s),
                3.5 * s,
                Color::WHITE,
            );
            ui.rounded(r, 4.0 * s, Color::rgba(10, 12, 16, 0.92));
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &t,
                13.5 * s,
                Weight::Bold,
                r,
                Align::Center,
                TEXT,
            );
        }
    }

    /// A mouse press while the map is open: on it, a drag or a button; outside, it closes.
    pub fn map_press(&mut self, x: f32, y: f32) {
        if !self.map_hit(x, y) {
            self.city.open = false;
            return;
        }
        let local = Vec2::new(x - self.city.rect[0], y - self.city.rect[1]);
        let hit = self
            .city
            .buttons
            .iter()
            .find(|(r, _)| r.contains(local))
            .map(|b| b.1);
        // the lines panel: its rows and tabs, or its background (never a drag of the map)
        if hit.is_none()
            && self.city.lines.open
            && self.city.lines.panel.is_some_and(|p| p.contains(local))
        {
            if let Some(h) = self
                .city
                .lines
                .hits
                .iter()
                .find(|(r, _)| r.contains(local))
                .map(|h| h.1)
            {
                self.lines_click(h);
            }
            return;
        }
        match hit {
            Some(4) => self.city.lines.open = !self.city.lines.open,
            Some(5) if self.show_surface => {
                self.show_contours = !self.show_contours;
                // (the heights are made off the frame, from the moment the terrain view is turned on)
                if let (true, Some(sf)) = (self.show_contours, self.surface.as_ref()) {
                    sf.request_contour_marks();
                }
            }
            Some(6) => self.orient = (self.orient + 1) % 3,
            Some(0) => self.city.follow = true,
            Some(1) => self.city.mpp = (self.city.mpp / 1.6).max(0.25),
            Some(2) => self.city.mpp = (self.city.mpp * 1.6).min(self.max_mpp()),
            Some(3) => self.city.open = false,
            _ => self.city.drag = Some((x, y)),
        }
    }

    /// The world point (x, y) under the window point, when the map is open and the point
    /// is on it.
    pub fn map_point(&self, x: f32, y: f32) -> Option<DVec2> {
        if !self.city.open || !self.map_hit(x, y) {
            return None;
        }
        let r = self.city.rect;
        let (w, h) = ((r[2] - r[0]) as f64, (r[3] - r[1]) as f64);
        let (lx, ly) = ((x - r[0]) as f64 - w * 0.5, (y - r[1]) as f64 - h * 0.5);
        Some(self.city.center + rot(DVec2::new(lx, -ly) * self.city.mpp, -self.city.phi))
    }

    pub fn map_release(&mut self) {
        self.city.drag = None;
    }

    pub fn map_move(&mut self, x: f32, y: f32) {
        self.city.hover = self.map_hit(x, y).then_some((x, y));
        if let Some((px, py)) = self.city.drag {
            let (dx, dy) = ((x - px) as f64, (y - py) as f64);
            if dx.abs() + dy.abs() > 0.5 {
                self.city.follow = false;
            }
            self.city.center -= rot(DVec2::new(dx, -dy) * self.city.mpp, -self.city.phi);
            self.city.drag = Some((x, y));
        }
    }

    /// The name of the street under the pointer, from the map's package: any street the package names, also where
    /// the game has no road (not while the map is dragged or the pointer is on the lines panel).
    fn hover_street(&mut self) -> Option<(Option<crate::map_surface::Street>, Option<f32>)> {
        let (x, y) = self.city.hover?;
        let r = self.city.rect;
        if self.city.drag.is_some()
            || (self.city.lines.open
                && self
                    .city
                    .lines
                    .panel
                    .is_some_and(|p| p.contains(Vec2::new(x - r[0], y - r[1]))))
        {
            return None;
        }
        let pos = self.map_point(x, y)?;
        // (the search reach follows the zoom, so the zoom is part of the key)
        let reach = (self.city.mpp * 10.0).clamp(8.0, 40.0);
        let key = (
            (pos.x / 2.0).round() as i64,
            (pos.y / 2.0).round() as i64,
            (reach * 100.0).round() as i64,
            self.show_surface && self.show_contours,
        );
        if let Some((k, found)) = &self.city.hover_cache {
            if *k == key {
                return Some(found.clone()).filter(|f| f.0.is_some() || f.1.is_some());
            }
        }
        let street = self
            .surface
            .as_ref()
            .and_then(|s| s.street_info(pos, reach));
        let alt = self
            .surface
            .as_ref()
            .filter(|_| self.show_surface && self.show_contours)
            .and_then(|s| s.altitude_at(pos));
        self.city.hover_cache = Some((key, (street.clone(), alt)));
        Some((street, alt)).filter(|f| f.0.is_some() || f.1.is_some())
    }

    /// The wheel over the map: zoom, keeping the point under the cursor where it is.
    pub fn map_wheel(&mut self, amount: f32, x: f32, y: f32) {
        let r = self.city.rect;
        // over the lines list the wheel scrolls it (anywhere else it zooms the map)
        if self.city.lines.open
            && self
                .city
                .lines
                .list
                .is_some_and(|l| l.contains(Vec2::new(x - r[0], y - r[1])))
        {
            self.city.lines.scroll -= amount * 2.0;
            return;
        }
        let (w, h) = ((r[2] - r[0]) as f64, (r[3] - r[1]) as f64);
        let (lx, ly) = ((x - r[0]) as f64 - w * 0.5, (y - r[1]) as f64 - h * 0.5);
        let before = self.city.center + rot(DVec2::new(lx, -ly) * self.city.mpp, -self.city.phi);
        let k = (1.0 - amount as f64 * 0.15).clamp(0.6, 1.6);
        self.city.mpp = (self.city.mpp * k).clamp(0.25, self.max_mpp());
        let after = self.city.center + rot(DVec2::new(lx, -ly) * self.city.mpp, -self.city.phi);
        self.city.center += before - after;
        if amount.abs() > 0.0 && (lx.abs() > 40.0 || ly.abs() > 40.0) {
            self.city.follow = false;
        }
    }

    fn max_mpp(&self) -> f64 {
        let (lo, hi) = self.city.extent;
        let span = (hi - lo).max_element().max(2000.0);
        let r = self.city.rect;
        span / ((r[2] - r[0]).max(200.0) as f64) * 1.2
    }

    /// Draw the city map and lay it over the picture.
    pub(super) fn city(&mut self, renderer: &Renderer, scene: &mut Scene, f: &NavFrame) {
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
        let phi = super::geometry::turn_towards(self.city.phi, target, f.dt as f64);
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

/// The city map's header, in points.
const HEAD_H: f32 = 52.0;
/// A card's eyebrow and each of its rows, in points.
const CARD_HEAD: f32 = 30.0;
const CARD_ROW: f32 = 32.0;

/// The height of a card of `rows` rows.
fn card_h(rows: usize, s: f32) -> f32 {
    (CARD_HEAD + CARD_ROW * rows as f32) * s
}

/// A time of day (seconds) as the map writes it.
fn clock_of(t: f64) -> String {
    format!(
        "{:02}:{:02}",
        (t / 3600.0) as i32 % 24,
        ((t % 3600.0) / 60.0) as i32
    )
}

/// A distance (metres) as the map writes it.
fn distance_text(d: f64) -> String {
    if d >= 1000.0 {
        format!("{:.1} km", d / 1000.0)
    } else {
        format!("{:.0} m", d)
    }
}

/// The header's colour: the in-game windows' panel, solid.
const WINDOW_BAR: Color = Color::hex(0x202124);

/// A button of the map in the new look: dark and outlined, or (`on`) outlined in the accent
/// on its faint shade. Returns the ink for what is written on it.
fn outlined(ui: &mut Painter, r: Rect, s: f32, on: bool) -> Color {
    ui.rounded(
        r,
        6.0 * s,
        if on {
            omsi_ui::theme::accent_soft()
        } else {
            v5::CARD
        },
    );
    ui.rounded_border(
        r,
        6.0 * s,
        1.0,
        if on {
            omsi_ui::theme::accent()
        } else {
            v5::LINE
        },
    );
    if on {
        omsi_ui::theme::accent()
    } else {
        v5::FG
    }
}

/// A tab or a toggle of the lines panel: the field's grey, or (`on`) the accent's shade
/// outlined in the accent.
fn chip(ui: &mut Painter, r: Rect, radius: f32, on: bool) {
    ui.rounded(
        r,
        radius,
        if on {
            omsi_ui::theme::accent_soft()
        } else {
            v5::FIELD
        },
    );
    if on {
        ui.rounded_border(r, radius, 1.0, omsi_ui::theme::accent());
    }
}

/// Small letter-spaced capitals (an eyebrow) from `x`, centred on `cy`, written as `text` is
/// (the caller translates it). Returns their width.
#[allow(clippy::too_many_arguments)]
fn spaced(
    ui: &mut Painter,
    atlas: &mut Atlas,
    fonts: &Fonts,
    text: &str,
    px: f32,
    x: f32,
    cy: f32,
    c: Color,
    tracking: f32,
) -> f32 {
    let base = cy + fonts.cap_height(px, Weight::Bold) * 0.5;
    let mut at = x;
    for (k, ch) in text.chars().enumerate() {
        if k > 0 {
            at += tracking;
        }
        let one = ch.to_string();
        if !ch.is_whitespace() {
            ui.text_as_is(
                atlas,
                fonts,
                &one,
                px,
                Weight::Bold,
                Vec2::new(at, base),
                Align::Left,
                c,
            );
        }
        at += fonts.width_as_is(&one, px, Weight::Bold);
    }
    at - x
}

impl Navigator {
    /// The rows of the card that says where the bus is (from the map's package): the
    /// municipality and the street with its height; none where the package does not say.
    fn here_rows(&self, lt: &LinesText) -> Vec<(&'static str, String)> {
        let mut rows = Vec::new();
        if !self.show_surface || !self.surface.as_ref().is_some_and(|sf| sf.has_location()) {
            return rows;
        }
        if let Some(c) = self.loc_city.clone() {
            rows.push((lt.city, c));
        }
        if let Some(st) = self.loc_street.clone() {
            let alt = self.loc_alt.map_or(String::new(), |a| {
                format!("  ·  {} {:.0} m", tr(lt.elev), a)
            });
            rows.push((lt.street, format!("{st}{alt}")));
        }
        rows
    }

    /// The rows of the trip's card: the next stop, how far it is and when the bus is due
    /// there, and how many stops are still to come; none without a trip.
    fn trip_rows(&self, f: &NavFrame, lt: &LinesText) -> Vec<(&'static str, String)> {
        let Some(st) = f.stops.first() else {
            return Vec::new();
        };
        let mut rows = vec![(lt.next_stop, st.name.trim().to_string())];
        if let Some(d) = self.next_dist {
            rows.push((lt.distance, distance_text(d)));
        }
        rows.push((lt.arrival, clock_of(st.arrival)));
        rows.push((lt.stops_left, f.stops.len().to_string()));
        rows
    }

    /// Where the legend goes (the bottom right of a map of `w` x `h`) and what it lists:
    /// each colour's name, its colour, and whether it is a band (else a dot). `you` is the
    /// colour the bus is drawn in.
    fn legend_layout(
        &self,
        lt: &LinesText,
        you: Color,
        w: f32,
        h: f32,
        s: f32,
    ) -> (Rect, Vec<(&'static str, Color, bool)>) {
        let mut items = vec![(lt.route, LEVEL[0], true), (lt.driven, DRIVEN, true)];
        if self.show_ai {
            items.push((lt.traffic, super::DOT, false));
        }
        items.push((lt.you, you, false));
        let px = 11.5 * s;
        let row: f32 = items
            .iter()
            .map(|(l, _, bar)| {
                self.fonts.width(l, px, Weight::Medium)
                    + if *bar { 15.0 } else { 10.0 } * s
                    + 12.0 * s
            })
            .sum::<f32>()
            - 12.0 * s;
        let hint = self.fonts.width(lt.keys, px, Weight::Medium);
        // (and a little room: each name is fitted into what is left of the row)
        let lw = row.max(hint) + 24.0 * s + 8.0 * s;
        let lh = 54.0 * s;
        (
            Rect::new(w - 16.0 * s - lw, h - 12.0 * s - lh, lw, lh),
            items,
        )
    }

    /// A card over the map in the new look: an eyebrow (`title`) over rows of a label (muted,
    /// on the left) and its value (bold, on the right), parted by faint lines.
    fn map_card(
        &mut self,
        ui: &mut Painter,
        r: Rect,
        s: f32,
        title: &str,
        rows: &[(&'static str, String)],
    ) {
        ui.rounded(r, 8.0 * s, v5::CARD.alpha(0.94));
        ui.rounded_border(r, 8.0 * s, 1.0, v5::LINE);
        let pad = 14.0 * s;
        spaced(
            ui,
            &mut self.atlas,
            &self.fonts,
            &tr(title).to_uppercase(),
            10.0 * s,
            r.x + pad,
            r.y + CARD_HEAD * s * 0.5 + 1.0 * s,
            v5::MUTED,
            1.6 * s,
        );
        for (k, (label, value)) in rows.iter().enumerate() {
            let y = r.y + (CARD_HEAD + CARD_ROW * k as f32) * s;
            ui.rect(Rect::new(r.x + 1.0, y, r.w - 2.0, 1.0), v5::ROW_LINE);
            let row = Rect::new(r.x + pad, y, r.w - 2.0 * pad, CARD_ROW * s);
            let lw = ui.text_in(
                &mut self.atlas,
                &self.fonts,
                label,
                12.5 * s,
                Weight::Medium,
                row,
                Align::Left,
                v5::MUTED,
            );
            let value = self.fonts.fit(
                value,
                12.5 * s,
                Weight::Bold,
                (row.w - lw - 12.0 * s).max(0.0),
            );
            ui.text_in(
                &mut self.atlas,
                &self.fonts,
                &value,
                12.5 * s,
                Weight::Bold,
                row,
                Align::Right,
                v5::FG,
            );
        }
    }
}

/// Where the lines panel stands across the city map: its left edge and its width (it keeps 12 px from the right edge).
pub fn lines_panel_span(w: f32, s: f32) -> (f32, f32) {
    let pw = (332.0 * s).min(w * 0.42);
    (w - pw - 12.0 * s, pw)
}

/// The compass' centre on a city map `w` x `h` when no legend carries it: at the bottom right, and left of the lines
/// panel while that is open.
pub fn compass_centre(w: f32, h: f32, s: f32, panel_open: bool) -> Vec2 {
    let r = 24.0 * s;
    let right = if panel_open {
        lines_panel_span(w, s).0 - 12.0 * s
    } else {
        w - 36.0 * s
    };
    Vec2::new(right - r, h - 62.0 * s - r)
}

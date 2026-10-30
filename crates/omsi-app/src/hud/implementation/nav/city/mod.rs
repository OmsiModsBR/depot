//! The city map window: opening and placing it, the mouse on it, and the helpers its
//! picture and lines panel share (`picture` draws it, `lines_panel` the bus lines panel).

use super::draw::{rot, DRIVEN, LEVEL};
use super::lines::LinesText;
use super::{LinesHit, NavFrame, Navigator};
use glam::{DVec2, DVec3, Vec2};
use omsi_sim::traffic::Network;
use omsi_ui::paint::Align;
use omsi_ui::theme::v5;
use omsi_ui::{tr, Atlas, Color, Fonts, Painter, Rect, Weight};

mod lines_panel;
mod picture;

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
        ui.text(
            &mut self.atlas,
            &self.fonts,
            &tr(title),
            12.0 * s,
            Weight::Bold,
            Vec2::new(
                r.x + pad,
                r.y + CARD_HEAD * s * 0.5 + self.fonts.cap_height(12.0 * s, Weight::Bold) * 0.5,
            ),
            Align::Left,
            v5::MUTED,
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

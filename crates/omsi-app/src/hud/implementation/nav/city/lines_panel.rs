//! The lines panel on the right of the city map: the package's bus lines, their data and
//! their timetable.

use super::super::draw::{TEXT, TEXT_DIM};
use super::super::lines::{
    decimal_comma, line_color, LinesText, GROUP_BRT_TAB, GROUP_EXPRESS_TAB, GROUP_FEEDER_TAB,
};
use super::super::{LinesHit, Navigator};
use super::*;
use crate::map_surface::{GROUP_BRT, GROUP_EXPRESS};
use glam::{DVec3, Vec2};
use hashbrown::HashMap;
use omsi_ui::paint::Align;
use omsi_ui::theme::v5;
use omsi_ui::{tr, Color, Painter, Rect, Weight};

impl Navigator {
    /// The lines panel on the right of the city map (after the package's `lines.lin`): the data of
    /// the line in focus above, the list of the map's lines below with a tick box each, filters
    /// and a scroll. `on_line` is how many vehicles each ticked line has on the road now.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lines_panel(
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
    pub(super) fn shown_lines(&self) -> Vec<usize> {
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
    pub(super) fn sheet_labels(
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
}

//! A settings window: a sidebar of pages and the rows of the page shown.

use super::super::super::domain::Frame;
use super::*;
use crate::hud::interface::output::{Renderer, Scene};

impl Ui {
    /// A settings window (options, vehicle, world): a sidebar of pages on the left, the rows
    /// of the page shown on the right. A row is `name\u{1f}kind\u{1f}value\u{1f}description\u{1f}fraction`:
    /// `s` a switch (value "on"/"off"), `v` a slider (value text, fraction of the way),
    /// `c` a stepper (value between arrows), `o` opens a list, `a` a button (its text is the
    /// value), `i` information.
    pub(super) fn draw_settings(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        f: &Frame,
        sel: usize,
        items: &[(&str, &str)],
    ) {
        let s = menu_scale(f);
        let dim = self.text.plate(r, scene, 6);
        let sep = self.text.plate(r, scene, 9);
        scene.overlays.push((dim, [0.0, 0.0, f.width, f.height]));
        let none: Vec<String> = Vec::new();
        let (titles, active) = match f.menu_tabs.as_ref() {
            Some((t, a)) => (t, *a),
            None => (&none, 0),
        };
        // a window of a fixed size (as far as the screen allows): the pages differ in how many
        // rows they have, the window does not - a long page scrolls inside it
        let want_side = if titles.is_empty() { 0.0 } else { 200.0 * s };
        let w = (want_side + 680.0 * s)
            .min(f.width - 24.0 * s)
            .max(260.0 * s);
        let side_w = want_side.min(w * 0.38);
        let header_h = 72.0 * s;
        let pad = PAD * s;
        let tin = TEXT_IN * s;
        let fixed_h = (if f.vr { 440.0 } else { 600.0 }) * s;
        let h = fixed_h
            .min(f.height * (if f.vr { 0.70 } else { 0.94 }))
            .max(220.0 * s);
        let room = h - header_h - pad;
        let row_h = ((if f.vr { 54.0 } else { 62.0 }) * s)
            .min(room)
            .max(30.0 * s);
        let rows = ((room / row_h).floor() as usize).clamp(1, items.len().max(1));
        let start = match (items.len() > rows, f.menu_top) {
            (false, _) => 0,
            (true, Some(top)) => (top.max(0.0).round() as usize).min(items.len() - rows),
            (true, None) => sel.saturating_sub(rows / 2).min(items.len() - rows),
        };
        self.hits.menu_start = start;
        self.hits.menu_rows = rows;
        self.hits.menu_row_h = row_h;
        let x = ((f.width - w) * 0.5).round();
        let y = ((f.height - h) * 0.5).round();
        let radius = CARD_R * s;
        self.text.shadow(
            r,
            scene,
            [x, y, x + w, y + h],
            radius,
            28.0 * s,
            10.0 * s,
            110,
        );
        self.text.rounded(
            r,
            scene,
            [x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0],
            radius + 1.0,
            BORDER,
        );
        self.text
            .rounded(r, scene, [x, y, x + w, y + h], radius, PANEL);
        // the header: the game's name small, the title large
        let (title, sub): (String, String) = match f.menu_head.as_ref() {
            Some((t, u)) => (t.clone(), u.clone()),
            None => ("Options".to_string(), String::new()),
        };
        self.menu_header(r, scene, x, y, w, header_h, &title, &sub, s);
        let over = |rect: [f32; 4]| {
            f.cursor.0 >= rect[0]
                && f.cursor.0 <= rect[2]
                && f.cursor.1 >= rect[1]
                && f.cursor.1 <= rect[3]
        };
        // the sidebar: the pages, and the way back at its foot
        if side_w > 0.0 {
            let (sx0, sx1) = (x + pad, x + side_w);
            self.text.rounded(
                r,
                scene,
                [sx0 - 1.0, y + header_h - 1.0, sx1 + 1.0, y + h - pad + 1.0],
                CARD_R * s + 1.0,
                BORDER,
            );
            self.text.rounded(
                r,
                scene,
                [sx0, y + header_h, sx1, y + h - pad],
                CARD_R * s,
                PANEL_ALT,
            );
            let inset = 6.0 * s;
            let bottom = y + h - pad - inset;
            let pages_top = y + header_h + inset;
            let step = if f.vr {
                vr_settings_sidebar_step(bottom - 38.0 * s - inset - pages_top, titles.len(), s)
            } else {
                42.0 * s
            };
            let page_h = (step - 4.0 * s).max(1.0);
            let spx = (15.0 * s).min(page_h * 0.6).max(1.0) as u32;
            for (i, title) in titles.iter().enumerate() {
                let top = pages_top + i as f32 * step;
                let rect = [sx0 + inset, top, sx1 - inset, top + page_h];
                let on = i == active;
                let hov = over(rect) && !on;
                // (the page chosen and the page under the mouse fade in and out)
                let a_on = self.easeq(
                    (1, title.as_str(), 0),
                    if on { 1.0 } else { 0.0 },
                    1.0 / FADE_SECS,
                );
                let a_hov = self.easeq(
                    (2, title.as_str(), 0),
                    if hov { 1.0 } else { 0.0 },
                    1.0 / FADE_SECS,
                );
                let a_bar = self.easeq(
                    (9, title.as_str(), 0),
                    if on { 1.0 } else { 0.0 },
                    1.0 / BAR_SECS,
                );
                if a_hov > 0.0 {
                    self.text
                        .rounded(r, scene, rect, ROW_R * s, fade(LIT, a_hov));
                }
                if a_on > 0.0 {
                    self.text
                        .rounded(r, scene, rect, ROW_R * s, fade(SELECTED, a_on));
                }
                if a_bar > 0.0 {
                    self.accent_bar(r, scene, rect, a_bar, false, s);
                }
                let ink = mix(mix(MUTED, SOFT, a_hov), WHITE, a_on);
                let text = clip_to(&self.text, title, spx as f32, rect[2] - rect[0] - tin * 2.0);
                self.put(
                    r,
                    scene,
                    &text,
                    spx,
                    ink,
                    rect[0] + tin,
                    (rect[1] + rect[3]) * 0.5,
                );
                self.hits.menu_side.push(rect);
            }
            let rect = [sx0 + inset, bottom - 38.0 * s, sx1 - inset, bottom];
            let a_back = self.easeq(
                (3, "back", 0),
                if over(rect) { 1.0 } else { 0.0 },
                1.0 / FADE_SECS,
            );
            if a_back > 0.0 {
                self.text
                    .rounded(r, scene, rect, ROW_R * s, fade(LIT, a_back));
            }
            let back = format!("‹  {}", omsi_ui::tr("Back"));
            self.put(
                r,
                scene,
                &back,
                spx,
                mix(MUTED, WHITE, a_back),
                rect[0] + tin,
                (rect[1] + rect[3]) * 0.5,
            );
            self.hits.menu_side.push(rect);
        }
        // the scroll bar of a long page
        let scrolls = items.len() > rows;
        let cx0 = x + side_w + if side_w > 0.0 { 8.0 * s } else { pad };
        let cx1 = x + w - pad - if scrolls { 8.0 * s } else { 0.0 };
        if scrolls {
            let top = y + header_h;
            let track = [
                x + w - 12.0 * s,
                top,
                x + w - 8.0 * s,
                top + row_h * rows as f32 - 4.0 * s,
            ];
            self.hits.menu_scroll_track = Some(track);
            let hot = over_rect(f.cursor, [x + side_w, y + header_h, x + w, y + h]);
            let thumb = self.thumb(r, scene, track, start, rows, items.len(), hot, s);
            self.hits.menu_scroll_thumb =
                Some([thumb[0] - 6.0 * s, thumb[1], thumb[2] + 6.0 * s, thumb[3]]);
        }
        // the rows
        let any_hovered = over([x + side_w, y + header_h, x + w, y + h]);
        let px = ((15.0 * s).min(row_h * 0.34)) as u32;
        // (the light of the row above: the line between two rows is hidden when either is lit)
        let mut prev_a = 0.0f32;
        for (k, &(id, label)) in items.iter().enumerate().skip(start).take(rows) {
            let ry = y + header_h + row_h * (k - start) as f32;
            let rect = [cx0, ry, cx1, ry + row_h - 4.0 * s];
            // (lit by the mouse over it; by the keyboard's choice only when the keyboard chose)
            let lit =
                f.dropdown.is_none() && (over(rect) || (k == sel && f.menu_kbd && !any_hovered));
            // the light of the row eases in and out
            let a = self.easeq((4, id, k), if lit { 1.0 } else { 0.0 }, 1.0 / FADE_SECS);
            let a_bar = self.easeq((10, id, k), if lit { 1.0 } else { 0.0 }, 1.0 / BAR_SECS);
            if a > 0.0 {
                self.text.rounded(r, scene, rect, ROW_R * s, fade(LIT, a));
            }
            if a_bar > 0.0 {
                self.accent_bar(r, scene, rect, a_bar, false, s);
            }
            if k > start && a.max(prev_a) < 0.5 {
                let sy = (rect[1] - 2.0 * s).round();
                scene
                    .overlays
                    .push((sep, [rect[0] + tin, sy, rect[2] - tin, sy + 1.0]));
            }
            prev_a = a;
            let ink = mix(SOFT, WHITE, a);
            let cy = (rect[1] + rect[3]) * 0.5;
            let nx = rect[0] + tin;
            let rx = rect[2] - tin;
            let mut parts = label.split('\u{1f}');
            let name = parts.next().unwrap_or("");
            let kind = parts.next().unwrap_or("a");
            let value = parts.next().unwrap_or("");
            let desc = parts.next().unwrap_or("");
            let frac: Option<f32> = parts.next().and_then(|p| p.parse().ok());
            let mut ctl: Option<[f32; 4]> = None;
            let left: f32 = match kind {
                // a switch
                "s" => {
                    let on = value == "on";
                    // (the launcher's switch - 34 x 18, a white knob 6 px smaller than the
                    // track - a tenth larger for the menu's larger text)
                    let (tw, th) = (38.0 * s, 20.0 * s);
                    let tx = rx - tw;
                    let track = [tx, cy - th * 0.5, tx + tw, cy + th * 0.5];
                    // (the knob slides over and the track changes its colour)
                    let t = self.ease((5, id, k), if on { 1.0 } else { 0.0 }, 1.0 / FADE_SECS);
                    let tq = quant(t);
                    self.text
                        .rounded(r, scene, track, th * 0.5, mix(TRACK_OFF, accent(), tq));
                    let kn = th - 6.0 * s;
                    let kx = tx + 3.0 * s + (tw - kn - 6.0 * s) * t;
                    self.text.rounded(
                        r,
                        scene,
                        [kx, cy - kn * 0.5, kx + kn, cy + kn * 0.5],
                        kn * 0.5,
                        KNOB,
                    );
                    tx
                }
                // a slider: the track with its knob, the value right of it
                "v" => {
                    let vw = 64.0 * s;
                    self.put_right(
                        r,
                        scene,
                        value,
                        (14.0 * s) as u32,
                        mix(SOFT, WHITE, a),
                        rx,
                        cy,
                    );
                    let tw = (200.0 * s).min((rx - nx) * 0.45);
                    let x1 = rx - vw - 10.0 * s;
                    let x0 = x1 - tw;
                    let th = 4.0 * s;
                    self.text.rounded(
                        r,
                        scene,
                        [x0, cy - th * 0.5, x1, cy + th * 0.5],
                        th * 0.5,
                        SLIDER_TRACK,
                    );
                    // (the knob glides to its place and grows a little under the mouse)
                    let fr = self.ease(
                        (6, id, k),
                        frac.unwrap_or(0.0).clamp(0.0, 1.0),
                        2.0 / FADE_SECS,
                    );
                    let fx = x0 + tw * fr;
                    if fx - x0 >= 1.0 {
                        self.text.rounded(
                            r,
                            scene,
                            [x0, cy - th * 0.5, fx, cy + th * 0.5],
                            th * 0.5,
                            accent(),
                        );
                    }
                    let kn = (13.0 + 2.0 * a) * s;
                    self.text.rounded(
                        r,
                        scene,
                        [fx - kn * 0.5, cy - kn * 0.5, fx + kn * 0.5, cy + kn * 0.5],
                        kn * 0.5,
                        KNOB,
                    );
                    ctl = Some([x0, rect[1], x1, rect[3]]);
                    x0
                }
                // a stepper: the value between two arrows, the left half steps back
                "c" => {
                    let text = format!("‹  {value}  ›");
                    let l = self.chip(
                        r,
                        scene,
                        &text,
                        (13.0 * s) as u32,
                        mix(SOFT, WHITE, a),
                        mix(CHIP, SELECTED, a),
                        false,
                        rx,
                        cy,
                        s,
                    );
                    ctl = Some([l, rect[1], rx, rect[3]]);
                    l
                }
                // opens another list
                "o" => {
                    let cw = self.put_right(r, scene, "›", px + 6, mix(MUTED, WHITE, a), rx, cy);
                    if value.is_empty() {
                        rx - cw
                    } else {
                        // (the value now, as the stepper showed it)
                        let vw = self.put_right(
                            r,
                            scene,
                            value,
                            (14.0 * s) as u32,
                            SOFT,
                            rx - cw - 8.0 * s,
                            cy,
                        );
                        rx - cw - 8.0 * s - vw
                    }
                }
                "i" => {
                    let cw = self.put_right(r, scene, value, (14.0 * s) as u32, SOFT, rx, cy);
                    rx - cw
                }
                // a value being typed: lit in the accent
                "E" => self.chip(
                    r,
                    scene,
                    value,
                    (13.0 * s) as u32,
                    accent_ink(),
                    accent_soft(),
                    false,
                    rx,
                    cy,
                    s,
                ),
                // a button
                _ => {
                    if value.is_empty() {
                        rx
                    } else {
                        self.chip(
                            r,
                            scene,
                            value,
                            (13.0 * s) as u32,
                            mix(SOFT, accent_ink(), a),
                            mix(CHIP, accent_soft(), a),
                            false,
                            rx,
                            cy,
                            s,
                        )
                    }
                }
            };
            let avail = left - 16.0 * s - nx;
            if desc.is_empty() {
                let n = clip_to(&self.text, name, px as f32, avail);
                self.put(r, scene, &n, px, ink, nx, cy);
            } else {
                let n = clip_to(&self.text, name, px as f32, avail);
                self.put(r, scene, &n, px, ink, nx, cy - 9.0 * s);
                let dpx = (12.0 * s) as u32;
                let d = clip_to(&self.text, desc, dpx as f32, avail);
                self.put(r, scene, &d, dpx, MUTED, nx, cy + 10.0 * s);
            }
            self.hits.menu_rects.push(rect);
            self.hits.menu_ctl.push(ctl);
        }
        // a drop-down over a row (the weather preset, the clouds): the entries under the
        // row's value as a select's in a page - over it where there is no room under
        if let Some(dd) = f
            .dropdown
            .as_ref()
            .filter(|d| d.row >= start && d.row < start + rows && !d.items.is_empty())
        {
            let ry = y + header_h + row_h * (dd.row - start) as f32;
            let row_b = ry + row_h - 4.0 * s;
            let item_h = (36.0 * s).min(row_h);
            let inner = 4.0 * s;
            let below = y + h - pad - row_b - 4.0 * s;
            let above = ry - (y + header_h) - 4.0 * s;
            let want = dd.items.len().min(8);
            let fits =
                |room: f32| (((room - 2.0 * inner) / item_h).floor().max(0.0) as usize).min(want);
            let down = fits(below) >= want || fits(below) >= fits(above);
            let n_vis = (if down { fits(below) } else { fits(above) }).max(1);
            let ph = n_vis as f32 * item_h + 2.0 * inner;
            let pw = (300.0 * s).min(cx1 - cx0);
            let (px1, py0) = (
                cx1 - 6.0 * s,
                if down {
                    row_b + 4.0 * s
                } else {
                    ry - 4.0 * s - ph
                },
            );
            let px0 = px1 - pw;
            let panel = [px0, py0, px1, py0 + ph];
            let top = dd.top.min(dd.items.len() - n_vis.min(dd.items.len()));
            self.hits.dd_top = top;
            self.hits.dd_rows = n_vis;
            let rad = (CARD_R * s).min(10.0 * s);
            self.text
                .shadow(r, scene, panel, rad, 18.0 * s, 6.0 * s, 150);
            // (the launcher's popup: the field's grey, a hairline round it)
            self.text.rounded(
                r,
                scene,
                [
                    panel[0] - 1.0,
                    panel[1] - 1.0,
                    panel[2] + 1.0,
                    panel[3] + 1.0,
                ],
                rad + 1.0,
                BORDER,
            );
            self.text.rounded(r, scene, panel, rad, PANEL_ALT);
            let more = dd.items.len() > n_vis;
            let dpx = (14.0 * s) as u32;
            let tin = TEXT_IN * s;
            for i in 0..n_vis {
                let idx = top + i;
                let rect = [
                    px0 + inner,
                    py0 + inner + i as f32 * item_h,
                    px1 - inner - if more { 8.0 * s } else { 0.0 },
                    py0 + inner + (i + 1) as f32 * item_h,
                ];
                let cur = dd.current == Some(idx);
                let hot = over(rect) || (idx == dd.sel && f.menu_kbd && !over(panel));
                let a = self.easeq(
                    (15, "dropdown", idx),
                    if hot { 1.0 } else { 0.0 },
                    1.0 / FADE_SECS,
                );
                if cur {
                    self.text.rounded(r, scene, rect, ROW_R * s, LIT);
                }
                if a > 0.0 {
                    self.text
                        .rounded(r, scene, rect, ROW_R * s, fade(SELECTED, a));
                }
                if cur {
                    self.accent_bar(r, scene, rect, 1.0, false, s);
                }
                let text = clip_to(
                    &self.text,
                    dd.items[idx],
                    dpx as f32,
                    rect[2] - rect[0] - tin * 2.0,
                );
                self.put(
                    r,
                    scene,
                    &text,
                    dpx,
                    if cur { WHITE } else { mix(SOFT, WHITE, a) },
                    rect[0] + tin,
                    (rect[1] + rect[3]) * 0.5,
                );
                self.hits.dd_rects.push(rect);
            }
            if more {
                let track = [px1 - 8.0 * s, py0 + inner, px1 - 4.0 * s, py0 + ph - inner];
                let thumb = self.thumb(r, scene, track, top, n_vis, dd.items.len(), over(panel), s);
                self.hits.dd_scroll = Some((
                    track,
                    [thumb[0] - 6.0 * s, thumb[1], thumb[2] + 4.0 * s, thumb[3]],
                ));
            }
        }
    }
}

/// Fit all VR page buttons above the separately reserved Back button.
pub fn vr_settings_sidebar_step(available: f32, pages: usize, scale: f32) -> f32 {
    (available.max(0.0) / pages.max(1) as f32).min(42.0 * scale)
}

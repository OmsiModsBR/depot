//! The game menu and its lists (the pick list): the parts it is built of and the menu itself.

use super::super::super::domain::{Frame, MenuKind};
use super::*;
use crate::hud::interface::output::{Renderer, Scene};

impl Ui {
    /// `text` at `x`, its middle on `cy`; returns its width.
    pub(super) fn put(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        text: &str,
        px: u32,
        color: [u8; 4],
        x: f32,
        cy: f32,
    ) -> f32 {
        let l = self.text.label(r, scene, text, px, color);
        let y = cy - l.h as f32 * 0.5;
        scene
            .overlays
            .push((l.tex, [x, y, x + l.w as f32, y + l.h as f32]));
        l.w as f32
    }

    /// The value that eases to `target` (a part of the menu changing its colour or place):
    /// each frame it goes `speed` per second of the way (exponentially). One value per `key`;
    /// the first time it is asked for it is there already.
    pub(super) fn ease(&mut self, key: (u8, &str, usize), target: f32, speed: f32) -> f32 {
        // (`speed`: the share of the whole way it goes in a second: it takes 1 / speed seconds)
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        let dt = self.anim_dt;
        let v = self.anim.entry(h.finish()).or_insert(target);
        let step = speed * dt;
        if (target - *v).abs() <= step {
            *v = target;
        } else if target > *v {
            *v += step;
        } else {
            *v -= step;
        }
        *v
    }

    /// `ease`, in eighths: a colour or a light that fades is a texture made once per step
    /// (a plate or a text of that colour), so a fade goes in a few steps, not in hundreds.
    pub(super) fn easeq(&mut self, key: (u8, &str, usize), target: f32, speed: f32) -> f32 {
        quant(self.ease(key, target, speed))
    }

    /// `text` ending at `right`, its middle on `cy`; returns its width.
    pub(super) fn put_right(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        text: &str,
        px: u32,
        color: [u8; 4],
        right: f32,
        cy: f32,
    ) -> f32 {
        let l = self.text.label(r, scene, text, px, color);
        let y = cy - l.h as f32 * 0.5;
        scene
            .overlays
            .push((l.tex, [right - l.w as f32, y, right, y + l.h as f32]));
        l.w as f32
    }

    /// A scroll bar's thumb in `track` for `shown` of `n` lines from `first`, as the
    /// launcher's: no track, a thin white bar that lightens while the mouse is over its list
    /// (`hot`), at least 28 px long. Returns the rect drawn.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn thumb(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        track: [f32; 4],
        first: usize,
        shown: usize,
        n: usize,
        hot: bool,
        s: f32,
    ) -> [f32; 4] {
        let th = track[3] - track[1];
        let n = n.max(1) as f32;
        let len = (th * shown as f32 / n).max((28.0 * s).min(th));
        let t0 =
            track[1] + (th - len) * (first as f32 / (n - shown as f32).max(1.0)).clamp(0.0, 1.0);
        let thumb = [track[0], t0, track[2], t0 + len];
        let a = self.easeq(
            (16, "thumb", track[0] as usize),
            if hot { 1.0 } else { 0.0 },
            1.0 / FADE_SECS,
        );
        self.text
            .rounded(r, scene, thumb, 2.0 * s, mix(THUMB, THUMB_HOT, a));
        thumb
    }

    /// The accent bar at the left edge of a line, growing from its middle as `k` goes 0 to 1.
    pub(super) fn accent_bar(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        rect: [f32; 4],
        k: f32,
        danger: bool,
        s: f32,
    ) {
        let full = (rect[3] - rect[1] - 20.0 * s).max(10.0 * s);
        let bh = full * (0.4 + 0.6 * k);
        let by = (rect[1] + rect[3]) * 0.5 - bh * 0.5;
        self.text.rounded(
            r,
            scene,
            [rect[0], by, rect[0] + 2.0 * s, by + bh],
            1.0 * s,
            fade(if danger { DANGER } else { accent() }, k),
        );
    }

    /// The header of the card at (`x`, `y`) of `w` wide: what the list is of, small and in
    /// capitals, the title large under it, a hairline under both. Its text starts where the
    /// text of the lines does. Returns the middle of the header.
    pub(super) fn menu_header(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        x: f32,
        y: f32,
        w: f32,
        header_h: f32,
        title: &str,
        sub: &str,
        s: f32,
    ) -> f32 {
        let left = x + (PAD + TEXT_IN) * s;
        // (the game's name in the accent; the list names above are in capitals)
        // (as the launcher's section headings: small bold capitals, dim; the title bold)
        let eyebrow = if sub.is_empty() {
            "OPENOMSI".to_string()
        } else {
            sub.to_uppercase()
        };
        let room = w - (PAD + TEXT_IN) * 2.0 * s;
        let eyebrow = clip_bold(&self.text, &eyebrow, 11.0 * s, room);
        let e = self
            .text
            .label(r, scene, &eyebrow, (11.0 * s) as u32 | BOLD, DIM);
        let title = clip_bold(&self.text, title, 22.0 * s, room);
        let t = self
            .text
            .label(r, scene, &title, (22.0 * s) as u32 | BOLD, WHITE);
        let band = header_h - 6.0 * s;
        let top = y + (band - (e.h as f32 + t.h as f32 - 2.0 * s)) * 0.5;
        scene
            .overlays
            .push((e.tex, [left, top, left + e.w as f32, top + e.h as f32]));
        let ty = top + e.h as f32 - 2.0 * s;
        scene
            .overlays
            .push((t.tex, [left, ty, left + t.w as f32, ty + t.h as f32]));
        y + band * 0.5
    }

    /// A pill (`cap`: a key cap) of `fill` with `text` in it, ending at `right`; returns
    /// its left edge.
    pub(super) fn chip(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        text: &str,
        px: u32,
        color: [u8; 4],
        fill: [u8; 4],
        cap: bool,
        right: f32,
        cy: f32,
        s: f32,
    ) -> f32 {
        let l = self.text.label(r, scene, text, px, color);
        let (cw, ch) = (l.w as f32 + 18.0 * s, l.h as f32 + 6.0 * s);
        let x0 = right - cw;
        if cap {
            // (a key cap: a fine light edge round it)
            self.text.rounded(
                r,
                scene,
                [
                    x0 - 1.0,
                    cy - ch * 0.5 - 1.0,
                    right + 1.0,
                    cy + ch * 0.5 + 1.0,
                ],
                5.0 * s,
                [96, 96, 96, 255],
            );
        }
        self.text.rounded(
            r,
            scene,
            [x0, cy - ch * 0.5, right, cy + ch * 0.5],
            if cap { 4.0 * s } else { ROW_R * s },
            fill,
        );
        let (tx, ty) = (x0 + 9.0 * s, cy - l.h as f32 * 0.5);
        scene
            .overlays
            .push((l.tex, [tx, ty, tx + l.w as f32, ty + l.h as f32]));
        x0
    }

    /// The game menu and its lists (options, lines, tours ...): a card in the middle of a
    /// dimmed picture. Options are settings lines with switches and values; lines and tours
    /// are bigger lines with the timetable of the chosen one beside them.
    pub(super) fn draw_menu(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame) {
        self.hits.menu_rects.clear();
        self.hits.menu_ctl.clear();
        self.hits.menu_side.clear();
        self.hits.menu_pane.clear();
        self.hits.menu_pane_start = 0;
        self.hits.menu_pane_go = None;
        self.hits.menu_pane_box = None;
        self.hits.menu_pane_scroll = None;
        self.hits.menu_time.clear();
        self.hits.menu_scroll_thumb = None;
        self.hits.menu_scroll_track = None;
        self.hits.dd_rects.clear();
        self.hits.dd_scroll = None;
        let overlay_start = scene.overlays.len();
        let Some((sel, items)) = f.menu else {
            self.menu_overlay_range = overlay_start..overlay_start;
            self.anim.clear();
            return;
        };
        // a settings window has its own layout
        if f.menu_kind == MenuKind::Options && f.menu_tabs.is_some() {
            self.draw_settings(r, scene, f, sel, items);
            self.menu_overlay_range = overlay_start..scene.overlays.len();
            return;
        }
        let s = menu_scale(f);
        let kind = f.menu_kind;
        // the picture dimmed behind the menu (the first overlay: the headset's menu takes it
        // for the backdrop)
        let dim = self.text.plate(r, scene, 6);
        let sep = self.text.plate(r, scene, 9);
        scene.overlays.push((dim, [0.0, 0.0, f.width, f.height]));
        // key hints only where there is a keyboard
        let keys = !f.vr && !crate::core::platform::touch_controls();
        // the timetable beside the list, where the window is wide enough
        let preview = f
            .menu_preview
            .as_ref()
            .filter(|_| !f.vr && f.width >= 760.0 * s);
        let timetable_kind = matches!(kind, MenuKind::Lines | MenuKind::Tours);
        let pane_w = if preview.is_some() {
            (if timetable_kind { 320.0 } else { 340.0 }) * s
        } else {
            0.0
        };
        let want = if timetable_kind { 360.0 * s } else { 380.0 * s };
        let w = (want + pane_w).min(f.width - 24.0 * s).max(200.0 * s);
        let list_w = w - pane_w;
        let header_h = 72.0 * s;
        let pad = PAD * s;
        let tin = TEXT_IN * s;
        // as many lines as fit at a readable height; a longer menu scrolls (the wheel, the
        // arrow keys), the chosen line kept in view
        // (lines and tours: a card of one fixed height, a share of the screen's)
        // "Back" of lines, tours and the other lists stands alone under the list (not a line of
        // it: no scrolling down to it), and their card keeps one size
        let back_txt = omsi_ui::tr("Back").into_owned();
        let back_footer = (timetable_kind || kind == MenuKind::List)
            && items
                .last()
                .is_some_and(|&(id, l)| id == "back" && l == back_txt.as_str());
        let nl = items.len() - back_footer as usize;
        let foot_h = if back_footer { 48.0 * s } else { 0.0 };
        let fixed_h =
            matches!(kind, MenuKind::Lines | MenuKind::Tours | MenuKind::List).then(|| {
                if f.vr {
                    f.height * 0.60
                } else {
                    (520.0 * s).min(f.height * 0.94)
                }
            });
        let room = match fixed_h {
            Some(fh) => fh - header_h - pad - foot_h,
            None => f.height * (if f.vr { 0.60 } else { 0.92 }) - header_h - pad - 8.0 * s,
        };
        let base = match (f.vr, kind) {
            (true, _) => 40.0,
            (_, MenuKind::Lines) => 44.0,
            (_, MenuKind::Tours) => 40.0,
            _ => 42.0,
        } * s;
        let row_h = base.min(room / nl.max(1) as f32).max(34.0 * s);
        let rows = ((room / row_h).floor() as usize).clamp(1, nl.max(1));
        let sel_l = sel.min(nl.saturating_sub(1));
        let start = match (nl > rows, f.menu_top) {
            (false, _) => 0,
            (true, Some(top)) => (top.max(0.0).round() as usize).min(nl - rows),
            (true, None) => sel_l.saturating_sub(rows / 2).min(nl - rows),
        };
        self.hits.menu_start = start;
        self.hits.menu_rows = rows;
        self.hits.menu_row_h = row_h;
        let px = (((if timetable_kind { 14.0 } else { 16.0 }) * s).min(row_h * 0.45)) as u32;
        let mut h = header_h + row_h * rows as f32 + pad + foot_h;
        if let Some(fh) = fixed_h {
            // (lines and tours: one size, whatever the list holds, like the settings window)
            h = fh;
        } else if preview.is_some() {
            h = h.max((420.0 * s).min(f.height * 0.92));
        }
        let x = ((f.width - w) * 0.5).round();
        let y = ((f.height - h) * 0.5).round();
        let list_r = x + list_w;
        let radius = CARD_R * s;
        // the card: a soft shadow, a 1 px hairline round it, the flat panel
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
        // the header: what the list is of (or the game's name), and the title
        let (title, sub): (String, String) = match f.menu_head.as_ref() {
            Some((t, u)) => (t.clone(), u.clone()),
            None => {
                let t = if f.paused { "Paused" } else { "Menu" };
                (t.to_string(), String::new())
            }
        };
        self.menu_header(r, scene, x, y, w, header_h, &title, &sub, s);
        // the scroll bar: where the lines shown lie in the whole menu
        let scrolls = nl > rows;
        if scrolls {
            let top = y + header_h;
            let track = [
                list_r - 12.0 * s,
                top,
                list_r - 8.0 * s,
                top + row_h * rows as f32 - 4.0 * s,
            ];
            self.hits.menu_scroll_track = Some(track);
            let hot = over_rect(f.cursor, [x, y, list_r, y + h]);
            let thumb = self.thumb(r, scene, track, start, rows, nl, hot, s);
            // (a wider grip than the drawn thumb: three pixels are hard to hit)
            self.hits.menu_scroll_thumb =
                Some([thumb[0] - 6.0 * s, thumb[1], thumb[2] + 6.0 * s, thumb[3]]);
        }
        // (the line under the mouse is the one lit; the keyboard's choice only while the
        // mouse is off the lines - both lit at once read as two choices)
        let over = |rect: [f32; 4]| {
            f.cursor.0 >= rect[0]
                && f.cursor.0 <= rect[2]
                && f.cursor.1 >= rect[1]
                && f.cursor.1 <= rect[3]
        };
        // (the whole list: in the gaps between the lines the keyboard's choice, the top line,
        // lit up for a moment as the mouse went down the list)
        let any_hovered = over([x, y, list_r, y + h]);
        let right = list_r - if scrolls { 20.0 * s } else { pad };
        let line_pre = format!("{} ", omsi_ui::tr("Line"));
        let tour_pre = format!("{} ", omsi_ui::tr("Tour"));
        // (all line signs as wide as the widest, so that the texts after them line up)
        let mut sign_w = 48.0 * s;
        if kind == MenuKind::Lines {
            for &(_, label) in items.iter() {
                if let Some(n) = label
                    .strip_prefix(line_pre.as_str())
                    .and_then(|rest| rest.rsplit_once("  ("))
                    .map(|(n, _)| n)
                {
                    sign_w = sign_w.max(self.text.width(n, 15.0 * s) + 22.0 * s);
                }
            }
        }
        // (a list of codes and names - the destinations, `"{code:>3}  {name}"` - in two
        // columns: the codes right-aligned in one as wide as the widest, the names after it.
        // Padded with spaces, the proportional font left them ragged.)
        let mut code_w = 0.0f32;
        if kind == MenuKind::List {
            for &(_, label) in items[..nl].iter() {
                if let Some((c, _)) = code_and_name(label) {
                    code_w = code_w.max(self.text.width(c, px as f32));
                }
            }
        }
        for (k, &(id, label)) in items[..nl].iter().enumerate().skip(start).take(rows) {
            let ry = y + header_h + row_h * (k - start) as f32;
            let gap = 4.0 * s;
            let rect = [x + pad, ry, right, ry + row_h - gap];
            let lit = over(rect) || (k == sel && f.menu_kbd && !any_hovered);
            let danger = id == "quit";
            let is_back = id == "back" && label == back_txt.as_str();
            // a thin line between the groups: the everyday lines apart from the rarer ones
            // (none in the other lists: "Back" there stands apart under them, without a line)
            let apart = k > start
                && match kind {
                    MenuKind::Lines => id == "free",
                    _ => false,
                };
            if apart {
                let sy = (ry - 2.0 * s).round();
                scene
                    .overlays
                    .push((sep, [x + pad + tin, sy, right - tin, sy + 1.0]));
            }
            // (the light of the line eases in and out)
            let glow = self.easeq((7, id, k), if lit { 1.0 } else { 0.0 }, 1.0 / FADE_SECS);
            // (the tour chosen stays marked, whatever the mouse is over)
            let active = kind == MenuKind::Tours && k == sel && !is_back;
            let a_act = self.easeq((14, id, k), if active { 1.0 } else { 0.0 }, 1.0 / FADE_SECS);
            let bar = self.easeq(
                (8, id, k),
                if lit || active { 1.0 } else { 0.0 },
                1.0 / BAR_SECS,
            );
            if a_act > 0.0 {
                self.text
                    .rounded(r, scene, rect, ROW_R * s, fade(SELECTED, a_act));
            }
            if glow > 0.0 {
                self.text.rounded(
                    r,
                    scene,
                    rect,
                    ROW_R * s,
                    fade(if danger { LIT_DANGER } else { LIT }, glow),
                );
            }
            if bar > 0.0 {
                // the accent bar at the left edge of the lit line, growing from its middle
                self.accent_bar(r, scene, rect, bar, danger, s);
            }
            let ink = if danger {
                mix([232, 138, 128, 0], [255, 176, 166, 0], glow)
            } else {
                mix(SOFT, WHITE, glow.max(a_act))
            };
            let cy = (rect[1] + rect[3]) * 0.5;
            let lx = rect[0] + tin;
            let rx = rect[2] - tin;
            let mut done = false;
            match kind {
                // a line of the timetable: its number on a solid sign, like a line sign on a bus
                MenuKind::Lines => {
                    let parsed = label
                        .strip_prefix(line_pre.as_str())
                        .and_then(|rest| rest.rsplit_once("  ("))
                        .map(|(n, t)| (n, t.trim_end_matches(')')));
                    if let Some((name, info)) = parsed {
                        let bl = self
                            .text
                            .label(r, scene, name, (15.0 * s) as u32, ON_ACCENT);
                        let (bw, bh) = (sign_w, 28.0 * s);
                        let bx = lx;
                        let sign = mix(mix(accent(), [0, 0, 0, 255], 0.11), accent_hot(), glow);
                        self.text.rounded(
                            r,
                            scene,
                            [bx, cy - bh * 0.5, bx + bw, cy + bh * 0.5],
                            7.0 * s,
                            sign,
                        );
                        let (lx0, ly0) = (bx + (bw - bl.w as f32) * 0.5, cy - bl.h as f32 * 0.5);
                        scene
                            .overlays
                            .push((bl.tex, [lx0, ly0, lx0 + bl.w as f32, ly0 + bl.h as f32]));
                        let tx = bx + bw + 14.0 * s;
                        let d = 22.0 * s;
                        self.text.rounded(
                            r,
                            scene,
                            [rx - d, cy - d * 0.5, rx, cy + d * 0.5],
                            d * 0.5,
                            fade(accent(), 0.10 + 0.30 * glow),
                        );
                        let aw = self.text.width("›", (px + 2) as f32);
                        self.put(
                            r,
                            scene,
                            "›",
                            px + 2,
                            mix(MUTED, accent_hot(), glow),
                            rx - d * 0.5 - aw * 0.5,
                            cy - 1.0 * s,
                        );
                        let info = clip_to(&self.text, info, px as f32, rx - d - 12.0 * s - tx);
                        self.put(r, scene, &info, px, ink, tx, cy);
                        done = true;
                    }
                }
                MenuKind::List if code_w > 0.0 => {
                    if let Some((code, name)) = code_and_name(label) {
                        self.put_right(
                            r,
                            scene,
                            code,
                            px,
                            mix(MUTED, accent_hot(), glow),
                            lx + code_w,
                            cy,
                        );
                        let nx = lx + code_w + 12.0 * s;
                        let name = clip_to(&self.text, name, px as f32, rx - nx);
                        self.put(r, scene, &name, px, ink, nx, cy);
                        done = true;
                    }
                }
                // a tour: its name (the time is chosen beside the list)
                MenuKind::Tours => {
                    if let Some(rest) = label.strip_prefix(tour_pre.as_str()) {
                        let num = rest.split_once("  ").map(|(n, _)| n).unwrap_or(rest).trim();
                        let name =
                            clip_to(&self.text, &format!("{tour_pre}{num}"), px as f32, rx - lx);
                        self.put(r, scene, &name, px, ink, lx, cy);
                        done = true;
                    }
                }
                _ => {}
            }
            if !done {
                let (text, more) = strip_more(label);
                let mut avail = rx - lx;
                if is_back {
                    let plain = matches!(kind, MenuKind::Lines | MenuKind::Tours);
                    if plain {
                        self.text
                            .rounded(r, scene, rect, ROW_R * s, fade(LIT, 0.55));
                    }
                    let idle = if plain { SOFT } else { MUTED };
                    self.put(
                        r,
                        scene,
                        &format!("‹  {text}"),
                        px,
                        if lit { WHITE } else { idle },
                        lx,
                        cy,
                    );
                } else {
                    if more {
                        // (the line opens another list)
                        let cw = self.put_right(
                            r,
                            scene,
                            "›",
                            px + 6,
                            if lit { WHITE } else { MUTED },
                            rx,
                            cy,
                        );
                        avail -= cw + 10.0 * s;
                    } else if id == "resume" && keys {
                        // ("Resume" shows the key that does the same)
                        let left = self.chip(
                            r,
                            scene,
                            "Esc",
                            (12.0 * s) as u32,
                            WHITE,
                            [52, 52, 52, 255],
                            true,
                            rx,
                            cy,
                            s,
                        );
                        avail = left - 10.0 * s - lx;
                    }
                    let text = clip_to(&self.text, text, px as f32, avail);
                    self.put(r, scene, &text, px, ink, lx, cy);
                }
            }
            self.hits.menu_rects.push(rect);
        }
        if back_footer {
            // ("Back" pinned to the bottom of the list: the rects of the lines
            // out of view are empty ones, so that a rect stays at the index of its line)
            let bk = items.len() - 1;
            let fr = [x + pad, y + h - pad - 36.0 * s, right, y + h - pad];
            let lit = over(fr) || (sel == bk && f.menu_kbd && !any_hovered);
            let glow = self.easeq(
                (7, "back", bk),
                if lit { 1.0 } else { 0.0 },
                1.0 / FADE_SECS,
            );
            self.text
                .rounded(r, scene, fr, ROW_R * s, fade(LIT, 0.55 + 0.45 * glow));
            if glow > 0.0 {
                self.accent_bar(r, scene, fr, glow, false, s);
            }
            let (text, _) = strip_more(back_txt.as_str());
            self.put(
                r,
                scene,
                &format!("‹  {text}"),
                px,
                mix(SOFT, WHITE, glow),
                fr[0] + tin,
                (fr[1] + fr[3]) * 0.5,
            );
            while self.hits.menu_rects.len() < bk - start {
                self.hits.menu_rects.push([-1.0e9; 4]);
            }
            self.hits.menu_rects.push(fr);
        }
        // the timetable of the chosen line or tour, beside the list
        if let Some(p) = preview {
            let (px0, py0) = (list_r + 4.0 * s, y + header_h);
            let (px1, py1) = (x + w - pad, y + h - pad);
            self.text.rounded(
                r,
                scene,
                [px0 - 1.0, py0 - 1.0, px1 + 1.0, py1 + 1.0],
                CARD_R * s + 1.0,
                BORDER,
            );
            self.text
                .rounded(r, scene, [px0, py0, px1, py1], CARD_R * s, PANEL_ALT);
            let pad = tin;
            let inner = px1 - px0 - pad * 2.0;
            let mut cy = py0 + 28.0 * s;
            let head = clip_to(&self.text, &p.title, 16.0 * s, inner);
            self.put(r, scene, &head, (16.0 * s) as u32, WHITE, px0 + pad, cy);
            cy += 22.0 * s;
            let meta = clip_to(&self.text, &p.meta, 11.0 * s, inner);
            self.put(r, scene, &meta, (11.0 * s) as u32, MUTED, px0 + pad, cy);
            cy += 18.0 * s;
            scene
                .overlays
                .push((sep, [px0 + pad, cy.round(), px1 - pad, cy.round() + 1.0]));
            let rpx = (13.0 * s) as u32;
            let lh = 24.0 * s;
            let mut top = cy + 10.0 * s;
            // the tour, and the time of the trip, each between arrows that step through them
            let nav_rows: Vec<(&String, f32, usize)> =
                p.time.iter().map(|t| (t, 20.0f32, 0usize)).collect();
            for (time, fs, base) in nav_rows {
                let bh = 30.0 * s;
                let bw = 46.0 * s;
                let by = top;
                let right = px1 - pad;
                let left = px0 + pad;
                for j in 0..2usize {
                    let rect = if j == 0 {
                        [left, by, left + bw, by + bh]
                    } else {
                        [right - bw, by, right, by + bh]
                    };
                    let a = self.easeq(
                        (14, "time", j + base),
                        if over(rect) { 1.0 } else { 0.0 },
                        1.0 / FADE_SECS,
                    );
                    self.text.rounded(
                        r,
                        scene,
                        rect,
                        ROW_R * s,
                        mix(
                            [accent()[0], accent()[1], accent()[2], 60],
                            accent(),
                            a * 0.7,
                        ),
                    );
                    let col = mix(accent_hot(), [18, 14, 8, 255], a);
                    let (cx, cy) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
                    // (a real arrow: a shaft and a head of stacked strips)
                    let dir = if j == 0 { -1.0 } else { 1.0 };
                    let (half, head_w, head_h) = (9.0 * s, 7.0 * s, 7.0 * s);
                    let t = (2.0 * s).max(2.0);
                    self.text.rounded(
                        r,
                        scene,
                        [cx - half, cy - t * 0.5, cx + half, cy + t * 0.5],
                        0.0,
                        col,
                    );
                    let tip = cx + dir * half;
                    let strips = 7;
                    for k in 0..strips {
                        // (strip k, from the head's base towards the tip, narrowing)
                        let bx = tip - dir * head_w * (1.0 - k as f32 / strips as f32);
                        let ex = tip - dir * head_w * (1.0 - (k as f32 + 1.0) / strips as f32);
                        let h = head_h * (1.0 - (k as f32 + 0.5) / strips as f32);
                        self.text.rounded(
                            r,
                            scene,
                            [bx.min(ex), cy - h, bx.max(ex), cy + h],
                            0.0,
                            col,
                        );
                    }
                    self.hits.menu_time.push(rect);
                }
                let mid_l = left + bw + 10.0 * s;
                let mid_r = right - bw - 10.0 * s;
                let time = clip_to(&self.text, time, fs * s, mid_r - mid_l);
                let tw = self.text.width(&time, fs * s);
                self.put(
                    r,
                    scene,
                    &time,
                    (fs * s) as u32,
                    if base == 0 { accent_ink() } else { WHITE },
                    mid_l + (mid_r - mid_l - tw) * 0.5,
                    by + bh * 0.5,
                );
                top = by + bh + 8.0 * s;
            }
            let n = p.rows.len();
            if let (Some(chosen), Some(button)) = (p.chosen, p.button.as_ref()) {
                // the stops to start from: the one chosen marked, a click chooses another
                let go_h = 34.0 * s;
                let go = [px0 + pad, py1 - 12.0 * s - go_h, px1 - pad, py1 - 12.0 * s];
                let fit = (((go[1] - 10.0 * s) - top) / lh).floor().max(1.0) as usize;
                let first = match f.pane_first {
                    Some(p) if n > fit => p.min(n - fit),
                    _ if n > fit => chosen.saturating_sub(fit / 2).min(n - fit),
                    _ => 0,
                };
                self.hits.menu_pane_start = first;
                self.hits.menu_pane_box = Some([px0, py0, px1, py1]);
                // (a long list of stops: a thin scroll bar at the pane's edge)
                if n > fit {
                    let hot = over([px0, py0, px1, py1]);
                    let track = [px1 - 7.0 * s, top, px1 - 3.0 * s, go[1] - 10.0 * s];
                    let thumb = self.thumb(r, scene, track, first, fit, n, hot, s);
                    self.hits.menu_pane_scroll = Some((
                        track,
                        [thumb[0] - 6.0 * s, thumb[1], thumb[2] + 3.0 * s, thumb[3]],
                        n,
                        fit,
                    ));
                }
                let time_w = p
                    .rows
                    .iter()
                    .skip(first)
                    .take(fit)
                    .map(|row| self.text.width(&row.1, rpx as f32))
                    .fold(0.0f32, f32::max);
                for (i, (what, when)) in p.rows.iter().enumerate().skip(first).take(fit) {
                    let ry = top + lh * (i - first) as f32 + lh * 0.5;
                    let rect = [px0 + 8.0 * s, ry - lh * 0.5, px1 - 8.0 * s, ry + lh * 0.5];
                    let on = i == chosen;
                    let hov = over(rect) && !on;
                    let a_on =
                        self.easeq((11, "stop", i), if on { 1.0 } else { 0.0 }, 1.0 / FADE_SECS);
                    let a_hov = self.easeq(
                        (12, "stop", i),
                        if hov { 1.0 } else { 0.0 },
                        1.0 / FADE_SECS,
                    );
                    if a_hov > 0.0 {
                        self.text
                            .rounded(r, scene, rect, ROW_R * s, fade(LIT, a_hov));
                    }
                    if a_on > 0.0 {
                        self.text
                            .rounded(r, scene, rect, ROW_R * s, fade(SELECTED, a_on));
                        self.text.rounded(
                            r,
                            scene,
                            [
                                rect[0],
                                rect[1] + 7.0 * s,
                                rect[0] + 2.0 * s,
                                rect[3] - 7.0 * s,
                            ],
                            1.0 * s,
                            fade(accent(), a_on),
                        );
                    }
                    self.put_right(r, scene, when, rpx, accent_ink(), px1 - pad, ry);
                    let what = clip_to(&self.text, what, rpx as f32, inner - time_w - 14.0 * s);
                    self.put(r, scene, &what, rpx, mix(SOFT, WHITE, a_on), px0 + pad, ry);
                    self.hits.menu_pane.push(rect);
                }
                // the button that starts the trip
                let a_go = self.easeq(
                    (13, "go", 0),
                    if over(go) { 1.0 } else { 0.0 },
                    1.0 / FADE_SECS,
                );
                self.text
                    .rounded(r, scene, go, ROW_R * s, mix(accent(), accent_hot(), a_go));
                let l = self
                    .text
                    .label(r, scene, button, (14.0 * s) as u32 | BOLD, ON_ACCENT);
                let (gx, gy) = (
                    go[0] + (go[2] - go[0] - l.w as f32) * 0.5,
                    (go[1] + go[3]) * 0.5 - l.h as f32 * 0.5,
                );
                scene
                    .overlays
                    .push((l.tex, [gx, gy, gx + l.w as f32, gy + l.h as f32]));
                self.hits.menu_pane_go = Some(go);
            } else {
                let fit = ((py1 - 12.0 * s - top) / lh).floor().max(1.0) as usize;
                // (a list too long for the pane ends in how many more there are)
                let shown = if n > fit { fit.saturating_sub(1) } else { n };
                let time_w = p
                    .rows
                    .iter()
                    .take(shown)
                    .map(|row| self.text.width(&row.1, rpx as f32))
                    .fold(0.0f32, f32::max);
                let tp = format!("{} ", omsi_ui::tr("Tour"));
                // (the tour numbers in tiles of one width, the destination after them)
                let tile_w = p
                    .rows
                    .iter()
                    .take(shown)
                    .filter_map(|row| {
                        row.0
                            .strip_prefix(tp.as_str())
                            .and_then(|x| x.split_once("  ›  ").map(|(n, _)| n).or(Some(x)))
                    })
                    .map(|n| self.text.width(n.trim(), rpx as f32) + 16.0 * s)
                    .fold(30.0 * s, f32::max);
                for (i, (what, when)) in p.rows.iter().take(shown).enumerate() {
                    let ry = top + lh * i as f32 + lh * 0.5;
                    self.put_right(r, scene, when, rpx, accent_ink(), px1 - pad, ry);
                    if let Some(rest) = what.strip_prefix(tp.as_str()) {
                        let (num, dest) = rest.split_once("  ›  ").unwrap_or((rest, ""));
                        let th = lh - 6.0 * s;
                        self.text.rounded(
                            r,
                            scene,
                            [px0 + pad, ry - th * 0.5, px0 + pad + tile_w, ry + th * 0.5],
                            5.0 * s,
                            accent_soft(),
                        );
                        let nw = self.text.width(num.trim(), rpx as f32);
                        self.put(
                            r,
                            scene,
                            num.trim(),
                            rpx,
                            txt(accent()),
                            px0 + pad + (tile_w - nw) * 0.5,
                            ry,
                        );
                        let dx = px0 + pad + tile_w + 10.0 * s;
                        let dest = clip_to(
                            &self.text,
                            dest,
                            rpx as f32,
                            px1 - pad - time_w - 14.0 * s - dx,
                        );
                        self.put(r, scene, &dest, rpx, SOFT, dx, ry);
                    } else {
                        let what = clip_to(&self.text, what, rpx as f32, inner - time_w - 14.0 * s);
                        self.put(r, scene, &what, rpx, SOFT, px0 + pad, ry);
                    }
                }
                if n > shown {
                    let ry = top + lh * shown as f32 + lh * 0.5;
                    self.put(
                        r,
                        scene,
                        &omsi_ui::tr("+{} more").replacen("{}", &(n - shown).to_string(), 1),
                        (12.0 * s) as u32,
                        MUTED,
                        px0 + pad,
                        ry,
                    );
                }
            }
        }
        self.menu_overlay_range = overlay_start..scene.overlays.len();
    }
}

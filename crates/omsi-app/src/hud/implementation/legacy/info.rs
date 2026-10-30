//! The information drawn over the picture: the frame rate, the information bar, the
//! timetable window and a tutorial page.

use super::super::super::domain::{Frame, InfoTone};
use super::*;
use crate::hud::interface::output::{Renderer, Scene, TextureId};

/// One readout of an information bar's second tier, laid out.
struct TierCell {
    items: Vec<(Label, InfoTone, f32)>,
    pub(super) w: f32,
    pub(super) h: f32,
    meter: Option<f32>,
    alert: bool,
}

/// The information bar's groups in rows no wider than `room` (`widths`: each group's), each
/// as many groups as fit with `gap` between them; a group wider than the room alone is a row
/// of its own (it ran off both sides of a narrow window, and on a phone it lay under the
/// buttons along the top, #1164).
pub fn info_rows(widths: &[f32], gap: f32, room: f32) -> Vec<Vec<usize>> {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut used = 0.0f32;
    for (k, w) in widths.iter().enumerate() {
        match rows.last_mut() {
            Some(row) if used + gap + w <= room => {
                row.push(k);
                used += gap + w;
            }
            _ => {
                rows.push(vec![k]);
                used = *w;
            }
        }
    }
    rows
}

/// The plate (13 green, 14 amber, 15 red) of the dot beside a frame rate.
pub fn fps_dot(fps: f32) -> u8 {
    fps_dot_level(fps_level(fps))
}

/// 0 keeps up (50 and over), 1 slow (30 to 49), 2 struggles.
pub fn fps_level(fps: f32) -> u8 {
    if fps >= 50.0 {
        0
    } else if fps >= 30.0 {
        1
    } else {
        2
    }
}

fn fps_dot_level(level: u8) -> u8 {
    13 + level.min(2)
}

impl Ui {
    /// The frame rate, top right, with a dot of its tone.
    pub(super) fn draw_fps(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame, s: f32) {
        if let Some((fps, low)) = f.fps {
            let text = match low {
                Some(low) => format!("{fps:.0} fps   ·   {} {low:.0}", omsi_ui::tr("1 % low")),
                None => format!("{fps:.0} fps"),
            };
            let l = self
                .text
                .label(r, scene, &text, (13.0 * s) as u32, [255, 255, 255, 200]);
            let dot = 7.0 * s;
            let x = f.width - l.w as f32 - 12.0 * s - dot - 6.0 * s;
            // (on a plate, as the notes are: over a bright sky the outline alone was not enough)
            let plate = self.text.plate(r, scene, 7);
            scene.overlays.push((
                plate,
                [
                    x - 5.0 * s,
                    10.0 * s,
                    x + l.w as f32 + dot + 11.0 * s,
                    10.0 * s + l.h as f32,
                ],
            ));
            let tone = self.text.plate(r, scene, fps_dot(fps));
            let cy = 10.0 * s + l.h as f32 * 0.5;
            scene
                .overlays
                .push((tone, [x, cy - dot * 0.5, x + dot, cy + dot * 0.5]));
            let tx = x + dot + 6.0 * s;
            scene.overlays.push((
                l.tex,
                [tx, 10.0 * s, tx + l.w as f32, 10.0 * s + l.h as f32],
            ));
        }
    }

    /// The information bar, along the top (or the bottom) in the middle: in as many rows as
    /// the room it has needs.
    pub(super) fn draw_info(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame, s: f32) {
        self.hits.info_rect = None;
        self.hits.info_chevron = None;
        // (nothing to say - every item switched off, a layout that has only the vehicle's items
        // and no vehicle - is no empty plate)
        if let Some(info) = f.info.as_ref().filter(|i| !i.groups.is_empty()) {
            let px = (15.0 * s) as u32;
            let pad = 10.0 * s;
            let gap = 6.0 * s;
            let group_gap = 24.0 * s;
            let [x0, x1, top_y] = f
                .info_room
                .unwrap_or([12.0 * s, f.width - 12.0 * s, 8.0 * s]);
            // (a touch screen's buttons own the corners: the bar stays in the gap between them)
            let bottom = info.bottom && f.info_room.is_none();
            let colour = |tone: InfoTone| match tone {
                InfoTone::Dim => [163, 168, 178, 0],
                InfoTone::Badge | InfoTone::Limit => [26, 17, 0, 0],
                InfoTone::Late => [238, 106, 95, 0],
                InfoTone::Early => [95, 207, 138, 0],
                InfoTone::Plain | InfoTone::Fps(_) => [255, 255, 255, 0],
            };
            // each item as a label and the room it takes
            let groups: Vec<Vec<(Label, InfoTone, f32)>> = info
                .groups
                .iter()
                .map(|g| {
                    g.iter()
                        .map(|it| {
                            let l = if it.raw {
                                self.text.label_raw(r, scene, &it.text, px, colour(it.tone))
                            } else {
                                self.text.label(r, scene, &it.text, px, colour(it.tone))
                            };
                            let w = match it.tone {
                                InfoTone::Badge => l.w as f32 + 12.0 * s,
                                InfoTone::Limit => {
                                    (l.w as f32 + 10.0 * s).max(l.h as f32 + 4.0 * s)
                                }
                                InfoTone::Fps(_) => l.w as f32 + 7.0 * s + 5.0 * s,
                                _ => l.w as f32,
                            };
                            (l, it.tone, w)
                        })
                        .collect()
                })
                .collect();
            let widths: Vec<f32> = groups
                .iter()
                .map(|g| {
                    g.iter().map(|i| i.2).sum::<f32>() + gap * g.len().saturating_sub(1) as f32
                })
                .collect();
            // (the arrow of a two-tier layout takes room at the end of the first tier)
            let chev_w = if info.tiered { 26.0 * s } else { 0.0 };
            let rows = info_rows(&widths, group_gap, (x1 - x0 - pad * 2.0 - chev_w).max(1.0));
            let lh = groups.iter().flatten().map(|i| i.0.h).max().unwrap_or(0) as f32;
            let row_h = lh + 6.0 * s;
            let row_w = |row: &Vec<usize>| {
                row.iter().map(|k| widths[*k]).sum::<f32>() + group_gap * (row.len() - 1) as f32
            };
            let lw = rows.iter().map(row_w).fold(0.0f32, f32::max);
            let (w1, h1) = (
                lw + chev_w + pad * 2.0,
                row_h * rows.len() as f32 + pad * 0.5,
            );
            // --- the second tier: small readouts (a name over a value, a meter under it)
            let small = (12.0 * s) as u32;
            let cells: Vec<TierCell> = info
                .tier2
                .iter()
                .map(|c| {
                    let items: Vec<(Label, InfoTone, f32)> = c
                        .items
                        .iter()
                        .map(|it| {
                            let px_i = if it.tone == InfoTone::Dim { small } else { px };
                            let l = if it.raw {
                                self.text
                                    .label_raw(r, scene, &it.text, px_i, colour(it.tone))
                            } else {
                                self.text.label(r, scene, &it.text, px_i, colour(it.tone))
                            };
                            let w = l.w as f32
                                + if matches!(it.tone, InfoTone::Fps(_)) {
                                    12.0 * s
                                } else {
                                    0.0
                                };
                            (l, it.tone, w)
                        })
                        .collect();
                    let meter_w = if c.meter.is_some() { 48.0 * s } else { 0.0 };
                    let w = items.iter().map(|i| i.2).fold(meter_w, f32::max);
                    let h = items.iter().map(|i| i.0.h as f32).sum::<f32>()
                        + if c.meter.is_some() { 7.0 * s } else { 0.0 };
                    TierCell {
                        items,
                        w,
                        h,
                        meter: c.meter,
                        alert: c.alert,
                    }
                })
                .collect();
            let cell_ws: Vec<f32> = cells.iter().map(|c| c.w).collect();
            let rows2 = info_rows(&cell_ws, group_gap, (x1 - x0 - pad * 2.0).max(1.0));
            let row2_w = |row: &Vec<usize>| {
                row.iter().map(|k| cell_ws[*k]).sum::<f32>() + group_gap * (row.len() - 1) as f32
            };
            let row2_h = cells.iter().map(|c| c.h).fold(0.0f32, f32::max) + 8.0 * s;
            let (w2, h2) = if cells.is_empty() {
                (0.0, 0.0)
            } else {
                (
                    rows2.iter().map(row2_w).fold(0.0f32, f32::max) + pad * 2.0,
                    row2_h * rows2.len() as f32 + pad * 0.5,
                )
            };
            // (stacked, the two tiers are one plate as wide as the wider; split, two plates)
            let gap2 = if info.split && h2 > 0.0 { 4.0 * s } else { 0.0 };
            let (w, h) = if info.split {
                (w1.max(w2), h1 + gap2 + h2)
            } else {
                (w1.max(w2), h1 + h2)
            };
            let x = (x0 + x1 - w) * 0.5;
            let y = if bottom {
                f.height - h - 10.0 * s
            } else {
                top_y
            };
            let p1x = (x0 + x1 - if info.split { w1 } else { w }) * 0.5;
            let p1w = if info.split { w1 } else { w };
            let plate = self.text.plate(r, scene, 3);
            scene.overlays.push((
                plate,
                [p1x, y, p1x + p1w, y + if info.split { h1 } else { h }],
            ));
            let line = self.text.plate(r, scene, 10);
            for (n, row) in rows.iter().enumerate() {
                let ry = y + pad * 0.25 + n as f32 * row_h;
                let mut cx = p1x + (p1w - chev_w - row_w(row)) * 0.5;
                for (i, k) in row.iter().enumerate() {
                    if i > 0 {
                        let hx = cx - group_gap * 0.5;
                        scene.overlays.push((
                            line,
                            [hx, ry + 3.0 * s, hx + s.max(1.0), ry + row_h - 3.0 * s],
                        ));
                    }
                    for (l, tone, iw) in &groups[*k] {
                        let ty = ry + (row_h - l.h as f32) * 0.5;
                        let mut tx = cx + (iw - l.w as f32) * 0.5;
                        match tone {
                            InfoTone::Badge => {
                                let b = self.text.plate(r, scene, 4);
                                scene.overlays.push((
                                    b,
                                    [cx, ty - 1.0 * s, cx + iw, ty + l.h as f32 + 1.0 * s],
                                ));
                            }
                            InfoTone::Limit => {
                                let red = self.text.plate(r, scene, 11);
                                let white = self.text.plate(r, scene, 12);
                                let (t0, t1) = (ty - 2.0 * s, ty + l.h as f32 + 2.0 * s);
                                scene.overlays.push((red, [cx, t0, cx + iw, t1]));
                                scene.overlays.push((
                                    white,
                                    [cx + 2.5 * s, t0 + 2.5 * s, cx + iw - 2.5 * s, t1 - 2.5 * s],
                                ));
                            }
                            InfoTone::Fps(level) => {
                                let d = self.text.plate(r, scene, fps_dot_level(*level));
                                let c = ry + row_h * 0.5;
                                scene
                                    .overlays
                                    .push((d, [cx, c - 3.5 * s, cx + 7.0 * s, c + 3.5 * s]));
                                tx = cx + 12.0 * s;
                            }
                            _ => {}
                        }
                        scene
                            .overlays
                            .push((l.tex, [tx, ty, tx + l.w as f32, ty + l.h as f32]));
                        cx += iw + gap;
                    }
                    cx += group_gap - gap;
                }
            }
            // the arrow: a "v" (details folded away: click to show) or a "^" (click to hide)
            if info.tiered {
                let cr = [p1x + p1w - chev_w, y, p1x + p1w, y + h1];
                if f.cursor.0 >= cr[0]
                    && f.cursor.0 <= cr[2]
                    && f.cursor.1 >= cr[1]
                    && f.cursor.1 <= cr[3]
                {
                    let hl = self.text.plate(r, scene, 5);
                    scene.overlays.push((
                        hl,
                        [
                            cr[0] + 2.0 * s,
                            cr[1] + 3.0 * s,
                            cr[2] - 4.0 * s,
                            cr[3] - 3.0 * s,
                        ],
                    ));
                }
                let mark = self.text.plate(r, scene, 12);
                let u = (2.4 * s).max(1.0);
                let (mx, my) = ((cr[0] + cr[2]) * 0.5 - u * 0.65, y + row_h * 0.5 - u * 0.5);
                for k in -2i32..=2 {
                    let step = if info.collapsed { 2 - k.abs() } else { k.abs() };
                    let (px0, py0) = (mx + k as f32 * u, my + (step as f32 - 1.0) * u);
                    scene
                        .overlays
                        .push((mark, [px0, py0, px0 + u * 1.2, py0 + u * 1.2]));
                }
                self.hits.info_chevron = Some(cr);
            }
            // the second tier
            if h2 > 0.0 {
                let (p2x, p2w, y2) = if info.split {
                    ((x0 + x1 - w2) * 0.5, w2, y + h1 + gap2)
                } else {
                    (p1x, p1w, y + h1)
                };
                if info.split {
                    let plate2 = self.text.plate(r, scene, 3);
                    scene.overlays.push((plate2, [p2x, y2, p2x + p2w, y2 + h2]));
                } else {
                    // (a hairline between the tiers)
                    scene.overlays.push((
                        line,
                        [
                            p2x + pad,
                            y2 - s.max(1.0) * 0.5,
                            p2x + p2w - pad,
                            y2 + s.max(1.0) * 0.5,
                        ],
                    ));
                }
                for (n, row) in rows2.iter().enumerate() {
                    let ry = y2 + pad * 0.25 + n as f32 * row2_h;
                    let mut cx = p2x + (p2w - row2_w(row)) * 0.5;
                    for (i, k) in row.iter().enumerate() {
                        let c = &cells[*k];
                        if i > 0 {
                            let hx = cx - group_gap * 0.5;
                            scene.overlays.push((
                                line,
                                [hx, ry + 4.0 * s, hx + s.max(1.0), ry + row2_h - 4.0 * s],
                            ));
                        }
                        if c.alert {
                            let hot = self.text.plate(r, scene, 16);
                            scene.overlays.push((
                                hot,
                                [
                                    cx - group_gap * 0.4,
                                    ry + 1.0 * s,
                                    cx + c.w + group_gap * 0.4,
                                    ry + row2_h - 1.0 * s,
                                ],
                            ));
                        }
                        let mut ty = ry + (row2_h - c.h) * 0.5;
                        for (l, tone, _) in &c.items {
                            let mut tx = cx;
                            if let InfoTone::Fps(level) = tone {
                                let d = self.text.plate(r, scene, fps_dot_level(*level));
                                let cy = ty + l.h as f32 * 0.5;
                                scene
                                    .overlays
                                    .push((d, [cx, cy - 3.5 * s, cx + 7.0 * s, cy + 3.5 * s]));
                                tx = cx + 12.0 * s;
                            }
                            scene
                                .overlays
                                .push((l.tex, [tx, ty, tx + l.w as f32, ty + l.h as f32]));
                            ty += l.h as f32;
                        }
                        if let Some(level) = c.meter {
                            let track = self.text.plate(r, scene, 10);
                            let fill = self.text.plate(r, scene, if c.alert { 15 } else { 13 });
                            let (mw, mh) = (48.0 * s, 3.0 * s);
                            let my = ty + 4.0 * s;
                            scene.overlays.push((track, [cx, my, cx + mw, my + mh]));
                            if level > 0.0 {
                                scene.overlays.push((
                                    fill,
                                    [cx, my, cx + mw * level.clamp(0.0, 1.0), my + mh],
                                ));
                            }
                        }
                        cx += c.w + group_gap;
                    }
                }
            }
            self.hits.info_rect = Some([x, y, x + w, y + h]);
        }
    }

    /// The timetable window, on the right.
    pub(super) fn draw_timetable(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        f: &Frame,
        s: f32,
        corner_top: f32,
        tutorial_w: f32,
    ) {
        if let Some((title, rows)) = f.timetable.as_ref() {
            let px = (14.0 * s) as u32;
            let lh = px as f32 * 1.55;
            let w = (340.0 * s).min(f.width * 0.4);
            let shown = rows.len().min(((f.height * 0.7) / lh) as usize).max(1);
            // (from the stop before the next one on)
            let next = rows.iter().position(|r| r.2 == 1).unwrap_or(0);
            let first = next.saturating_sub(1).min(rows.len().saturating_sub(shown));
            let h = lh * (shown as f32 + 1.6);
            // (left of a tutorial page, which has the same corner)
            let beside = if f.tutorial.is_some() {
                tutorial_w + 12.0 * s
            } else {
                0.0
            };
            let x = (f.width - w - 16.0 * s - beside).max(16.0 * s);
            let y = corner_top;
            let plate = self.text.plate(r, scene, 3);
            scene.overlays.push((plate, [x, y, x + w, y + h]));
            let t = self.text.label(
                r,
                scene,
                &clip_to(&self.text, title, px as f32 * 1.1, w - 20.0 * s),
                (px as f32 * 1.1) as u32,
                [255, 255, 255, 0],
            );
            scene.overlays.push((
                t.tex,
                [
                    x + 10.0 * s,
                    y + 6.0 * s,
                    x + 10.0 * s + t.w as f32,
                    y + 6.0 * s + t.h as f32,
                ],
            ));
            // (the names start after the widest time: "12:03-05" of a stop with a wait)
            let time_w = rows
                .iter()
                .map(|r| self.text.width(&r.1, px as f32))
                .fold(0.0f32, f32::max)
                .max(40.0 * s);
            let name_x = x + 10.0 * s + time_w + 12.0 * s;
            for (k, (name, time, state)) in rows.iter().skip(first).take(shown).enumerate() {
                let ry = y + lh * (k as f32 + 1.3);
                if *state == 1 {
                    let hl = self.text.plate(r, scene, 5);
                    scene.overlays.push((
                        hl,
                        [
                            x + 4.0 * s,
                            ry - 2.0 * s,
                            x + w - 4.0 * s,
                            ry + lh - 4.0 * s,
                        ],
                    ));
                }
                let color = match state {
                    0 => [140, 140, 140, 0],
                    1 => accent_ink(),
                    _ => [235, 235, 235, 0],
                };
                let tl = self.text.label(r, scene, time, px, color);
                scene.overlays.push((
                    tl.tex,
                    [
                        x + 10.0 * s,
                        ry,
                        x + 10.0 * s + tl.w as f32,
                        ry + tl.h as f32,
                    ],
                ));
                let nl = self.text.label(
                    r,
                    scene,
                    &clip_to(&self.text, name, px as f32, x + w - name_x - 10.0 * s),
                    px,
                    color,
                );
                scene
                    .overlays
                    .push((nl.tex, [name_x, ry, name_x + nl.w as f32, ry + nl.h as f32]));
            }
        }
    }

    /// A tutorial page, on the right.
    pub(super) fn draw_tutorial(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        f: &Frame,
        s: f32,
        corner_top: f32,
        tutorial_w: f32,
    ) {
        if let Some((title, text, image, at, count)) = f.tutorial {
            let w = tutorial_w;
            let x = f.width - w - 16.0 * s;
            let mut y = corner_top;
            let top = y;
            let pad = 14.0 * s;
            let mut items: Vec<(TextureId, [f32; 4])> = Vec::new();
            if let Some(p) = image {
                let entry = self.images.entry(p.to_path_buf()).or_insert_with(|| {
                    omsi_texture::decode_file(p).ok().map(|img| {
                        let (iw, ih) = (img.width, img.height);
                        (r.add_texture(scene, &img, false), iw, ih)
                    })
                });
                if let Some((tex, iw, ih)) = *entry {
                    let dw = w - pad * 2.0;
                    let dh = dw * ih as f32 / iw.max(1) as f32;
                    items.push((tex, [x + pad, y + pad, x + pad + dw, y + pad + dh]));
                    y += dh + pad;
                }
            }
            y += pad * 0.6;
            let tp = (18.0 * s) as u32;
            if !title.is_empty() {
                for line in wrap(&self.text, title, tp as f32, w - pad * 2.0) {
                    let l = self.text.label(r, scene, &line, tp, accent_ink());
                    items.push((l.tex, [x + pad, y, x + pad + l.w as f32, y + l.h as f32]));
                    y += l.h as f32;
                }
                y += 6.0 * s;
            }
            let bp = (14.0 * s) as u32;
            let max_y = f.height - 60.0 * s;
            'text: for para in text.lines() {
                for line in wrap(&self.text, para, bp as f32, w - pad * 2.0) {
                    if y > max_y {
                        break 'text;
                    }
                    let l = self.text.label(r, scene, &line, bp, [235, 235, 235, 0]);
                    items.push((l.tex, [x + pad, y, x + pad + l.w as f32, y + l.h as f32]));
                    y += l.h as f32 * 0.95;
                }
                y += 5.0 * s;
            }
            let tr = |t: &str| omsi_ui::tr(t).into_owned();
            let foot = format!(
                "{} {}/{}   ·   {}   ·   {}   ·   {}",
                tr("Page"),
                at + 1,
                count,
                tr("Enter next"),
                tr("Page Up back"),
                tr("Ctrl+T hide")
            );
            let l = self
                .text
                .label(r, scene, &foot, (12.0 * s) as u32, [150, 150, 150, 0]);
            y += 4.0 * s;
            items.push((l.tex, [x + pad, y, x + pad + l.w as f32, y + l.h as f32]));
            y += l.h as f32 + pad;
            let plate = self.text.plate(r, scene, 3);
            scene.overlays.push((plate, [x, top, x + w, y]));
            scene.overlays.extend(items);
        }
    }
}

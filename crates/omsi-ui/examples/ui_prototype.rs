//! Local prototype of the new interface drawn from the concepts, offscreen, with placeholder
//! data only: `cargo run -p omsi-ui --example ui_prototype -- out_dir [logo.png]`
//! (the logo defaults to `assets/logos/pd.png`)
//! One PNG per screen: launcher (Vehicles, Maps), loading, game menu with the city map,
//! Shift+M map, Alt icon bar and the in-game windows.
//! Layout in design units (the concept is 1672x941); the window scales it uniformly and the
//! flexible columns take the rest of the width.
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::{Atlas, Color, Draw, Fonts, Gpu, Layer, Painter, Rect, Sprite, Weight};
use std::ops::Range;

const BG: Color = Color::hex(0x101215);
const SIDE: Color = Color::hex(0x0c0d10);
const PANEL: Color = Color::hex(0x171a1f);
const PANEL2: Color = Color::hex(0x1e2229);
const LINE: Color = Color::hex(0x2b2f37);
const ROWLINE: Color = Color::hex(0x20242b);
const FG: Color = Color::hex(0xe9eaec);
const MUT: Color = Color::hex(0x9aa0aa);
const ACC: Color = Color::hex(0xf0a30a);
const OK: Color = Color::hex(0x4fc27a);
const WARN: Color = Color::hex(0xe0a030);
const NAVTXT: Color = Color::hex(0xc9ccd2);
const ON_ACC: Color = Color::hex(0x14120a);
/// Opacity of panels over the scene behind them (8% see-through: the 5 to 10% asked for).
const PA: f32 = 0.92;
/// How much of the scene the launcher keeps behind its panels.
const VEIL: f32 = 0.88;

/// The concept's face (Inter) is wider than ours at the same size: text is drawn this much
/// larger to take the same room.
const TS: f32 = 1.16;

struct Cx<'a> {
    ui: Painter,
    atlas: Atlas,
    fonts: &'a Fonts,
    /// (range, layer, texture)
    draws: Vec<(Range<u32>, usize, usize)>,
    mark: u32,
    /// Clip box (x0, y0, x1, y1) of draw layer 1, in design units.
    clip: [f32; 4],
}

impl Cx<'_> {
    /// Close the vertices drawn since the last cut as one draw.
    fn cut(&mut self, layer: usize, tex: usize) {
        let end = self.ui.len();
        if end > self.mark {
            self.draws.push((self.mark..end, layer, tex));
        }
        self.mark = end;
    }
    fn fill(&mut self, r: Rect, c: Color) {
        self.ui.rect(r, c);
    }
    /// A 1px frame inside `r`.
    fn frame(&mut self, r: Rect, c: Color) {
        let t = 1.0;
        self.ui.rect(Rect::new(r.x, r.y, r.w, t), c);
        self.ui.rect(Rect::new(r.x, r.bottom() - t, r.w, t), c);
        self.ui.rect(Rect::new(r.x, r.y + t, t, r.h - 2.0 * t), c);
        self.ui
            .rect(Rect::new(r.right() - t, r.y + t, t, r.h - 2.0 * t), c);
    }
    fn hline(&mut self, x: f32, y: f32, w: f32, c: Color) {
        self.ui.rect(Rect::new(x, y, w, 1.0), c);
    }
    /// Text vertically centred on `cy`, left edge at `x`.
    fn left(&mut self, s: &str, px: f32, w: Weight, x: f32, cy: f32, c: Color) -> f32 {
        let px = px * TS;
        let r = Rect::new(x, cy - px, 900.0, px * 2.0);
        self.ui
            .text_in(&mut self.atlas, self.fonts, s, px, w, r, Align::Left, c)
    }
    fn right(&mut self, s: &str, px: f32, w: Weight, xr: f32, cy: f32, c: Color) -> f32 {
        let px = px * TS;
        let r = Rect::new(xr - 900.0, cy - px, 900.0, px * 2.0);
        self.ui
            .text_in(&mut self.atlas, self.fonts, s, px, w, r, Align::Right, c)
    }
    fn center(&mut self, s: &str, px: f32, w: Weight, cx: f32, cy: f32, c: Color) -> f32 {
        let px = px * TS;
        let r = Rect::new(cx - 450.0, cy - px, 900.0, px * 2.0);
        self.ui
            .text_in(&mut self.atlas, self.fonts, s, px, w, r, Align::Center, c)
    }
    fn width(&self, s: &str, px: f32, w: Weight) -> f32 {
        self.fonts.width(s, px * TS, w)
    }
    /// Letter-spaced text (CSS `letter-spacing`), centred vertically on `cy`.
    #[allow(clippy::too_many_arguments)]
    fn spaced(&mut self, s: &str, px: f32, w: Weight, x: f32, cy: f32, sp: f32, c: Color) -> f32 {
        let px = px * TS;
        let base = cy + self.fonts.cap_height(px, w) * 0.5;
        let mut x = x;
        for ch in s.chars() {
            let t = ch.to_string();
            let adv = self.fonts.width_as_is(&t, px, w);
            self.ui.text_as_is(
                &mut self.atlas,
                self.fonts,
                &t,
                px,
                w,
                Vec2::new(x, base),
                Align::Left,
                c,
            );
            x += adv + sp;
        }
        x
    }
    fn spaced_width(&self, s: &str, px: f32, w: Weight, sp: f32) -> f32 {
        s.chars()
            .map(|ch| self.fonts.width_as_is(&ch.to_string(), px * TS, w) + sp)
            .sum::<f32>()
    }
    fn icon(&mut self, name: &str, cx: f32, cy: f32, size: f32, c: Color) {
        self.ui
            .icon(&mut self.atlas, name, Vec2::new(cx, cy), size, c);
    }
    fn dot(&mut self, cx: f32, cy: f32, c: Color) {
        self.ui.circle(Vec2::new(cx, cy), 4.5, c);
    }
}

/// The placeholder bus of the concept (a 96x54 picture) drawn into `r`.
fn bus(cx: &mut Cx, r: Rect, body: Color) {
    let k = r.w / 96.0;
    let p = |x: f32, y: f32| Vec2::new(r.x + x * k, r.y + y * k);
    cx.fill(r, Color::hex(0x3a3d43));
    cx.ui.rounded(
        Rect::new(r.x + 8.0 * k, r.y + 14.0 * k, 80.0 * k, 26.0 * k),
        4.0 * k,
        body,
    );
    cx.fill(
        Rect::new(r.x + 12.0 * k, r.y + 18.0 * k, 62.0 * k, 10.0 * k),
        Color::hex(0x1d1f23),
    );
    cx.ui.circle(p(26.0, 42.0), 6.0 * k, Color::hex(0x0f1012));
    cx.ui.circle(p(68.0, 42.0), 6.0 * k, Color::hex(0x0f1012));
}

/// CSS `radial-gradient(a, b)` (farthest corner ellipse) over `r`.
fn radial(cx: &mut Cx, r: Rect, a: Color, b: Color) {
    let c = r.center();
    let (rx, ry) = (r.w * 0.5 * 2f32.sqrt(), r.h * 0.5 * 2f32.sqrt());
    let n = 48;
    let mut pts = Vec::new();
    for i in 0..n {
        let t = i as f32 / n as f32 * 4.0;
        let (side, f) = (t as usize, t.fract());
        pts.push(match side {
            0 => Vec2::new(r.x + f * r.w, r.y),
            1 => Vec2::new(r.right(), r.y + f * r.h),
            2 => Vec2::new(r.right() - f * r.w, r.bottom()),
            _ => Vec2::new(r.x, r.bottom() - f * r.h),
        });
    }
    let col = |p: Vec2| {
        let d = (((p.x - c.x) / rx).powi(2) + ((p.y - c.y) / ry).powi(2)).sqrt();
        a.mix(b, d.clamp(0.0, 1.0))
    };
    for i in 0..n {
        let (p, q) = (pts[i], pts[(i + 1) % n]);
        cx.ui.tri(c, p, q, a, col(p), col(q));
    }
}

fn card(cx: &mut Cx, r: Rect, title: &str, note: Option<&str>) {
    cx.fill(r, PANEL.alpha(PA));
    cx.frame(r, LINE);
    cx.spaced(title, 13.0, Weight::Medium, r.x + 19.0, r.y + 23.0, 2.0, FG);
    if let Some(n) = note {
        cx.right(n, 12.0, Weight::Regular, r.right() - 19.0, r.y + 23.0, MUT);
    }
}

fn sidebar(cx: &mut Cx, h: f32, sel: usize) {
    cx.fill(Rect::new(0.0, 0.0, 264.0, h), SIDE.alpha(0.95));
    cx.fill(Rect::new(263.0, 0.0, 1.0, h), LINE);
    logo(cx, 20.0, 20.0, 224.0);
    let items = [
        ("home", "Drive"),
        ("airport_shuttle", "Vehicles"),
        ("map", "Maps"),
        ("extension", "Mods"),
        ("schedule", "Timetable"),
        ("play_arrow", "Sessions"),
        ("person", "Profile"),
        ("sports_esports", "Controls"),
        ("help", "Tutorials"),
        ("monitor", "Diagnostics"),
    ];
    for (i, (icon, label)) in items.iter().enumerate() {
        let top = 119.0 + i as f32 * 56.0;
        let on = i == sel;
        if on {
            cx.fill(Rect::new(0.0, top, 263.0, 56.0), ACC);
        }
        let c = if on { ON_ACC } else { NAVTXT };
        cx.icon(icon, 39.0, top + 28.0, 22.0, c);
        let w = if on { Weight::Medium } else { Weight::Regular };
        cx.left(label, 17.0, w, 68.0, top + 28.0, c);
    }
    // settings and setup, then the version strip, anchored to the bottom
    for (i, (icon, label)) in [("settings", "Settings"), ("folder_open", "Setup")]
        .iter()
        .enumerate()
    {
        let top = h - 141.0 + i as f32 * 48.0;
        cx.icon(icon, 39.0, top + 24.0, 22.0, NAVTXT);
        cx.left(label, 17.0, Weight::Regular, 68.0, top + 24.0, NAVTXT);
    }
    cx.hline(0.0, h - 45.0, 263.0, LINE);
    cx.left("concept", 13.0, Weight::Regular, 28.0, h - 22.0, MUT);
    cx.left("example data", 13.0, Weight::Regular, 96.0, h - 22.0, MUT);
}

fn vehicles(cx: &mut Cx, w: f32, h: f32) {
    backdrop(cx, w, h, 0.5, true);
    sidebar(cx, h, 1);
    let x0 = 292.0;
    let x1 = w - 28.0;
    let cw = x1 - x0;
    // title block
    let mut x = cx.spaced("VEHICLES", 13.0, Weight::Medium, x0, 42.0, 2.0, ACC);
    x += 10.0;
    x = cx.left("\u{203a}", 13.0, Weight::Regular, x, 42.0, MUT) + x + 12.0;
    cx.spaced("PREVIEW", 13.0, Weight::Regular, x, 42.0, 2.0, MUT);
    cx.left(
        "Choose and check your bus",
        40.0,
        Weight::Bold,
        x0,
        78.0,
        FG,
    );
    cx.left(
        "Inspect the bus with its paint and the author's description before starting the map.",
        18.0,
        Weight::Regular,
        x0,
        112.0,
        MUT,
    );

    // top row: 335 / flex / 392
    let top = 137.0;
    let row1 = 418.0;
    let mid = cw - 335.0 - 392.0 - 32.0;
    let lib = Rect::new(x0, top, 335.0, row1);
    let pre = Rect::new(lib.right() + 16.0, top, mid, row1);
    let det = Rect::new(pre.right() + 16.0, top, 392.0, row1);

    // vehicle library
    card(cx, lib, "VEHICLE LIBRARY", None);
    let s = Rect::new(lib.x + 15.0, lib.y + 44.0, lib.w - 30.0, 37.0);
    cx.fill(s, PANEL2);
    cx.frame(s, LINE);
    cx.left(
        "Search vehicles...",
        14.0,
        Weight::Regular,
        s.x + 15.0,
        s.y + 18.5,
        MUT,
    );
    let list = [
        (
            "Example bus A",
            "Vehicle \u{2022} v1.2",
            "Maker One",
            Color::hex(0xf0a30a),
        ),
        (
            "Example bus B",
            "Vehicle \u{2022} v2.3",
            "Maker Two",
            Color::hex(0xe8e8ea),
        ),
        (
            "Example bus C",
            "Vehicle \u{2022} v1.5",
            "Maker Two",
            Color::hex(0xd9d9dc),
        ),
    ];
    for (i, (name, ver, maker, col)) in list.iter().enumerate() {
        let r = Rect::new(
            lib.x + 9.0,
            lib.y + 91.0 + i as f32 * 97.0,
            lib.w - 18.0,
            91.0,
        );
        if i == 0 {
            cx.fill(r, Color::hex(0x201d14));
            cx.frame(r, ACC);
        }
        bus(cx, Rect::new(r.x + 13.0, r.y + 19.0, 96.0, 54.0), *col);
        let tx = r.x + 123.0;
        cx.left(name, 15.0, Weight::Bold, tx, r.y + 19.0, FG);
        cx.left(ver, 13.0, Weight::Regular, tx, r.y + 39.0, MUT);
        cx.left(maker, 13.0, Weight::Regular, tx, r.y + 57.0, MUT);
        cx.dot(tx + 4.5, r.y + 75.0, OK);
        cx.left(
            "Installed",
            13.0,
            Weight::Regular,
            tx + 15.0,
            r.y + 75.0,
            OK,
        );
        cx.center(
            "\u{203a}",
            22.0,
            Weight::Regular,
            r.right() - 19.0,
            r.y + 44.0,
            MUT,
        );
    }

    // preview
    card(
        cx,
        pre,
        "PREVIEW",
        Some("image from cache \u{b7} live view only on this page"),
    );
    let pv = Rect::new(pre.x + 1.0, pre.y + 44.0, pre.w - 2.0, 268.0);
    radial(cx, pv, Color::hex(0x4a4d55), Color::hex(0x26282d));
    let b = Rect::new(pv.x + (pv.w - 470.0) * 0.5, pv.y + 2.0, 470.0, 264.0);
    bus(
        cx,
        Rect::new(b.x, b.y + 1.0, 470.0, 264.5),
        Color::hex(0xf0a30a),
    );
    let tag = Rect::new(pv.x + 12.0, pv.y + 10.0, 114.0, 20.0);
    dashed(cx, tag, LINE);
    cx.left(
        "example placeholder",
        11.0,
        Weight::Regular,
        tag.x + 8.0,
        tag.y + 10.0,
        MUT,
    );
    cx.right(
        "preview helper: running",
        12.0,
        Weight::Regular,
        pv.right() - 12.0,
        pv.y + 18.0,
        OK,
    );
    cx.dot(
        pv.right() - 12.0 - cx.width("preview helper: running", 12.0, Weight::Regular) - 8.0,
        pv.y + 18.0,
        OK,
    );
    let tools = Rect::new(pre.x + 1.0, pv.bottom(), pre.w - 2.0, 43.0);
    cx.hline(tools.x, tools.y, tools.w, LINE);
    let names = ["Orbit", "Pan", "Zoom", "Reset view", "Lighting"];
    let tw = tools.w / 5.0;
    for (i, n) in names.iter().enumerate() {
        let cxm = tools.x + tw * (i as f32 + 0.5);
        if i < 4 {
            cx.fill(
                Rect::new(
                    tools.x + tw * (i as f32 + 1.0) - 1.0,
                    tools.y + 1.0,
                    1.0,
                    tools.h - 1.0,
                ),
                LINE,
            );
        }
        if i == 0 {
            cx.fill(
                Rect::new(tools.x + tw * i as f32, tools.bottom() - 2.0, tw - 1.0, 2.0),
                ACC,
            );
        }
        cx.center(
            n,
            13.0,
            Weight::Regular,
            cxm,
            tools.y + 21.0,
            if i == 0 { ACC } else { MUT },
        );
    }

    // details
    card(cx, det, "VEHICLE DETAILS", None);
    let dy = det.y + 45.0;
    bus(
        cx,
        Rect::new(det.x + 19.0, dy, 64.0, 36.0),
        Color::hex(0xf0a30a),
    );
    cx.left(
        "Example bus A",
        17.0,
        Weight::Bold,
        det.x + 95.0,
        dy + 9.0,
        FG,
    );
    cx.left(
        "Maker One, 2-door",
        13.0,
        Weight::Regular,
        det.x + 95.0,
        dy + 28.0,
        MUT,
    );
    let chip = Rect::new(det.right() - 19.0 - 94.0, dy + 1.0, 94.0, 30.0);
    cx.fill(chip, Color::hex(0x12261a));
    cx.frame(chip, OK);
    cx.center(
        "Files found",
        13.0,
        Weight::Regular,
        chip.x + chip.w * 0.5,
        chip.y + 15.0,
        OK,
    );
    let kv = [
        ("Author", "Author name"),
        ("Category", "City bus"),
        ("Scripts", "23 files"),
        ("Textures", "512 files"),
        ("Sound set", "found"),
    ];
    let ky = det.y + 94.0;
    for (i, (k, v)) in kv.iter().enumerate() {
        let y = ky + i as f32 * 29.0;
        cx.hline(det.x + 1.0, y, det.w - 2.0, ROWLINE);
        cx.left(k, 13.0, Weight::Regular, det.x + 19.0, y + 14.5, MUT);
        cx.left(v, 13.0, Weight::Regular, det.x + 149.0, y + 14.5, FG);
    }
    cx.spaced(
        "AUTHOR'S DESCRIPTION",
        12.0,
        Weight::Medium,
        det.x + 19.0,
        det.y + 260.0,
        2.0,
        MUT,
    );
    let desc = Rect::new(det.x + 15.0, det.y + 275.0, det.w - 30.0, 128.0);
    cx.fill(desc, PANEL2);
    cx.frame(desc, LINE);
    cx.cut(0, 0);
    let lines: [(&str, bool); 8] = [
        ("Technical sheet", true),
        ("Engine, power, gearbox, maximum speed, length and", false),
        ("weight, as the author wrote them (example text).", false),
        ("", false),
        ("Credits", true),
        ("Modelling and conversion, as credited by the author", false),
        ("(example text).", false),
        ("", false),
    ];
    for (i, (t, bold)) in lines.iter().enumerate() {
        let y = desc.y + 11.0 + 9.0 + i as f32 * 18.2;
        let wt = if *bold { Weight::Bold } else { Weight::Regular };
        cx.left(t, 13.0, wt, desc.x + 15.0, y, FG);
    }
    // the box clips its text
    cx.clip = [
        desc.x + 1.0,
        desc.y + 1.0,
        desc.right() - 1.0,
        desc.bottom() - 1.0,
    ];
    cx.cut(1, 0);

    // second row: flex / 330 / flex
    let r2y = top + row1 + 14.0;
    let r2h = (h - 94.0 - r2y).max(200.0);
    // (the concept's `1fr 330px 1fr` grows the first column to its four tiles)
    let pw = 564.0;
    let fw = cw - pw - 330.0 - 32.0;
    let paints = Rect::new(x0, r2y, pw, r2h);
    let files = Rect::new(paints.right() + 16.0, r2y, 330.0, r2h);
    let launch = Rect::new(files.right() + 16.0, r2y, fw, r2h);

    card(cx, paints, "PAINTS", Some("4 of 142 (example)"));
    let tiles = [
        ("Default paint", Color::hex(0xf0a30a)),
        ("Paint 2", Color::hex(0x3b7dd8)),
        ("Paint 3", Color::hex(0xd94a4a)),
        ("Paint 4", Color::hex(0xe8e8ea)),
    ];
    for (i, (n, col)) in tiles.iter().enumerate() {
        let r = Rect::new(
            paints.x + 19.0 + i as f32 * 134.0,
            paints.y + 45.0,
            124.0,
            93.0,
        );
        if i == 0 {
            cx.frame(r, ACC);
        }
        bus(cx, Rect::new(r.x + 3.0, r.y + 3.0, 118.0, 66.4), *col);
        cx.center(
            n,
            12.0,
            Weight::Regular,
            r.x + r.w * 0.5,
            r.y + 83.0,
            if i == 0 { FG } else { MUT },
        );
    }

    card(cx, files, "FILES FOUND", None);
    let checks = [
        ("Model file", "found", OK),
        ("Textures", "0 missing", OK),
        ("Sound set", "found", OK),
        ("Scripts", "read without errors", OK),
        ("Depot file", "not found next to bus", WARN),
    ];
    for (i, (n, v, c)) in checks.iter().enumerate() {
        let y = files.y + 45.0 + i as f32 * 30.0;
        cx.hline(files.x + 1.0, y, files.w - 2.0, ROWLINE);
        cx.dot(files.x + 23.0 + 4.5, y + 16.0, *c);
        cx.left(n, 14.0, Weight::Regular, files.x + 46.0, y + 16.0, FG);
        cx.right(
            v,
            12.0,
            Weight::Regular,
            files.right() - 19.0,
            y + 16.0,
            MUT,
        );
    }

    card(cx, launch, "LAUNCH PREPARATION", None);
    let rows = [
        ("Bus", "Example bus A"),
        ("Map", "Map name (example)"),
        ("Profile", "Default driver"),
        ("Fleet number", "50044"),
    ];
    for (i, (k, v)) in rows.iter().enumerate() {
        let y = launch.y + 45.0 + i as f32 * 41.0;
        cx.hline(launch.x + 1.0, y, launch.w - 2.0, ROWLINE);
        cx.left(k, 13.0, Weight::Regular, launch.x + 19.0, y + 20.5, MUT);
        let f = Rect::new(launch.x + 137.0, y + 6.0, launch.w - 137.0 - 19.0, 30.0);
        cx.fill(f, PANEL2);
        cx.frame(f, LINE);
        cx.left(v, 13.0, Weight::Regular, f.x + 11.0, f.y + 15.0, FG);
    }
    let go = Rect::new(
        launch.x + 19.0,
        launch.y + 45.0 + 4.0 * 41.0 + 9.0,
        launch.w - 38.0,
        48.0,
    );
    cx.fill(go, ACC);
    let gw = cx.spaced_width("START DRIVING", 17.0, Weight::Bold, 0.5);
    cx.spaced(
        "START DRIVING",
        17.0,
        Weight::Bold,
        go.x + (go.w - gw) * 0.5,
        go.y + 24.0,
        0.5,
        ON_ACC,
    );

    // status chips
    let mut x = x0;
    for t in [
        "Game: not running",
        "Content folder: read",
        "Preview helper: separate process, stops when this page closes",
    ] {
        let w = cx.width(t, 12.6, Weight::Regular) + 22.0;
        let r = Rect::new(x, h - 8.0 - 24.0, w, 24.0);
        pill(cx, r, LINE);
        cx.left(t, 12.6, Weight::Regular, r.x + 11.0, r.y + 12.0, MUT);
        x += w + 14.0;
    }
}

fn dashed(cx: &mut Cx, r: Rect, c: Color) {
    let mut x = r.x;
    while x < r.right() {
        let w = 3.0f32.min(r.right() - x);
        cx.ui.rect(Rect::new(x, r.y, w, 1.0), c);
        cx.ui.rect(Rect::new(x, r.bottom() - 1.0, w, 1.0), c);
        x += 5.0;
    }
    let mut y = r.y;
    while y < r.bottom() {
        let h = 3.0f32.min(r.bottom() - y);
        cx.ui.rect(Rect::new(r.x, y, 1.0, h), c);
        cx.ui.rect(Rect::new(r.right() - 1.0, y, 1.0, h), c);
        y += 5.0;
    }
}

fn pill(cx: &mut Cx, r: Rect, c: Color) {
    cx.ui.rounded_border(r, 12.0, 1.0, c);
}

// ---------------------------------------------------------------------------------------
// shared pieces

/// Height over width of the logo picture, set once when it is loaded.
static LOGO_ASPECT: std::sync::OnceLock<f32> = std::sync::OnceLock::new();

fn logo(cx: &mut Cx, x: f32, y: f32, w: f32) {
    cx.cut(0, 0);
    let s = Sprite {
        uv: [0.0, 0.0, 1.0, 1.0],
        w,
        h: w * LOGO_ASPECT.get().copied().unwrap_or(0.25),
        ascent: 0.0,
    };
    cx.ui
        .sprite(s, Vec2::new(x, y), Vec2::new(s.w, s.h), Color::WHITE);
    cx.cut(0, 1);
}

/// The scene behind the interface (placeholder for a map picture or the 3D view): sky, road
/// and ground. `veil` darkens it for the launcher pages.
fn backdrop(cx: &mut Cx, w: f32, h: f32, horizon: f32, veil: bool) {
    let hy = h * horizon;
    cx.ui.gradient(
        Rect::new(0.0, 0.0, w, hy),
        Color::hex(0x172030),
        Color::hex(0x5d7494),
    );
    cx.ui.gradient(
        Rect::new(0.0, hy, w, h - hy),
        Color::hex(0x3d3f42),
        Color::hex(0x121315),
    );
    cx.ui.convex(
        &[
            Vec2::new(w * 0.31, hy),
            Vec2::new(w * 0.70, hy),
            Vec2::new(w * 0.95, h),
            Vec2::new(w * 0.10, h),
        ],
        Color::hex(0x55585c).alpha(0.35),
    );
    if veil {
        cx.fill(Rect::new(0.0, 0.0, w, h), BG.alpha(VEIL));
    }
}

fn btn(cx: &mut Cx, r: Rect, label: &str, primary: bool) {
    if primary {
        cx.fill(r, ACC);
        cx.center(
            label,
            14.0,
            Weight::Bold,
            r.x + r.w * 0.5,
            r.y + r.h * 0.5,
            ON_ACC,
        );
    } else {
        cx.fill(r, PANEL2);
        cx.frame(r, LINE);
        cx.center(
            label,
            14.0,
            Weight::Regular,
            r.x + r.w * 0.5,
            r.y + r.h * 0.5,
            FG,
        );
    }
}

/// An in-game window: 40px title strip, optional note on the right.
fn win(cx: &mut Cx, r: Rect, title: &str, note: &str) {
    cx.fill(r, PANEL.alpha(PA));
    cx.frame(r, LINE);
    cx.spaced(title, 13.0, Weight::Medium, r.x + 17.0, r.y + 20.0, 2.0, FG);
    cx.right(
        note,
        12.0,
        Weight::Regular,
        r.right() - 17.0,
        r.y + 20.0,
        MUT,
    );
    cx.hline(r.x + 1.0, r.y + 40.0, r.w - 2.0, ROWLINE);
}

/// The strip across the top of the game: route, next stop, time, hints.
fn game_bar(cx: &mut Cx, w: f32, doors: bool, hint: &str) {
    cx.fill(Rect::new(0.0, 0.0, w, 40.0), SIDE.alpha(0.96));
    let mut x = 20.0;
    x += cx.left(
        "Line 50 \u{b7} Example route",
        14.0,
        Weight::Bold,
        x,
        20.0,
        FG,
    ) + 28.0;
    x += cx.left("Next stop: ", 14.0, Weight::Regular, x, 20.0, MUT);
    x += cx.left("Central Station", 14.0, Weight::Bold, x, 20.0, FG) + 28.0;
    x += cx.left("Time ", 14.0, Weight::Regular, x, 20.0, MUT);
    x += cx.left("08:42", 14.0, Weight::Bold, x, 20.0, FG) + 28.0;
    if doors {
        cx.left("Doors: open", 14.0, Weight::Regular, x, 20.0, ACC);
    }
    cx.right(hint, 13.0, Weight::Regular, w - 20.0, 20.0, MUT);
}

fn lcg(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    ((*seed >> 8) & 0xffff) as f32 / 65535.0
}

/// A schematic street grid in `r`: slightly crooked streets over a block colour.
fn streets(cx: &mut Cx, r: Rect, block: Color, street: Color, wd: f32, seed: u32) {
    let mut s = seed;
    cx.fill(r, block);
    let mut x = r.x + 10.0;
    while x < r.right() {
        let (a, b) = (lcg(&mut s) * 14.0 - 7.0, lcg(&mut s) * 14.0 - 7.0);
        cx.ui.line(
            Vec2::new((x + a).max(r.x), r.y),
            Vec2::new((x + b).min(r.right()), r.bottom()),
            wd,
            street,
        );
        x += 120.0 + lcg(&mut s) * 60.0;
    }
    let mut y = r.y + 8.0;
    while y < r.bottom() {
        let (a, b) = (lcg(&mut s) * 20.0 - 10.0, lcg(&mut s) * 20.0 - 10.0);
        cx.ui.line(
            Vec2::new(r.x, (y + a).max(r.y)),
            Vec2::new(r.right(), (y + b).min(r.bottom())),
            wd,
            street,
        );
        y += 105.0 + lcg(&mut s) * 50.0;
    }
}

/// A smooth thick path through `pts` (Catmull-Rom).
fn path(cx: &mut Cx, pts: &[Vec2], wd: f32, c: Color) {
    let p = |i: isize| pts[i.clamp(0, pts.len() as isize - 1) as usize];
    let mut prev = pts[0];
    for i in 0..pts.len() as isize - 1 {
        for k in 1..=16 {
            let t = k as f32 / 16.0;
            let (p0, p1, p2, p3) = (p(i - 1), p(i), p(i + 1), p(i + 2));
            let q = 0.5
                * (2.0 * p1
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t * t * t);
            cx.ui.line(prev, q, wd, c);
            cx.ui.circle(q, wd * 0.5, c);
            prev = q;
        }
    }
}

fn ring(cx: &mut Cx, at: Vec2, r: f32, c: Color, fill: Color) {
    cx.ui.circle(at, r, c);
    cx.ui.circle(at, r - 2.5, fill);
}

// ---------------------------------------------------------------------------------------
// launcher: Maps

fn maps(cx: &mut Cx, w: f32, h: f32) {
    backdrop(cx, w, h, 0.5, true);
    sidebar(cx, h, 2);
    let x0 = 292.0;
    let cw = w - 28.0 - x0;
    let mut x = cx.spaced("MAPS", 13.0, Weight::Medium, x0, 42.0, 2.0, ACC);
    x += 10.0;
    x = cx.left("\u{203a}", 13.0, Weight::Regular, x, 42.0, MUT) + x + 12.0;
    cx.spaced("MAP & DUTY", 13.0, Weight::Regular, x, 42.0, 2.0, MUT);
    cx.left("Choose your map and duty", 40.0, Weight::Bold, x0, 78.0, FG);
    cx.left(
        "Check the map and its description, then pick where to start and which line to drive.",
        18.0,
        Weight::Regular,
        x0,
        112.0,
        MUT,
    );
    let top = 137.0;
    let row1 = 418.0;
    let lib = Rect::new(x0, top, 335.0, row1);
    let mid = Rect::new(lib.right() + 16.0, top, cw - 335.0 - 392.0 - 32.0, row1);
    let files = Rect::new(mid.right() + 16.0, top, 392.0, row1);

    card(cx, lib, "MAP LIBRARY", None);
    let s = Rect::new(lib.x + 15.0, lib.y + 44.0, lib.w - 30.0, 37.0);
    cx.fill(s, PANEL2);
    cx.frame(s, LINE);
    cx.left(
        "Search maps...",
        14.0,
        Weight::Regular,
        s.x + 15.0,
        s.y + 18.5,
        MUT,
    );
    let list = [
        ("Example map", "Map \u{2022} v1.0", "Maker One"),
        ("Example city 2", "Map \u{2022} v2.1", "Maker Two"),
        ("Example town", "Map \u{2022} v0.9", "Maker Three"),
    ];
    for (i, (name, ver, maker)) in list.iter().enumerate() {
        let r = Rect::new(
            lib.x + 9.0,
            lib.y + 91.0 + i as f32 * 97.0,
            lib.w - 18.0,
            91.0,
        );
        if i == 0 {
            cx.fill(r, Color::hex(0x201d14));
            cx.frame(r, ACC);
        }
        let t = Rect::new(r.x + 13.0, r.y + 19.0, 96.0, 54.0);
        cx.fill(t, Color::hex(0x2c3d44));
        cx.ui.line(
            Vec2::new(t.x + 10.0, t.y + 38.0),
            Vec2::new(t.right() - 8.0, t.y + 16.0),
            4.0,
            Color::hex(0x6b7b83),
        );
        cx.ui.line(
            Vec2::new(t.x + 40.0, t.y + 6.0),
            Vec2::new(t.x + 58.0, t.bottom() - 6.0),
            3.0,
            Color::hex(0x6b7b83),
        );
        let tx = r.x + 123.0;
        cx.left(name, 15.0, Weight::Bold, tx, r.y + 19.0, FG);
        cx.left(ver, 13.0, Weight::Regular, tx, r.y + 39.0, MUT);
        cx.left(maker, 13.0, Weight::Regular, tx, r.y + 57.0, MUT);
        cx.dot(tx + 4.5, r.y + 75.0, OK);
        cx.left(
            "Installed",
            13.0,
            Weight::Regular,
            tx + 15.0,
            r.y + 75.0,
            OK,
        );
        cx.center(
            "\u{203a}",
            22.0,
            Weight::Regular,
            r.right() - 19.0,
            r.y + 44.0,
            MUT,
        );
    }

    card(cx, mid, "MAP", Some("MAP NAME (EXAMPLE)"));
    let pv = Rect::new(mid.x + 1.0, mid.y + 44.0, mid.w - 2.0, mid.h - 45.0);
    cx.ui
        .gradient(pv, Color::hex(0x2c3d44), Color::hex(0x1f2a30));
    let inner = Rect::new(
        pv.x + (pv.w - 480.0) * 0.5,
        pv.y + 60.0,
        480.0,
        pv.h - 120.0,
    );
    cx.fill(inner, Color::hex(0x34454c));
    let road = Color::hex(0x6b7b83);
    cx.ui.line(
        Vec2::new(inner.x + 8.0, inner.bottom() - 30.0),
        Vec2::new(inner.right() - 8.0, inner.y + 50.0),
        9.0,
        road,
    );
    cx.ui.line(
        Vec2::new(inner.x + 120.0, inner.y),
        Vec2::new(inner.x + 190.0, inner.bottom()),
        7.0,
        road,
    );
    cx.ui.line(
        Vec2::new(inner.x + 330.0, inner.y),
        Vec2::new(inner.x + 355.0, inner.bottom()),
        7.0,
        road,
    );
    for (bx, by, bw, bh) in [
        (40.0, 20.0, 60.0, 40.0),
        (360.0, 20.0, 70.0, 40.0),
        (205.0, 120.0, 50.0, 50.0),
    ] {
        cx.fill(
            Rect::new(inner.x + bx, inner.y + by, bw, bh),
            Color::hex(0x3f535b),
        );
    }
    let tag = Rect::new(pv.x + 12.0, pv.y + 10.0, 262.0, 20.0);
    dashed(cx, tag, LINE);
    cx.left(
        "picture from the map's folder (example placeholder)",
        11.0,
        Weight::Regular,
        tag.x + 8.0,
        tag.y + 10.0,
        MUT,
    );

    card(cx, files, "FILES FOUND", None);
    let checks = [
        ("Map picture", "found", OK),
        ("Map description", "found (language of settings)", OK),
        ("Depot file", "not found", WARN),
        ("Timetables", "26 lines read", OK),
    ];
    for (i, (n, v, c)) in checks.iter().enumerate() {
        let y = files.y + 45.0 + i as f32 * 30.0;
        cx.hline(files.x + 1.0, y, files.w - 2.0, ROWLINE);
        cx.dot(files.x + 23.0 + 4.5, y + 16.0, *c);
        cx.left(n, 14.0, Weight::Regular, files.x + 46.0, y + 16.0, FG);
        cx.right(
            v,
            12.0,
            Weight::Regular,
            files.right() - 19.0,
            y + 16.0,
            MUT,
        );
    }

    let r2y = top + row1 + 14.0;
    let r2h = (h - 94.0 - r2y).max(200.0);
    let fw = (cw - 392.0 - 32.0) * 0.5;
    let a = Rect::new(x0, r2y, fw, r2h);
    let b = Rect::new(a.right() + 16.0, r2y, fw, r2h);
    let l = Rect::new(b.right() + 16.0, r2y, 392.0, r2h);
    card(cx, a, "ABOUT THE SELECTED MAP", None);
    for (i, t) in [
        "Description text of the map, read from its folder in",
        "the language of the settings. Shown as plain text,",
        "length-limited. (example text)",
        "",
        "Lines: 26 \u{b7} Stops: 412",
    ]
    .iter()
    .enumerate()
    {
        cx.left(
            t,
            13.0,
            Weight::Regular,
            a.x + 19.0,
            a.y + 66.0 + i as f32 * 21.0,
            FG,
        );
    }
    card(cx, b, "ABOUT THE SELECTED TIME", None);
    for (i, t) in [
        "Date and time of the chosen situation, and the text",
        "that comes with it, when there is one. (example text)",
    ]
    .iter()
    .enumerate()
    {
        cx.left(
            t,
            13.0,
            Weight::Regular,
            b.x + 19.0,
            b.y + 66.0 + i as f32 * 21.0,
            MUT,
        );
    }
    card(cx, l, "LAUNCH PREPARATION", None);
    let rows = [
        ("Map", "Map name (example)"),
        ("Entry point", "Terminal (example)"),
        ("Line / tour", "Line 1 \u{b7} tour 3 (example)"),
    ];
    for (i, (k, v)) in rows.iter().enumerate() {
        let y = l.y + 45.0 + i as f32 * 41.0;
        cx.hline(l.x + 1.0, y, l.w - 2.0, ROWLINE);
        cx.left(k, 13.0, Weight::Regular, l.x + 19.0, y + 20.5, MUT);
        let f = Rect::new(l.x + 137.0, y + 6.0, l.w - 137.0 - 19.0, 30.0);
        cx.fill(f, PANEL2);
        cx.frame(f, LINE);
        cx.left(v, 13.0, Weight::Regular, f.x + 11.0, f.y + 15.0, FG);
    }
    let go = Rect::new(l.x + 19.0, l.y + 45.0 + 3.0 * 41.0 + 9.0, l.w - 38.0, 48.0);
    cx.fill(go, ACC);
    let gw = cx.spaced_width("START DRIVING", 17.0, Weight::Bold, 0.5);
    cx.spaced(
        "START DRIVING",
        17.0,
        Weight::Bold,
        go.x + (go.w - gw) * 0.5,
        go.y + 24.0,
        0.5,
        ON_ACC,
    );
}

// ---------------------------------------------------------------------------------------
// loading

fn loading(cx: &mut Cx, w: f32, h: f32) {
    backdrop(cx, w, h, 0.46, false);
    cx.ui.gradient(
        Rect::new(0.0, h * 0.6, w, h * 0.4),
        Color::BLACK.alpha(0.0),
        Color::BLACK.alpha(0.55),
    );
    logo(cx, 40.0, 32.0, 260.0);
    cx.right(
        "EXAMPLE VERSION",
        11.0,
        Weight::Regular,
        w - 41.0,
        47.0,
        MUT,
    );
    cx.left(
        "Example map \u{b7} Line 50 \u{b7} 08:00",
        15.0,
        Weight::Bold,
        40.0,
        h - 110.0,
        FG,
    );
    cx.left(
        "Loading map tiles (first area) \u{b7} 62 of 100",
        13.0,
        Weight::Regular,
        40.0,
        h - 89.0,
        MUT,
    );
    let bar = Rect::new(40.0, h - 70.0, 640.0, 8.0);
    cx.fill(bar, Color::hex(0x44464a));
    cx.fill(Rect::new(bar.x, bar.y, bar.w * 0.62, bar.h), ACC);
    cx.left(
        "Preparing the world",
        12.0,
        Weight::Regular,
        40.0,
        h - 47.0,
        MUT,
    );
    cx.right("62%", 12.0, Weight::Regular, 680.0, h - 47.0, MUT);

    let p = Rect::new(w - 440.0, h - 307.0, 400.0, 267.0);
    cx.fill(p, PANEL.alpha(PA));
    cx.frame(p, LINE);
    cx.spaced(
        "LOADING",
        11.0,
        Weight::Medium,
        p.x + 18.0,
        p.y + 18.0,
        2.0,
        MUT,
    );
    cx.hline(p.x + 1.0, p.y + 35.0, p.w - 2.0, LINE);
    let items = [
        (
            "bus image",
            "Example bus A",
            "Example Maker \u{b7} v1.2 \u{b7} Default paint",
            [
                "Author's description, first lines as written",
                "by the author (example text).",
            ],
        ),
        (
            "map picture",
            "Example map",
            "Example City",
            [
                "Map description from the map folder",
                "(example text, a few lines).",
            ],
        ),
    ];
    for (i, (ph, t, sub, d)) in items.iter().enumerate() {
        let y = p.y + 36.0 + i as f32 * 115.0;
        if i > 0 {
            cx.hline(p.x + 1.0, y, p.w - 2.0, LINE);
        }
        let th = Rect::new(p.x + 19.0, y + 15.0, 96.0, 60.0);
        cx.fill(th, PANEL2);
        cx.center(
            ph,
            11.0,
            Weight::Regular,
            th.x + th.w * 0.5,
            th.y + 30.0,
            MUT,
        );
        let tx = p.x + 129.0;
        cx.left(t, 19.0, Weight::Bold, tx, y + 26.0, FG);
        cx.left(sub, 12.0, Weight::Regular, tx, y + 49.0, ACC);
        cx.left(d[0], 13.0, Weight::Regular, tx, y + 72.0, MUT);
        cx.left(d[1], 13.0, Weight::Regular, tx, y + 91.0, MUT);
    }
}

// ---------------------------------------------------------------------------------------
// game menu with the city map (concept "pause") and the Shift+M map

fn game_menu(cx: &mut Cx, h: f32, step: f32, head0: f32) {
    cx.fill(Rect::new(0.0, 0.0, 340.0, h), SIDE.alpha(0.96));
    cx.fill(Rect::new(339.0, 0.0, 1.0, h), LINE);
    logo(cx, 22.0, 22.0, 220.0);
    cx.spaced(
        "GAME MENU",
        12.0,
        Weight::Regular,
        22.0,
        head0 - 33.0,
        2.0,
        MUT,
    );
    type Item = (&'static str, &'static str, &'static str);
    let sections: [(&str, [Item; 6]); 3] = [
        (
            "SESSION",
            [
                ("play_arrow", "Resume", "Esc"),
                ("route", "Line and tour...", ""),
                ("turn_slight_right", "Skip the next stop", ""),
                ("stop_circle", "End the tour", ""),
                ("", "", ""),
                ("", "", ""),
            ],
        ),
        (
            "SETTINGS",
            [
                ("settings", "Options...", ""),
                ("keyboard", "Controls...", ""),
                ("videocam", "Camera...", ""),
                ("directions_bus", "Vehicle options...", ""),
                ("cloud", "World options...", ""),
                ("map", "City map", "Shift+M"),
            ],
        ),
        (
            "GAME",
            [
                ("save", "Save the situation", ""),
                ("download", "Save to a new slot", ""),
                ("upload", "Load the quicksave", ""),
                ("photo_camera", "Screenshot", ""),
                ("power_settings_new", "End the session", ""),
                ("", "", ""),
            ],
        ),
    ];
    let mut head = head0;
    for (name, items) in sections.iter() {
        cx.spaced(name, 11.0, Weight::Regular, 22.0, head, 2.0, MUT);
        let mut top = head + 13.0;
        for (icon, label, key) in items.iter().filter(|i| !i.1.is_empty()) {
            let r = Rect::new(10.0, top, 319.0, step);
            let resume = *label == "Resume";
            let citymap = *label == "City map";
            let danger = *label == "End the session";
            if resume {
                cx.fill(r, ACC);
            } else if citymap {
                cx.fill(r, Color::hex(0x2a2518));
                cx.fill(Rect::new(r.x, r.y, 3.0, r.h), ACC);
            }
            let c = if resume {
                ON_ACC
            } else if danger {
                Color::hex(0xe8523f)
            } else {
                FG
            };
            cx.icon(
                icon,
                45.0,
                r.y + r.h * 0.5,
                14.0,
                if resume {
                    ON_ACC
                } else if danger {
                    c
                } else {
                    MUT
                },
            );
            cx.left(
                label,
                15.0,
                if resume {
                    Weight::Bold
                } else {
                    Weight::Regular
                },
                69.0,
                r.y + r.h * 0.5,
                c,
            );
            cx.right(
                key,
                11.0,
                if resume {
                    Weight::Bold
                } else {
                    Weight::Regular
                },
                r.right() - 21.0,
                r.y + r.h * 0.5,
                if resume { ON_ACC } else { MUT },
            );
            top += step;
        }
        head = top + 18.0;
    }
}

fn stop_pill(cx: &mut Cx, x: f32, y: f32, name: &str, time: &str) {
    let w = cx.width(name, 13.0, Weight::Bold) + cx.width(time, 12.0, Weight::Regular) + 36.0;
    let r = Rect::new(x, y, w, 25.0);
    cx.ui.rounded(r, 12.0, Color::hex(0x14171c).alpha(0.94));
    cx.ui.rounded_border(r, 12.0, 1.0, LINE);
    let a = cx.left(name, 13.0, Weight::Bold, r.x + 13.0, r.y + 12.5, FG);
    cx.left(
        time,
        12.0,
        Weight::Regular,
        r.x + 13.0 + a + 10.0,
        r.y + 12.5,
        MUT,
    );
}

fn pause_map(cx: &mut Cx, w: f32, h: f32) {
    cx.fill(Rect::new(0.0, 0.0, w, h), BG);
    let map = Rect::new(340.0, 45.0, w - 340.0, h - 45.0);
    streets(cx, map, Color::hex(0x1b212b), Color::hex(0x2a2f3a), 13.0, 7);
    cx.fill(Rect::new(340.0, 0.0, w - 340.0, 45.0), Color::hex(0x0e1013));
    cx.left("City map", 14.0, Weight::Bold, 360.0, 23.0, FG);
    cx.left(
        "Line 50 \u{b7} Example route",
        14.0,
        Weight::Regular,
        438.0,
        23.0,
        MUT,
    );
    cx.left("08:42", 14.0, Weight::Regular, 605.0, 23.0, MUT);
    cx.spaced("PAUSED", 13.0, Weight::Medium, w - 83.0, 23.0, 2.0, ACC);
    let pts = [
        Vec2::new(360.0, 760.0),
        Vec2::new(560.0, 732.0),
        Vec2::new(900.0, 590.0),
        Vec2::new(1180.0, 470.0),
        Vec2::new(1440.0, 330.0),
        Vec2::new(1620.0, 180.0),
    ];
    path(cx, &pts, 5.0, ACC);
    for (p, name, dx, dy) in [
        (pts[1], "Depot", 14.0, -16.0),
        (pts[2], "Market", 14.0, -18.0),
        (pts[3], "Main Street", 14.0, -16.0),
        (Vec2::new(1440.0, 330.0), "Central Station", 14.0, -16.0),
    ] {
        ring(cx, p, 9.0, ACC, Color::hex(0x1b212b));
        cx.left(name, 13.0, Weight::Regular, p.x + dx, p.y + dy, FG);
    }
    let me = Vec2::new(1000.0, 560.0);
    cx.ui.circle(me, 12.0, Color::WHITE);
    cx.ui.circle(me, 9.0, OK);
    cx.left(
        "You \u{b7} Example bus A",
        13.0,
        Weight::Regular,
        me.x + 18.0,
        me.y + 1.0,
        FG,
    );
    legend(
        cx,
        w,
        h,
        &[
            ("\u{25cf} You", FG),
            ("\u{25cb} Stops of the tour", FG),
            ("\u{2014} Route", FG),
        ],
        "Mouse wheel: zoom \u{b7} drag: pan",
    );
    game_menu(cx, h, 37.0, 142.0);
}

fn legend(cx: &mut Cx, w: f32, h: f32, items: &[(&str, Color)], hint: &str) {
    let r = Rect::new(w - 268.0, h - 88.0, 248.0, 68.0);
    cx.fill(r, PANEL.alpha(PA));
    cx.frame(r, LINE);
    let mut x = r.x + 14.0;
    for (t, c) in items {
        x += cx.left(t, 12.0, Weight::Regular, x, r.y + 23.0, *c) + 12.0;
    }
    cx.left(hint, 12.0, Weight::Regular, r.x + 14.0, r.y + 46.0, MUT);
}

fn shift_m(cx: &mut Cx, w: f32, h: f32) {
    cx.fill(Rect::new(0.0, 0.0, w, h), Color::hex(0x0e1014));
    let map = Rect::new(340.0, 60.0, w - 340.0, h - 60.0);
    streets(cx, map, Color::hex(0x0e1014), Color::hex(0x22252d), 8.0, 21);
    // two main roads
    cx.ui.line(
        Vec2::new(340.0, 380.0),
        Vec2::new(w, 345.0),
        16.0,
        Color::hex(0x2a2d35),
    );
    cx.ui.line(
        Vec2::new(820.0, 60.0),
        Vec2::new(850.0, h),
        12.0,
        Color::hex(0x2a2d35),
    );
    cx.fill(Rect::new(340.0, 0.0, w - 340.0, 60.0), Color::hex(0x0b0c0f));
    cx.hline(340.0, 60.0, w - 340.0, LINE);
    cx.spaced("CITY MAP", 11.0, Weight::Regular, 362.0, 30.0, 2.0, ACC);
    cx.left("50", 21.0, Weight::Bold, 454.0, 30.0, FG);
    cx.left("\u{203a}", 21.0, Weight::Bold, 485.0, 30.0, FG);
    cx.left("Central Station", 21.0, Weight::Bold, 507.0, 30.0, FG);
    cx.left(
        "Next stop: Depot \u{b7} 1.2 km \u{b7} 08:00",
        14.0,
        Weight::Regular,
        675.0,
        30.0,
        MUT,
    );
    let mut bx = w - 360.0 - 0.0;
    let _ = &mut bx;
    for (label, x, wd, on) in [
        ("\u{2192} Lines", w - 361.0, 77.0, false),
        ("\u{25ce} Follow bus", w - 277.0, 113.0, true),
        ("+", w - 156.0, 39.0, false),
        ("\u{2212}", w - 107.0, 39.0, false),
        ("\u{d7}", w - 59.0, 39.0, false),
    ] {
        let r = Rect::new(x, 14.0, wd, 32.0);
        cx.fill(r, PANEL.alpha(PA));
        cx.frame(r, if on { ACC } else { LINE });
        cx.center(
            label,
            14.0,
            Weight::Regular,
            r.x + r.w * 0.5,
            r.y + 16.0,
            if on { ACC } else { FG },
        );
    }
    // route: driven (grey) then ahead (amber)
    let drv = [
        Vec2::new(400.0, 760.0),
        Vec2::new(600.0, 698.0),
        Vec2::new(810.0, 650.0),
    ];
    path(cx, &drv, 6.0, Color::hex(0x3a4050));
    let ahead = [
        Vec2::new(810.0, 650.0),
        Vec2::new(1000.0, 540.0),
        Vec2::new(1140.0, 450.0),
        Vec2::new(1320.0, 380.0),
        Vec2::new(1490.0, 300.0),
        Vec2::new(1545.0, 258.0),
    ];
    path(cx, &ahead, 7.0, ACC);
    // traffic
    let mut s = 99;
    for _ in 0..22 {
        let p = Vec2::new(
            380.0 + lcg(&mut s) * (w - 420.0),
            90.0 + lcg(&mut s) * (h - 130.0),
        );
        cx.ui.circle(p, 3.0, Color::hex(0x3b82f6));
    }
    for (p, n, c) in [
        (Vec2::new(640.0, 250.0), "12", ACC),
        (Vec2::new(1240.0, 330.0), "7", OK),
        (Vec2::new(980.0, 520.0), "21", Color::hex(0xe8523f)),
    ] {
        cx.ui.circle(p, 4.0, c);
        let r = Rect::new(p.x - 12.0, p.y - 20.0, 24.0, 16.0);
        cx.ui.rounded(r, 8.0, c);
        cx.center(
            n,
            11.0,
            Weight::Bold,
            p.x,
            p.y - 12.0,
            if c == ACC {
                ON_ACC
            } else {
                Color::hex(0x0b0c0f)
            },
        );
    }
    for p in [
        Vec2::new(810.0, 650.0),
        Vec2::new(1140.0, 450.0),
        Vec2::new(1490.0, 300.0),
    ] {
        ring(cx, p, 7.0, Color::hex(0x3a4050), Color::hex(0x14171c));
    }
    stop_pill(cx, 613.0, 664.0, "Depot", "08:00");
    stop_pill(cx, 825.0, 612.0, "Market", "08:25");
    stop_pill(cx, 1155.0, 412.0, "Main Street", "08:12");
    stop_pill(cx, 1506.0, 262.0, "Central Station", "08:42");
    cx.ui.circle(Vec2::new(600.0, 700.0), 9.0, OK);
    cx.ui.circle(
        Vec2::new(700.0, 679.0),
        22.0,
        Color::hex(0x4a3d1a).alpha(0.7),
    );
    cx.ui.convex(
        &[
            Vec2::new(690.0, 668.0),
            Vec2::new(714.0, 681.0),
            Vec2::new(692.0, 692.0),
            Vec2::new(696.0, 680.0),
        ],
        Color::WHITE,
    );
    // panels
    let a = Rect::new(360.0, 80.0, 260.0, 104.0);
    win_small(
        cx,
        a,
        "YOU ARE HERE",
        &[("City", "Example City"), ("Street", "Example Street")],
    );
    let b = Rect::new(w - 310.0, 80.0, 290.0, 173.0);
    win_small(
        cx,
        b,
        "TRIP",
        &[
            ("Next stop", "Depot"),
            ("Distance", "1.2 km"),
            ("Arrival", "08:00"),
            ("Stops left", "4"),
        ],
    );
    legend(
        cx,
        w,
        h,
        &[
            ("\u{25ac} Route", ACC),
            ("\u{25ac} Driven", MUT),
            ("\u{25cf} Traffic", Color::hex(0x3b82f6)),
            ("\u{25cf} You", OK),
        ],
        "Wheel: zoom \u{b7} Drag: pan \u{b7} Esc: close",
    );
    game_menu(cx, h, 35.0, 138.0);
}

fn win_small(cx: &mut Cx, r: Rect, title: &str, rows: &[(&str, &str)]) {
    cx.fill(r, PANEL.alpha(PA));
    cx.frame(r, LINE);
    cx.spaced(
        title,
        11.0,
        Weight::Medium,
        r.x + 15.0,
        r.y + 18.0,
        2.0,
        MUT,
    );
    for (i, (k, v)) in rows.iter().enumerate() {
        let y = r.y + 35.0 + i as f32 * 34.0;
        cx.hline(r.x + 1.0, y, r.w - 2.0, ROWLINE);
        cx.left(k, 14.0, Weight::Regular, r.x + 15.0, y + 17.0, MUT);
        cx.right(v, 14.0, Weight::Bold, r.right() - 15.0, y + 17.0, FG);
    }
}

// ---------------------------------------------------------------------------------------
// in game: Alt icon bar and the windows

fn game_scene(cx: &mut Cx, w: f32, h: f32) {
    cx.ui.gradient(
        Rect::new(0.0, 0.0, w, h * 0.55),
        Color::hex(0x3c4b60),
        Color::hex(0x1b2536),
    );
    cx.ui.gradient(
        Rect::new(0.0, h * 0.55, w, h * 0.45),
        Color::hex(0x2a2a2b),
        Color::hex(0x161617),
    );
}

fn alt_bar(cx: &mut Cx, w: f32, h: f32) {
    game_scene(cx, w, h);
    game_bar(
        cx,
        w,
        false,
        "Alt = toggle the icon bar \u{b7} the simulation keeps running",
    );
    let p = Rect::new(w - 306.0, h - 218.0, 290.0, 198.0);
    cx.fill(p, PANEL.alpha(PA));
    cx.frame(p, LINE);
    cx.icon("play_arrow", p.x + 15.0, p.y + 17.0, 9.0, ACC);
    cx.left(
        "Reposition at entry point",
        13.0,
        Weight::Regular,
        p.x + 27.0,
        p.y + 17.0,
        FG,
    );
    cx.hline(p.x + 1.0, p.y + 34.0, p.w - 2.0, ROWLINE);
    let icons: [(&str, bool, bool); 15] = [
        ("add", true, false),
        ("sync_alt", true, false),
        ("close", true, false),
        ("download", true, true),
        ("AI", true, false),
        ("grid_view", true, false),
        ("view_list", true, false),
        ("warning", false, false),
        ("construction", true, false),
        ("bolt", false, false),
        ("help", true, false),
        ("schedule", true, false),
        ("cloud", true, false),
        ("my_location", true, false),
        ("menu", true, false),
    ];
    for (i, (n, on, sel)) in icons.iter().enumerate() {
        let (cxn, cyn) = ((i % 5) as f32, (i / 5) as f32);
        let r = Rect::new(p.x + 9.0 + cxn * 55.6, p.y + 43.0 + cyn * 51.0, 50.0, 45.0);
        cx.fill(r, Color::hex(0x1b1d22));
        cx.frame(r, if *sel { ACC } else { LINE });
        let c = if *on { ACC } else { ACC.alpha(0.3) };
        if *n == "AI" {
            cx.center("AI", 17.0, Weight::Bold, r.x + 25.0, r.y + 22.5, c);
        } else {
            cx.icon(
                n,
                r.x + 25.0,
                r.y + 22.5,
                20.0,
                if *n == "construction" {
                    Color::hex(0x7fb2d9)
                } else {
                    c
                },
            );
        }
    }
}

fn windows(cx: &mut Cx, w: f32, h: f32) {
    game_scene(cx, w, h);
    game_bar(cx, w, true, "Alt = menu \u{b7} F4 = select/move vehicle");
    let sel_row = |cx: &mut Cx, r: Rect| {
        cx.fill(r, Color::hex(0x2a2316));
        cx.fill(Rect::new(r.x, r.y, 3.0, r.h), ACC);
    };
    // session vehicles
    let a = Rect::new(40.0, 70.0, 520.0, 243.0);
    win(cx, a, "SESSION \u{b7} VEHICLES", "add / swap / remove");
    for (t, x) in [("Vehicle", 57.0), ("Paint", 257.0), ("Position", 376.0)] {
        cx.left(t, 12.0, Weight::Regular, x, a.y + 57.0, MUT);
    }
    for (i, (v, p, pos)) in [
        ("Example bus A", "Default", "Player"),
        ("Example bus B", "Paint 2", "AI \u{b7} line 50"),
        ("Example bus C", "Paint 3", "Parked \u{b7} depot"),
    ]
    .iter()
    .enumerate()
    {
        let r = Rect::new(a.x + 1.0, a.y + 73.0 + i as f32 * 34.0, a.w - 2.0, 34.0);
        if i == 0 {
            sel_row(cx, r);
        } else {
            cx.hline(r.x, r.y, r.w, ROWLINE);
        }
        for (t, x) in [(v, 57.0), (p, 257.0), (pos, 376.0)] {
            cx.left(
                t,
                14.0,
                Weight::Regular,
                x + if i == 0 && x == 57.0 { 1.0 } else { 0.0 },
                r.y + 17.0,
                FG,
            );
        }
    }
    btn(
        cx,
        Rect::new(57.0, 259.0, 133.0, 39.0),
        "Add vehicle...",
        true,
    );
    btn(cx, Rect::new(199.0, 259.0, 73.0, 39.0), "Swap", false);
    btn(cx, Rect::new(280.0, 259.0, 91.0, 39.0), "Remove", false);

    // choose vehicle
    let b = Rect::new(40.0, 370.0, 520.0, 300.0);
    win(cx, b, "CHOOSE VEHICLE", "from cache");
    cx.fill(Rect::new(281.0, b.y + 41.0, 1.0, 190.0), ROWLINE);
    for (i, (n, v)) in [
        ("Example bus A", "v1.2"),
        ("Example bus B", "v2.3"),
        ("Example bus C", "v1.5"),
    ]
    .iter()
    .enumerate()
    {
        let r = Rect::new(b.x + 1.0, b.y + 41.0 + i as f32 * 36.0, 240.0, 36.0);
        if i == 0 {
            sel_row(cx, r);
        } else {
            cx.hline(r.x, r.y, r.w, ROWLINE);
        }
        cx.left(
            n,
            14.0,
            Weight::Regular,
            57.0 + if i == 0 { 3.0 } else { 0.0 },
            r.y + 18.0,
            FG,
        );
        cx.right(v, 14.0, Weight::Regular, 264.0, r.y + 18.0, MUT);
    }
    let pv = Rect::new(295.0, 425.0, 250.0, 100.0);
    cx.fill(pv, PANEL2);
    cx.center(
        "preview image",
        14.0,
        Weight::Regular,
        pv.x + pv.w * 0.5,
        pv.y + 50.0,
        MUT,
    );
    cx.spaced(
        "AUTHOR'S DESCRIPTION",
        12.0,
        Weight::Regular,
        295.0,
        545.0,
        1.0,
        MUT,
    );
    cx.left(
        "Technical sheet and credits as written",
        13.0,
        Weight::Regular,
        295.0,
        564.0,
        FG,
    );
    cx.left(
        "by the author (example text).",
        13.0,
        Weight::Regular,
        295.0,
        580.0,
        FG,
    );
    btn(cx, Rect::new(57.0, 616.0, 72.0, 39.0), "Load", true);
    btn(cx, Rect::new(137.0, 616.0, 84.0, 39.0), "Cancel", false);

    // end of trip report
    let c = Rect::new(620.0, 120.0, 780.0, 349.0);
    win(cx, c, "END OF TRIP REPORT", "sim paused");
    let cols = [635.0, 825.0, 959.0, 1060.0, 1184.0, 1299.0];
    for (t, x) in [
        "Stop",
        "Scheduled",
        "Actual",
        "Deviation",
        "Boarded",
        "Alighted",
    ]
    .iter()
    .zip(cols)
    {
        cx.left(t, 12.0, Weight::Regular, x, 177.0, MUT);
    }
    let rows = [
        ("Depot", "08:00", "08:00", "0:00", OK, "0", "0"),
        (
            "Main Street",
            "08:12",
            "08:13",
            "+1:05",
            Color::hex(0xe8523f),
            "14",
            "2",
        ),
        ("Market", "08:25", "08:25", "0:00", OK, "9", "11"),
        ("Central Station", "08:42", "08:41", "-0:40", OK, "3", "18"),
    ];
    for (i, (s, sc, ac, dv, dc, bo, al)) in rows.iter().enumerate() {
        let y = 193.0 + i as f32 * 34.0;
        cx.hline(c.x + 1.0, y, c.w - 2.0, ROWLINE);
        cx.left(s, 14.0, Weight::Regular, cols[0], y + 17.0, FG);
        cx.left(sc, 14.0, Weight::Regular, cols[1], y + 17.0, FG);
        cx.left(ac, 14.0, Weight::Regular, cols[2], y + 17.0, FG);
        cx.left(dv, 14.0, Weight::Regular, cols[3], y + 17.0, *dc);
        cx.left(bo, 14.0, Weight::Regular, cols[4], y + 17.0, FG);
        cx.left(al, 14.0, Weight::Regular, cols[5], y + 17.0, FG);
    }
    for (i, (k, v)) in [
        ("Total passengers", "26"),
        ("Punctuality", "3 of 4 stops on time"),
    ]
    .iter()
    .enumerate()
    {
        let y = 329.0 + i as f32 * 36.0;
        cx.hline(c.x + 1.0, y, c.w - 2.0, ROWLINE);
        cx.left(k, 14.0, Weight::Regular, 637.0, y + 18.0, MUT);
        cx.right(v, 14.0, Weight::Bold, 1383.0, y + 18.0, FG);
    }
    btn(cx, Rect::new(637.0, 415.0, 100.0, 39.0), "Continue", true);
    btn(cx, Rect::new(745.0, 415.0, 93.0, 39.0), "New trip", false);
    btn(
        cx,
        Rect::new(847.0, 415.0, 129.0, 39.0),
        "Back to menu",
        false,
    );

    // menu
    let d = Rect::new(620.0, 520.0, 380.0, 293.0);
    win(cx, d, "MENU", "Alt");
    for (i, t) in [
        "Resume",
        "Vehicles in session",
        "Timetable",
        "Settings",
        "Controls",
        "Save / load",
        "Leave session",
    ]
    .iter()
    .enumerate()
    {
        let y = 561.0 + i as f32 * 36.0;
        if i > 0 {
            cx.hline(d.x + 1.0, y, d.w - 2.0, ROWLINE);
        }
        cx.left(t, 14.0, Weight::Regular, 637.0, y + 18.0, FG);
    }

    // passengers and stop
    let e = Rect::new(1030.0, 520.0, 370.0, 186.0);
    win(cx, e, "PASSENGERS & STOP", "live");
    for (i, (k, v, c)) in [
        ("Onboard", "26 / 94", FG),
        ("Standing", "4", FG),
        ("Doors", "open", OK),
        ("Stop distance", "0.4 m", FG),
    ]
    .iter()
    .enumerate()
    {
        let y = 561.0 + i as f32 * 36.0;
        if i > 0 {
            cx.hline(e.x + 1.0, y, e.w - 2.0, ROWLINE);
        }
        cx.left(k, 14.0, Weight::Regular, 1047.0, y + 18.0, MUT);
        cx.right(v, 14.0, Weight::Bold, 1383.0, y + 18.0, *c);
    }
}

fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fonts: &Fonts,
    (w, h): (u32, u32),
    out: &str,
    logo_path: &str,
    screen: fn(&mut Cx, f32, f32),
) {
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut gpu = Gpu::new(device, format, 4, 512);
    let scale = (w as f32 / 1672.0).min(h as f32 / 941.0).max(0.75);
    let (dw, dh) = (w as f32 / scale, h as f32 / scale);
    let logo = image::open(logo_path).expect("logo").to_rgba8();
    let _ = LOGO_ASPECT.set(logo.height() as f32 / logo.width() as f32);
    // (texture 1 is the logo)
    gpu.add_image(device, queue, logo.width(), logo.height(), logo.as_raw());
    let mut cx = Cx {
        ui: Painter::with_scale(scale),
        atlas: Atlas::new(1024),
        fonts,
        draws: Vec::new(),
        mark: 0,
        clip: [0.0; 4],
    };
    cx.atlas.begin_frame();
    screen(&mut cx, dw, dh);
    cx.cut(0, 0);
    // (everything was laid out in design units: scale the pixels once, here)
    for v in &mut cx.ui.verts {
        v.pos[0] *= scale;
        v.pos[1] *= scale;
    }
    gpu.upload(device, queue, 0, &cx.ui.verts);
    gpu.upload_atlas(queue, &mut cx.atlas);
    let layer0 = Layer::flat([0.0, 0.0, w as f32, h as f32], 0.0, 1.0);
    let layer1 = Layer::flat(cx.clip.map(|v| v * scale), 0.0, 1.0);
    let layers = [layer0, layer1];
    let draws: Vec<Draw> = cx
        .draws
        .iter()
        .map(|(r, l, t)| Draw {
            buffer: 0,
            range: r.clone(),
            layer: *l,
            texture: *t,
        })
        .collect();
    let mut enc = device.create_command_encoder(&Default::default());
    gpu.render(
        device,
        queue,
        &mut enc,
        &view,
        (w, h),
        Some(wgpu::Color::BLACK),
        &layers,
        &draws,
    );
    let stride = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (stride * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    enc.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).ok();
    let data = buf.slice(..).get_mapped_range();
    let mut img = image::RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let i = (y * stride + x * 4) as usize;
            img.put_pixel(x, y, image::Rgba([data[i], data[i + 1], data[i + 2], 255]));
        }
    }
    img.save(out).unwrap();
    println!("wrote {out}");
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(".".into());
    let logo_path = std::env::args().nth(2).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/logos/pd.png").into()
    });
    std::fs::create_dir_all(&dir).unwrap();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("device");
    let fonts = Fonts::new();
    type Screen = fn(&mut Cx, f32, f32);
    let screens: [(&str, Screen); 7] = [
        ("launcher-vehicles", vehicles),
        ("launcher-maps", maps),
        ("loading", loading),
        ("game-menu-city-map", pause_map),
        ("map-shift-m", shift_m),
        ("alt-icon-bar", alt_bar),
        ("ingame-windows", windows),
    ];
    for (name, f) in screens {
        for ((w, h), size) in [
            ((1672, 941), "1672x941"),
            ((1920, 1080), "1920x1080"),
            ((2560, 1080), "ultrawide-2560x1080"),
        ] {
            render(
                &device,
                &queue,
                &fonts,
                (w, h),
                &format!("{dir}/{name}-{size}.png"),
                &logo_path,
                f,
            );
        }
    }
}

//! The Setup page: where the original OMSI 2 is, where this game's program is and where the
//! Depot keeps its content (Browse each, Save or Revert at the foot), the Depot's own files,
//! and what the content holds - what was found, and what it takes on disk.

use super::theme::*;
use super::ui::{id_of, ButtonKind, Ui};
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_launcher_lib::disk::{self, Node};
use omsi_launcher_ui::kit::Status;
use omsi_launcher_ui::Page as UiPage;
use omsi_ui::paint::Align;
use omsi_ui::{Color, Rect, Weight};

/// The Setup page's edits and what it shows.
#[derive(Default)]
pub struct SetupView {
    /// The folders as typed or browsed, until Save or Revert (none: as saved).
    pub root: Option<String>,
    pub game: Option<String>,
    pub content: Option<String>,
    /// The Content card's tab: 0 what was found, 1 the disk usage.
    pub tab: usize,
    /// The part of the disk usage chart looked into (the indices of its parts from the top).
    pub focus: Vec<usize>,
    checks: [(String, Option<(Status, String)>); 3],
    /// Where the content folder is now (the field's hint while none is chosen).
    content_now: Option<String>,
}

const FIELDS: [&str; 3] = ["root", "game", "content"];

/// A folder's line under its field: what was found there.
fn check(field: usize, value: &str, root: &str) -> (Status, String) {
    let v = value.trim();
    let p = std::path::Path::new(v);
    match field {
        0 if v.is_empty() => (Status::Missing, "Not chosen yet: the folder with maps and Vehicles in it.".into()),
        0 => {
            let missing = omsi_cfg::missing_original_essentials(p);
            if missing.is_empty() {
                (Status::Found, "The essential files are there. Read only: the launcher never writes to it.".into())
            } else {
                let shown: Vec<&str> = missing.iter().map(String::as_str).take(4).collect();
                let more = if missing.len() > shown.len() { format!(" and {} more", missing.len() - shown.len()) } else { String::new() };
                (Status::Error, format!("Not a complete OMSI 2 installation: it lacks {}{more}.", shown.join(", ")))
            }
        }
        1 if v.is_empty() => (Status::Warning, "Not found: choose the depot program.".into()),
        1 if p.is_file() => {
            let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf()));
            if beside.is_some_and(|d| p.parent() == Some(d.as_path())) {
                (Status::Found, format!("Found next to the launcher. Version {}", env!("CARGO_PKG_VERSION")))
            } else {
                (Status::Found, "Found.".into())
            }
        }
        1 => (Status::Error, "There is no program at this path.".into()),
        // (the content folder: nothing is written while it is typed - Save makes it and
        // tries it)
        // (`OMSI_CONTENT` wins over what is chosen here)
        _ if omsi_cfg::flags::CONTENT.os().is_some() => (Status::Warning, "Set by OMSI_CONTENT: what is chosen here is used once it is no longer set.".into()),
        _ if v.is_empty() => (Status::Found, "Beside the game program. Mods put here are not available in OMSI 2.".into()),
        _ if p.is_file() => (Status::Error, "This is a file, not a folder.".into()),
        _ => match core::content_choice_problem(p, std::path::Path::new(root.trim())) {
            Some(why) => (Status::Error, format!("Cannot be used: {why}.")),
            None if !p.exists() => (Status::Warning, "It does not exist yet: it is made when you save.".into()),
            None => (Status::Found, "Mods put here are not available in OMSI 2.".into()),
        },
    }
}

/// The colour of a type of content (`disk::TYPES`), in the chart and its legend.
pub fn type_color(name: &str) -> Color {
    match name {
        "Vehicles" => Color::hex(0xe8a33d),
        "Sceneryobjects" => Color::hex(0x4c8ddb),
        "Texture" => Color::hex(0xa27bdb),
        "Maps" => Color::hex(0x4cb86a),
        "Sounds" => Color::hex(0xe0605a),
        "Splines" => Color::hex(0x2aa7a0),
        "Archives" => Color::hex(0xd46fa8),
        _ => Color::hex(0x8a8f96),
    }
}

pub fn setup(l: &mut Launcher, area: Rect) {
    let body = l.page_title(area, UiPage::Setup);
    let phone = super::mobile::mobile();
    if !l.state.usage_asked && !phone {
        l.state.usage_asked = true;
        l.state.load_usage();
    }
    if l.pages.setup.content_now.is_none() {
        l.pages.setup.content_now = Some(core::content_dir().map(|d| d.to_string_lossy().to_string()).unwrap_or_default());
    }
    let foot_h = 60.0;
    let main = Rect::new(body.x, body.y, body.w, body.h - foot_h - GAP);
    let right_w = if phone { 0.0 } else { (body.w * 0.3).clamp(300.0, 420.0) };
    let left = Rect::new(main.x, main.y, if phone { main.w } else { main.w - right_w - GAP }, main.h);
    let mut y = folders(l, left);
    if !phone {
        if l.pages.setup.tab == 1 {
            y = free_space(l, Rect::new(left.x, y + GAP, left.w, 0.0));
        }
        own_files(l, Rect::new(left.x, y + GAP, left.w, 0.0));
        content(l, Rect::new(left.right() + GAP, main.y, right_w, main.h));
    }
    foot(l, Rect::new(body.x, body.bottom() - foot_h, body.w, foot_h));
}

/// A folder's field (label over it, Browse beside it, what was found under it). True when
/// Browse was pressed.
#[allow(clippy::too_many_arguments)]
fn folder_row(ui: &mut Ui, x: f32, y: f32, w: f32, label: &str, id: &str, value: &mut String, hint: &str, icon: &str, status: &(Status, String)) -> (bool, bool) {
    ui.text_in(label, Rect::new(x, y, w, 18.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    let changed = ui.text_input(&format!("cfg-{id}"), Rect::new(x, y + 20.0, w - 92.0, 34.0), value, hint, Some(icon));
    let browse = ui.button(&format!("browse-{id}"), Rect::new(x + w - 82.0, y + 20.0, 82.0, 34.0), "Browse", None, ButtonKind::Normal);
    omsi_launcher_ui::kit::dot(ui, x + 4.0, y + 68.0, status.0);
    let c = match status.0 {
        Status::Found => OK,
        Status::Warning => WARN,
        Status::Unverified => TEXT_DIM,
        Status::Missing | Status::Error => DANGER,
    };
    omsi_launcher_ui::kit::clipped(ui, &status.1, x + 15.0, y + 68.0, w - 15.0, 12.5, Weight::Regular, c);
    (changed, browse)
}

/// The Folders card; returns its foot.
fn folders(l: &mut Launcher, col: Rect) -> f32 {
    let phone = super::mobile::mobile();
    let in_process = core::IN_PROCESS_GAMES;
    let rows = if in_process { 1.0 } else { 3.0 };
    let extra = if in_process { 96.0 } else { 0.0 };
    let r = Rect::new(col.x, col.y, col.w, 46.0 + rows * 88.0 + extra);
    l.ui.panel(r);
    let inner = Launcher::card_title(&mut l.ui, Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0), "Folders");
    let saved = [l.state.config.root.clone(), l.state.config.game.clone(), l.state.config.content.clone()];
    let mut values = [
        l.pages.setup.root.clone().unwrap_or_else(|| saved[0].clone()),
        l.pages.setup.game.clone().unwrap_or_else(|| saved[1].clone()),
        l.pages.setup.content.clone().unwrap_or_else(|| saved[2].clone()),
    ];
    for f in 0..3 {
        // (the content folder is checked again when the OMSI 2 folder changes too)
        let key = if f == 2 { format!("{}\u{0}{}", values[2], values[0]) } else { values[f].clone() };
        if l.pages.setup.checks[f].1.is_none() || l.pages.setup.checks[f].0 != key {
            let out = check(f, &values[f], &values[0]);
            l.pages.setup.checks[f] = (key, Some(out));
        }
    }
    let status = |v: &SetupView, f: usize| v.checks[f].1.clone().unwrap_or((Status::Unverified, String::new()));
    let content_hint = l.pages.setup.content_now.clone().unwrap_or_default();
    let mut y = inner.y;
    let list: &[(usize, &str, &str, &str)] = if in_process {
        &[(0, "OMSI 2 folder", "/path/to/OMSI 2", "o-folder")]
    } else {
        &[(0, "OMSI 2 folder", "D:\\OMSI 2", "o-folder"), (1, "Game program", "depot", "o-play"), (2, "Depot content folder", "", "o-folder")]
    };
    for &(f, label, hint, icon) in list {
        let hint = if f == 2 { content_hint.as_str() } else { hint };
        let st = status(&l.pages.setup, f);
        let (changed, browse) = folder_row(&mut l.ui, inner.x, y, inner.w, label, FIELDS[f], &mut values[f], hint, icon, &st);
        if changed {
            set(&mut l.pages.setup, f, values[f].clone());
        }
        if browse {
            match f {
                0 if phone || in_process => {
                    let start = values[0].clone();
                    l.browse(super::mobile::Purpose::Root, &start);
                }
                0 => {
                    if let Some(p) = core::pick_folder("The OMSI 2 folder (with maps and Vehicles in it)") {
                        set(&mut l.pages.setup, 0, p.to_string_lossy().to_string());
                    }
                }
                1 => {
                    if let Some(p) = core::pick_file("The depot program") {
                        set(&mut l.pages.setup, 1, p.to_string_lossy().to_string());
                    }
                }
                _ => {
                    if let Some(p) = core::pick_folder("The Depot's content folder (mods are installed into it)") {
                        set(&mut l.pages.setup, 2, p.to_string_lossy().to_string());
                    }
                }
            }
        }
        y += 88.0;
    }
    if in_process {
        // (a phone: the game is this app itself)
        l.ui.paragraph("Copy the whole OMSI 2 folder (with maps and Vehicles in it) onto the phone - by cable, from a PC or a USB stick - for example as depot/OMSI 2 in the internal storage, then choose it here with Browse. Mods go into depot/Mods or are installed from the Mods page.", Vec2::new(inner.x, y), inner.w, 12.5, Weight::Regular, TEXT_DIM);
    }
    r.bottom()
}

fn set(v: &mut SetupView, f: usize, s: String) {
    match f {
        0 => v.root = Some(s),
        1 => v.game = Some(s),
        _ => v.content = Some(s),
    }
}

/// The "Free up space" card (Disk usage): what the content takes. Compressing it is not
/// offered yet.
fn free_space(l: &mut Launcher, at: Rect) -> f32 {
    let r = Rect::new(at.x, at.y, at.w, 186.0);
    l.ui.panel(r);
    let inner = Launcher::card_title(&mut l.ui, Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0), "Free up space");
    chip(&mut l.ui, r.right() - 18.0, r.y + 22.0, "Experimental");
    let half = inner.w * 0.5;
    let total = l.state.usage.as_ref().map(|u| fmt_size(u.tree.bytes)).unwrap_or_else(|| "Counting…".into());
    l.ui.text_in("Content found (both folders)", Rect::new(inner.x, inner.y, half, 18.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    l.ui.text_in(&total, Rect::new(inner.x, inner.y + 18.0, half, 30.0), 22.0, Weight::Bold, TEXT, Align::Left);
    l.ui.text_in("Compression with NTFS (LZX)", Rect::new(inner.x + half, inner.y, half, 18.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    l.ui.text_in("Not available yet", Rect::new(inner.x + half, inner.y + 18.0, half, 30.0), 22.0, Weight::Bold, WARN, Align::Left);
    let y = inner.y + 60.0;
    dim_button(&mut l.ui, Rect::new(inner.x, y, 220.0, 34.0), "Compress with NTFS (LZX)...");
    dim_button(&mut l.ui, Rect::new(inner.x + 230.0, y, 160.0, 34.0), "Undo compression");
    l.ui.paragraph("How compressing the content should work is still being decided; nothing is compressed for now.", Vec2::new(inner.x, y + 44.0), inner.w, 12.0, Weight::Regular, TEXT_FAINT);
    r.bottom()
}

/// A button that cannot be pressed.
fn dim_button(ui: &mut Ui, r: Rect, label: &str) {
    ui.p().rounded(r, CONTROL_RADIUS, FIELD);
    ui.p().rounded_border(r, CONTROL_RADIUS, 1.0, EDGE);
    ui.text_in(label, r, 13.0, Weight::Medium, TEXT_FAINT, Align::Center);
}

/// A small outlined label in the accent colour, its right edge at `right`.
fn chip(ui: &mut Ui, right: f32, cy: f32, label: &str) {
    let w = ui.width(label, 12.0, Weight::Medium) + 20.0;
    let r = Rect::new(right - w, cy - 11.0, w, 22.0);
    ui.p().rounded(r, 11.0, accent().alpha(0.12));
    ui.p().rounded_border(r, 11.0, 1.0, accent().alpha(0.5));
    ui.text_in(label, r, 12.0, Weight::Medium, accent(), Align::Center);
}

/// The Depot's own files: its folder (logs, previews, settings), and a button to open it.
fn own_files(l: &mut Launcher, at: Rect) {
    let r = Rect::new(at.x, at.y, at.w, 128.0);
    l.ui.panel(r);
    let inner = Launcher::card_title(&mut l.ui, Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0), "The Depot's own files");
    omsi_launcher_ui::kit::right(&mut l.ui, "logs, previews and settings", r.right() - 18.0, r.y + 22.0, 12.0, Weight::Regular, TEXT_DIM);
    let dir = core::data_dir();
    let field = Rect::new(inner.x, inner.y, inner.w - 132.0, 34.0);
    l.ui.p().rounded(field, CONTROL_RADIUS, FIELD);
    l.ui.p().rounded_border(field, CONTROL_RADIUS, 1.0, EDGE);
    l.ui.icon("o-folder", Vec2::new(field.x + 18.0, field.center().y), 15.0, TEXT_DIM);
    omsi_launcher_ui::kit::clipped(&mut l.ui, &dir.to_string_lossy(), field.x + 34.0, field.center().y, field.w - 44.0, 13.5, Weight::Regular, TEXT);
    if l.ui.button("open-depot-folder", Rect::new(inner.right() - 122.0, inner.y, 122.0, 34.0), "Open folder", None, ButtonKind::Normal) && !core::open_folder(&dir) {
        l.state.set_status(format!("The folder could not be opened: {}", dir.display()), true);
    }
    l.ui.paragraph("Holds the game log, the launcher log, the preview cache and your settings. Safe to open; nothing here changes the original OMSI 2.", Vec2::new(inner.x, inner.y + 44.0), inner.w, 12.0, Weight::Regular, TEXT_FAINT);
}

/// The Content card: what was found, or what it takes on disk.
fn content(l: &mut Launcher, r: Rect) {
    l.ui.panel(r);
    let inner = Launcher::card_title(&mut l.ui, Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0), "Content");
    let mut tab = l.pages.setup.tab;
    l.ui.segmented("setup-content-tab", Rect::new(inner.x, inner.y, inner.w, 36.0), &mut tab, &["What was found", "Disk usage"]);
    l.pages.setup.tab = tab;
    let y = inner.y + 48.0;
    let Some(usage) = l.state.usage.clone() else {
        l.ui.text_in("Counting what is in both folders…", Rect::new(inner.x, y, inner.w, 22.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
        return;
    };
    if tab == 0 {
        let c = usage.counts;
        let rows = [("Maps", c.maps), ("Vehicles", c.vehicles), ("Sceneryobjects", c.sceneryobjects), ("Splines", c.splines), ("Archives (.zip)", c.archives)];
        let mut ry = y;
        for (k, (name, n)) in rows.iter().enumerate() {
            if k > 0 {
                l.ui.p().rect(Rect::new(inner.x, ry, inner.w, 1.0), EDGE);
            }
            omsi_launcher_ui::kit::dot(&mut l.ui, inner.x + 4.0, ry + 17.0, if *n > 0 { Status::Found } else { Status::Unverified });
            l.ui.text_in(name, Rect::new(inner.x + 18.0, ry + 5.0, inner.w - 80.0, 24.0), 13.5, Weight::Regular, TEXT, Align::Left);
            l.ui.text_in(&n.to_string(), Rect::new(inner.x, ry + 5.0, inner.w, 24.0), 13.5, Weight::Regular, TEXT_DIM, Align::Right);
            ry += 35.0;
        }
        let note = "Counted each time the page opens, in the OMSI 2 folder and the content folder.";
        let h = l.ui.paragraph_height(note, inner.w, 12.0, Weight::Regular);
        l.ui.paragraph(note, Vec2::new(inner.x, inner.bottom() - h), inner.w, 12.0, Weight::Regular, TEXT_FAINT);
        return;
    }
    let note = "Inside out: folder, then type, then what is in it, then file kind. Click a slice; click the middle to go back.";
    let y = y + l.ui.paragraph(note, Vec2::new(inner.x, y), inner.w, 12.0, Weight::Regular, TEXT_DIM) + 10.0;
    let legend = disk::by_type(&usage.tree);
    let legend_h = legend.len().div_ceil(2) as f32 * 22.0;
    let side = (inner.w).min(inner.bottom() - y - legend_h - 24.0).max(120.0);
    let chart_r = Rect::new(inner.x + (inner.w - side) * 0.5, y, side, side);
    chart(&mut l.ui, chart_r, &usage.tree, &mut l.pages.setup.focus);
    let mut ly = chart_r.bottom() + 18.0;
    let cw = (inner.w - GAP) * 0.5;
    for (k, (name, bytes)) in legend.iter().enumerate() {
        let x = inner.x + (k % 2) as f32 * (cw + GAP);
        if k > 0 && k % 2 == 0 {
            ly += 22.0;
        }
        l.ui.p().rounded(Rect::new(x, ly + 6.0, 10.0, 10.0), 2.0, type_color(name));
        l.ui.text_in(name, Rect::new(x + 18.0, ly, cw - 70.0, 22.0), 13.0, Weight::Regular, TEXT, Align::Left);
        l.ui.text_in(&fmt_size(*bytes), Rect::new(x, ly, cw, 22.0), 13.0, Weight::Regular, TEXT_DIM, Align::Right);
    }
}

/// A size as the chart says it: whole gigabytes from 10 GB, else one decimal.
pub fn fmt_size(b: u64) -> String {
    let gb = b as f64 / 1_073_741_824.0;
    if gb >= 10.0 {
        format!("{gb:.0} GB")
    } else if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        super::state::fmt_bytes(b)
    }
}

/// A slice of the chart: its ring (0 the inner one), its angles, its path from the top of the
/// tree and its depth there (1 a folder, 2 a type, 3 what is in it, 4 a kind of file).
struct Slice<'a> {
    ring: usize,
    a0: f32,
    a1: f32,
    path: Vec<usize>,
    depth: usize,
    node: &'a Node,
    /// The type it belongs to (a folder belongs to none).
    ty: Option<&'a str>,
    /// Its place among its parent's parts.
    index: usize,
}

/// The node at `path` from `tree`, with its depth and the type it belongs to; a path that no
/// longer fits (the content read again) is cut back to what still fits.
fn node_at<'a>(tree: &'a Node, path: &mut Vec<usize>) -> (&'a Node, Option<&'a str>) {
    let mut n = tree;
    let mut ty = None;
    for (d, &k) in path.clone().iter().enumerate() {
        match n.kids.get(k) {
            Some(c) => {
                n = c;
                if d == 1 {
                    ty = Some(c.name.as_str());
                }
            }
            None => {
                path.truncate(d);
                break;
            }
        }
    }
    (n, ty)
}

/// The disk usage as rings around the middle (a sunburst): the part looked into in the
/// middle, its parts around it, theirs further out.
fn chart(ui: &mut Ui, r: Rect, tree: &Node, focus: &mut Vec<usize>) {
    let (top, top_ty) = node_at(tree, focus);
    let depth0 = focus.len();
    let rings = 4usize.saturating_sub(depth0).max(1);
    let c = r.center();
    let outer = r.w.min(r.h) * 0.5 - 2.0;
    let hole = outer * 0.3;
    let ring_w = (outer - hole) / rings as f32;
    let mut slices: Vec<Slice> = Vec::new();
    // (breadth first: each ring from the one inside it)
    let mut level: Vec<(&Node, f32, f32, Vec<usize>, Option<&str>)> = vec![(top, -std::f32::consts::FRAC_PI_2, -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU, focus.clone(), top_ty)];
    for ring in 0..rings {
        let mut next = Vec::new();
        for (n, a0, a1, path, ty) in level {
            if n.bytes == 0 {
                continue;
            }
            let mut a = a0;
            for (k, kid) in n.kids.iter().enumerate() {
                let span = (a1 - a0) * kid.bytes as f32 / n.bytes as f32;
                let mut p = path.clone();
                p.push(k);
                let depth = p.len();
                let kty = if depth == 2 { Some(kid.name.as_str()) } else { ty };
                slices.push(Slice { ring, a0: a, a1: a + span, path: p.clone(), depth, node: kid, ty: kty, index: k });
                next.push((kid, a, a + span, p, kty));
                a += span;
            }
        }
        level = next;
    }
    let id = id_of("setup-usage-chart");
    let (hover, _, clicked) = ui.interact(id, r);
    let m = ui.input.mouse - c;
    let dist = m.length();
    let hovered = if hover && dist > hole && dist < outer {
        let ring = ((dist - hole) / ring_w) as usize;
        let mut ang = m.y.atan2(m.x);
        if ang < -std::f32::consts::FRAC_PI_2 {
            ang += std::f32::consts::TAU;
        }
        slices.iter().position(|s| s.ring == ring && ang >= s.a0 && ang < s.a1)
    } else {
        None
    };
    for (k, s) in slices.iter().enumerate() {
        let span = s.a1 - s.a0;
        if span < 0.002 {
            continue;
        }
        let gap = (span * 0.08).min(0.006);
        let r0 = hole + s.ring as f32 * ring_w + 1.0;
        let r1 = r0 + ring_w - 2.0;
        let base = match s.ty {
            None => Color::hex(if s.index % 2 == 0 { 0x6b7078 } else { 0x50555c }),
            Some(t) => {
                let tc = type_color(t);
                let darker = match s.depth {
                    0..=2 => 0.0,
                    3 => 0.28,
                    _ => 0.45,
                } + if s.index % 2 == 1 { 0.1 } else { 0.0 };
                tc.mix(PANEL, darker)
            }
        };
        let col = if hovered == Some(k) { base.lighten(0.18) } else { base };
        ui.p().arc(c, r0, r1, s.a0 + gap, s.a1 - gap, col);
    }
    ui.p().circle(c, hole - 2.0, FIELD);
    let (title, bytes) = match hovered {
        Some(k) => (slices[k].node.name.clone(), slices[k].node.bytes),
        None => (top.name.clone(), top.bytes),
    };
    let size = fmt_size(bytes);
    ui.text_in(&size, Rect::new(c.x - hole, c.y - 16.0, hole * 2.0, 24.0), 18.0, Weight::Bold, TEXT, Align::Center);
    let name = omsi_launcher_ui::kit::fit_text(ui, &title, 11.0, hole * 1.7);
    ui.text_in(&name, Rect::new(c.x - hole, c.y + 8.0, hole * 2.0, 16.0), 11.0, Weight::Regular, TEXT_DIM, Align::Center);
    if clicked {
        match hovered {
            Some(k) if !slices[k].node.kids.is_empty() => *focus = slices[k].path.clone(),
            None if dist <= hole => {
                focus.pop();
            }
            _ => {}
        }
    }
}

/// The foot: when changes apply, Revert and Save.
fn foot(l: &mut Launcher, r: Rect) {
    l.ui.panel(r);
    let v = &l.pages.setup;
    let cfg = &l.state.config;
    let moving = v.content.as_ref().is_some_and(|s| s.trim() != cfg.content.trim());
    let edited = v.root.as_ref().is_some_and(|s| *s != cfg.root) || v.game.as_ref().is_some_and(|s| *s != cfg.game) || moving;
    // (another content folder: what the one before holds is not taken along)
    let (text, c) = if moving {
        ("Another content folder: the mods, your controls, saved situations and screenshots of the one before stay there; nothing is moved.", WARN)
    } else {
        ("Changes apply to the next game you start.", TEXT_SOFT)
    };
    let text = omsi_launcher_ui::kit::fit_text(&mut l.ui, text, 13.5, r.w - 300.0);
    l.ui.text_in(&text, Rect::new(r.x + 18.0, r.y, r.w - 300.0, r.h), 13.5, Weight::Regular, c, Align::Left);
    let save = Rect::new(r.right() - 18.0 - 104.0, r.center().y - 17.0, 104.0, 34.0);
    let revert = Rect::new(save.x - 10.0 - 92.0, save.y, 92.0, 34.0);
    if edited {
        if l.ui.button("cfg-revert", revert, "Revert", None, ButtonKind::Normal) {
            l.pages.setup.root = None;
            l.pages.setup.game = None;
            l.pages.setup.content = None;
        }
    } else {
        dim_button(&mut l.ui, revert, "Revert");
    }
    if l.ui.button("cfg-save", save, "Save", Some("save"), ButtonKind::Primary) {
        save_folders(l);
    }
}

fn save_folders(l: &mut Launcher) {
    let cfg = l.state.config.clone();
    let chosen = l.pages.setup.root.clone().unwrap_or_else(|| cfg.root.clone()).trim().to_string();
    let game = l.pages.setup.game.clone().unwrap_or_else(|| cfg.game.clone()).trim().to_string();
    let content = l.pages.setup.content.clone().unwrap_or_else(|| cfg.content.clone()).trim().to_string();
    // A folder that is no complete installation is said so, with what it lacks, and
    // nothing is saved: it was saved, then quietly replaced by the installation found
    // before, while the page said "Saved" (#1656).
    let missing = if chosen.is_empty() { Vec::new() } else { omsi_cfg::missing_original_essentials(std::path::Path::new(&chosen)) };
    if !missing.is_empty() {
        let shown: Vec<&str> = missing.iter().map(String::as_str).take(6).collect();
        let more = if missing.len() > shown.len() { format!(" and {} more", missing.len() - shown.len()) } else { String::new() };
        l.state.set_status(format!("Not saved: {chosen} is not a complete OMSI 2 installation - it lacks {}{more}", shown.join(", ")), true);
        return;
    }
    // (checked at each Save, against the OMSI 2 folder saved with it)
    if !content.is_empty() {
        if let Some(why) = core::content_folder_problem(std::path::Path::new(&content), std::path::Path::new(&chosen)) {
            l.state.set_status(format!("Not saved: the content folder cannot be used - {why}"), true);
            return;
        }
    }
    let content_moved = content != cfg.content;
    l.state.config.root = chosen.clone();
    l.state.config.game = game;
    l.state.config.content = content;
    match core::save_config(&l.state.config) {
        Ok(()) => {
            // (what was read of the folders before is forgotten: a folder copied or
            // changed while the launcher ran is read as it is now)
            omsi_cfg::content_changed();
            l.state.config = core::load_config();
            l.pages.setup = SetupView { tab: l.pages.setup.tab, ..SetupView::default() };
            let same = chosen.is_empty() || std::path::Path::new(&chosen) == std::path::Path::new(&l.state.config.root);
            if same && content_moved {
                l.state.set_status("Saved. What is in the content folder used before (mods, your controls, saved situations, screenshots) stays there; nothing is moved.", false);
            } else if same {
                l.state.set_status("Saved. Reading the content again…", false);
            } else {
                l.state.set_status(format!("Saved, but the OMSI 2 folder used is {}", l.state.config.root), true);
            }
            // (a phone: the gallery is kept out of the folder chosen at once, before
            // its media scanner lists its textures as photos, #1632)
            #[cfg(target_os = "android")]
            crate::android::hide_content_from_gallery();
            if content_moved {
                // (the mods are the new folder's)
                l.state.mods_asked = false;
            }
            l.state.load_content();
            if !super::mobile::mobile() {
                l.state.load_usage();
            }
        }
        Err(e) => l.state.set_status(format!("{e:#}"), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Node {
        let leaf = |n: &str, b: u64| Node { name: n.into(), bytes: b, kids: Vec::new() };
        let part = |n: &str, kids: Vec<Node>| Node { name: n.into(), bytes: kids.iter().map(|k| k.bytes).sum(), kids };
        part("content", vec![part("OMSI 2 folder", vec![part("Vehicles", vec![part("Bus", vec![leaf("Textures", 60), leaf("Models", 20)])]), part("Maps", vec![part("Town", vec![leaf("Settings and text", 20)])])])])
    }

    #[test]
    fn a_path_that_no_longer_fits_is_cut_back() {
        let t = tree();
        let mut path = vec![0, 0, 0];
        let (n, ty) = node_at(&t, &mut path);
        assert_eq!((n.name.as_str(), ty), ("Bus", Some("Vehicles")));
        let mut gone = vec![0, 5, 0];
        let (n, _) = node_at(&t, &mut gone);
        assert_eq!(n.name, "OMSI 2 folder");
        assert_eq!(gone, [0]);
    }

    #[test]
    fn the_content_folder_is_never_the_omsi_folder() {
        let base = std::env::temp_dir().join(format!("depot-setup-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let root = base.to_string_lossy().to_string();
        assert_eq!(check(2, &root, &root).0, Status::Error);
        assert_eq!(check(2, &base.join("depot").to_string_lossy(), &root).0, Status::Error);
        let beside = base.with_extension("content");
        assert_eq!(check(2, &beside.to_string_lossy(), &root).0, Status::Warning);
        assert_eq!(check(2, "", &root).0, Status::Found);
        assert!(core::content_folder_problem(&base, &base).is_some());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sizes_read_as_the_chart_says_them() {
        assert_eq!(fmt_size(100 * 1_073_741_824), "100 GB");
        assert_eq!(fmt_size(3 * 536_870_912), "1.5 GB");
    }
}

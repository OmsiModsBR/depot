//! The AI traffic tab of the Diagnostics page: a map's AI lists as the game takes them on a
//! chosen day (`omsi_integrity::AiTraffic`) - the car groups and their weights, the depots'
//! fleets behind the timetable's tours, the parking lists, the people and drivers, and the
//! random traffic's density over the day. Edit lists changes the map's own car groups and
//! parking lists (see `ai_edit`); everything else here only reads.

use super::Launcher;
use omsi_integrity::traffic::{clock, day, Listed, TrafficGroup};
use omsi_integrity::AiTraffic;
use omsi_launcher_ui::kit::{card, clipped, dot, right, Status};
use omsi_launcher_ui::look::*;
use omsi_ui::paint::{Align, Color};
use omsi_ui::widgets::ButtonKind;
use omsi_ui::{Rect, Weight};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

#[derive(Default)]
pub struct AiView {
    /// The map looked at (its place in the map list), once the launcher's own was taken.
    item: Option<usize>,
    /// The day, YYYY-MM-DD.
    date: String,
    /// The map's folder and day being read, and the answer.
    loading: Option<(String, i32, Receiver<AiTraffic>)>,
    /// The map's folder and day read, and what was read.
    pub(super) data: Option<(String, i32, Arc<AiTraffic>)>,
    /// The OMSI 2 folder and the content's generation (`omsi_cfg::content_generation`) of what
    /// was read: another folder, or a change to the content (a timetable saved, a mod
    /// installed), reads it again.
    seen: Option<(String, u64)>,
    /// 0 car groups, 1 bus fleet and lines, 2 parked cars, 3 people, 4 traffic density.
    tab: usize,
    group: usize,
    vehicle: Option<usize>,
    line: usize,
    tour: usize,
    /// (vehicle type, entry) of the fleet.
    bus: Option<(usize, usize)>,
    park: usize,
    car: Option<usize>,
    /// 0 `humans.txt`, 1 `drivers.txt`.
    people: usize,
    person: Option<usize>,
    /// 0 weekdays, 1 Saturday, 2 Sunday or holiday.
    day: usize,
    /// The map's `global.cfg` (as the map list names it), for the preview's light.
    map_file: String,
    /// The last frame showed a vehicle's preview (the page's own bus is not asked for then).
    showing: bool,
    /// The map's lists being edited.
    pub edit: Option<super::ai_edit::Editor>,
}

impl AiView {
    /// The tab shows the preview of one of its vehicles.
    pub fn shows_vehicle(&self) -> bool {
        self.showing
    }

    /// A window of the editing lies over the tab.
    pub fn dialog_open(&self) -> bool {
        self.edit.as_ref().is_some_and(|e| e.dialog_open())
    }
}

const ROW: f32 = 30.0;
const LIST_ROW: f32 = 54.0;

/// The colours of the density chart's groups, in order.
const SERIES: [Color; 8] = [
    Color::hex(0x4f8ff7),
    Color::hex(0x4fc27a),
    Color::hex(0xe8ab3a),
    Color::hex(0xe8628f),
    Color::hex(0x9b7bea),
    Color::hex(0x3fb8c4),
    Color::hex(0xd9d36a),
    Color::hex(0xb08a6a),
];

fn today() -> String {
    match omsi_launcher_lib::local_now() {
        Some((y, m, d, _, _)) => format!("{y:04}-{m:02}-{d:02}"),
        None => "2024-01-01".into(),
    }
}

fn read(root: PathBuf, folder: String, date: i32) -> Receiver<AiTraffic> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let dir = omsi_cfg::resolve_path(&omsi_cfg::resolve_path(&root, "Maps"), &folder);
        let _ = tx.send(AiTraffic::load(&root, &dir, date));
    });
    rx
}

/// The panel's chosen row in a list of `n`, kept inside it.
pub(super) fn keep(i: usize, n: usize) -> usize {
    i.min(n.saturating_sub(1))
}

pub fn draw(l: &mut Launcher, area: Rect) {
    l.pages.diag.ai.showing = false;
    let list = super::integrity::items(l, 0);
    let root = PathBuf::from(l.state.config.root.trim());
    if list.is_empty() || root.as_os_str().is_empty() {
        clipped(
            &mut l.ui,
            "Nothing to verify here.",
            area.x + 4.0,
            area.y + 18.0,
            area.w - 8.0,
            13.0,
            Weight::Regular,
            MUTED,
        );
        return;
    }
    // (first opened: the map and the day chosen for the drive)
    if l.pages.diag.ai.item.is_none() {
        let chosen = l
            .state
            .maps
            .iter()
            .position(|m| m.file == l.state.choice.map)
            .and_then(|i| {
                let folder = l.state.maps[i].file.split('/').nth(1)?;
                list.iter().position(|x| x.1 == folder)
            });
        l.pages.diag.ai.item = Some(chosen.unwrap_or(0));
        let date = l.state.choice.date.trim();
        l.pages.diag.ai.date = if omsi_map::ailists::date_code(date).is_some() {
            date.to_string()
        } else {
            today()
        };
    }
    let mut item = keep(l.pages.diag.ai.item.unwrap_or(0), list.len());
    // (while its lists are edited, the map stays)
    if let Some(e) = l.pages.diag.ai.edit.as_ref() {
        match list.iter().position(|x| x.1 == e.folder) {
            Some(i) => item = i,
            None => l.pages.diag.ai.edit = None,
        }
    }
    let editing = l.pages.diag.ai.edit.is_some();
    let folder = list[item].1.clone();
    l.pages.diag.ai.map_file = l
        .state
        .maps
        .iter()
        .find(|m| m.file.split('/').nth(1) == Some(folder.as_str()))
        .map(|m| m.file.clone())
        .unwrap_or_default();
    let date = omsi_map::ailists::date_code(&l.pages.diag.ai.date);

    // what was read is dropped when the OMSI 2 folder or its content changed
    let now = (
        l.state.config.root.trim().to_string(),
        omsi_cfg::content_generation(),
    );
    if l.pages.diag.ai.seen.as_ref() != Some(&now) {
        l.pages.diag.ai.data = None;
        l.pages.diag.ai.loading = None;
        l.pages.diag.ai.seen = Some(now);
    }

    // the answer of a read, and a new read when the map or the day changed
    if let Some((f, d, rx)) = l.pages.diag.ai.loading.as_ref() {
        match rx.try_recv() {
            Ok(t) => {
                let (f, d) = (f.clone(), *d);
                l.pages.diag.ai.data = Some((f, d, Arc::new(t)));
                l.pages.diag.ai.loading = None;
            }
            Err(TryRecvError::Disconnected) => l.pages.diag.ai.loading = None,
            Err(TryRecvError::Empty) => {}
        }
    }
    if let Some(date) = date {
        let wanted = (folder.as_str(), date);
        let have = l
            .pages
            .diag
            .ai
            .data
            .as_ref()
            .is_some_and(|(f, d, _)| (f.as_str(), *d) == wanted);
        let asked = l
            .pages
            .diag
            .ai
            .loading
            .as_ref()
            .is_some_and(|(f, d, _)| (f.as_str(), *d) == wanted);
        if !have && !asked {
            let rx = read(root, folder.clone(), date);
            l.pages.diag.ai.loading = Some((folder.clone(), date, rx));
        }
    }
    let data = l
        .pages
        .diag
        .ai
        .data
        .as_ref()
        .filter(|(f, d, _)| *f == folder && date.is_none_or(|x| x == *d))
        .map(|(_, _, t)| t.clone());

    // the map, the day, the scenarios in force, and the export
    let h = 36.0;
    let names: Vec<String> = list.iter().map(|x| x.0.clone()).collect();
    let mut it = item;
    let sel_w = 300.0f32.min(area.w * 0.3);
    let map_r = Rect::new(area.x, area.y, sel_w, h);
    if editing {
        l.ui.p()
            .rounded(map_r, omsi_ui::theme::CONTROL_RADIUS, PANEL_2);
        l.ui.p()
            .rounded_border(map_r, omsi_ui::theme::CONTROL_RADIUS, 1.0, LINE);
        l.ui.text_in_as_is(
            &names[item],
            Rect::new(map_r.x + 12.0, map_r.y, map_r.w - 24.0, map_r.h),
            13.0,
            Weight::Regular,
            MUTED,
        );
        l.ui.tooltip(map_r, "Stop editing to look at another map.");
    } else if l.ui.select_as_is("ai-map", map_r, &mut it, &names) {
        l.pages.diag.ai.item = Some(it);
        reset(l);
    }
    let date_r = Rect::new(map_r.right() + 10.0, area.y, 170.0, h);
    let mut d = l.pages.diag.ai.date.clone();
    if l.ui.date_field("ai-date", date_r, &mut d) {
        l.pages.diag.ai.date = d;
    }
    l.ui.tooltip(
        date_r,
        "The day the lists are shown for: the Chrono scenarios in force then and the fleet in service.",
    );
    if let Some(t) = data.as_ref() {
        let chrono = if t.chrono.is_empty() {
            omsi_ui::tr("Chrono: no scenario in force").to_string()
        } else {
            omsi_ui::tr("Chrono: %{n} in force").replace("%{n}", &t.chrono.len().to_string())
        };
        let cw = l.ui.width(&chrono, 12.0, Weight::Regular) + 26.0;
        let chip = Rect::new(date_r.right() + 10.0, area.y + 4.0, cw, h - 8.0);
        l.ui.p().rounded(chip, 14.0, PANEL_2);
        l.ui.p().rounded_border(chip, 14.0, 1.0, LINE);
        l.ui.text_in_as_is(&chrono, chip.inset(2.0), 12.0, Weight::Regular, MUTED);
        let tip = if t.chrono.is_empty() {
            omsi_ui::tr("No Chrono scenario of this map is in force on this day.").to_string()
        } else {
            t.chrono.join("\n")
        };
        l.ui.tooltip(chip, &tip);
    }
    let export_r = Rect::new(area.right() - 170.0, area.y, 170.0, h);
    let can_export = data.is_some();
    if l.ui.button(
        "ai:export",
        export_r,
        "Export (TXT)",
        Some("o-save"),
        ButtonKind::Normal,
    ) && can_export
    {
        if let Some(t) = data.as_ref() {
            export(l, t, &list[item]);
        }
    }
    if !can_export {
        l.ui.p()
            .rounded(export_r, omsi_ui::theme::CONTROL_RADIUS, PANEL.alpha(0.6));
    }
    l.ui.tooltip(
        export_r,
        "Saves everything on this tab for the chosen map and day as a text file.",
    );
    if !editing {
        let edit_r = Rect::new(export_r.x - 10.0 - 150.0, area.y, 150.0, h);
        let why = super::ai_edit::cannot_edit(l, &folder);
        let clicked = l.ui.button(
            "ai:edit",
            edit_r,
            "Edit lists",
            Some("tune"),
            ButtonKind::Normal,
        );
        if why.is_some() {
            l.ui.p()
                .rounded(edit_r, omsi_ui::theme::CONTROL_RADIUS, PANEL.alpha(0.6));
        }
        l.ui.tooltip(
            edit_r,
            why.unwrap_or(
                "Change the map's car groups and parking lists. The files are copied before each save.",
            ),
        );
        if clicked && why.is_none() {
            let name = names[item].clone();
            super::ai_edit::start(l, &folder, &name);
        }
    }

    // the editing banner
    let mut ty = area.y + h + 12.0;
    if editing {
        super::ai_edit::banner(l, Rect::new(area.x, ty, area.w, 46.0));
        ty += 58.0;
    }

    // a tab for each part of the traffic
    let mut counts = data
        .as_ref()
        .map(|t| {
            [
                t.groups.iter().filter(|g| !g.is_depot).count(),
                t.lines.len(),
                t.parklists.len(),
                usize::from(!t.humans.is_empty()) + usize::from(!t.drivers.is_empty()),
                t.density.len(),
            ]
        })
        .unwrap_or_default();
    if let Some(e) = l.pages.diag.ai.edit.as_ref() {
        (counts[0], counts[2]) = super::ai_edit::counts(e);
    }
    let labels = [
        (
            "Car groups",
            "The groups of cars and trucks of ailists.cfg, with their weights.",
        ),
        (
            "Bus fleet and lines",
            "The timetable's lines, the AI group of each tour, and the depot's buses behind it.",
        ),
        (
            "Parked cars",
            "The parking lists the map's parking spaces take cars from.",
        ),
        (
            "People",
            "The people on foot and the drivers the map draws from.",
        ),
        (
            "Traffic density",
            "How much random traffic each group makes over the day.",
        ),
    ];
    let bar = Rect::new(area.x, ty, area.w, 36.0);
    if let Some(i) = count_tabs(l, bar, &labels, &counts, l.pages.diag.ai.tab) {
        l.pages.diag.ai.tab = i;
    }

    let body = Rect::new(
        area.x,
        ty + 48.0,
        area.w,
        (area.bottom() - ty - 56.0).max(160.0),
    );
    // the buses of the lines: the lines are those read, the fleet is the map's own file
    if l.pages.diag.ai.edit.is_some() && l.pages.diag.ai.tab == 1 {
        if let Some(t) = data.as_ref() {
            super::ai_edit::bus_lines(l, body, t);
            return;
        }
    }
    // the lists being edited are the map's own, read from its files
    if l.pages.diag.ai.edit.is_some() && matches!(l.pages.diag.ai.tab, 0 | 2) {
        if l.pages.diag.ai.tab == 0 {
            super::ai_edit::car_groups(l, body);
        } else {
            super::ai_edit::parked(l, body);
        }
        return;
    }
    let Some(t) = data else {
        let r = body;
        l.ui.solid(r);
        l.ui.p()
            .rounded(r, omsi_ui::theme::RADIUS, omsi_ui::theme::surface(PANEL));
        l.ui.p()
            .rounded_border(r, omsi_ui::theme::RADIUS, 1.0, LINE);
        let msg = if date.is_none() {
            "Type a day as YYYY-MM-DD, or pick one in the calendar."
        } else {
            "Reading the map's AI lists…"
        };
        clipped(
            &mut l.ui,
            msg,
            r.x + 19.0,
            r.y + 28.0,
            r.w - 38.0,
            13.0,
            Weight::Regular,
            MUTED,
        );
        if date.is_some() {
            l.ui.progress(
                Rect::new(r.x + 19.0, r.y + 46.0, r.w - 38.0, 4.0),
                0.35,
                true,
            );
        }
        return;
    };
    match l.pages.diag.ai.tab {
        0 => car_groups(l, body, &t),
        1 => bus_lines(l, body, &t),
        2 => parked(l, body, &t),
        3 => people(l, body, &t),
        _ => density(l, body, &t),
    }
}

/// Another map: the choices in its lists start over.
fn reset(l: &mut Launcher) {
    let a = &mut l.pages.diag.ai;
    a.group = 0;
    a.vehicle = None;
    a.line = 0;
    a.tour = 0;
    a.bus = None;
    a.park = 0;
    a.car = None;
    a.person = None;
}

fn export(l: &mut Launcher, t: &AiTraffic, map: &(String, String)) {
    let clean: String = map
        .0
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = format!("{} - AI traffic {}.txt", clean.trim(), day(t.date));
    let title = omsi_ui::tr("Export the AI traffic").to_string();
    let Some(path) = omsi_launcher_lib::save_file(&title, &name) else {
        return;
    };
    let mut header = vec![("Map".to_string(), format!("{} ({})", map.0, map.1))];
    if let Some((y, mo, d, h, m)) = omsi_launcher_lib::local_now() {
        header.push((
            "Exported".into(),
            format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}"),
        ));
    }
    header.push((
        "Build".into(),
        format!(
            "Project Depot {} ({})",
            crate::startup::VERSION,
            crate::startup::BUILD
        ),
    ));
    match std::fs::write(&path, t.to_text(&header)) {
        Ok(()) => l
            .state
            .set_status("The AI traffic was saved as a text file.", false),
        Err(e) => {
            log::warn!("AI traffic export to {}: {e}", path.display());
            l.state
                .set_status("The text file could not be saved.", true);
        }
    }
}

/// A tab bar with a count badge on each tab; the tab clicked, if any.
fn count_tabs(
    l: &mut Launcher,
    bar: Rect,
    labels: &[(&str, &str)],
    counts: &[usize],
    on: usize,
) -> Option<usize> {
    l.ui.p()
        .rect(Rect::new(bar.x, bar.bottom() - 1.0, bar.w, 1.0), LINE);
    let px = 12.5;
    let texts: Vec<String> = labels
        .iter()
        .map(|(t, _)| omsi_ui::tr(t).to_string())
        .collect();
    let natural: f32 = texts
        .iter()
        .zip(counts)
        .map(|(t, n)| {
            l.ui.width(t, px, Weight::Medium)
                + l.ui.width(&n.to_string(), 11.0, Weight::Medium)
                + 46.0
        })
        .sum();
    let squeeze = (bar.w / natural).min(1.0);
    let mut x = bar.x;
    let mut picked = None;
    for (i, label) in texts.iter().enumerate() {
        let n = counts.get(i).copied().unwrap_or(0);
        let lw = l.ui.width(label, px, Weight::Medium);
        let nw = l.ui.width(&n.to_string(), 11.0, Weight::Medium) + 12.0;
        let w = (lw + nw + 46.0) * squeeze;
        let cell = Rect::new(x, bar.y, w, bar.h);
        let id = omsi_ui::widgets::id_of(&format!("ai-tab-{i}"));
        let (hover, _, clicked) = l.ui.interact_key(id, cell);
        if clicked {
            picked = Some(i);
        }
        let sel = on == i;
        if hover && !sel {
            l.ui.p().rounded(cell.inset(1.0), 4.0, HOVER_LAYER);
        }
        let tx = x + 14.0 * squeeze;
        let room = (w - (tx - x) - nw - 6.0).max(10.0);
        l.ui.text_in_as_is(
            label,
            Rect::new(tx, bar.y, room, bar.h),
            px,
            Weight::Medium,
            if sel { FG } else { MUTED },
        );
        let badge = Rect::new(
            (tx + room.min(lw) + 6.0).min(cell.right() - nw),
            bar.y + bar.h * 0.5 - 9.0,
            nw,
            18.0,
        );
        l.ui.p().rounded(badge, 9.0, PANEL_2);
        l.ui.text_in(
            &n.to_string(),
            badge,
            11.0,
            Weight::Medium,
            MUTED,
            Align::Center,
        );
        if sel {
            l.ui.p().rounded(
                Rect::new(cell.x + 8.0, bar.bottom() - 3.0, cell.w - 16.0, 3.0),
                1.5,
                omsi_ui::theme::accent(),
            );
        }
        l.ui.tooltip(cell, labels[i].1);
        x += w;
    }
    picked
}

/// The three panels of a tab: the lists at the left, what the chosen one holds in the middle,
/// and the details of the chosen entry at the right.
pub(super) fn columns(body: Rect) -> (Rect, Rect, Rect) {
    let gap = 12.0;
    let lw = (body.w * 0.2).clamp(200.0, 260.0);
    let rw = (body.w * 0.24).clamp(240.0, 330.0);
    let left = Rect::new(body.x, body.y, lw, body.h);
    let right = Rect::new(body.right() - rw, body.y, rw, body.h);
    let mid = Rect::new(
        left.right() + gap,
        body.y,
        (right.x - gap - left.right() - gap).max(120.0),
        body.h,
    );
    (left, mid, right)
}

/// A line of quiet text at the foot of a panel.
pub(super) fn foot(l: &mut Launcher, r: Rect, s: &str) {
    l.ui.p().rect(
        Rect::new(r.x + 1.0, r.bottom() - 50.0, r.w - 2.0, 1.0),
        LINE,
    );
    l.ui.paragraph(
        s,
        glam::Vec2::new(r.x + 17.0, r.bottom() - 42.0),
        r.w - 34.0,
        11.5,
        Weight::Regular,
        MUTED,
    );
}

/// A list of entries (title, line under it, status) in a panel; the entry clicked, if any.
pub(super) fn entries(
    l: &mut Launcher,
    id: &str,
    r: Rect,
    rows: &[(String, String, Status)],
    sel: usize,
) -> Option<usize> {
    let mut picked = None;
    l.ui.scroll_area(id, r, &mut |ui, v| {
        let mut y = v.y;
        for (i, (title, sub, st)) in rows.iter().enumerate() {
            let cell = Rect::new(v.x + 8.0, y + 2.0, v.w - 16.0, LIST_ROW - 4.0);
            let (hover, _, clicked) =
                ui.interact_key(omsi_ui::widgets::id_of(&format!("{id}-{i}")), cell);
            if clicked {
                picked = Some(i);
            }
            if i == sel {
                ui.p().rounded(cell, 6.0, SELECTED_LAYER);
                ui.p()
                    .rounded_border(cell, 6.0, 1.0, omsi_ui::theme::accent().alpha(0.7));
            } else if hover {
                ui.p().rounded(cell, 6.0, HOVER_LAYER);
            }
            ui.text_in_as_is(
                title,
                Rect::new(cell.x + 12.0, cell.y + 5.0, cell.w - 40.0, 22.0),
                13.5,
                Weight::Medium,
                FG,
            );
            ui.text_in_as_is(
                sub,
                Rect::new(cell.x + 12.0, cell.y + 25.0, cell.w - 40.0, 20.0),
                11.5,
                Weight::Regular,
                MUTED,
            );
            dot(ui, cell.right() - 14.0, cell.y + cell.h * 0.5, *st);
            y += LIST_ROW;
        }
        y - v.y
    });
    picked
}

/// A column of a table: its heading, its share of the width, and how it is aligned.
pub(super) struct Col(pub(super) &'static str, pub(super) f32, pub(super) Align);

/// A table (heading row, then rows of cells) in `r`; the row clicked, if any. A row whose
/// status is not `Found` gets a dot of its colour; `dim` rows are drawn quieter.
pub(super) fn table(
    l: &mut Launcher,
    id: &str,
    r: Rect,
    cols: &[Col],
    rows: &[(Vec<String>, Status, bool)],
    sel: Option<usize>,
) -> Option<usize> {
    let total: f32 = cols.iter().map(|c| c.1).sum();
    let xs = |w: f32| -> Vec<(f32, f32)> {
        let mut x = 28.0;
        cols.iter()
            .map(|c| {
                let cw = (w - 36.0) * c.1 / total;
                let at = (x, cw);
                x += cw;
                at
            })
            .collect()
    };
    let head = Rect::new(r.x, r.y, r.w, ROW);
    for (c, (x, w)) in cols.iter().zip(xs(r.w)) {
        let cell = Rect::new(head.x + x, head.y, w - 8.0, ROW);
        let s = omsi_ui::tr(c.0).to_string();
        l.ui.text_in(&s, cell, 12.0, Weight::Regular, MUTED, c.2);
    }
    let body = Rect::new(r.x, r.y + ROW, r.w, (r.h - ROW).max(0.0));
    let mut picked = None;
    l.ui.scroll_area(id, body, &mut |ui, v| {
        let mut y = v.y;
        let at = xs(v.w);
        for (i, (cells, st, dim)) in rows.iter().enumerate() {
            let row = Rect::new(v.x + 4.0, y, v.w - 8.0, ROW + 4.0);
            let (hover, _, clicked) =
                ui.interact_key(omsi_ui::widgets::id_of(&format!("{id}-{i}")), row);
            if clicked {
                picked = Some(i);
            }
            if sel == Some(i) {
                ui.p().rounded(row, 4.0, SELECTED_LAYER);
                ui.p().rect(
                    Rect::new(row.x, row.y + 6.0, 3.0, row.h - 12.0),
                    omsi_ui::theme::accent(),
                );
            } else if hover {
                ui.p().rounded(row, 4.0, HOVER_LAYER);
            }
            if !matches!(st, Status::Found) {
                dot(ui, v.x + 16.0, y + row.h * 0.5, *st);
            }
            let c = if *dim { MUTED } else { FG };
            for (k, (x, w)) in at.iter().enumerate() {
                let s = cells.get(k).map(String::as_str).unwrap_or("");
                let cell = Rect::new(v.x + x, y, w - 8.0, row.h);
                let align = cols.get(k).map(|c| c.2).unwrap_or(Align::Left);
                if align == Align::Left {
                    ui.text_in_as_is(s, cell, 12.5, Weight::Regular, c);
                } else {
                    ui.text_in(s, cell, 12.5, Weight::Regular, c, align);
                }
            }
            y += row.h;
        }
        y - v.y
    });
    picked
}

/// The details of the chosen entry: (label, value) rows under the card's title.
pub(super) fn details(l: &mut Launcher, r: Rect, rows: &[(&str, String)]) {
    card(&mut l.ui, r, "Details", None);
    let lw = (r.w * 0.36).max(90.0);
    // (the rows scroll when the card is short: under a vehicle's picture)
    let list = Rect::new(r.x, r.y + 46.0, r.w, (r.h - 50.0).max(0.0));
    l.ui.scroll_area("ai-details", list, &mut |ui, v| {
        let mut y = v.y + 4.0;
        for (k, val) in rows {
            ui.p()
                .rect(Rect::new(v.x + 1.0, y - 4.0, v.w - 2.0, 1.0), LINE);
            clipped(
                ui,
                k,
                v.x + 17.0,
                y + 13.0,
                lw - 10.0,
                12.0,
                Weight::Regular,
                MUTED,
            );
            ui.text_in_as_is(
                val,
                Rect::new(v.x + 17.0 + lw, y, v.w - lw - 30.0, 26.0),
                12.5,
                Weight::Regular,
                FG,
            );
            ui.tooltip(Rect::new(v.x, y - 4.0, v.w, 30.0), val);
            y += 31.0;
        }
        y - v.y
    });
}

/// The vehicle `file` in `paint` (empty: its own) at the top of `r`, as the preview helper
/// draws it: the picture kept from an earlier time at once, the live view when it is turned.
/// Returns the rest of `r`, under it.
pub(super) fn vehicle_view(
    l: &mut Launcher,
    r: Rect,
    file: &str,
    paint: &str,
    exists: bool,
) -> Rect {
    let h = (r.w * 0.68).min(r.h * 0.5).max(120.0);
    let top = Rect::new(r.x, r.y, r.w, h);
    if !exists || l.state.in_game() || l.pages.diag.ai.map_file.is_empty() {
        l.ui.solid(top);
        l.ui.p()
            .rounded(top, omsi_ui::theme::RADIUS, omsi_ui::theme::FIELD);
        let why = if !exists {
            "The file is missing: there is nothing to show."
        } else if l.state.in_game() {
            "No preview while the game runs."
        } else {
            "No preview for this map."
        };
        l.ui.text_in(why, top, 12.5, Weight::Regular, MUTED, Align::Center);
    } else {
        let c = &l.state.choice;
        let look = super::showroom::Look {
            root: PathBuf::from(l.state.config.root.trim()),
            map: l.pages.diag.ai.map_file.clone(),
            bus: file.to_string(),
            paint: paint.to_string(),
            weather: c.weather.clone(),
            time: c.time,
            date: l.pages.diag.ai.date.clone(),
        };
        l.preview.want_picture(look);
        l.preview.focus_x = 0.5;
        l.preview(top);
        l.pages.diag.ai.showing = true;
    }
    Rect::new(
        r.x,
        top.bottom() + 12.0,
        r.w,
        (r.bottom() - top.bottom() - 12.0).max(80.0),
    )
}

pub(super) fn status_of(exists: bool) -> Status {
    if exists {
        Status::Found
    } else {
        Status::Error
    }
}

fn worst(all: impl IntoIterator<Item = bool>) -> Status {
    if all.into_iter().all(|x| x) {
        Status::Found
    } else {
        Status::Error
    }
}

pub(super) fn weight_text(w: f32) -> String {
    if w.fract() == 0.0 {
        format!("{w:.0}")
    } else {
        format!("{w}")
    }
}

/// `one` for one, else `more` with the count in place of `%{n}`, translated.
pub(super) fn many(n: usize, one: &str, more: &str) -> String {
    if n == 1 {
        omsi_ui::tr(one).to_string()
    } else {
        omsi_ui::tr(more).replace("%{n}", &n.to_string())
    }
}

pub(super) fn percent(x: f32) -> String {
    format!("{:.1} %", x * 100.0)
}

/// What the game calls a vehicle: its friendly name, else its file's name.
pub(super) fn shown_name(name: &str, file: &str) -> String {
    if name.is_empty() {
        file.rsplit(['\\', '/']).next().unwrap_or(file).to_string()
    } else {
        name.to_string()
    }
}

fn car_groups(l: &mut Launcher, body: Rect, t: &AiTraffic) {
    let (left, mid, right_r) = columns(body);
    let groups: Vec<(usize, &TrafficGroup)> = t
        .groups
        .iter()
        .enumerate()
        .filter(|(_, g)| !g.is_depot)
        .collect();
    card(&mut l.ui, left, "Groups", Some(&groups.len().to_string()));
    if groups.is_empty() {
        clipped(
            &mut l.ui,
            "This map has no car groups.",
            left.x + 17.0,
            left.y + 60.0,
            left.w - 34.0,
            12.5,
            Weight::Regular,
            MUTED,
        );
        foot(
            l,
            left,
            "From ailists.cfg, with the Chrono scenarios in force on the chosen day.",
        );
        return;
    }
    let sel = keep(l.pages.diag.ai.group, groups.len());
    let rows: Vec<(String, String, Status)> = groups
        .iter()
        .map(|(i, g)| {
            let mut title = g.name.trim().to_string();
            if *i == t.default_group {
                title = format!("{title} · {}", omsi_ui::tr("default"));
            }
            (
                title,
                many(g.vehicles.len(), "1 vehicle", "%{n} vehicles"),
                worst(g.vehicles.iter().map(|v| v.exists)),
            )
        })
        .collect();
    let list_r = Rect::new(left.x, left.y + 40.0, left.w, left.h - 96.0);
    if let Some(i) = entries(l, "ai-groups", list_r, &rows, sel) {
        l.pages.diag.ai.group = i;
        l.pages.diag.ai.vehicle = None;
    }
    foot(
        l,
        left,
        "From ailists.cfg, with the Chrono scenarios in force on the chosen day.",
    );

    let g = groups[sel].1;
    let note = format!(
        "{} · {}",
        many(g.vehicles.len(), "1 vehicle", "%{n} vehicles"),
        omsi_ui::tr("weights add up to %{w}").replace("%{w}", &weight_text(g.total_weight()))
    );
    card(&mut l.ui, mid, g.name.trim(), Some(&note));
    let rows: Vec<(Vec<String>, Status, bool)> = g
        .vehicles
        .iter()
        .enumerate()
        .map(|(k, v)| {
            (
                vec![
                    shown_name(&v.name, &v.file),
                    v.file.clone(),
                    weight_text(v.weight),
                    percent(g.chance(k)),
                ],
                status_of(v.exists),
                v.weight <= 0.0,
            )
        })
        .collect();
    let table_r = Rect::new(mid.x, mid.y + 40.0, mid.w, mid.h - 96.0);
    let cols = [
        Col("Vehicle", 2.2, Align::Left),
        Col("File", 3.0, Align::Left),
        Col("Weight", 0.8, Align::Right),
        Col("Chance", 1.0, Align::Right),
    ];
    let vsel = l.pages.diag.ai.vehicle.filter(|&v| v < g.vehicles.len());
    if let Some(i) = table(l, "ai-vehicles", table_r, &cols, &rows, vsel) {
        l.pages.diag.ai.vehicle = Some(i);
    }
    let random = match g.random {
        Some(c) => omsi_ui::tr("The random traffic uses this group (density class %{c}).")
            .replace("%{c}", &c.to_string()),
        None if !t.has_random_groups && groups[sel].0 == t.default_group => {
            omsi_ui::tr("The map has no unsched_vehgroups.txt: the random traffic takes its vehicles from this default group.").to_string()
        }
        None => omsi_ui::tr("The random traffic does not use this group; tours or paths may.").to_string(),
    };
    foot(
        l,
        mid,
        &format!(
            "{random} {}",
            omsi_ui::tr("Chance = weight / sum of the group's weights.")
        ),
    );

    match vsel.and_then(|v| g.vehicles.get(v).map(|x| (v, x))) {
        Some((k, v)) => {
            let w = omsi_ui::tr("%{w} of %{total} (%{p})")
                .replace("%{w}", &weight_text(v.weight))
                .replace("%{total}", &weight_text(g.total_weight()))
                .replace("%{p}", &percent(g.chance(k)));
            let rest = vehicle_view(l, right_r, &v.file, "", v.exists);
            details(
                l,
                rest,
                &[
                    ("Vehicle", shown_name(&v.name, &v.file)),
                    ("File", v.file.clone()),
                    ("Weight", w),
                    ("Listed in", v.listed_in.clone()),
                    (
                        "State",
                        omsi_ui::tr(if v.exists { "Installed" } else { "Missing" }).to_string(),
                    ),
                ],
            );
        }
        None => {
            let missing = g.vehicles.iter().filter(|v| !v.exists).count();
            details(
                l,
                right_r,
                &[
                    ("Group", g.name.trim().to_string()),
                    ("Vehicles", g.vehicles.len().to_string()),
                    ("Missing", missing.to_string()),
                    (
                        "Random traffic",
                        match g.random {
                            Some(c) => {
                                omsi_ui::tr("Density class %{c}").replace("%{c}", &c.to_string())
                            }
                            None => omsi_ui::tr("Not used").to_string(),
                        },
                    ),
                    ("Tours", t.tours_of(&g.name).len().to_string()),
                ],
            );
        }
    }
}

fn bus_lines(l: &mut Launcher, body: Rect, t: &AiTraffic) {
    let (left, mid, right_r) = columns(body);
    card(&mut l.ui, left, "Lines", Some(&t.lines.len().to_string()));
    if t.lines.is_empty() {
        clipped(
            &mut l.ui,
            "This map has no timetable lines.",
            left.x + 17.0,
            left.y + 60.0,
            left.w - 34.0,
            12.5,
            Weight::Regular,
            MUTED,
        );
        foot(
            l,
            left,
            "From TTData, with the Chrono scenarios in force on the chosen day.",
        );
        return;
    }
    let sel = keep(l.pages.diag.ai.line, t.lines.len());
    let rows: Vec<(String, String, Status)> = t
        .lines
        .iter()
        .map(|line| {
            let trips: usize = line.tours.iter().map(|x| x.trips).sum();
            (
                line.name.clone(),
                format!(
                    "{} · {}",
                    many(line.tours.len(), "1 tour", "%{n} tours"),
                    many(trips, "1 trip", "%{n} trips")
                ),
                if line.tours.iter().all(|x| x.group_known) {
                    Status::Found
                } else {
                    Status::Warning
                },
            )
        })
        .collect();
    let list_r = Rect::new(left.x, left.y + 40.0, left.w, left.h - 96.0);
    if let Some(i) = entries(l, "ai-lines", list_r, &rows, sel) {
        l.pages.diag.ai.line = i;
        l.pages.diag.ai.tour = 0;
        l.pages.diag.ai.bus = None;
    }
    foot(
        l,
        left,
        "From TTData, with the Chrono scenarios in force on the chosen day.",
    );

    // the line's tours
    let line = &t.lines[sel];
    let tour_h = (mid.h * 0.45).max(150.0);
    let tours_r = Rect::new(mid.x, mid.y, mid.w, tour_h);
    let title = omsi_ui::tr("Line %{name} · tours").replace("%{name}", &line.name);
    let types = match &line.types {
        Some((f, list)) if *f >= 1.0 => {
            omsi_ui::tr("car_use: only %{types}").replace("%{types}", &short_list(list))
        }
        Some((f, list)) => omsi_ui::tr("car_use: %{types} preferred (%{p})")
            .replace("%{types}", &short_list(list))
            .replace("%{p}", &percent(*f)),
        None => omsi_ui::tr("car_use: any bus of the group").to_string(),
    };
    card(&mut l.ui, tours_r, &title, Some(&types));
    let tsel = keep(l.pages.diag.ai.tour, line.tours.len());
    let rows: Vec<(Vec<String>, Status, bool)> = line
        .tours
        .iter()
        .map(|x| {
            (
                vec![
                    x.number.clone(),
                    x.ai_group.clone(),
                    x.trips.to_string(),
                    x.first.map(clock).unwrap_or_default(),
                    x.last.map(clock).unwrap_or_default(),
                ],
                if x.group_known {
                    Status::Found
                } else {
                    Status::Warning
                },
                false,
            )
        })
        .collect();
    let cols = [
        Col("Tour", 1.0, Align::Left),
        Col("AI group", 2.0, Align::Left),
        Col("Trips", 0.7, Align::Right),
        Col("First", 0.8, Align::Right),
        Col("Last", 0.8, Align::Right),
    ];
    let table_r = Rect::new(tours_r.x, tours_r.y + 40.0, tours_r.w, tours_r.h - 46.0);
    if let Some(i) = table(l, "ai-tours", table_r, &cols, &rows, Some(tsel)) {
        l.pages.diag.ai.tour = i;
        l.pages.diag.ai.bus = None;
    }

    // the depot behind the chosen tour
    let fleet_r = Rect::new(
        mid.x,
        tours_r.bottom() + 12.0,
        mid.w,
        mid.bottom() - tours_r.bottom() - 12.0,
    );
    let Some(tour) = line.tours.get(tsel) else {
        card(&mut l.ui, fleet_r, "Depot fleet", None);
        details(l, right_r, &[]);
        return;
    };
    let Some(group) = t.group(&tour.ai_group) else {
        card(&mut l.ui, fleet_r, "Depot fleet", None);
        l.ui.paragraph(
            &omsi_ui::tr("No AI list has the group \"%{g}\": the game has no bus to take for this tour. The Integrity check names it too.")
                .replace("%{g}", &tour.ai_group),
            glam::Vec2::new(fleet_r.x + 17.0, fleet_r.y + 46.0),
            fleet_r.w - 34.0,
            12.5,
            Weight::Regular,
            WARN,
        );
        details(
            l,
            right_r,
            &[
                ("Tour", format!("{}/{}", line.name, tour.number)),
                ("AI group", tour.ai_group.clone()),
                ("State", omsi_ui::tr("Unknown group").to_string()),
            ],
        );
        return;
    };
    if !group.is_depot {
        // a plain group: its vehicles by weight
        let title = omsi_ui::tr("Group %{g}").replace("%{g}", group.name.trim());
        card(
            &mut l.ui,
            fleet_r,
            &title,
            Some(&many(group.vehicles.len(), "1 vehicle", "%{n} vehicles")),
        );
        let rows: Vec<(Vec<String>, Status, bool)> = group
            .vehicles
            .iter()
            .enumerate()
            .map(|(k, v)| {
                (
                    vec![
                        shown_name(&v.name, &v.file),
                        v.file.clone(),
                        percent(group.chance(k)),
                    ],
                    status_of(v.exists),
                    false,
                )
            })
            .collect();
        let cols = [
            Col("Vehicle", 2.0, Align::Left),
            Col("File", 3.0, Align::Left),
            Col("Chance", 1.0, Align::Right),
        ];
        let r = Rect::new(fleet_r.x, fleet_r.y + 40.0, fleet_r.w, fleet_r.h - 46.0);
        table(l, "ai-tour-group", r, &cols, &rows, None);
        details(
            l,
            right_r,
            &[
                ("Tour", format!("{}/{}", line.name, tour.number)),
                ("AI group", group.name.trim().to_string()),
                ("Kind", omsi_ui::tr("Car group").to_string()),
            ],
        );
        return;
    }
    let all: usize = group.fleet.iter().map(|f| f.entries.len()).sum();
    let note = omsi_ui::tr("%{hof} · %{n} of %{all} in service")
        .replace("%{hof}", group.hof.as_deref().unwrap_or("-"))
        .replace("%{n}", &group.in_service().to_string())
        .replace("%{all}", &all.to_string());
    let title = omsi_ui::tr("Depot fleet · %{g}").replace("%{g}", group.name.trim());
    card(&mut l.ui, fleet_r, &title, Some(&note));
    let mut index: Vec<(usize, usize)> = Vec::new();
    let mut rows: Vec<(Vec<String>, Status, bool)> = Vec::new();
    for (ti, ft) in group.fleet.iter().enumerate() {
        for (ei, (e, on)) in ft.entries.iter().enumerate() {
            index.push((ti, ei));
            rows.push((
                vec![
                    e.number.clone(),
                    e.registration.clone(),
                    e.paint.clone(),
                    shown_name(&ft.name, &ft.file),
                    e.from.map(day).unwrap_or_default(),
                    e.to.map(day).unwrap_or_default(),
                ],
                status_of(ft.exists),
                !on,
            ));
        }
    }
    let cols = [
        Col("Number", 0.8, Align::Left),
        Col("Plate", 1.1, Align::Left),
        Col("Repaint", 1.6, Align::Left),
        Col("Bus", 1.8, Align::Left),
        Col("From", 1.0, Align::Left),
        Col("Until", 1.0, Align::Left),
    ];
    let r = Rect::new(fleet_r.x, fleet_r.y + 40.0, fleet_r.w, fleet_r.h - 46.0);
    let bsel = l
        .pages
        .diag
        .ai
        .bus
        .and_then(|b| index.iter().position(|x| *x == b));
    if let Some(i) = table(l, "ai-fleet", r, &cols, &rows, bsel) {
        l.pages.diag.ai.bus = index.get(i).copied();
    }
    let tours = t.tours_of(&group.name);
    let tours_text = if tours.len() > 4 {
        format!("{} … ({})", tours[..4].join(", "), tours.len())
    } else {
        tours.join(", ")
    };
    match l
        .pages
        .diag
        .ai
        .bus
        .and_then(|(ti, ei)| Some((group.fleet.get(ti)?, group.fleet.get(ti)?.entries.get(ei)?)))
    {
        Some((ft, (e, on))) => {
            let service = match (e.from, e.to) {
                (None, None) => omsi_ui::tr("Always").to_string(),
                (Some(f), None) => omsi_ui::tr("From %{d}").replace("%{d}", &day(f)),
                (None, Some(u)) => omsi_ui::tr("Until %{d}").replace("%{d}", &day(u)),
                (Some(f), Some(u)) => omsi_ui::tr("%{a} to %{b}")
                    .replace("%{a}", &day(f))
                    .replace("%{b}", &day(u)),
            };
            let rest = vehicle_view(l, right_r, &ft.file, &e.paint, ft.exists);
            details(
                l,
                rest,
                &[
                    ("Bus", shown_name(&ft.name, &ft.file)),
                    ("File", ft.file.clone()),
                    ("Number", format!("{} · {}", e.number, e.registration)),
                    ("Repaint", e.paint.clone()),
                    ("In service", service),
                    (
                        "On this day",
                        omsi_ui::tr(if *on { "In service" } else { "Not in service" }).to_string(),
                    ),
                    ("Tours", tours_text),
                    ("Listed in", ft.listed_in.clone()),
                ],
            );
        }
        None => details(
            l,
            right_r,
            &[
                ("Tour", format!("{}/{}", line.name, tour.number)),
                ("AI group", group.name.trim().to_string()),
                ("Depot file", group.hof.clone().unwrap_or_default()),
                ("In service", format!("{} / {all}", group.in_service())),
                ("Tours", tours_text),
            ],
        ),
    }
}

/// A list of files shortened to their names, at most three.
fn short_list(list: &[String]) -> String {
    let names: Vec<String> = list.iter().map(|f| shown_name("", f)).collect();
    if names.len() > 3 {
        format!("{} …", names[..3].join(", "))
    } else {
        names.join(", ")
    }
}

/// A list of files (parked cars, people) in the middle panel and the chosen one's details.
fn files_panel(
    l: &mut Launcher,
    mid: Rect,
    right_r: Rect,
    title: &str,
    id: &str,
    list: &[Listed],
    sel: Option<usize>,
) -> Option<usize> {
    let total: usize = list.iter().map(|c| c.count).sum();
    let missing = list.iter().filter(|c| !c.exists).count();
    let note = if missing > 0 {
        format!(
            "{}, {}",
            many(total, "1 entry", "%{n} entries"),
            omsi_ui::tr("%{m} missing").replace("%{m}", &missing.to_string())
        )
    } else {
        many(total, "1 entry", "%{n} entries")
    };
    card(&mut l.ui, mid, title, Some(&note));
    let rows: Vec<(Vec<String>, Status, bool)> = list
        .iter()
        .map(|c| {
            (
                vec![
                    shown_name("", &c.file),
                    c.file.clone(),
                    format!("{}×", c.count),
                    percent(c.count as f32 / total.max(1) as f32),
                ],
                status_of(c.exists),
                false,
            )
        })
        .collect();
    let cols = [
        Col("Name", 1.6, Align::Left),
        Col("File", 3.0, Align::Left),
        Col("Listed", 0.7, Align::Right),
        Col("Chance", 0.9, Align::Right),
    ];
    let r = Rect::new(mid.x, mid.y + 40.0, mid.w, mid.h - 46.0);
    let sel = sel.filter(|&s| s < list.len());
    let picked = table(l, id, r, &cols, &rows, sel);
    match sel.and_then(|s| list.get(s)) {
        Some(c) => details(
            l,
            right_r,
            &[
                ("Name", shown_name("", &c.file)),
                ("File", c.file.clone()),
                (
                    "In the list",
                    format!(
                        "{} ({})",
                        many(c.count, "Once", "%{n} times"),
                        omsi_ui::tr("chance %{n} of %{total}")
                            .replace("%{n}", &c.count.to_string())
                            .replace("%{total}", &total.to_string())
                    ),
                ),
                (
                    "State",
                    omsi_ui::tr(if c.exists { "Installed" } else { "Missing" }).to_string(),
                ),
            ],
        ),
        None => details(
            l,
            right_r,
            &[
                ("Entries", total.to_string()),
                ("Different", list.len().to_string()),
                ("Missing", missing.to_string()),
            ],
        ),
    }
    picked
}

fn parked(l: &mut Launcher, body: Rect, t: &AiTraffic) {
    let (left, mid, right_r) = columns(body);
    card(
        &mut l.ui,
        left,
        "Parking lists",
        Some(&t.parklists.len().to_string()),
    );
    if t.parklists.is_empty() {
        clipped(
            &mut l.ui,
            "This map has no parking lists.",
            left.x + 17.0,
            left.y + 60.0,
            left.w - 34.0,
            12.5,
            Weight::Regular,
            MUTED,
        );
        return;
    }
    let sel = keep(l.pages.diag.ai.park, t.parklists.len());
    let rows: Vec<(String, String, Status)> = t
        .parklists
        .iter()
        .map(|p| {
            let name = p.file.rsplit('/').next().unwrap_or(&p.file).to_string();
            let sub = if p.index == 0 {
                format!(
                    "{} · {}",
                    omsi_ui::tr("Main list"),
                    many(p.total(), "1 car", "%{n} cars")
                )
            } else {
                format!(
                    "{} · {}",
                    many(p.total(), "1 car", "%{n} cars"),
                    omsi_ui::tr("for spaces that name list %{i}")
                        .replace("%{i}", &p.index.to_string())
                )
            };
            (name, sub, worst(p.cars.iter().map(|c| c.exists)))
        })
        .collect();
    let list_r = Rect::new(left.x, left.y + 40.0, left.w, left.h - 96.0);
    if let Some(i) = entries(l, "ai-parks", list_r, &rows, sel) {
        l.pages.diag.ai.park = i;
        l.pages.diag.ai.car = None;
    }
    foot(
        l,
        left,
        "Each parking space of the map takes a car at random from its list.",
    );
    let p = &t.parklists[sel];
    let title = p.file.rsplit('/').next().unwrap_or(&p.file).to_string();
    let car = l.pages.diag.ai.car;
    if let Some(i) = files_panel(l, mid, right_r, &title, "ai-cars", &p.cars, car) {
        l.pages.diag.ai.car = Some(i);
    }
}

fn people(l: &mut Launcher, body: Rect, t: &AiTraffic) {
    let (left, mid, right_r) = columns(body);
    card(&mut l.ui, left, "Lists", Some("2"));
    let sel = l.pages.diag.ai.people.min(1);
    let count = |x: &[Listed]| x.iter().map(|c| c.count).sum::<usize>();
    let rows = vec![
        (
            "humans.txt".to_string(),
            if t.humans.is_empty() {
                omsi_ui::tr("None: every person installed").to_string()
            } else {
                format!(
                    "{} · {}",
                    omsi_ui::tr("People"),
                    many(count(&t.humans), "1 entry", "%{n} entries")
                )
            },
            worst(t.humans.iter().map(|c| c.exists)),
        ),
        (
            "drivers.txt".to_string(),
            if t.drivers.is_empty() {
                omsi_ui::tr("None: the game's own driver").to_string()
            } else {
                format!(
                    "{} · {}",
                    omsi_ui::tr("Drivers"),
                    many(count(&t.drivers), "1 entry", "%{n} entries")
                )
            },
            worst(t.drivers.iter().map(|c| c.exists)),
        ),
    ];
    let list_r = Rect::new(left.x, left.y + 40.0, left.w, left.h - 96.0);
    if let Some(i) = entries(l, "ai-people", list_r, &rows, sel) {
        l.pages.diag.ai.people = i;
        l.pages.diag.ai.person = None;
    }
    foot(
        l,
        left,
        "The people on foot and the passengers, and the drivers of the AI buses.",
    );
    let (title, list) = if sel == 0 {
        ("humans.txt", &t.humans)
    } else {
        ("drivers.txt", &t.drivers)
    };
    let person = l.pages.diag.ai.person;
    if let Some(i) = files_panel(l, mid, right_r, title, "ai-persons", list, person) {
        l.pages.diag.ai.person = Some(i);
    }
}

fn density(l: &mut Launcher, body: Rect, t: &AiTraffic) {
    let gap = 12.0;
    let rw = (body.w * 0.24).clamp(240.0, 330.0);
    let chart_r = Rect::new(body.x, body.y, body.w - rw - gap, body.h);
    let legend_r = Rect::new(chart_r.right() + gap, body.y, rw, body.h);
    card(&mut l.ui, chart_r, "Random traffic over the day", None);
    let seg = Rect::new(chart_r.right() - 346.0, chart_r.y + 8.0, 330.0, 30.0);
    let mut d = l.pages.diag.ai.day;
    if l.ui.segmented(
        "ai-day",
        seg,
        &mut d,
        &["Weekdays", "Saturday", "Sunday, holiday"],
    ) {
        l.pages.diag.ai.day = d;
    }
    card(
        &mut l.ui,
        legend_r,
        "Groups",
        Some("unsched_trafficdens.txt"),
    );
    if t.density.is_empty() {
        clipped(
            &mut l.ui,
            "This map has no unsched_trafficdens.txt: its random traffic follows global.cfg.",
            chart_r.x + 17.0,
            chart_r.y + 60.0,
            chart_r.w - 34.0,
            12.5,
            Weight::Regular,
            MUTED,
        );
        return;
    }
    let day = l.pages.diag.ai.day;
    // the curves, sampled every quarter of an hour
    let samples: Vec<Vec<f32>> = t
        .density
        .iter()
        .map(|g| (0..=96).map(|k| g.at(day, k as f32 * 0.25)).collect())
        .collect();
    let top = samples
        .iter()
        .flatten()
        .copied()
        .fold(0.0f32, f32::max)
        .max(0.1);
    let step = nice_step(top / 4.0);
    let top = (top / step).ceil() * step;
    let plot = Rect::new(
        chart_r.x + 56.0,
        chart_r.y + 70.0,
        chart_r.w - 80.0,
        chart_r.h - 150.0,
    );
    let ys = |v: f32| plot.bottom() - v / top * plot.h;
    let xs = |h: f32| plot.x + h / 24.0 * plot.w;
    let mut v = 0.0;
    while v <= top + step * 0.01 {
        let y = ys(v);
        l.ui.p()
            .rect(Rect::new(plot.x, y, plot.w, 1.0), LINE.alpha(0.7));
        right(
            &mut l.ui,
            &trim_num(v),
            plot.x - 10.0,
            y,
            11.0,
            Weight::Regular,
            MUTED,
        );
        v += step;
    }
    for h in (0..=24).step_by(3) {
        let x = xs(h as f32);
        l.ui.text_in_as_is(
            &format!("{h:02}:00"),
            Rect::new(x - 22.0, plot.bottom() + 6.0, 44.0, 20.0),
            11.0,
            Weight::Regular,
            MUTED,
        );
    }
    for (k, s) in samples.iter().enumerate() {
        let c = SERIES[k % SERIES.len()];
        for w in 0..s.len() - 1 {
            let a = glam::Vec2::new(xs(w as f32 * 0.25), ys(s[w]));
            let b = glam::Vec2::new(xs((w + 1) as f32 * 0.25), ys(s[w + 1]));
            l.ui.p().line(a, b, 2.0, c);
        }
    }
    // the values at the hour the pointer is on
    let m = l.ui.input.mouse;
    if plot.contains(m) {
        let hour = ((m.x - plot.x) / plot.w * 24.0).clamp(0.0, 24.0);
        let x = xs(hour);
        l.ui.p().rect(
            Rect::new(x, plot.y, 1.0, plot.h),
            omsi_ui::theme::accent().alpha(0.8),
        );
        let mut tip = clock(hour * 60.0);
        for g in &t.density {
            tip.push_str(&format!(
                "\n{}: {}",
                g.name.trim(),
                trim_num(g.at(day, hour))
            ));
        }
        l.ui.tooltip(plot, &tip);
    }
    l.ui.paragraph(
        "Each line is the group's curve for the day times its factor. The traffic options in Settings scale it again.",
        glam::Vec2::new(chart_r.x + 17.0, chart_r.bottom() - 34.0),
        chart_r.w - 34.0,
        11.5,
        Weight::Regular,
        MUTED,
    );

    // the groups and their factors
    let mut y = legend_r.y + 50.0;
    for (k, g) in t.density.iter().enumerate() {
        if y > legend_r.bottom() - 30.0 {
            break;
        }
        l.ui.p().rect(
            Rect::new(legend_r.x + 1.0, y - 4.0, legend_r.w - 2.0, 1.0),
            LINE,
        );
        l.ui.p().circle(
            glam::Vec2::new(legend_r.x + 22.0, y + 13.0),
            4.5,
            SERIES[k % SERIES.len()],
        );
        l.ui.text_in_as_is(
            g.name.trim(),
            Rect::new(legend_r.x + 34.0, y, legend_r.w - 150.0, 26.0),
            12.5,
            Weight::Regular,
            FG,
        );
        let f = omsi_ui::tr("factor %{f}").replace("%{f}", &trim_num(g.factor));
        right(
            &mut l.ui,
            &f,
            legend_r.right() - 17.0,
            y + 13.0,
            12.0,
            Weight::Regular,
            if g.curve(day).is_some() { FG } else { MUTED },
        );
        if g.curve(day).is_none() {
            l.ui.tooltip(
                Rect::new(legend_r.x, y - 4.0, legend_r.w, 30.0),
                "This group has no curve for this day: it makes no random traffic then.",
            );
        }
        y += 31.0;
    }
}

/// 1, 2 or 5 times a power of ten, at least `x`.
fn nice_step(x: f32) -> f32 {
    let p = 10f32.powf(x.max(1e-3).log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        if m * p >= x {
            return m * p;
        }
    }
    10.0 * p
}

fn trim_num(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_steps_and_numbers_read_well() {
        assert_eq!(nice_step(0.3), 0.5);
        assert_eq!(nice_step(4.0), 5.0);
        assert_eq!(nice_step(1.5), 2.0);
        assert_eq!(trim_num(1.0), "1");
        assert_eq!(trim_num(0.25), "0.25");
        assert_eq!(trim_num(0.5), "0.5");
        assert_eq!(weight_text(4.0), "4");
        assert_eq!(weight_text(0.5), "0.5");
        assert_eq!(shown_name("", "Vehicles\\Car\\a.ovh"), "a.ovh");
        assert_eq!(shown_name("Maker Sedan", "x"), "Maker Sedan");
    }
}

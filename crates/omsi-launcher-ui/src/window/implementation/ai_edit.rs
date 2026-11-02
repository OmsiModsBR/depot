//! Editing a map's own AI lists on the AI traffic tab: the car groups of `ailists.cfg` (each
//! vehicle's weight, vehicles added, taken out or moved, a new group), the parking lists
//! (how often each car is listed) and, on the bus tab (`fleet`), the buses of the depots'
//! fleet lists and the AI group each tour of the map's own `TTData` lines takes its buses
//! from. The Chrono scenarios' lists, the density and the people stay read only.
//!
//! Nothing is written until Save: then only the lines that changed are (see
//! `omsi_map::ailists_edit`), after a copy of each file goes to
//! `~/.depot/backups/<map>/<stamp>/` (see `omsi_map::backup`), from where Backups puts it
//! back. Not while the game runs.

use super::super::domain::ai::AiView;
use super::super::domain::ai_lists::*;
use super::super::domain::frame::Frame;
use super::super::interface::output::State;
use super::ai_traffic::{
    columns, details, entries, foot, keep, many, percent, shown_name, vehicle_view, weight_text,
};
use super::dialog::{dialog, dialog_foot};
use crate::kit::{card, clipped, Status};
use crate::look::*;
use omsi_integrity::traffic::VehicleFile;
use omsi_map::ailists_edit::{EditError, GroupKind};
use omsi_map::backup::{self, Backup};
use omsi_ui::paint::{Align, Color};
use omsi_ui::widgets::{ButtonKind, Key, Ui};
use omsi_ui::{Rect, Weight};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

mod fleet;
pub use fleet::bus_lines;

/// The real folder of the map `folder`.
fn map_dir(state: &State, folder: &str) -> PathBuf {
    let root = PathBuf::from(state.config.root.trim());
    omsi_cfg::resolve_path(&omsi_cfg::resolve_path(&root, "Maps"), folder)
}

/// Why the lists of the map `folder` cannot be edited now, if they cannot.
pub fn cannot_edit(state: &State, folder: &str) -> Option<&'static str> {
    if state.in_game() {
        return Some("The lists cannot be changed while the game runs.");
    }
    let dir = map_dir(state, folder);
    if !dir.is_dir() {
        return Some("This map is inside an archive: its lists cannot be changed here.");
    }
    let lists = Lists::load(&dir);
    if !backup::writable(&dir.join(&lists.ailists_file)) {
        return Some("The map's files cannot be written (read only, or inside an archive).");
    }
    // (broken UTF-16, say: written back, it would change more than the lines edited)
    if !lists.ailists.exact() || lists.parks.iter().any(|p| !p.doc.exact()) {
        return Some(
            "A list of this map is in a text encoding that cannot be written back unchanged: it is read only here.",
        );
    }
    None
}

/// Start editing the lists of the map `folder` (`name` as it is shown).
pub fn start(f: &mut Frame, view: &mut AiView, folder: &str, name: &str) {
    let dir = map_dir(f.state, folder);
    let lists = Lists::load(&dir);
    let a = &mut *view;
    let mut names = HashMap::new();
    if let Some((_, _, t)) = a.data.as_ref() {
        for v in t.groups.iter().flat_map(|g| &g.vehicles) {
            if !v.name.is_empty() {
                names.insert(v.file.to_ascii_lowercase(), v.name.clone());
            }
        }
    }
    a.edit = Some(Editor {
        folder: folder.to_string(),
        name: name.to_string(),
        map_dir: dir,
        now: lists.clone(),
        saved: lists,
        undo: Vec::new(),
        redo: Vec::new(),
        group: 0,
        vehicle: None,
        park: 0,
        car: None,
        line: 0,
        tour: 0,
        bus: None,
        dialog: None,
        vehicles: None,
        reading: None,
        exists: HashMap::new(),
        names,
    });
}

/// A button that does nothing, drawn quieter, when it is not `enabled`.
#[allow(clippy::too_many_arguments)]
fn button(
    f: &mut Frame,
    id: &str,
    r: Rect,
    label: &str,
    icon: Option<&str>,
    kind: ButtonKind,
    enabled: bool,
    tip: &str,
) -> bool {
    let kind = if enabled { kind } else { ButtonKind::Normal };
    let clicked = f.ui.button(id, r, label, icon, kind);
    if !enabled {
        f.ui.p()
            .rounded(r, omsi_ui::theme::CONTROL_RADIUS, PANEL.alpha(0.6));
    }
    f.ui.tooltip(r, tip);
    clicked && enabled
}

/// A value with a minus and a plus beside it: -1 or +1 when one was clicked.
fn stepper(ui: &mut Ui, id: &str, r: Rect, value: &str, less: &str, more: &str) -> i32 {
    let b = r.h;
    let field = Rect::new(r.x, r.y, r.w - 2.0 * b - 4.0, r.h);
    ui.p().rounded(field, 4.0, omsi_ui::theme::FIELD);
    ui.p().rounded_border(field, 4.0, 1.0, LINE);
    ui.text_in(value, field, 12.5, Weight::Medium, FG, Align::Center);
    let mut step = 0;
    for (k, (icon, tip, d)) in [("remove", less, -1), ("add", more, 1)]
        .into_iter()
        .enumerate()
    {
        let c = Rect::new(field.right() + 2.0 + k as f32 * (b + 2.0), r.y, b, b);
        let (hover, _, clicked) = ui.interact_key(omsi_ui::widgets::id_of(&format!("{id}-{k}")), c);
        ui.p()
            .rounded(c, 4.0, if hover { HOVER_LAYER } else { PANEL_2 });
        ui.icon(icon, c.center(), b * 0.6, if hover { FG } else { MUTED });
        ui.tooltip(c, tip);
        if clicked {
            step = d;
        }
    }
    step
}

/// The editing banner over the tabs: what is edited, Undo, Redo, Backups and Done.
pub fn banner(f: &mut Frame, view: &mut AiView, r: Rect) {
    let Some(mut e) = view.edit.take() else {
        return;
    };
    f.ui.p().rounded(r, 6.0, WARN.alpha(0.12));
    f.ui.p().rounded_border(r, 6.0, 1.0, WARN.alpha(0.5));
    let bw = [96.0, 96.0, 120.0, 140.0];
    let total: f32 = bw.iter().sum::<f32>() + 8.0 * 4.0;
    let text = omsi_ui::tr(
        "Editing the AI lists of %{map}. A copy of each file is kept before it is saved, and only the lines you change are written.",
    )
    .replace("%{map}", &e.name);
    clipped(
        f.ui,
        &text,
        r.x + 16.0,
        r.y + r.h * 0.5,
        r.w - total - 32.0,
        12.5,
        Weight::Regular,
        FG,
    );
    let mut x = r.right() - total;
    let y = r.y + (r.h - 30.0) * 0.5;
    let mut at = |w: f32| {
        let b = Rect::new(x, y, w, 30.0);
        x += w + 8.0;
        b
    };
    let (u, rd, bk, done) = (at(bw[0]), at(bw[1]), at(bw[2]), at(bw[3]));
    let ctrl = f.ui.input.ctrl;
    let keys_undo = ctrl && f.ui.input.raw_key == Some(winit::keyboard::KeyCode::KeyZ);
    let keys_redo = ctrl && f.ui.input.raw_key == Some(winit::keyboard::KeyCode::KeyY);
    if button(
        f,
        "ai-edit:undo",
        u,
        "Undo",
        None,
        ButtonKind::Normal,
        !e.undo.is_empty(),
        "Takes the last change back (Ctrl+Z).",
    ) || (keys_undo && e.dialog.is_none())
    {
        e.undo();
    }
    if button(
        f,
        "ai-edit:redo",
        rd,
        "Redo",
        None,
        ButtonKind::Normal,
        !e.redo.is_empty(),
        "Makes the change taken back again (Ctrl+Y).",
    ) || (keys_redo && e.dialog.is_none())
    {
        e.redo();
    }
    if button(
        f,
        "ai-edit:backups",
        bk,
        "Backups",
        Some("history"),
        ButtonKind::Normal,
        true,
        "The copies kept before each save of this map's lists, to put one back.",
    ) {
        let list = backup::store_of(&e.map_dir)
            .map(|s| backup::list(&s))
            .unwrap_or_default();
        e.dialog = Some(ListsDialog::Backups { list, sel: 0 });
    }
    if button(
        f,
        "ai-edit:done",
        done,
        "Stop editing",
        Some("close"),
        ButtonKind::Normal,
        true,
        "Back to looking at the lists. Changes not saved are asked about first.",
    ) {
        if e.dirty() {
            e.dialog = Some(ListsDialog::Leave);
        } else {
            return;
        }
    }
    view.edit = Some(e);
}

/// The foot of the middle panel: what is not saved yet, the size of `ailists.cfg`, Discard
/// and Save.
fn save_bar(f: &mut Frame, e: &mut Editor, r: Rect) {
    let y = r.bottom() - 50.0;
    f.ui.p().rect(Rect::new(r.x + 1.0, y, r.w - 2.0, 1.0), LINE);
    let n = changes(&e.saved, &e.now).len();
    let (text, c) = if n == 0 {
        (omsi_ui::tr("No changes").to_string(), MUTED)
    } else {
        (
            many(n, "1 change not saved", "%{n} changes not saved"),
            WARN,
        )
    };
    let size = e.now.ailists.to_bytes().len() as u64;
    let max = omsi_integrity::AILISTS_RECOMMENDED_MAX;
    let size_text = omsi_ui::tr("ailists.cfg: %{s} KB").replace("%{s}", &kb(size));
    let over = size > max;
    let bw = 110.0;
    let room = r.w - 2.0 * bw - 52.0;
    clipped(
        f.ui,
        &text,
        r.x + 17.0,
        y + 17.0,
        room,
        12.5,
        Weight::Medium,
        c,
    );
    clipped(
        f.ui,
        &size_text,
        r.x + 17.0,
        y + 35.0,
        room,
        11.5,
        Weight::Regular,
        if over { WARN } else { MUTED },
    );
    f.ui.tooltip(
        Rect::new(r.x, y, room, 50.0),
        if over {
            "Larger than 16 KB: not recommended except on high-performance computers. Big AI lists slow the game down when it loads and fills the streets."
        } else {
            "The size ailists.cfg will have. Up to 16 KB is recommended; more only on high-performance computers."
        },
    );
    let save = Rect::new(r.right() - 16.0 - bw, y + 9.0, bw, 32.0);
    let discard = Rect::new(save.x - 8.0 - bw, y + 9.0, bw, 32.0);
    let in_game = f.state.in_game();
    if button(
        f,
        "ai-edit:discard",
        discard,
        "Discard",
        None,
        ButtonKind::Normal,
        n > 0,
        "Forgets every change not saved yet.",
    ) {
        let before = e.now.clone();
        e.now = e.saved.clone();
        e.undo.push(before);
        e.redo.clear();
    }
    if button(
        f,
        "ai-edit:save",
        save,
        "Save",
        Some("save"),
        ButtonKind::Primary,
        n > 0 && !in_game,
        if in_game {
            "The lists cannot be changed while the game runs."
        } else {
            "Shows the changes and the files they go into, then saves them."
        },
    ) {
        e.dialog = Some(ListsDialog::Save);
    }
}

fn kb(bytes: u64) -> String {
    format!("{:.1}", bytes as f32 / 1024.0)
}

/// The car groups, editable.
pub fn car_groups(f: &mut Frame, view: &mut AiView, body: Rect) {
    let Some(mut e) = view.edit.take() else {
        return;
    };
    let root = PathBuf::from(f.state.config.root.trim());
    let (left, mid, right_r) = columns(body);
    let n = e.now.ailists.groups().len();
    card(f.ui, left, "Groups", Some(&n.to_string()));
    e.group = keep(e.group, n);
    let all = e.now.ailists.groups().to_vec();
    let mut rows: Vec<(String, String, Status)> = Vec::new();
    for g in &all {
        let sub = match g.kind {
            GroupKind::Depot => omsi_ui::tr("Depot · read only").to_string(),
            _ => many(g.vehicles.len(), "1 vehicle", "%{n} vehicles"),
        };
        let ok = g.vehicles.iter().all(|v| e.exists(&root, &v.file));
        let st = if ok { Status::Found } else { Status::Error };
        rows.push((g.name.trim().to_string(), sub, st));
    }
    let list_r = Rect::new(left.x, left.y + 40.0, left.w, left.h - 96.0);
    if let Some(i) = entries(f, "ai-edit-groups", list_r, &rows, e.group) {
        e.group = i;
        e.vehicle = None;
    }
    // new group, remove group
    let half = (left.w - 42.0) * 0.5;
    let ng = Rect::new(left.x + 17.0, left.bottom() - 42.0, half, 32.0);
    let rg = Rect::new(ng.right() + 8.0, ng.y, half, 32.0);
    f.ui.p().rect(
        Rect::new(left.x + 1.0, left.bottom() - 52.0, left.w - 2.0, 1.0),
        LINE,
    );
    if button(
        f,
        "ai-edit:new-group",
        ng,
        "New group",
        Some("add"),
        ButtonKind::Normal,
        true,
        "Adds an empty group of cars at the end of ailists.cfg.",
    ) {
        e.dialog = Some(ListsDialog::NewGroup {
            name: String::new(),
            error: false,
        });
    }
    let can_remove = e
        .now
        .ailists
        .groups()
        .get(e.group)
        .is_some_and(|g| g.kind != GroupKind::Depot && g.vehicles.is_empty());
    if button(
        f,
        "ai-edit:remove-group",
        rg,
        "Remove",
        Some("delete"),
        ButtonKind::Normal,
        can_remove,
        "Takes the chosen group out of ailists.cfg. Only an empty group can be taken out.",
    ) {
        let g = e.group;
        if let Err(err) = e.act(|x| x.ailists.remove_group(g)) {
            f.state.set_status(error_text(&err), true);
        } else {
            e.group = e.group.saturating_sub(1);
        }
    }

    let Some(g) = e.now.ailists.groups().get(e.group).cloned() else {
        card(f.ui, mid, "Vehicles", None);
        clipped(
            f.ui,
            "This map has no car groups yet. Add one with New group.",
            mid.x + 17.0,
            mid.y + 60.0,
            mid.w - 34.0,
            12.5,
            Weight::Regular,
            MUTED,
        );
        save_bar(f, &mut e, mid);
        details(f, right_r, &[]);
        view.edit = Some(e);
        return;
    };
    let total: f32 = g.vehicles.iter().map(|v| v.weight.max(0.0)).sum();
    let note = if g.kind.has_weights() {
        format!(
            "{} · {}",
            many(g.vehicles.len(), "1 vehicle", "%{n} vehicles"),
            omsi_ui::tr("weights add up to %{w}").replace("%{w}", &weight_text(total))
        )
    } else {
        many(g.vehicles.len(), "1 vehicle", "%{n} vehicles")
    };
    // the note goes under the title: the buttons take the header's right side
    card(f.ui, mid, g.name.trim(), None);
    clipped(
        f.ui,
        &note,
        mid.x + 17.0,
        mid.y + 46.0,
        mid.w - 34.0,
        12.0,
        Weight::Regular,
        MUTED,
    );
    if g.kind == GroupKind::Depot {
        clipped(
            f.ui,
            "A depot's buses are in its depot file and fleet lists: they are read only here.",
            mid.x + 17.0,
            mid.y + 60.0,
            mid.w - 34.0,
            12.5,
            Weight::Regular,
            MUTED,
        );
        save_bar(f, &mut e, mid);
        details(
            f,
            right_r,
            &[
                ("Group", g.name.trim().to_string()),
                ("Kind", omsi_ui::tr("Depot").to_string()),
            ],
        );
        view.edit = Some(e);
        return;
    }
    // add, move
    let aw = 140.0;
    let add_r = Rect::new(mid.right() - 16.0 - 2.0 * aw - 8.0, mid.y + 8.0, aw, 30.0);
    let move_r = Rect::new(add_r.right() + 8.0, add_r.y, aw, 30.0);
    if button(
        f,
        "ai-edit:add",
        add_r,
        "Add vehicle",
        Some("add"),
        ButtonKind::Normal,
        true,
        "Picks a car, truck or bus of the installation or of a mod and adds it to this group.",
    ) {
        e.dialog = Some(ListsDialog::AddVehicle {
            group: e.group,
            search: String::new(),
            kind: 0,
            sel: None,
            weight: 1.0,
        });
        if e.vehicles.is_none() && e.reading.is_none() {
            let (tx, rx) = channel();
            let root = root.clone();
            std::thread::spawn(move || {
                let _ = tx.send(omsi_integrity::traffic::vehicle_files(&root));
            });
            e.reading = Some(rx);
        }
    }
    let vsel = e.vehicle.filter(|&v| v < g.vehicles.len());
    if button(
        f,
        "ai-edit:move",
        move_r,
        "Move to group",
        Some("open_with"),
        ButtonKind::Normal,
        vsel.is_some(),
        "Moves the chosen vehicle, with its weight, to another group.",
    ) {
        e.dialog = Some(ListsDialog::MoveTo { sel: None });
    }

    // the vehicles: name and file, weight, chance, take out
    let table_r = Rect::new(mid.x, mid.y + 66.0, mid.w, mid.h - 66.0 - 56.0);
    let heads = [("Vehicle", 0.0), ("Weight", 0.52), ("Chance", 0.7)];
    for (h, at) in heads {
        f.ui.text_in(
            h,
            Rect::new(
                table_r.x + 28.0 + (table_r.w - 36.0) * at,
                table_r.y,
                120.0,
                26.0,
            ),
            12.0,
            Weight::Regular,
            MUTED,
            Align::Left,
        );
    }
    let list = Rect::new(table_r.x, table_r.y + 28.0, table_r.w, table_r.h - 28.0);
    let exists: Vec<bool> = g
        .vehicles
        .iter()
        .map(|v| e.exists(&root, &v.file))
        .collect();
    let mut picked = None;
    let mut step: Option<(usize, i32)> = None;
    let mut drop: Option<usize> = None;
    let weights = g.kind.has_weights();
    let names = &e.names;
    f.ui.scroll_area("ai-edit-vehicles", list, &mut |ui, v| {
        let mut y = v.y;
        let w = v.w - 8.0;
        for (i, veh) in g.vehicles.iter().enumerate() {
            let row = Rect::new(v.x + 4.0, y, w, 44.0);
            let (hover, _, clicked) =
                ui.interact_key(omsi_ui::widgets::id_of(&format!("ai-edit-v-{i}")), row);
            if vsel == Some(i) {
                ui.p().rounded(row, 4.0, SELECTED_LAYER);
                ui.p().rect(
                    Rect::new(row.x, row.y + 6.0, 3.0, row.h - 12.0),
                    omsi_ui::theme::accent(),
                );
            } else if hover {
                ui.p().rounded(row, 4.0, HOVER_LAYER);
            }
            if !exists[i] {
                crate::kit::dot(ui, v.x + 16.0, y + 22.0, Status::Error);
            }
            let x0 = v.x + 28.0;
            let cols = w - 36.0;
            let name_w = cols * 0.5 - 8.0;
            let dim = veh.weight <= 0.0;
            let name = names
                .get(&veh.file.to_ascii_lowercase())
                .map_or("", String::as_str);
            ui.text_in_as_is(
                &shown_name(name, &veh.file),
                Rect::new(x0, y + 3.0, name_w, 22.0),
                12.5,
                Weight::Medium,
                if dim { MUTED } else { FG },
            );
            ui.text_in_as_is(
                &veh.file,
                Rect::new(x0, y + 21.0, name_w, 20.0),
                11.0,
                Weight::Regular,
                MUTED,
            );
            let sx = x0 + cols * 0.52;
            let s = Rect::new(sx, y + 9.0, (cols * 0.17).clamp(90.0, 120.0), 26.0);
            if weights {
                let d = stepper(
                    ui,
                    &format!("ai-edit-w-{i}"),
                    s,
                    &weight_text(veh.weight),
                    "Lower the weight by one.",
                    "Raise the weight by one.",
                );
                if d != 0 {
                    step = Some((i, d));
                }
            } else {
                ui.text_in("1", s, 12.5, Weight::Regular, MUTED, Align::Left);
            }
            // chance: a bar and the share
            let cx = x0 + cols * 0.7;
            let share = if total > 0.0 {
                veh.weight.max(0.0) / total
            } else {
                0.0
            };
            let bar = Rect::new(cx, y + 20.0, (cols * 0.14).max(30.0), 4.0);
            ui.p().rounded(bar, 2.0, PANEL_2);
            ui.p().rounded(
                Rect::new(bar.x, bar.y, bar.w * share, bar.h),
                2.0,
                omsi_ui::theme::accent(),
            );
            ui.text_in(
                &percent(share),
                Rect::new(bar.right() + 6.0, y, cols * 0.12, 44.0),
                12.0,
                Weight::Regular,
                MUTED,
                Align::Left,
            );
            if ui.icon_button(
                &format!("ai-edit-x-{i}"),
                glam::Vec2::new(row.right() - 18.0, y + 22.0),
                13.0,
                "delete",
                "Takes this vehicle out of the group.",
            ) {
                drop = Some(i);
            } else if clicked {
                picked = Some(i);
            }
            y += 46.0;
        }
        y - v.y
    });
    if let Some(i) = picked {
        e.vehicle = Some(i);
    }
    let gi = e.group;
    if let Some((i, d)) = step {
        // (a weight of 0.5 goes to 1.5: only the step changes it)
        let w = (g.vehicles[i].weight + d as f32).max(0.0);
        let _ = e.act(|x| x.ailists.set_weight(gi, i, w));
        e.vehicle = Some(i);
    }
    if let Some(i) = drop {
        let _ = e.act(|x| x.ailists.remove_vehicle(gi, i));
        e.vehicle = None;
    }
    save_bar(f, &mut e, mid);

    // the chosen vehicle (Discard may have taken the group away: a new one, not saved)
    let Some(g) = e.now.ailists.groups().get(gi).cloned() else {
        e.group = 0;
        e.vehicle = None;
        details(f, right_r, &[]);
        view.edit = Some(e);
        return;
    };
    let total: f32 = g.vehicles.iter().map(|v| v.weight.max(0.0)).sum();
    match e.vehicle.and_then(|v| g.vehicles.get(v).map(|x| (v, x))) {
        Some((k, v)) => {
            let ex = exists.get(k).copied().unwrap_or(false);
            let share = if total > 0.0 {
                v.weight.max(0.0) / total
            } else {
                0.0
            };
            let file = v.file.clone();
            let before = e
                .saved
                .ailists
                .find(&g.name)
                .and_then(|s| {
                    e.saved.ailists.groups()[s]
                        .vehicles
                        .iter()
                        .find(|x| x.file.eq_ignore_ascii_case(&file))
                })
                .map(|x| x.weight);
            let weight = match before {
                Some(b) if b != v.weight => format!(
                    "{} → {} ({})",
                    weight_text(b),
                    weight_text(v.weight),
                    percent(share)
                ),
                _ => format!("{} ({})", weight_text(v.weight), percent(share)),
            };
            let name = e
                .names
                .get(&file.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default();
            view.edit = Some(e);
            let rest = vehicle_view(f, view, right_r, &file, "", ex);
            details(
                f,
                rest,
                &[
                    ("Vehicle", shown_name(&name, &file)),
                    ("File", file.clone()),
                    ("Weight", weight),
                    (
                        "State",
                        omsi_ui::tr(if ex { "Installed" } else { "Missing" }).to_string(),
                    ),
                ],
            );
            return;
        }
        None => {
            details(
                f,
                right_r,
                &[
                    ("Group", g.name.trim().to_string()),
                    ("Vehicles", g.vehicles.len().to_string()),
                    ("Weights", weight_text(total)),
                ],
            );
        }
    }
    view.edit = Some(e);
}

/// The parking lists, editable.
pub fn parked(f: &mut Frame, view: &mut AiView, body: Rect) {
    let Some(mut e) = view.edit.take() else {
        return;
    };
    let root = PathBuf::from(f.state.config.root.trim());
    let (left, mid, right_r) = columns(body);
    card(
        f.ui,
        left,
        "Parking lists",
        Some(&e.now.parks.len().to_string()),
    );
    if e.now.parks.is_empty() {
        clipped(
            f.ui,
            "This map has no parking lists.",
            left.x + 17.0,
            left.y + 60.0,
            left.w - 34.0,
            12.5,
            Weight::Regular,
            MUTED,
        );
        save_bar(f, &mut e, mid);
        view.edit = Some(e);
        return;
    }
    e.park = keep(e.park, e.now.parks.len());
    let rows: Vec<(String, String, Status)> = e
        .now
        .parks
        .iter()
        .map(|p| {
            let n: usize = p.doc.cars().iter().map(|c| c.1).sum();
            (p.file.clone(), many(n, "1 car", "%{n} cars"), Status::Found)
        })
        .collect();
    let list_r = Rect::new(left.x, left.y + 40.0, left.w, left.h - 96.0);
    if let Some(i) = entries(f, "ai-edit-parks", list_r, &rows, e.park) {
        e.park = i;
        e.car = None;
    }
    foot(
        f,
        left,
        "Each parking space of the map takes a car at random from its list.",
    );

    let pi = e.park;
    let cars = e.now.parks[pi].doc.cars();
    let total: usize = cars.iter().map(|c| c.1).sum();
    let title = e.now.parks[pi].file.clone();
    card(f.ui, mid, &title, None);
    clipped(
        f.ui,
        &many(total, "1 entry", "%{n} entries"),
        mid.x + 17.0,
        mid.y + 46.0,
        mid.w - 34.0,
        12.0,
        Weight::Regular,
        MUTED,
    );
    let add_r = Rect::new(mid.right() - 16.0 - 140.0, mid.y + 8.0, 140.0, 30.0);
    if button(
        f,
        "ai-edit:add-car",
        add_r,
        "Add car",
        Some("add"),
        ButtonKind::Normal,
        true,
        "Picks a scenery object (.sco) of the installation to park on this list's spaces.",
    ) {
        let title = omsi_ui::tr("Pick a parked car (.sco)").to_string();
        if let Some(p) = omsi_launcher_lib::pick_file(&title) {
            match content_name(&root, &p) {
                Some(name) if name.to_ascii_lowercase().ends_with(".sco") => {
                    if let Err(err) = e.act(|x| x.parks[pi].doc.add(&name)) {
                        f.state.set_status(error_text(&err), true);
                    }
                }
                Some(_) => f
                    .state
                    .set_status("A parked car is a scenery object (.sco).", true),
                None => f.state.set_status(
                    "The file must be inside the OMSI 2 folder or a mod's folder.",
                    true,
                ),
            }
        }
    }
    let table_r = Rect::new(mid.x, mid.y + 66.0, mid.w, mid.h - 66.0 - 56.0);
    for (h, at) in [("Name", 0.0), ("Listed", 0.52), ("Chance", 0.7)] {
        f.ui.text_in(
            h,
            Rect::new(
                table_r.x + 28.0 + (table_r.w - 36.0) * at,
                table_r.y,
                120.0,
                26.0,
            ),
            12.0,
            Weight::Regular,
            MUTED,
            Align::Left,
        );
    }
    let list = Rect::new(table_r.x, table_r.y + 28.0, table_r.w, table_r.h - 28.0);
    let exists: Vec<bool> = cars.iter().map(|(f, _)| e.exists(&root, f)).collect();
    let csel = e.car.filter(|&c| c < cars.len());
    let mut picked = None;
    let mut step: Option<(usize, i32)> = None;
    let mut drop: Option<usize> = None;
    f.ui.scroll_area("ai-edit-cars", list, &mut |ui, v| {
        let mut y = v.y;
        let w = v.w - 8.0;
        for (i, (file, n)) in cars.iter().enumerate() {
            let row = Rect::new(v.x + 4.0, y, w, 44.0);
            let (hover, _, clicked) =
                ui.interact_key(omsi_ui::widgets::id_of(&format!("ai-edit-c-{i}")), row);
            if csel == Some(i) {
                ui.p().rounded(row, 4.0, SELECTED_LAYER);
                ui.p().rect(
                    Rect::new(row.x, row.y + 6.0, 3.0, row.h - 12.0),
                    omsi_ui::theme::accent(),
                );
            } else if hover {
                ui.p().rounded(row, 4.0, HOVER_LAYER);
            }
            if !exists[i] {
                crate::kit::dot(ui, v.x + 16.0, y + 22.0, Status::Error);
            }
            let x0 = v.x + 28.0;
            let cols = w - 36.0;
            let name_w = cols * 0.5 - 8.0;
            ui.text_in_as_is(
                &shown_name("", file),
                Rect::new(x0, y + 3.0, name_w, 22.0),
                12.5,
                Weight::Medium,
                FG,
            );
            ui.text_in_as_is(
                file,
                Rect::new(x0, y + 21.0, name_w, 20.0),
                11.0,
                Weight::Regular,
                MUTED,
            );
            let s = Rect::new(
                x0 + cols * 0.52,
                y + 9.0,
                (cols * 0.17).clamp(90.0, 120.0),
                26.0,
            );
            let d = stepper(
                ui,
                &format!("ai-edit-n-{i}"),
                s,
                &format!("{n}×"),
                "Lists this car once less.",
                "Lists this car once more: it is parked more often.",
            );
            if d != 0 {
                step = Some((i, d));
            }
            let share = *n as f32 / total.max(1) as f32;
            ui.text_in(
                &percent(share),
                Rect::new(x0 + cols * 0.7, y, cols * 0.2, 44.0),
                12.0,
                Weight::Regular,
                MUTED,
                Align::Left,
            );
            if ui.icon_button(
                &format!("ai-edit-cx-{i}"),
                glam::Vec2::new(row.right() - 18.0, y + 22.0),
                13.0,
                "delete",
                "Takes this car out of the list.",
            ) {
                drop = Some(i);
            } else if clicked {
                picked = Some(i);
            }
            y += 46.0;
        }
        y - v.y
    });
    if let Some(i) = picked {
        e.car = Some(i);
    }
    if let Some((i, d)) = step {
        let (f, n) = cars[i].clone();
        let to = (n as i32 + d).max(0) as usize;
        let _ = e.act(|x| x.parks[pi].doc.set_count(&f, to));
        e.car = (to > 0).then_some(i);
    }
    if let Some(i) = drop {
        let f = cars[i].0.clone();
        let _ = e.act(|x| x.parks[pi].doc.set_count(&f, 0));
        e.car = None;
    }
    save_bar(f, &mut e, mid);
    let cars = e.now.parks[pi].doc.cars();
    let total: usize = cars.iter().map(|c| c.1).sum();
    match e.car.and_then(|c| cars.get(c).map(|x| (c, x.clone()))) {
        Some((k, (file, n))) => {
            let ex = exists.get(k).copied().unwrap_or(false);
            details(
                f,
                right_r,
                &[
                    ("Name", shown_name("", &file)),
                    ("File", file.clone()),
                    (
                        "In the list",
                        format!(
                            "{} ({})",
                            many(n, "Once", "%{n} times"),
                            percent(n as f32 / total.max(1) as f32)
                        ),
                    ),
                    (
                        "State",
                        omsi_ui::tr(if ex { "Installed" } else { "Missing" }).to_string(),
                    ),
                ],
            );
        }
        None => details(
            f,
            right_r,
            &[
                ("Entries", total.to_string()),
                ("Different", cars.len().to_string()),
            ],
        ),
    }
    view.edit = Some(e);
}

/// `p` as a list names it: from the OMSI 2 folder or the content root it is in, with `\`.
fn content_name(root: &Path, p: &Path) -> Option<String> {
    let roots: Vec<PathBuf> = std::iter::once(root.to_path_buf())
        .chain(omsi_cfg::content_roots())
        .collect();
    let canon = |x: &Path| x.canonicalize().unwrap_or_else(|_| x.to_path_buf());
    let p = canon(p);
    roots
        .iter()
        .filter_map(|r| p.strip_prefix(canon(r)).ok())
        .min_by_key(|rest| rest.as_os_str().len())
        .map(|rest| rest.to_string_lossy().replace('/', "\\"))
}

fn error_text(e: &EditError) -> &'static str {
    match e {
        EditError::Exists => "There is a group of that name already.",
        EditError::NoName => "A group needs a name.",
        EditError::Default => {
            "The list's header names this group, or one after it, as the default group: it stays."
        }
        EditError::HoldsFleet => "A depot's fleet lists follow this group: it stays.",
        EditError::Depot => "A depot's group is read only here.",
        EditError::Unwritable => {
            "The list's text encoding cannot hold this name: rename the folder or file to plain letters."
        }
        EditError::NoNumber => "A fleet number is needed.",
        EditError::BadField => "A number, plate or repaint cannot hold a tab or a line break.",
        EditError::BadDate => "A day is a date as YYYY-MM-DD.",
        _ => "This change cannot be made to the list.",
    }
}

/// A change as the save window lists it: (where, what, colour).
pub fn change_row(c: &Change) -> (String, String, Color) {
    let name = |f: &str| shown_name("", f);
    match c {
        Change::Weight {
            group,
            file,
            from,
            to,
        } => (
            group.clone(),
            omsi_ui::tr("%{v}: weight %{a} → %{b}")
                .replace("%{v}", &name(file))
                .replace("%{a}", &weight_text(*from))
                .replace("%{b}", &weight_text(*to)),
            FG,
        ),
        Change::Added {
            group,
            file,
            weight,
        } => (
            group.clone(),
            match weight {
                Some(w) => omsi_ui::tr("Added %{v}, weight %{w}")
                    .replace("%{v}", &name(file))
                    .replace("%{w}", &weight_text(*w)),
                None => omsi_ui::tr("Added %{v}").replace("%{v}", &name(file)),
            },
            OK,
        ),
        Change::Removed { group, file } => (
            group.clone(),
            omsi_ui::tr("Removed %{v}").replace("%{v}", &name(file)),
            omsi_ui::theme::DANGER,
        ),
        Change::GroupAdded(g) => (g.clone(), omsi_ui::tr("New group").to_string(), OK),
        Change::GroupRemoved(g) => (
            g.clone(),
            omsi_ui::tr("Group removed").to_string(),
            omsi_ui::theme::DANGER,
        ),
        Change::BusAdded { group, what } => (
            group.clone(),
            omsi_ui::tr("Added bus %{b}").replace("%{b}", what),
            OK,
        ),
        Change::BusRemoved { group, what } => (
            group.clone(),
            omsi_ui::tr("Removed bus %{b}").replace("%{b}", what),
            omsi_ui::theme::DANGER,
        ),
        Change::BusChanged { group, what } => (
            group.clone(),
            omsi_ui::tr("Changed bus %{b}").replace("%{b}", what),
            FG,
        ),
        Change::TourGroup {
            line,
            tour,
            from,
            to,
        } => (
            omsi_ui::tr("Line %{l}, tour %{t}")
                .replace("%{l}", line)
                .replace("%{t}", tour),
            omsi_ui::tr("AI group %{a} → %{b}")
                .replace("%{a}", from)
                .replace("%{b}", to),
            FG,
        ),
        Change::Parked {
            list,
            file,
            from,
            to,
        } => (
            list.clone(),
            omsi_ui::tr("%{v}: listed %{a} → %{b} times")
                .replace("%{v}", &name(file))
                .replace("%{a}", &from.to_string())
                .replace("%{b}", &to.to_string()),
            if to > from { OK } else { FG },
        ),
    }
}

fn stamp_now() -> String {
    match omsi_launcher_lib::local_now() {
        Some((y, m, d, h, min)) => backup::stamp(y, m, d, h, min),
        None => "backup".into(),
    }
}

/// The window open over the tab, if any.
pub fn ai_traffic_dialog(f: &mut Frame, view: &mut AiView) {
    let Some(mut e) = view.edit.take() else {
        return;
    };
    if let Some(rx) = e.reading.as_ref() {
        if let Ok(v) = rx.try_recv() {
            for f in &v {
                if !f.name.is_empty() {
                    e.names
                        .entry(f.file.to_ascii_lowercase())
                        .or_insert_with(|| f.name.clone());
                }
            }
            e.vehicles = Some(v);
            e.reading = None;
        }
    }
    let Some(d) = e.dialog.take() else {
        view.edit = Some(e);
        return;
    };
    let escape = f.ui.input.keys.contains(&Key::Escape);
    let keep_open = if escape {
        None
    } else {
        match d {
            ListsDialog::AddVehicle {
                group,
                search,
                kind,
                sel,
                weight,
            } => add_vehicle(f, &mut e, group, search, kind, sel, weight),
            ListsDialog::MoveTo { sel } => move_to(f, &mut e, sel),
            ListsDialog::NewGroup { name, error } => new_group(f, &mut e, name, error),
            ListsDialog::Bus(form) => fleet::bus_dialog(f, &mut e, form),
            ListsDialog::TourGroup { file, tour, sel } => {
                fleet::tour_group(f, &mut e, file, tour, sel)
            }
            ListsDialog::Save => save_dialog(f, view, &mut e),
            ListsDialog::Backups { list, sel } => backups(f, view, &mut e, list, sel),
            ListsDialog::Leave => {
                if leave(f, &mut e) {
                    // (the editing ends)
                    return;
                }
                None
            }
        }
    };
    if e.dialog.is_none() {
        e.dialog = keep_open;
    }
    view.edit = Some(e);
}

fn add_vehicle(
    f: &mut Frame,
    e: &mut Editor,
    group: usize,
    mut search: String,
    mut kind: usize,
    mut sel: Option<usize>,
    mut weight: f32,
) -> Option<ListsDialog> {
    let gname = e
        .now
        .ailists
        .groups()
        .get(group)
        .map(|g| g.name.trim().to_string())
        .unwrap_or_default();
    let title = omsi_ui::tr("Add a vehicle to %{g}").replace("%{g}", &gname);
    let inside = dialog(
        f.ui,
        860.0,
        620.0,
        &title,
        "AI vehicles of Vehicles and of your mods",
    );
    let (buttons, body) = dialog_foot(f.ui, inside);
    let sw = body.w - 360.0;
    f.ui.text_input(
        "ai-edit-search",
        Rect::new(body.x, body.y, sw, 34.0),
        &mut search,
        "Search by name or folder…",
        Some("search"),
    );
    let seg = Rect::new(body.x + sw + 12.0, body.y, body.w - sw - 12.0, 34.0);
    if f.ui.segmented(
        "ai-edit-kind",
        seg,
        &mut kind,
        &["Cars and trucks", "Buses", "All"],
    ) {
        sel = None;
    }
    f.ui.tooltip(
        seg,
        "Cars and trucks are .ovh files, buses .bus files; a group may hold both.",
    );
    let list_r = Rect::new(body.x, body.y + 46.0, body.w, body.h - 46.0);
    let shown: Vec<VehicleFile> = match e.vehicles.as_ref() {
        Some(all) => {
            let needle = search.trim().to_lowercase();
            all.iter()
                .filter(|v| match kind {
                    0 => !v.bus,
                    1 => v.bus,
                    _ => true,
                })
                .filter(|v| {
                    needle.is_empty()
                        || v.name.to_lowercase().contains(&needle)
                        || v.file.to_lowercase().contains(&needle)
                })
                .cloned()
                .collect()
        }
        None => {
            clipped(
                f.ui,
                "Looking for the vehicles…",
                list_r.x,
                list_r.y + 16.0,
                list_r.w,
                12.5,
                Weight::Regular,
                MUTED,
            );
            Vec::new()
        }
    };
    if e.vehicles.as_ref().is_some_and(|_| shown.is_empty()) {
        clipped(
            f.ui,
            "No vehicle matches.",
            list_r.x,
            list_r.y + 16.0,
            list_r.w,
            12.5,
            Weight::Regular,
            MUTED,
        );
    }
    let rows: Vec<(String, String, Status)> = shown
        .iter()
        .map(|v| (shown_name(&v.name, &v.file), v.file.clone(), Status::Found))
        .collect();
    let s = sel.filter(|&i| i < rows.len()).unwrap_or(usize::MAX);
    if let Some(i) = entries(f, "ai-edit-pick", list_r, &rows, s) {
        sel = Some(i);
    }

    // weight, cancel, add
    let weights = e
        .now
        .ailists
        .groups()
        .get(group)
        .is_some_and(|g| g.kind.has_weights());
    if weights {
        f.ui.text_in(
            "Weight",
            Rect::new(buttons.x, buttons.y, 70.0, buttons.h),
            13.0,
            Weight::Regular,
            FG,
            Align::Left,
        );
        let d = stepper(
            f.ui,
            "ai-edit-new-w",
            Rect::new(buttons.x + 70.0, buttons.y + 3.0, 110.0, 28.0),
            &weight_text(weight),
            "Lower the weight by one.",
            "Raise the weight by one.",
        );
        weight = (weight + d as f32).max(0.0);
    }
    let add_r = Rect::new(buttons.right() - 180.0, buttons.y, 180.0, buttons.h);
    let cancel_r = Rect::new(add_r.x - 118.0, buttons.y, 110.0, buttons.h);
    if button(
        f,
        "ai-edit:pick-cancel",
        cancel_r,
        "Cancel",
        None,
        ButtonKind::Normal,
        true,
        "Closes this window without adding anything.",
    ) {
        return None;
    }
    let chosen = sel.and_then(|i| shown.get(i)).map(|v| v.file.clone());
    if button(
        f,
        "ai-edit:pick-add",
        add_r,
        "Add to the group",
        Some("add"),
        ButtonKind::Primary,
        chosen.is_some(),
        "Adds the chosen vehicle at the end of the group.",
    ) {
        if let Some(file) = chosen {
            match e.act(|x| x.ailists.add_vehicle(group, &file, weight)) {
                Ok(()) => {
                    e.group = group;
                    e.vehicle = e
                        .now
                        .ailists
                        .groups()
                        .get(group)
                        .map(|g| g.vehicles.len().saturating_sub(1));
                    return None;
                }
                Err(err) => f.state.set_status(error_text(&err), true),
            }
        }
    }
    Some(ListsDialog::AddVehicle {
        group,
        search,
        kind,
        sel,
        weight,
    })
}

fn move_to(f: &mut Frame, e: &mut Editor, mut sel: Option<usize>) -> Option<ListsDialog> {
    let (g, v) = (e.group, e.vehicle?);
    let file = e.now.ailists.groups().get(g)?.vehicles.get(v)?.file.clone();
    let title = omsi_ui::tr("Move %{v} to").replace("%{v}", &shown_name("", &file));
    let inside = dialog(f.ui, 520.0, 520.0, &title, "");
    let (buttons, body) = dialog_foot(f.ui, inside);
    let targets: Vec<usize> = e
        .now
        .ailists
        .groups()
        .iter()
        .enumerate()
        .filter(|(i, x)| *i != g && x.kind != GroupKind::Depot)
        .map(|(i, _)| i)
        .collect();
    let rows: Vec<(String, String, Status)> = targets
        .iter()
        .map(|&i| {
            let x = &e.now.ailists.groups()[i];
            (
                x.name.trim().to_string(),
                many(x.vehicles.len(), "1 vehicle", "%{n} vehicles"),
                Status::Found,
            )
        })
        .collect();
    if rows.is_empty() {
        clipped(
            f.ui,
            "There is no other car group. Add one with New group.",
            body.x,
            body.y + 16.0,
            body.w,
            12.5,
            Weight::Regular,
            MUTED,
        );
    }
    let s = sel.filter(|&i| i < rows.len()).unwrap_or(usize::MAX);
    if let Some(i) = entries(f, "ai-edit-move", body, &rows, s) {
        sel = Some(i);
    }
    let ok_r = Rect::new(buttons.right() - 140.0, buttons.y, 140.0, buttons.h);
    let cancel_r = Rect::new(ok_r.x - 118.0, buttons.y, 110.0, buttons.h);
    if button(
        f,
        "ai-edit:move-cancel",
        cancel_r,
        "Cancel",
        None,
        ButtonKind::Normal,
        true,
        "Closes this window without moving anything.",
    ) {
        return None;
    }
    let to = sel.and_then(|i| targets.get(i).copied());
    if button(
        f,
        "ai-edit:move-ok",
        ok_r,
        "Move",
        Some("open_with"),
        ButtonKind::Primary,
        to.is_some(),
        "Moves the vehicle, with its weight, to the end of the chosen group.",
    ) {
        if let Some(to) = to {
            match e.act(|x| x.ailists.move_vehicle(g, v, to)) {
                Ok(()) => {
                    e.group = to;
                    e.vehicle = e
                        .now
                        .ailists
                        .groups()
                        .get(to)
                        .map(|x| x.vehicles.len().saturating_sub(1));
                    return None;
                }
                Err(err) => f.state.set_status(error_text(&err), true),
            }
        }
    }
    Some(ListsDialog::MoveTo { sel })
}

fn new_group(
    f: &mut Frame,
    e: &mut Editor,
    mut name: String,
    mut error: bool,
) -> Option<ListsDialog> {
    let inside = dialog(f.ui, 520.0, 250.0, "New group", "ailists.cfg");
    let (buttons, body) = dialog_foot(f.ui, inside);
    if f.ui.text_input(
        "ai-edit-group-name",
        Rect::new(body.x, body.y, body.w, 34.0),
        &mut name,
        "The group's name, as the timetable and the density name it",
        None,
    ) {
        error = false;
    }
    let note = if error {
        "There is a group of that name already."
    } else {
        "The new group is empty; add vehicles to it next. The random traffic uses it once unsched_vehgroups.txt names it."
    };
    f.ui.paragraph(
        note,
        glam::Vec2::new(body.x, body.y + 46.0),
        body.w,
        12.0,
        Weight::Regular,
        if error { omsi_ui::theme::DANGER } else { MUTED },
    );
    let ok_r = Rect::new(buttons.right() - 140.0, buttons.y, 140.0, buttons.h);
    let cancel_r = Rect::new(ok_r.x - 118.0, buttons.y, 110.0, buttons.h);
    if button(
        f,
        "ai-edit:group-cancel",
        cancel_r,
        "Cancel",
        None,
        ButtonKind::Normal,
        true,
        "Closes this window without adding a group.",
    ) {
        return None;
    }
    let enter = f.ui.input.keys.contains(&Key::Enter);
    if button(
        f,
        "ai-edit:group-ok",
        ok_r,
        "Add group",
        Some("add"),
        ButtonKind::Primary,
        !name.trim().is_empty(),
        "Adds the group at the end of ailists.cfg.",
    ) || (enter && !name.trim().is_empty())
    {
        let n = name.clone();
        match e.act(|x| x.ailists.add_group(&n).map(|_| ())) {
            Ok(()) => {
                e.group = e.now.ailists.groups().len().saturating_sub(1);
                e.vehicle = None;
                return None;
            }
            Err(EditError::Exists) => error = true,
            Err(err) => f.state.set_status(error_text(&err), true),
        }
    }
    Some(ListsDialog::NewGroup { name, error })
}

fn save_dialog(f: &mut Frame, view: &mut AiView, e: &mut Editor) -> Option<ListsDialog> {
    let list = changes(&e.saved, &e.now);
    let files = e.now.to_write(&e.saved);
    let store = backup::store_of(&e.map_dir);
    let inside = dialog(f.ui, 860.0, 600.0, "Save the changes?", &e.name);
    let (buttons, body) = dialog_foot(f.ui, inside);
    // the changes
    let head = Rect::new(body.x, body.y, body.w, 26.0);
    f.ui.text_in("Where", head, 12.0, Weight::Regular, MUTED, Align::Left);
    f.ui.text_in(
        "Change",
        Rect::new(body.x + 180.0, body.y, body.w - 180.0, 26.0),
        12.0,
        Weight::Regular,
        MUTED,
        Align::Left,
    );
    let size = e.now.ailists.to_bytes().len() as u64;
    let over = size > omsi_integrity::AILISTS_RECOMMENDED_MAX;
    let tail = 150.0 + if over { 40.0 } else { 0.0 };
    let rows_r = Rect::new(
        body.x,
        body.y + 28.0,
        body.w,
        (body.h - 28.0 - tail).max(60.0),
    );
    let rows: Vec<(String, String, Color)> = list.iter().map(change_row).collect();
    f.ui.scroll_area("ai-edit-changes", rows_r, &mut |ui, v| {
        let mut y = v.y;
        for (place, what, c) in &rows {
            ui.p().rect(Rect::new(v.x, y, v.w, 1.0), LINE);
            ui.text_in_as_is(
                place,
                Rect::new(v.x, y, 170.0, 30.0),
                12.5,
                Weight::Regular,
                FG,
            );
            ui.text_in_as_is(
                what,
                Rect::new(v.x + 180.0, y, v.w - 180.0, 30.0),
                12.5,
                Weight::Regular,
                *c,
            );
            y += 30.0;
        }
        y - v.y
    });
    // what is written, where the copy goes
    let mut y = rows_r.bottom() + 10.0;
    let written: Vec<String> = files
        .iter()
        .map(|(f, _)| format!("{}\\{f}", e.folder))
        .collect();
    // the copy's folder from the home folder on, as ~
    let home = omsi_cfg::paths::data_dir().and_then(|d| d.parent().map(Path::to_path_buf));
    let copy = store
        .as_ref()
        .map(|s| {
            let p = s.join(stamp_now());
            match home.as_ref().and_then(|h| p.strip_prefix(h).ok()) {
                Some(r) => Path::new("~").join(r).display().to_string(),
                None => p.display().to_string(),
            }
        })
        .unwrap_or_default();
    for (k, v) in [("Writes", written.join(", ")), ("Copy kept in", copy)] {
        f.ui.p().rect(Rect::new(body.x, y, body.w, 1.0), LINE);
        f.ui.text_in(
            k,
            Rect::new(body.x, y, 170.0, 34.0),
            12.5,
            Weight::Regular,
            MUTED,
            Align::Left,
        );
        f.ui.text_in_as_is(
            &v,
            Rect::new(body.x + 180.0, y, body.w - 180.0, 34.0),
            12.5,
            Weight::Regular,
            FG,
        );
        f.ui.tooltip(Rect::new(body.x, y, body.w, 34.0), &v);
        y += 36.0;
    }
    if over {
        let t = omsi_ui::tr("ailists.cfg will be %{s} KB: larger than 16 KB is not recommended except on high-performance computers.")
            .replace("%{s}", &kb(size));
        f.ui.paragraph(
            &t,
            glam::Vec2::new(body.x, y + 6.0),
            body.w,
            12.5,
            Weight::Medium,
            WARN,
        );
        y += 40.0;
    }
    f.ui.paragraph(
        "Comments, order, blank lines and the file's text encoding stay as they are. Put a copy back at any time with Backups. The game must not be running.",
        glam::Vec2::new(body.x, y + 6.0),
        body.w,
        12.0,
        Weight::Regular,
        MUTED,
    );
    let ok_r = Rect::new(buttons.right() - 120.0, buttons.y, 120.0, buttons.h);
    let back_r = Rect::new(ok_r.x - 118.0, buttons.y, 110.0, buttons.h);
    if button(
        f,
        "ai-edit:save-back",
        back_r,
        "Back",
        None,
        ButtonKind::Normal,
        true,
        "Back to the lists, nothing saved.",
    ) {
        return None;
    }
    let in_game = f.state.in_game();
    if button(
        f,
        "ai-edit:save-ok",
        ok_r,
        "Save",
        Some("save"),
        ButtonKind::Primary,
        !in_game && store.is_some() && !files.is_empty(),
        if in_game {
            "The lists cannot be changed while the game runs."
        } else {
            "Keeps a copy of each file, then writes the changed lines."
        },
    ) {
        let store = store?;
        match backup::write_with_backup(&e.map_dir, &store, &stamp_now(), &files) {
            Ok(b) => {
                log::info!(
                    "AI lists of {} saved ({} files), copy in {}",
                    e.map_dir.display(),
                    files.len(),
                    b.dir.display()
                );
                e.saved = e.now.clone();
                e.exists.clear();
                // the tab reads the map again
                view.data = None;
                f.state.set_status("The AI lists were saved.", false);
            }
            Err(err) => {
                log::warn!("saving the AI lists of {}: {err}", e.map_dir.display());
                f.state.set_status(
                    "The AI lists could not be saved: nothing was changed.",
                    true,
                );
            }
        }
        return None;
    }
    Some(ListsDialog::Save)
}

fn backups(
    f: &mut Frame,
    view: &mut AiView,
    e: &mut Editor,
    list: Vec<Backup>,
    mut sel: usize,
) -> Option<ListsDialog> {
    let inside = dialog(f.ui, 760.0, 560.0, "Backups", &e.name);
    let (buttons, body) = dialog_foot(f.ui, inside);
    if list.is_empty() {
        clipped(
            f.ui,
            "No copy kept yet: one is made at each save.",
            body.x,
            body.y + 16.0,
            body.w,
            12.5,
            Weight::Regular,
            MUTED,
        );
    }
    let rows: Vec<(String, String, Status)> = list
        .iter()
        .map(|b| {
            let files: Vec<String> = b
                .files
                .iter()
                .map(|f| {
                    if f.existed {
                        f.file.clone()
                    } else {
                        format!("{} ({})", f.file, omsi_ui::tr("new"))
                    }
                })
                .collect();
            (b.stamp.replace('_', " "), files.join(", "), Status::Found)
        })
        .collect();
    sel = keep(sel, rows.len());
    let list_r = Rect::new(body.x, body.y, body.w, body.h - 40.0);
    if let Some(i) = entries(f, "ai-edit-backups", list_r, &rows, sel) {
        sel = i;
    }
    f.ui.paragraph(
        "Putting a copy back writes its files over the map's and takes out the files that save made; what is there now is copied first.",
        glam::Vec2::new(body.x, body.bottom() - 32.0),
        body.w,
        11.5,
        Weight::Regular,
        MUTED,
    );
    let ok_r = Rect::new(buttons.right() - 160.0, buttons.y, 160.0, buttons.h);
    let close_r = Rect::new(ok_r.x - 118.0, buttons.y, 110.0, buttons.h);
    if button(
        f,
        "ai-edit:backups-close",
        close_r,
        "Close",
        None,
        ButtonKind::Normal,
        true,
        "Closes this window.",
    ) {
        return None;
    }
    let in_game = f.state.in_game();
    let dirty = e.dirty();
    if button(
        f,
        "ai-edit:restore",
        ok_r,
        "Put it back",
        Some("restart_alt"),
        ButtonKind::Primary,
        !list.is_empty() && !in_game && !dirty,
        if dirty {
            "Save or discard your changes first."
        } else if in_game {
            "The lists cannot be changed while the game runs."
        } else {
            "Writes the chosen copy back over the map's lists."
        },
    ) {
        if let (Some(b), Some(store)) = (list.get(sel), backup::store_of(&e.map_dir)) {
            match backup::restore(&e.map_dir, &store, &stamp_now(), b) {
                Ok(_) => {
                    let lists = Lists::load(&e.map_dir);
                    e.saved = lists.clone();
                    e.now = lists;
                    e.undo.clear();
                    e.redo.clear();
                    e.exists.clear();
                    view.data = None;
                    f.state.set_status("The copy was put back.", false);
                }
                Err(err) => {
                    log::warn!("restoring {}: {err}", b.dir.display());
                    f.state.set_status("The copy could not be put back.", true);
                }
            }
        }
        return None;
    }
    Some(ListsDialog::Backups { list, sel })
}

/// Leave with changes not saved: true when the editing ends.
fn leave(f: &mut Frame, e: &mut Editor) -> bool {
    let inside = dialog(f.ui, 520.0, 220.0, "Leave without saving?", &e.name);
    let (buttons, body) = dialog_foot(f.ui, inside);
    let n = changes(&e.saved, &e.now).len();
    f.ui.paragraph(
        &many(
            n,
            "1 change is not saved. It is lost when you stop editing now.",
            "%{n} changes are not saved. They are lost when you stop editing now.",
        ),
        glam::Vec2::new(body.x, body.y + 4.0),
        body.w,
        12.5,
        Weight::Regular,
        FG,
    );
    let ok_r = Rect::new(buttons.right() - 170.0, buttons.y, 170.0, buttons.h);
    let back_r = Rect::new(ok_r.x - 118.0, buttons.y, 110.0, buttons.h);
    if button(
        f,
        "ai-edit:leave-back",
        back_r,
        "Back",
        None,
        ButtonKind::Normal,
        true,
        "Back to the lists, to save them.",
    ) {
        e.dialog = None;
        return false;
    }
    if button(
        f,
        "ai-edit:leave-ok",
        ok_r,
        "Discard changes",
        None,
        ButtonKind::Danger,
        true,
        "Stops editing; the changes not saved are lost.",
    ) {
        return true;
    }
    e.dialog = Some(ListsDialog::Leave);
    false
}

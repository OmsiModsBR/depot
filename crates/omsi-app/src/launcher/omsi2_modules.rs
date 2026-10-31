//! Settings > Modules in OMSI 2 mode: a card for each optional OmsiLaunch module Depot offers
//! (#484 Omsi Prefetch, #485 openHUD). The card installs and removes the module through the
//! same confirmed steps as OmsiLaunch itself (`original_engine::modules`, the dialog over the
//! page) and keeps the player's choices in `~/.depot/omsi2/modules.json`; they are sent to
//! OmsiLaunch at every start (`original_engine::launch::module_args`), only while the module
//! is in the OMSI 2 folder.

use super::theme::*;
use super::ui::{ButtonKind, Ui};
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_launcher_lib::original_engine::{config, modules};
use omsi_launcher_ui::kit::{self, Status};
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// What a module's card asks of the launcher (it needs the launcher's state and dialog).
#[derive(Default)]
pub struct Asks {
    /// The module's install ("Download and install…").
    pub look: Option<&'static str>,
    /// The module's removal ("Remove…").
    pub remove: Option<&'static str>,
    /// (in) Whether an install may start now: no game runs and no other install is open.
    pub idle: bool,
}

/// What is known of a module in the OMSI 2 folder, read again every few seconds.
#[derive(Clone)]
struct Found {
    /// The version its manifest names, when it is there.
    version: Option<String>,
    /// Depot placed it (its record), so Depot may remove it.
    ours: bool,
    /// The folder of what it learned exists.
    data: bool,
}

thread_local! {
    /// The player's module choices (`modules.json`), read the first time a card is drawn.
    static CHOICES: RefCell<Option<Map<String, Value>>> = const { RefCell::new(None) };
    /// What was found per module: for which OMSI 2 folder, and when.
    static FOUND: RefCell<Vec<(&'static str, PathBuf, Instant, Found)>> = const { RefCell::new(Vec::new()) };
}

fn found(root: &Path, id: &'static str) -> Found {
    FOUND.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((_, _, _, f)) = c
            .iter()
            .find(|(i, at, when, _)| *i == id && at == root && when.elapsed().as_secs_f32() < 2.0)
        {
            return f.clone();
        }
        let data = core::data_dir();
        let f = Found {
            version: modules::present_version(root, id),
            ours: modules::load_record(&data, root, id).is_some(),
            data: modules::data_folder(root, id).is_dir(),
        };
        c.retain(|(i, _, _, _)| *i != id);
        c.push((id, root.to_path_buf(), Instant::now(), f.clone()));
        f
    })
}

/// The choices of module `id` as the card shows them: switched on unless the player turned
/// it off, and each setting at the module's default until one is picked.
fn choice(id: &str) -> (bool, Map<String, Value>) {
    CHOICES.with(|c| {
        let mut c = c.borrow_mut();
        let all = c.get_or_insert_with(|| config::load_modules_in(&core::data_dir()));
        let m = all.get(id);
        let on = m
            .and_then(|m| m.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or_else(|| modules::on_by_default(id));
        let settings = m
            .and_then(|m| m.get("settings"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        (on, settings)
    })
}

/// Keeps the choices of module `id` (only values its card offers are written).
fn keep(id: &str, on: bool, settings: Map<String, Value>) {
    CHOICES.with(|c| {
        let mut c = c.borrow_mut();
        let all = c.get_or_insert_with(|| config::load_modules_in(&core::data_dir()));
        let settings: Map<String, Value> = settings
            .into_iter()
            .filter(|(k, v)| {
                v.as_str()
                    .is_some_and(|t| modules::setting_allowed(id, k, t))
            })
            .collect();
        all.insert(id.to_string(), json!({"enabled": on, "settings": settings}));
        if let Err(e) = config::save_modules_in(&core::data_dir(), all) {
            log::warn!("modules.json: {e:#}");
        }
    });
}

const ROW: f32 = 50.0;

/// The value of `key` the card shows: the saved one when the card offers it, else the default.
fn value_of(settings: &Map<String, Value>, s: &modules::CardSetting) -> String {
    settings
        .get(s.key)
        .and_then(|v| match v {
            Value::String(t) => Some(t.clone()),
            Value::Bool(b) => Some(b.to_string()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .and_then(|t| s.values.iter().find(|v| v.eq_ignore_ascii_case(&t)))
        .unwrap_or(&s.default)
        .to_string()
}

/// A row's name and the line under it.
fn row_text(ui: &mut Ui, r: Rect, room: f32, label: &str, hint: &str) {
    ui.p().rect(Rect::new(r.x, r.y, r.w, 1.0), EDGE);
    let x = r.x + 16.0;
    let label = kit::fit_text(ui, label, 13.5, room);
    ui.text_in(
        &label,
        Rect::new(x, r.y + 6.0, room, 20.0),
        13.5,
        Weight::Regular,
        TEXT,
        Align::Left,
    );
    let hint = kit::fit_text(ui, hint, 11.5, room);
    ui.text_in(
        &hint,
        Rect::new(x, r.y + 26.0, room, 16.0),
        11.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
}

/// A button that may be unavailable: then drawn faint, with why in its tooltip.
fn button(
    ui: &mut Ui,
    id: &str,
    r: Rect,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
    tip: &str,
) -> bool {
    let pressed = if enabled {
        ui.button(id, r, label, None, kind)
    } else {
        ui.p().rounded(r, CONTROL_RADIUS, FIELD);
        ui.text_in(label, r, 13.0, Weight::Medium, TEXT_FAINT, Align::Center);
        false
    };
    ui.tooltip(r, tip);
    pressed && enabled
}

/// The card of module `id`. Returns the height it needed.
pub fn card(ui: &mut Ui, r: Rect, id: &'static str, root: &Path, asks: &mut Asks) -> f32 {
    let Some(def) = modules::module_def(id) else {
        return 0.0;
    };
    ui.panel(r);
    kit::heading(
        ui,
        Rect::new(r.x + 16.0, r.y + 6.0, r.w - 32.0, 30.0),
        def.name,
    );
    let mut y = r.y + 44.0;
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    let f = found(root, id);
    let (mut on, mut settings) = choice(id);
    let mut changed = false;
    // the switch: what the module does, its licence
    let (what, does) = match id {
        "omsi-prefetch" => (
            "Preload maps and vehicles into memory",
            "Faster loading. A separate program under its own license (%{license}).",
        ),
        "openhud" => (
            "Live radar and a new pause menu",
            "Replaces the pause menu of OMSI 2 only. A separate program under its own license (%{license}).",
        ),
        _ => ("", ""),
    };
    let tw = 60.0;
    let trow = Rect::new(r.x, y, r.w, ROW);
    row_text(
        ui,
        trow,
        w - tw - 12.0,
        what,
        &omsi_ui::tr(does).replace("%{license}", def.license),
    );
    let sw = Rect::new(r.right() - 16.0 - tw, y + (ROW - 32.0) * 0.5, tw, 32.0);
    if ui.toggle(&format!("mod-{id}-on"), sw, &mut on, "") {
        changed = true;
    }
    ui.tooltip(
        sw,
        "On: OMSI 2 starts with this module. Off: it is kept out of the session, though it stays installed.",
    );
    y += ROW;
    // its settings: a dropdown each, or a switch for a yes or no
    for s in modules::card_settings(id) {
        let (label, hint, names): (&str, &str, &[&str]) = match s.key {
            "ram_budget_mb" => (
                "Memory it may use",
                "Auto keeps 4 GB for OMSI 2 and uses 70% of the rest",
                &[
                    "Auto (70% of free memory)",
                    "2 GB",
                    "4 GB",
                    "6 GB",
                    "8 GB",
                    "12 GB",
                ],
            ),
            "user_vehicles" => (
                "Your vehicles",
                "Vehicles the map's traffic does not use, kept in memory too",
                &["None", "All of them"],
            ),
            "monitor" => (
                "Monitor window",
                "A window that shows what is kept in memory while you play",
                &[],
            ),
            "radar" => (
                "Radar",
                "A round map of the surroundings over the game view",
                &[],
            ),
            "radar_corner" => (
                "Radar corner",
                "Where the radar sits on the screen",
                &["Bottom left", "Bottom right", "Top left", "Top right"],
            ),
            "radar_zoom" => (
                "Radar zoom",
                "How much of the surroundings the radar shows",
                &["Near", "Medium", "Far"],
            ),
            "radar_size_percent" => (
                "Radar size",
                "In percent of its normal size",
                &["50%", "75%", "100%", "150%", "200%"],
            ),
            "radar_opacity_percent" => (
                "Radar opacity",
                "How solid the radar is drawn",
                &["40%", "60%", "80%", "90%", "100%"],
            ),
            "pause_menu" => (
                "New pause menu",
                "openHUD's pause menu in place of OMSI 2's own",
                &[],
            ),
            "units" => (
                "Units",
                "For speed and distance on the radar",
                &["Metric", "Imperial"],
            ),
            "ui_scale_percent" => (
                "Interface size",
                "Size of openHUD's text and panels",
                &["75%", "100%", "125%", "150%", "200%"],
            ),
            // (the languages openHUD has, named in their own words; Auto is English for now)
            "language" => (
                "Language",
                "openHUD's own texts; Auto is English for now",
                &["Auto", "English", "Português (Brasil)", "Deutsch"],
            ),
            _ => continue,
        };
        let cw = if names.is_empty() {
            60.0
        } else {
            (r.w * 0.42).clamp(140.0, 210.0)
        };
        let rr = Rect::new(r.x, y, r.w, ROW);
        row_text(ui, rr, w - cw - 12.0, label, hint);
        let ctrl = Rect::new(r.right() - 16.0 - cw, y + (ROW - 32.0) * 0.5, cw, 32.0);
        let cur = value_of(&settings, s);
        if names.is_empty() {
            let mut v = cur == "true";
            if ui.toggle(&format!("mod-{id}-{}", s.key), ctrl, &mut v, "") {
                settings.insert(s.key.into(), json!(v.to_string()));
                changed = true;
            }
            ui.tooltip(ctrl, hint);
        } else {
            let mut sel = s.values.iter().position(|v| *v == cur).unwrap_or(0);
            // (the sizes are not words: only the others are translated)
            let labels: Vec<String> = names.iter().map(|n| omsi_ui::tr(n).into_owned()).collect();
            if ui.select_as_is(&format!("mod-{id}-{}", s.key), ctrl, &mut sel, &labels) {
                settings.insert(s.key.into(), json!(s.values[sel]));
                changed = true;
            }
            ui.tooltip(ctrl, hint);
        }
        y += ROW;
    }
    if changed {
        keep(id, on, settings);
    }
    // where it is: installed (by Depot or by hand) or not, and what can be done
    ui.p().rect(Rect::new(r.x, y, r.w, 1.0), EDGE);
    let (status, text) = match (&f.version, f.ours) {
        (Some(v), true) => (
            Status::Found,
            omsi_ui::tr("Installed by Depot · version %{version}").replace("%{version}", v),
        ),
        (Some(v), false) => (
            Status::Warning,
            omsi_ui::tr("Installed, not by Depot · version %{version}").replace("%{version}", v),
        ),
        (None, _) => (
            Status::Unverified,
            omsi_ui::tr("Not installed").into_owned(),
        ),
    };
    let sr = Rect::new(x, y + 8.0, w, 34.0);
    kit::dot(ui, sr.x + 4.0, sr.center().y, status);
    let text = kit::fit_text(ui, &text, 13.0, w - 18.0);
    ui.text_in(
        &text,
        Rect::new(sr.x + 18.0, sr.y, w - 18.0, sr.h),
        13.0,
        Weight::Regular,
        TEXT,
        Align::Left,
    );
    y += 50.0;
    let bh = 34.0;
    let busy_tip = "Not while a game runs or another install is open.";
    if f.version.is_none() {
        let br = Rect::new(x, y, w, bh);
        if button(
            ui,
            &format!("mod-{id}-install"),
            br,
            "Download and install…",
            ButtonKind::Primary,
            asks.idle,
            if asks.idle {
                "Looks up the module's official release. Nothing is downloaded or changed until you confirm."
            } else {
                busy_tip
            },
        ) {
            asks.look = Some(id);
        }
    } else {
        let half = (w - 10.0) * 0.5;
        let open = Rect::new(x, y, half, bh);
        if button(
            ui,
            &format!("mod-{id}-data"),
            open,
            "Open data folder",
            ButtonKind::Normal,
            f.data,
            if f.data {
                "Opens the folder where the module keeps what it learned about each map. The module manages it."
            } else {
                "The module makes this folder at its first session."
            },
        ) {
            core::open_folder(&modules::data_folder(root, id));
        }
        let rm = Rect::new(x + half + 10.0, y, half, bh);
        let can = f.ours && asks.idle;
        let tip = if !f.ours {
            "Depot did not install this module, so it does not remove it. Remove its folder by hand."
        } else if !asks.idle {
            busy_tip
        } else {
            "Removes the files Depot installed for this module, after a confirmation."
        };
        if button(
            ui,
            &format!("mod-{id}-remove"),
            rm,
            "Remove…",
            ButtonKind::Danger,
            can,
            tip,
        ) {
            asks.remove = Some(id);
        }
    }
    y += bh + 10.0;
    y += ui.paragraph(
        "Your choices are sent to OMSI 2 at every start while the module is installed.",
        Vec2::new(x, y),
        w,
        12.0,
        Weight::Regular,
        TEXT_DIM,
    ) + 14.0;
    y - r.y
}

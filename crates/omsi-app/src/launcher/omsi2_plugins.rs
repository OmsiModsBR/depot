//! Settings > Plug-ins in OMSI 2 mode: the OMSI 2 folder's plug-ins, each switched on or off
//! (`original_engine::plugins`: off renames its `.opl` to `.opl.disabled`, on renames it
//! back). OmsiLaunch's own plug-in links Depot to OMSI 2 and is shown locked on.

use super::theme::*;
use super::ui::Ui;
use glam::Vec2;
use omsi_launcher_lib::original_engine::plugins::{self, Plugin};
use omsi_launcher_ui::kit;
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Instant;

thread_local! {
    /// The plug-ins found: for which OMSI 2 folder, and when (read again every few seconds,
    /// and at once after a switch).
    static FOUND: RefCell<Option<(PathBuf, Instant, Vec<Plugin>)>> = const { RefCell::new(None) };
    /// Why the last switch did not happen.
    static FAILED: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn found(root: &Path) -> Vec<Plugin> {
    FOUND.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref()
            .is_none_or(|(at, when, _)| at != root || when.elapsed().as_secs_f32() > 3.0)
        {
            *c = Some((root.to_path_buf(), Instant::now(), plugins::list(root)));
        }
        c.as_ref().map(|x| x.2.clone()).unwrap_or_default()
    })
}

const ROW: f32 = 44.0;

/// The card. `game_runs`: no plug-in is switched while a game runs. Returns its height.
pub fn card(ui: &mut Ui, r: Rect, root: &Path, game_runs: bool) -> f32 {
    let list = found(root);
    ui.panel(r);
    kit::heading(
        ui,
        Rect::new(r.x + 16.0, r.y + 6.0, r.w - 80.0, 30.0),
        "OMSI 2 plug-ins",
    );
    ui.text_in(
        &list.len().to_string(),
        Rect::new(r.right() - 56.0, r.y + 8.0, 40.0, 24.0),
        12.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Right,
    );
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    let mut y = r.y + 44.0;
    y += ui.paragraph(
        "OMSI 2 loads every .opl file of its plugins folder. Turning a plug-in off renames its .opl file to .opl.disabled, and turning it on renames it back: nothing is deleted, and the plug-in's other files are not touched.",
        Vec2::new(x, y),
        w,
        12.0,
        Weight::Regular,
        TEXT_DIM,
    ) + 10.0;
    if list.is_empty() {
        ui.text_in(
            "The OMSI 2 folder has no plug-ins.",
            Rect::new(x, y, w, 24.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        y += 30.0;
    }
    let mut flip: Option<(String, bool)> = None;
    for (k, p) in list.iter().enumerate() {
        ui.p().rect(Rect::new(r.x, y, r.w, 1.0), EDGE);
        let cw = 60.0;
        let room = w - cw - 12.0;
        let name = kit::fit_text(ui, &p.name, 13.5, room);
        ui.text_in(
            &name,
            Rect::new(x, y + 4.0, room, 20.0),
            13.5,
            Weight::Regular,
            if p.on { TEXT } else { TEXT_DIM },
            Align::Left,
        );
        // the line under the name: why it cannot be switched, when it cannot
        let (note, nc) = if p.locked {
            (omsi_ui::tr("Always on").into_owned(), TEXT_DIM)
        } else if p.twin {
            (
                omsi_ui::tr(
                    "Both %{name}.opl and %{name}.opl.disabled exist. Keep one of them by hand.",
                )
                .replace("%{name}", &p.name),
                WARN,
            )
        } else {
            (String::new(), TEXT_DIM)
        };
        if !note.is_empty() {
            let note = kit::fit_text(ui, &note, 11.5, room);
            ui.text_in(
                &note,
                Rect::new(x, y + 24.0, room, 16.0),
                11.5,
                Weight::Regular,
                nc,
                Align::Left,
            );
        }
        let ctrl = Rect::new(r.right() - 16.0 - cw, y + (ROW - 32.0) * 0.5, cw, 32.0);
        if p.locked {
            ui.icon(
                "lock",
                Vec2::new(ctrl.right() - 20.0, ctrl.center().y),
                16.0,
                TEXT_DIM,
            );
            ui.tooltip(
                ctrl,
                "OmsiLaunch is the link between Depot and OMSI 2, so it is always on.",
            );
        } else if p.twin || game_runs {
            // (shown as it is, not switchable)
            let v = p.on;
            let track = Rect::new(ctrl.right() - 40.0, ctrl.center().y - 10.0, 40.0, 20.0);
            ui.p()
                .rounded(track, 10.0, if v { accent().alpha(0.4) } else { FIELD });
            ui.p().rounded_border(track, 10.0, 1.0, EDGE);
            let knob = if v {
                track.right() - 10.0
            } else {
                track.x + 10.0
            };
            ui.p()
                .circle(Vec2::new(knob, track.center().y), 5.0, TEXT_FAINT);
            ui.tooltip(
                ctrl,
                if game_runs {
                    "Not while a game runs."
                } else {
                    "Both files exist: keep one of them in the plugins folder first."
                },
            );
        } else {
            let mut v = p.on;
            if ui.toggle(&format!("omsi2-plugin-{k}"), ctrl, &mut v, "") {
                flip = Some((p.name.clone(), v));
            }
            ui.tooltip(
                ctrl,
                "On: OMSI 2 loads this plug-in. Off: its .opl file is renamed to .opl.disabled and OMSI 2 skips it.",
            );
        }
        y += ROW;
    }
    if let Some((name, on)) = flip {
        let r = plugins::set_enabled(root, &name, on);
        match &r {
            Ok(()) => log::info!(
                "OMSI 2 plug-in {name} switched {}",
                if on { "on" } else { "off" }
            ),
            Err(e) => log::warn!("OMSI 2 plug-in {name}: {e:#}"),
        }
        FAILED.with(|f| {
            *f.borrow_mut() = r.err().map(|e| {
                omsi_ui::tr("The plug-in could not be switched: %{error}")
                    .replace("%{error}", &format!("{e:#}"))
            })
        });
        FOUND.with(|c| *c.borrow_mut() = None);
    }
    if let Some(e) = FAILED.with(|f| f.borrow().clone()) {
        y += 6.0;
        y += ui.paragraph(&e, Vec2::new(x, y), w, 12.0, Weight::Regular, DANGER);
    }
    y + 14.0 - r.y
}

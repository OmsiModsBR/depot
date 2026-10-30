//! The route number and destination as the lists set them, and the route arrows over the road.

use super::chooser::LIST_DIRTY;
use super::duty::natural;
use super::select::sync_live;
use super::store::remember_setting;
use super::values::{toggle_now, toggle_set};
use crate::core::app::App;

/// The route numbers to choose from: the lines of the depot file's routes (their own
/// line, or the route code without its last two digits) and of the map's timetable.
pub(in crate::hud) fn route_numbers(app: &App) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(hof) = app.player.as_ref().and_then(|p| p.vehicle.host.hof.clone()) {
        for t in &hof.info_trips {
            let code = t.code.trim();
            let l = if !t.line.trim().is_empty() {
                t.line.trim().to_string()
            } else if code.len() > 2 && code.chars().all(|c| c.is_ascii_digit()) {
                code[..code.len() - 2].trim_start_matches('0').to_string()
            } else {
                String::new()
            };
            // A route number is display text, not necessarily an alphanumeric IBIS code:
            // Brazilian matrices use values such as "-10". Keep the HOF spelling intact.
            let l = l.trim().to_string();
            if !l.is_empty() && !out.contains(&l) {
                out.push(l);
            }
        }
    }
    if let Some(sch) = app.schedule.as_ref() {
        for l in &sch.data.lines {
            let n = l.name.trim().to_string();
            if !n.is_empty() && !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out.sort_by(|a, b| natural(a, b));
    out
}

/// Whether a line can be represented by the numeric IBIS line/suffix variables openOMSI
/// already knows how to encode. Everything else must stay as display text: turning "-10"
/// or "EXP" into a number loses information.
pub fn numeric_ibis_line(line: &str) -> bool {
    let line = line.trim();
    if line.is_empty() {
        return false;
    }
    if line.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    let mut chars = line.chars();
    if let Some(first) = chars.next() {
        let rest: String = chars.collect();
        if matches!(
            first.to_ascii_uppercase(),
            'E' | 'S' | 'A' | 'D' | 'C' | 'B' | 'U' | 'M' | 'N' | 'X'
        ) && !rest.is_empty()
            && rest.chars().all(|c| c.is_ascii_digit())
        {
            return true;
        }
    }
    let digits: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
    let suffix = &line[digits.len()..];
    !digits.is_empty()
        && suffix.chars().count() == 1
        && matches!(
            suffix.chars().next().unwrap().to_ascii_uppercase(),
            'E' | 'U' | 'N' | 'S' | 'M'
        )
}

/// The route number on the bus's IBIS and display, as picked or typed in the destination
/// list (the destination stays: the one on the display now, else the first -
/// `schedule::shown_destination`).
pub(crate) fn set_route_by_hand(app: &mut App, line: &str) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    if let Some(p) = app.player.as_mut() {
        if numeric_ibis_line(line) {
            let hof = p.vehicle.host.hof.clone();
            if let Some(hof) = hof.as_deref() {
                if let Some(ti) =
                    crate::schedule::shown_destination(&p.vehicle, hof, p.blind_pick.as_ref())
                {
                    p.set_destination_by_hand(hof, line, ti);
                }
            }
        } else {
            // OMSI's route-number field is also used as arbitrary display text. Do not
            // force symbols/unknown letters through IBIS_LinieKurs: that would turn "-10"
            // into line 0 (and matrix scripts would then blank or rewrite the rear sign).
            for name in ["SetLineTo", "Matrix_Nr", "Linie"] {
                if let Some(i) = p.vehicle.ty.program.str_var(name) {
                    p.vehicle.state.str_vars[i as usize] = line.to_string();
                }
            }
        }
        log::info!(
            "route number set by hand: {line} (IBIS_LinieKurs {:?}, Matrix_Nr {:?})",
            p.vehicle.var("IBIS_LinieKurs"),
            p.vehicle.str_var("Matrix_Nr")
        );
        app.service_msg = Some((
            omsi_ui::tr("Route %{line}").replace("%{line}", &line.to_string()),
            3.0,
        ));
    }
}

/// The route number the bus shows: its matrix's (`Matrix_Nr`), else the one it was set to
/// (`SetLineTo`), else its IBIS's line number.
pub(crate) fn line_shown(v: &omsi_sim::VehicleInstance) -> Option<String> {
    [v.str_var("Matrix_Nr"), v.str_var("SetLineTo")]
        .into_iter()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
        .or_else(|| ibis_line_number(v))
}

pub(in crate::hud) fn ibis_line_number(v: &omsi_sim::VehicleInstance) -> Option<String> {
    v.var("IBIS_LinieKurs")
        .filter(|l| *l > 0.0)
        .map(|l| format!("{}", l as i64))
}

/// The line a destination picked from the list is set with: the route number the bus
/// shows, when the IBIS can take it - its matrix's (`Matrix_Nr`), a roller blind's
/// (`SetLineTo`, which its crank sets) - else the IBIS's own number. Taken from
/// `IBIS_LinieKurs`, the IBIS's number without its letter, a pick made 92E into 92 on the
/// IBIS and the matrix. Not `SetLineTo` on any other bus: no script of its own writes it,
/// only an earlier pick, so a line typed on the IBIS since went back to that pick's.
pub fn destination_line(v: &omsi_sim::VehicleInstance) -> String {
    let blind = crate::schedule::has_roller_blind(v).then(|| v.str_var("SetLineTo"));
    [Some(v.str_var("Matrix_Nr")), blind]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
        .filter(|l| numeric_ibis_line(l))
        .or_else(|| ibis_line_number(v))
        .unwrap_or_default()
}

/// Whether the route arrows over the road are on now.
pub(crate) fn route_arrows_on(app: &App) -> bool {
    toggle_now(app, "nav_arrows").unwrap_or(false)
}

/// The route arrows over the road (the Alt bar's help marker button, as the Esc menu's switch):
/// flips them, keeps the choice for the next game and tells the world. The new state.
pub(crate) fn toggle_route_arrows(app: &mut App) -> bool {
    let on = !toggle_now(app, "nav_arrows").unwrap_or(false);
    if let Some((k, v)) = toggle_set(app, "nav_arrows", on) {
        remember_setting(k, &v);
    }
    sync_live(app);
    LIST_DIRTY.store(true, std::sync::atomic::Ordering::Relaxed);
    on
}

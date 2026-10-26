//! The Integrity tab of the Diagnostics page: on request, a complete check of one map or one
//! vehicle (`omsi-integrity`) - what is missing or unreadable, and what works but is wasteful
//! for OMSI 2 and DirectX 9. Nothing on it changes a file.

use super::Launcher;
use omsi_integrity::{Report, Severity};
use omsi_launcher_ui::kit::{action, clipped, dot, text, Status};
use omsi_launcher_ui::look::*;
use omsi_ui::{Rect, Weight};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

#[derive(Default)]
pub struct DiagView {
    /// The page's tab: 0 the overview, 1 the integrity check.
    pub tab: usize,
    /// What is checked: 0 a map, 1 a vehicle.
    kind: usize,
    item: usize,
    /// The map or vehicle compared with (0: none; k: the list's k - 1th).
    compare: usize,
    run: Option<Run>,
    done: Option<Done>,
}

/// A check running in the background. Cancelling lets it run out (a file read cannot be
/// stopped half way) but drops what it finds.
struct Run {
    rx: Receiver<Report>,
    cancel: Arc<AtomicBool>,
}

struct Done {
    name: String,
    report: Report,
    compared: bool,
}

/// One line of the result.
enum Row {
    Head(Severity, usize),
    Item {
        severity: Severity,
        file: String,
        message: String,
        tip: String,
        known: bool,
    },
}

const ROW: f32 = 28.0;

fn status_of(s: Severity) -> Status {
    match s {
        Severity::Error => Status::Error,
        Severity::Warning => Status::Warning,
        Severity::NotOptimized => Status::Unverified,
    }
}

fn heading_of(s: Severity) -> &'static str {
    match s {
        Severity::Error => "Errors",
        Severity::Warning => "Warnings",
        Severity::NotOptimized => "Not optimized",
    }
}

/// What can be checked: (shown name, the map's folder or the vehicle's file).
fn items(l: &Launcher, kind: usize) -> Vec<(String, String)> {
    if kind == 0 {
        l.state
            .maps
            .iter()
            .filter_map(|m| {
                let folder = m.file.split('/').nth(1)?.to_string();
                Some((m.friendly.clone(), folder))
            })
            .collect()
    } else {
        l.state
            .vehicles
            .iter()
            .map(|v| {
                (
                    format!("{} ({})", core_name(&v.name), v.folder),
                    v.file.clone(),
                )
            })
            .collect()
    }
}

fn core_name(name: &str) -> String {
    omsi_launcher_lib::display_bus_name(name)
}

/// The check itself, on the thread that runs it.
fn run_check(root: &Path, kind: usize, target: &str) -> Report {
    if kind == 0 {
        let dir = omsi_cfg::resolve_path(&omsi_cfg::resolve_path(root, "Maps"), target);
        omsi_integrity::check_map(root, &dir)
    } else {
        omsi_integrity::check_vehicle(root, &omsi_cfg::resolve_path(root, target))
    }
}

fn start(
    root: PathBuf,
    kind: usize,
    target: String,
    other: Option<String>,
) -> (Receiver<Report>, Arc<AtomicBool>) {
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    std::thread::spawn(move || {
        let mut r = run_check(&root, kind, &target);
        if let Some(o) = other {
            if !flag.load(Ordering::Relaxed) {
                r.mark_known(&run_check(&root, kind, &o));
            }
        }
        if !flag.load(Ordering::Relaxed) {
            let _ = tx.send(r);
        }
    });
    (rx, cancel)
}

fn rows_of(r: &Report) -> Vec<Row> {
    let mut rows = Vec::new();
    for s in [Severity::Error, Severity::Warning, Severity::NotOptimized] {
        let n = r.count(s);
        if n == 0 {
            continue;
        }
        rows.push(Row::Head(s, n));
        for f in r.findings.iter().filter(|f| f.severity == s) {
            let used = f.referenced_by.as_ref().map(|(by, n)| {
                if *n > 1 {
                    omsi_ui::tr("Used by %{file} and %{n} more")
                        .replace("%{file}", by)
                        .replace("%{n}", &(n - 1).to_string())
                } else {
                    omsi_ui::tr("Used by %{file}").replace("%{file}", by)
                }
            });
            let tip = [f.detail.clone(), used]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("\n\n");
            rows.push(Row::Item {
                severity: s,
                file: f.file.clone(),
                message: f.message.clone(),
                tip,
                known: f.known,
            });
        }
    }
    rows
}

pub fn draw(l: &mut Launcher, area: Rect) {
    // the answer of a running check
    if let Some(run) = l.pages.diag.run.as_ref() {
        match run.rx.try_recv() {
            Ok(report) => {
                let d = &mut l.pages.diag;
                let name = d.done.as_ref().map(|x| x.name.clone()).unwrap_or_default();
                d.done = Some(Done {
                    name,
                    report,
                    compared: d.compare > 0,
                });
                d.run = None;
            }
            Err(TryRecvError::Disconnected) => {
                log::warn!("integrity check: the check stopped without an answer");
                l.pages.diag.run = None;
                l.pages.diag.done = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }
    let kind = l.pages.diag.kind;
    let list = items(l, kind);
    let root = PathBuf::from(l.state.config.root.trim());
    let empty = list.is_empty() || root.as_os_str().is_empty();
    l.pages.diag.item = l.pages.diag.item.min(list.len().saturating_sub(1));
    l.pages.diag.compare = l.pages.diag.compare.min(list.len());
    let busy = l.pages.diag.run.is_some();

    // what to check
    let h = 36.0;
    let mut k = kind;
    if l.ui.segmented(
        "integrity-kind",
        Rect::new(area.x, area.y, 220.0f32.min(area.w), h),
        &mut k,
        &["Map", "Vehicle"],
    ) && !busy
    {
        let d = &mut l.pages.diag;
        d.kind = k;
        d.item = 0;
        d.compare = 0;
        d.done = None;
    }
    if empty {
        clipped(
            &mut l.ui,
            "Nothing to verify here.",
            area.x + 4.0,
            area.y + h + 26.0,
            area.w - 8.0,
            13.0,
            Weight::Regular,
            MUTED,
        );
        return;
    }
    let names: Vec<String> = list.iter().map(|x| x.0.clone()).collect();
    let sel_w = ((area.w - 240.0 - 12.0) * 0.5).clamp(160.0, 340.0);
    let mut item = l.pages.diag.item;
    if l.ui.select(
        "integrity-item",
        Rect::new(area.x + 232.0, area.y, sel_w, h),
        &mut item,
        &names,
    ) && !busy
    {
        l.pages.diag.item = item;
        l.pages.diag.done = None;
    }

    // what to compare with, and the button
    let y2 = area.y + h + 12.0;
    let label_w = text(
        &mut l.ui,
        "Compare with",
        area.x + 4.0,
        y2 + h * 0.5,
        13.0,
        Weight::Regular,
        MUTED,
    );
    let mut opts = vec![omsi_ui::tr("None").to_string()];
    opts.extend(names.iter().cloned());
    let mut cmp = l.pages.diag.compare;
    if l.ui.select(
        "integrity-compare",
        Rect::new(area.x + 16.0 + label_w, y2, sel_w, h),
        &mut cmp,
        &opts,
    ) && !busy
    {
        l.pages.diag.compare = cmp;
    }
    let bw = 160.0f32;
    let b = Rect::new(area.right() - bw, y2, bw, h);
    if busy {
        if action(
            &mut l.ui,
            "integrity:cancel",
            b,
            "Cancel",
            "Stops waiting for the check; what it finds is dropped.",
            true,
        ) {
            if let Some(run) = l.pages.diag.run.take() {
                run.cancel.store(true, Ordering::Relaxed);
            }
            // (the placeholder of the check that was running is not a result)
            l.pages.diag.done = None;
        }
    } else if action(
        &mut l.ui,
        "integrity:verify",
        b,
        "VERIFY",
        "Checks it for what is missing or unreadable, and what works but is wasteful. Nothing is changed.",
        true,
    ) {
        let target = list[l.pages.diag.item].1.clone();
        let other = l
            .pages
            .diag
            .compare
            .checked_sub(1)
            .map(|k| list[k].1.clone())
            .filter(|o| *o != target);
        let (rx, cancel) = start(root, kind, target, other);
        let d = &mut l.pages.diag;
        d.done = Some(Done {
            name: list[d.item].0.clone(),
            report: Report::default(),
            compared: false,
        });
        d.run = Some(Run { rx, cancel });
    }

    // the result
    let top = y2 + h + 16.0;
    let r = Rect::new(area.x, top, area.w, (area.bottom() - top - 8.0).max(120.0));
    l.ui.solid(r);
    l.ui.p()
        .rounded(r, omsi_ui::theme::RADIUS, omsi_ui::theme::surface(PANEL));
    l.ui.p()
        .rounded_border(r, omsi_ui::theme::RADIUS, 1.0, LINE);
    if busy {
        clipped(
            &mut l.ui,
            "Checking…",
            r.x + 19.0,
            r.y + 28.0,
            r.w - 38.0,
            13.0,
            Weight::Regular,
            MUTED,
        );
        return;
    }
    let Some(done) = l.pages.diag.done.as_ref() else {
        l.ui.paragraph(
            "Checks a map or a vehicle for what is missing or wasteful. Nothing is changed.",
            glam::Vec2::new(r.x + 19.0, r.y + 18.0),
            r.w - 38.0,
            13.0,
            Weight::Regular,
            MUTED,
        );
        return;
    };
    let rep = &done.report;
    let (e, w, o) = if done.compared {
        (
            rep.count_new(Severity::Error),
            rep.count_new(Severity::Warning),
            rep.count_new(Severity::NotOptimized),
        )
    } else {
        (
            rep.count(Severity::Error),
            rep.count(Severity::Warning),
            rep.count(Severity::NotOptimized),
        )
    };
    let summary = omsi_ui::tr("%{e} errors, %{w} warnings, %{o} not optimized")
        .replace("%{e}", &e.to_string())
        .replace("%{w}", &w.to_string())
        .replace("%{o}", &o.to_string());
    let dot_status = if e > 0 {
        Status::Error
    } else if w > 0 || o > 0 {
        Status::Warning
    } else {
        Status::Found
    };
    dot(&mut l.ui, r.x + 24.0, r.y + 26.0, dot_status);
    let name = done.name.clone();
    let tw = text(
        &mut l.ui,
        &name,
        r.x + 42.0,
        r.y + 26.0,
        14.0,
        Weight::Medium,
        FG,
    );
    clipped(
        &mut l.ui,
        &summary,
        r.x + 60.0 + tw,
        r.y + 26.0,
        (r.w - 80.0 - tw).max(40.0),
        13.0,
        Weight::Regular,
        MUTED,
    );
    let list_r = Rect::new(r.x + 8.0, r.y + 52.0, r.w - 16.0, r.h - 60.0);
    if rep.findings.is_empty() {
        clipped(
            &mut l.ui,
            "No problems found.",
            r.x + 19.0,
            r.y + 68.0,
            r.w - 38.0,
            13.0,
            Weight::Regular,
            OK,
        );
        return;
    }
    let rows = rows_of(rep);
    l.ui.scroll_area("integrity-rows", list_r, &mut |ui, v| {
        let mut y = v.y;
        for row in &rows {
            let cell = Rect::new(v.x, y, v.w, ROW);
            match row {
                Row::Head(s, n) => {
                    let c = status_of(*s);
                    dot(ui, v.x + 12.0, y + ROW * 0.5, c);
                    let head = format!("{} ({n})", omsi_ui::tr(heading_of(*s)));
                    text(
                        ui,
                        &head,
                        v.x + 30.0,
                        y + ROW * 0.5,
                        13.0,
                        Weight::Medium,
                        FG,
                    );
                }
                Row::Item {
                    severity,
                    file,
                    message,
                    tip,
                    known,
                } => {
                    let c = if *known { MUTED } else { FG };
                    let fw = (v.w * 0.42).max(120.0);
                    clipped(
                        ui,
                        file,
                        v.x + 30.0,
                        y + ROW * 0.5,
                        fw - 30.0,
                        12.5,
                        Weight::Regular,
                        c,
                    );
                    let note = if *known {
                        format!("{message} - {}", omsi_ui::tr("Already in the compared one"))
                    } else {
                        message.clone()
                    };
                    clipped(
                        ui,
                        &note,
                        v.x + fw + 8.0,
                        y + ROW * 0.5,
                        v.w - fw - 16.0,
                        12.5,
                        Weight::Regular,
                        if *known {
                            MUTED
                        } else {
                            status_of(*severity).color()
                        },
                    );
                    if !tip.is_empty() {
                        ui.tooltip(cell, tip);
                    }
                }
            }
            y += ROW;
        }
        y - v.y
    });
}

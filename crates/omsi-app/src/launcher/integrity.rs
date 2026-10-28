//! The Integrity tab of the Diagnostics page: on request, a complete check of one map or one
//! vehicle (`omsi-integrity`) - what is missing or unreadable, and what works but is wasteful
//! for OMSI 2 and DirectX 9. The result has a tab for each kind of file, filters by severity
//! and by text, and goes out as a plain text report. Nothing on it changes a file.

use super::ui::Key;
use super::Launcher;
use omsi_integrity::{Category, Progress, Report, Severity, TextOptions};
use omsi_launcher_ui::kit::{action, clipped, dot, text, Status};
use omsi_launcher_ui::look::*;
use omsi_ui::paint::Align;
use omsi_ui::widgets::ButtonKind;
use omsi_ui::{Rect, Weight};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

pub struct DiagView {
    /// The page's tab: 0 the overview, 1 the integrity check, 2 the AI traffic.
    pub tab: usize,
    /// What is checked: 0 a map, 1 a vehicle.
    kind: usize,
    item: usize,
    /// The map or vehicle compared with (0: none; k: the list's k - 1th).
    compare: usize,
    run: Option<Run>,
    done: Option<Done>,
    /// The result's tab: 0 all, k the kind of file `tabs(..)[k - 1]`.
    cat: usize,
    /// Errors, warnings, not optimized: which are shown.
    show: [bool; 3],
    search: String,
    /// The export window, while it is open.
    export: Option<Export>,
    /// The AI traffic tab.
    pub(super) ai: super::ai_traffic::AiView,
}

impl Default for DiagView {
    fn default() -> Self {
        DiagView {
            tab: 0,
            kind: 0,
            item: 0,
            compare: 0,
            run: None,
            done: None,
            cat: 0,
            show: [true; 3],
            search: String::new(),
            export: None,
            ai: Default::default(),
        }
    }
}

impl DiagView {
    /// A window lies over the page (the page sees no mouse meanwhile).
    pub fn dialog_open(&self) -> bool {
        (self.tab == 1 && self.export.is_some()) || (self.tab == 2 && self.ai.dialog_open())
    }

    /// The AI traffic tab shows one of its vehicles in the preview.
    pub fn shows_vehicle(&self) -> bool {
        self.tab == 2 && self.ai.shows_vehicle()
    }
}

/// A check running in the background; cancelling it stops it within a file read.
struct Run {
    rx: Receiver<Report>,
    progress: Arc<Progress>,
}

pub struct Done {
    pub name: String,
    /// The map's folder or the vehicle's file, as checked.
    pub target: String,
    /// The name of what it was compared with.
    pub compared: Option<String>,
    /// When it was checked (`YYYY-MM-DD HH:MM`), for the report.
    pub when: String,
    pub report: Report,
}

/// The export window's choices.
struct Export {
    severities: [bool; 3],
    why: bool,
    used_by: bool,
    known: bool,
    /// 0 sections by kind of file, 1 by severity.
    group: usize,
    /// 0 every kind, 1 only the result's open tab.
    only_tab: usize,
}

/// One line of the result.
pub enum Row {
    Head(Severity, usize),
    Item {
        severity: Severity,
        category: Category,
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

const SEVERITIES: [Severity; 3] = [Severity::Error, Severity::Warning, Severity::NotOptimized];

/// The result's tabs after "All": the kinds of file a map (0) or a vehicle (1) has, and any
/// other kind the report found something in.
pub fn tabs(kind: usize, r: &Report) -> Vec<Category> {
    let base: &[Category] = if kind == 0 {
        &[
            Category::Tiles,
            Category::Objects,
            Category::Splines,
            Category::Models,
            Category::Textures,
            Category::AiLists,
            Category::Timetable,
            Category::Paths,
        ]
    } else {
        &[
            Category::Vehicle,
            Category::Models,
            Category::Textures,
            Category::Sounds,
            Category::Scripts,
            Category::Paths,
        ]
    };
    Category::ALL
        .into_iter()
        .filter(|c| base.contains(c) || r.count_in(*c) > 0)
        .collect()
}

/// What can be checked: (shown name, the map's folder or the vehicle's file).
pub(super) fn items(l: &Launcher, kind: usize) -> Vec<(String, String)> {
    if kind == 0 {
        l.state
            .maps
            .iter()
            .filter_map(|m| {
                let folder = m.file.split('/').nth(1)?.to_string();
                // (a map without a friendly name goes by its folder's)
                let name = if m.friendly.trim().is_empty() {
                    folder.clone()
                } else {
                    m.friendly.clone()
                };
                Some((name, folder))
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
fn run_check(root: &Path, kind: usize, target: &str, progress: &Progress) -> Report {
    if kind == 0 {
        let dir = omsi_cfg::resolve_path(&omsi_cfg::resolve_path(root, "Maps"), target);
        omsi_integrity::check_map_with(root, &dir, progress)
    } else {
        omsi_integrity::check_vehicle_with(root, &omsi_cfg::resolve_path(root, target), progress)
    }
}

fn start(
    root: PathBuf,
    kind: usize,
    target: String,
    other: Option<String>,
) -> (Receiver<Report>, Arc<Progress>) {
    let (tx, rx) = channel();
    let progress = Arc::new(Progress::new());
    let p = progress.clone();
    std::thread::spawn(move || {
        let run = |t: &str, p: &Progress| run_check(&root, kind, t, p);
        if let Some(r) = check_and_compare(run, &target, other.as_deref(), &p) {
            let _ = tx.send(r);
        }
    });
    (rx, progress)
}

/// `target` checked by `run`, its findings that `other` has too marked as known; nothing
/// when the check was cancelled. Both checks follow the one `progress`, so that Cancel stops
/// the comparison too (whose steps are not shown: the bar stays full meanwhile, as `steps`
/// keeps the count within the total).
pub fn check_and_compare(
    run: impl Fn(&str, &Progress) -> Report,
    target: &str,
    other: Option<&str>,
    progress: &Progress,
) -> Option<Report> {
    let mut r = run(target, progress);
    if let Some(o) = other {
        if !progress.cancelled() {
            r.mark_known(&run(o, progress));
        }
    }
    (!progress.cancelled()).then_some(r)
}

/// Whether a finding counts in the heading, the tabs and the chips: with a comparison, only
/// those the compared one does not have (the others are still listed, greyed).
fn counts(f: &omsi_integrity::Finding, compared: bool) -> bool {
    !compared || !f.known
}

fn now_text() -> String {
    match omsi_launcher_lib::local_now() {
        Some((y, mo, d, h, m)) => format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}"),
        None => String::new(),
    }
}

pub fn rows_of(r: &Report, only: Option<Category>, show: [bool; 3], search: &str) -> Vec<Row> {
    let needle = search.trim().to_lowercase();
    let mut rows = Vec::new();
    for (i, s) in SEVERITIES.into_iter().enumerate() {
        if !show[i] {
            continue;
        }
        let these: Vec<&omsi_integrity::Finding> = r
            .findings
            .iter()
            .filter(|f| f.severity == s)
            .filter(|f| only.is_none_or(|c| f.category() == c))
            .filter(|f| {
                needle.is_empty()
                    || f.file.to_lowercase().contains(&needle)
                    || f.message.to_lowercase().contains(&needle)
            })
            .collect();
        if these.is_empty() {
            continue;
        }
        rows.push(Row::Head(s, these.len()));
        for f in these {
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
                category: f.category(),
                file: f.file.clone(),
                message: f.message.clone(),
                tip,
                known: f.known,
            });
        }
    }
    rows
}

/// The text a report goes out as: what the export window chose, or the summary alone.
fn report_text(d: &Done, kind: usize, opts: Option<&Export>, open: Option<Category>) -> String {
    let mut header = vec![(
        if kind == 0 { "Map" } else { "Vehicle" }.to_string(),
        format!("{} ({})", d.name, d.target),
    )];
    if !d.when.is_empty() {
        header.push(("Checked".into(), d.when.clone()));
    }
    header.push((
        "Build".into(),
        format!(
            "Project Depot {} ({})",
            crate::startup::VERSION,
            crate::startup::BUILD
        ),
    ));
    header.push((
        "Compared".into(),
        d.compared.clone().unwrap_or_else(|| "none".into()),
    ));
    let o = match opts {
        Some(e) => TextOptions {
            header,
            severities: e.severities,
            why: e.why,
            used_by: e.used_by,
            known: e.known,
            by_severity: e.group == 1,
            only: if e.only_tab == 1 { open } else { None },
            items: true,
        },
        None => TextOptions {
            header,
            items: false,
            ..Default::default()
        },
    };
    d.report.to_text(&o)
}

/// The report's file name: what was checked and when, without what a file name cannot hold.
pub fn file_name(d: &Done) -> String {
    let stamp = d.when.replace(':', "");
    let clean: String = d
        .name
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    format!("{} - integrity {}.txt", clean.trim(), stamp)
        .replace("  ", " ")
        .replace(" .txt", ".txt")
}

pub fn draw(l: &mut Launcher, area: Rect) {
    // the answer of a running check
    if let Some(run) = l.pages.diag.run.as_ref() {
        match run.rx.try_recv() {
            Ok(report) => {
                let d = &mut l.pages.diag;
                if let Some(done) = d.done.as_mut() {
                    done.report = report;
                    done.when = now_text();
                }
                d.run = None;
            }
            Err(TryRecvError::Disconnected) => {
                if !run.progress.cancelled() {
                    log::warn!("integrity check: the check stopped without an answer");
                }
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
    let kind_r = Rect::new(area.x, area.y, 220.0f32.min(area.w), h);
    if l.ui
        .segmented("integrity-kind", kind_r, &mut k, &["Map", "Vehicle"])
        && !busy
    {
        let d = &mut l.pages.diag;
        d.kind = k;
        d.item = 0;
        d.compare = 0;
        d.done = None;
        d.cat = 0;
    }
    l.ui.segmented_tips(
        kind_r,
        &[
            "Check a map: its tiles, objects, splines and their files.",
            "Check a vehicle: its model, scripts, sounds and paint schemes.",
        ],
    );
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
    if l.ui.select_as_is(
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
    let cmp_r = Rect::new(area.x + 16.0 + label_w, y2, sel_w, h);
    if l.ui
        .select_as_is("integrity-compare", cmp_r, &mut cmp, &opts)
        && !busy
    {
        l.pages.diag.compare = cmp;
    }
    l.ui.tooltip(
        cmp_r,
        "What another map or vehicle (for example the original of an optimized copy) has too is marked, and only the rest is counted.",
    );
    let bw = 160.0f32;
    let b = Rect::new(area.right() - bw, y2, bw, h);
    if busy {
        if l.ui.button(
            "integrity:cancel",
            b,
            "Cancel",
            Some("o-close"),
            ButtonKind::Normal,
        ) {
            if let Some(run) = l.pages.diag.run.take() {
                run.progress.cancel();
            }
            // (the placeholder of the check that was running is not a result)
            l.pages.diag.done = None;
        }
        l.ui.tooltip(b, "Stops the check now. What it found so far is dropped.");
    } else {
        if action(&mut l.ui, "integrity:verify", b, "VERIFY", "", true) {
            let target = list[l.pages.diag.item].1.clone();
            let other = l
                .pages
                .diag
                .compare
                .checked_sub(1)
                .map(|k| list[k].clone())
                .filter(|o| o.1 != target);
            let (rx, progress) = start(root, kind, target.clone(), other.clone().map(|o| o.1));
            let d = &mut l.pages.diag;
            d.done = Some(Done {
                name: list[d.item].0.clone(),
                target,
                compared: other.map(|o| o.0),
                when: String::new(),
                report: Report::default(),
            });
            d.run = Some(Run { rx, progress });
        }
        l.ui.tooltip(
            b,
            "Checks it for what is missing or unreadable, and what works but is wasteful. Nothing is changed.",
        );
    }

    // the result
    let top = y2 + h + 16.0;
    let r = Rect::new(area.x, top, area.w, (area.bottom() - top - 8.0).max(160.0));
    l.ui.solid(r);
    l.ui.p()
        .rounded(r, omsi_ui::theme::RADIUS, omsi_ui::theme::surface(PANEL));
    l.ui.p()
        .rounded_border(r, omsi_ui::theme::RADIUS, 1.0, LINE);
    if let Some(run) = l.pages.diag.run.as_ref() {
        let steps = run.progress.steps();
        let (frac, line) = match steps {
            Some((done, total)) => (
                done as f32 / total as f32,
                omsi_ui::tr("Reading tile %{n} of %{total}")
                    .replace("%{n}", &done.to_string())
                    .replace("%{total}", &total.to_string()),
            ),
            None => (0.35, omsi_ui::tr("Checking…").to_string()),
        };
        clipped(
            &mut l.ui,
            &line,
            r.x + 19.0,
            r.y + 28.0,
            r.w - 38.0,
            13.0,
            Weight::Regular,
            MUTED,
        );
        l.ui.progress(
            Rect::new(r.x + 19.0, r.y + 46.0, r.w - 38.0, 4.0),
            frac,
            true,
        );
        return;
    }
    if l.pages.diag.done.is_none() {
        l.ui.paragraph(
            "Checks a map or a vehicle for what is missing or wasteful. Nothing is changed.",
            glam::Vec2::new(r.x + 19.0, r.y + 18.0),
            r.w - 38.0,
            13.0,
            Weight::Regular,
            MUTED,
        );
        return;
    }
    result(l, r, kind);
}

/// The result of the last check, in the panel `r`.
fn result(l: &mut Launcher, r: Rect, kind: usize) {
    let Some(done) = l.pages.diag.done.as_ref() else {
        return;
    };
    let rep = &done.report;
    let compared = done.compared.is_some();
    let count = |s: Severity| {
        if compared {
            rep.count_new(s)
        } else {
            rep.count(s)
        }
    };
    let (e, w, o) = (
        count(Severity::Error),
        count(Severity::Warning),
        count(Severity::NotOptimized),
    );
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
    let name = done.name.clone();
    let tabs = tabs(kind, rep);
    let stats: Vec<(&str, usize)> = if kind == 0 {
        vec![
            ("Tiles", rep.tiles),
            ("Objects", rep.objects),
            ("Splines", rep.splines),
            ("Meshes", rep.meshes),
            ("Textures", rep.textures),
        ]
    } else {
        vec![("Meshes", rep.meshes), ("Textures", rep.textures)]
    };
    let empty = rep.findings.is_empty();

    // the heading: what, how it went, and the report's buttons
    dot(&mut l.ui, r.x + 24.0, r.y + 26.0, dot_status);
    let tw = l.ui.width(&name, 14.0 * TEXT_SCALE, Weight::Medium);
    let buttons_w = 360.0f32.min(r.w * 0.45);
    let tw = tw.min(r.w - buttons_w - 80.0).max(40.0);
    l.ui.text_in_as_is(
        &name,
        Rect::new(r.x + 42.0, r.y + 12.0, tw, 28.0),
        14.0,
        Weight::Medium,
        FG,
    );
    clipped(
        &mut l.ui,
        &summary,
        r.x + 58.0 + tw,
        r.y + 26.0,
        (r.w - buttons_w - 90.0 - tw).max(40.0),
        13.0,
        Weight::Regular,
        MUTED,
    );
    let bx = r.right() - 16.0 - buttons_w;
    let half = (buttons_w - 8.0) * 0.5;
    let export_r = Rect::new(bx, r.y + 11.0, half + 30.0, 32.0);
    let copy_r = Rect::new(export_r.right() + 8.0, r.y + 11.0, half - 30.0, 32.0);
    if l.ui.button(
        "integrity:export",
        export_r,
        "Export report",
        Some("o-save"),
        ButtonKind::Normal,
    ) {
        let compared = l
            .pages
            .diag
            .done
            .as_ref()
            .is_some_and(|d| d.compared.is_some());
        l.pages.diag.export = Some(Export {
            severities: l.pages.diag.show,
            why: true,
            used_by: true,
            known: !compared,
            group: 0,
            only_tab: 0,
        });
    }
    l.ui.tooltip(
        export_r,
        "Saves the result as a text file, to keep or to send along.",
    );
    if l.ui.button(
        "integrity:copy",
        copy_r,
        "Copy summary",
        Some("content_copy"),
        ButtonKind::Normal,
    ) {
        if let Some(d) = l.pages.diag.done.as_ref() {
            l.ui.clipboard_out = Some(report_text(d, kind, None, None));
            l.state
                .set_status("The summary of the check is on the clipboard.", false);
        }
    }
    l.ui.tooltip(
        copy_r,
        "Puts what was checked and how many findings of each kind there are on the clipboard.",
    );

    // what was read: a cell per kind of file
    let sy = r.y + 52.0;
    let cell_w = (r.w - 32.0) / stats.len() as f32;
    let strip = Rect::new(r.x + 16.0, sy, r.w - 32.0, 46.0);
    l.ui.p().rounded_border(strip, 6.0, 1.0, LINE);
    for (i, (label, n)) in stats.iter().enumerate() {
        let x = strip.x + cell_w * i as f32;
        if i > 0 {
            l.ui.p()
                .rect(Rect::new(x, strip.y + 1.0, 1.0, strip.h - 2.0), LINE);
        }
        text(
            &mut l.ui,
            &n.to_string(),
            x + 14.0,
            strip.y + 17.0,
            15.0,
            Weight::Medium,
            FG,
        );
        clipped(
            &mut l.ui,
            label,
            x + 14.0,
            strip.y + 34.0,
            cell_w - 20.0,
            11.5,
            Weight::Regular,
            MUTED,
        );
    }
    if rep.cancelled {
        clipped(
            &mut l.ui,
            "The check was stopped before its end: only part of the content was looked at.",
            r.x + 19.0,
            sy + 62.0,
            r.w - 38.0,
            12.5,
            Weight::Regular,
            WARN,
        );
    }

    // a tab for each kind of file
    let ty = sy + 58.0;
    l.pages.diag.cat = l.pages.diag.cat.min(tabs.len());
    let only = l.pages.diag.cat.checked_sub(1).map(|k| tabs[k]);
    type_tabs(l, Rect::new(r.x + 8.0, ty, r.w - 16.0, 36.0), &tabs);

    if empty {
        clipped(
            &mut l.ui,
            "No problems found.",
            r.x + 19.0,
            ty + 60.0,
            r.w - 38.0,
            13.0,
            Weight::Regular,
            OK,
        );
        return;
    }

    // the filters: severities and text
    let fy = ty + 46.0;
    let mut x = r.x + 16.0;
    for (i, s) in SEVERITIES.into_iter().enumerate() {
        let n = l
            .pages
            .diag
            .done
            .as_ref()
            .map(|d| {
                d.report
                    .findings
                    .iter()
                    .filter(|f| f.severity == s && only.is_none_or(|c| f.category() == c))
                    .filter(|f| counts(f, d.compared.is_some()))
                    .count()
            })
            .unwrap_or(0);
        let label = format!("{} {n}", omsi_ui::tr(heading_of(s)));
        let cw = l.ui.width(&label, 12.0, Weight::Medium) + 40.0;
        let chip = Rect::new(x, fy, cw, 28.0);
        let on = l.pages.diag.show[i];
        let id = omsi_ui::widgets::id_of(&format!("integrity-sev-{i}"));
        let (hover, _, clicked) = l.ui.interact_key(id, chip);
        if clicked {
            l.pages.diag.show[i] = !on;
        }
        let fill = if on {
            SELECTED_LAYER
        } else if hover {
            HOVER_LAYER
        } else {
            PANEL
        };
        l.ui.p().rounded(chip, 14.0, fill);
        l.ui.p().rounded_border(
            chip,
            14.0,
            1.0,
            if on {
                status_of(s).color().alpha(0.6)
            } else {
                LINE
            },
        );
        dot(&mut l.ui, chip.x + 15.0, chip.y + 14.0, status_of(s));
        l.ui.text_in_as_is(
            &label,
            Rect::new(chip.x + 26.0, chip.y, cw - 30.0, chip.h),
            12.0,
            Weight::Medium,
            if on { FG } else { MUTED },
        );
        l.ui.tooltip(
            chip,
            if on {
                "Shown. Click to hide these findings."
            } else {
                "Hidden. Click to show these findings."
            },
        );
        x += cw + 8.0;
    }
    let sw = 300.0f32.min(r.right() - 16.0 - x - 8.0).max(120.0);
    let search_r = Rect::new(r.right() - 16.0 - sw, fy - 2.0, sw, 32.0);
    let mut search = std::mem::take(&mut l.pages.diag.search);
    l.ui.text_input(
        "integrity-search",
        search_r,
        &mut search,
        "Filter by file or problem…",
        Some("o-search"),
    );
    l.pages.diag.search = search;

    // the findings
    let list_r = Rect::new(
        r.x + 8.0,
        fy + 40.0,
        r.w - 16.0,
        (r.bottom() - fy - 48.0).max(40.0),
    );
    let d = &l.pages.diag;
    let rows = rows_of(&d.done.as_ref().unwrap().report, only, d.show, &d.search);
    if rows.is_empty() {
        clipped(
            &mut l.ui,
            "Nothing matches the filter.",
            list_r.x + 11.0,
            list_r.y + 16.0,
            list_r.w - 22.0,
            13.0,
            Weight::Regular,
            MUTED,
        );
        return;
    }
    let with_type = only.is_none();
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
                    category,
                    file,
                    message,
                    tip,
                    known,
                } => {
                    let c = if *known { MUTED } else { FG };
                    let type_w = if with_type { 96.0 } else { 0.0 };
                    if with_type {
                        clipped(
                            ui,
                            category.label(),
                            v.x + 30.0,
                            y + ROW * 0.5,
                            type_w - 8.0,
                            12.5,
                            Weight::Regular,
                            MUTED,
                        );
                    }
                    let fx = v.x + 30.0 + type_w;
                    let fw = ((v.w - type_w) * 0.42).max(120.0);
                    let file_r = Rect::new(fx, y, fw - 30.0, ROW);
                    ui.text_in_as_is(file, file_r, 12.5, Weight::Regular, c);
                    let note = if *known {
                        format!("{message} - {}", omsi_ui::tr("Already in the compared one"))
                    } else {
                        message.clone()
                    };
                    let mx = fx + fw - 22.0;
                    ui.text_in_as_is(
                        &note,
                        Rect::new(mx, y, (v.right() - mx - 8.0).max(20.0), ROW),
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

/// The result's tab bar: "All" and a tab for each kind of file, with how many findings it
/// has and a dot in the colour of the worst of them.
fn type_tabs(l: &mut Launcher, bar: Rect, tabs: &[Category]) {
    let Some(done) = l.pages.diag.done.as_ref() else {
        return;
    };
    let mut labels: Vec<(String, usize, Option<Severity>, String)> = Vec::new();
    {
        // (with a comparison, as the heading: only what the compared one does not have)
        let counted: Vec<&omsi_integrity::Finding> = done
            .report
            .findings
            .iter()
            .filter(|f| counts(f, done.compared.is_some()))
            .collect();
        let worst = |f: &dyn Fn(&omsi_integrity::Finding) -> bool| {
            counted.iter().filter(|x| f(x)).map(|x| x.severity).min()
        };
        let tip_of = |f: &dyn Fn(&omsi_integrity::Finding) -> bool| {
            let n = |s: Severity| counted.iter().filter(|x| f(x) && x.severity == s).count();
            omsi_ui::tr("%{e} errors, %{w} warnings, %{o} not optimized")
                .replace("%{e}", &n(Severity::Error).to_string())
                .replace("%{w}", &n(Severity::Warning).to_string())
                .replace("%{o}", &n(Severity::NotOptimized).to_string())
        };
        labels.push((
            omsi_ui::tr("All").to_string(),
            counted.len(),
            worst(&|_| true),
            tip_of(&|_| true),
        ));
        for c in tabs {
            let c = *c;
            labels.push((
                omsi_ui::tr(c.label()).to_string(),
                counted.iter().filter(|f| f.category() == c).count(),
                worst(&|f| f.category() == c),
                tip_of(&|f| f.category() == c),
            ));
        }
    }
    l.ui.p()
        .rect(Rect::new(bar.x, bar.bottom() - 1.0, bar.w, 1.0), LINE);
    let px = 12.5;
    // (narrower padding when the tabs do not fit the bar)
    let natural: f32 = labels
        .iter()
        .map(|(t, n, _, _)| {
            l.ui.width(t, px, Weight::Medium)
                + l.ui.width(&n.to_string(), 11.0, Weight::Medium)
                + 46.0
        })
        .sum();
    let squeeze = (bar.w / natural).min(1.0);
    let mut x = bar.x;
    for (i, (label, n, worst, tip)) in labels.iter().enumerate() {
        let lw = l.ui.width(label, px, Weight::Medium);
        let nw = l.ui.width(&n.to_string(), 11.0, Weight::Medium) + 12.0;
        let w = (lw + nw + 46.0) * squeeze;
        let cell = Rect::new(x, bar.y, w, bar.h);
        let id = omsi_ui::widgets::id_of(&format!("integrity-cat-{i}"));
        let (hover, _, clicked) = l.ui.interact_key(id, cell);
        if clicked {
            l.pages.diag.cat = i;
        }
        let on = l.pages.diag.cat == i;
        if hover && !on {
            l.ui.p().rounded(cell.inset(1.0), 4.0, HOVER_LAYER);
        }
        let st = worst.map(status_of).unwrap_or(Status::Found);
        if *n > 0 {
            dot(&mut l.ui, x + 12.0 * squeeze + 4.0, bar.y + bar.h * 0.5, st);
        }
        let tx = x + 12.0 * squeeze + 14.0;
        let room = (w - (tx - x) - nw - 6.0).max(10.0);
        l.ui.text_in_as_is(
            label,
            Rect::new(tx, bar.y, room, bar.h),
            px,
            Weight::Medium,
            if on { FG } else { MUTED },
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
        if on {
            l.ui.p().rounded(
                Rect::new(cell.x + 8.0, bar.bottom() - 3.0, cell.w - 16.0, 3.0),
                1.5,
                omsi_ui::theme::accent(),
            );
        }
        l.ui.tooltip(cell, tip);
        x += w;
    }
}

/// The export window over the page: what goes into the report, a preview, and where it goes.
pub fn dialog(l: &mut Launcher) {
    if l.pages.diag.tab == 2 {
        super::ai_edit::dialog(l);
        return;
    }
    let Some(mut e) = l.pages.diag.export.take() else {
        return;
    };
    if l.ui.input.keys.contains(&Key::Escape) {
        return;
    }
    let kind = l.pages.diag.kind;
    let open = l.pages.diag.done.as_ref().and_then(|d| {
        let t = tabs(kind, &d.report);
        l.pages
            .diag
            .cat
            .checked_sub(1)
            .and_then(|k| t.get(k).copied())
    });
    let Some(name) = l.pages.diag.done.as_ref().map(|d| d.name.clone()) else {
        return;
    };
    let note = format!("{name} · {}", omsi_ui::tr("plain text"));
    let inside = super::start::dialog(l, 1060.0, 680.0, "Export the report", &note);
    let (buttons, body) = super::start::dialog_foot(l, inside);
    let compared = l
        .pages
        .diag
        .done
        .as_ref()
        .is_some_and(|d| d.compared.is_some());

    // the choices
    let cw = 320.0f32.min(body.w * 0.4);
    let mut y = body.y;
    clipped(
        &mut l.ui,
        "Include",
        body.x,
        y + 8.0,
        cw,
        12.0,
        Weight::Regular,
        MUTED,
    );
    y += 20.0;
    let [se, sw, so] = &mut e.severities;
    let mut toggles: Vec<(&str, &str, &mut bool)> = vec![
        ("Errors", "integrity-ex-e", se),
        ("Warnings", "integrity-ex-w", sw),
        ("Not optimized", "integrity-ex-o", so),
        ("Why it matters", "integrity-ex-why", &mut e.why),
        ("Which files use it", "integrity-ex-by", &mut e.used_by),
    ];
    if compared {
        toggles.push((
            "Items already in the compared one",
            "integrity-ex-known",
            &mut e.known,
        ));
    }
    for (label, id, v) in toggles {
        let tr = Rect::new(body.x, y, cw, 30.0);
        l.ui.toggle(id, tr, v, label);
        y += 32.0;
    }
    y += 10.0;
    clipped(
        &mut l.ui,
        "Group by",
        body.x,
        y + 8.0,
        cw,
        12.0,
        Weight::Regular,
        MUTED,
    );
    y += 20.0;
    let group_r = Rect::new(body.x, y, cw, 34.0);
    l.ui.segmented(
        "integrity-ex-group",
        group_r,
        &mut e.group,
        &["Kind of file", "Severity"],
    );
    l.ui.segmented_tips(
        group_r,
        &[
            "A section for each kind of file: tiles, objects, textures...",
            "A section for errors, one for warnings, one for not optimized.",
        ],
    );
    y += 50.0;
    clipped(
        &mut l.ui,
        "Kind of file",
        body.x,
        y + 8.0,
        cw,
        12.0,
        Weight::Regular,
        MUTED,
    );
    y += 20.0;
    let only_r = Rect::new(body.x, y, cw, 34.0);
    if open.is_some() {
        l.ui.segmented(
            "integrity-ex-only",
            only_r,
            &mut e.only_tab,
            &["All", "Only the open tab"],
        );
        l.ui.segmented_tips(
            only_r,
            &[
                "Every kind of file.",
                "Only the kind of file of the result's open tab.",
            ],
        );
    } else {
        e.only_tab = 0;
        clipped(
            &mut l.ui,
            "All",
            body.x + 4.0,
            y + 17.0,
            cw,
            13.0,
            Weight::Regular,
            FG,
        );
    }
    l.ui.paragraph(
        "Saved as UTF-8 text with Windows line endings, so Notepad and the forums show it as it is. Nothing in the map or vehicle is changed.",
        glam::Vec2::new(body.x, body.bottom() - 54.0),
        cw,
        12.0,
        Weight::Regular,
        MUTED,
    );

    // the preview
    let px = body.x + cw + 24.0;
    let pw = body.right() - px;
    clipped(
        &mut l.ui,
        "Preview",
        px,
        body.y + 8.0,
        pw,
        12.0,
        Weight::Regular,
        MUTED,
    );
    let pr = Rect::new(px, body.y + 20.0, pw, body.h - 46.0);
    l.ui.p()
        .rounded(pr, 6.0, omsi_ui::Color::rgba(0, 0, 0, 0.25));
    l.ui.p().rounded_border(pr, 6.0, 1.0, LINE);
    let Some(d) = l.pages.diag.done.as_ref() else {
        return;
    };
    let report = report_text(d, kind, Some(&e), open);
    let fname = file_name(d);
    let lines: Vec<String> = report
        .split("\r\n")
        .map(|s| s.replace(' ', "\u{2007}").to_string())
        .collect();
    l.ui.scroll_area("integrity-ex-preview", pr.inset(10.0), &mut |ui, v| {
        let mut yy = v.y;
        for line in &lines {
            ui.text_in_as_is(
                line,
                Rect::new(v.x, yy, v.w, 17.0),
                12.0,
                Weight::Regular,
                FG,
            );
            yy += 17.0;
        }
        yy - v.y
    });
    clipped(
        &mut l.ui,
        &omsi_ui::tr("File name: %{name}").replace("%{name}", &fname),
        px,
        pr.bottom() + 14.0,
        pw,
        12.0,
        Weight::Regular,
        MUTED,
    );

    // the buttons
    let bw = 150.0;
    let copy_r = Rect::new(buttons.x, buttons.y, bw + 30.0, buttons.h);
    let save_r = Rect::new(buttons.right() - bw, buttons.y, bw, buttons.h);
    let cancel_r = Rect::new(save_r.x - 8.0 - 120.0, buttons.y, 120.0, buttons.h);
    if l.ui.button(
        "integrity-ex-copy",
        copy_r,
        "Copy to clipboard",
        Some("content_copy"),
        ButtonKind::Normal,
    ) {
        l.ui.clipboard_out = Some(report.clone());
        l.state.set_status("The report is on the clipboard.", false);
        return;
    }
    l.ui.tooltip(
        copy_r,
        "Puts the whole report on the clipboard, to paste where you send it.",
    );
    if l.ui.button(
        "integrity-ex-cancel",
        cancel_r,
        "Cancel",
        None,
        ButtonKind::Normal,
    ) {
        return;
    }
    l.ui.tooltip(cancel_r, "Closes this window without saving.");
    if l.ui.button(
        "integrity-ex-save",
        save_r,
        "Save as…",
        Some("o-save"),
        ButtonKind::Primary,
    ) {
        if let Some(path) = omsi_launcher_lib::save_file("Save the integrity report", &fname) {
            match std::fs::write(&path, report.as_bytes()) {
                Ok(()) => {
                    log::info!("integrity check: report saved to {}", path.display());
                    l.state.set_status("The report is saved.", false);
                }
                Err(err) => {
                    log::warn!(
                        "integrity check: the report could not be saved to {}: {err}",
                        path.display()
                    );
                    l.state.set_status("The report could not be saved.", true);
                }
            }
            return;
        }
    }
    l.ui.tooltip(save_r, "Chooses where to save the report as a text file.");
    l.pages.diag.export = Some(e);
}

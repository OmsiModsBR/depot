//! Setup's "Original OMSI 2 engine" card and the dialogs of OmsiLaunch's installation and
//! removal. The card is drawn on Windows computers only.

use super::original::{available, Flow};
use super::theme::*;
use super::ui::ButtonKind;
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib::original_engine::detect::Integrity;
use omsi_launcher_lib::original_engine::install::{InstallPlan, LICENSE, REPO};
use omsi_launcher_ui::kit::Status;
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};
use std::sync::atomic::Ordering::Relaxed;

const DOTNET_URL: &str = "https://dotnet.microsoft.com/download/dotnet/6.0";
const ROW: f32 = 34.0;

/// What the install dialog lists: the package's files grouped by their first folder, with
/// how many and how big, root files first.
pub fn summarize(plan: &InstallPlan) -> Vec<(String, usize, u64)> {
    let mut groups: Vec<(String, usize, u64)> = Vec::new();
    for f in &plan.files {
        let name = match f.path.split_once('/') {
            Some((dir, _)) => format!("{dir}\\"),
            None => f.path.clone(),
        };
        match groups.iter_mut().find(|g| g.0 == name) {
            Some(g) => {
                g.1 += 1;
                g.2 += f.bytes;
            }
            None => groups.push((name, 1, f.bytes)),
        }
    }
    groups.sort_by_key(|g| g.0.ends_with('\\'));
    groups
}

fn requirement(
    l: &mut Launcher,
    x: f32,
    y: f32,
    w: f32,
    label: &str,
    status: Status,
    detail: &str,
) {
    omsi_launcher_ui::kit::dot(&mut l.ui, x + 4.0, y + ROW * 0.5, status);
    l.ui.text_in(
        label,
        Rect::new(x + 18.0, y, 210.0, ROW),
        13.0,
        Weight::Medium,
        TEXT,
        Align::Left,
    );
    let c = match status {
        Status::Found => OK,
        Status::Warning => WARN,
        Status::Unverified => TEXT_DIM,
        Status::Missing | Status::Error => DANGER,
    };
    l.ui.text_in_tip(
        detail,
        Rect::new(x + 232.0, y, w - 232.0 - 190.0, ROW),
        12.5,
        Weight::Regular,
        c,
        Align::Left,
    );
}

/// The card; returns its foot. Nothing is drawn (and 0 is returned) where the mode does not
/// exist.
pub fn engine_card(l: &mut Launcher, at: Rect) -> f32 {
    if !available() {
        return at.y - super::theme::GAP;
    }
    l.state.engine_refresh(false);
    let on = l.state.config.original_engine.enabled;
    let rows = if on { 4.0 } else { 0.0 };
    let note_h =
        l.ui.paragraph_height(NOTE, at.w - 36.0, 12.0, Weight::Regular);
    let h = 46.0 + ROW + 10.0 + note_h + if on { 12.0 + rows * ROW } else { 0.0 } + 16.0;
    let r = Rect::new(at.x, at.y, at.w, h);
    l.ui.panel(r);
    let inner = Launcher::card_title(
        &mut l.ui,
        Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0),
        "Original OMSI 2 engine",
    );
    let mut value = on;
    let row = Rect::new(inner.x, inner.y, inner.w, ROW);
    if l.ui.toggle(
        "engine-flag",
        row,
        &mut value,
        "Also use Project Depot to open the original OMSI 2 engine",
    ) {
        l.state.engine_set_enabled(value);
    }
    l.ui.tooltip(row, "Adds a switch to start games with the original OMSI 2 through OmsiLaunch. Off by default; nothing in your OMSI 2 folder changes until you confirm an installation.");
    let mut y = inner.y + ROW + 8.0;
    y += l.ui.paragraph(
        NOTE,
        Vec2::new(inner.x, y),
        inner.w,
        12.0,
        Weight::Regular,
        TEXT_FAINT,
    ) + 4.0;
    if !on {
        return r.bottom();
    }
    y += 8.0;
    let st = l.state.original.status.clone();
    let (root_status, root_text) = match &st {
        None => (Status::Unverified, omsi_ui::tr("Checking…").to_string()),
        Some(s) => match (&s.root, &s.folder) {
            (None, _) => (Status::Missing, omsi_ui::tr("Not found: choose the OMSI 2 folder above.").to_string()),
            (Some(_), Some(f)) if f.in_program_files => (
                Status::Error,
                omsi_ui::tr("The folder is under Program Files: OmsiLaunch cannot be installed there. Move OMSI 2 to another folder.").to_string(),
            ),
            (Some(_), Some(f)) if !f.writable => (Status::Warning, omsi_ui::tr("The folder is read-only for this user.").to_string()),
            _ => (Status::Found, omsi_ui::tr("Found").to_string()),
        },
    };
    requirement(
        l,
        inner.x,
        y,
        inner.w,
        "OMSI 2 folder",
        root_status,
        &root_text,
    );
    y += ROW;
    let (x64, x86) = st
        .as_ref()
        .map(|s| (s.dotnet.x64, s.dotnet.x86))
        .unwrap_or((false, false));
    let known = st.is_some();
    for (label, ok, id) in [
        (".NET 6 Desktop Runtime (x64)", x64, "dotnet-x64"),
        (".NET 6 Runtime (x86)", x86, "dotnet-x86"),
    ] {
        let (s, t) = if !known {
            (Status::Unverified, omsi_ui::tr("Checking…").to_string())
        } else if ok {
            (Status::Found, omsi_ui::tr("Installed").to_string())
        } else {
            (Status::Missing, omsi_ui::tr("Not installed").to_string())
        };
        requirement(l, inner.x, y, inner.w, label, s, &t);
        if known && !ok {
            let b = Rect::new(inner.right() - 182.0, y, 182.0, ROW - 4.0);
            if l.ui
                .button(id, b, "Open Microsoft download", None, ButtonKind::Normal)
            {
                crate::about::open_url(DOTNET_URL);
            }
            l.ui.tooltip(
                b,
                "Opens Microsoft's download page for the .NET 6 runtimes in your browser.",
            );
        }
        y += ROW;
    }
    // OmsiLaunch itself
    let ol = st.as_ref().map(|s| s.omsilaunch.clone());
    let (s, t) = match &ol {
        None => (Status::Unverified, omsi_ui::tr("Checking…").to_string()),
        Some(o) if !o.installed => (Status::Missing, omsi_ui::tr("Not installed").to_string()),
        Some(o) if matches!(o.integrity, Integrity::Damaged(_) | Integrity::Unreadable) => (
            Status::Error,
            omsi_ui::tr("Installed, but its files are damaged: install it again.").to_string(),
        ),
        Some(o) if o.control_protocol.is_some() && !l.state.original.ready() => (
            Status::Warning,
            omsi_ui::tr("This version is not one Depot can drive: install a newer one.")
                .to_string(),
        ),
        Some(o) => (
            Status::Found,
            omsi_ui::tr("Installed, version %{version}")
                .replace("%{version}", o.version.as_deref().unwrap_or("?")),
        ),
    };
    requirement(l, inner.x, y, inner.w, "OmsiLaunch", s, &t);
    if let Some(o) = &ol {
        let can = st.as_ref().is_some_and(|s| {
            s.root.is_some() && s.folder.as_ref().is_some_and(|f| !f.in_program_files)
        });
        let busy = l.state.original.busy();
        if o.installed {
            let b = Rect::new(inner.right() - 182.0, y, 182.0, ROW - 4.0);
            if l.ui.button(
                "engine-remove",
                b,
                "Remove OmsiLaunch…",
                None,
                ButtonKind::Normal,
            ) && !busy
            {
                l.state.engine_ask_remove();
            }
            l.ui.tooltip(b, "Removes the OmsiLaunch files from your OMSI 2 folder. Your OMSI 2 files and other plug-ins stay.");
            if matches!(o.integrity, Integrity::Damaged(_) | Integrity::Unreadable)
                || !l.state.original.ready()
            {
                let b = Rect::new(inner.right() - 182.0 - 150.0, y, 142.0, ROW - 4.0);
                if l.ui.button(
                    "engine-reinstall",
                    b,
                    "Install again",
                    None,
                    ButtonKind::Primary,
                ) && !busy
                {
                    l.state.engine_look();
                }
                l.ui.tooltip(
                    b,
                    "Downloads the official OmsiLaunch package again, after you confirm.",
                );
            }
        } else if can {
            let b = Rect::new(inner.right() - 182.0, y, 182.0, ROW - 4.0);
            if l.ui.button(
                "engine-install",
                b,
                "Install OmsiLaunch…",
                None,
                ButtonKind::Primary,
            ) && !busy
            {
                l.state.engine_look();
            }
            l.ui.tooltip(b, "Looks up the official OmsiLaunch release. Nothing is downloaded or changed until you confirm.");
        }
    }
    r.bottom()
}

const NOTE: &str = "With this on and OmsiLaunch installed, a switch under the logo starts games with the original OMSI 2 instead. The files OmsiLaunch changes for a game are restored when it ends.";

impl Launcher {
    /// The install and removal dialogs, over the page.
    pub(super) fn draw_engine_dialog(&mut self) {
        if matches!(self.state.original.flow, Flow::Idle) {
            return;
        }
        let size = self.ui.size;
        let full = Rect::new(0.0, 0.0, size.x, size.y);
        self.ui.solid(full);
        self.ui.p().rect(full, omsi_ui::Color::rgba(0, 0, 0, 0.62));
        let w = (size.x - 48.0).min(600.0);
        // the body, measured first so the panel fits it
        let (title, lines, buttons) = self.engine_dialog_content();
        let mut th = 0.0;
        for (t, px, _, _) in &lines {
            th += self.ui.paragraph_height(t, w - 48.0, *px, Weight::Regular) + 8.0;
        }
        let extra = if matches!(self.state.original.flow, Flow::Downloading { .. }) {
            24.0
        } else {
            0.0
        };
        let h = (130.0 + th + extra).min(size.y - 24.0);
        let r = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
        self.ui.panel(r);
        let inner = Rect::new(r.x + 24.0, r.y + 20.0, r.w - 48.0, r.h - 40.0);
        self.ui.text_in(
            &title,
            Rect::new(inner.x, inner.y, inner.w, 28.0),
            18.0,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        let body = Rect::new(inner.x, inner.y + 40.0, inner.w, inner.h - 130.0);
        self.ui
            .scroll_area("engine-dialog-body", body, &mut |ui, v| {
                let mut y = v.y;
                for (t, px, c, bold) in &lines {
                    let hh = ui.paragraph(
                        t,
                        Vec2::new(v.x, y),
                        v.w - 10.0,
                        *px,
                        if *bold { Weight::Bold } else { Weight::Regular },
                        *c,
                    );
                    y += hh + 8.0;
                }
                y - v.y
            });
        if let Flow::Downloading { .. } = self.state.original.flow {
            let (d, t) = (
                self.state.original.done.load(Relaxed),
                self.state.original.total.load(Relaxed),
            );
            let frac = if t > 0 { d as f32 / t as f32 } else { 0.0 };
            self.ui.progress(
                Rect::new(inner.x, inner.bottom() - 70.0, inner.w, 8.0),
                frac,
                t == 0,
            );
        }
        // buttons: the last is the primary, at the right
        let by = inner.bottom() - 38.0;
        let mut x = inner.right();
        let mut pressed: Option<&'static str> = None;
        for (id, label, kind, enabled, tip) in buttons.iter().rev() {
            let bw = (self.ui.width(label, 13.0, Weight::Medium) + 36.0).max(100.0);
            x -= bw;
            let b = Rect::new(x, by, bw, 38.0);
            if *enabled {
                if self
                    .ui
                    .button(&format!("engine-dlg-{id}"), b, label, None, *kind)
                {
                    pressed = Some(id);
                }
            } else {
                self.ui.p().rounded(b, CONTROL_RADIUS, FIELD);
                self.ui
                    .text_in(label, b, 13.0, Weight::Medium, TEXT_FAINT, Align::Center);
            }
            self.ui.tooltip(b, tip);
            x -= 10.0;
        }
        match pressed {
            Some("cancel") | Some("close") => self.state.engine_cancel(),
            Some("download") => self.state.engine_download(),
            Some("install") => self.state.engine_install(),
            Some("remove") => self.state.engine_remove(),
            _ => {}
        }
    }

    #[allow(clippy::type_complexity)]
    fn engine_dialog_content(
        &self,
    ) -> (
        String,
        Vec<(String, f32, omsi_ui::Color, bool)>,
        Vec<(&'static str, &'static str, ButtonKind, bool, &'static str)>,
    ) {
        let tr = |s: &str| omsi_ui::tr(s).to_string();
        let cancel = (
            "cancel",
            "Cancel",
            ButtonKind::Normal,
            true,
            "Closes this window; nothing is changed.",
        );
        match &self.state.original.flow {
            Flow::Looking => (
                tr("Install OmsiLaunch"),
                vec![(tr("Looking for the newest OmsiLaunch release…"), 13.0, TEXT_DIM, false)],
                vec![cancel],
            ),
            Flow::Offer { release, sha } => (
                tr("Install OmsiLaunch"),
                vec![
                    (tr("Version %{version}").replace("%{version}", &release.version), 15.0, TEXT, true),
                    (tr("From the official release of github.com/%{repo}.").replace("%{repo}", REPO), 13.0, TEXT_DIM, false),
                    (tr("License: %{license}. It is a separate program: Depot does not include or change it.").replace("%{license}", LICENSE), 13.0, TEXT_DIM, false),
                    (tr("SHA-256 of the package:"), 12.5, TEXT_DIM, false),
                    (sha.clone(), 12.0, TEXT, false),
                    (tr("OmsiLaunch is installed into your OMSI 2 folder. Nothing is downloaded until you press Download."), 13.0, TEXT_DIM, false),
                ],
                vec![cancel, ("download", "Download", ButtonKind::Primary, true, "Downloads the package and checks it against the SHA-256 above. Nothing is installed yet.")],
            ),
            Flow::Downloading { release } => {
                let (d, t) = (self.state.original.done.load(Relaxed), self.state.original.total.load(Relaxed));
                let text = if t > 0 {
                    tr("Downloading %{done} of %{total}").replace("%{done}", &super::setup::fmt_size(d)).replace("%{total}", &super::setup::fmt_size(t))
                } else {
                    tr("Downloading…")
                };
                (tr("Install OmsiLaunch"), vec![(tr("Version %{version}").replace("%{version}", &release.version), 15.0, TEXT, true), (text, 13.0, TEXT_DIM, false)], vec![cancel])
            }
            Flow::Review { release, plan, .. } => {
                let mut lines = vec![
                    (tr("Version %{version}").replace("%{version}", &release.version), 15.0, TEXT, true),
                    (tr("The package matches its SHA-256. These files will be added to your OMSI 2 folder:"), 13.0, TEXT_DIM, false),
                ];
                for (name, n, bytes) in summarize(plan) {
                    let line = if n == 1 && !name.ends_with('\\') {
                        format!("{name}   {}", super::setup::fmt_size(bytes))
                    } else {
                        tr("%{name}   %{n} files, %{size}").replace("%{name}", &name).replace("%{n}", &n.to_string()).replace("%{size}", &super::setup::fmt_size(bytes))
                    };
                    lines.push((line, 12.5, TEXT, false));
                }
                if !plan.replaces.is_empty() {
                    lines.push((tr("%{n} files of the OmsiLaunch already installed are replaced.").replace("%{n}", &plan.replaces.len().to_string()), 12.5, TEXT_DIM, false));
                }
                if plan.conflicts.is_empty() {
                    lines.push((tr("No other file of your OMSI 2 folder is changed or overwritten."), 12.5, TEXT_DIM, false));
                } else {
                    lines.push((tr("These files already exist and are not OmsiLaunch's, so nothing is installed: %{files}").replace("%{files}", &plan.conflicts.join(", ")), 12.5, DANGER, false));
                }
                (
                    tr("Install OmsiLaunch"),
                    lines,
                    vec![cancel, ("install", "Install", ButtonKind::Primary, plan.ok(), "Adds these files to your OMSI 2 folder. Removing them later is one button in Setup.")],
                )
            }
            Flow::Installing => (tr("Install OmsiLaunch"), vec![(tr("Installing…"), 13.0, TEXT_DIM, false)], vec![]),
            Flow::ConfirmRemove(plan) => {
                let mut lines = vec![(tr("%{n} files are removed from your OMSI 2 folder. Your OMSI 2 files, session profiles and other plug-ins stay.").replace("%{n}", &plan.files.len().to_string()), 13.0, TEXT_DIM, false)];
                if plan.pending_recovery {
                    lines.push((tr("A session left OMSI 2 files changed. Restore them first: start a game from Depot, or run OmsiLaunch with /recover."), 12.5, DANGER, false));
                }
                (
                    tr("Remove OmsiLaunch"),
                    lines,
                    vec![cancel, ("remove", "Remove", ButtonKind::Danger, !plan.pending_recovery, "Deletes the OmsiLaunch files listed by its own package manifest.")],
                )
            }
            Flow::Removing => (tr("Remove OmsiLaunch"), vec![(tr("Removing…"), 13.0, TEXT_DIM, false)], vec![]),
            Flow::Failed(e) => (
                tr("OmsiLaunch"),
                vec![(e.clone(), 13.0, DANGER, false)],
                vec![("close", "Close", ButtonKind::Primary, true, "Closes this window.")],
            ),
            Flow::Idle => (String::new(), vec![], vec![]),
        }
    }
}

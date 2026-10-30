//! The Mods page: installing a mod, the installs and the content folder's mods.

use super::super::domain::frame::Frame;
use super::super::interface::output::{core, fmt_bytes, kit, mobile, tr, Page};
use super::heading::{card_title, page_title, part_title};
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::theme::{accent, DANGER, EDGE, GAP, OK, TEXT, TEXT_DIM, TEXT_FAINT, TEXT_SOFT, WARN};
use omsi_ui::widgets::{id_of, ButtonKind};
use omsi_ui::{Color, Rect, Weight};

/// The Mods page: a mod's .zip installed (picked or dropped on the window), the installs
/// running and done, and the content folder's mods.
pub fn mods(f: &mut Frame, area: Rect) {
    if !f.state.mods_asked {
        f.state.load_mods();
    }
    let body = page_title(f.ui, area, Page::Mods);
    let phone = mobile();
    // (three columns: installing, the installs, the content folder - on a phone of one width)
    let (w0, w2) = if phone {
        let cw = (body.w - GAP * 4.0) / 3.0;
        (cw, cw)
    } else {
        (420.0f32.min(body.w * 0.36), 340.0f32.min(body.w * 0.28))
    };
    let c0 = Rect::new(body.x, body.y, w0, body.h);
    let c2 = Rect::new(body.right() - w2, body.y, w2, body.h);
    let c1 = Rect::new(
        c0.right() + GAP * 2.0,
        body.y,
        c2.x - GAP * 2.0 - (c0.right() + GAP * 2.0),
        body.h,
    );
    // install
    f.ui.panel(c0);
    let inner = card_title(
        f.ui,
        Rect::new(c0.x + 18.0, c0.y + 14.0, c0.w - 36.0, c0.h - 28.0),
        "Install a mod",
    );
    let mut y = inner.y;
    // (a .zip only, everywhere: loose files are copied by hand, see below)
    if f.ui.button(
        "mod-zip",
        Rect::new(inner.x, y, inner.w, 42.0),
        "Choose a .zip file",
        Some("o-package"),
        ButtonKind::Primary,
    ) {
        if let Some(p) = core::pick_mod() {
            f.state.install(p.to_string_lossy().to_string());
        }
    }
    y += 54.0;
    let drop = Rect::new(inner.x, y, inner.w, 130.0);
    let hot = f.drop_hover;
    let t = f.ui.anim(id_of("drop"), if hot { 1.0 } else { 0.0 }, 0.1);
    f.ui.p().rounded(drop, 8.0, accent().alpha(0.08 + 0.12 * t));
    // a dashed edge
    let per = 2.0 * (drop.w + drop.h);
    let n = (per / 9.0) as usize;
    for k in 0..n {
        let s = k as f32 * per / n as f32;
        let (p, horizontal) = if s < drop.w {
            (Vec2::new(drop.x + s, drop.y), true)
        } else if s < drop.w + drop.h {
            (Vec2::new(drop.right(), drop.y + s - drop.w), false)
        } else if s < 2.0 * drop.w + drop.h {
            (
                Vec2::new(drop.right() - (s - drop.w - drop.h), drop.bottom()),
                true,
            )
        } else {
            (
                Vec2::new(drop.x, drop.bottom() - (s - 2.0 * drop.w - drop.h)),
                false,
            )
        };
        let dash = if horizontal {
            Rect::new(p.x - 2.5, p.y - 0.75, 5.0, 1.5)
        } else {
            Rect::new(p.x - 0.75, p.y - 2.5, 1.5, 5.0)
        };
        f.ui.p().rect(dash, accent().alpha(0.45 + 0.45 * t));
    }
    f.ui.icon(
        "o-plus",
        Vec2::new(drop.center().x, drop.center().y - 12.0),
        20.0,
        TEXT_SOFT,
    );
    f.ui.text_in(
        "…or drop a .zip onto this window",
        Rect::new(drop.x, drop.center().y + 4.0, drop.w, 24.0),
        13.0,
        Weight::Regular,
        TEXT_SOFT,
        Align::Center,
    );
    y += 142.0;
    if !f.state.mod_path.is_empty() {
        let p = f.state.mod_path.clone();
        y += f.ui.paragraph(
            &p,
            Vec2::new(inner.x, y),
            inner.w,
            11.5,
            Weight::Regular,
            TEXT_FAINT,
        );
        match f.state.mod_info.clone() {
            Some(Ok(i)) if i.is_archive => {
                let place = if i.in_place_ok {
                    tr("can be used in place").into_owned()
                } else {
                    i.in_place.clone()
                };
                y += f.ui.paragraph(
                    &tr("%{size} archive, %{n} files - %{place}")
                        .replace("%{size}", &fmt_bytes(i.archive_bytes))
                        .replace("%{n}", &i.files.to_string())
                        .replace("%{place}", &place),
                    Vec2::new(inner.x, y),
                    inner.w,
                    12.0,
                    Weight::Regular,
                    if i.in_place_ok { TEXT_DIM } else { WARN },
                );
            }
            Some(Err(e)) => {
                y += f.ui.paragraph(
                    &e,
                    Vec2::new(inner.x, y),
                    inner.w,
                    12.0,
                    Weight::Regular,
                    DANGER,
                );
            }
            _ => {}
        }
        y += 6.0;
    }
    y += f.ui.paragraph(
        "The .zip is kept as it is in the Archives folder and read in place; nothing is unpacked.",
        Vec2::new(inner.x, y),
        inner.w,
        13.0,
        Weight::Regular,
        TEXT_DIM,
    );
    let mods = f.state.mods.clone();
    if let Some(m) = &mods {
        y += 4.0 + f.ui.paragraph(
            &tr("A .zip put into %{folder} is installed by itself once it has finished copying.")
                .replace("%{folder}", &m.inbox.to_string()),
            Vec2::new(inner.x, y + 4.0),
            inner.w,
            12.0,
            Weight::Regular,
            TEXT_FAINT,
        );
        if !m.inbox_items.is_empty() {
            // (a long name with no spaces is cut to the card, not run past it)
            let now: Vec<String> = m
                .inbox_items
                .iter()
                .map(|n| kit::fit_text(f.ui, n, 12.0, inner.w - 80.0))
                .collect();
            let now = now.join(", ");
            y += 4.0
                + f.ui.paragraph(
                    &tr("In it now: %{list}").replace("%{list}", &now),
                    Vec2::new(inner.x, y + 4.0),
                    inner.w,
                    12.0,
                    Weight::Regular,
                    TEXT_SOFT,
                );
        }
    }
    y += 18.0;
    part_title(
        f.ui,
        Rect::new(inner.x, y, inner.w, 28.0),
        "Loose files (not a .zip)",
    );
    y += 32.0;
    y += f.ui.paragraph("The launcher does not install loose folders. If you want to install them, copy them yourself:", Vec2::new(inner.x, y), inner.w, 13.0, Weight::Regular, TEXT_SOFT) + 10.0;
    let content = mods
        .as_ref()
        .map(|m| m.content_dir.clone())
        .unwrap_or_default();
    let places = [
        (
            "mods-open-root",
            "In your original OMSI 2 folder",
            "The mod works in OMSI 2 and in the Depot.",
            TEXT_DIM,
            f.state.config.root.clone(),
        ),
        (
            "mods-open-content",
            "In the Depot content folder",
            "The mod will not be available in OMSI 2.",
            WARN,
            content,
        ),
    ];
    for (id, title, note, note_c, folder) in places {
        let r = Rect::new(inner.x, y, inner.w, 50.0);
        f.ui.p().rounded(r, 6.0, Color::WHITE.alpha(0.04));
        f.ui.icon(
            "o-folder",
            Vec2::new(r.x + 20.0, r.center().y),
            18.0,
            TEXT_SOFT,
        );
        let bw = 104.0;
        let tw = r.w - 40.0 - bw - 20.0;
        f.ui.text_in_tip(
            title,
            Rect::new(r.x + 40.0, r.y + 7.0, tw, 20.0),
            13.5,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        f.ui.text_in_tip(
            note,
            Rect::new(r.x + 40.0, r.y + 26.0, tw, 18.0),
            12.0,
            Weight::Regular,
            note_c,
            Align::Left,
        );
        let b = Rect::new(r.right() - 10.0 - bw, r.y + 11.0, bw, 28.0);
        if f.ui.button(id, b, "Open folder", None, ButtonKind::Normal) {
            if folder.trim().is_empty() || !core::open_folder(std::path::Path::new(&folder)) {
                let msg = if id == "mods-open-root" {
                    "Choose the OMSI 2 folder under Setup first."
                } else {
                    "The content folder is not there yet."
                };
                f.state.set_status(msg, true);
            }
        }
        y += 60.0;
    }
    // installs
    f.ui.panel(c1);
    let inner = card_title(
        f.ui,
        Rect::new(c1.x + 18.0, c1.y + 14.0, c1.w - 36.0, c1.h - 28.0),
        "Installs",
    );
    if f.ui.button(
        "jobs-clear",
        Rect::new(c1.right() - 18.0 - 120.0, c1.y + 12.0, 120.0, 28.0),
        "Clear finished",
        None,
        ButtonKind::Normal,
    ) {
        core::install::clear_finished();
        f.state.poll_now();
    }
    let jobs = f.state.jobs.clone();
    let mut cancel = None;
    f.ui.scroll_area("jobs", Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h), &mut |ui, v| {
        if jobs.is_empty() {
            ui.paragraph("Nothing installed since the launcher started. A .zip is checked before it is used; a cancelled or failed install leaves nothing behind.", Vec2::new(v.x + 6.0, v.y), v.w - 12.0, 12.5, Weight::Regular, TEXT_DIM);
            return 60.0;
        }
        let mut y = v.y;
        for j in &jobs {
            let running = j.finished.is_none();
            let msg_h = ui.paragraph_height(&j.message, v.w - 40.0, 12.0, Weight::Regular);
            let h = 50.0 + msg_h + if running { 44.0 } else { 0.0 } + j.warnings.len() as f32 * 18.0;
            let r = Rect::new(v.x + 6.0, y, v.w - 16.0, h);
            ui.p().rounded(r, 10.0, Color::WHITE.alpha(0.04));
            ui.text_in(&j.name, Rect::new(r.x + 12.0, r.y + 8.0, r.w - 120.0, 20.0), 13.5, Weight::Bold, TEXT, Align::Left);
            let sc = match j.state.as_str() {
                "done" => OK,
                "failed" => DANGER,
                "cancelled" => TEXT_DIM,
                _ => accent(),
            };
            ui.badge(Vec2::new(r.right() - 90.0, r.y + 10.0), &j.state.to_uppercase(), sc);
            let mut yy = r.y + 34.0;
            if running {
                let frac = if j.bytes_total > 0 { j.bytes_done as f32 / j.bytes_total as f32 } else if j.files_total > 0 { j.files_done as f32 / j.files_total as f32 } else { 0.0 };
                ui.progress(Rect::new(r.x + 12.0, yy, r.w - 24.0, 8.0), frac, true);
                ui.text_in(&tr("%{done} / %{total} files · %{bytes} / %{size}").replace("%{done}", &j.files_done.to_string()).replace("%{total}", &j.files_total.to_string()).replace("%{bytes}", &fmt_bytes(j.bytes_done)).replace("%{size}", &fmt_bytes(j.bytes_total)), Rect::new(r.x + 12.0, yy + 10.0, r.w - 24.0, 16.0), 11.0, Weight::Regular, TEXT_DIM, Align::Left);
                yy += 30.0;
            }
            yy += ui.paragraph(&j.message, Vec2::new(r.x + 12.0, yy), r.w - 24.0, 12.0, Weight::Regular, if j.state == "failed" { DANGER } else { TEXT_SOFT });
            for w in &j.warnings {
                ui.text_in(&format!("⚠ {w}"), Rect::new(r.x + 12.0, yy, r.w - 24.0, 16.0), 11.0, Weight::Regular, WARN, Align::Left);
                yy += 18.0;
            }
            if running && ui.button(&format!("cancel-{}", j.id), Rect::new(r.x + 12.0, r.bottom() - 34.0, 100.0, 28.0), "Cancel", None, ButtonKind::Danger) {
                cancel = Some(j.id);
            }
            y += h + 10.0;
        }
        y - v.y
    });
    if let Some(id) = cancel {
        core::install::cancel(id);
        f.state.poll_now();
    }
    // content folder
    f.ui.panel(c2);
    let inner = card_title(
        f.ui,
        Rect::new(c2.x + 18.0, c2.y + 14.0, c2.w - 36.0, c2.h - 28.0),
        "Content folder",
    );
    let Some(m) = mods else {
        f.ui.text_in(
            "Reading…",
            Rect::new(inner.x, inner.y, inner.w, 20.0),
            12.5,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        return;
    };
    // (the free space beside the title, warm: what installs are checked against)
    f.ui.text_in(
        &tr("%{size} free").replace("%{size}", &fmt_bytes(m.free_bytes)),
        Rect::new(c2.x + 18.0, c2.y + 14.0, c2.w - 36.0, 24.0),
        13.0,
        Weight::Medium,
        WARN,
        Align::Right,
    );
    // (scrolled: a long list of archives or buses waiting would run past the card)
    f.ui.scroll_area(
        "mods-content",
        Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h),
        &mut |ui, v| {
            let (x, w) = (v.x + 6.0, v.w - 16.0);
            let mut y = v.y;
            y += ui.paragraph(
                &m.content_dir,
                Vec2::new(x, y),
                w,
                11.5,
                Weight::Regular,
                TEXT_FAINT,
            );
            y += 8.0;
            for (f, n) in &m.folders {
                ui.p().rect(Rect::new(x, y, w, 1.0), EDGE);
                ui.icon("o-folder", Vec2::new(x + 9.0, y + 16.0), 16.0, TEXT_SOFT);
                ui.text_in(
                    f,
                    Rect::new(x + 26.0, y + 5.0, w * 0.6, 22.0),
                    13.0,
                    Weight::Regular,
                    TEXT,
                    Align::Left,
                );
                ui.text_in(
                    &format!("{n} {}", if *n == 1 { "entry" } else { "entries" }),
                    Rect::new(x, y + 5.0, w, 22.0),
                    13.0,
                    Weight::Regular,
                    TEXT_DIM,
                    Align::Right,
                );
                y += 31.0;
            }
            if !m.archives.is_empty() {
                y += 8.0;
                part_title(ui, Rect::new(x, y, w, 28.0), "Archives used in place");
                y += 30.0;
                for (n, b) in &m.archives {
                    ui.text_in(
                        &format!("{n}  ({})", fmt_bytes(*b)),
                        Rect::new(x, y, w, 20.0),
                        12.0,
                        Weight::Regular,
                        TEXT_SOFT,
                        Align::Left,
                    );
                    y += 22.0;
                }
            }
            if !m.waiting.is_empty() {
                y += 8.0;
                part_title(
                    ui,
                    Rect::new(x, y, w, 28.0),
                    "Kept aside by an older version (copy by hand)",
                );
                y += 30.0;
                for item in &m.waiting {
                    ui.text_in(
                        item,
                        Rect::new(x, y, w, 20.0),
                        12.0,
                        Weight::Regular,
                        TEXT_DIM,
                        Align::Left,
                    );
                    y += 22.0;
                }
            }
            y - v.y
        },
    );
}

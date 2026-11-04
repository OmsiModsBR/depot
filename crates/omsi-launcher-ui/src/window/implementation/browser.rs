//! A phone's storage browser over the page: the places, the folder it is in and its folders,
//! and the folder chosen.

use super::super::domain::browser::{storage_roots, Browser, Purpose};
use super::super::domain::{frame::Frame, setup::SetupView};
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::theme::*;
use omsi_ui::widgets::ButtonKind;
use omsi_ui::{Rect, Weight};
use std::path::{Path, PathBuf};

/// The storage browser over the page, when it is open.
pub fn browser(f: &mut Frame, slot: &mut Option<Browser>, setup: &mut SetupView) {
    let Some(mut b) = slot.take() else {
        return;
    };
    let size = f.ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    f.ui.solid(full);
    f.ui.p().rect(full, omsi_ui::Color::rgba(0, 0, 0, 0.72));
    let r = Rect::new(24.0, 14.0, size.x - 48.0, size.y - 28.0);
    f.ui.panel(r);
    let inner = Rect::new(r.x + 16.0, r.y + 12.0, r.w - 32.0, r.h - 24.0);
    f.ui.text_in(
        b.title(),
        Rect::new(inner.x, inner.y, inner.w - 130.0, 26.0),
        17.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    let mut close = f.ui.button(
        "browse-cancel",
        Rect::new(inner.right() - 120.0, inner.y - 2.0, 120.0, 34.0),
        "Cancel",
        Some("close"),
        ButtonKind::Ghost,
    );
    // the places, then the folder we are in
    let mut x = inner.x;
    let y = inner.y + 34.0;
    let mut go_to: Option<PathBuf> = None;
    let up = Rect::new(x, y, 44.0, 34.0);
    let up_clicked = f.ui.button(
        "browse-up",
        up,
        "",
        Some("drive_folder_upload"),
        ButtonKind::Normal,
    );
    f.ui.tooltip(up, "Up one folder");
    if up_clicked {
        if let Some(p) = b.dir.parent() {
            go_to = Some(p.to_path_buf());
        }
    }
    x += 52.0;
    for (k, (name, path)) in storage_roots().into_iter().enumerate() {
        let w = f.ui.width(&name, 13.0, Weight::Medium) + 48.0;
        if x + w > inner.right() {
            break;
        }
        if f.ui.button(
            &format!("browse-root-{k}"),
            Rect::new(x, y, w, 34.0),
            &name,
            Some("sd_card"),
            ButtonKind::Normal,
        ) {
            go_to = Some(path);
        }
        x += w + 8.0;
    }
    let path_y = y + 42.0;
    f.ui.icon(
        "folder_open",
        Vec2::new(inner.x + 10.0, path_y + 11.0),
        16.0,
        TEXT_DIM,
    );
    let shown = f.ui.fonts.fit(
        &b.dir.to_string_lossy(),
        12.5,
        Weight::Medium,
        inner.w - 30.0,
    );
    f.ui.text_in(
        &shown,
        Rect::new(inner.x + 26.0, path_y, inner.w - 26.0, 22.0),
        12.5,
        Weight::Medium,
        TEXT_SOFT,
        Align::Left,
    );
    // what to do with this folder
    let foot_h = 46.0;
    let foot = Rect::new(
        inner.x,
        inner.bottom() - foot_h + 6.0,
        inner.w,
        foot_h - 6.0,
    );
    let mut chosen: Option<PathBuf> = None;
    match b.purpose {
        Purpose::Root => {
            let (text, c) = if b.is_root {
                ("A complete OMSI 2 installation", OK)
            } else {
                (
                    "Not an OMSI 2 folder (it needs Omsi.exe, maps and Vehicles)",
                    TEXT_DIM,
                )
            };
            f.ui.text_in(
                text,
                Rect::new(foot.x, foot.y, foot.w - 230.0, foot.h),
                12.5,
                Weight::Medium,
                c,
                Align::Left,
            );
            if f.ui.button(
                "browse-use",
                Rect::new(foot.right() - 220.0, foot.y, 220.0, foot.h),
                "Use this folder",
                Some("check"),
                if b.is_root {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Normal
                },
            ) {
                chosen = Some(b.dir.clone());
            }
        }
    }
    // the folder's contents
    let list = Rect::new(inner.x, path_y + 28.0, inner.w, foot.y - path_y - 36.0);
    f.ui.p().rounded(list, 6.0, FIELD);
    if let Some(e) = b.error.clone() {
        f.ui.paragraph(
            &e,
            Vec2::new(list.x + 12.0, list.y + 12.0),
            list.w - 24.0,
            13.0,
            Weight::Regular,
            DANGER,
        );
    } else if b.entries.is_empty() {
        f.ui.text_in(
            "Nothing here",
            list,
            13.0,
            Weight::Regular,
            TEXT_FAINT,
            Align::Center,
        );
    }
    let entries = b.entries.clone();
    let mut picked: Option<(String, bool)> = None;
    let key = format!("browse-list-{}", b.dir.display());
    f.ui.scroll_area(&key, list, &mut |ui, area| {
        let row_h = 40.0;
        for (k, (name, dir, bytes)) in entries.iter().enumerate() {
            let rr = Rect::new(
                area.x + 4.0,
                area.y + 4.0 + k as f32 * row_h,
                area.w - 12.0,
                row_h - 2.0,
            );
            if rr.bottom() < list.y || rr.y > list.bottom() {
                continue;
            }
            if ui.row(&format!("{key}-{k}"), rr, false) {
                picked = Some((name.clone(), *dir));
            }
            ui.icon(
                if *dir { "folder" } else { "inventory_2" },
                Vec2::new(rr.x + 18.0, rr.center().y),
                20.0,
                if *dir { accent() } else { TEXT_SOFT },
            );
            ui.text_in(
                name,
                Rect::new(rr.x + 40.0, rr.y, rr.w - 140.0, rr.h),
                13.5,
                Weight::Medium,
                TEXT,
                Align::Left,
            );
            if !*dir {
                ui.text_in(
                    &super::super::interface::output::fmt_bytes(*bytes),
                    Rect::new(rr.x, rr.y, rr.w - 12.0, rr.h),
                    12.0,
                    Weight::Regular,
                    TEXT_DIM,
                    Align::Right,
                );
            }
        }
        8.0 + entries.len() as f32 * row_h
    });
    if let Some((name, dir)) = picked {
        let p = b.dir.join(&name);
        if dir {
            go_to = Some(p);
        } else {
            chosen = Some(p);
        }
    }
    if let Some(d) = go_to {
        b.open(d);
    }
    if f.ui.input.keys.contains(&omsi_ui::widgets::Key::Escape) {
        close = true;
    }
    if let Some(p) = chosen {
        chose(f, setup, b.purpose, &p);
        close = true;
    }
    if !close {
        *slot = Some(b);
    }
}

fn chose(f: &mut Frame, setup: &mut SetupView, purpose: Purpose, p: &Path) {
    let s = p.to_string_lossy().to_string();
    match purpose {
        Purpose::Root => {
            setup.root = Some(s);
            f.state.set_status("Folder chosen: press Save.", false);
        }
    }
}

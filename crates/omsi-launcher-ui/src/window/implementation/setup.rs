//! The Setup page: where the original OMSI 2 is, where this game's program is and where the
//! Depot keeps its content (Browse each, Save or Revert at the foot), the Depot's own files,
//! and what the content holds - what was found, and what it takes on disk.

use super::super::domain::{frame::Frame, setup::SetupView};
use super::super::interface::output::{available, core, fmt_bytes, kit, mobile, tr, Page, Squeeze};
use super::engine::engine_card;
use super::heading::{card_title, page_title};
use crate::kit::Status;
use glam::Vec2;
use omsi_launcher_lib::disk::{self, Node};
use omsi_ui::paint::Align;
use omsi_ui::theme::{
    accent, CONTROL_RADIUS, DANGER, EDGE, FIELD, GAP, OK, PANEL, TEXT, TEXT_DIM, TEXT_FAINT,
    TEXT_SOFT, WARN,
};
use omsi_ui::widgets::{id_of, ButtonKind, Ui};
use omsi_ui::{Color, Rect, Weight};

const FIELDS: [&str; 3] = ["root", "game", "content"];

/// A folder's line under its field: what was found there.
/// `version` is the running game's version (not this crate's own package number).
pub fn check(field: usize, value: &str, root: &str, version: &str) -> (Status, String) {
    let v = value.trim();
    let p = std::path::Path::new(v);
    match field {
        0 if v.is_empty() => (
            Status::Missing,
            "Not chosen yet: the folder with maps and Vehicles in it.".into(),
        ),
        0 => {
            let missing = omsi_cfg::missing_original_essentials(p);
            if missing.is_empty() {
                (
                    Status::Found,
                    "The essential files are there. Read only: the launcher never writes to it."
                        .into(),
                )
            } else {
                (
                    Status::Error,
                    tr("Not a complete OMSI 2 installation: it lacks %{files}.")
                        .replace("%{files}", &lacking(&missing, 4)),
                )
            }
        }
        1 if v.is_empty() => (
            Status::Warning,
            "Not found: choose the depot program.".into(),
        ),
        1 if p.is_file() => {
            let beside = std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(|d| d.to_path_buf()));
            if beside.is_some_and(|d| p.parent() == Some(d.as_path())) {
                (
                    Status::Found,
                    tr("Found next to the launcher. Version %{version}")
                        .replace("%{version}", version),
                )
            } else {
                (Status::Found, "Found.".into())
            }
        }
        1 => (Status::Error, "There is no program at this path.".into()),
        // (the content folder: nothing is written while it is typed - Save makes it and
        // tries it)
        // (`OMSI_CONTENT` wins over what is chosen here)
        _ if omsi_cfg::flags::CONTENT.os().is_some() => (
            Status::Warning,
            "Set by OMSI_CONTENT: what is chosen here is used once it is no longer set.".into(),
        ),
        _ if v.is_empty() => (
            Status::Found,
            "Beside the game program. Mods put here are not available in OMSI 2.".into(),
        ),
        _ if p.is_file() => (Status::Error, "This is a file, not a folder.".into()),
        _ => match core::content_choice_problem(p, std::path::Path::new(root.trim())) {
            Some(why) => (
                Status::Error,
                tr("Cannot be used: %{why}.").replace("%{why}", &why_text(&why)),
            ),
            None if !p.exists() => (
                Status::Warning,
                "It does not exist yet: it is made when you save.".into(),
            ),
            None => (
                Status::Found,
                "Mods put here are not available in OMSI 2.".into(),
            ),
        },
    }
}

/// What an OMSI 2 folder lacks, as its lines say it: the first `n` files, and how many more.
fn lacking(missing: &[String], n: usize) -> String {
    let shown = missing
        .iter()
        .take(n)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if missing.len() > n {
        tr("%{files} and %{n} more")
            .replace("%{files}", &shown)
            .replace("%{n}", &(missing.len() - n).to_string())
    } else {
        shown
    }
}

/// Why a content folder cannot be used (`core::content_folder_problem`, in English with the
/// folder in it) in the interface's language.
fn why_text(why: &str) -> String {
    let templates = [
        "%{path} is not a full path (from the drive, or from /)",
        "it is in an OMSI 2 installation (%{path}), which the Depot never writes to",
        "%{path} cannot be made or written to",
    ];
    for t in templates {
        let (head, tail) = t.split_once("%{path}").unwrap_or((t, ""));
        if let Some(path) = why.strip_prefix(head).and_then(|x| x.strip_suffix(tail)) {
            return tr(t).replace("%{path}", path);
        }
    }
    tr(why).into_owned()
}

/// The colour of a type of content (`disk::TYPES`), in the chart and its legend.
pub fn type_color(name: &str) -> Color {
    match name {
        "Vehicles" => Color::hex(0xe8a33d),
        "Sceneryobjects" => Color::hex(0x4c8ddb),
        "Texture" => Color::hex(0xa27bdb),
        "Maps" => Color::hex(0x4cb86a),
        "Sounds" => Color::hex(0xe0605a),
        "Splines" => Color::hex(0x2aa7a0),
        "Archives" => Color::hex(0xd46fa8),
        _ => Color::hex(0x8a8f96),
    }
}

pub fn setup(f: &mut Frame, view: &mut SetupView, area: Rect) {
    let mut body = page_title(f.ui, area, Page::Setup);
    let phone = mobile();
    // (the first run, or a folder gone: what to do, over the page, until a complete OMSI 2
    // folder is saved)
    if !phone && !f.root.ok(&f.state.config.root) {
        let message = "The launcher reads the maps and vehicles of your original OMSI 2. Choose its folder (the one with Omsi.exe, maps and Vehicles in it) with Browse, then press Save.";
        let h = kit::info_bar_height(f.ui, body.w, message);
        kit::info_bar(
            f.ui,
            "setup:first-run",
            Rect::new(body.x, body.y, body.w, h),
            accent(),
            "info",
            "Choose your OMSI 2 folder",
            message,
        );
        body = Rect::new(body.x, body.y + h + GAP, body.w, body.h - h - GAP);
    }
    if !f.state.usage_asked && !phone {
        f.state.usage_asked = true;
        f.state.load_usage();
    }
    if view.content_now.is_none() {
        view.content_now = Some(
            core::content_dir()
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_default(),
        );
    }
    let foot_h = 60.0;
    let main = Rect::new(body.x, body.y, body.w, body.h - foot_h - GAP);
    let right_w = if phone {
        0.0
    } else {
        (body.w * 0.3).clamp(300.0, 420.0)
    };
    let left = Rect::new(
        main.x,
        main.y,
        if phone {
            main.w
        } else {
            main.w - right_w - GAP
        },
        main.h,
    );
    // (the left column scrolls: on a short window the Depot's own files used to sit under the
    // foot, out of reach)
    let off =
        f.ui.scroll
            .get(&id_of("setup-left"))
            .copied()
            .unwrap_or(0.0);
    let top = left.y - off;
    f.ui.push_clip(left, 0.0);
    let shifted = Rect::new(left.x, top, left.w, left.h);
    let mut y = folders(f, view, shifted);
    if !phone {
        if view.tab == 1 {
            y = free_space(f, Rect::new(left.x, y + GAP, left.w, 0.0));
        }
        // (Windows only: opening the original OMSI 2 through OmsiLaunch)
        if available() {
            y = engine_card(f, Rect::new(left.x, y + GAP, left.w, 0.0));
        }
        own_files(f, Rect::new(left.x, y + GAP, left.w, 0.0));
        y += GAP + OWN_FILES_H;
    }
    f.ui.pop_clip();
    f.ui.scroll_keep("setup-left", left, y - top);
    if !phone {
        content(
            f,
            view,
            Rect::new(left.right() + GAP, main.y, right_w, main.h),
        );
    }
    foot(
        f,
        view,
        Rect::new(body.x, body.bottom() - foot_h, body.w, foot_h),
    );
}

/// A folder's field (label over it, Browse beside it, what was found under it). True when
/// Browse was pressed.
#[allow(clippy::too_many_arguments)]
fn folder_row(
    ui: &mut Ui,
    x: f32,
    y: f32,
    w: f32,
    label: &str,
    id: &str,
    value: &mut String,
    hint: &str,
    icon: &str,
    status: &(Status, String),
    primary: bool,
) -> (bool, bool) {
    ui.text_in(
        label,
        Rect::new(x, y, w, 18.0),
        12.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
    let changed = ui.text_input(
        &format!("cfg-{id}"),
        Rect::new(x, y + 20.0, w - 92.0, 34.0),
        value,
        hint,
        Some(icon),
    );
    let browse = ui.button(
        &format!("browse-{id}"),
        Rect::new(x + w - 82.0, y + 20.0, 82.0, 34.0),
        "Browse",
        None,
        if primary {
            ButtonKind::Primary
        } else {
            ButtonKind::Normal
        },
    );
    kit::dot(ui, x + 4.0, y + 68.0, status.0);
    let c = match status.0 {
        Status::Found => OK,
        Status::Warning => WARN,
        Status::Unverified => TEXT_DIM,
        Status::Missing | Status::Error => DANGER,
    };
    kit::clipped(
        ui,
        &status.1,
        x + 15.0,
        y + 68.0,
        w - 15.0,
        12.5,
        Weight::Regular,
        c,
    );
    (changed, browse)
}

/// The Folders card; returns its foot.
fn folders(f: &mut Frame, view: &mut SetupView, col: Rect) -> f32 {
    let phone = mobile();
    let in_process = core::IN_PROCESS_GAMES;
    let rows = if in_process { 1.0 } else { 3.0 };
    let extra = if in_process { 96.0 } else { 0.0 };
    let r = Rect::new(col.x, col.y, col.w, 46.0 + rows * 88.0 + extra);
    f.ui.panel(r);
    let inner = card_title(
        f.ui,
        Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0),
        "Folders",
    );
    let saved = [
        f.state.config.root.clone(),
        f.state.config.game.clone(),
        f.state.config.content.clone(),
    ];
    let mut values = [
        view.root.clone().unwrap_or_else(|| saved[0].clone()),
        view.game.clone().unwrap_or_else(|| saved[1].clone()),
        view.content.clone().unwrap_or_else(|| saved[2].clone()),
    ];
    for i in 0..3 {
        // (the content folder is checked again when the OMSI 2 folder changes too)
        let key = if i == 2 {
            format!("{}\u{0}{}", values[2], values[0])
        } else {
            values[i].clone()
        };
        if view.checks[i].1.is_none() || view.checks[i].0 != key {
            let out = check(i, &values[i], &values[0], f.host.version().0);
            view.checks[i] = (key, Some(out));
        }
    }
    let status = |v: &SetupView, f: usize| {
        v.checks[f]
            .1
            .clone()
            .unwrap_or((Status::Unverified, String::new()))
    };
    let content_hint = view.content_now.clone().unwrap_or_default();
    let first = !f.root.ok(&f.state.config.root);
    let mut y = inner.y;
    let list: &[(usize, &str, &str, &str)] = if in_process {
        &[(0, "OMSI 2 folder", "/path/to/OMSI 2", "o-folder")]
    } else {
        &[
            (0, "OMSI 2 folder", "D:\\OMSI 2", "o-folder"),
            (1, "Game program", "depot", "o-play"),
            (2, "Depot content folder", "", "o-folder"),
        ]
    };
    for &(i, label, hint, icon) in list {
        let hint = if i == 2 { content_hint.as_str() } else { hint };
        let st = status(view, i);
        let (changed, browse) = folder_row(
            f.ui,
            inner.x,
            y,
            inner.w,
            label,
            FIELDS[i],
            &mut values[i],
            hint,
            icon,
            &st,
            // (no complete folder yet: Browse is the way on, until a good one is chosen)
            i == 0 && first && st.0 != Status::Found,
        );
        if changed {
            set(view, i, values[i].clone());
        }
        if browse {
            match i {
                0 if phone || in_process => {
                    view.browse_root = Some(values[0].clone());
                }
                0 => {
                    if let Some(p) =
                        core::pick_folder("The OMSI 2 folder (with maps and Vehicles in it)")
                    {
                        set(view, 0, p.to_string_lossy().to_string());
                    }
                }
                1 => {
                    if let Some(p) = core::pick_file("The depot program") {
                        set(view, 1, p.to_string_lossy().to_string());
                    }
                }
                _ => {
                    if let Some(p) =
                        core::pick_folder("The Depot's content folder (mods are installed into it)")
                    {
                        set(view, 2, p.to_string_lossy().to_string());
                    }
                }
            }
        }
        y += 88.0;
    }
    if in_process {
        // (a phone: the game is this app itself)
        f.ui.paragraph("Copy the whole OMSI 2 folder (with maps and Vehicles in it) onto the phone - by cable, from a PC or a USB stick - for example as depot/OMSI 2 in the internal storage, then choose it here with Browse. A mod's .zip goes into depot/Mods.", Vec2::new(inner.x, y), inner.w, 12.5, Weight::Regular, TEXT_DIM);
    }
    r.bottom()
}

fn set(v: &mut SetupView, f: usize, s: String) {
    match f {
        0 => v.root = Some(s),
        1 => v.game = Some(s),
        _ => v.content = Some(s),
    }
}

/// The repositories to point to when the original OMSI 2 is compressed.
const OMSILAUNCH_URL: &str = "https://github.com/OmsiModsBR/OmsiLaunch";
const OMSI_PREFETCH_URL: &str = "https://github.com/OmsiModsBR/omsi-prefetch";

/// Text height (or, drawing, text) in a card: returns the height.
#[allow(clippy::too_many_arguments)]
fn para(
    ui: &mut Ui,
    draw: bool,
    text: &str,
    x: f32,
    y: f32,
    w: f32,
    px: f32,
    weight: Weight,
    c: Color,
) -> f32 {
    if draw {
        ui.paragraph(text, Vec2::new(x, y), w, px, weight, c)
    } else {
        ui.paragraph_height(text, w, px, weight)
    }
}

/// Why a folder cannot be compressed, as a sentence.
fn unsupported_text(u: &core::compress::Unsupported) -> String {
    use core::compress::Unsupported as U;
    match u {
        U::System => tr("Not available here: compression only works on Windows.").into(),
        U::Cloud => tr("Not available here: the folder is in a cloud-sync folder.").into(),
        U::NotFixed => tr("Not available here: the folder is not on a fixed local disk.").into(),
        U::NotNtfs(fs) => tr("Not available here: the disk is %{fs}, not NTFS.")
            .replace("%{fs}", if fs.is_empty() { "?" } else { fs }),
        U::Unreadable => tr("Not available here: the disk could not be read.").into(),
    }
}

/// The "Free up space" card (Disk usage): what the content takes, and compressing it with
/// NTFS (LZX) - the folders to do it in, an estimate of what it frees, the warnings that go
/// with it, and the job running in the background.
fn free_space(f: &mut Frame, at: Rect) -> f32 {
    if !f.state.squeeze.asked && f.state.squeeze.job.is_none() {
        f.state.load_squeeze();
    }
    let inner_w = at.w - 36.0;
    // (laid out once to know the card's height, then drawn)
    let h = 40.0 + squeeze_body(f, false, Rect::new(at.x + 18.0, at.y + 40.0, inner_w, 0.0)) + 14.0;
    let r = Rect::new(at.x, at.y, at.w, h);
    f.ui.panel(r);
    let inner = card_title(
        f.ui,
        Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0),
        "Free up space",
    );
    chip(f.ui, r.right() - 18.0, r.y + 22.0, "Experimental");
    squeeze_body(f, true, inner);
    r.bottom()
}

/// The card's contents from `inner` down; true `draw` paints and handles the controls, false
/// only measures. Returns the height used below `inner`'s top.
fn squeeze_body(f: &mut Frame, draw: bool, inner: Rect) -> f32 {
    use super::super::interface::output::Squeezed;
    use core::compress::Disk;
    use std::sync::atomic::Ordering::Relaxed;
    let x = inner.x;
    let w = inner.w;
    // (the title's own height is added for the measuring pass: the drawn inner starts below it)
    let top = inner.y;
    let mut y = top;
    let half = w * 0.5;
    let folders = f.state.squeeze.folders.clone().unwrap_or_default();
    let busy = f.state.squeeze.job.clone();
    let saved: u64 = folders.iter().map(|f| f.compressed.1).sum();
    let total = f
        .state
        .usage
        .as_ref()
        .map(|u| fmt_size(u.tree.bytes))
        .unwrap_or_else(|| "Counting…".into());
    let right = if busy.is_some() {
        tr("In progress").to_string()
    } else if saved > 0 {
        tr("%{size} saved").replace("%{size}", &fmt_size(saved))
    } else if folders.iter().all(|f| f.disk.is_err()) && f.state.squeeze.folders.is_some() {
        tr("Not available").to_string()
    } else {
        tr("Nothing compressed").to_string()
    };
    if draw {
        f.ui.text_in(
            "Content found (both folders)",
            Rect::new(x, y, half, 18.0),
            12.5,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        f.ui.text_in(
            &total,
            Rect::new(x, y + 18.0, half, 30.0),
            22.0,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        f.ui.text_in(
            "Compression with NTFS (LZX)",
            Rect::new(x + half, y, half, 18.0),
            12.5,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        f.ui.text_in(
            &right,
            Rect::new(x + half, y + 18.0, half, 30.0),
            22.0,
            Weight::Bold,
            if saved > 0 { OK } else { TEXT },
            Align::Left,
        );
    }
    y += 60.0;

    // the folders: a switch each, and what is known of it
    let mut any_chosen = false;
    let mut any_undo = false;
    let mut files = 0usize;
    let mut freed = 0u64;
    let mut hdd = false;
    let mut original = false;
    let mut need = 0u64;
    let mut least_free: Option<u64> = None;
    for folder in folders.iter() {
        let i = Squeeze::index(folder.name);
        let fits = folder.disk.is_ok();
        if fits {
            let mut on = f.state.squeeze.chosen[i];
            if draw {
                let locked = busy.is_some() || f.state.squeeze.confirm.is_some();
                let mut v = on;
                if f.ui.toggle(
                    &format!("squeeze-{i}"),
                    Rect::new(x, y, w, 30.0),
                    &mut v,
                    folder.name,
                ) && !locked
                {
                    f.state.squeeze.chosen[i] = v;
                    on = v;
                }
            }
            if on {
                any_chosen = true;
                files += folder.estimate.files;
                freed += folder.estimate.saved;
                hdd |= folder.disk == Ok(Disk::Hdd);
                original |= i == 1;
                if folder.compressed.0 > 0 {
                    any_undo = true;
                    need += folder.compressed.1;
                    least_free = match (least_free, folder.free) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (a, b) => a.or(b),
                    };
                }
            }
            y += 34.0;
            let line = if folder.estimate.files == 0 {
                tr("No files to compress here.").to_string()
            } else {
                tr("%{files} files can be compressed (%{size}); about %{saved} could be freed (estimate).")
                    .replace("%{files}", &folder.estimate.files.to_string())
                    .replace("%{size}", &fmt_size(folder.estimate.bytes))
                    .replace("%{saved}", &fmt_size(folder.estimate.saved))
            };
            y += para(f.ui, draw, &line, x, y, w, 12.0, Weight::Regular, TEXT_DIM) + 2.0;
            if folder.compressed.0 > 0 {
                let line = tr("Compressed now: %{files} files, %{size} saved.")
                    .replace("%{files}", &folder.compressed.0.to_string())
                    .replace("%{size}", &fmt_size(folder.compressed.1));
                y += para(f.ui, draw, &line, x, y, w, 12.0, Weight::Regular, OK) + 2.0;
            }
            y += 8.0;
        } else if let Err(why) = &folder.disk {
            if draw {
                f.ui.text_in(
                    folder.name,
                    Rect::new(x, y, w, 20.0),
                    13.0,
                    Weight::Medium,
                    TEXT_DIM,
                    Align::Left,
                );
            }
            y += 22.0;
            y += para(
                f.ui,
                draw,
                &unsupported_text(why),
                x,
                y,
                w,
                12.0,
                Weight::Regular,
                WARN,
            ) + 10.0;
        }
    }
    // (the files take their full size again: the disk with the least room must hold the sum)
    let low_space = least_free.is_some_and(|free| free < need);
    y += para(
        f.ui,
        draw,
        "Files the game writes, archives (.zip) and media that are already compressed are left as they are.",
        x,
        y,
        w,
        12.0,
        Weight::Regular,
        TEXT_FAINT,
    ) + 10.0;

    // what comes next: the job, the confirmation, or the buttons
    if let Some((job, undo)) = busy {
        let (done, tot) = (job.done.load(Relaxed), job.total.load(Relaxed));
        let t = tr(if undo {
            "Undoing the compression… %{done} of %{total}"
        } else {
            "Compressing… %{done} of %{total}"
        })
        .replace("%{done}", &fmt_size(done))
        .replace("%{total}", &fmt_size(tot));
        y += para(f.ui, draw, &t, x, y, w, 13.0, Weight::Medium, TEXT) + 6.0;
        if draw {
            f.ui.progress(
                Rect::new(x, y, w, 8.0),
                if tot > 0 {
                    done as f32 / tot as f32
                } else {
                    0.0
                },
                true,
            );
        }
        y += 18.0;
        if draw {
            let cancelling = job.cancel.load(Relaxed);
            if cancelling {
                dim_button(f.ui, Rect::new(x, y, 160.0, 34.0), "Cancel");
            } else if f.ui.button(
                "squeeze-cancel",
                Rect::new(x, y, 160.0, 34.0),
                "Cancel",
                None,
                ButtonKind::Normal,
            ) {
                job.cancel.store(true, Relaxed);
            }
        }
        return y + 34.0 - top;
    }
    if let Some(undo) = f.state.squeeze.confirm {
        if undo {
            let t = tr("Undo the compression in the chosen folders? Their files will take their full size on disk again: %{need} are needed, and the disk has %{free} free.")
                .replace("%{need}", &fmt_size(need))
                .replace(
                    "%{free}",
                    &least_free.map(fmt_size).unwrap_or_else(|| "?".into()),
                );
            y += para(f.ui, draw, &t, x, y, w, 13.0, Weight::Regular, TEXT) + 8.0;
            if low_space {
                y += para(
                    f.ui,
                    draw,
                    "There is not enough free space on the disk to undo it.",
                    x,
                    y,
                    w,
                    13.0,
                    Weight::Medium,
                    DANGER,
                ) + 8.0;
            }
        } else {
            let t = tr("Compress %{files} files in the chosen folders? About %{saved} could be freed. This is an estimate from a sample; the real result will differ, and you can undo it.")
                .replace("%{files}", &files.to_string())
                .replace("%{saved}", &fmt_size(freed));
            y += para(f.ui, draw, &t, x, y, w, 13.0, Weight::Regular, TEXT) + 8.0;
            if original {
                y += para(
                    f.ui,
                    draw,
                    "Strong recommendation for the original OMSI 2 folder: play it through OmsiLaunch with the Omsi Prefetch module. OMSI 2 makes tens of millions of tiny reads, and compressed files make each one cost more; Omsi Prefetch keeps the files in memory and answers them from larger blocks. Without it, loading can become much slower. It needs about 4 GB of free memory.",
                    x,
                    y,
                    w,
                    12.5,
                    Weight::Regular,
                    WARN,
                ) + 8.0;
                if draw {
                    if f.ui.button(
                        "squeeze-link-launch",
                        Rect::new(x, y, 190.0, 32.0),
                        "Open the OmsiLaunch page",
                        None,
                        ButtonKind::Normal,
                    ) {
                        f.host.open_url(OMSILAUNCH_URL);
                    }
                    if f.ui.button(
                        "squeeze-link-prefetch",
                        Rect::new(x + 200.0, y, 210.0, 32.0),
                        "Open the Omsi Prefetch page",
                        None,
                        ButtonKind::Normal,
                    ) {
                        f.host.open_url(OMSI_PREFETCH_URL);
                    }
                }
                y += 32.0 + 10.0;
            }
            if hdd {
                y += para(
                    f.ui,
                    draw,
                    "A chosen folder is on a hard disk, or on a disk that could not be identified. Compressed files can load much more slowly from a hard disk.",
                    x,
                    y,
                    w,
                    12.5,
                    Weight::Regular,
                    DANGER,
                ) + 8.0;
                if draw {
                    let mut v = f.state.squeeze.hdd_ok;
                    if f.ui.toggle(
                        "squeeze-hdd",
                        Rect::new(x, y, w, 30.0),
                        &mut v,
                        "I understand it can be much slower on a hard disk",
                    ) {
                        f.state.squeeze.hdd_ok = v;
                    }
                }
                y += 38.0;
            }
        }
        if draw {
            let go = if undo {
                !low_space
            } else {
                any_chosen && files > 0 && (!hdd || f.state.squeeze.hdd_ok)
            };
            let label = if undo {
                "Undo compression"
            } else {
                "Start compression"
            };
            if go {
                if f.ui.button(
                    "squeeze-go",
                    Rect::new(x, y, 190.0, 34.0),
                    label,
                    None,
                    ButtonKind::Primary,
                ) {
                    f.state.start_squeeze(undo);
                    f.state.squeeze.confirm = None;
                }
            } else {
                dim_button(f.ui, Rect::new(x, y, 190.0, 34.0), label);
            }
            if f.ui.button(
                "squeeze-back",
                Rect::new(x + 200.0, y, 120.0, 34.0),
                "Cancel",
                None,
                ButtonKind::Normal,
            ) {
                f.state.squeeze.confirm = None;
                f.state.squeeze.hdd_ok = false;
            }
        }
        return y + 34.0 - top;
    }
    if draw {
        let can = any_chosen && files > 0;
        if can {
            if f.ui.button(
                "squeeze-ask",
                Rect::new(x, y, 250.0, 34.0),
                "Compress with NTFS (LZX)...",
                None,
                ButtonKind::Normal,
            ) {
                f.state.squeeze.confirm = Some(false);
                f.state.squeeze.hdd_ok = false;
            }
        } else {
            dim_button(
                f.ui,
                Rect::new(x, y, 250.0, 34.0),
                "Compress with NTFS (LZX)...",
            );
        }
        if any_undo {
            if f.ui.button(
                "squeeze-undo",
                Rect::new(x + 260.0, y, 190.0, 34.0),
                "Undo compression",
                None,
                ButtonKind::Normal,
            ) {
                f.state.squeeze.confirm = Some(true);
            }
        } else {
            dim_button(
                f.ui,
                Rect::new(x + 260.0, y, 190.0, 34.0),
                "Undo compression",
            );
        }
    }
    y += 34.0 + 10.0;
    if let Some(done) = f.state.squeeze.done.clone() {
        let (t, c) = match done {
            Squeezed::Done(0) => (tr("Done.").to_string(), OK),
            Squeezed::Done(n) => (
                tr("Done. %{n} files were left as they were (in use, not allowed, or not worth compressing).")
                    .replace("%{n}", &n.to_string()),
                TEXT_DIM,
            ),
            Squeezed::Cancelled => (
                tr("Stopped. What was already compressed stays compressed; use Undo compression to restore it.").to_string(),
                WARN,
            ),
            Squeezed::Error(e) => (
                tr("It could not be done: %{why}").replace("%{why}", &e),
                DANGER,
            ),
        };
        y += para(f.ui, draw, &t, x, y, w, 12.5, Weight::Regular, c) + 4.0;
    }
    y - top
}

/// A button that cannot be pressed.
fn dim_button(ui: &mut Ui, r: Rect, label: &str) {
    ui.p().rounded(r, CONTROL_RADIUS, FIELD);
    ui.p().rounded_border(r, CONTROL_RADIUS, 1.0, EDGE);
    ui.text_in(label, r, 13.0, Weight::Medium, TEXT_FAINT, Align::Center);
}

/// A small outlined label in the accent colour, its right edge at `right`.
fn chip(ui: &mut Ui, right: f32, cy: f32, label: &str) {
    let w = ui.width(label, 12.0, Weight::Medium) + 20.0;
    let r = Rect::new(right - w, cy - 11.0, w, 22.0);
    ui.p().rounded(r, 11.0, accent().alpha(0.12));
    ui.p().rounded_border(r, 11.0, 1.0, accent().alpha(0.5));
    ui.text_in(label, r, 12.0, Weight::Medium, accent(), Align::Center);
}

/// The Depot's own files: its folder (logs, previews, settings), and a button to open it.
/// The height of the card of the Depot's own files.
const OWN_FILES_H: f32 = 128.0;

fn own_files(f: &mut Frame, at: Rect) {
    let r = Rect::new(at.x, at.y, at.w, OWN_FILES_H);
    f.ui.panel(r);
    let inner = card_title(
        f.ui,
        Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0),
        "The Depot's own files",
    );
    kit::right(
        f.ui,
        "logs, previews and settings",
        r.right() - 18.0,
        r.y + 22.0,
        12.0,
        Weight::Regular,
        TEXT_DIM,
    );
    let dir = core::data_dir();
    let field = Rect::new(inner.x, inner.y, inner.w - 132.0, 34.0);
    f.ui.p().rounded(field, CONTROL_RADIUS, FIELD);
    f.ui.p().rounded_border(field, CONTROL_RADIUS, 1.0, EDGE);
    f.ui.icon(
        "o-folder",
        Vec2::new(field.x + 18.0, field.center().y),
        15.0,
        TEXT_DIM,
    );
    kit::clipped(
        f.ui,
        &dir.to_string_lossy(),
        field.x + 34.0,
        field.center().y,
        field.w - 44.0,
        13.5,
        Weight::Regular,
        TEXT,
    );
    if f.ui.button(
        "open-depot-folder",
        Rect::new(inner.right() - 122.0, inner.y, 122.0, 34.0),
        "Open folder",
        None,
        ButtonKind::Normal,
    ) && !core::open_folder(&dir)
    {
        f.state.set_status(
            tr("The folder could not be opened: %{path}")
                .replace("%{path}", &dir.display().to_string()),
            true,
        );
    }
    f.ui.paragraph("Holds the game log, the launcher log, the preview cache and your settings. Safe to open; nothing here changes the original OMSI 2.", Vec2::new(inner.x, inner.y + 44.0), inner.w, 12.0, Weight::Regular, TEXT_FAINT);
}

/// The Content card: what was found, or what it takes on disk.
fn content(f: &mut Frame, view: &mut SetupView, r: Rect) {
    f.ui.panel(r);
    let inner = card_title(
        f.ui,
        Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0),
        "Content",
    );
    let mut tab = view.tab;
    f.ui.segmented(
        "setup-content-tab",
        Rect::new(inner.x, inner.y, inner.w, 36.0),
        &mut tab,
        &["What was found", "Disk usage"],
    );
    view.tab = tab;
    let y = inner.y + 48.0;
    let Some(usage) = f.state.usage.clone() else {
        f.ui.text_in(
            "Counting what is in both folders…",
            Rect::new(inner.x, y, inner.w, 22.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        return;
    };
    if tab == 0 {
        let c = usage.counts;
        let rows = [
            ("Maps", c.maps),
            ("Vehicles", c.vehicles),
            ("Sceneryobjects", c.sceneryobjects),
            ("Splines", c.splines),
            ("Archives (.zip)", c.archives),
        ];
        let mut ry = y;
        for (k, (name, n)) in rows.iter().enumerate() {
            if k > 0 {
                f.ui.p().rect(Rect::new(inner.x, ry, inner.w, 1.0), EDGE);
            }
            kit::dot(
                f.ui,
                inner.x + 4.0,
                ry + 17.0,
                if *n > 0 {
                    Status::Found
                } else {
                    Status::Unverified
                },
            );
            f.ui.text_in(
                name,
                Rect::new(inner.x + 18.0, ry + 5.0, inner.w - 80.0, 24.0),
                13.5,
                Weight::Regular,
                TEXT,
                Align::Left,
            );
            f.ui.text_in(
                &n.to_string(),
                Rect::new(inner.x, ry + 5.0, inner.w, 24.0),
                13.5,
                Weight::Regular,
                TEXT_DIM,
                Align::Right,
            );
            ry += 35.0;
        }
        let note = "Counted each time the page opens, in the OMSI 2 folder and the content folder.";
        let h = f.ui.paragraph_height(note, inner.w, 12.0, Weight::Regular);
        f.ui.paragraph(
            note,
            Vec2::new(inner.x, inner.bottom() - h),
            inner.w,
            12.0,
            Weight::Regular,
            TEXT_FAINT,
        );
        return;
    }
    let note = "Inside out: folder, then type, then what is in it, then file kind. Click a slice; click the middle to go back.";
    let y =
        y + f.ui.paragraph(
            note,
            Vec2::new(inner.x, y),
            inner.w,
            12.0,
            Weight::Regular,
            TEXT_DIM,
        ) + 10.0;
    let legend = disk::by_type(&usage.tree);
    let legend_h = legend.len().div_ceil(2) as f32 * 22.0;
    let side = (inner.w)
        .min(inner.bottom() - y - legend_h - 24.0 - TRAIL_H)
        .max(120.0);
    let chart_r = Rect::new(inner.x + (inner.w - side) * 0.5, y, side, side);
    let (trail, bytes) = chart(f.ui, chart_r, &usage.tree, &mut view.focus);
    // what the slice under the mouse (or the one in the middle) is, with its whole names
    let line = if trail.is_empty() {
        tr("Everything in both folders").to_string()
    } else {
        trail.join(" › ")
    };
    let line = format!("{line} · {}", fmt_size(bytes));
    // (a name long enough for more than the reserved lines pushes the legend down, not under it)
    let lh = f.ui.paragraph_height(&line, inner.w, 12.0, Weight::Regular);
    f.ui.paragraph(
        &line,
        Vec2::new(inner.x, chart_r.bottom() + 6.0),
        inner.w,
        12.0,
        Weight::Regular,
        TEXT_SOFT,
    );
    let mut ly = chart_r.bottom() + 12.0 + lh.max(TRAIL_H) + 6.0;
    let cw = (inner.w - GAP) * 0.5;
    for (k, (name, bytes)) in legend.iter().enumerate() {
        let x = inner.x + (k % 2) as f32 * (cw + GAP);
        if k > 0 && k % 2 == 0 {
            ly += 22.0;
        }
        f.ui.p()
            .rounded(Rect::new(x, ly + 6.0, 10.0, 10.0), 2.0, type_color(name));
        f.ui.text_in(
            name,
            Rect::new(x + 18.0, ly, cw - 70.0, 22.0),
            13.0,
            Weight::Regular,
            TEXT,
            Align::Left,
        );
        f.ui.text_in(
            &fmt_size(*bytes),
            Rect::new(x, ly, cw, 22.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Right,
        );
    }
}

/// The room under the chart for the whole names of the part it shows (three lines).
const TRAIL_H: f32 = 48.0;

/// A size as the chart says it: whole gigabytes from 10 GB, else one decimal.
pub fn fmt_size(b: u64) -> String {
    let gb = b as f64 / 1_073_741_824.0;
    if gb >= 10.0 {
        format!("{gb:.0} GB")
    } else if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        fmt_bytes(b)
    }
}

/// A slice of the chart: its ring (0 the inner one), its angles, its path from the top of the
/// tree and its depth there (1 a folder, 2 a type, 3 what is in it, 4 a kind of file).
struct Slice<'a> {
    ring: usize,
    a0: f32,
    a1: f32,
    path: Vec<usize>,
    depth: usize,
    node: &'a Node,
    /// The type it belongs to (a folder belongs to none).
    ty: Option<&'a str>,
    /// Its place among its parent's parts.
    index: usize,
}

/// The node at `path` from `tree`, with its depth and the type it belongs to; a path that no
/// longer fits (the content read again) is cut back to what still fits.
pub fn node_at<'a>(tree: &'a Node, path: &mut Vec<usize>) -> (&'a Node, Option<&'a str>) {
    let mut n = tree;
    let mut ty = None;
    for (d, &k) in path.clone().iter().enumerate() {
        match n.kids.get(k) {
            Some(c) => {
                n = c;
                if d == 1 {
                    ty = Some(c.name.as_str());
                }
            }
            None => {
                path.truncate(d);
                break;
            }
        }
    }
    (n, ty)
}

/// The disk usage as rings around the middle (a sunburst): the part looked into in the
/// middle, its parts around it, theirs further out.
fn chart(ui: &mut Ui, r: Rect, tree: &Node, focus: &mut Vec<usize>) -> (Vec<String>, u64) {
    let (top, top_ty) = node_at(tree, focus);
    let depth0 = focus.len();
    let rings = 4usize.saturating_sub(depth0).max(1);
    let c = r.center();
    let outer = r.w.min(r.h) * 0.5 - 2.0;
    let hole = outer * 0.3;
    let ring_w = (outer - hole) / rings as f32;
    let mut slices: Vec<Slice> = Vec::new();
    // (breadth first: each ring from the one inside it)
    let mut level: Vec<(&Node, f32, f32, Vec<usize>, Option<&str>)> = vec![(
        top,
        -std::f32::consts::FRAC_PI_2,
        -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU,
        focus.clone(),
        top_ty,
    )];
    for ring in 0..rings {
        let mut next = Vec::new();
        for (n, a0, a1, path, ty) in level {
            if n.bytes == 0 {
                continue;
            }
            let mut a = a0;
            for (k, kid) in n.kids.iter().enumerate() {
                let span = (a1 - a0) * kid.bytes as f32 / n.bytes as f32;
                let mut p = path.clone();
                p.push(k);
                let depth = p.len();
                let kty = if depth == 2 {
                    Some(kid.name.as_str())
                } else {
                    ty
                };
                slices.push(Slice {
                    ring,
                    a0: a,
                    a1: a + span,
                    path: p.clone(),
                    depth,
                    node: kid,
                    ty: kty,
                    index: k,
                });
                next.push((kid, a, a + span, p, kty));
                a += span;
            }
        }
        level = next;
    }
    let id = id_of("setup-usage-chart");
    let (hover, _, clicked) = ui.interact(id, r);
    let m = ui.input.mouse - c;
    let dist = m.length();
    let hovered = if hover && dist > hole && dist < outer {
        let ring = ((dist - hole) / ring_w) as usize;
        let mut ang = m.y.atan2(m.x);
        if ang < -std::f32::consts::FRAC_PI_2 {
            ang += std::f32::consts::TAU;
        }
        slices
            .iter()
            .position(|s| s.ring == ring && ang >= s.a0 && ang < s.a1)
    } else {
        None
    };
    for (k, s) in slices.iter().enumerate() {
        let span = s.a1 - s.a0;
        if span < 0.002 {
            continue;
        }
        let gap = (span * 0.08).min(0.006);
        let r0 = hole + s.ring as f32 * ring_w + 1.0;
        let r1 = r0 + ring_w - 2.0;
        let base = match s.ty {
            None => Color::hex(if s.index % 2 == 0 { 0x6b7078 } else { 0x50555c }),
            Some(t) => {
                let tc = type_color(t);
                let darker = match s.depth {
                    0..=2 => 0.0,
                    3 => 0.28,
                    _ => 0.45,
                } + if s.index % 2 == 1 { 0.1 } else { 0.0 };
                tc.mix(PANEL, darker)
            }
        };
        let col = if hovered == Some(k) {
            base.lighten(0.18)
        } else {
            base
        };
        ui.p().arc(c, r0, r1, s.a0 + gap, s.a1 - gap, col);
    }
    ui.p().circle(c, hole - 2.0, FIELD);
    let (title, bytes) = match hovered {
        Some(k) => (slices[k].node.name.clone(), slices[k].node.bytes),
        None => (top.name.clone(), top.bytes),
    };
    // (the whole names, from the folder down: the middle only has room for the end of one)
    let trail_path: &[usize] = match hovered {
        Some(k) => &slices[k].path,
        None => focus,
    };
    let trail = trail_names(tree, trail_path);
    let size = fmt_size(bytes);
    ui.text_in(
        &size,
        Rect::new(c.x - hole, c.y - 16.0, hole * 2.0, 24.0),
        18.0,
        Weight::Bold,
        TEXT,
        Align::Center,
    );
    let name = kit::fit_text(ui, &title, 11.0, hole * 1.7);
    ui.text_in(
        &name,
        Rect::new(c.x - hole, c.y + 8.0, hole * 2.0, 16.0),
        11.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Center,
    );
    if clicked {
        match hovered {
            Some(k) if !slices[k].node.kids.is_empty() => *focus = slices[k].path.clone(),
            None if dist <= hole => {
                focus.pop();
            }
            _ => {}
        }
    }
    (trail, bytes)
}

/// The names along `path` from the top of `tree`, whole.
fn trail_names(tree: &Node, path: &[usize]) -> Vec<String> {
    let mut n = tree;
    let mut out = Vec::new();
    for &k in path {
        match n.kids.get(k) {
            Some(c) => {
                out.push(c.name.clone());
                n = c;
            }
            None => break,
        }
    }
    out
}

/// The foot: when changes apply, Revert and Save.
fn foot(f: &mut Frame, view: &mut SetupView, r: Rect) {
    f.ui.panel(r);
    let v = &view;
    let cfg = &f.state.config;
    let moving = v
        .content
        .as_ref()
        .is_some_and(|s| s.trim() != cfg.content.trim());
    let edited = v.root.as_ref().is_some_and(|s| *s != cfg.root)
        || v.game.as_ref().is_some_and(|s| *s != cfg.game)
        || moving;
    // (another content folder: what the one before holds is not taken along)
    let (text, c) = if moving {
        ("Another content folder: the mods, your controls, saved situations and screenshots of the one before stay there; nothing is moved.", WARN)
    } else {
        ("Changes apply to the next game you start.", TEXT_SOFT)
    };
    let text = kit::fit_text(f.ui, text, 13.5, r.w - 300.0);
    f.ui.text_in(
        &text,
        Rect::new(r.x + 18.0, r.y, r.w - 300.0, r.h),
        13.5,
        Weight::Regular,
        c,
        Align::Left,
    );
    let save = Rect::new(r.right() - 18.0 - 104.0, r.center().y - 17.0, 104.0, 34.0);
    let revert = Rect::new(save.x - 10.0 - 92.0, save.y, 92.0, 34.0);
    if edited {
        if f.ui
            .button("cfg-revert", revert, "Revert", None, ButtonKind::Normal)
        {
            view.root = None;
            view.game = None;
            view.content = None;
        }
    } else {
        dim_button(f.ui, revert, "Revert");
    }
    // (nothing chosen yet on the first run: nothing to save)
    if !edited && !f.root.ok(&f.state.config.root) {
        dim_button(f.ui, save, "Save");
    } else if f
        .ui
        .button("cfg-save", save, "Save", Some("save"), ButtonKind::Primary)
    {
        save_folders(f, view);
    }
}

fn save_folders(f: &mut Frame, view: &mut SetupView) {
    let cfg = f.state.config.clone();
    let chosen = view
        .root
        .clone()
        .unwrap_or_else(|| cfg.root.clone())
        .trim()
        .to_string();
    let game = view
        .game
        .clone()
        .unwrap_or_else(|| cfg.game.clone())
        .trim()
        .to_string();
    let content = view
        .content
        .clone()
        .unwrap_or_else(|| cfg.content.clone())
        .trim()
        .to_string();
    // A folder that is no complete installation is said so, with what it lacks, and
    // nothing is saved: it was saved, then quietly replaced by the installation found
    // before, while the page said "Saved" (#1656).
    let missing = if chosen.is_empty() {
        Vec::new()
    } else {
        omsi_cfg::missing_original_essentials(std::path::Path::new(&chosen))
    };
    if !missing.is_empty() {
        f.state.set_status(
            tr("Not saved: %{path} is not a complete OMSI 2 installation - it lacks %{files}")
                .replace("%{path}", &chosen)
                .replace("%{files}", &lacking(&missing, 6)),
            true,
        );
        return;
    }
    // (checked at each Save, against the OMSI 2 folder saved with it)
    if !content.is_empty() {
        if let Some(why) = core::content_folder_problem(
            std::path::Path::new(&content),
            std::path::Path::new(&chosen),
        ) {
            f.state.set_status(
                tr("Not saved: the content folder cannot be used - %{why}")
                    .replace("%{why}", &why_text(&why)),
                true,
            );
            return;
        }
    }
    let content_moved = content != cfg.content;
    f.state.config.root = chosen.clone();
    f.state.config.game = game;
    f.state.config.content = content;
    match core::save_config(&f.state.config) {
        Ok(()) => {
            // (what was read of the folders before is forgotten: a folder copied or
            // changed while the launcher ran is read as it is now)
            omsi_cfg::content_changed();
            f.state.config = core::load_config();
            *view = SetupView {
                tab: view.tab,
                // (another installation: the pictures of its vehicles are drawn once its
                // content has been read)
                other_root: chosen != cfg.root,
                ..SetupView::default()
            };
            let same = chosen.is_empty()
                || std::path::Path::new(&chosen) == std::path::Path::new(&f.state.config.root);
            if same && content_moved {
                f.state.set_status("Saved. What is in the content folder used before (mods, your controls, saved situations, screenshots) stays there; nothing is moved.", false);
            } else if same {
                f.state
                    .set_status("Saved. Reading the content again…", false);
            } else {
                f.state.set_status(
                    tr("Saved, but the OMSI 2 folder used is %{path}")
                        .replace("%{path}", &f.state.config.root),
                    true,
                );
            }
            // (a phone: the gallery is kept out of the folder chosen at once, before
            // its media scanner lists its textures as photos, #1632)
            f.host.hide_content_from_gallery();
            if content_moved {
                // (the mods are the new folder's)
                f.state.mods_asked = false;
            }
            f.state.load_content();
            if !mobile() {
                f.state.load_usage();
                f.state.reset_squeeze();
            }
        }
        Err(e) => f.state.set_status(format!("{e:#}"), true),
    }
}

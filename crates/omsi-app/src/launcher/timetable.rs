//! The Timetable page: a map's lines, their tours and the trips each tour runs - the part of
//! OMSI 2's timetable editor a driver uses to change when buses go. A line is saved as its
//! `.ttl`; a map of the original installation gets its `TTData` copied into the content
//! folder first (the game reads that copy before the original's, which stays untouched).
//! Trips (`.ttp`) and their tracks (`.ttr`) are the map maker's and are only chosen here.

use super::theme::*;
use super::ui::{ButtonKind, Ui};
use super::Launcher;
use omsi_launcher_lib as core;
use omsi_timetable::{Line, TimetableData, Tour, TourTrip};
use omsi_ui::paint::Align;
use omsi_ui::{Color, Rect, Weight};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct TimetableView {
    pub map: usize,
    /// The map the data was read for (its `global.cfg`).
    loaded: Option<String>,
    data: Option<TimetableData>,
    map_dir: PathBuf,
    line: usize,
    tour: usize,
    /// The departures being typed, one per trip of the shown tour.
    times: Vec<String>,
    times_for: Option<(usize, usize)>,
    /// Minutes a copied tour runs after the one it copies (and the repeat's interval).
    offset: String,
    /// The repeat's last departure ("h:mm").
    until: String,
    /// The name typed for a new line.
    new_line: String,
    /// The lines changed and not saved yet (by name): kept while the player moves between
    /// lines - the page used to drop a line's changes when another was clicked, and a day's
    /// timetable took a save after every line.
    dirty: std::collections::BTreeSet<String>,
    /// The reset button was pressed once: the next press puts the map's own timetable back.
    reset_armed: bool,
    /// The tab: 0 the map, 1 the schematic, 2 the trips.
    pub tab: usize,
    /// The map's `global.cfg` (the map tab reads it).
    global: PathBuf,
    /// What the lines run (their stops and times) for the map and the start's date, read in
    /// the background; what it was read for.
    plan: Vec<core::LineInfo>,
    plan_for: Option<(String, String)>,
    plan_rx: Option<std::sync::mpsc::Receiver<Result<Vec<core::LineInfo>, String>>>,
    /// The lines hidden on the map (by name).
    hidden: std::collections::BTreeSet<String>,
    /// The stop chosen on the map (by name), and what the search holds.
    stop: Option<String>,
    find: String,
    /// The direction the schematic draws: which of the line's termini it goes to.
    dir: usize,
}

impl TimetableView {
    /// The stop `name` chosen on the map.
    pub fn set_stop(&mut self, name: &str) {
        self.stop = Some(name.to_string());
    }
}

/// Marks a `TTData` folder the launcher copied into the content folder (see `save_target`):
/// the reset deletes such a copy, and only such a one.
const COPY_MARK: &str = ".depot-ttdata-copy";

/// "h:mm" (or "h:mm:ss") from minutes after midnight.
pub fn fmt_time(min: f32) -> String {
    let s = (min as f64 * 60.0).round() as i64;
    if s % 60 == 0 {
        format!("{}:{:02}", s / 3600, s / 60 % 60)
    } else {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    }
}

/// Minutes after midnight from "h:mm", "h:mm:ss" or "h.mm" (after midnight as 24:10 and on).
pub fn parse_time(t: &str) -> Option<f32> {
    let parts: Vec<&str> = t.trim().split([':', '.']).collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let h: u32 = parts[0].trim().parse().ok()?;
    let m: u32 = parts[1].trim().parse().ok()?;
    let s: u32 = parts.get(2).map(|x| x.trim().parse().ok()).unwrap_or(Some(0))?;
    (h < 48 && m < 60 && s < 60).then(|| h as f32 * 60.0 + m as f32 + s as f32 / 60.0)
}

/// Where a map's line is written: in place when the map lies unpacked in the content folder
/// (its file backed up as `<file>.orig` the first time, for the reset), else in the content
/// folder's copy of its `TTData` (made whole first - the game reads the one folder, not a mix
/// of both). A map inside a mod archive (`.zip`) counts as not in the content folder: its
/// file was "saved in place" into the archive's path, which is no folder, and never saved.
fn save_target(line: &Line, map_folder: &str, original_ttdata: &Path) -> Result<PathBuf, String> {
    let content = core::content_dir().ok_or("no content folder")?;
    let file = line.path.file_name().map(|f| f.to_owned()).unwrap_or_else(|| format!("{}.ttl", line.name).into());
    if line.path.starts_with(&content) && omsi_cfg::vfs::archive_of(&line.path).is_none() {
        let orig = PathBuf::from(format!("{}.orig", line.path.display()));
        let copied = line.path.parent().is_some_and(|d| d.join(COPY_MARK).is_file());
        if !copied && line.path.is_file() && !orig.exists() {
            std::fs::copy(&line.path, &orig).map_err(|e| e.to_string())?;
        }
        return Ok(line.path.clone());
    }
    let dir = content.join("maps").join(map_folder).join("TTData");
    if !dir.is_dir() {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for (name, is_dir) in omsi_cfg::vfs::list_dir(original_ttdata).ok_or_else(|| format!("{} cannot be read", original_ttdata.display()))? {
            if !is_dir {
                let bytes = omsi_cfg::vfs::read(&original_ttdata.join(&name)).map_err(|e| e.to_string())?;
                std::fs::write(dir.join(&name), bytes).map_err(|e| e.to_string())?;
            }
        }
        std::fs::write(dir.join(COPY_MARK), b"TTData copied by the Project Depot launcher's timetable editor; its reset deletes this folder\n").map_err(|e| e.to_string())?;
    }
    Ok(dir.join(file))
}

/// The map's own timetable back: the launcher's copy of its `TTData` deleted, or (a map
/// unpacked in the content folder) every line saved over put back from its `.orig`.
/// Returns what was done.
fn reset_timetable(map_dir: &Path, map_folder: &str) -> Result<String, String> {
    let content = core::content_dir().ok_or("no content folder")?;
    let copy = content.join("maps").join(map_folder).join("TTData");
    if copy.join(COPY_MARK).is_file() {
        std::fs::remove_dir_all(&copy).map_err(|e| e.to_string())?;
        return Ok(format!("The timetable of {map_folder} is the map's own again (the edited copy was removed)"));
    }
    let own = map_dir.join("TTData");
    let mut restored = 0;
    if own.is_dir() {
        for e in std::fs::read_dir(&own).map_err(|e| e.to_string())?.flatten() {
            let p = e.path();
            if let Some(orig) = p.to_str().and_then(|s| s.strip_suffix(".orig")) {
                std::fs::rename(&p, orig).map_err(|e| e.to_string())?;
                restored += 1;
            }
        }
    }
    if restored > 0 {
        Ok(format!("{restored} line(s) of {map_folder} put back as they were"))
    } else {
        Err(format!("The timetable of {map_folder} has not been changed here"))
    }
}

/// Save every changed line of the map (see `save_target`); how many were saved, and the
/// first error.
fn save_all(tv: &mut TimetableView) -> (usize, Option<String>) {
    let Some(data) = tv.data.as_mut() else { return (0, None) };
    let folder = tv.map_dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let original = omsi_cfg::resolve_path(&tv.map_dir, "TTData");
    let mut saved = 0;
    let mut err = None;
    for line in data.lines.iter_mut().filter(|l| tv.dirty.contains(&l.name)) {
        for t in &mut line.tours {
            t.trips.sort_by(|a, b| a.departure.total_cmp(&b.departure));
        }
        match save_target(line, &folder, &original).and_then(|p| line.save(&p).map(|_| p).map_err(|e| e.to_string())) {
            Ok(p) => {
                line.path = p;
                saved += 1;
            }
            Err(e) => {
                err.get_or_insert(format!("line {}: {e}", line.name));
            }
        }
    }
    if err.is_none() {
        tv.dirty.clear();
    }
    if saved > 0 {
        omsi_cfg::content_changed();
    }
    (saved, err)
}

/// Copies of `base` every `every` minutes after it, as long as the copy's first departure is
/// not after `until` (minutes of the day); numbered on from the highest. Returns how many.
fn repeat_tour(tours: &mut Vec<Tour>, base: &Tour, every: f32, until: f32) -> usize {
    let first = base.trips.first().map(|t| t.departure).unwrap_or(0.0);
    let mut made = 0;
    let mut k = 1.0;
    while first + every * k <= until + 1e-3 && made < 500 {
        let mut t = base.clone();
        for x in &mut t.trips {
            x.departure += every * k;
        }
        t.number = next_number(tours);
        tours.push(t);
        made += 1;
        k += 1.0;
    }
    made
}

/// A new tour's number: one past the highest that is a number.
fn next_number(tours: &[Tour]) -> String {
    (tours.iter().filter_map(|t| t.number.trim().parse::<i64>().ok()).max().unwrap_or(0) + 1).to_string()
}

// --- the plan: what the map's lines run, read for the Map and Schematic tabs -------------------

/// The colours the lines wear on the map and in the lists, by their place in the list.
const LINE_COLORS: [u32; 8] = [0xe8a33d, 0x4c8ddb, 0xa27bdb, 0x4cb86a, 0xe0605a, 0x2aa7a0, 0xd46fa8, 0xd9853b];

pub fn line_color(k: usize) -> Color {
    Color::hex(LINE_COLORS[k % LINE_COLORS.len()])
}

/// A line's short name for its badge.
fn badge_text(name: &str) -> String {
    let n = name.trim();
    if n.chars().count() <= 4 {
        n.to_string()
    } else {
        n.chars().take(3).collect::<String>() + "…"
    }
}

/// A line's badge: its short name on its colour.
fn badge(ui: &mut Ui, r: Rect, name: &str, c: Color, dim: bool) {
    let c = if dim { c.mix(PANEL, 0.55) } else { c };
    ui.p().rounded(r, 4.0, c);
    ui.text_in(&badge_text(name), r, 11.5, Weight::Bold, omsi_ui::theme::on(c), Align::Center);
}

/// The ways a line runs: its termini, in the order the line lists them.
fn directions(line: &core::LineInfo) -> Vec<String> {
    let mut out: Vec<String> = line.termini.clone();
    for t in line.tours.iter().flat_map(|t| t.trips.iter()) {
        if !t.terminus.is_empty() && !out.contains(&t.terminus) {
            out.push(t.terminus.clone());
        }
    }
    out
}

/// The trip that stands for a line going to its `dir`-th terminus: the one with the most stops.
pub fn route_of(line: &core::LineInfo, dir: usize) -> Option<&core::TripInfo> {
    let dirs = directions(line);
    let to = dirs.get(dir % dirs.len().max(1)).cloned().unwrap_or_default();
    let trips = line.tours.iter().flat_map(|t| t.trips.iter());
    trips.filter(|t| to.is_empty() || t.terminus == to).max_by_key(|t| t.stops.len())
}

/// The departures at the stop named `stop` from `from` (seconds of the day) on, of the tours
/// that run on the date: (time, the line's place in `lines`, where it goes), the first `n`.
pub fn departures_at(lines: &[core::LineInfo], stop: &str, from: f64, n: usize) -> Vec<(f64, usize, String)> {
    let mut out = Vec::new();
    for (k, line) in lines.iter().enumerate() {
        for tour in line.tours.iter().filter(|t| t.runs) {
            for trip in &tour.trips {
                // (the last stop is where the trip ends: nothing leaves from there)
                for st in trip.stops.iter().take(trip.stops.len().saturating_sub(1)) {
                    if st.name.eq_ignore_ascii_case(stop) && st.dep >= from {
                        out.push((st.dep, k, trip.terminus.clone()));
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.truncate(n);
    out
}

/// The lines that call at the stop named `stop`, each with where it goes from there.
pub fn serving(lines: &[core::LineInfo], stop: &str) -> Vec<(usize, Vec<String>)> {
    let mut out: Vec<(usize, Vec<String>)> = Vec::new();
    for (k, line) in lines.iter().enumerate() {
        for trip in line.tours.iter().flat_map(|t| t.trips.iter()) {
            let calls = trip.stops.iter().take(trip.stops.len().saturating_sub(1)).any(|s| s.name.eq_ignore_ascii_case(stop));
            if !calls {
                continue;
            }
            match out.iter_mut().find(|x| x.0 == k) {
                Some((_, to)) if !to.contains(&trip.terminus) => to.push(trip.terminus.clone()),
                Some(_) => {}
                None => out.push((k, vec![trip.terminus.clone()])),
            }
        }
    }
    out
}

/// "h:mm" of seconds of the day (past midnight as 24:10 and on).
fn hm(secs: f64) -> String {
    let m = (secs / 60.0).round() as i64;
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// The line of `data` named `name` among the plan's.
fn info_of<'a>(plan: &'a [core::LineInfo], name: &str) -> Option<(usize, &'a core::LineInfo)> {
    plan.iter().enumerate().find(|(_, x)| x.name.eq_ignore_ascii_case(name))
}

pub fn draw(l: &mut Launcher, area: Rect) {
    let body = l.page_title(area, omsi_launcher_ui::Page::Timetable);
    let maps: Vec<(String, String)> = l.state.maps.iter().map(|m| (m.friendly.clone(), m.file.clone())).collect();
    if maps.is_empty() {
        // (a card, as Sessions says it has none)
        let r = Rect::new(body.x, body.y, body.w.min(720.0), 120.0);
        l.ui.panel(r);
        l.ui.icon("schedule", glam::Vec2::new(r.x + 40.0, r.center().y), 36.0, TEXT_FAINT);
        l.ui.paragraph("No maps found.", glam::Vec2::new(r.x + 76.0, r.y + 52.0), r.w - 100.0, 13.5, Weight::Regular, TEXT_DIM);
        return;
    }
    let tv = &mut l.pages.tt;
    // (first shown: the map chosen on the Maps page, not the first of the list)
    if tv.loaded.is_none() {
        if let Some(k) = maps.iter().position(|m| m.1 == l.state.choice.map) {
            tv.map = k;
        }
    }
    tv.map = tv.map.min(maps.len() - 1);
    // read the chosen map's timetable
    if tv.loaded.as_deref() != Some(maps[tv.map].1.as_str()) {
        let file = &maps[tv.map].1;
        let root = PathBuf::from(&l.state.config.root);
        let global = core::content_dir().map(|c| c.join(file)).filter(|p| p.is_file()).unwrap_or_else(|| omsi_cfg::resolve_path(&root, file));
        tv.map_dir = global.parent().map(Path::to_path_buf).unwrap_or_default();
        tv.global = global;
        let mut d = TimetableData::load(&tv.map_dir);
        d.lines.sort_by_key(|x| x.name.to_lowercase());
        tv.data = Some(d);
        tv.loaded = Some(file.clone());
        tv.line = 0;
        tv.tour = 0;
        tv.times_for = None;
        tv.dirty.clear();
        tv.hidden.clear();
        tv.plan_for = None;
    }
    // what the lines run (stops, times), read in the background for the map and the start's date
    let want = (maps[tv.map].1.clone(), l.state.choice.date.clone());
    if tv.plan_for.as_ref() != Some(&want) {
        tv.plan_for = Some(want.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(core::list_lines(&want.0, &want.1).map_err(|e| format!("{e:#}")));
        });
        tv.plan_rx = Some(rx);
    }
    if let Some(r) = tv.plan_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
        tv.plan = r.unwrap_or_default();
        // (in the list's order: a line's colour is its place there)
        tv.plan.sort_by_key(|x| x.name.to_lowercase());
        tv.plan_rx = None;
    }
    if tv.offset.is_empty() {
        tv.offset = "20".into();
    }
    if tv.until.is_empty() {
        tv.until = "22:00".into();
    }
    // the tabs, the map and what can be changed
    let bar = Rect::new(body.x, body.y, body.w, 36.0);
    let mut tab = tv.tab;
    if l.ui.segmented("tt-tab", Rect::new(bar.x, bar.y, 360.0f32.min(bar.w * 0.4), bar.h), &mut tab, &["Map", "Schematic", "Trips"]) {
        tv.tab = tab;
    }
    let names: Vec<String> = maps.iter().map(|m| m.0.clone()).collect();
    let mw = 240.0f32.min(bar.w * 0.25);
    let mut m = tv.map;
    let map_r = Rect::new(bar.right() - mw, bar.y, mw, bar.h);
    let note = "Stops, streets and routes are read from the map; only departures can be edited";
    let nw = l.ui.width(note, 12.0, Weight::Regular) + 24.0;
    if map_r.x - nw - 12.0 > bar.x + 380.0 {
        let cr = Rect::new(map_r.x - nw - 12.0, bar.y + 5.0, nw, 26.0);
        l.ui.p().rounded(cr, 13.0, Color::WHITE.alpha(0.04));
        l.ui.p().rounded_border(cr, 13.0, 1.0, EDGE);
        l.ui.text_in(note, cr, 12.0, Weight::Regular, TEXT_DIM, Align::Center);
    }
    if l.ui.select("tt-map", map_r, &mut m, &names) {
        // (another map: this one's changes are saved first, not lost)
        if !tv.dirty.is_empty() {
            let (saved, err) = save_all(tv);
            match err {
                Some(e) => l.state.set_status(format!("Not saved: {e}"), true),
                None => l.state.set_status(format!("{saved} line(s) saved"), false),
            }
        }
        tv.map = m;
        tv.loaded = None;
        tv.stop = None;
        return;
    }
    let main = Rect::new(body.x, bar.bottom() + 14.0, body.w, (body.bottom() - bar.bottom() - 14.0).max(0.0));
    let lw = (main.w * 0.27).clamp(240.0, 320.0);
    let left = Rect::new(main.x, main.y, lw, main.h);
    let right = Rect::new(left.right() + GAP, main.y, main.right() - left.right() - GAP, main.h);
    if !lines_card(l, left) {
        return;
    }
    match l.pages.tt.tab {
        0 => map_tab(l, right),
        1 => schematic_tab(l, right),
        _ => trips_card(l, right),
    }
}

/// The map's lines: chosen with a click, shown or hidden on the map with their switch; under
/// them what the tab offers (new line and the map's own timetable back, the direction drawn,
/// or all shown or hidden). False when the page should stop for this frame.
fn lines_card(l: &mut Launcher, left: Rect) -> bool {
    let tab = l.pages.tt.tab;
    let ui: &mut Ui = &mut l.ui;
    let tv = &mut l.pages.tt;
    let mut status: Option<(String, bool)> = None;
    ui.panel(left);
    let inner = Launcher::card_title(ui, Rect::new(left.x + 18.0, left.y + 14.0, left.w - 36.0, left.h - 28.0), "Lines");
    let hint = match tab {
        0 => "the switch shows or hides",
        1 => "pick one to draw",
        _ => "",
    };
    omsi_launcher_ui::kit::right(ui, hint, left.right() - 18.0, left.y + 22.0, 12.0, Weight::Regular, TEXT_DIM);
    let foot_h = 2.0 * ROW + 8.0;
    let foot = Rect::new(inner.x, inner.bottom() - foot_h, inner.w, foot_h);
    let Some(data) = tv.data.as_mut() else { return false };
    match tab {
        0 => {
            let half = (foot.w - GAP) * 0.5;
            if ui.button("tt-show-all", Rect::new(foot.x, foot.bottom() - ROW, half, ROW), "Show all", None, ButtonKind::Normal) {
                tv.hidden.clear();
            }
            if ui.button("tt-hide-all", Rect::new(foot.x + half + GAP, foot.bottom() - ROW, half, ROW), "Hide all", None, ButtonKind::Normal) {
                tv.hidden = data.lines.iter().map(|x| x.name.clone()).collect();
            }
        }
        1 => {
            let dirs = data.lines.get(tv.line).and_then(|x| info_of(&tv.plan, &x.name)).map(|(_, i)| directions(i)).unwrap_or_default();
            if !dirs.is_empty() {
                let names: Vec<String> = dirs.iter().map(|d| format!("Direction: to {d}")).collect();
                let mut d = tv.dir % dirs.len();
                if ui.select("tt-dir", Rect::new(foot.x, foot.y, foot.w, ROW), &mut d, &names) {
                    tv.dir = d;
                }
                if ui.button("tt-swap", Rect::new(foot.x, foot.bottom() - ROW, foot.w, ROW), "Swap direction", None, ButtonKind::Normal) {
                    tv.dir = (tv.dir + 1) % dirs.len();
                }
            }
        }
        _ => {
            // a new line: its name, then Add (the file is `<name>.ttl` in the map's TTData)
            let new_r = Rect::new(foot.x, foot.y, foot.w - 90.0, ROW);
            ui.text_input("tt-new-line", new_r, &mut tv.new_line, "New line (name)", Some("add"));
            let add_line = ui.button("tt-add-line", Rect::new(new_r.right() + 8.0, new_r.y, 82.0, ROW), "Add", None, ButtonKind::Normal);
            // the map's own timetable back (two presses)
            let reset_r = Rect::new(foot.x, foot.bottom() - ROW, foot.w, ROW);
            if ui.button("tt-reset", reset_r, if tv.reset_armed { "Press again to restore" } else { "Restore the map's original timetable" }, Some("restart_alt"), ButtonKind::Normal) {
                if tv.reset_armed {
                    tv.reset_armed = false;
                    let folder = tv.map_dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    match reset_timetable(&tv.map_dir, &folder) {
                        Ok(m) => {
                            omsi_cfg::content_changed();
                            // read again from where the game reads it
                            tv.loaded = None;
                            l.state.set_status(m, false);
                            return false;
                        }
                        Err(e) => status = Some((e, true)),
                    }
                } else {
                    tv.reset_armed = true;
                    status = Some(("Press the button again to put the map's own timetable back (the changes made here are lost)".into(), false));
                }
            }
            if add_line {
                let name = tv.new_line.trim().to_string();
                if name.is_empty() || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
                    status = Some(("Type the new line's name (it becomes the file name, so no / \\ : * ? \" < > |)".into(), true));
                } else if data.lines.iter().any(|x| x.name.eq_ignore_ascii_case(&name)) {
                    status = Some((format!("Line {name} is there already"), true));
                } else {
                    let path = omsi_cfg::resolve_path(&tv.map_dir, "TTData").join(format!("{name}.ttl"));
                    data.lines.push(Line { path, name: name.clone(), user_allowed: true, priority: 0, tours: Vec::new() });
                    data.lines.sort_by_key(|x| x.name.to_lowercase());
                    tv.line = data.lines.iter().position(|x| x.name == name).unwrap_or(0);
                    tv.tour = 0;
                    tv.times_for = None;
                    tv.dirty.insert(name.clone());
                    tv.new_line.clear();
                    status = Some((format!("Line {name} added: give it tours, then save"), false));
                }
            }
        }
    }
    let list = Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, foot.y - 10.0 - inner.y);
    let rows: Vec<(String, String, String, Color, bool)> = data
        .lines
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let info = info_of(&tv.plan, &x.name);
            let (title, sub) = match info {
                Some((_, li)) => {
                    let dirs = directions(li);
                    let stops = route_of(li, 0).map(|t| t.stops.len()).unwrap_or(0);
                    let trips: usize = li.tours.iter().map(|t| t.trips.len()).sum();
                    let title = if dirs.len() >= 2 { format!("{} - {}", dirs[1], dirs[0]) } else { dirs.first().cloned().unwrap_or_else(|| x.name.clone()) };
                    (title, format!("{stops} stops · {trips} trips"))
                }
                None => (x.name.clone(), format!("{} tours", x.tours.len())),
            };
            let dirty = if tv.dirty.contains(&x.name) { " •" } else { "" };
            (x.name.clone(), format!("{title}{dirty}"), sub, line_color(info.map(|i| i.0).unwrap_or(i)), tv.hidden.contains(&x.name))
        })
        .collect();
    if rows.is_empty() {
        ui.paragraph("This map has no timetable (no TTData lines).", glam::Vec2::new(inner.x, inner.y + 8.0), inner.w, 13.0, Weight::Regular, TEXT_DIM);
        if let Some((s, e)) = status {
            l.state.set_status(s, e);
        }
        return false;
    }
    let mut pick = None;
    let mut flip = None;
    let sel = tv.line;
    ui.scroll_area("tt-lines", list, &mut |ui, v| {
        for (i, (name, title, sub, c, hidden)) in rows.iter().enumerate() {
            let r = Rect::new(v.x + 6.0, v.y + i as f32 * 56.0, v.w - 16.0, 50.0);
            if ui.row(&format!("tt-line-{i}"), r, i == sel) {
                pick = Some(i);
            }
            badge(ui, Rect::new(r.x + 10.0, r.y + 14.0, 36.0, 22.0), name, *c, *hidden && tab == 0);
            let tw = r.w - 70.0 - if tab < 2 { 56.0 } else { 0.0 };
            let t = omsi_launcher_ui::kit::fit_text(ui, title, 13.5, tw);
            ui.text_in(&t, Rect::new(r.x + 56.0, r.y + 6.0, tw, 20.0), 13.5, Weight::Medium, TEXT, Align::Left);
            ui.text_in(sub, Rect::new(r.x + 56.0, r.y + 26.0, tw, 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
            if tab < 2 {
                let mut on = !hidden;
                if ui.toggle(&format!("tt-show-{i}"), Rect::new(r.right() - 52.0, r.y + 12.0, 44.0, 26.0), &mut on, "") {
                    flip = Some(name.clone());
                }
            }
        }
        rows.len() as f32 * 56.0
    });
    if let Some(name) = flip {
        if !tv.hidden.remove(&name) {
            tv.hidden.insert(name);
        }
    }
    if let Some(i) = pick {
        // (the line left keeps its changes: they are saved with the others)
        tv.line = i;
        tv.tour = 0;
        tv.dir = 0;
        tv.times_for = None;
    }
    tv.line = tv.line.min(data.lines.len() - 1);
    if let Some((s, e)) = status {
        l.state.set_status(s, e);
    }
    true
}

/// A stop of the shown lines, where it stands on the map.
struct Placed {
    name: String,
    at: glam::Vec2,
}

/// The Map tab: the map with the shown lines drawn from stop to stop, and the stop chosen
/// with the lines that serve it and what leaves from it next.
fn map_tab(l: &mut Launcher, r: Rect) {
    let side_w = (r.w * 0.36).clamp(260.0, 330.0);
    let card = Rect::new(r.x, r.y, r.w - side_w - GAP, r.h);
    let side = Rect::new(card.right() + GAP, r.y, side_w, r.h);
    l.ui.panel(card);
    let map = Rect::new(card.x + 1.0, card.y + 58.0, card.w - 2.0, card.h - 58.0 - 46.0);
    let file = l.pages.tt.loaded.clone().unwrap_or_default();
    l.mapview.want(super::mapview::Look { map: file, global: l.pages.tt.global.clone(), date: l.state.choice.date.clone(), trip: String::new(), entry: -1 });
    l.map_background(map);
    // the shown lines, from stop to stop
    let plan = l.pages.tt.plan.clone();
    let hidden = l.pages.tt.hidden.clone();
    let mut placed: Vec<Placed> = Vec::new();
    for (k, line) in plan.iter().enumerate() {
        if hidden.iter().any(|h| h.eq_ignore_ascii_case(&line.name)) {
            continue;
        }
        for dir in 0..directions(line).len().clamp(1, 2) {
            let Some(trip) = route_of(line, dir) else { continue };
            let pts: Vec<(String, glam::Vec2)> = trip.stops.iter().filter_map(|s| Some((s.name.clone(), l.mapview.project(l.mapview.place_of(s.id)?)))).collect();
            if dir == 0 {
                for w in pts.windows(2) {
                    l.ui.p().line(w[0].1, w[1].1, 4.0, line_color(k));
                }
            }
            for (name, at) in pts {
                if !placed.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
                    placed.push(Placed { name, at });
                }
            }
        }
    }
    let mouse = l.ui.input.mouse;
    let over = placed.iter().position(|p| map.contains(p.at) && p.at.distance(mouse) < 9.0);
    let chosen = l.pages.tt.stop.clone();
    let few = placed.len() <= 30;
    // (the chosen and the hovered stop's names first, then the rest where they fit)
    let mut order: Vec<usize> = (0..placed.len()).collect();
    order.sort_by_key(|&i| !(chosen.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(&placed[i].name)) || over == Some(i)));
    let mut labels: Vec<Rect> = Vec::new();
    for &i in &order {
        let p = &placed[i];
        if !map.contains(p.at) {
            continue;
        }
        let on = chosen.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(&p.name));
        if on {
            l.ui.p().circle(p.at, 9.0, accent());
        }
        l.ui.p().circle(p.at, 5.5, Color::rgba(20, 20, 20, 1.0));
        l.ui.p().circle(p.at, 3.5, Color::rgba(240, 240, 240, 1.0));
        if few || on || over == Some(i) {
            let w = l.ui.width(&p.name, 11.5, Weight::Medium) + 14.0;
            // right of the stop, else left of it; a name that would cover another is left out
            let fits = |r: &Rect| map.contains(glam::Vec2::new(r.x, r.y)) && map.contains(glam::Vec2::new(r.right(), r.bottom())) && !labels.iter().any(|o| o.x < r.right() && r.x < o.right() && o.y < r.bottom() && r.y < o.bottom());
            let right = Rect::new(p.at.x + 9.0, p.at.y - 20.0, w, 18.0);
            let left = Rect::new(p.at.x - 9.0 - w, p.at.y - 20.0, w, 18.0);
            if let Some(lr) = [right, left].into_iter().find(fits) {
                labels.push(lr);
                l.ui.p().rounded(lr, 3.0, Color::rgba(0, 0, 0, 0.6));
                l.ui.text_in(&p.name, lr.pad(5.0, 0.0), 11.5, Weight::Medium, TEXT, Align::Left);
            }
        }
    }
    if let (Some(i), true) = (over, l.ui.input.pressed) {
        l.pages.tt.stop = Some(placed[i].name.clone());
    }
    // the search: the first stop whose name holds what is typed
    let find = Rect::new(card.x + 12.0, card.y + 12.0, (card.w - 24.0).min(320.0), 34.0);
    if l.ui.text_input("tt-find", find, &mut l.pages.tt.find, "Find a stop...", Some("search")) {
        let q = l.pages.tt.find.trim().to_lowercase();
        if let Some(p) = placed.iter().find(|p| !q.is_empty() && p.name.to_lowercase().contains(&q)) {
            l.pages.tt.stop = Some(p.name.clone());
        }
    }
    let hint_y = card.bottom() - 34.0;
    let shown = placed.iter().filter(|p| map.contains(p.at)).count();
    let mut x = card.x + 12.0;
    for t in ["Scroll to zoom · drag to move · click a stop".to_string(), format!("{shown} stops shown")] {
        let w = l.ui.width(&t, 12.0, Weight::Regular) + 22.0;
        let cr = Rect::new(x, hint_y, w, 24.0);
        l.ui.p().rounded(cr, 12.0, Color::WHITE.alpha(0.04));
        l.ui.p().rounded_border(cr, 12.0, 1.0, EDGE);
        l.ui.text_in(&t, cr, 12.0, Weight::Regular, TEXT_DIM, Align::Center);
        x += w + 8.0;
    }
    stop_card(l, side);
    // (looking only: the map's entry points are the Maps page's, a click here takes none)
    let entry = l.state.choice.entry;
    l.map_interact(map, map);
    l.state.choice.entry = entry;
}

/// The stop chosen on the map: the lines that serve it and the next departures from it.
fn stop_card(l: &mut Launcher, r: Rect) {
    l.ui.panel(r);
    let inner = Launcher::card_title(&mut l.ui, Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0), "Stop");
    omsi_launcher_ui::kit::right(&mut l.ui, "on the map", r.right() - 18.0, r.y + 22.0, 12.0, Weight::Regular, TEXT_DIM);
    let Some(stop) = l.pages.tt.stop.clone() else {
        l.ui.paragraph("Click a stop on the map to see which lines serve it and what leaves from it next.", glam::Vec2::new(inner.x, inner.y), inner.w, 12.5, Weight::Regular, TEXT_DIM);
        return;
    };
    let plan = l.pages.tt.plan.clone();
    let lines = serving(&plan, &stop);
    let from = l.state.choice.time as f64 * 60.0;
    let mut y = inner.y;
    l.ui.icon("o-pin", glam::Vec2::new(inner.x + 8.0, y + 16.0), 18.0, TEXT_SOFT);
    l.ui.text_in(&stop, Rect::new(inner.x + 28.0, y, inner.w - 28.0, 22.0), 15.0, Weight::Bold, TEXT, Align::Left);
    let kind = match lines.len() {
        0 => "No line leaves from here".to_string(),
        1 => "1 line".to_string(),
        n => format!("Interchange · {n} lines"),
    };
    l.ui.text_in(&kind, Rect::new(inner.x + 28.0, y + 22.0, inner.w - 28.0, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
    y += 52.0;
    Launcher::part_title(&mut l.ui, Rect::new(inner.x, y, inner.w, 28.0), "Lines that serve this stop");
    y += 32.0;
    let hidden = l.pages.tt.hidden.clone();
    for (k, to) in &lines {
        let line = &plan[*k];
        let off = hidden.iter().any(|h| h.eq_ignore_ascii_case(&line.name));
        let row = Rect::new(inner.x, y, inner.w, 46.0);
        l.ui.p().rounded(row, 6.0, if off { FIELD.alpha(0.5) } else { FIELD });
        badge(&mut l.ui, Rect::new(row.x + 10.0, row.y + 12.0, 36.0, 22.0), &line.name, line_color(*k), off);
        let next: Vec<String> = departures_at(std::slice::from_ref(line), &stop, from, 2).iter().map(|d| hm(d.0)).collect();
        let sub = if off { "line hidden".to_string() } else if next.is_empty() { "no more departures today".to_string() } else { format!("next {}", next.join(", ")) };
        let title = format!("to {}", to.join(" / "));
        let title = omsi_launcher_ui::kit::fit_text(&mut l.ui, &title, 13.5, row.w - 70.0);
        l.ui.text_in(&title, Rect::new(row.x + 56.0, row.y + 5.0, row.w - 66.0, 20.0), 13.5, Weight::Medium, if off { TEXT_DIM } else { TEXT }, Align::Left);
        l.ui.text_in(&sub, Rect::new(row.x + 56.0, row.y + 24.0, row.w - 66.0, 16.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
        y += 52.0;
        if y > r.bottom() - 260.0 {
            break;
        }
    }
    y += 8.0;
    Launcher::part_title(&mut l.ui, Rect::new(inner.x, y, inner.w, 28.0), "Next departures here");
    y += 32.0;
    let buttons_y = inner.bottom() - 2.0 * ROW - 8.0;
    let cols = [(0.0, "Time"), (0.24, "Line"), (0.46, "To")];
    for (f, t) in cols {
        l.ui.text_in(t, Rect::new(inner.x + inner.w * f, y, inner.w * 0.5, 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
    }
    y += 24.0;
    let fit = ((buttons_y - y - 8.0) / 30.0).max(0.0) as usize;
    for (t, k, to) in departures_at(&plan, &stop, from, fit) {
        l.ui.p().rect(Rect::new(inner.x, y, inner.w, 1.0), EDGE);
        l.ui.text_in(&hm(t), Rect::new(inner.x, y + 4.0, inner.w * 0.24, 22.0), 13.0, Weight::Regular, TEXT, Align::Left);
        l.ui.text_in(&plan[k].name, Rect::new(inner.x + inner.w * 0.24, y + 4.0, inner.w * 0.22, 22.0), 13.0, Weight::Bold, line_color(k), Align::Left);
        let to = omsi_launcher_ui::kit::fit_text(&mut l.ui, &to, 13.0, inner.w * 0.54);
        l.ui.text_in(&to, Rect::new(inner.x + inner.w * 0.46, y + 4.0, inner.w * 0.54, 22.0), 13.0, Weight::Regular, TEXT, Align::Left);
        y += 30.0;
    }
    if l.ui.button("tt-only-these", Rect::new(inner.x, buttons_y, inner.w, ROW), "Show only these lines", None, ButtonKind::Normal) {
        let keep: Vec<String> = lines.iter().map(|(k, _)| plan[*k].name.to_lowercase()).collect();
        if let Some(d) = l.pages.tt.data.as_ref() {
            l.pages.tt.hidden = d.lines.iter().filter(|x| !keep.contains(&x.name.to_lowercase())).map(|x| x.name.clone()).collect();
        }
    }
    if l.ui.button("tt-edit-line", Rect::new(inner.x, buttons_y + ROW + 8.0, inner.w, ROW), "Edit this line's trips", Some("chevron_right"), ButtonKind::Normal) {
        let tv = &mut l.pages.tt;
        if let Some(d) = tv.data.as_ref() {
            let current = d.lines.get(tv.line).map(|x| x.name.to_lowercase());
            let serves = |n: &str| lines.iter().any(|(k, _)| plan[*k].name.eq_ignore_ascii_case(n));
            if !current.as_deref().is_some_and(serves) {
                if let Some(i) = d.lines.iter().position(|x| serves(&x.name)) {
                    tv.line = i;
                    tv.tour = 0;
                    tv.times_for = None;
                }
            }
        }
        tv.tab = 2;
    }
}

/// The Schematic tab: the chosen line's stops in order, the minutes between them, the other
/// lines at each stop, and where one can change to them.
fn schematic_tab(l: &mut Launcher, r: Rect) {
    l.ui.panel(r);
    let tv = &l.pages.tt;
    let name = tv.data.as_ref().and_then(|d| d.lines.get(tv.line)).map(|x| x.name.clone()).unwrap_or_default();
    let inner = Launcher::card_title(&mut l.ui, Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0), &format!("Line {name} · diagram"));
    omsi_launcher_ui::kit::right(&mut l.ui, "schematic, stops in order", r.right() - 18.0, r.y + 22.0, 12.0, Weight::Regular, TEXT_DIM);
    let plan = tv.plan.clone();
    let Some((k, info)) = info_of(&plan, &name) else {
        let t = if tv.plan_rx.is_some() { "Reading the lines…" } else { "This line runs no trips the map's timetable knows of." };
        l.ui.text_in(t, Rect::new(inner.x, inner.y, inner.w, 22.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
        return;
    };
    let Some(trip) = route_of(info, tv.dir) else {
        l.ui.text_in("This line has no trips yet.", Rect::new(inner.x, inner.y, inner.w, 22.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
        return;
    };
    let stops = &trip.stops;
    let c = line_color(k);
    let from = l.state.choice.time as f64 * 60.0;
    // the stops in rows, as many as fit side by side
    // (inset by half a name: the first and last stop's names stay inside the card)
    let pad = 60.0f32.min(inner.w * 0.1);
    let span = inner.w - 2.0 * pad;
    let per = (span / 120.0).floor().max(2.0) as usize;
    let rows = stops.len().div_ceil(per);
    // (a line that fits in one row spreads over the whole of it)
    let slots = if rows == 1 { stops.len().max(2) } else { per };
    let row_h = 120.0;
    let mut y0 = inner.y + 20.0;
    for row in 0..rows {
        let part = &stops[row * per..((row + 1) * per).min(stops.len())];
        let n = part.len();
        let step = span / (slots - 1) as f32;
        let x_of = |i: usize| inner.x + pad + i as f32 * step;
        let cy = y0 + 30.0;
        if n > 1 {
            l.ui.p().rect(Rect::new(x_of(0), cy - 3.0, x_of(n - 1) - x_of(0), 6.0), c);
        }
        // (the line goes on in the next row)
        if row + 1 < rows {
            l.ui.p().rect(Rect::new(x_of(n - 1), cy - 3.0, inner.right() - x_of(n - 1), 6.0), c.alpha(0.4));
        }
        for (i, st) in part.iter().enumerate() {
            let x = x_of(i);
            let label = omsi_launcher_ui::kit::fit_text(&mut l.ui, &st.name, 12.0, step.max(80.0) - 8.0);
            l.ui.text_in(&label, Rect::new(x - step * 0.5, y0, step, 18.0), 12.0, Weight::Medium, TEXT, Align::Center);
            l.ui.p().circle(glam::Vec2::new(x, cy), 10.0, TEXT);
            l.ui.p().circle(glam::Vec2::new(x, cy), 7.0, PANEL);
            // the minutes to the next stop
            let gi = row * per + i;
            if let Some(next) = stops.get(gi + 1) {
                if i + 1 < n {
                    let mins = ((next.arr - st.dep) / 60.0).round().max(0.0);
                    l.ui.text_in(&format!("{mins:.0} min"), Rect::new(x, cy + 6.0, step, 16.0), 10.5, Weight::Regular, TEXT_DIM, Align::Center);
                }
            }
            // the other lines here
            let others: Vec<usize> = serving(&plan, &st.name).into_iter().map(|x| x.0).filter(|o| *o != k).collect();
            for (j, o) in others.iter().take(3).enumerate() {
                badge(&mut l.ui, Rect::new(x - 20.0, cy + 22.0 + j as f32 * 20.0, 40.0, 16.0), &plan[*o].name, line_color(*o), false);
            }
            if others.len() > 3 {
                l.ui.text_in(&format!("+{}", others.len() - 3), Rect::new(x - 20.0, cy + 82.0, 40.0, 14.0), 10.5, Weight::Regular, TEXT_DIM, Align::Center);
            }
        }
        y0 += row_h;
    }
    // what the drawing means
    let whole = ((stops.last().map(|s| s.arr).unwrap_or(0.0) - stops.first().map(|s| s.dep).unwrap_or(0.0)) / 60.0).round();
    let mut x = inner.x;
    let mut y = y0 + 4.0;
    for t in ["Coloured tags under a stop: other lines that serve it".to_string(), "Minutes: running time between stops".to_string(), format!("Whole route {whole:.0} min")] {
        let w = l.ui.width(&t, 12.0, Weight::Regular) + 22.0;
        if x + w > inner.right() {
            x = inner.x;
            y += 32.0;
        }
        let cr = Rect::new(x, y, w, 24.0);
        l.ui.p().rounded(cr, 12.0, Color::WHITE.alpha(0.04));
        l.ui.p().rounded_border(cr, 12.0, 1.0, EDGE);
        l.ui.text_in(&t, cr, 12.0, Weight::Regular, TEXT_DIM, Align::Center);
        x += w + 8.0;
    }
    y += 44.0;
    // where one can change
    Launcher::part_title(&mut l.ui, Rect::new(inner.x, y, inner.w, 28.0), "Interchanges");
    y += 34.0;
    for (f, t) in [(0.0, "Stop"), (0.4, "Change to"), (0.62, "Next departures of those lines")] {
        l.ui.text_in(t, Rect::new(inner.x + 12.0 + inner.w * f, y, inner.w * 0.4, 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
    }
    y += 24.0;
    let foot = "Diagram drawn from the stop order of the line's route. Nothing here changes the map files.";
    let bottom = inner.bottom() - 26.0;
    let mut any = false;
    for st in stops {
        if y + 32.0 > bottom {
            break;
        }
        let others: Vec<usize> = serving(&plan, &st.name).into_iter().map(|x| x.0).filter(|o| *o != k).collect();
        if others.is_empty() {
            continue;
        }
        any = true;
        l.ui.p().rect(Rect::new(inner.x, y, inner.w, 1.0), EDGE);
        let sname = omsi_launcher_ui::kit::fit_text(&mut l.ui, &st.name, 13.0, inner.w * 0.38);
        l.ui.text_in(&sname, Rect::new(inner.x + 12.0, y + 5.0, inner.w * 0.38, 22.0), 13.0, Weight::Regular, TEXT, Align::Left);
        let mut bx = inner.x + 12.0 + inner.w * 0.4;
        for o in &others {
            let w = l.ui.width(&plan[*o].name, 13.0, Weight::Bold) + 10.0;
            if bx + w > inner.x + inner.w * 0.62 {
                break;
            }
            l.ui.text_in(&plan[*o].name, Rect::new(bx, y + 5.0, w, 22.0), 13.0, Weight::Bold, line_color(*o), Align::Left);
            bx += w;
        }
        let mut times: Vec<f64> = others.iter().flat_map(|o| departures_at(std::slice::from_ref(&plan[*o]), &st.name, from, 2)).map(|d| d.0).collect();
        times.sort_by(f64::total_cmp);
        times.dedup();
        let next: Vec<String> = times.iter().take(2).map(|t| hm(*t)).collect();
        l.ui.text_in(&next.join(", "), Rect::new(inner.x + 12.0 + inner.w * 0.62, y + 5.0, inner.w * 0.38, 22.0), 13.0, Weight::Regular, TEXT_SOFT, Align::Left);
        y += 33.0;
    }
    if !any {
        l.ui.text_in("No other line calls at this line's stops.", Rect::new(inner.x + 12.0, y + 4.0, inner.w, 22.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
    }
    l.ui.text_in(foot, Rect::new(inner.x, inner.bottom() - 18.0, inner.w, 18.0), 12.0, Weight::Regular, TEXT_FAINT, Align::Left);
}

/// The Trips tab: the chosen line's tours (one at a time), the trips of the tour with when
/// each leaves, and the tools to change them; Save writes every changed line.
fn trips_card(l: &mut Launcher, right: Rect) {
    let ui: &mut Ui = &mut l.ui;
    let tv = &mut l.pages.tt;
    let mut status: Option<(String, bool)> = None;
    let Some(data) = tv.data.as_mut() else { return };
    if data.lines.is_empty() {
        return;
    }
    let line_name = data.lines[tv.line].name.clone();
    // the trips this line can run: those naming it, else those it already runs, else all
    let mut trips: Vec<String> = data.trips.iter().filter(|t| t.line.trim().eq_ignore_ascii_case(line_name.trim())).map(|t| t.name.clone()).collect();
    for t in data.lines[tv.line].tours.iter().flat_map(|t| t.trips.iter()) {
        if !trips.iter().any(|x| x.eq_ignore_ascii_case(&t.trip)) {
            trips.push(t.trip.clone());
        }
    }
    if trips.is_empty() {
        trips = data.trips.iter().map(|t| t.name.clone()).collect();
    }
    trips.sort_by_key(|t| t.to_lowercase());
    let profiles_of = |name: &str| -> Vec<String> { data.trip(name).map(|t| t.profiles.iter().map(|p| p.name.clone()).collect::<Vec<_>>()).filter(|p| !p.is_empty()).unwrap_or_else(|| vec!["0".into()]) };
    let terminus_of = |name: &str| data.trip(name).map(|t| t.terminus.clone()).unwrap_or_default();
    let profiles: Vec<(String, Vec<String>, String)> = trips.iter().map(|t| (t.clone(), profiles_of(t), terminus_of(t))).collect();
    // where each trip starts: its first stop, from the plan
    let first_stop = |trip: &str| -> String {
        tv.plan.iter().flat_map(|li| li.tours.iter()).flat_map(|t| t.trips.iter()).find(|t| t.name.eq_ignore_ascii_case(trip)).map(|t| t.from.clone()).unwrap_or_default()
    };
    let froms: Vec<String> = trips.iter().map(|t| first_stop(t)).collect();
    let line = &mut data.lines[tv.line];
    ui.panel(right);
    let inner = Launcher::card_title(ui, Rect::new(right.x + 18.0, right.y + 14.0, right.w - 36.0, right.h - 28.0), "Trips");
    // the tour: chosen in a list, made, copied or deleted at the top right
    tv.tour = tv.tour.min(line.tours.len().saturating_sub(1));
    let tour_names: Vec<String> = line.tours.iter().map(|t| format!("Tour {} · {} · {} trips", t.number, t.trips.first().map(|x| fmt_time(x.departure)).unwrap_or_default(), t.trips.len())).collect();
    let bw = 34.0;
    let tools = Rect::new(inner.right() - 3.0 * (bw + 6.0), right.y + 10.0, 3.0 * (bw + 6.0), bw);
    let sel_r = Rect::new(tools.x - 10.0 - 260.0f32.min(inner.w * 0.45), right.y + 10.0, 260.0f32.min(inner.w * 0.45), bw);
    if !tour_names.is_empty() {
        let mut t = tv.tour;
        if ui.select("tt-tour", sel_r, &mut t, &tour_names) {
            tv.tour = t;
        }
    } else {
        ui.text_in(&format!("line {line_name} · no tours yet"), sel_r, 12.0, Weight::Regular, TEXT_DIM, Align::Right);
    }
    let icon_at = |k: usize| glam::Vec2::new(tools.x + k as f32 * (bw + 6.0) + bw * 0.5, tools.center().y);
    let off = tv.offset.trim().parse::<f32>().ok();
    if ui.icon_button("tt-new-tour", icon_at(0), 15.0, "add", "New tour") {
        let ai_group = line.tours.last().map(|t| t.ai_group.clone()).unwrap_or_else(|| "Busses".into());
        let extra = line.tours.last().map(|t| t.extra.clone()).unwrap_or_default();
        let first = trips.first().cloned().unwrap_or_default();
        line.tours.push(Tour { number: next_number(&line.tours), ai_group, extra, trips: vec![TourTrip { trip: first, profile: 0, departure: 6.0 * 60.0 }] });
        tv.tour = line.tours.len() - 1;
        tv.dirty.insert(line_name.clone());
    }
    if ui.icon_button("tt-copy-tour", icon_at(1), 15.0, "content_copy", "Copy this tour, later by the minutes of Repeat") {
        match (off, line.tours.get(tv.tour).cloned()) {
            (Some(off), Some(mut t)) => {
                for x in &mut t.trips {
                    x.departure += off;
                }
                t.number = next_number(&line.tours);
                line.tours.push(t);
                tv.tour = line.tours.len() - 1;
                tv.dirty.insert(line_name.clone());
            }
            (None, _) => status = Some(("Type the minutes the copy runs later (negative for earlier) in Repeat every.".into(), true)),
            _ => {}
        }
    }
    if ui.icon_button("tt-del-tour", icon_at(2), 15.0, "delete", "Delete this tour") && tv.tour < line.tours.len() {
        line.tours.remove(tv.tour);
        tv.tour = tv.tour.saturating_sub(1);
        tv.dirty.insert(line_name.clone());
    }
    tv.tour = tv.tour.min(line.tours.len().saturating_sub(1));
    let save_label = match tv.dirty.len() {
        0 => "Saved".to_string(),
        1 => "Save".to_string(),
        n => format!("Save all ({n} lines)"),
    };
    let foot_y = inner.bottom() - ROW;
    let Some(tour) = line.tours.get_mut(tv.tour) else {
        ui.paragraph("This line has no tours yet: make one with + at the top right.", glam::Vec2::new(inner.x, inner.y), inner.w, 13.0, Weight::Regular, TEXT_DIM);
        if ui.button("tt-save", Rect::new(inner.right() - 170.0, foot_y, 170.0, ROW), &save_label, Some("save"), if tv.dirty.is_empty() { ButtonKind::Normal } else { ButtonKind::Primary }) && !tv.dirty.is_empty() {
            let (saved, err) = save_all(tv);
            status = Some(match err {
                Some(e) => (format!("Not saved: {e}"), true),
                None => (format!("{saved} line(s) saved"), false),
            });
        }
        if let Some((s, e)) = status {
            l.state.set_status(s, e);
        }
        return;
    };
    if tv.times_for != Some((tv.line, tv.tour)) || tv.times.len() != tour.trips.len() {
        tv.times = tour.trips.iter().map(|t| fmt_time(t.departure)).collect();
        tv.times_for = Some((tv.line, tv.tour));
    }
    let trip_names: Vec<String> = profiles.iter().map(|p| p.0.clone()).collect();
    // the columns: departs, trip, from, to, profile, bus type
    let w = inner.w - 40.0;
    let cols = [0.0, 0.12, 0.36, 0.56, 0.76, 0.9];
    let col = |k: usize| inner.x + w * cols[k];
    let width = |k: usize| w * (cols.get(k + 1).copied().unwrap_or(1.0) - cols[k]) - GAP;
    // (under the tour's list)
    let top = inner.y.max(sel_r.bottom() + 12.0);
    for (k, t) in ["Departs", "Trip", "From", "To", "Profile", "Bus type"].iter().enumerate() {
        ui.text_in(t, Rect::new(col(k), top, width(k), 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
    }
    let mut remove = None;
    let mut changed = false;
    let rows = Rect::new(inner.x - 6.0, top + 24.0, inner.w + 12.0, foot_y - 12.0 - top - 24.0);
    let times = &mut tv.times;
    let key = format!("{line_name}-{}", tv.tour);
    let group = tour.ai_group.clone();
    ui.scroll_area(&format!("tt-trips-{key}"), rows, &mut |ui, v| {
        let rh = ROW + 4.0;
        for (i, t) in tour.trips.iter_mut().enumerate() {
            let y = v.y + i as f32 * rh;
            let dx = v.x + 6.0 - inner.x;
            ui.p().rect(Rect::new(v.x + 6.0, y - 2.0, v.w - 12.0, 1.0), EDGE);
            let bad = parse_time(&times[i]).is_none();
            let tr = Rect::new(col(0) + dx, y, width(0), ROW - 4.0);
            if ui.text_input(&format!("tt-dep-{key}-{i}"), tr, &mut times[i], "h:mm", None) {
                if let Some(m) = parse_time(&times[i]) {
                    t.departure = m;
                    changed = true;
                }
            }
            if bad {
                ui.p().rounded_border(tr, 6.0, 1.5, DANGER);
            }
            let mut ti = trip_names.iter().position(|n| n.eq_ignore_ascii_case(&t.trip)).unwrap_or(0);
            if ui.select(&format!("tt-trip-{key}-{i}"), Rect::new(col(1) + dx, y, width(1), ROW - 4.0), &mut ti, &trip_names) {
                t.trip = trip_names[ti].clone();
                t.profile = 0;
                changed = true;
            }
            let k = profiles.iter().position(|p| p.0.eq_ignore_ascii_case(&t.trip));
            let (profs, dest, from) = k.map(|k| (profiles[k].1.clone(), profiles[k].2.clone(), froms[k].clone())).unwrap_or_default();
            let cell = |ui: &mut Ui, c: usize, s: &str| {
                let s = omsi_launcher_ui::kit::fit_text(ui, s, 13.0, width(c));
                ui.text_in(&s, Rect::new(col(c) + dx, y, width(c), ROW - 4.0), 13.0, Weight::Regular, TEXT, Align::Left);
            };
            cell(ui, 2, &from);
            cell(ui, 3, &dest);
            let profs = if profs.is_empty() { vec![t.profile.to_string()] } else { profs };
            let mut pi = (t.profile.max(0) as usize).min(profs.len() - 1);
            if ui.select(&format!("tt-prof-{key}-{i}"), Rect::new(col(4) + dx, y, width(4), ROW - 4.0), &mut pi, &profs) {
                t.profile = pi as i32;
                changed = true;
            }
            cell(ui, 5, &group);
            if ui.icon_button(&format!("tt-x-{key}-{i}"), glam::Vec2::new(v.right() - 22.0, y + (ROW - 4.0) * 0.5), 13.0, "delete", "Remove this trip") {
                remove = Some(i);
            }
        }
        tour.trips.len() as f32 * rh
    });
    if let Some(i) = remove {
        tour.trips.remove(i);
        tv.times.remove(i);
        changed = true;
    }
    // the tools under the list
    let mut x = inner.x;
    let bw = 120.0;
    if ui.button("tt-add-trip", Rect::new(x, foot_y, bw, ROW), "Add trip", Some("add"), ButtonKind::Normal) {
        // the next of the alternation (there and back), as far after as the last gap
        let n = tour.trips.len();
        let t = match n {
            0 => TourTrip { trip: trips.first().cloned().unwrap_or_default(), profile: 0, departure: 6.0 * 60.0 },
            1 => TourTrip { departure: tour.trips[0].departure + 30.0, ..tour.trips[0].clone() },
            _ => TourTrip { departure: tour.trips[n - 1].departure + (tour.trips[n - 1].departure - tour.trips[n - 2].departure).max(1.0), ..tour.trips[n - 2].clone() },
        };
        tv.times.push(fmt_time(t.departure));
        tour.trips.push(t);
        changed = true;
    }
    x += bw + 8.0;
    for (k, (label, d)) in [("−1 min", -1.0f32), ("+1 min", 1.0)].iter().enumerate() {
        if ui.button(&format!("tt-shift-{k}"), Rect::new(x, foot_y, 80.0, ROW), label, None, ButtonKind::Normal) {
            for t in &mut tour.trips {
                t.departure += d;
            }
            tv.times_for = None;
            changed = true;
        }
        x += 88.0;
    }
    // the whole day at once: copies of the tour every <min> minutes up to a last departure
    ui.text_in("Repeat every", Rect::new(x, foot_y, 90.0, ROW), 13.0, Weight::Regular, TEXT_SOFT, Align::Left);
    x += 92.0;
    ui.text_input("tt-offset", Rect::new(x, foot_y, 56.0, ROW), &mut tv.offset, "min", None);
    x += 62.0;
    ui.text_in("min until", Rect::new(x, foot_y, 70.0, ROW), 13.0, Weight::Regular, TEXT_SOFT, Align::Left);
    x += 70.0;
    ui.text_input("tt-until", Rect::new(x, foot_y, 70.0, ROW), &mut tv.until, "h:mm", None);
    x += 76.0;
    if ui.icon_button("tt-repeat", glam::Vec2::new(x + 17.0, foot_y + ROW * 0.5), 15.0, "autorenew", "Repeat the tour") {
        let base = line.tours.get(tv.tour).cloned();
        match (off.filter(|o| *o >= 1.0), parse_time(&tv.until), base) {
            (Some(every), Some(until), Some(base)) => {
                let made = repeat_tour(&mut line.tours, &base, every, until);
                if made > 0 {
                    tv.tour = line.tours.len() - 1;
                    tv.dirty.insert(line_name.clone());
                }
                status = Some((format!("{made} tour(s) made, every {every} min up to {}", fmt_time(until)), false));
            }
            (None, _, _) => status = Some(("Type the minutes between the tours (1 or more)".into(), true)),
            (_, None, _) => status = Some(("Type the last departure as h:mm".into(), true)),
            _ => {}
        }
    }
    if changed {
        tv.dirty.insert(line_name.clone());
    }
    let n = tv.dirty.len();
    let save_label = match n {
        0 => "Saved".to_string(),
        1 => "Save".to_string(),
        n => format!("Save all ({n} lines)"),
    };
    let save_r = Rect::new(inner.right() - 150.0, foot_y, 150.0, ROW);
    if ui.button("tt-save", save_r, &save_label, Some("save"), if n > 0 { ButtonKind::Primary } else { ButtonKind::Normal }) && n > 0 {
        if tv.times.iter().any(|t| parse_time(t).is_none()) {
            status = Some(("A departure is not a time (h:mm).".into(), true));
        } else {
            let (saved, err) = save_all(tv);
            tv.times_for = None;
            // (what the lines run is read again: the map and the diagram show the new times)
            tv.plan_for = None;
            status = Some(match err {
                Some(e) => (format!("Not saved: {e}"), true),
                None => (format!("{saved} line(s) saved"), false),
            });
        }
    }
    if let Some((s, err)) = status {
        l.state.set_status(s, err);
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(parse_time("4:07"), Some(247.0));
        assert_eq!(parse_time("24:10"), Some(1450.0));
        assert_eq!(parse_time("4:7:30"), Some(247.5));
        assert_eq!(parse_time("4"), None);
        assert_eq!(parse_time("4:61"), None);
        assert_eq!(fmt_time(247.0), "4:07");
        assert_eq!(fmt_time(247.5), "4:07:30");
    }

    #[test]
    fn numbers() {
        let t = |n: &str| Tour { number: n.into(), ..Default::default() };
        assert_eq!(next_number(&[t("1"), t("7"), t("x")]), "8");
        assert_eq!(next_number(&[]), "1");
    }
}

#[cfg(test)]
mod repeat_tests {
    use super::*;

    #[test]
    fn a_tour_every_twenty_minutes_until_eight() {
        let base = Tour { number: "1".into(), ai_group: "Busses".into(), extra: String::new(), trips: vec![TourTrip { trip: "a".into(), profile: 0, departure: 6.0 * 60.0 }, TourTrip { trip: "b".into(), profile: 0, departure: 6.5 * 60.0 }] };
        let mut tours = vec![base.clone()];
        let made = repeat_tour(&mut tours, &base, 20.0, 8.0 * 60.0);
        // 6:20, 6:40 … 8:00
        assert_eq!(made, 6);
        assert_eq!(tours.last().unwrap().trips[0].departure, 8.0 * 60.0);
        assert_eq!(tours.last().unwrap().trips[1].departure, 8.5 * 60.0);
        assert_eq!(tours.last().unwrap().number, "7");
    }
}

#[cfg(test)]
mod plan_tests {
    use super::*;

    /// A trip of `stops` leaving at `dep` (seconds), two minutes from stop to stop.
    fn trip(name: &str, dep: f64, stops: &[&str]) -> core::TripInfo {
        let stops: Vec<core::StopInfo> = stops.iter().enumerate().map(|(i, s)| core::StopInfo { id: i as i64, name: s.to_string(), arr: dep + i as f64 * 120.0, dep: dep + i as f64 * 120.0 }).collect();
        core::TripInfo {
            name: name.into(),
            index: 1,
            line: String::new(),
            from: stops[0].name.clone(),
            terminus: stops.last().unwrap().name.clone(),
            departure: dep,
            arrival: stops.last().unwrap().arr,
            stops,
            km: 0.0,
        }
    }

    fn line(name: &str, runs: bool, trips: Vec<core::TripInfo>) -> core::LineInfo {
        let tour = core::TourInfo { number: "1".into(), ai_group: "Busses".into(), first: 0.0, last: 0.0, days: "daily".into(), runs, next_run: None, trips };
        core::LineInfo { name: name.into(), user_allowed: true, termini: Vec::new(), tours: vec![tour] }
    }

    fn town() -> Vec<core::LineInfo> {
        vec![
            line("12", true, vec![trip("out", 3600.0, &["North", "Market", "Hall", "East"]), trip("back", 4200.0, &["East", "Hall", "North"]), trip("short", 4800.0, &["North", "Market"])]),
            line("7", true, vec![trip("a", 3700.0, &["Station", "Hall", "Uni"])]),
            line("N3", false, vec![trip("n", 3650.0, &["North", "Hall", "Park"])]),
        ]
    }

    #[test]
    fn a_direction_is_drawn_from_its_longest_trip() {
        let lines = town();
        assert_eq!(route_of(&lines[0], 0).unwrap().name, "out");
        assert_eq!(route_of(&lines[0], 1).unwrap().name, "back");
        // (a short working ends elsewhere: a third direction; and the directions go round)
        assert_eq!(route_of(&lines[0], 2).unwrap().name, "short");
        assert_eq!(route_of(&lines[0], 4).unwrap().name, "back");
        assert!(route_of(&line("x", true, Vec::new()), 0).is_none());
    }

    #[test]
    fn departures_are_of_running_tours_and_not_from_the_last_stop() {
        let lines = town();
        let d = departures_at(&lines, "hall", 0.0, 10);
        // 12 out at 3840, 7 at 3820, 12 back at 4320; N3 does not run that day
        assert_eq!(d.iter().map(|x| (x.0 as i64, x.1)).collect::<Vec<_>>(), vec![(3820, 1), (3840, 0), (4320, 0)]);
        assert_eq!(d[0].2, "Uni");
        // nothing leaves from where the trip ends, and the earlier ones are gone
        assert!(departures_at(&lines, "East", 4000.0, 10).iter().all(|x| x.2 == "North"));
        assert_eq!(departures_at(&lines, "Hall", 4000.0, 10).len(), 1);
        assert_eq!(departures_at(&lines, "Hall", 0.0, 2).len(), 2);
    }

    #[test]
    fn a_stop_is_served_by_the_lines_that_call_there() {
        let lines = town();
        let s = serving(&lines, "Hall");
        assert_eq!(s.iter().map(|x| x.0).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(s[0].1, vec!["East".to_string(), "North".to_string()]);
        // (a line that only ends there does not serve it)
        assert_eq!(serving(&lines, "Uni"), Vec::new());
        assert_eq!(serving(&lines, "market").iter().map(|x| x.0).collect::<Vec<_>>(), vec![0]);
    }
}

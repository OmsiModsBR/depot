//! The Maps page: the library of maps, what is known about the chosen one, its start points
//! and what was found of its files. Read like the Vehicles page, on a thread of its own.

use crate::kit::{self, card, clipped, dot, shorten, text, Status};
use crate::look::*;
use omsi_launcher_lib::MapInfo;
use omsi_ui::paint::Align;
use omsi_ui::widgets::{matches, Ui};
use omsi_ui::{Rect, Weight};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

const DESCRIPTION_CHARS: usize = 1200;

enum Catalog {
    NotStarted,
    Loading(Receiver<Result<Vec<MapInfo>, String>>),
    Ready(Rc<Vec<MapInfo>>),
    Failed(String),
}

pub struct MapsView {
    catalog: Catalog,
    query: String,
    selected: usize,
    /// The start point picked in the map's list; `None` is automatic (the game's choice).
    entry: Option<usize>,
    /// A map (its file) and start point to show once the catalog is there (`select`).
    pending: Option<(String, i32)>,
    /// "Drive on this map" was clicked: the map's file and the start point's place in its
    /// list, -1 for automatic (`take_use`).
    used: Option<(String, i32)>,
    /// The chosen map's file (for the host to find its picture by).
    current: Option<String>,
    /// The picture from the chosen map's folder, on the GPU: its texture and width over height.
    map_picture: Option<(usize, f32)>,
}

impl Default for MapsView {
    fn default() -> Self {
        MapsView {
            catalog: Catalog::NotStarted,
            query: String::new(),
            selected: 0,
            entry: None,
            pending: None,
            used: None,
            current: None,
            map_picture: None,
        }
    }
}

impl MapsView {
    /// A page showing `list` (pictures and tests).
    pub fn with_catalog(list: Vec<MapInfo>) -> MapsView {
        MapsView {
            catalog: Catalog::Ready(Rc::new(list)),
            ..Default::default()
        }
    }

    /// The file of the map the page shows (after `draw`), for its picture.
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// The picture from that map's folder (its texture and width over height), when it has one.
    pub fn set_picture(&mut self, p: Option<(usize, f32)>) {
        self.map_picture = p;
    }

    /// The map chosen in the library, and its start point (`None`: automatic).
    pub fn chosen(&self) -> (usize, Option<usize>) {
        (self.selected, self.entry)
    }

    /// Take `list` as the catalog (the host's, which it reads again after Setup or an
    /// install). The map shown stays the one it was when it is still in the list.
    pub fn set_catalog(&mut self, list: Vec<MapInfo>) {
        let keep = match &self.catalog {
            Catalog::Ready(old) => old.get(self.selected).map(|m| {
                let entry = self.entry.map(|e| e as i32).unwrap_or(-1);
                (m.file.clone(), entry)
            }),
            _ => None,
        };
        self.catalog = Catalog::Ready(Rc::new(list));
        if let (Some(keep), None) = (keep, &self.pending) {
            self.pending = Some(keep);
        }
        self.selected = 0;
        self.entry = None;
        self.apply_pending();
    }

    /// How many maps the catalog holds, once it is read.
    pub fn catalog_len(&self) -> Option<usize> {
        match &self.catalog {
            Catalog::Ready(list) => Some(list.len()),
            _ => None,
        }
    }

    /// Read the catalog again on the next frame when reading it failed (the OMSI 2 folder
    /// may have been chosen since).
    pub fn retry_if_failed(&mut self) {
        if matches!(self.catalog, Catalog::Failed(_)) {
            self.catalog = Catalog::NotStarted;
        }
    }

    /// Show the map whose file is `file` with the start point at `entry` in its list (-1:
    /// automatic), when it is in the catalog; one not yet read is shown once it is.
    pub fn select(&mut self, file: &str, entry: i32) {
        self.pending = Some((file.to_string(), entry));
        self.apply_pending();
    }

    fn apply_pending(&mut self) {
        let Catalog::Ready(list) = &self.catalog else {
            return;
        };
        let Some((file, entry)) = self.pending.take() else {
            return;
        };
        if let Some(i) = list.iter().position(|m| m.file.eq_ignore_ascii_case(&file)) {
            self.selected = i;
            let known = usize::try_from(entry)
                .ok()
                .filter(|&e| e < list[i].entry_points.len());
            self.entry = known;
            self.query.clear();
        }
    }

    /// The map (its file) and start point "Drive on this map" was clicked for, once.
    pub fn take_use(&mut self) -> Option<(String, i32)> {
        self.used.take()
    }

    fn load(&mut self) {
        if !matches!(self.catalog, Catalog::NotStarted) {
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(omsi_launcher_lib::list_maps().map_err(|e| format!("{e:#}")));
        });
        self.catalog = Catalog::Loading(rx);
    }

    fn poll(&mut self) {
        if let Catalog::Loading(rx) = &self.catalog {
            match rx.try_recv() {
                Ok(Ok(list)) => self.catalog = Catalog::Ready(Rc::new(list)),
                Ok(Err(e)) => self.catalog = Catalog::Failed(e),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.catalog = Catalog::Failed("the catalog reader stopped".into())
                }
            }
        }
    }

    /// Whether the page still waits for something (the window keeps drawing).
    pub fn busy(&self) -> bool {
        matches!(self.catalog, Catalog::Loading(_))
    }

    fn shown(&self, list: &[MapInfo]) -> Vec<usize> {
        (0..list.len())
            .filter(|&i| {
                self.query.trim().is_empty()
                    || matches(
                        &format!("{} {}", list[i].name, list[i].friendly),
                        &self.query,
                    )
            })
            .collect()
    }

    pub fn draw(&mut self, ui: &mut Ui, body: Rect) {
        self.load();
        self.poll();
        let list = match &self.catalog {
            Catalog::Ready(l) => Rc::clone(l),
            Catalog::Failed(e) => {
                let r = Rect::new(body.x, body.y + 137.0, body.w, 120.0);
                card(ui, r, "Maps could not be listed", None);
                clipped(
                    ui,
                    e,
                    r.x + 19.0,
                    r.y + 62.0,
                    r.w - 38.0,
                    14.0,
                    Weight::Regular,
                    MUTED,
                );
                clipped(
                    ui,
                    "Check the original installation in Setup.",
                    r.x + 19.0,
                    r.y + 90.0,
                    r.w - 38.0,
                    14.0,
                    Weight::Regular,
                    MUTED,
                );
                return;
            }
            _ => {
                let r = Rect::new(body.x, body.y + 137.0, body.w, 90.0);
                card(ui, r, "Map library", None);
                clipped(
                    ui,
                    "Reading the maps\u{2026}",
                    r.x + 19.0,
                    r.y + 62.0,
                    r.w - 38.0,
                    14.0,
                    Weight::Regular,
                    MUTED,
                );
                return;
            }
        };
        self.apply_pending();
        let shown = self.shown(&list);
        if !shown.is_empty() && !shown.contains(&self.selected) {
            self.selected = shown[0];
            self.entry = None;
        }
        let top = body.y + 137.0;
        let rows_h = (body.h - 137.0 - 24.0).max(300.0);
        let upper_h = (rows_h * 0.6).floor();
        let lower_h = rows_h - upper_h - 16.0;
        let lib = Rect::new(body.x, top, kit::fit(335.0, body.w), upper_h);
        let files_w = kit::fit(392.0, body.w);
        let files = Rect::new(body.right() - files_w, top, files_w, upper_h);
        let mid = Rect::new(
            lib.right() + 16.0,
            top,
            files.x - lib.right() - 32.0,
            upper_h,
        );
        self.library(ui, lib, &list, &shown);
        let chosen = kit::chosen(&list, &shown, self.selected);
        self.current = chosen.map(|m| m.file.clone());
        self.picture(ui, mid, chosen);
        self.files(ui, files, chosen);
        let y2 = top + upper_h + 16.0;
        let launch_w = kit::fit(426.0, body.w);
        let launch = Rect::new(body.right() - launch_w, y2, launch_w, lower_h);
        let about_w = (launch.x - 16.0 - body.x) * 0.5 - 8.0;
        let about = Rect::new(body.x, y2, about_w, lower_h);
        let starts = Rect::new(about.right() + 16.0, y2, about_w, lower_h);
        self.about(ui, about, chosen);
        self.starts(ui, starts, chosen);
        self.launch(ui, launch, chosen);
    }

    fn library(&mut self, ui: &mut Ui, r: Rect, list: &[MapInfo], shown: &[usize]) {
        card(
            ui,
            r,
            "Map library",
            Some(&format!("{} of {}", shown.len(), list.len())),
        );
        let search = Rect::new(r.x + 15.0, r.y + 44.0, r.w - 30.0, 36.0);
        ui.text_input(
            "maps:search",
            search,
            &mut self.query,
            "Search maps...",
            Some("search"),
        );
        let area = Rect::new(r.x + 8.0, r.y + 92.0, r.w - 16.0, r.h - 100.0);
        let mut picked = None;
        let selected = self.selected;
        ui.scroll_area("maps:list", area, &mut |ui, view| {
            let mut y = view.y;
            for &i in shown {
                let m = &list[i];
                let row = Rect::new(view.x, y, view.w - 8.0, 72.0);
                if ui.row(&format!("maps:row:{i}"), row, i == selected) {
                    picked = Some(i);
                }
                if ui.rect_visible(row) {
                    clipped(
                        ui,
                        &map_title(m),
                        row.x + 16.0,
                        row.y + 22.0,
                        row.w - 40.0,
                        15.0,
                        Weight::Bold,
                        FG,
                    );
                    let (status, label) = map_status(m);
                    dot(ui, row.x + 21.0, row.y + 48.0, status);
                    text(
                        ui,
                        label,
                        row.x + 34.0,
                        row.y + 48.0,
                        12.0,
                        Weight::Regular,
                        status.color(),
                    );
                }
                y += 72.0;
            }
            y - view.y
        });
        if let Some(i) = picked {
            self.selected = i;
            self.entry = None;
        }
    }

    fn picture(&mut self, ui: &mut Ui, r: Rect, m: Option<&MapInfo>) {
        card(ui, r, "Map", m.map(map_title).as_deref());
        let inner = Rect::new(r.x + 1.0, r.y + 44.0, r.w - 2.0, r.h - 45.0);
        ui.p().rect(inner, PANEL_2);
        if m.is_none() {
            return;
        }
        match self.map_picture {
            Some((tex, aspect)) => {
                ui.image_cover(inner, tex, aspect, 1.0);
                // (as the concept: where the picture comes from, top left over it)
                let t = "picture from the map's folder";
                let px = 11.5 * TEXT_SCALE;
                let w = ui.width(t, px, Weight::Regular) + 16.0;
                let b = Rect::new(inner.x + 12.0, inner.y + 10.0, w, 22.0);
                ui.p()
                    .rounded(b, 4.0, omsi_ui::Color::rgba(10, 10, 12, 0.72));
                ui.text_in(t, b, px, Weight::Regular, MUTED, Align::Center);
            }
            None => {
                ui.text_in(
                    "This map has no picture in its folder.",
                    inner.inset(24.0),
                    14.0 * TEXT_SCALE,
                    Weight::Regular,
                    MUTED,
                    Align::Center,
                );
            }
        }
    }

    fn files(&mut self, ui: &mut Ui, r: Rect, m: Option<&MapInfo>) {
        card(ui, r, "Files found", None);
        let Some(m) = m else { return };
        let mut y = r.y + 58.0;
        for (label, status, note) in file_status(m) {
            dot(ui, r.x + 24.0, y, status);
            text(ui, label, r.x + 42.0, y, 13.5, Weight::Regular, FG);
            kit::right(ui, &note, r.right() - 19.0, y, 12.0, Weight::Regular, MUTED);
            y += 30.0;
        }
    }

    fn about(&mut self, ui: &mut Ui, r: Rect, m: Option<&MapInfo>) {
        card(ui, r, "About the selected map", None);
        let Some(m) = m else { return };
        let desc = shorten(m.description.trim(), DESCRIPTION_CHARS);
        let desc = if desc.is_empty() {
            "The author wrote no description.".to_string()
        } else {
            desc
        };
        let box_ = Rect::new(r.x + 15.0, r.y + 44.0, r.w - 30.0, (r.h - 58.0).max(0.0));
        ui.p().rounded(box_, 6.0, PANEL_2);
        ui.push_clip(box_.inset(1.0), 5.0);
        ui.paragraph(
            &desc,
            glam::Vec2::new(box_.x + 12.0, box_.y + 18.0),
            box_.w - 24.0,
            12.5 * TEXT_SCALE,
            Weight::Regular,
            FG,
        );
        ui.pop_clip();
    }

    fn starts(&mut self, ui: &mut Ui, r: Rect, m: Option<&MapInfo>) {
        let n = m.map(|m| m.entry_points.len()).unwrap_or(0);
        card(ui, r, "Start points", Some(&format!("{n} found")));
        let Some(m) = m else { return };
        if m.entry_points.is_empty() {
            clipped(
                ui,
                "This map lists no start point.",
                r.x + 19.0,
                r.y + 62.0,
                r.w - 38.0,
                13.0,
                Weight::Regular,
                MUTED,
            );
            return;
        }
        let area = Rect::new(r.x + 8.0, r.y + 44.0, r.w - 16.0, r.h - 52.0);
        let mut picked = None;
        let chosen = self.entry;
        ui.scroll_area("maps:starts", area, &mut |ui, view| {
            let mut y = view.y;
            let rows = std::iter::once((None, "Automatic", "maps:start:auto".to_string())).chain(
                m.entry_points
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (Some(i), e.name.as_str(), format!("maps:start:{i}"))),
            );
            for (i, name, id) in rows {
                let row = Rect::new(view.x, y, view.w - 8.0, 36.0);
                if ui.row(&id, row, i == chosen) {
                    picked = Some(i);
                }
                clipped(
                    ui,
                    name,
                    row.x + 16.0,
                    row.y + 18.0,
                    row.w - 32.0,
                    13.5,
                    Weight::Medium,
                    if i == chosen { FG } else { MUTED },
                );
                y += 36.0;
            }
            y - view.y
        });
        if let Some(i) = picked {
            self.entry = i;
        }
    }

    fn launch(&mut self, ui: &mut Ui, r: Rect, m: Option<&MapInfo>) {
        card(ui, r, "Launch preparation", None);
        let map = m.map(map_title).unwrap_or_default();
        let start = match self.entry {
            Some(e) => m
                .and_then(|m| m.entry_points.get(e))
                .map(|e| e.name.clone())
                .unwrap_or_default(),
            None => "Automatic".to_string(),
        };
        let mut y = r.y + 58.0;
        for (k, val) in [("Map", map), ("Start point", start)] {
            ui.p()
                .rect(Rect::new(r.x + 1.0, y - 20.0, r.w - 2.0, 1.0), LINE);
            text(ui, k, r.x + 19.0, y, 13.0, Weight::Regular, MUTED);
            clipped(
                ui,
                &val,
                r.x + 150.0,
                y,
                r.w - 170.0,
                13.0,
                Weight::Regular,
                FG,
            );
            y += 40.0;
        }
        let b = Rect::new(r.x + 19.0, r.bottom() - 100.0, r.w - 38.0, 48.0);
        if kit::action(
            ui,
            "maps:use",
            b,
            "DRIVE ON THIS MAP",
            "Takes this map and start point to the Drive page.",
            m.is_some(),
        ) {
            if let Some(m) = m {
                let entry = self.entry.map(|e| e as i32).unwrap_or(-1);
                self.used = Some((m.file.clone(), entry));
            }
        }
    }
}

/// The name a map is shown under: the author's friendly name, else the folder's.
fn map_title(m: &MapInfo) -> String {
    let f = m.friendly.trim();
    if f.is_empty() {
        m.name.clone()
    } else {
        f.to_string()
    }
}

fn map_status(m: &MapInfo) -> (Status, &'static str) {
    if m.entry_points.is_empty() {
        (Status::Warning, "No start point listed")
    } else if m.installed {
        (Status::Found, "Installed mod")
    } else {
        (Status::Found, "Original content")
    }
}

/// What the catalog tells of a map's files. Roads, scenery objects and the timetable are not
/// opened by the catalog, so they are `Unverified`, never "found".
fn file_status(m: &MapInfo) -> Vec<(&'static str, Status, String)> {
    let mut rows = vec![(
        "Map file",
        if m.file.is_empty() {
            Status::Missing
        } else {
            Status::Found
        },
        "global.cfg".to_string(),
    )];
    rows.push(if m.entry_points.is_empty() {
        ("Start points", Status::Warning, "none listed".into())
    } else {
        (
            "Start points",
            Status::Found,
            format!("{} found", m.entry_points.len()),
        )
    });
    rows.push(if m.hof.is_empty() {
        (
            "Depot file",
            Status::Warning,
            "none for the map's buses".into(),
        )
    } else {
        ("Depot file", Status::Found, shorten(&m.hof, 24))
    });
    rows.push((
        "Roads, scenery, timetable",
        Status::Unverified,
        "not checked yet".into(),
    ));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample;

    #[test]
    fn a_map_without_start_points_or_depot_file_is_a_warning() {
        let list = sample::maps();
        assert_eq!(map_status(&list[2]).0, Status::Warning);
        let rows = file_status(&list[2]);
        assert!(rows.iter().filter(|r| r.1 == Status::Warning).count() == 2);
        assert_eq!(rows.last().unwrap().1, Status::Unverified);
        assert_eq!(map_status(&list[0]).1, "Original content");
        assert_eq!(map_status(&list[1]).1, "Installed mod");
    }

    #[test]
    fn the_search_keeps_the_maps_that_match() {
        let list = sample::maps();
        let mut v = MapsView::with_catalog(list.clone());
        assert_eq!(v.shown(&list).len(), 3);
        v.query = "city".into();
        assert_eq!(v.shown(&list), vec![1]);
        // a search that matches nothing leaves no map chosen, not the first one
        v.query = "no such map".into();
        let shown = v.shown(&list);
        assert!(shown.is_empty());
        assert!(kit::chosen(&list, &shown, v.selected).is_none());
    }
}

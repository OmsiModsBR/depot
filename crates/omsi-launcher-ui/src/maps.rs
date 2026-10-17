//! The Maps page: the library of maps, what is known about the chosen one, its start points
//! and what was found of its files. Read like the Vehicles page, on a thread of its own.

use crate::kit::{self, card, clipped, dot, shorten, text, Status};
use crate::look::*;
use omsi_launcher_lib::MapInfo;
use omsi_ui::paint::Align;
use omsi_ui::widgets::{matches, Ui};
use omsi_ui::{Rect, Weight};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

const DESCRIPTION_CHARS: usize = 1200;

enum Catalog {
    NotStarted,
    Loading(Receiver<Result<Vec<MapInfo>, String>>),
    Ready(Vec<MapInfo>),
    Failed(String),
}

pub struct MapsView {
    catalog: Catalog,
    query: String,
    selected: usize,
    entry: usize,
}

impl Default for MapsView {
    fn default() -> Self {
        MapsView {
            catalog: Catalog::NotStarted,
            query: String::new(),
            selected: 0,
            entry: 0,
        }
    }
}

impl MapsView {
    /// A page showing `list` (pictures and tests).
    pub fn with_catalog(list: Vec<MapInfo>) -> MapsView {
        MapsView {
            catalog: Catalog::Ready(list),
            ..Default::default()
        }
    }

    /// The map chosen in the library, and its start point.
    pub fn chosen(&self) -> (usize, usize) {
        (self.selected, self.entry)
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
                Ok(Ok(list)) => self.catalog = Catalog::Ready(list),
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
            Catalog::Ready(l) => l.clone(),
            Catalog::Failed(e) => {
                let r = Rect::new(body.x, body.y + 100.0, body.w, 120.0);
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
                let r = Rect::new(body.x, body.y + 100.0, body.w, 90.0);
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
        let shown = self.shown(&list);
        if !shown.contains(&self.selected) {
            self.selected = shown.first().copied().unwrap_or(0);
            self.entry = 0;
        }
        let top = body.y + 100.0;
        let rows_h = (body.h - 100.0 - 24.0).max(300.0);
        let upper_h = (rows_h * 0.6).floor();
        let lower_h = rows_h - upper_h - 16.0;
        let lib = Rect::new(body.x, top, 335.0, upper_h);
        let files_w = 392.0;
        let files = Rect::new(body.right() - files_w, top, files_w, upper_h);
        let mid = Rect::new(
            lib.right() + 16.0,
            top,
            files.x - lib.right() - 32.0,
            upper_h,
        );
        self.library(ui, lib, &list, &shown);
        let chosen = list.get(self.selected);
        self.picture(ui, mid, chosen);
        self.files(ui, files, chosen);
        let y2 = top + upper_h + 16.0;
        let launch_w = 426.0;
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
            self.entry = 0;
        }
    }

    fn picture(&mut self, ui: &mut Ui, r: Rect, m: Option<&MapInfo>) {
        card(ui, r, "Map", m.map(map_title).as_deref());
        let inner = Rect::new(r.x + 1.0, r.y + 44.0, r.w - 2.0, r.h - 45.0);
        ui.p().rect(inner, PANEL_2);
        ui.text_in(
            "No map picture yet. The picture of the map comes in a later step.",
            inner.inset(24.0),
            14.0 * TEXT_SCALE,
            Weight::Regular,
            MUTED,
            Align::Center,
        );
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
            for (i, e) in m.entry_points.iter().enumerate() {
                let row = Rect::new(view.x, y, view.w - 8.0, 36.0);
                if ui.row(&format!("maps:start:{i}"), row, i == chosen) {
                    picked = Some(i);
                }
                clipped(
                    ui,
                    &e.name,
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
        let start = m
            .and_then(|m| m.entry_points.get(self.entry))
            .map(|e| e.name.clone())
            .unwrap_or_default();
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
        ui.p().rounded(b, 6.0, PANEL_2);
        ui.text_in(
            "START DRIVING",
            b,
            14.0 * TEXT_SCALE,
            Weight::Bold,
            MUTED,
            Align::Center,
        );
        clipped(
            ui,
            "Starting a session comes with the Drive page.",
            r.x + 19.0,
            r.bottom() - 30.0,
            r.w - 38.0,
            12.0,
            Weight::Regular,
            MUTED,
        );
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
    }
}

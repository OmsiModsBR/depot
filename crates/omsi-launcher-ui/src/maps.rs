//! The Maps page, where a session starts: the library of maps on the left, the chosen map's
//! picture and its author's description in the middle, and on the right the start (the date,
//! the kind of session, where to start and the weather), which the host draws into
//! `start_area` - it holds the session's choice. Read like the Vehicles page, on a thread of
//! its own.

use crate::kit::{self, card, clipped, dot, shorten, text, Status};
use crate::look::*;
use crate::shell::BODY_TOP;
use omsi_launcher_lib::MapInfo;
use omsi_ui::paint::Align;
use omsi_ui::widgets::{matches, ButtonKind, Ui};
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
    /// A map (its file) to show once the catalog is there (`select`).
    pending: Option<String>,
    /// The chosen map's file (for the host to find its picture by).
    current: Option<String>,
    /// The picture from the chosen map's folder, on the GPU: its texture and width over height.
    map_picture: Option<(usize, f32)>,
    /// That picture is still being looked for or read.
    picture_reading: bool,
    /// Where the start's card goes (the host draws it), once the library is drawn.
    start_area: Option<Rect>,
    /// "Explore map" was clicked (`take_explore`).
    explore: bool,
}

impl Default for MapsView {
    fn default() -> Self {
        MapsView {
            catalog: Catalog::NotStarted,
            query: String::new(),
            selected: 0,
            pending: None,
            current: None,
            map_picture: None,
            picture_reading: false,
            start_area: None,
            explore: false,
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

    /// The picture from that map's folder (its texture and width over height), when it has one,
    /// and whether it is still being read.
    pub fn set_picture(&mut self, p: Option<(usize, f32)>, reading: bool) {
        self.map_picture = p;
        self.picture_reading = reading;
    }

    /// The map chosen in the library (its place in the catalog).
    pub fn chosen(&self) -> usize {
        self.selected
    }

    /// Where the start's card goes, in the frame just drawn (none while the catalog is read
    /// or could not be).
    pub fn start_area(&self) -> Option<Rect> {
        self.start_area
    }

    /// "Explore map" was clicked, once.
    pub fn take_explore(&mut self) -> bool {
        std::mem::take(&mut self.explore)
    }

    /// Take `list` as the catalog (the host's, which it reads again after Setup or an
    /// install). The map shown stays the one it was when it is still in the list.
    pub fn set_catalog(&mut self, list: Vec<MapInfo>) {
        let keep = match &self.catalog {
            Catalog::Ready(old) => old.get(self.selected).map(|m| m.file.clone()),
            _ => None,
        };
        self.catalog = Catalog::Ready(Rc::new(list));
        if let (Some(keep), None) = (keep, &self.pending) {
            self.pending = Some(keep);
        }
        self.selected = 0;
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

    /// Show the map whose file is `file` when it is in the catalog; one not yet read is shown
    /// once it is.
    pub fn select(&mut self, file: &str) {
        self.pending = Some(file.to_string());
        self.apply_pending();
    }

    fn apply_pending(&mut self) {
        let Catalog::Ready(list) = &self.catalog else {
            return;
        };
        let Some(file) = self.pending.take() else {
            return;
        };
        if let Some(i) = list.iter().position(|m| m.file.eq_ignore_ascii_case(&file)) {
            self.selected = i;
            self.query.clear();
        }
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
        self.start_area = None;
        let list = match &self.catalog {
            Catalog::Ready(l) => Rc::clone(l),
            Catalog::Failed(e) => {
                let r = Rect::new(body.x, body.y + BODY_TOP, body.w, 120.0);
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
                let r = Rect::new(body.x, body.y + BODY_TOP, body.w, 90.0);
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
        }
        // (three columns: the library, the map and its description, the start)
        let top = body.y + BODY_TOP;
        let rows_h = (body.h - BODY_TOP - 24.0).max(300.0);
        let lib = Rect::new(body.x, top, kit::fit(290.0, body.w), rows_h);
        let start_w = kit::fit(320.0, body.w);
        let start = Rect::new(body.right() - start_w, top, start_w, rows_h);
        let mid_x = lib.right() + 14.0;
        let mid_w = start.x - 14.0 - mid_x;
        let pic_h = (rows_h * 0.56).floor();
        let picture = Rect::new(mid_x, top, mid_w, pic_h);
        let about = Rect::new(mid_x, top + pic_h + 14.0, mid_w, rows_h - pic_h - 14.0);
        self.library(ui, lib, &list, &shown);
        let chosen = kit::chosen(&list, &shown, self.selected);
        self.current = chosen.map(|m| m.file.clone());
        self.picture(ui, picture, chosen);
        self.about(ui, about, chosen);
        self.start_area = chosen.map(|_| start);
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
                        row.y + 24.0,
                        row.w - 40.0,
                        14.5,
                        Weight::Bold,
                        FG,
                    );
                    let (status, label) = map_status(m);
                    dot(ui, row.x + 21.0, row.y + 49.0, status);
                    text(
                        ui,
                        &label,
                        row.x + 34.0,
                        row.y + 49.0,
                        12.0,
                        Weight::Regular,
                        status.color(),
                    );
                    ui.icon(
                        "o-chevron",
                        glam::Vec2::new(row.right() - 16.0, row.center().y),
                        14.0,
                        MUTED,
                    );
                }
                y += 72.0;
            }
            y - view.y
        });
        if let Some(i) = picked {
            self.selected = i;
        }
    }

    fn picture(&mut self, ui: &mut Ui, r: Rect, m: Option<&MapInfo>) {
        card(ui, r, "Map", None);
        // (the map's own radar view, with its streets and start points, in a window over the
        // page)
        let bw = ui.width("EXPLORE MAP", 12.5, Weight::Bold) + 52.0;
        let b = Rect::new(r.right() - 15.0 - bw, r.y + 10.0, bw, 30.0);
        if m.is_some()
            && ui.button(
                "maps:explore",
                b,
                "EXPLORE MAP",
                Some("o-pin"),
                ButtonKind::Normal,
            )
        {
            self.explore = true;
        }
        if m.is_some() && self.map_picture.is_some() {
            let note = "Picture from the map's folder";
            let room = b.x - 12.0 - (r.x + 80.0);
            let note = kit::fit_text(ui, note, 12.0 * TEXT_SCALE, room);
            kit::right(
                ui,
                &note,
                b.x - 12.0,
                r.y + 22.0,
                12.0,
                Weight::Regular,
                MUTED,
            );
        }
        let inner = Rect::new(r.x + 1.0, r.y + 50.0, r.w - 2.0, r.h - 51.0);
        ui.p().rect(inner, PANEL_2);
        if m.is_none() {
            return;
        }
        match self.map_picture {
            Some((tex, aspect)) => {
                // (the whole picture, not cut: it is often the map's overview)
                let want = inner.w / inner.h.max(1.0);
                let (w, h) = if aspect > want {
                    (inner.w, inner.w / aspect.max(0.01))
                } else {
                    (inner.h * aspect, inner.h)
                };
                let pic = Rect::new(
                    inner.x + (inner.w - w) * 0.5,
                    inner.y + (inner.h - h) * 0.5,
                    w,
                    h,
                );
                ui.image(pic, tex, 0.0);
            }
            None => {
                ui.text_in(
                    if self.picture_reading {
                        "Loading…"
                    } else {
                        "No picture found for this map."
                    },
                    inner.inset(24.0),
                    14.0 * TEXT_SCALE,
                    Weight::Regular,
                    MUTED,
                    Align::Center,
                );
            }
        }
    }

    fn about(&mut self, ui: &mut Ui, r: Rect, m: Option<&MapInfo>) {
        card(
            ui,
            r,
            "About this map",
            Some("scroll for the full description"),
        );
        let Some(m) = m else { return };
        let desc = shorten(m.description.trim(), DESCRIPTION_CHARS);
        let desc = if desc.is_empty() {
            "The author wrote no description.".to_string()
        } else {
            desc
        };
        let area = Rect::new(r.x + 8.0, r.y + 44.0, r.w - 16.0, (r.h - 52.0).max(0.0));
        let px = 12.5 * TEXT_SCALE;
        ui.scroll_area("maps:about", area, &mut |ui, view| {
            ui.paragraph(
                &desc,
                glam::Vec2::new(view.x + 9.0, view.y + 14.0),
                view.w - 26.0,
                px,
                Weight::Regular,
                FG,
            ) + 24.0
        });
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

fn map_status(m: &MapInfo) -> (Status, String) {
    match m.entry_points.len() {
        0 => (Status::Warning, "No start point listed".into()),
        1 => (Status::Found, "1 start point".into()),
        n => (Status::Found, format!("{n} start points")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample;

    #[test]
    fn a_map_without_start_points_is_a_warning() {
        let list = sample::maps();
        assert_eq!(map_status(&list[2]).0, Status::Warning);
        assert_eq!(map_status(&list[0]).1, "3 start points");
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

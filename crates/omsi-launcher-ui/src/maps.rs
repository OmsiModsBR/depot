//! The Maps page, where a session starts: the library of maps on the left, the chosen map's
//! picture and its author's description in the middle, and on the right the start (the date,
//! the kind of session, where to start and the weather), which the host draws into
//! `start_area` - it holds the session's choice. Read like the Vehicles page, on a thread of
//! its own.

use crate::kit::{self, card, clipped, dot, shorten, Status};
use crate::look::*;
use crate::shell::BODY_TOP;
use omsi_launcher_lib::MapInfo;
use omsi_ui::paint::Align;
use omsi_ui::widgets::{matches, ButtonKind, Ui};
use omsi_ui::{Color, Rect, Weight};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// The longest description drawn (third-party text, bounded; the card scrolls).
const DESCRIPTION_CHARS: usize = 20_000;

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
    /// The rows' small pictures on the GPU, by map file (`set_thumb`).
    thumbs: std::collections::HashMap<String, usize>,
    /// The rows whose picture has just been read, and fades in when it is drawn.
    fresh_thumbs: std::collections::HashSet<String>,
    /// The files of the rows drawn on screen in the last frame (`visible`).
    visible: Vec<String>,
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
            thumbs: Default::default(),
            fresh_thumbs: Default::default(),
            visible: Vec::new(),
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
    /// The files of the library's rows on screen (after `draw`), for their small pictures.
    pub fn visible(&self) -> &[String] {
        &self.visible
    }

    /// The small picture of `file`'s row, once the host has one on the GPU.
    pub fn set_thumb(&mut self, file: &str, tex: Option<usize>) {
        match tex {
            Some(t) => {
                if self.thumbs.insert(file.to_string(), t).is_none() {
                    self.fresh_thumbs.insert(file.to_string());
                }
            }
            None => {
                self.thumbs.remove(file);
                self.fresh_thumbs.remove(file);
            }
        }
    }

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
                skeleton(ui, columns(body));
                return;
            }
        };
        self.apply_pending();
        let shown = self.shown(&list);
        let (lib, picture, about, start) = columns(body);
        self.library(ui, lib, &list, &shown);
        // (the map chosen stays while a search hides its row: only a click chooses another)
        let chosen = list.get(self.selected);
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
        let thumbs = &self.thumbs;
        let fresh = &self.fresh_thumbs;
        let mut faded = Vec::new();
        let mut visible = Vec::new();
        ui.scroll_area("maps:list", area, &mut |ui, view| {
            let mut y = view.y;
            for &i in shown {
                let m = &list[i];
                let row = Rect::new(view.x, y, view.w - 8.0, 72.0);
                if ui.row(&format!("maps:row:{i}"), row, i == selected) {
                    picked = Some(i);
                }
                if ui.rect_visible(row) {
                    visible.push(m.file.clone());
                    // (the map's own picture, small, cut to the row's shape; a quiet panel
                    // until it is read, or when the map has none)
                    let tw = (row.w * 0.3).clamp(56.0, THUMB_W);
                    let th = tw * THUMB_H / THUMB_W;
                    let pic = Rect::new(row.x + 10.0, row.center().y - th * 0.5, tw, th);
                    match thumbs.get(&m.file) {
                        Some(&tex) if fresh.contains(&m.file) => {
                            // (a picture just read fades in, once)
                            let a = ui.appear(
                                omsi_ui::widgets::id_of(&format!("maps:thumb:{}", m.file)),
                                omsi_ui::theme::MOTION_NORMAL,
                            );
                            ui.p().rounded(pic, 4.0, PANEL_2.alpha(1.0 - a));
                            ui.push_fade(a);
                            ui.image_cover(pic, tex, THUMB_W / THUMB_H, 1.0);
                            ui.pop_fade();
                            if a >= 1.0 {
                                faded.push(m.file.clone());
                            }
                        }
                        Some(&tex) => ui.image_cover(pic, tex, THUMB_W / THUMB_H, 1.0),
                        None => {
                            ui.p().rounded(pic, 4.0, PANEL_2);
                            ui.icon("o-map", pic.center(), 18.0, MUTED);
                        }
                    }
                    let x = pic.right() + 12.0;
                    clipped(
                        ui,
                        &map_title(m),
                        x,
                        row.y + 24.0,
                        row.right() - 28.0 - x,
                        14.5,
                        Weight::Bold,
                        FG,
                    );
                    let (status, label) = map_status(m);
                    dot(ui, x + 5.0, row.y + 42.0, status);
                    clipped(
                        ui,
                        &label,
                        x + 18.0,
                        row.y + 42.0,
                        row.right() - 28.0 - (x + 18.0),
                        12.0,
                        Weight::Regular,
                        status.color(),
                    );
                    clipped(
                        ui,
                        map_source(m),
                        x,
                        row.y + 58.0,
                        row.right() - 28.0 - x,
                        12.0,
                        Weight::Regular,
                        MUTED,
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
        self.visible = visible;
        for f in faded {
            self.fresh_thumbs.remove(&f);
        }
        if let Some(i) = picked {
            self.selected = i;
        }
    }

    fn picture(&mut self, ui: &mut Ui, r: Rect, m: Option<&MapInfo>) {
        card(ui, r, "Map", None);
        // (the map's own radar view, with its streets and start points, in a window over the
        // page)
        let bw = ui.width("Explore map", 13.0, Weight::Medium) + 52.0;
        let b = Rect::new(r.right() - 15.0 - bw, r.y + 10.0, bw, 30.0);
        if m.is_some()
            && ui.button(
                "maps:explore",
                b,
                "Explore map",
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
        // (the picture area ends in the card's own round corners, and the room the picture
        // leaves is darker than the card, as round a photo; square, its corners stood out of
        // the card)
        ui.push_clip(
            r.inset(1.0),
            omsi_ui::theme::corner(omsi_ui::theme::RADIUS - 1.0),
        );
        ui.p().rect(inner, PICTURE_BG);
        if m.is_none() {
            ui.pop_clip();
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
                // (a picture as wide as the card reaches its round lower corners: rounded
                // as they are - the clip above cuts colours, not pictures)
                let wide = pic.w >= inner.w - 0.5 && pic.bottom() >= inner.bottom() - 0.5;
                let round = if wide {
                    omsi_ui::theme::corner(omsi_ui::theme::RADIUS - 1.0)
                } else {
                    0.0
                };
                // (fading in once read, from the dark room around it)
                let key = format!("maps:picture:{}:{tex}", m.map_or("", |m| m.file.as_str()));
                let a = ui.appear(omsi_ui::widgets::id_of(&key), omsi_ui::theme::MOTION_NORMAL);
                ui.push_fade(a);
                ui.image(pic, tex, round);
                ui.pop_fade();
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
        ui.pop_clip();
    }

    fn about(&mut self, ui: &mut Ui, r: Rect, m: Option<&MapInfo>) {
        let Some(m) = m else {
            card(ui, r, "About this map", None);
            return;
        };
        let desc = shorten(m.description.trim(), DESCRIPTION_CHARS);
        let desc = if desc.is_empty() {
            "The author wrote no description.".to_string()
        } else {
            desc
        };
        let area = Rect::new(r.x + 8.0, r.y + 44.0, r.w - 16.0, (r.h - 52.0).max(0.0));
        let px = 12.5 * TEXT_SCALE;
        // (the hint to scroll only when there is more than the card shows)
        let longer = ui.paragraph_height(&desc, area.w - 26.0, px, Weight::Regular) + 24.0 > area.h;
        card(
            ui,
            r,
            "About this map",
            longer.then_some("scroll for the full description"),
        );
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

/// The size of a row's small picture.
/// The page's three columns in `body`: the library, the map's picture and its description
/// under it, the start.
fn columns(body: Rect) -> (Rect, Rect, Rect, Rect) {
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
    (lib, picture, about, start)
}

/// The page while the maps are being read: its cards, with quiet bars where their rows and
/// lines will be, gently pulsing (still with motion off). Only while reading: nothing moves
/// on a page at rest.
fn skeleton(ui: &mut Ui, (lib, picture, about, start): (Rect, Rect, Rect, Rect)) {
    let motion = omsi_ui::theme::motion();
    let t = ui.time;
    // (a slow pulse running down the page, 1.6 s a beat)
    let shade = |y: f32| -> Color {
        let k = if motion {
            0.5 + 0.5 * (std::f32::consts::TAU * (t / 1.6 - y / 900.0)).sin()
        } else {
            0.5
        };
        PANEL_2.alpha(0.55 + 0.45 * k)
    };
    card(ui, lib, "Map library", Some("Reading the maps\u{2026}"));
    let mut y = lib.y + 92.0;
    while y + 72.0 <= lib.bottom() - 8.0 {
        let th = 46.0;
        ui.p().rounded(
            Rect::new(lib.x + 18.0, y + 36.0 - th * 0.5, 74.0, th),
            4.0,
            shade(y),
        );
        let x = lib.x + 104.0;
        let w = lib.right() - 36.0 - x;
        ui.p()
            .rounded(Rect::new(x, y + 18.0, w * 0.75, 10.0), 5.0, shade(y));
        ui.p()
            .rounded(Rect::new(x, y + 38.0, w * 0.5, 8.0), 4.0, shade(y + 20.0));
        y += 72.0;
    }
    card(ui, picture, "Map", None);
    ui.p().rounded(
        Rect::new(
            picture.x + 1.0,
            picture.y + 50.0,
            picture.w - 2.0,
            picture.h - 51.0,
        ),
        omsi_ui::theme::corner(omsi_ui::theme::RADIUS - 1.0),
        PICTURE_BG,
    );
    card(ui, about, "About this map", None);
    for (k, f) in [0.9, 0.8, 0.85, 0.5].iter().enumerate() {
        let y = about.y + 60.0 + k as f32 * 22.0;
        if y + 10.0 < about.bottom() - 12.0 {
            ui.p().rounded(
                Rect::new(about.x + 19.0, y, (about.w - 38.0) * f, 9.0),
                4.5,
                shade(y),
            );
        }
    }
    card(ui, start, "Start", None);
    for k in 0..4 {
        let y = start.y + 64.0 + k as f32 * 70.0;
        if y + 34.0 < start.bottom() - 12.0 {
            ui.p().rounded(
                Rect::new(start.x + 19.0, y, start.w * 0.3, 8.0),
                4.0,
                shade(y),
            );
            ui.p().rounded(
                Rect::new(start.x + 19.0, y + 16.0, start.w - 38.0, 34.0),
                omsi_ui::theme::CONTROL_RADIUS,
                shade(y + 16.0),
            );
        }
    }
}

const THUMB_W: f32 = 84.0;
const THUMB_H: f32 = 52.0;

/// Where a map comes from: installed here (the content folder) or the original OMSI 2's.
fn map_source(m: &MapInfo) -> &'static str {
    if m.installed {
        "Installed mod"
    } else {
        "Original OMSI 2"
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
        v.query = "no such map".into();
        assert!(v.shown(&list).is_empty());
    }

    /// The map the choice holds is the one shown once the catalog comes, and a search that
    /// hides it does not choose another (the first map was taken, and saved, at every start).
    #[test]
    fn the_map_chosen_stays_until_another_is_clicked() {
        let list = sample::maps();
        let mut v = MapsView::default();
        v.select(&list[1].file);
        v.set_catalog(list.clone());
        assert_eq!(v.chosen(), 1);
        let mut ui = Ui::new();
        let body = Rect::new(0.0, 0.0, 1180.0, 820.0);
        ui.begin(glam::Vec2::new(1440.0, 900.0), 1.0, 0.016);
        v.query = "no such map".into();
        v.draw(&mut ui, body);
        ui.finish();
        assert_eq!(v.current(), Some(list[1].file.as_str()));
        assert!(v.start_area().is_some());
    }

    /// The rows on screen are the ones whose small pictures the host reads, and a row says
    /// where its map comes from.
    #[test]
    fn the_rows_on_screen_ask_for_their_pictures() {
        let list = sample::maps();
        let mut v = MapsView::with_catalog(list.clone());
        let mut ui = Ui::new();
        ui.begin(glam::Vec2::new(1440.0, 900.0), 1.0, 0.016);
        v.draw(&mut ui, Rect::new(0.0, 0.0, 1180.0, 820.0));
        ui.finish();
        let files: Vec<_> = list.iter().map(|m| m.file.clone()).collect();
        assert_eq!(v.visible(), files.as_slice());
        v.set_thumb(&files[0], Some(7));
        assert_eq!(v.thumbs.get(&files[0]), Some(&7));
        v.set_thumb(&files[0], None);
        assert!(v.thumbs.is_empty());
        assert_ne!(
            map_source(&list[0]),
            map_source(&MapInfo {
                installed: !list[0].installed,
                ..list[0].clone()
            })
        );
    }
}

//! The Vehicles page: the library of buses, the chosen one's preview, what is known about it
//! (and whether its files were found), its paints, and the bus to use for the next start from
//! Maps - optional: in the game the ALT menu adds and changes vehicles as always. The catalog
//! is read by the launcher core on a thread of its own, so the window never waits for the disk.

use crate::kit::{self, card, clipped, dot, shorten, subheading, text, Status};
use crate::look::*;
use crate::shell::BODY_TOP;
use omsi_launcher_lib::VehicleInfo;
use omsi_ui::paint::Align;
use omsi_ui::widgets::{matches, ButtonKind, Ui};
use omsi_ui::{Rect, Weight};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// The longest description drawn: third-party text, bounded.
const DESCRIPTION_CHARS: usize = 1200;

enum Catalog {
    NotStarted,
    Loading(Receiver<Result<Vec<VehicleInfo>, String>>),
    Ready(Rc<Vec<VehicleInfo>>),
    Failed(String),
}

pub struct VehiclesView {
    catalog: Catalog,
    query: String,
    selected: usize,
    /// The paint chosen among the bus's paints; `None` is the bus's own default livery.
    paint: Option<usize>,
    /// A bus (its file) and paint to show once the catalog is there (`select`).
    pending: Option<(String, String)>,
    /// "Use this bus" was clicked (`take_use`).
    used: Option<UseBus>,
    /// The "Use for the next start" card's choices, for the bus `prep_for` (its file): the
    /// depot file (0: automatic, else the bus's list from 1), the fleet number and the plate.
    prep_for: String,
    garage: usize,
    number: usize,
    plate: String,
    /// Those were just chosen afresh for another bus (`fresh_bus`).
    fresh: bool,
    /// The bus and paint the page shows (its file and the paint's name), for the host to draw.
    current: Option<(String, String)>,
    /// What the host drew of that bus for the preview (`set_picture`), and where the preview
    /// was this frame.
    picture: Option<Picture>,
    preview_area: Option<Rect>,
}

/// The bus to start with from Maps, as the "Use for the next start" card has it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct UseBus {
    /// The bus's file.
    pub file: String,
    /// The paint's name, empty for the bus's own livery.
    pub paint: String,
    /// The depot file (`None`: chosen as the map and the date say).
    pub hof: Option<String>,
    /// The fleet number from the bus's list (`None`: it has none).
    pub number: Option<String>,
    /// The number plate typed (empty: as the content says).
    pub plate: String,
}

/// The bus's picture in the preview, as the host has it: drawn by a helper process (live) or
/// kept from an earlier time.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    /// The texture and its width over its height; None while there is none.
    pub texture: Option<(usize, f32)>,
    pub from_cache: bool,
    pub live: bool,
    pub busy: bool,
    /// Why there is none (shown instead of the picture).
    pub error: Option<String>,
}

impl Default for VehiclesView {
    fn default() -> Self {
        VehiclesView {
            catalog: Catalog::NotStarted,
            query: String::new(),
            selected: 0,
            paint: None,
            pending: None,
            used: None,
            prep_for: String::new(),
            garage: 0,
            number: 0,
            plate: String::new(),
            fresh: false,
            current: None,
            picture: None,
            preview_area: None,
        }
    }
}

impl VehiclesView {
    /// A page showing `list` (screenshots and tests).
    pub fn with_catalog(list: Vec<VehicleInfo>) -> VehiclesView {
        VehiclesView {
            catalog: Catalog::Ready(Rc::new(list)),
            ..Default::default()
        }
    }

    /// The bus and paint shown (file, paint name), once the catalog is read.
    pub fn current(&self) -> Option<&(String, String)> {
        self.current.as_ref()
    }

    /// Where the preview's picture goes this frame (in interface points), once drawn.
    pub fn preview_area(&self) -> Option<Rect> {
        self.preview_area
    }

    /// What the host has to show of `current` (None: the page says the Drive page shows it).
    pub fn set_picture(&mut self, p: Option<Picture>) {
        self.picture = p;
    }

    /// The vehicle chosen in the library, and its paint (`None`: the default livery).
    pub fn chosen(&self) -> (usize, Option<usize>) {
        (self.selected, self.paint)
    }

    /// Take `list` as the catalog (the host's, which it reads again after Setup or an
    /// install). The bus shown stays the one it was when it is still in the list.
    pub fn set_catalog(&mut self, list: Vec<VehicleInfo>) {
        let keep = match &self.catalog {
            Catalog::Ready(old) => old
                .get(self.selected)
                .map(|v| (v.file.clone(), self.paint_name(v))),
            _ => None,
        };
        self.catalog = Catalog::Ready(Rc::new(list));
        if let (Some(keep), None) = (keep, &self.pending) {
            self.pending = Some(keep);
        }
        self.selected = 0;
        self.paint = None;
        self.apply_pending();
    }

    /// How many buses the catalog holds, once it is read.
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

    /// The chosen paint's name for `v`, empty for the default livery.
    fn paint_name(&self, v: &VehicleInfo) -> String {
        self.paint
            .and_then(|i| v.paints.get(i))
            .cloned()
            .unwrap_or_default()
    }

    /// Show the bus whose file is `file`, in `paint` (empty: its first), when it is in the
    /// catalog; one not yet read is shown once it is.
    pub fn select(&mut self, file: &str, paint: &str) {
        self.pending = Some((file.to_string(), paint.to_string()));
        // (the card starts over from what the next start uses: edits never applied with
        // "Use for the next start" do not stay on it)
        self.prep_for.clear();
        self.apply_pending();
    }

    fn apply_pending(&mut self) {
        let Catalog::Ready(list) = &self.catalog else {
            return;
        };
        let Some((file, paint)) = self.pending.take() else {
            return;
        };
        if let Some(i) = list.iter().position(|v| v.file.eq_ignore_ascii_case(&file)) {
            self.selected = i;
            self.paint = list[i].paints.iter().position(|p| *p == paint);
            self.query.clear();
        }
    }

    /// The bus "Use this bus" was clicked for, once.
    pub fn take_use(&mut self) -> Option<UseBus> {
        self.used.take()
    }

    /// The bus (its file) the "Use for the next start" card has just started over for, once:
    /// when it is the one the next start uses, the host fills the card in with `prepare`.
    pub fn fresh_bus(&mut self) -> Option<String> {
        std::mem::take(&mut self.fresh).then(|| self.prep_for.clone())
    }

    /// Fill the "Use for the next start" card with what the next start uses, when it is the
    /// bus shown: its depot file (None: automatic), fleet number and plate.
    pub fn prepare(&mut self, file: &str, hof: Option<&str>, number: &str, plate: &str) {
        let Catalog::Ready(list) = &self.catalog else {
            return;
        };
        let Some(v) = list
            .get(self.selected)
            .filter(|v| v.file.eq_ignore_ascii_case(file))
        else {
            return;
        };
        self.garage = hof
            .and_then(|h| v.hofs.iter().position(|x| x.eq_ignore_ascii_case(h)))
            .map(|i| i + 1)
            .unwrap_or(0);
        self.number = v.numbers.iter().position(|(n, _)| n == number).unwrap_or(0);
        self.plate = plate.to_string();
        self.prep_for = v.file.clone();
    }

    /// Start reading the catalog if that has not begun.
    pub fn load(&mut self) {
        if !matches!(self.catalog, Catalog::NotStarted) {
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let r = omsi_launcher_lib::list_vehicles().map_err(|e| format!("{e:#}"));
            let _ = tx.send(r);
        });
        self.catalog = Catalog::Loading(rx);
    }

    /// Take the catalog in when the thread has finished.
    pub fn poll(&mut self) {
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

    /// Indices of the vehicles the search shows.
    fn shown(&self, list: &[VehicleInfo]) -> Vec<usize> {
        (0..list.len())
            .filter(|&i| {
                let v = &list[i];
                self.query.trim().is_empty()
                    || matches(
                        &format!("{} {} {}", v.name, v.manufacturer, v.type_name),
                        &self.query,
                    )
            })
            .collect()
    }

    /// The Vehicles page in the body area `body` (the sidebar is outside it).
    pub fn draw(&mut self, ui: &mut Ui, body: Rect) {
        self.load();
        self.poll();
        let list = match &self.catalog {
            Catalog::Ready(l) => Rc::clone(l),
            Catalog::Failed(e) => {
                let r = Rect::new(body.x, body.y + BODY_TOP, body.w, 120.0);
                card(ui, r, "Vehicles could not be listed", None);
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
                card(ui, r, "Vehicle library", None);
                clipped(
                    ui,
                    "Reading the vehicles\u{2026}",
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
            self.paint = None;
        }
        // (three columns over two rows: the library, the preview and the details above; the
        // paints under the first two, the bus for the next start under the details)
        let top = body.y + BODY_TOP;
        let rows_h = (body.h - BODY_TOP - 24.0).max(420.0);
        let lower_h = 222.0_f32.min(rows_h * 0.45);
        let upper_h = rows_h - lower_h - 14.0;
        let lib = Rect::new(body.x, top, kit::fit(330.0, body.w), upper_h);
        let side_w = kit::fit(340.0, body.w);
        let details = Rect::new(body.right() - side_w, top, side_w, upper_h);
        let preview = Rect::new(
            lib.right() + 14.0,
            top,
            details.x - lib.right() - 28.0,
            upper_h,
        );
        self.library(ui, lib, &list, &shown);
        let chosen = kit::chosen(&list, &shown, self.selected);
        self.current = chosen.map(|v| (v.file.clone(), self.paint_name(v)));
        if let Some(v) = chosen.filter(|v| !v.file.eq_ignore_ascii_case(&self.prep_for)) {
            // (another bus: its own depot files and numbers, chosen afresh)
            self.prep_for = v.file.clone();
            (self.garage, self.number) = (0, 0);
            self.plate.clear();
            self.fresh = true;
        }
        self.preview(ui, preview);
        self.details(ui, details, chosen);
        let y2 = top + upper_h + 14.0;
        let paints = Rect::new(body.x, y2, preview.right() - body.x, lower_h);
        let next = Rect::new(details.x, y2, side_w, lower_h);
        self.paints(ui, paints, chosen);
        self.next_start(ui, next, chosen);
    }

    fn library(&mut self, ui: &mut Ui, r: Rect, list: &[VehicleInfo], shown: &[usize]) {
        card(
            ui,
            r,
            "Vehicle library",
            Some(&format!("{} of {}", shown.len(), list.len())),
        );
        let search = Rect::new(r.x + 15.0, r.y + 44.0, r.w - 30.0, 36.0);
        ui.text_input(
            "vehicles:search",
            search,
            &mut self.query,
            "Search vehicles...",
            Some("search"),
        );
        let area = Rect::new(r.x + 8.0, r.y + 92.0, r.w - 16.0, r.h - 100.0);
        let mut picked = None;
        let selected = self.selected;
        ui.scroll_area("vehicles:list", area, &mut |ui, view| {
            let mut y = view.y;
            for &i in shown {
                let v = &list[i];
                let row = Rect::new(view.x, y, view.w - 8.0, 84.0);
                if ui.row(&format!("vehicles:row:{i}"), row, i == selected) {
                    picked = Some(i);
                }
                if ui.rect_visible(row) {
                    let name = omsi_launcher_lib::display_bus_name(&v.name);
                    clipped(
                        ui,
                        &name,
                        row.x + 16.0,
                        row.y + 24.0,
                        row.w - 40.0,
                        15.0,
                        Weight::Bold,
                        FG,
                    );
                    let sub = format!("{} \u{b7} {}", v.type_name, v.manufacturer);
                    clipped(
                        ui,
                        &sub,
                        row.x + 16.0,
                        row.y + 46.0,
                        row.w - 40.0,
                        12.0,
                        Weight::Regular,
                        MUTED,
                    );
                    let (status, label) = vehicle_status(v);
                    dot(ui, row.x + 21.0, row.y + 66.0, status);
                    text(
                        ui,
                        label,
                        row.x + 34.0,
                        row.y + 66.0,
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
                y += 84.0;
            }
            y - view.y
        });
        if let Some(i) = picked {
            self.selected = i;
            self.paint = None;
        }
    }

    fn preview(&mut self, ui: &mut Ui, r: Rect) {
        let pic = self.picture.clone();
        let note = pic
            .as_ref()
            .filter(|p| p.texture.is_some() && p.from_cache)
            .map(|_| "image from cache \u{b7} live view while this page is open");
        card(ui, r, "Preview", note);
        let inner = Rect::new(r.x + 1.0, r.y + 44.0, r.w - 2.0, r.h - 45.0);
        self.preview_area = Some(inner);
        ui.p().rect(inner, PANEL_2);
        let Some(pic) = pic else {
            ui.text_in(
                "Choose a map on Maps: the bus is shown in its light.",
                inner.inset(24.0),
                14.0 * TEXT_SCALE,
                Weight::Regular,
                MUTED,
                Align::Center,
            );
            return;
        };
        match pic.texture {
            Some((tex, aspect)) => ui.image_cover(inner, tex, aspect, 1.0),
            None => {
                let t = if pic.error.is_some() {
                    "No preview"
                } else {
                    "Loading\u{2026}"
                };
                ui.text_in(
                    t,
                    inner.inset(24.0),
                    14.0 * TEXT_SCALE,
                    Weight::Regular,
                    MUTED,
                    Align::Center,
                );
            }
        }
        if pic.live {
            // (as the concept: a green dot and the words, top right over the picture)
            let t = "preview helper: running";
            let w = ui.width(t, 12.0 * TEXT_SCALE, Weight::Regular);
            let x = inner.right() - 14.0 - w;
            ui.p()
                .circle(glam::Vec2::new(x - 9.0, inner.y + 18.0), 4.0, OK);
            // (`text` scales the size itself)
            text(ui, t, x, inner.y + 18.0, 12.0, Weight::Regular, OK);
        }
        if pic.busy && pic.texture.is_some() {
            let c = glam::Vec2::new(inner.x + 18.0, inner.y + 18.0);
            let a = ui.time * 5.0;
            ui.p().arc(c, 6.0, 8.0, a, a + 4.2, MUTED);
        }
    }

    fn details(&mut self, ui: &mut Ui, r: Rect, v: Option<&VehicleInfo>) {
        card(ui, r, "Vehicle details", None);
        let Some(v) = v else {
            clipped(
                ui,
                "No vehicle matches the search.",
                r.x + 19.0,
                r.y + 62.0,
                r.w - 38.0,
                14.0,
                Weight::Regular,
                MUTED,
            );
            return;
        };
        // (whether its files were found: the worst of them, all of them under the mouse)
        let files = file_status(v);
        let worst = if files.iter().any(|f| f.1 == Status::Missing) {
            (Status::Missing, "Files missing")
        } else if files.iter().any(|f| f.1 == Status::Warning) {
            (Status::Warning, "Check files")
        } else {
            (Status::Found, "Files found")
        };
        let bw = ui.width(worst.1, 12.0 * TEXT_SCALE, Weight::Medium) + 20.0;
        let badge = Rect::new(r.right() - 17.0 - bw, r.y + 58.0, bw, 24.0);
        ui.p().rounded(badge, 4.0, worst.0.color().alpha(0.12));
        ui.p()
            .rounded_border(badge, 4.0, 1.0, worst.0.color().alpha(0.5));
        ui.text_in(
            worst.1,
            badge,
            12.0 * TEXT_SCALE,
            Weight::Medium,
            worst.0.color(),
            Align::Center,
        );
        let tip: Vec<String> = files
            .iter()
            .map(|(label, _, note)| format!("{label}: {note}"))
            .collect();
        ui.tooltip(badge, &tip.join("\n"));
        let name = omsi_launcher_lib::display_bus_name(&v.name);
        let name_w = badge.x - 12.0 - (r.x + 19.0);
        clipped(
            ui,
            &name,
            r.x + 19.0,
            r.y + 62.0,
            name_w,
            15.0,
            Weight::Bold,
            FG,
        );
        clipped(
            ui,
            &v.manufacturer,
            r.x + 19.0,
            r.y + 82.0,
            name_w,
            12.0,
            Weight::Regular,
            MUTED,
        );
        let mut rows = vec![
            ("Type", v.type_name.clone()),
            ("Folder", v.folder.clone()),
            // (the bus's own livery and its repaints, as the Paints card counts them)
            ("Paints", (v.paints.len() + 1).to_string()),
            ("Depot files", v.hofs.len().to_string()),
            ("Fleet numbers", v.numbers.len().to_string()),
        ];
        if !v.missing_packs.is_empty() {
            rows.push(("Missing packs", v.missing_packs.join(", ")));
        }
        let desc = shorten(v.description.trim(), DESCRIPTION_CHARS);
        let desc = if desc.is_empty() {
            "The author wrote no description.".to_string()
        } else {
            desc
        };
        // (the rows, then the description in a box of its own that scrolls; on a short window,
        // where that box would be under DESCRIPTION_MIN, the rows and the description scroll
        // together)
        let top = r.y + 102.0;
        let id = format!("vehicles:about:{}", v.file);
        if r.bottom() - (top + rows.len() as f32 * ROW) - 54.0 >= DESCRIPTION_MIN {
            let y = detail_rows(ui, r.x, r.w, top, &rows);
            subheading(
                ui,
                Rect::new(r.x + 18.0, y + 20.0, r.w - 36.0, 20.0),
                "Author's description",
            );
            let box_ = Rect::new(r.x + 15.0, y + 46.0, r.w - 30.0, r.bottom() - y - 54.0);
            ui.scroll_area(&id, box_, &mut |ui, view| {
                let h = ui.paragraph(
                    &desc,
                    glam::Vec2::new(view.x + 4.0, view.y + 12.0),
                    view.w - 16.0,
                    12.5 * TEXT_SCALE,
                    Weight::Regular,
                    FG,
                );
                h + 20.0
            });
        } else {
            let area = Rect::new(r.x + 1.0, top - 2.0, r.w - 2.0, r.bottom() - top - 6.0);
            ui.scroll_area(&id, area, &mut |ui, view| {
                let y = detail_rows(ui, view.x - 1.0, view.w + 2.0, view.y + 2.0, &rows);
                subheading(
                    ui,
                    Rect::new(view.x + 17.0, y + 20.0, view.w - 36.0, 20.0),
                    "Author's description",
                );
                let h = ui.paragraph(
                    &desc,
                    glam::Vec2::new(view.x + 18.0, y + 58.0),
                    view.w - 40.0,
                    12.5 * TEXT_SCALE,
                    Weight::Regular,
                    FG,
                );
                y + 58.0 + h + 16.0 - view.y
            });
        }
    }

    fn paints(&mut self, ui: &mut Ui, r: Rect, v: Option<&VehicleInfo>) {
        // the bus's own livery and its repaints, as the chips show them
        let n = v.map(|v| v.paints.len() + 1).unwrap_or(0);
        card(
            ui,
            r,
            "Paints",
            Some(&omsi_ui::tr("%{n} found").replace("%{n}", &n.to_string())),
        );
        let Some(v) = v else { return };
        let area = Rect::new(r.x + 12.0, r.y + 44.0, r.w - 24.0, r.h - 52.0);
        let mut picked = None;
        let chosen = self.paint;
        ui.scroll_area("vehicles:paints", area, &mut |ui, view| {
            let (mut x, mut y) = (view.x, view.y);
            let chips = std::iter::once((
                None,
                shorten(default_livery(v), 28),
                "vehicles:paint:default".to_string(),
            ))
            .chain(
                v.paints
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (Some(i), shorten(p, 28), format!("vehicles:paint:{i}"))),
            );
            for (i, label, id) in chips {
                let w = ui.width(&label, 13.0 * TEXT_SCALE, Weight::Medium) + 28.0;
                if x + w > view.right() - 8.0 {
                    x = view.x;
                    y += 42.0;
                }
                let chip = Rect::new(x, y, w, 34.0);
                if ui.row(&id, chip, i == chosen) {
                    picked = Some(i);
                }
                ui.text_in(
                    &label,
                    chip,
                    13.0 * TEXT_SCALE,
                    Weight::Medium,
                    if i == chosen { FG } else { MUTED },
                    Align::Center,
                );
                x += w + 8.0;
            }
            y + 42.0 - view.y
        });
        if let Some(i) = picked {
            self.paint = i;
        }
    }

    /// The bus to start with from Maps: its paint, depot file, fleet number and plate, and
    /// "Use this bus". Optional: in the game the ALT menu adds and changes vehicles as always.
    fn next_start(&mut self, ui: &mut Ui, r: Rect, v: Option<&VehicleInfo>) {
        card(ui, r, "Use for the next start", None);
        let Some(v) = v else { return };
        let gap = 10.0;
        let col = (r.w - 34.0 - gap) / 2.0;
        let (x1, x2) = (r.x + 17.0, r.x + 17.0 + col + gap);
        let label = |ui: &mut Ui, s: &str, x: f32, y: f32| {
            text(ui, s, x, y, 12.0, Weight::Regular, MUTED);
        };
        let mut y = r.y + 50.0;
        label(ui, "Paint", x1, y);
        label(ui, "Garage", x2, y);
        y += 10.0;
        let paints: Vec<String> = std::iter::once(default_livery(v).to_string())
            .chain(v.paints.iter().cloned())
            .collect();
        let mut paint = self.paint.map(|i| i + 1).unwrap_or(0);
        if ui.select(
            "vehicles:next-paint",
            Rect::new(x1, y, col, 30.0),
            &mut paint,
            &paints,
        ) {
            self.paint = paint.checked_sub(1);
        }
        let garages: Vec<String> = std::iter::once("Automatic".to_string())
            .chain(v.hofs.iter().cloned())
            .collect();
        ui.select(
            "vehicles:next-garage",
            Rect::new(x2, y, col, 30.0),
            &mut self.garage,
            &garages,
        );
        y += 46.0;
        label(ui, "Fleet number", x1, y);
        label(ui, "Plate", x2, y);
        y += 10.0;
        let numbers: Vec<String> = if v.numbers.is_empty() {
            vec!["None listed".to_string()]
        } else {
            v.numbers.iter().map(|(n, _)| n.clone()).collect()
        };
        ui.select(
            "vehicles:next-number",
            Rect::new(x1, y, col, 30.0),
            &mut self.number,
            &numbers,
        );
        // (the plate the number comes with, unless one is typed)
        let shown_plate = v
            .numbers
            .get(self.number)
            .map(|(_, p)| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "Automatic".to_string());
        ui.text_input(
            "vehicles:next-plate",
            Rect::new(x2, y, col, 30.0),
            &mut self.plate,
            &shown_plate,
            None,
        );
        let b = Rect::new(r.x + 17.0, y + 42.0, r.w - 34.0, 32.0);
        let note_y = b.bottom() + 18.0;
        if ui.button("vehicles:use", b, "Use this bus", None, ButtonKind::Primary) {
            self.used = Some(UseBus {
                file: v.file.clone(),
                paint: self.paint_name(v),
                hof: self
                    .garage
                    .checked_sub(1)
                    .and_then(|i| v.hofs.get(i))
                    .cloned(),
                number: v.numbers.get(self.number).map(|(n, _)| n.clone()),
                plate: self.plate.trim().to_string(),
            });
        }
        clipped(
            ui,
            "Optional. The ALT menu works as always.",
            r.x + 17.0,
            note_y,
            r.w - 34.0,
            12.0,
            Weight::Regular,
            MUTED,
        );
    }
}

/// The name of a bus's own livery (no repaint), as the Drive page names it.
fn default_livery(v: &VehicleInfo) -> &str {
    match v.default_paint.trim() {
        "" => "Default paint",
        name => name,
    }
}

/// The status line of a library row.
fn vehicle_status(v: &VehicleInfo) -> (Status, &'static str) {
    if !v.missing_packs.is_empty() {
        (Status::Warning, "Needs a missing pack")
    } else if v.installed {
        (Status::Found, "Installed mod")
    } else {
        (Status::Found, "Original content")
    }
}

/// What the catalog tells of a vehicle's files. Scripts, textures and sounds are not opened
/// by the catalog, so they are `Unverified`, never "found".
fn file_status(v: &VehicleInfo) -> Vec<(&'static str, Status, String)> {
    let mut rows = vec![(
        "Vehicle file",
        if v.file.is_empty() {
            Status::Missing
        } else {
            Status::Found
        },
        v.file.rsplit('/').next().unwrap_or("").to_string(),
    )];
    // (a bus without repaints still has its own livery: nothing is missing then)
    rows.push(if v.paints.is_empty() {
        ("Repaints", Status::Found, "none".into())
    } else {
        (
            "Repaints",
            Status::Found,
            omsi_ui::tr("%{n} found").replace("%{n}", &v.paints.len().to_string()),
        )
    });
    rows.push(if v.hofs.is_empty() {
        (
            "Depot files",
            Status::Warning,
            "not found next to bus".into(),
        )
    } else {
        (
            "Depot files",
            Status::Found,
            omsi_ui::tr("%{n} found").replace("%{n}", &v.hofs.len().to_string()),
        )
    });
    rows.push(if v.missing_packs.is_empty() {
        ("Vehicle packs", Status::Found, "none missing".into())
    } else {
        (
            "Vehicle packs",
            Status::Warning,
            shorten(&v.missing_packs.join(", "), 24),
        )
    });
    rows.push(("Other files", Status::Unverified, "not checked yet".into()));
    rows
}

/// A detail row's height.
const ROW: f32 = 29.0;
/// The least room the author's description keeps under the rows before they scroll with it.
const DESCRIPTION_MIN: f32 = 80.0;

/// The detail rows (name and value, a line above each) of a card at `x` `w` wide, the first
/// line at `y`; returns where the last row ends.
fn detail_rows(ui: &mut Ui, x: f32, w: f32, y: f32, rows: &[(&str, String)]) -> f32 {
    let mut y = y;
    for (k, val) in rows {
        ui.p().rect(Rect::new(x + 1.0, y, w - 2.0, 1.0), LINE);
        text(ui, k, x + 19.0, y + 20.0, 12.5, Weight::Regular, MUTED);
        clipped(
            ui,
            val,
            x + 140.0,
            y + 20.0,
            w - 160.0,
            12.5,
            Weight::Regular,
            FG,
        );
        y += ROW;
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample;

    #[test]
    fn a_missing_pack_is_a_warning_and_the_catalog_never_claims_the_rest() {
        let list = sample::vehicles();
        let rows = file_status(&list[2]);
        let packs = rows.iter().find(|r| r.0 == "Vehicle packs").unwrap();
        assert_eq!(packs.1, Status::Warning);
        let depots = rows.iter().find(|r| r.0 == "Depot files").unwrap();
        assert_eq!(depots.1, Status::Warning);
        let rest = rows.last().unwrap();
        assert_eq!(rest.1, Status::Unverified);
        assert_eq!(vehicle_status(&list[2]).0, Status::Warning);
        assert_eq!(vehicle_status(&list[1]).1, "Installed mod");
        assert_eq!(vehicle_status(&list[0]).1, "Original content");
    }

    #[test]
    fn the_search_keeps_the_vehicles_that_match() {
        let list = sample::vehicles();
        let mut v = VehiclesView::with_catalog(list.clone());
        assert_eq!(v.shown(&list), vec![0, 1, 2]);
        v.query = "articulated".into();
        assert_eq!(v.shown(&list), vec![2]);
        v.query = "no such bus".into();
        assert!(v.shown(&list).is_empty());
        // and then no vehicle is chosen, not the first one
        let shown = v.shown(&list);
        assert!(kit::chosen(&list, &shown, v.selected).is_none());
    }
}

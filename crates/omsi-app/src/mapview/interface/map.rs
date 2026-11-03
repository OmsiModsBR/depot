//! The game's side of the pages' map (`omsi_launcher_ui::window::Map`): the launcher's map
//! view (`mapview`), and its picture as the window last drew it.

use crate::mapview::MapView;
use glam::{DVec2, Vec2};
use omsi_launcher_ui::window::{Map, MapLook, Pointer};
use omsi_ui::paint::Align;
use omsi_ui::theme::{RADIUS, TEXT_FAINT, TEXT_SOFT};
use omsi_ui::widgets::Ui;
use omsi_ui::{Rect, Weight};

/// The launcher's map, its picture's texture and where it was drawn.
pub(crate) struct GameMap<'a> {
    pub view: &'a mut MapView,
    pub tex: Option<usize>,
    pub rect: &'a mut Option<Rect>,
}

impl Map for GameMap<'_> {
    fn want(&mut self, look: MapLook) {
        self.view.want(look);
    }

    fn background(&mut self, ui: &mut Ui, r: Rect) {
        background(ui, self.view, self.tex, self.rect, r);
    }

    fn ready(&self) -> bool {
        self.view.ready()
    }

    fn roads_key(&self) -> usize {
        self.view.roads_key()
    }

    fn lanes_of(&self, pieces: &[omsi_launcher_lib::RoadPiece]) -> (Vec<Vec<DVec2>>, usize) {
        self.view.lanes_of(pieces)
    }

    fn project(&self, p: DVec2) -> Vec2 {
        self.view.project(p)
    }

    fn place_of(&self, id: i64) -> Option<DVec2> {
        self.view.place_of(id)
    }

    fn streets(&self) -> &[(String, DVec2)] {
        self.view.streets()
    }

    fn zoom(&mut self, by: f64) {
        self.view.zoom(by);
    }

    fn refit(&mut self) {
        self.view.refit();
    }

    fn look_at(&mut self, p: DVec2) {
        self.view.look_at(p);
    }

    fn think(&mut self, rect: Rect, window: Rect, scale: f32, p: Pointer) {
        self.view.think(rect, window, scale, p);
    }

    fn take_clicked(&mut self) -> Option<usize> {
        self.view.take_clicked()
    }

    fn counts(&self) -> Option<(usize, usize, usize)> {
        self.view.counts()
    }

    fn stops_placed(&self) -> &[(DVec2, usize)] {
        self.view.stops_placed()
    }

    fn entry_name(&self, i: usize) -> Option<&str> {
        self.view.entry_name(i)
    }

    fn entry_at(&self, i: usize) -> Option<Vec2> {
        self.view.entry_at(i)
    }

    fn hovered(&self) -> Option<usize> {
        self.view.hovered()
    }

    fn shown_of(&self, choice: i32) -> Option<usize> {
        self.view.shown_of(choice)
    }
}

/// The chosen map across `r`, the whole page behind the panels: every road the map has, its
/// entry points and the chosen trip's route, read from the tile files (see `mapview`), or a
/// word while it is read.
pub(crate) fn background(
    ui: &mut Ui,
    view: &MapView,
    tex: Option<usize>,
    rect: &mut Option<Rect>,
    r: Rect,
) {
    *rect = Some(r);
    let status = view.status();
    match (tex, status.is_empty()) {
        (Some(tex), true) => {
            ui.image(r, tex, RADIUS);
            // (the data's licence asks for it where its surface is shown)
            if let Some(credit) = view.credit().map(str::to_string) {
                ui.text_in(
                    &credit,
                    Rect::new(r.x + 12.0, r.bottom() - 24.0, (r.w - 24.0).max(0.0), 18.0),
                    10.5,
                    Weight::Medium,
                    TEXT_FAINT,
                    Align::Left,
                );
            }
        }
        _ => {
            ui.solid(r);
            ui.p()
                .rounded(r, RADIUS, omsi_ui::Color::rgba(13, 13, 13, 1.0));
            let t = if status.is_empty() {
                "Loading…"
            } else {
                status
            };
            ui.text_in(
                t,
                Rect::new(r.x, r.y + r.h * 0.5 - 12.0, r.w, 24.0),
                13.5,
                Weight::Regular,
                TEXT_FAINT,
                Align::Center,
            );
        }
    }
    if view.busy() {
        // (out of the way of the panels: the map is being read, the page is not)
        let c = Vec2::new(r.right() - 26.0, r.y + r.h - 26.0);
        let a = ui.time * 5.0;
        ui.p().arc(c, 6.0, 8.0, a, a + 4.2, TEXT_SOFT);
    }
}

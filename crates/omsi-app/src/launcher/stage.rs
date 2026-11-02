//! The game's side of the Drive page's bus (`omsi_launcher_ui::window::Showroom`): the bus
//! preview (`crate::showroom`), and its picture as the window last drew it.

use crate::showroom::Preview;
use omsi_launcher_ui::window::Showroom;
use omsi_ui::paint::Align;
use omsi_ui::theme::{FIELD, RADIUS, TEXT_DIM, TEXT_FAINT};
use omsi_ui::widgets::Ui;
use omsi_ui::{Rect, Weight};

/// The launcher's bus preview, its picture's texture and where it was drawn.
pub(super) struct GameShowroom<'a> {
    pub preview: &'a mut Preview,
    pub tex: Option<usize>,
    pub rect: &'a mut Option<Rect>,
}

impl Showroom for GameShowroom<'_> {
    fn full(&mut self, ui: &mut Ui, r: Rect, focus: f32) {
        *self.rect = Some(r);
        self.preview.focus_x = focus.clamp(0.2, 0.95);
        match (self.tex, self.preview.has_picture()) {
            (Some(tex), true) => ui.image_cover(r, tex, self.preview.aspect, 1.0),
            _ => {
                ui.solid(r);
                ui.p().rounded(r, RADIUS, FIELD);
                let t = if self.preview.error.is_some() {
                    "No preview"
                } else {
                    "Loading…"
                };
                ui.text_in(
                    t,
                    Rect::new(r.x, r.y + r.h * 0.5 - 12.0, r.w, 24.0),
                    13.0,
                    Weight::Regular,
                    TEXT_FAINT,
                    Align::Center,
                );
            }
        }
        // (left of the panels, which lie over the right part of the page)
        let w = (r.w * self.preview.focus_x * 2.0 - r.w).clamp(r.w * 0.3, r.w);
        badge(ui, self.preview, Rect::new(r.x, r.y, w, r.h));
    }

    fn pointer(&mut self, ui: &mut Ui, r: Rect) {
        if ui.over_ui || !r.contains(ui.input.mouse) {
            return;
        }
        if ui.input.wheel.y.abs() > 0.0 {
            self.preview
                .zoom_by((1.0 - ui.input.wheel.y * 0.08).clamp(0.8, 1.25));
        }
        ui.cursor = winit::window::CursorIcon::Grab;
    }
}

/// Where the preview's picture comes from, in the lower left of `r`: kept from an earlier
/// time (the helper draws the live one only while the page shows it) or drawn live.
pub(super) fn badge(ui: &mut Ui, preview: &Preview, r: Rect) {
    let t = if preview.live {
        "Preview helper: running"
    } else if preview.has_picture() && preview.picture_only() {
        // (a page that shows the kept picture: the live view when the player turns it)
        "Image from cache · drag it to see it live"
    } else if preview.has_picture() && preview.from_cache {
        "Image from cache · live view only on this page"
    } else {
        return;
    };
    let w = ui.width(t, 11.5, Weight::Regular) + 20.0;
    let b = Rect::new(r.x + 14.0, r.y + r.h - 38.0, w, 24.0);
    ui.p()
        .rounded(b, 12.0, omsi_ui::Color::rgba(10, 10, 12, 0.72));
    ui.text_in(t, b, 11.5, Weight::Regular, TEXT_DIM, Align::Center);
}

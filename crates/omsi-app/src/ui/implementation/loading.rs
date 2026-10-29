//! The loading screen of the new interface: while the first area of a streamed map loads,
//! `omsi_ui::loading` is drawn through the renderer's overlay hook over the blank picture the
//! game renders then - the player's own screenshots of that map as a slideshow, a progress
//! bar, and the bus and map being loaded. No screenshots: the default background.
//!
//! The screenshots are listed and decoded on a thread of their own (a few, shrunk to fit the
//! screen) and handed over one by one; the overlay, which must not block, only uploads them.
//! The bus's own name, maker and description are read from its file on another, once.

use super::super::domain::loading::{activity, heading, stage, CardText, Decoded, LoadingFrame};
use super::super::interface::output::{folder_of, App, MapPicture, VERSION};
use std::path::Path;

/// A picture wider than this is shrunk to it before it goes to the GPU.
const SLIDE_WIDTH: u32 = 1280;

/// A screenshot as RGBA, shrunk to `SLIDE_WIDTH` when wider. None when it cannot be read.
pub fn decode(path: &Path) -> Option<Decoded> {
    let img = image::open(path).ok()?;
    let img = if img.width() > SLIDE_WIDTH {
        let h = (img.height() as u64 * SLIDE_WIDTH as u64 / img.width() as u64).max(1) as u32;
        img.resize_exact(SLIDE_WIDTH, h, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    Some(Decoded {
        w: rgba.width(),
        h: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

/// The map's own picture (`picture.jpg` and the like in its folder), as the launcher's Maps
/// page reads it; `None` when the map ships none.
pub(crate) fn map_picture(root: &Path, map: &str) -> Option<Decoded> {
    let dir = MapPicture::folder(root, map)?;
    let img = MapPicture::read(&MapPicture::find(&dir)?)?;
    Some(Decoded {
        w: img.width(),
        h: img.height(),
        rgba: img.into_raw(),
    })
}

/// The game's name for a folder in a path (`Maps/Grundorf/global.cfg` is `Grundorf`).
fn folder(path: &str) -> String {
    folder_of(path).unwrap_or_default()
}

impl App {
    /// While the first area loads: hand the overlay what the screen says, and start the
    /// screenshots on their way (once).
    pub(crate) fn new_loading_update(
        &mut self,
        renderer: &mut omsi_render::Renderer,
        done: usize,
        total: usize,
    ) {
        let mirror = (!self.new_ui.installed())
            .then(|| self.d3d9_ui_mirror())
            .flatten();
        self.new_ui.install(renderer, mirror);
        let map_folder = folder(&self.args.map);
        self.new_ui.start_slides(&map_folder);
        self.new_ui
            .start_map_picture(&self.args.root, &self.args.map);
        if let Some(b) = self.args.bus.as_deref() {
            self.new_ui
                .start_bus(&self.args.root, b, self.args.paint.as_deref());
        }
        let name = self
            .world
            .as_ref()
            .map(|w| {
                let g = &w.global;
                let name = if g.friendly_name.trim().is_empty() {
                    g.name.clone()
                } else {
                    g.friendly_name.clone()
                };
                name
            })
            .unwrap_or_default();
        // (the folder's name until the bus's file has been read)
        let bus = self.args.bus.as_deref().map(|b| {
            self.new_ui.bus_card().unwrap_or_else(|| CardText {
                title: folder(b),
                sub: String::new(),
                detail: String::new(),
            })
        });
        let frame = LoadingFrame {
            version: VERSION.to_string(),
            heading: heading(name.trim(), self.args.line.as_deref(), &self.args.time),
            stage: stage(done, total),
            activity: activity(done).to_string(),
            progress: done as f32 / total.max(1) as f32,
            bus,
            map: Some(CardText {
                title: name.trim().to_string(),
                sub: if map_folder.eq_ignore_ascii_case(name.trim()) {
                    String::new()
                } else {
                    map_folder
                },
                detail: String::new(),
            }),
        };
        self.new_ui.set_loading(Some(frame));
    }
}

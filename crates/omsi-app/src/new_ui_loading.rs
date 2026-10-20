//! The loading screen of the new interface: while the first area of a streamed map loads,
//! `omsi_ui::loading` is drawn through the renderer's overlay hook over the blank picture the
//! game renders then - the player's own screenshots of that map as a slideshow, a progress
//! bar, and the bus and map being loaded. No screenshots: the default background.
//!
//! The screenshots are listed and decoded on a thread of their own (a few, shrunk to fit the
//! screen) and handed over one by one; the overlay, which must not block, only uploads them.

use crate::app::App;
use std::path::{Path, PathBuf};

/// How many screenshots the slideshow keeps (the latest ones).
pub(crate) const MAX_SLIDES: usize = 6;
/// A picture wider than this is shrunk to it before it goes to the GPU.
const SLIDE_WIDTH: u32 = 1280;
const DESCRIPTION_CHARS: usize = 240;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CardText {
    pub title: String,
    pub sub: String,
    pub description: String,
}

/// What the screen says in this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LoadingFrame {
    pub version: String,
    pub heading: String,
    pub stage: String,
    pub progress: f32,
    pub bus: Option<CardText>,
    pub map: Option<CardText>,
}

/// A screenshot ready for the GPU.
pub(crate) struct Decoded {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// "Example map · 08:00"
pub(crate) fn heading(map: &str, time: &str) -> String {
    match (map.trim(), time.trim()) {
        ("", t) => t.to_string(),
        (m, "") => m.to_string(),
        (m, t) => format!("{m} \u{b7} {t}"),
    }
}

/// "Loading map tiles (first area) · 62 of 100"
pub(crate) fn stage(done: usize, total: usize) -> String {
    format!("Loading map tiles (first area) \u{b7} {done} of {total}")
}

/// The author's text on one line, cut to `max` characters.
pub(crate) fn short(text: &str, max: usize) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= max {
        return one;
    }
    let cut: String = one.chars().take(max.saturating_sub(1)).collect();
    format!("{}\u{2026}", cut.trim_end())
}

/// The last `n` of `paths` (screenshots, oldest first), still oldest first.
pub(crate) fn recent(paths: Vec<PathBuf>, n: usize) -> Vec<PathBuf> {
    let skip = paths.len().saturating_sub(n);
    paths.into_iter().skip(skip).collect()
}

/// A screenshot as RGBA, shrunk to `SLIDE_WIDTH` when wider. None when it cannot be read.
pub(crate) fn decode(path: &Path) -> Option<Decoded> {
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

/// The game's name for a folder in a path (`Maps/Grundorf/global.cfg` is `Grundorf`).
fn folder(path: &str) -> String {
    crate::screenshot_name::folder_of(path).unwrap_or_default()
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
        let mirror = (!self.new_ui.installed()).then(|| self.d3d9_ui_mirror()).flatten();
        self.new_ui.install(renderer, mirror);
        let map_folder = folder(&self.args.map);
        self.new_ui.start_slides(&map_folder);
        let (name, description) = self
            .world
            .as_ref()
            .map(|w| {
                let g = &w.global;
                let name = if g.friendly_name.trim().is_empty() {
                    g.name.clone()
                } else {
                    g.friendly_name.clone()
                };
                (name, g.description.clone())
            })
            .unwrap_or_default();
        let bus = self.args.bus.as_deref().map(|b| CardText {
            title: folder(b),
            sub: String::new(),
            description: String::new(),
        });
        let frame = LoadingFrame {
            version: crate::startup::VERSION.to_string(),
            heading: heading(name.trim(), &self.args.time),
            stage: stage(done, total),
            progress: done as f32 / total.max(1) as f32,
            bus,
            map: Some(CardText {
                title: name.trim().to_string(),
                sub: if map_folder.eq_ignore_ascii_case(name.trim()) {
                    String::new()
                } else {
                    map_folder
                },
                description: short(&description, DESCRIPTION_CHARS),
            }),
        };
        self.new_ui.set_loading(Some(frame));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_heading_and_the_stage_read_as_in_the_mockup() {
        assert_eq!(heading("Example map", "08:00"), "Example map \u{b7} 08:00");
        assert_eq!(heading("", "08:00"), "08:00");
        assert_eq!(heading("Example map", " "), "Example map");
        assert_eq!(
            stage(62, 100),
            "Loading map tiles (first area) \u{b7} 62 of 100"
        );
    }

    #[test]
    fn a_description_is_one_line_cut_at_the_limit() {
        assert_eq!(short("a  b\n\nc", 20), "a b c");
        let long = "word ".repeat(100);
        let s = short(&long, 40);
        assert!(s.chars().count() <= 40, "{s}");
        assert!(s.ends_with('\u{2026}'));
        assert_eq!(short("", 10), "");
    }

    #[test]
    fn the_slideshow_keeps_the_latest_screenshots_in_order() {
        let p = |n: u32| PathBuf::from(format!("omsi_{n}.png"));
        let all: Vec<PathBuf> = (1..=10).map(p).collect();
        assert_eq!(recent(all.clone(), 3), vec![p(8), p(9), p(10)]);
        assert_eq!(recent(all[..2].to_vec(), 6), all[..2].to_vec());
        assert!(recent(Vec::new(), 6).is_empty());
    }

    #[test]
    fn a_wide_screenshot_is_shrunk_and_a_bad_file_is_none() {
        let dir = std::env::temp_dir().join(format!("depot-loading-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wide = dir.join("wide.png");
        image::RgbaImage::from_pixel(2560, 1440, image::Rgba([10, 20, 30, 255]))
            .save(&wide)
            .unwrap();
        let small = dir.join("small.png");
        image::RgbaImage::from_pixel(400, 300, image::Rgba([1, 2, 3, 255]))
            .save(&small)
            .unwrap();
        let junk = dir.join("junk.png");
        std::fs::write(&junk, b"not a picture").unwrap();
        let d = decode(&wide).unwrap();
        assert_eq!((d.w, d.h), (1280, 720));
        assert_eq!(d.rgba.len(), (d.w * d.h * 4) as usize);
        let d = decode(&small).unwrap();
        assert_eq!((d.w, d.h), (400, 300));
        assert!(decode(&junk).is_none());
        assert!(decode(&dir.join("missing.png")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

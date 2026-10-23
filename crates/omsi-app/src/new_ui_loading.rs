//! The loading screen of the new interface: while the first area of a streamed map loads,
//! `omsi_ui::loading` is drawn through the renderer's overlay hook over the blank picture the
//! game renders then - the player's own screenshots of that map as a slideshow, a progress
//! bar, and the bus and map being loaded. No screenshots: the default background.
//!
//! The screenshots are listed and decoded on a thread of their own (a few, shrunk to fit the
//! screen) and handed over one by one; the overlay, which must not block, only uploads them.
//! The bus's own name, maker and description are read from its file on another, once.

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
    /// Under the bar, left of the percentage.
    pub activity: String,
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

/// "Example map · Line 50 · 08:00" (the parts there are).
pub(crate) fn heading(map: &str, line: Option<&str>, time: &str) -> String {
    let line = line
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| omsi_ui::tr("Line %{line}").replace("%{line}", l));
    [
        Some(map.trim().to_string()),
        line,
        Some(time.trim().to_string()),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" \u{b7} ")
}

/// What is being done, under the bar: the map's files are read before its first tile comes.
pub(crate) fn activity(done: usize) -> &'static str {
    if done == 0 {
        "Reading the map"
    } else {
        "Preparing the world"
    }
}

/// The bus's block of the card from its file: its type as the title, the maker and the paint
/// under it, the author's description. `folder` names it when the file gives no name; a paint
/// asked for by its number is left out (the file's default one is shown then).
pub(crate) fn bus_card(v: &omsi_vehicle::Vehicle, folder: &str, paint: Option<&str>) -> CardText {
    let title = [v.type_name.trim(), folder.trim()]
        .into_iter()
        .find(|t| !t.is_empty())
        .unwrap_or_default()
        .to_string();
    let paint = paint
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(v.default_paint.trim());
    let sub = [v.manufacturer.trim(), paint]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" \u{b7} ");
    CardText {
        title,
        sub,
        description: short(&v.description, DESCRIPTION_CHARS),
    }
}

/// "Loading map tiles (first area) · 62 of 100"
pub(crate) fn stage(done: usize, total: usize) -> String {
    omsi_ui::tr("Loading map tiles (first area) · %{done} of %{total}")
        .replace("%{done}", &done.to_string())
        .replace("%{total}", &total.to_string())
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
        let mirror = (!self.new_ui.installed())
            .then(|| self.d3d9_ui_mirror())
            .flatten();
        self.new_ui.install(renderer, mirror);
        let map_folder = folder(&self.args.map);
        self.new_ui.start_slides(&map_folder);
        if let Some(b) = self.args.bus.as_deref() {
            self.new_ui
                .start_bus(&self.args.root, b, self.args.paint.as_deref());
        }
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
        // (the folder's name until the bus's file has been read)
        let bus = self.args.bus.as_deref().map(|b| {
            self.new_ui.bus_card().unwrap_or_else(|| CardText {
                title: folder(b),
                sub: String::new(),
                description: String::new(),
            })
        });
        let frame = LoadingFrame {
            version: crate::startup::VERSION.to_string(),
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
        assert_eq!(
            heading("Example map", Some("50"), "08:00"),
            "Example map \u{b7} Line 50 \u{b7} 08:00"
        );
        assert_eq!(
            heading("Example map", None, "08:00"),
            "Example map \u{b7} 08:00"
        );
        assert_eq!(heading("", Some(" "), "08:00"), "08:00");
        assert_eq!(heading("Example map", None, " "), "Example map");
        assert_eq!(activity(0), "Reading the map");
        assert_eq!(activity(62), "Preparing the world");
        assert_eq!(
            stage(62, 100),
            "Loading map tiles (first area) \u{b7} 62 of 100"
        );
    }

    #[test]
    fn the_bus_block_reads_as_in_the_mockup() {
        let v = omsi_vehicle::Vehicle {
            manufacturer: "Example Maker".into(),
            type_name: "Example bus A".into(),
            default_paint: "Default paint".into(),
            description: "Author's description,\nfirst lines.".into(),
            ..Default::default()
        };
        let c = bus_card(&v, "EXAMPLE_BUS", None);
        assert_eq!(c.title, "Example bus A");
        assert_eq!(c.sub, "Example Maker \u{b7} Default paint");
        assert_eq!(c.description, "Author's description, first lines.");
        // a paint asked for by name is shown, by number the default one
        assert_eq!(
            bus_card(&v, "", Some("Red")).sub,
            "Example Maker \u{b7} Red"
        );
        assert_eq!(
            bus_card(&v, "", Some("3")).sub,
            "Example Maker \u{b7} Default paint"
        );
        // a file with no name: the folder's
        let bare = omsi_vehicle::Vehicle::default();
        let c = bus_card(&bare, "EXAMPLE_BUS", None);
        assert_eq!((c.title.as_str(), c.sub.as_str()), ("EXAMPLE_BUS", ""));
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

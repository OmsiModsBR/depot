//! The loading screen's state: what it says in a frame (the heading, the stage, the cards of
//! the bus and the map) and the screenshots ready for the GPU, with the texts made from the
//! game's names.

use std::path::PathBuf;

/// How many screenshots the slideshow keeps (the latest ones).
pub(crate) const MAX_SLIDES: usize = 6;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CardText {
    pub title: String,
    /// The make (the bus), the map's folder.
    pub sub: String,
    /// The last, quieter line: the bus's livery.
    pub detail: String,
}

/// What the screen says in this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadingFrame {
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
pub struct Decoded {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// "Example map · Line 50 · 08:00" (the parts there are).
pub fn heading(map: &str, line: Option<&str>, time: &str) -> String {
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
pub fn activity(done: usize) -> &'static str {
    if done == 0 {
        "Reading the map"
    } else {
        "Preparing the world"
    }
}

/// The bus's block of the card from its file: its model as the title, the make under it and
/// the livery on the last line. `folder` names it when the file gives no name; a paint asked
/// for by its number is left out (the file's default one is shown then). The author's long
/// description is not part of the card.
pub fn bus_card(v: &omsi_vehicle::Vehicle, folder: &str, paint: Option<&str>) -> CardText {
    let title = [v.type_name.trim(), folder.trim()]
        .into_iter()
        .find(|t| !t.is_empty())
        .unwrap_or_default()
        .to_string();
    let paint = paint
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(v.default_paint.trim());
    CardText {
        title,
        sub: v.manufacturer.trim().to_string(),
        detail: if paint.is_empty() {
            String::new()
        } else {
            omsi_ui::tr("Livery: %{name}").replace("%{name}", paint)
        },
    }
}

/// "Loading map tiles (first area) · 62 of 100"
pub fn stage(done: usize, total: usize) -> String {
    omsi_ui::tr("Loading map tiles (first area) · %{done} of %{total}")
        .replace("%{done}", &done.to_string())
        .replace("%{total}", &total.to_string())
}

/// The last `n` of `paths` (screenshots, oldest first), still oldest first.
pub fn recent(paths: Vec<PathBuf>, n: usize) -> Vec<PathBuf> {
    let skip = paths.len().saturating_sub(n);
    paths.into_iter().skip(skip).collect()
}

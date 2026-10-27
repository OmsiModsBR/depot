//! The paper timetable in the driver's cab.
//!
//! Stock vehicle models show this through `[matl_freetex] file_schedule`. OMSI supplies the
//! bitmap named by that string; openOMSI makes it from the player's current duty and keeps
//! it in its own cache so the original installation remains read-only.

use crate::schedule::PlayerDuty;
use ab_glyph::{Font as _, FontVec, PxScale, ScaleFont};
use anyhow::{anyhow, Context, Result};
use omsi_content::font::{Font, FontAtlas, FontChar, TextAlign};
use omsi_sim::VehicleInstance;
use omsi_texture::Image;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// OMSI writes the paper as one text with DrawTextW into the rectangle from
// (60, 90) to the bitmap's right edge, in its typewriter font (Courier New) bold at 16 pt.
pub const PAPER_X: u32 = 60;
pub const PAPER_TOP: u32 = 90;
/// 16 pt at 96 DPI: the font's em in pixels (`-MulDiv(16, 96, 72)`).
const FONT_EM: f32 = 21.0;
/// A label is cut, or filled with dots, to this many characters.
const LABEL_CHARS: usize = 25;
/// Rows of the first column; the next 24 make the second, the rest are left out.
const ROWS_PER_COLUMN: usize = 24;
const TEXT_COLOR: [u8; 3] = [17, 15, 14];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperRow {
    pub name: String,
    pub time: String,
}

/// Update `file_schedule` to a cached drawing of the current trip. The renderer already
/// handles the model's `[matl_freetex]` slot, so switching this string updates the paper.
pub(crate) fn update_vehicle(
    vehicle: &mut VehicleInstance,
    duty: &PlayerDuty,
    fonts: &mut omsi_sim::texttex::FontLibrary,
) -> Result<()> {
    let (arr, dep) = tt_labels();
    let (title, rows) = paper_content(
        &duty.line,
        &duty.tour,
        &duty.trips,
        duty.trip_index,
        (arr, dep),
    );
    let lines = paper_lines(&title, &rows);
    let signature = content_signature(&lines);
    let path = cache_dir()?.join(format!("schedule-v7-{signature:016x}.png"));
    let filename = path.to_string_lossy().into_owned();

    if vehicle.str_var("file_schedule") == filename {
        return Ok(());
    }

    if !path.is_file() {
        let Some(font) = schedule_font(fonts, &lines) else {
            set_filename(vehicle, "");
            return Err(anyhow!(
                "no OMSI bitmap font is available for the driver's timetable"
            ));
        };
        let mut image = paper_base();
        draw_schedule(&mut image, &font, &lines);
        save_png(&image, &path)?;
    }
    set_filename(vehicle, &filename);
    Ok(())
}

fn cache_dir() -> Result<PathBuf> {
    let data = omsi_cfg::paths::data_dir()
        .ok_or_else(|| anyhow!("cannot locate the Project Depot user data folder"))?;
    Ok(data.join("cache").join("schedules"))
}

fn set_filename(vehicle: &mut VehicleInstance, value: &str) {
    if let Some(i) = vehicle.ty.program.str_var("file_schedule") {
        vehicle.state.str_vars[i as usize] = value.to_string();
    } else {
        log::debug!(
            "{} has no file_schedule string variable",
            vehicle.ty.def.type_name
        );
    }
}

fn schedule_font(
    fonts: &mut omsi_sim::texttex::FontLibrary,
    lines: &[String],
) -> Option<std::sync::Arc<FontAtlas>> {
    typewriter_font(lines).map(std::sync::Arc::new).or_else(|| {
        ["19_HHAschedule_font", "DIN Narrow", "DIN_Narrow", "DIN"]
            .into_iter()
            .find_map(|name| fonts.load(name))
    })
}

/// OMSI prints the schedule in a bold typewriter face, rather than a bus display's
/// proportional bitmap font. Rasterize an installed equivalent into fixed-width cells.
fn typewriter_font(lines: &[String]) -> Option<FontAtlas> {
    static FONT: OnceLock<Option<FontVec>> = OnceLock::new();
    let font = FONT
        .get_or_init(|| {
            #[cfg(target_os = "windows")]
            let paths = [std::env::var_os("WINDIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("C:/Windows"))
                .join("Fonts/courbd.ttf")];
            #[cfg(target_os = "macos")]
            let paths = [PathBuf::from(
                "/System/Library/Fonts/Supplemental/Courier New Bold.ttf",
            )];
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            let paths = [
                PathBuf::from("/usr/share/fonts/truetype/liberation2/LiberationMono-Bold.ttf"),
                PathBuf::from("/usr/share/fonts/truetype/liberation/LiberationMono-Bold.ttf"),
                PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"),
            ];
            paths
                .iter()
                .find_map(|path| FontVec::try_from_vec(std::fs::read(path).ok()?).ok())
        })
        .as_ref()?;
    // GDI sizes a font by its em; ab_glyph by ascent - descent
    let units = font.units_per_em().unwrap_or(2048.0);
    let px = PxScale::from(FONT_EM * font.height_unscaled() / units);
    let scaled = font.as_scaled(px);
    // a line is the font's height as GDI makes it, the ascent and the descent each rounded
    // to whole pixels (Courier New bold at 16 pt: 17.5 + 6.3 = 17 + 6 = 23 pixels), the
    // baseline at the ascent. The sum rounded made it 24, and the rows ran down past the
    // paper's lines, one pixel more with every row (#629)
    let line_height = gdi_line_height(scaled.ascent(), scaled.descent());
    let cell_width = scaled.h_advance(scaled.glyph_id('M')).round().max(1.0) as u32;
    let mut characters: Vec<char> = (32u8..=126).map(char::from).collect();
    for line in lines {
        characters.extend(line.chars());
    }
    characters.sort_unstable();
    characters.dedup();
    characters.retain(|&ch| scaled.glyph_id(ch).0 != 0);
    let width = cell_width * characters.len() as u32;
    let mut alpha = vec![0; (width * line_height * 4) as usize];
    let mut chars = Vec::with_capacity(characters.len());
    for (index, ch) in characters.into_iter().enumerate() {
        let x0 = index as u32 * cell_width;
        chars.push(FontChar {
            ch,
            x0: x0 as i32,
            x1: (x0 + cell_width) as i32,
            y: 0,
        });
        let glyph = scaled
            .glyph_id(ch)
            .with_scale_and_position(px, ab_glyph::point(0.0, scaled.ascent().round()));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|px, py, coverage| {
                let x = px as i32 + bounds.min.x as i32;
                let y = py as i32 + bounds.min.y as i32;
                if x >= 0 && x < cell_width as i32 && y >= 0 && y < line_height as i32 {
                    let offset = ((y as u32 * width + x0 + x as u32) * 4) as usize;
                    let a = (coverage * 255.0).round() as u8;
                    alpha[offset..offset + 4].fill(a);
                }
            });
        }
    }
    Some(FontAtlas::new(
        Font {
            name: "Project Depot timetable".into(),
            height: line_height as i32,
            gap: 0,
            chars,
            ..Default::default()
        },
        width,
        line_height,
        vec![0; alpha.len()],
        alpha,
    ))
}

/// GDI's `tmHeight` from a font's ascent and (negative) descent in pixels: each is rounded
/// by itself.
pub fn gdi_line_height(ascent: f32, descent: f32) -> u32 {
    (ascent.round() + (-descent).round()).max(1.0) as u32
}

/// `TT_Arr` and `TT_Dep` of the game's language (`Languages/<LANG>_basic.olf`), as
/// OMSI translates them for the paper: "Arrival " and "Depart." in English.
fn tt_labels() -> &'static (String, String) {
    static LABELS: OnceLock<(String, String)> = OnceLock::new();
    LABELS.get_or_init(|| {
        let lang = crate::settings::Settings::load().language;
        let find = |lang: &str| {
            omsi_cfg::content_dirs("Languages")
                .into_iter()
                .find_map(|dir| {
                    omsi_content::language::Language::load(&dir.join(format!("{lang}_basic.olf")))
                        .ok()
                })
        };
        let l = find(&lang).or_else(|| find("ENG"));
        let get = |key: &str, default: &str| {
            l.as_ref()
                .and_then(|l| l.strings.get(key))
                .filter(|v| !v.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| default.to_string())
        };
        (get("TT_Arr", "Arrival "), get("TT_Dep", "Depart. "))
    })
}

/// The rows of the paper as OMSI makes them: every stop of the trip with
/// its departure (a station the profile passes is left out), the last one as
/// "<stop> <TT_Arr>" with its arrival, then one more row "<TT_Dep>" with the last stop's
/// departure.
pub fn paper_content(
    duty_line: &str,
    duty_tour: &str,
    trips: &[crate::schedule::PlannedTrip],
    trip_index: usize,
    (arr_label, dep_label): (&str, &str),
) -> (String, Vec<PaperRow>) {
    let trip = &trips[trip_index];
    let line = if trip.line.trim().is_empty() {
        duty_line.trim()
    } else {
        trip.line.trim()
    };
    let title = format!("{line} - {} - {}", trip.terminus.trim(), duty_tour.trim());

    let mut rows = Vec::new();
    let Some(last) = trip.stops.len().checked_sub(1) else {
        return (title, rows);
    };
    for i in 0..=last + 1 {
        let stop = &trip.stops[i.min(last)];
        if i < last && !stop.stops {
            continue;
        }
        // (the station's name, a blank and its second name - empty for a map's bus stop -
        // and another blank)
        let name = if i > last {
            dep_label.to_string()
        } else if i == last {
            format!("{}  {arr_label}", stop.name.trim())
        } else {
            format!("{}  ", stop.name.trim())
        };
        let time = format_time(if i == last { stop.arr } else { stop.dep });
        rows.push(PaperRow { name, time });
    }
    (title, rows)
}

pub fn format_time(seconds: f64) -> String {
    let minute = (seconds / 60.0).round() as i64;
    let minute = minute.rem_euclid(24 * 60);
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

/// The text OMSI draws: the title, a rule of 67 '=', then the rows - each the label
/// cut or filled with dots to 25 characters, a blank and the time; rows 25 to 48 stand
/// beside the first 24, five blanks apart.
pub fn paper_lines(title: &str, rows: &[PaperRow]) -> Vec<String> {
    let cell = |row: &PaperRow| {
        let mut label: String = row.name.chars().take(LABEL_CHARS).collect();
        let n = label.chars().count();
        label.extend(std::iter::repeat_n('.', LABEL_CHARS - n));
        format!("{label} {}", row.time)
    };
    let (a, b) = rows.split_at(rows.len().min(ROWS_PER_COLUMN));
    let b = &b[..b.len().min(ROWS_PER_COLUMN)];
    let mut lines = vec![title.to_string(), "=".repeat(67)];
    for (i, row) in a.iter().enumerate() {
        let mut line = cell(row);
        if let Some(other) = b.get(i) {
            line.push_str("     ");
            line.push_str(&cell(other));
        }
        lines.push(line);
    }
    lines
}

fn content_signature(lines: &[String]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    lines.hash(&mut hasher);
    hasher.finish()
}

fn paper_base() -> Image {
    let dirs = omsi_cfg::content_dirs("Texture");
    let refs: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    omsi_texture::find_texture("Schedule.bmp", &refs)
        .and_then(|path| omsi_texture::decode_file(&path).ok())
        .unwrap_or_else(|| Image {
            width: 1024,
            height: 1024,
            rgba: [230, 227, 216, 255]
                .into_iter()
                .cycle()
                .take(1024 * 1024 * 4)
                .collect(),
            has_alpha: false,
        })
}

/// Draw the lines from (60, 90) down, one font height apart, as DrawTextW does; what
/// runs off the bitmap is cut off.
pub fn draw_schedule(image: &mut Image, font: &FontAtlas, lines: &[String]) {
    let line_height = font.font.height.max(1) as u32;
    for (i, line) in lines.iter().enumerate() {
        draw_text(
            image,
            font,
            line,
            PAPER_X,
            PAPER_TOP + i as u32 * line_height,
        );
    }
}

fn draw_text(image: &mut Image, font: &FontAtlas, text: &str, x: u32, y: u32) {
    if x >= image.width || y >= image.height || text.is_empty() {
        return;
    }
    let source_width = (font.text_width(text).max(0) as u32).max(1);
    let source_height = font.font.height.max(1) as u32;
    let rgba = font.render_aligned(
        text,
        source_width,
        source_height,
        false,
        TEXT_COLOR,
        TextAlign {
            orientation: 1,
            grid: 1,
        },
    );
    let width = source_width.min(image.width - x);
    let height = source_height.min(image.height - y);
    for py in 0..height {
        for px in 0..width {
            let alpha = rgba[((py * source_width + px) * 4 + 3) as usize] as u32;
            if alpha == 0 {
                continue;
            }
            let target = (((y + py) * image.width + x + px) * 4) as usize;
            for channel in 0..3 {
                let ink = TEXT_COLOR[channel] as u32;
                let paper = image.rgba[target + channel] as u32;
                image.rgba[target + channel] = ((ink * alpha + paper * (255 - alpha)) / 255) as u8;
            }
            image.rgba[target + 3] = 255;
        }
    }
}

fn save_png(image: &Image, path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("schedule cache path has no parent"))?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    let buffer = image::RgbaImage::from_raw(image.width, image.height, image.rgba.clone())
        .ok_or_else(|| anyhow!("schedule image has the wrong pixel count"))?;
    buffer
        .save(path)
        .with_context(|| format!("writing {}", path.display()))
}

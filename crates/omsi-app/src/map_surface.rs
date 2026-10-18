//! A map's pre-built surface: water, green, built-up areas, rivers, rail, borders and place
//! names under the roads of the navigator and the city map.
//!
//! The map author ships it: `Maps/<Map>/openomsi-realmap/` holds `manifest.cfg` and
//! `surface.oms` (docs/MODDING.md, "A map's surface"). Everything is made beforehand -
//! triangulated and cut into chunks - so this file only checks the header, keeps the chunk
//! index and turns the chunks near the camera into vertices. Nothing is computed from
//! OpenStreetMap or any other source here, and a map without the folder looks as before.
//!
//! Positions are metres of the author's tile grid (`grid` in the manifest: column times grid
//! plus the place in the tile), which is what OMSI 2 itself shows for the map's objects; they
//! are put onto this game's grid with `omsi_map::tile_local_to_world`, so a `[worldcoordinates]`
//! map (whose grid is not 300 m) lands where its roads are.

use std::collections::HashMap;
use std::path::Path;

use glam::{DVec2, Vec2, Vec3};
use omsi_ui::{Color, Painter, Vertex};

const FOLDER: &str = "openomsi-realmap";
const MAGIC: &[u8; 4] = b"OMSF";
const VERSION: u32 = 1;

/// The drawing classes of the file (unknown ones are skipped, so a later version can add more).
const FILL_WATER: u8 = 0;
const FILL_GREEN: u8 = 1;
const FILL_BUILT: u8 = 2;
const FILL_GRASS: u8 = 3;
const FILL_FOREST: u8 = 4;
const FILL_SPORT: u8 = 5;
const FILL_FARM: u8 = 6;
const FILL_CLASSES: usize = 7;
const LINE_RIVER: u8 = 0;
const LINE_RAIL: u8 = 1;
const LINE_BORDER: u8 = 2;
/// A named street: not drawn, only looked up ("you are on ...").
const LINE_STREET: u8 = 3;
/// A named street that also says its limits: `width` is the limit (km/h, 0 = none), `flags` the limit of heavy
/// vehicles (km/h, 0 = the same, or none).
const LINE_STREET_LIMITS: u8 = 4;
/// Where the height of a contour line is written: a point on it, the height (m) and whether it is an index line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContourMark {
    pub at: DVec2,
    pub height: i32,
    pub index: bool,
}

/// How strongly the contour lines are drawn at `mpp` metres a pixel: full close up, fainter as the view widens.
pub fn contour_strength(mpp: f64) -> f32 {
    if mpp < 4.0 {
        1.0
    } else if mpp < 12.0 {
        0.5
    } else {
        0.2
    }
}

/// The marks for the heights along one contour line: the first after 150 m (index lines) or 100 m, then every
/// 700 m or 450 m of the line; a mark sits at the middle of the segment that reaches the distance.
fn height_marks(pts: &[DVec2], height: i32, index: bool) -> Vec<ContourMark> {
    let (first, every) = if index { (150.0, 700.0) } else { (100.0, 450.0) };
    let (mut out, mut run, mut next) = (Vec::new(), 0.0, first);
    for s in pts.windows(2) {
        run += (s[1] - s[0]).length();
        if run >= next {
            out.push(ContourMark { at: (s[0] + s[1]) * 0.5, height, index });
            next = run + every;
        }
    }
    out
}

/// The marks to write at `mpp` metres a pixel: all of them close up (under 6), only the index lines' up to 40, and
/// none over another (at least 64 px apart close up, 120 px otherwise, times the interface scale `ui`); only those
/// `inside` the window. Returns each with its place on the screen.
pub fn marks_to_write<'a>(
    marks: &'a [ContourMark],
    mpp: f64,
    ui: f32,
    to_screen: impl Fn(DVec2) -> Vec2,
    inside: impl Fn(Vec2) -> bool,
) -> Vec<(Vec2, &'a ContourMark)> {
    let near = mpp < 6.0;
    if mpp >= 40.0 {
        return Vec::new();
    }
    let apart = if near { 64.0 } else { 120.0 } * ui;
    let mut shown: Vec<(Vec2, &ContourMark)> = Vec::new();
    for m in marks.iter().filter(|m| near || m.index) {
        let p = to_screen(m.at);
        if inside(p) && !shown.iter().any(|(q, _)| (*q - p).length() < apart) {
            shown.push((p, m));
        }
    }
    shown
}

/// A contour line of the terrain (`width` is its height in metres) and an index contour (every fifth: drawn heavier).
const LINE_CONTOUR: u8 = 5;
const LINE_CONTOUR_INDEX: u8 = 6;

/// A street found by place: its name and its speed limits (km/h, 0 = not said).
#[derive(Clone, Debug, PartialEq)]
pub struct Street {
    pub name: String,
    pub speed: u32,
    /// For heavy vehicles, when it is not the same.
    pub heavy: u32,
}

/// What the author's manifest may colour (`water = #rrggbb`, and `light_water = ...` for the
/// light map); a key left out keeps the default of that look.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// The bare ground the map is drawn on.
    pub land: Color,
    pub water: Color,
    pub river: Color,
    /// Parks and gardens, grass and meadows, woods and forests, sports grounds, farmland.
    pub green: Color,
    pub grass: Color,
    pub forest: Color,
    pub sport: Color,
    pub farm: Color,
    pub built: Color,
    pub rail: Color,
    pub rail_edge: Color,
    pub border: Color,
    pub contour: Color,
}

impl Palette {
    pub fn dark() -> Palette {
        Palette {
            land: Color::hex(0x1c2329),
            water: Color::hex(0x173341),
            river: Color::hex(0x2b6a86),
            green: Color::hex(0x24402f),
            grass: Color::hex(0x1f3a2b),
            forest: Color::hex(0x183021),
            sport: Color::hex(0x2a4a3a),
            farm: Color::hex(0x2b3a2a),
            built: Color::hex(0x252f37),
            rail: Color::hex(0x7d8a95),
            rail_edge: Color::hex(0x0c1013),
            border: Color::hex(0xb592d6),
            contour: Color::rgba(190, 150, 100, 0.8),
        }
    }

    pub fn light() -> Palette {
        Palette {
            land: Color::hex(0xe4e0d6),
            water: Color::hex(0x9cc3d6),
            river: Color::hex(0x7fb0cc),
            green: Color::hex(0xbfd8ab),
            grass: Color::hex(0xd0e0b8),
            forest: Color::hex(0xa9cb94),
            sport: Color::hex(0xc3dfb0),
            farm: Color::hex(0xe8e6c4),
            built: Color::hex(0xd6d1c3),
            rail: Color::hex(0x8c877b),
            rail_edge: Color::hex(0xf4f1ea),
            border: Color::hex(0x9070a8),
            contour: Color::rgba(150, 105, 60, 0.85),
        }
    }

    fn set(&mut self, key: &str, c: Color) {
        match key {
            "land" => self.land = c,
            "water" => self.water = c,
            "river" => self.river = c,
            "green" => self.green = c,
            "grass" => self.grass = c,
            "forest" => self.forest = c,
            "sport" => self.sport = c,
            "farm" => self.farm = c,
            "built" => self.built = c,
            "rail" => self.rail = c,
            "rail_edge" => self.rail_edge = c,
            "border" => self.border = c,
            "contour" => self.contour = c,
            _ => {}
        }
    }
}

/// A name on the map: where, what kind (0 district, 1 terminal, 2 town), how important
/// (the author's number, high shows from far away) and the text.
#[derive(Clone, Debug)]
pub struct Label {
    pub at: DVec2,
    pub kind: u8,
    pub rank: u8,
    pub name: String,
}

/// An icon on the map: where (game metres), which picture of the sheet and how wide it is
/// drawn (pixels at interface size 1). The file's last column, a name, is for people reading it.
#[derive(Clone, Debug)]
pub struct Marker {
    pub at: DVec2,
    pub icon: usize,
    pub width_px: f32,
}

/// Shelf packing of icon sizes into one sheet of at most 2048 x 2048 (safe on downlevel GPUs), one
/// transparent pixel round each icon. Returns the sheet size and each icon's corner, or `None` for
/// an icon that does not fit (it is left out with its markers).
fn pack_icons(sizes: &[(u32, u32)]) -> (u32, u32, Vec<Option<(u32, u32)>>) {
    const SHEET_MAX: u32 = 2048;
    let mut pos = Vec::with_capacity(sizes.len());
    let (mut x0, mut y0, mut row_h, mut w, mut h) = (1u32, 1u32, 0u32, 0u32, 0u32);
    for &(iw, ih) in sizes {
        if x0 + iw + 1 > SHEET_MAX {
            x0 = 1;
            y0 += row_h + 1;
            row_h = 0;
        }
        if x0 + iw + 1 > SHEET_MAX || y0 + ih + 1 > SHEET_MAX {
            log::warn!("map surface: icon sheet is full, an icon is left out");
            pos.push(None);
            continue;
        }
        pos.push(Some((x0, y0)));
        w = w.max(x0 + iw + 1);
        h = h.max(y0 + ih + 1);
        row_h = row_h.max(ih);
        x0 += iw + 1;
    }
    (w, h, pos)
}

/// The marker pictures packed side by side into one: its size and pixels (straight alpha) and,
/// per picture, where it lies (texture coordinates) and its height over its width.
pub struct IconSheet {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    rects: Vec<([f32; 4], f32)>,
}

/// How the player is drawn on the maps: a triangle in a colour by what the player drives.
#[derive(Clone, Debug)]
struct Player {
    free: Color,
    line: Color,
    brt: Color,
    brt_lines: Vec<String>,
}

/// A region of the map (a municipality): its outline, to say where the player is, and the box
/// its name fits in (`box_w` 0: no room for a readable name, so none is drawn).
#[derive(Clone, Debug)]
pub struct Region {
    pub name: String,
    pub label: DVec2,
    pub box_w: f64,
    pub box_h: f64,
    poly: Vec<DVec2>,
}

/// What kind of vehicle a line is for (`lines.lin`): each has its own list in the lines panel.
pub const MODAL_BUS: u8 = 0;
pub const MODAL_TRAM: u8 = 1;
pub const MODAL_METRO: u8 = 2;
pub const MODAL_RAIL: u8 = 3;

/// Kinds of bus line (`lines.lin`).
pub const GROUP_BRT: u8 = 1;
pub const GROUP_EXPRESS: u8 = 2;
pub const GROUP_FEEDER: u8 = 3;

/// One route of a bus line: what the official table says and the way it runs (game metres).
#[derive(Clone, Debug, Default)]
pub struct RouteInfo {
    pub km: f32,
    pub stops_n: u32,
    pub minutes: u32,
    pub circular: bool,
    pub days: String,
    pub model: String,
    pub itinerary: String,
    pub pts: Vec<DVec2>,
    pub stops: Vec<DVec2>,
}

/// A bus line of the map: number, kind, name and one or two routes.
#[derive(Clone, Debug)]
pub struct LineInfo {
    pub number: String,
    pub group: u8,
    pub modal: u8,
    /// The dates (yyyymmdd) this form of the line runs from and to; 0 is open. A map with eras has a record
    /// for each form a line took.
    pub from: u32,
    pub to: u32,
    pub name: String,
    pub routes: Vec<RouteInfo>,
}

struct Entry {
    cx: i32,
    cy: i32,
    at: usize,
    len: usize,
}

pub struct MapSurface {
    pub name: String,
    /// Attribution the data needs (the map's licence), drawn on the city map.
    pub credit: String,
    dark: Palette,
    light: Palette,
    origin: DVec2,
    grid: f64,
    chunk: f64,
    names: Vec<String>,
    index: Vec<Entry>,
    data: Vec<u8>,
    markers: Vec<Marker>,
    sheet: Option<IconSheet>,
    player: Option<Player>,
    by_key: HashMap<(i32, i32), usize>,
    regions: Vec<Region>,
    lines: Vec<LineInfo>,
    has_streets: bool,
    /// The package carries the terrain's contour lines.
    has_contours: bool,
    /// Where real north lies, in degrees clockwise from the map's own up (the manifest's `north`; 0 when it says nothing).
    north: f32,
    /// Added to the heights of the contour lines when the manifest says so (some maps' heights are relative).
    terrain_offset: f32,
    /// Where the index contours are written (a point and a height about every 700 m of them), made on first use.
    contour_marks: std::sync::OnceLock<Vec<ContourMark>>,
    /// What the map says its rail vehicles are: (a part of the vehicle file's path, MODAL_*), from the manifest's
    /// `tram_vehicles`, `metro_vehicles` and `rail_vehicles`. A rail vehicle named by none is drawn as a tram.
    rail_kinds: Vec<(String, u8)>,
    /// The map, for its timetable (read when the lines panel first asks).
    map_dir: std::path::PathBuf,
    times: std::sync::Mutex<Option<(i32, std::sync::Arc<crate::line_times::LineTimes>)>>,
}

/// Bounds-checked reading: any short or odd file ends in `None`, never a panic.
struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.i.checked_add(n)?;
        let s = self.b.get(self.i..end)?;
        self.i = end;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn f64(&mut self) -> Option<f64> {
        Some(f64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn left(&self) -> usize {
        self.b.len() - self.i
    }
}

fn parse_color(v: &str) -> Option<Color> {
    let h = v.trim().strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    u32::from_str_radix(h, 16).ok().map(Color::hex)
}

impl MapSurface {
    /// The map folder's package, or None when there is none or it cannot be used (one line
    /// in the log says why; the map then shows as it always did).
    pub fn open(map_dir: &Path) -> Option<MapSurface> {
        // the package sits in the map's own folder, or in the same place of another content root (a
        // package for a map one does not own goes into the game's content folder, as a mod does: the
        // OMSI 2 installation is never written to); the root with the most priority wins
        let dir = omsi_cfg::mirrored_dirs(map_dir)
            .into_iter()
            .map(|d| d.join(FOLDER))
            .find(|d| omsi_cfg::vfs::is_file(&d.join("manifest.cfg")))?;
        let manifest = omsi_cfg::vfs::read(&dir.join("manifest.cfg")).ok()?;
        let text = String::from_utf8_lossy(&manifest).into_owned();
        let (mut format, mut grid) = (0u32, 0.0f64);
        let (mut name, mut credit) = (String::new(), String::new());
        let mut triangle = false;
        let mut player_colors = [
            Color::hex(0xffffff),
            Color::hex(0x5588c7),
            Color::hex(0x1e7b34),
        ];
        let mut brt_lines: Vec<String> = Vec::new();
        let mut terrain_offset = 0.0f32;
        let mut north = 0.0f32;
        let mut kinds: Vec<(String, u8)> = Vec::new();
        let (mut dark, mut light) = (Palette::dark(), Palette::light());
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            match k.as_str() {
                "format" => format = v.parse().unwrap_or(0),
                "grid" => grid = v.parse().unwrap_or(0.0),
                "name" => name = v.to_string(),
                "credit" => credit = v.to_string(),
                "player" => triangle = v.eq_ignore_ascii_case("triangle"),
                "player_free" => player_colors[0] = parse_color(v).unwrap_or(player_colors[0]),
                "player_line" => player_colors[1] = parse_color(v).unwrap_or(player_colors[1]),
                "player_brt" => player_colors[2] = parse_color(v).unwrap_or(player_colors[2]),
                "terrain_offset" => terrain_offset = v.parse().unwrap_or(0.0),
                "north" => north = v.parse().ok().filter(|n: &f32| n.is_finite()).unwrap_or(0.0),
                "tram_vehicles" | "metro_vehicles" | "rail_vehicles" => {
                    let modal = match k.as_str() {
                        "tram_vehicles" => MODAL_TRAM,
                        "metro_vehicles" => MODAL_METRO,
                        _ => MODAL_RAIL,
                    };
                    kinds.extend(v.split(',').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).map(|p| (p, modal)));
                }
                "brt_lines" => {
                    brt_lines = v
                        .split(',')
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect()
                }
                _ => {
                    // a colour: `water = #..` for the dark map, `light_water = #..` for the light one
                    if let Some(c) = parse_color(v) {
                        match k.strip_prefix("light_") {
                            Some(key) => light.set(key, c),
                            None => dark.set(&k, c),
                        }
                    }
                }
            }
        }
        if format != VERSION || !(grid > 1.0) {
            log::warn!(
                "map surface: {} has format {format}, grid {grid}: not used",
                dir.display()
            );
            return None;
        }
        let data = omsi_cfg::vfs::read(&dir.join("surface.oms")).ok()?;
        match Self::parse(data, name, credit, dark, light, grid) {
            Some(mut s) => {
                if triangle {
                    s.player = Some(Player {
                        free: player_colors[0],
                        line: player_colors[1],
                        brt: player_colors[2],
                        brt_lines,
                    });
                }
                s.rail_kinds = kinds;
                s.load_markers(&dir);
                s.load_regions(&dir);
                s.load_lines(&dir);
                s.terrain_offset = terrain_offset;
                s.north = north;
                s.has_contours = s.index.iter().any(|e| s.data.get(e.at..e.at + e.len).is_some_and(chunk_has_contours));
                s.map_dir = map_dir.to_path_buf();
                s.has_streets = s.index.iter().any(|e| {
                    s.data
                        .get(e.at..e.at + e.len)
                        .is_some_and(chunk_has_streets)
                });
                log::info!(
                    "map surface: \"{}\", {} chunks, {} names from {}",
                    s.name,
                    s.index.len(),
                    s.names.len(),
                    dir.display()
                );
                if !s.markers.is_empty() {
                    log::info!("map surface: {} icons on the map", s.markers.len());
                }
                Some(s)
            }
            None => {
                log::warn!(
                    "map surface: {}/surface.oms is not a valid version {VERSION} file: not used",
                    dir.display()
                );
                None
            }
        }
    }

    fn parse(
        data: Vec<u8>,
        name: String,
        credit: String,
        dark: Palette,
        light: Palette,
        grid: f64,
    ) -> Option<MapSurface> {
        let mut r = Rd { b: &data, i: 0 };
        if r.take(4)? != MAGIC || r.u32()? != VERSION {
            return None;
        }
        let origin = DVec2::new(r.f64()?, r.f64()?);
        for _ in 0..4 {
            r.f32()?; // the extent: only readers that size a view by it need it
        }
        let chunk = r.f32()? as f64;
        if !(chunk > 1.0) || !origin.is_finite() {
            return None;
        }
        let n_names = r.u32()? as usize;
        if n_names > r.left() {
            return None;
        }
        let mut names = Vec::with_capacity(n_names);
        for _ in 0..n_names {
            let n = r.u16()? as usize;
            names.push(String::from_utf8_lossy(r.take(n)?).into_owned());
        }
        let n_chunks = r.u32()? as usize;
        if n_chunks.checked_mul(20)? > r.left() {
            return None;
        }
        let mut index = Vec::with_capacity(n_chunks);
        for _ in 0..n_chunks {
            let (cx, cy) = (r.i32()?, r.i32()?);
            let (at, len) = (r.u64()? as usize, r.u32()? as usize);
            if at.checked_add(len)? > data.len() {
                return None;
            }
            index.push(Entry { cx, cy, at, len });
        }
        let by_key = index
            .iter()
            .enumerate()
            .map(|(i, e)| ((e.cx, e.cy), i))
            .collect();
        Some(MapSurface {
            name,
            credit,
            dark,
            light,
            origin,
            grid,
            chunk,
            names,
            index,
            data,
            markers: Vec::new(),
            sheet: None,
            player: None,
            by_key,
            regions: Vec::new(),
            lines: Vec::new(),
            has_streets: false,
            has_contours: false,
            north: 0.0,
            terrain_offset: 0.0,
            contour_marks: std::sync::OnceLock::new(),
            map_dir: Default::default(),
            rail_kinds: Vec::new(),
            times: std::sync::Mutex::new(None),
        })
    }

    /// An author-grid position (metres from the file's origin) as game metres.
    fn to_world(&self, dx: f32, dy: f32) -> DVec2 {
        self.author_to_world(self.origin.x + dx as f64, self.origin.y + dy as f64)
    }

    /// An author-grid position (metres of the author's grid) as game metres.
    fn author_to_world(&self, ax: f64, ay: f64) -> DVec2 {
        let (tx, ty) = ((ax / self.grid).floor(), (ay / self.grid).floor());
        let (x, y) = omsi_map::tile_local_to_world(
            tx as i32,
            ty as i32,
            ax - tx * self.grid,
            ay - ty * self.grid,
        );
        DVec2::new(x, y)
    }

    /// `regions.cfg`: `region = name | x | y | box width | box height | x,y x,y ...`, metres of
    /// the author's grid. A line that cannot be read is left out.
    fn load_regions(&mut self, dir: &Path) {
        let Ok(bytes) = omsi_cfg::vfs::read(&dir.join("regions.cfg")) else {
            return;
        };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        self.regions = parse_regions(&text, &|x, y| self.author_to_world(x, y));
    }

    /// `lines.lin`: the bus lines (see `parse_lines`).
    fn load_lines(&mut self, dir: &Path) {
        let Ok(bytes) = omsi_cfg::vfs::read(&dir.join("lines.lin")) else {
            return;
        };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        self.lines = parse_lines(&text, &|x, y| self.author_to_world(x, y));
        if !self.lines.is_empty() {
            log::info!("map surface: {} bus lines", self.lines.len());
        }
    }

    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    pub fn lines(&self) -> &[LineInfo] {
        &self.lines
    }

    /// Where real north lies: degrees clockwise from the map's own up.
    pub fn north(&self) -> f32 {
        self.north
    }

    /// The package carries contour lines (the city map then has a button for them).
    pub fn has_contours(&self) -> bool {
        self.has_contours
    }

    /// The height at a game position, from the two nearest contour lines of different heights (between them it is
    /// interpolated by distance), plus the manifest's `terrain_offset`; None where no contour is near.
    pub fn altitude_at(&self, pos: DVec2) -> Option<f32> {
        if !self.has_contours {
            return None;
        }
        let a = self.world_to_author(pos) - self.origin;
        let (cx, cy) = ((a.x / self.chunk).floor() as i32, (a.y / self.chunk).floor() as i32);
        let mut found: Vec<(f64, f32)> = Vec::new();
        for dx in -2..=2 {
            for dy in -2..=2 {
                if let Some(&i) = self.by_key.get(&(cx + dx, cy + dy)) {
                    let e = &self.index[i];
                    if let Some(body) = self.data.get(e.at..e.at + e.len) {
                        let _ = contours_in_chunk(body, a, &mut found);
                    }
                }
            }
        }
        found.sort_by(|x, y| x.0.total_cmp(&y.0));
        let (d1, h1) = *found.first()?;
        if d1 > 1500.0 {
            return None;
        }
        let h = match found.iter().find(|(_, h)| (*h - h1).abs() > 0.01) {
            Some(&(d2, h2)) if d1 + d2 > 1e-6 => h1 + (h2 - h1) * (d1 / (d1 + d2)) as f32,
            _ => h1,
        };
        Some(h + self.terrain_offset)
    }

    /// Where the heights of the contour lines are written on the city map (made on first use).
    pub fn contour_marks(&self) -> &[ContourMark] {
        self.contour_marks.get_or_init(|| {
            let mut out = Vec::new();
            for e in &self.index {
                let Some(body) = self.data.get(e.at..e.at + e.len) else { continue };
                let _ = self.walk(
                    body,
                    |_, _| {},
                    |(class, w), pts| {
                        if class == LINE_CONTOUR || class == LINE_CONTOUR_INDEX {
                            out.extend(height_marks(&pts, (w + self.terrain_offset).round() as i32, class == LINE_CONTOUR_INDEX));
                        }
                    },
                    |_| {},
                );
            }
            out
        })
    }

    /// What kind of rail vehicle a vehicle file is, by what the map says (MODAL_TRAM, METRO or RAIL); None when
    /// the map does not name it.
    pub fn rail_kind(&self, vehicle_path: &str) -> Option<u8> {
        let p = vehicle_path.to_lowercase().replace('\\', "/");
        self.rail_kinds.iter().find(|(part, _)| p.contains(part.as_str())).map(|k| k.1)
    }

    /// Whether line record `i` holds on `date` (yyyymmdd; 0 = not known: all do).
    pub fn line_on(&self, i: usize, date: i32) -> bool {
        let l = &self.lines[i];
        date <= 0 || ((l.from == 0 || l.from as i32 <= date) && (l.to == 0 || date <= l.to as i32))
    }

    /// The map's timetable (`TTData`, with the eras in force): departures and the time at each stop, per line.
    /// Read again when the date (yyyymmdd; the eras and the school period follow it) changes.
    pub fn times(&self, chrono: &[std::path::PathBuf], date: i32) -> std::sync::Arc<crate::line_times::LineTimes> {
        let mut slot = self.times.lock().unwrap_or_else(|e| e.into_inner());
        match slot.as_ref() {
            Some((d, t)) if *d == date => t.clone(),
            _ => {
                let t = std::sync::Arc::new(crate::line_times::LineTimes::load(&self.map_dir, chrono, date));
                *slot = Some((date, t.clone()));
                t
            }
        }
    }

    /// The package can say where the player is (municipality outlines or named streets).
    pub fn has_location(&self) -> bool {
        !self.regions.is_empty() || self.has_streets
    }

    /// Game metres as metres of the author's grid (the inverse of `author_to_world`).
    fn world_to_author(&self, p: DVec2) -> DVec2 {
        let ((tx, ty), (lx, ly)) = omsi_map::world_to_tile_local(p.x, p.y);
        DVec2::new(tx as f64 * self.grid + lx, ty as f64 * self.grid + ly)
    }

    /// The municipality and the street at a game position: the region whose outline holds
    /// it, and the named street within 30 m (read from the chunks around, nothing kept).
    pub fn locate(&self, pos: DVec2) -> (Option<String>, Option<String>) {
        let region = self
            .regions
            .iter()
            .find(|r| point_in_poly(&r.poly, pos))
            .map(|r| r.name.clone());
        (region, self.street_info(pos, 30.0).map(|s| s.name))
    }

    /// The named street within `reach` metres of a game position, with its limits (read from the chunks around,
    /// nothing kept).
    pub fn street_info(&self, pos: DVec2, reach: f64) -> Option<Street> {
        let mut street = None;
        if self.has_streets {
            let a = self.world_to_author(pos) - self.origin;
            let (cx, cy) = (
                (a.x / self.chunk).floor() as i32,
                (a.y / self.chunk).floor() as i32,
            );
            let mut best: Option<(f64, u32, u32, u32)> = None;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let Some(&i) = self.by_key.get(&(cx + dx, cy + dy)) else {
                        continue;
                    };
                    let e = &self.index[i];
                    if let Some(body) = self.data.get(e.at..e.at + e.len) {
                        let _ = street_in_chunk(body, a, &mut best);
                    }
                }
            }
            street = best
                .filter(|b| b.0 <= reach)
                .map(|b| Street { name: self.name(b.1), speed: b.2, heavy: if b.3 == b.2 { 0 } else { b.3 } })
                .filter(|s| !s.name.is_empty());
        }
        street
    }

    /// The colours of the dark or the light map.
    pub fn palette(&self, light: bool) -> &Palette {
        if light {
            &self.light
        } else {
            &self.dark
        }
    }

    /// `markers.cfg`: `marker = icon.png | width px | x | y | name`, x and y in metres of the
    /// author's grid. A line that cannot be read, or a picture that cannot be decoded, is left
    /// out; the pictures are packed into one sheet for the GPU.
    fn load_markers(&mut self, dir: &Path) {
        let Ok(bytes) = omsi_cfg::vfs::read(&dir.join("markers.cfg")) else {
            return;
        };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let mut files: Vec<String> = Vec::new();
        let mut images: Vec<omsi_texture::Image> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            if line.starts_with('#') || !k.trim().eq_ignore_ascii_case("marker") {
                continue;
            }
            let f: Vec<&str> = v.split('|').map(str::trim).collect();
            if f.len() < 4 || f[0].contains("..") {
                continue;
            }
            let (Ok(width), Ok(x), Ok(y)) = (
                f[1].parse::<f32>(),
                f[2].parse::<f64>(),
                f[3].parse::<f64>(),
            ) else {
                continue;
            };
            if !(width > 0.0 && width < 512.0) || !x.is_finite() || !y.is_finite() {
                continue;
            }
            let icon = match files.iter().position(|n| n.eq_ignore_ascii_case(f[0])) {
                Some(i) => i,
                None => {
                    let path = dir.join(f[0]);
                    let img = omsi_cfg::vfs::read(&path)
                        .ok()
                        .and_then(|b| omsi_texture::decode_bytes(&b, &path).ok());
                    match img {
                        Some(i)
                            if i.width > 0
                                && i.height > 0
                                && i.width <= 1024
                                && i.height <= 1024 =>
                        {
                            files.push(f[0].to_string());
                            images.push(i);
                            images.len() - 1
                        }
                        _ => {
                            log::warn!("map surface: icon {} could not be read", path.display());
                            continue;
                        }
                    }
                }
            };
            let at = self.author_to_world(x, y);
            self.markers.push(Marker {
                at,
                icon,
                width_px: width,
            });
        }
        if images.is_empty() {
            self.markers.clear();
            return;
        }
        let sizes: Vec<(u32, u32)> = images.iter().map(|i| (i.width, i.height)).collect();
        let (w, h, pos) = pack_icons(&sizes);
        self.markers.retain(|m| pos.get(m.icon).is_some_and(Option::is_some));
        if self.markers.is_empty() {
            return;
        }
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let mut rects = Vec::with_capacity(images.len());
        for (i, at) in images.iter().zip(&pos) {
            let (x0, y0) = at.unwrap_or((0, 0));
            if at.is_some() {
                for row in 0..i.height {
                    let from = (row * i.width * 4) as usize;
                    let to = (((y0 + row) * w + x0) * 4) as usize;
                    rgba[to..to + (i.width * 4) as usize]
                        .copy_from_slice(&i.rgba[from..from + (i.width * 4) as usize]);
                }
            }
            rects.push((
                [
                    x0 as f32 / w as f32,
                    y0 as f32 / h as f32,
                    (x0 + i.width) as f32 / w as f32,
                    (y0 + i.height) as f32 / h as f32,
                ],
                i.height as f32 / i.width as f32,
            ));
        }
        self.sheet = Some(IconSheet { w, h, rgba, rects });
    }

    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }

    /// The marker pictures as one image for the GPU, when there are any.
    pub fn sheet(&self) -> Option<&IconSheet> {
        self.sheet.as_ref()
    }

    /// A marker's picture centred at `center` (pixels), `scale` times its width; six vertices
    /// for a drawing with the sheet as texture.
    pub fn push_icon(&self, m: &Marker, center: Vec2, scale: f32, out: &mut Vec<Vertex>) {
        let Some((uv, aspect)) = self.sheet.as_ref().and_then(|s| s.rects.get(m.icon)) else {
            return;
        };
        let (w, h) = (m.width_px * scale, m.width_px * scale * aspect);
        let (x0, y0, x1, y1) = (
            center.x - w * 0.5,
            center.y - h * 0.5,
            center.x + w * 0.5,
            center.y + h * 0.5,
        );
        let v = |x: f32, y: f32, u: f32, t: f32| Vertex {
            pos: [x, y, 0.0],
            uv: [u, t],
            color: [1.0; 4],
            mode: [0.0, 1.0],
            ..Default::default()
        };
        out.extend([
            v(x0, y0, uv[0], uv[1]),
            v(x1, y0, uv[2], uv[1]),
            v(x1, y1, uv[2], uv[3]),
            v(x0, y0, uv[0], uv[1]),
            v(x1, y1, uv[2], uv[3]),
            v(x0, y1, uv[0], uv[3]),
        ]);
    }

    /// The colour of the player's triangle and whether it has a line (a white ring round it),
    /// when the package draws the player so: white without a timetable, one colour on a normal
    /// line and another on a BRT line (the lines the manifest names).
    pub fn player_style(&self, line: Option<&str>) -> Option<(Color, bool)> {
        let p = self.player.as_ref()?;
        match line.map(str::trim).filter(|l| !l.is_empty()) {
            None => Some((p.free, false)),
            Some(l) => Some((
                if p.brt_lines.iter().any(|b| b.eq_ignore_ascii_case(l)) {
                    p.brt
                } else {
                    p.line
                },
                true,
            )),
        }
    }

    fn name(&self, i: u32) -> String {
        self.names.get(i as usize).cloned().unwrap_or_default()
    }

    /// Where a chunk is in game metres (its centre), for choosing the ones near the camera.
    fn chunk_centre(&self, e: &Entry) -> DVec2 {
        self.to_world(
            ((e.cx as f64 + 0.5) * self.chunk) as f32,
            ((e.cy as f64 + 0.5) * self.chunk) as f32,
        )
    }

    /// Every name of the map, once (the file is small: a few hundred).
    pub fn labels(&self) -> Vec<Label> {
        let mut out = Vec::new();
        for e in &self.index {
            let Some(body) = self.data.get(e.at..e.at + e.len) else {
                continue;
            };
            let _ = self.walk(body, |_, _| {}, |_, _| {}, |l| out.push(l));
        }
        out
    }

    /// Walk one chunk: `fill(class, triangle corners as game metres)`, `line(class, width,
    /// points)`, `label`. None on a chunk that ends early (what was read stays used).
    fn walk(
        &self,
        body: &[u8],
        mut fill: impl FnMut(u8, [DVec2; 3]),
        mut line: impl FnMut((u8, f32), Vec<DVec2>),
        mut label: impl FnMut(Label),
    ) -> Option<()> {
        let mut r = Rd { b: body, i: 0 };
        for _ in 0..r.u32()? {
            let class = r.u8()?;
            let n = r.u32()? as usize;
            if n.checked_mul(24)? > r.left() {
                return None;
            }
            for _ in 0..n {
                let mut c = [DVec2::ZERO; 3];
                for p in c.iter_mut() {
                    *p = self.to_world(r.f32()?, r.f32()?);
                }
                fill(class, c);
            }
        }
        for _ in 0..r.u32()? {
            let class = r.u8()?;
            let _flags = r.u8()?;
            let w = r.f32()?;
            let _name = r.u32()?;
            let n = r.u32()? as usize;
            if class == LINE_STREET || class == LINE_STREET_LIMITS {
                // a street to look up, not to draw: its points are not turned into game metres
                r.take(n.checked_mul(8)?)?;
                continue;
            }
            if n.checked_mul(8)? > r.left() {
                return None;
            }
            let mut pts = Vec::with_capacity(n);
            for _ in 0..n {
                pts.push(self.to_world(r.f32()?, r.f32()?));
            }
            line((class, w), pts);
        }
        for _ in 0..r.u32()? {
            let (kind, rank, name) = (r.u8()?, r.u8()?, r.u32()?);
            let at = self.to_world(r.f32()?, r.f32()?);
            label(Label {
                at,
                kind,
                rank,
                name: self.name(name),
            });
        }
        Some(())
    }

    /// The surface as vertices (relative to `anchor`, drawn under the roads): built-up
    /// land, green, water, then rivers, rail and borders. `near` limits it to the chunks
    /// within a radius of a point (the tilted navigator); None builds the whole map.
    pub fn build(&self, p: &mut Painter, anchor: DVec2, near: Option<(DVec2, f64)>, light: bool, contours: f32) {
        let strength = contours;
        let contours = contours > 0.0;
        let rel = |q: DVec2| Vec3::new((q.x - anchor.x) as f32, (q.y - anchor.y) as f32, 0.0);
        let mut tris: [Vec<[DVec2; 3]>; FILL_CLASSES] = Default::default();
        let mut lines: Vec<(u8, f32, Vec<DVec2>)> = Vec::new();
        // a chunk's corners can lie a little over its edge once put onto the game's grid
        let reach = self.chunk * std::f64::consts::SQRT_2 * 0.5 + 60.0;
        for e in &self.index {
            if let Some((c, radius)) = near {
                if (self.chunk_centre(e) - c).length() > radius + reach {
                    continue;
                }
            }
            let Some(body) = self.data.get(e.at..e.at + e.len) else {
                continue;
            };
            let _ = self.walk(
                body,
                |class, t| {
                    if let Some(list) = tris.get_mut(class as usize) {
                        list.push(t);
                    }
                },
                |(class, w), pts| lines.push((class, w, pts)),
                |_| {},
            );
        }
        let pal = self.palette(light);
        // under everything: built-up land, farmland, grass, parks, woods, sports grounds, water
        let order = [
            (FILL_BUILT, pal.built),
            (FILL_FARM, pal.farm),
            (FILL_GRASS, pal.grass),
            (FILL_GREEN, pal.green),
            (FILL_FOREST, pal.forest),
            (FILL_SPORT, pal.sport),
            (FILL_WATER, pal.water),
        ];
        for (class, color) in order {
            // (the terrain view: no built-up land, no green, only water)
            if contours && class != FILL_WATER {
                continue;
            }
            for t in &tris[class as usize] {
                p.world_poly(&[rel(t[0]), rel(t[1]), rel(t[2])], color);
            }
        }
        // rivers under rail under borders; a river's width tells river from stream
        for want in [LINE_RIVER, LINE_RAIL, LINE_BORDER].into_iter().filter(|w| !contours || *w != LINE_BORDER) {
            for (class, w, pts) in lines.iter().filter(|l| l.0 == want) {
                let v: Vec<Vec3> = pts.iter().map(|q| rel(*q)).collect();
                match *class {
                    LINE_RIVER if *w >= 5.0 => p.ribbon(&v, 9.0, 2.6, pal.river, false),
                    LINE_RIVER => p.ribbon(&v, 3.5, 1.5, pal.river, false),
                    // the railway as two lines: a wide edge with the rail's own colour on it
                    LINE_RAIL => {
                        p.ribbon(&v, 4.2, 3.2, pal.rail_edge, false);
                        p.ribbon(&v, 2.2, 1.6, pal.rail, false);
                    }
                    _ => p.ribbon(&v, 1.4, 1.4, pal.border.alpha(0.7), false),
                }
            }
        }
        // the terrain's contour lines (on request), the index lines heavier
        if contours {
            for (class, _, pts) in lines.iter().filter(|l| l.0 == LINE_CONTOUR || l.0 == LINE_CONTOUR_INDEX) {
                let v: Vec<Vec3> = pts.iter().map(|q| rel(*q)).collect();
                if *class == LINE_CONTOUR_INDEX {
                    p.ribbon(&v, 1.9, 1.5, pal.contour.alpha(strength), false);
                } else {
                    p.ribbon(&v, 1.0, 1.0, pal.contour.alpha(0.5 * strength), false);
                }
            }
        }
    }
}

fn point_in_poly(poly: &[DVec2], p: DVec2) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist_to_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let l2 = ab.length_squared();
    let t = if l2 > 1e-9 {
        ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a + ab * t - p).length()
}

/// The contour lines of a chunk near `p` (author-grid metres from the file's origin): per line its nearest distance
/// and its height.
fn contours_in_chunk(body: &[u8], p: DVec2, found: &mut Vec<(f64, f32)>) -> Option<()> {
    let mut r = Rd { b: body, i: 0 };
    for _ in 0..r.u32()? {
        r.u8()?;
        let n = r.u32()? as usize;
        r.take(n.checked_mul(24)?)?;
    }
    for _ in 0..r.u32()? {
        let class = r.u8()?;
        r.u8()?;
        let width = r.f32()?;
        r.u32()?;
        let n = r.u32()? as usize;
        if class != LINE_CONTOUR && class != LINE_CONTOUR_INDEX {
            r.take(n.checked_mul(8)?)?;
            continue;
        }
        let (mut prev, mut best): (Option<DVec2>, f64) = (None, f64::MAX);
        for _ in 0..n {
            let q = DVec2::new(r.f32()? as f64, r.f32()? as f64);
            if let Some(a) = prev {
                best = best.min(dist_to_segment(p, a, q));
            }
            prev = Some(q);
        }
        if best < f64::MAX {
            found.push((best, width));
        }
    }
    Some(())
}

/// Whether a chunk has contour lines (class 5 or 6).
fn chunk_has_contours(body: &[u8]) -> bool {
    let mut r = Rd { b: body, i: 0 };
    let scan = |r: &mut Rd| -> Option<bool> {
        for _ in 0..r.u32()? {
            r.u8()?;
            let n = r.u32()? as usize;
            r.take(n.checked_mul(24)?)?;
        }
        for _ in 0..r.u32()? {
            let class = r.u8()?;
            r.u8()?;
            r.f32()?;
            r.u32()?;
            let n = r.u32()? as usize;
            if class == LINE_CONTOUR || class == LINE_CONTOUR_INDEX {
                return Some(true);
            }
            r.take(n.checked_mul(8)?)?;
        }
        Some(false)
    };
    scan(&mut r).unwrap_or(false)
}

/// Whether a chunk has named streets (class 3 lines).
fn chunk_has_streets(body: &[u8]) -> bool {
    let mut r = Rd { b: body, i: 0 };
    let scan = |r: &mut Rd| -> Option<bool> {
        for _ in 0..r.u32()? {
            r.u8()?;
            let n = r.u32()? as usize;
            r.take(n.checked_mul(24)?)?;
        }
        for _ in 0..r.u32()? {
            let class = r.u8()?;
            r.u8()?;
            r.f32()?;
            r.u32()?;
            let n = r.u32()? as usize;
            if class == LINE_STREET || class == LINE_STREET_LIMITS {
                return Some(true);
            }
            r.take(n.checked_mul(8)?)?;
        }
        Some(false)
    };
    scan(&mut r).unwrap_or(false)
}

/// The nearest street piece of a chunk to `p` (author-grid metres from the file's origin):
/// `best` is (distance, name index, limit, heavy limit), kept when nearer than what it holds.
fn street_in_chunk(body: &[u8], p: DVec2, best: &mut Option<(f64, u32, u32, u32)>) -> Option<()> {
    let mut r = Rd { b: body, i: 0 };
    for _ in 0..r.u32()? {
        r.u8()?;
        let n = r.u32()? as usize;
        r.take(n.checked_mul(24)?)?;
    }
    for _ in 0..r.u32()? {
        let class = r.u8()?;
        let flags = r.u8()?;
        let width = r.f32()?;
        let name = r.u32()?;
        let n = r.u32()? as usize;
        let (speed, heavy) = if class == LINE_STREET_LIMITS { (width.max(0.0).round() as u32, flags as u32) } else { (0, 0) };
        if class != LINE_STREET && class != LINE_STREET_LIMITS {
            r.take(n.checked_mul(8)?)?;
            continue;
        }
        let mut prev: Option<DVec2> = None;
        for _ in 0..n {
            let q = DVec2::new(r.f32()? as f64, r.f32()? as f64);
            if let Some(a) = prev {
                let d = dist_to_segment(p, a, q);
                if best.is_none_or(|b| d < b.0) {
                    *best = Some((d, name, speed, heavy));
                }
            }
            prev = Some(q);
        }
    }
    Some(())
}

/// `regions.cfg` text as regions, positions put onto the game's grid by `to_world`.
fn parse_regions(text: &str, to_world: &dyn Fn(f64, f64) -> DVec2) -> Vec<Region> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if line.starts_with('#') || !k.trim().eq_ignore_ascii_case("region") {
            continue;
        }
        let f: Vec<&str> = v.split('|').map(str::trim).collect();
        if f.len() < 6 || f[0].is_empty() {
            continue;
        }
        let num = |i: usize| f[i].parse::<f64>().ok().filter(|v| v.is_finite());
        let (Some(x), Some(y), Some(w), Some(h)) = (num(1), num(2), num(3), num(4)) else {
            continue;
        };
        let poly: Vec<DVec2> = f[5]
            .split_whitespace()
            .filter_map(|p| {
                let (a, b) = p.split_once(',')?;
                Some(to_world(a.parse().ok()?, b.parse().ok()?))
            })
            .collect();
        if poly.len() < 3 {
            continue;
        }
        let label = to_world(x, y);
        out.push(Region {
            name: f[0].to_string(),
            label,
            box_w: w.max(0.0),
            box_h: h.max(0.0),
            poly,
        });
    }
    out
}

/// `lines.lin` text (tab separated; `ORIGEM x y`, `L number group name`, then per route `R km stops
/// minutes circular days model itinerary`, `P x y x y ...` for the way and `S x y ...` for the
/// stops, positions relative to the origin) as lines; a line with no usable way is left out.
fn parse_lines(text: &str, to_world: &dyn Fn(f64, f64) -> DVec2) -> Vec<LineInfo> {
    let mut origin = DVec2::ZERO;
    let mut out: Vec<LineInfo> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "ORIGEM" if f.len() >= 3 => {
                origin = DVec2::new(
                    f[1].trim().parse().unwrap_or(0.0),
                    f[2].trim().parse().unwrap_or(0.0),
                )
            }
            "L" if f.len() >= 4 => {
                let group = match f[2].trim() {
                    "BRT" => GROUP_BRT,
                    "EXP" => GROUP_EXPRESS,
                    _ => GROUP_FEEDER,
                };
                let date = |k: usize| f.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                out.push(LineInfo {
                    number: f[1].trim().to_string(),
                    group,
                    modal: match f.get(4).map(|m| m.trim()) {
                        Some("tram") => MODAL_TRAM,
                        Some("metro") => MODAL_METRO,
                        Some("rail") => MODAL_RAIL,
                        _ => MODAL_BUS,
                    },
                    from: date(5),
                    to: date(6),
                    name: f[3].trim().to_string(),
                    routes: Vec::new(),
                });
            }
            "R" if f.len() >= 8 => {
                if let Some(l) = out.last_mut() {
                    l.routes.push(RouteInfo {
                        km: f[1].trim().parse().unwrap_or(0.0),
                        stops_n: f[2].trim().parse().unwrap_or(0),
                        minutes: f[3].trim().parse().unwrap_or(0),
                        circular: f[4].trim() == "1",
                        days: f[5].trim().to_string(),
                        model: f[6].trim().to_string(),
                        itinerary: f[7].trim().to_string(),
                        ..Default::default()
                    });
                }
            }
            "P" | "S" if f.len() >= 2 => {
                let nums: Vec<f64> = f[1]
                    .split_whitespace()
                    .filter_map(|v| v.parse().ok())
                    .collect();
                let pts: Vec<DVec2> = nums
                    .chunks_exact(2)
                    .map(|c| to_world(origin.x + c[0], origin.y + c[1]))
                    .collect();
                if let Some(r) = out.last_mut().and_then(|l| l.routes.last_mut()) {
                    if f[0] == "P" {
                        r.pts = pts;
                    } else {
                        r.stops = pts;
                    }
                }
            }
            _ => {}
        }
    }
    out.retain(|l| l.routes.iter().any(|r| r.pts.len() >= 2));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One chunk with a triangle, a river piece and a name, written by hand in the file's layout.
    fn sample() -> Vec<u8> {
        let mut body = Vec::new();
        body.extend(1u32.to_le_bytes());
        body.push(FILL_WATER);
        body.extend(1u32.to_le_bytes());
        for v in [0.0f32, 0.0, 10.0, 0.0, 0.0, 10.0] {
            body.extend(v.to_le_bytes());
        }
        body.extend(1u32.to_le_bytes());
        body.extend([LINE_RIVER, 0]);
        body.extend(3.0f32.to_le_bytes());
        body.extend(u32::MAX.to_le_bytes());
        body.extend(2u32.to_le_bytes());
        for v in [0.0f32, 0.0, 10.0, 10.0] {
            body.extend(v.to_le_bytes());
        }
        body.extend(1u32.to_le_bytes());
        body.extend([0u8, 100]);
        body.extend(0u32.to_le_bytes());
        body.extend(5.0f32.to_le_bytes());
        body.extend(5.0f32.to_le_bytes());
        let mut f = Vec::new();
        f.extend(MAGIC);
        f.extend(VERSION.to_le_bytes());
        f.extend(1000.0f64.to_le_bytes());
        f.extend(2000.0f64.to_le_bytes());
        for v in [0.0f32, 100.0, 0.0, 100.0, 2000.0] {
            f.extend(v.to_le_bytes());
        }
        f.extend(1u32.to_le_bytes());
        f.extend(4u16.to_le_bytes());
        f.extend(b"Test");
        f.extend(1u32.to_le_bytes());
        let at = f.len() + 20;
        f.extend(0i32.to_le_bytes());
        f.extend(0i32.to_le_bytes());
        f.extend((at as u64).to_le_bytes());
        f.extend((body.len() as u32).to_le_bytes());
        f.extend(body);
        f
    }

    fn open(data: Vec<u8>) -> Option<MapSurface> {
        MapSurface::parse(
            data,
            String::new(),
            String::new(),
            Palette::dark(),
            Palette::light(),
            300.0,
        )
    }

    #[test]
    fn a_valid_file_builds_and_names_its_places() {
        let s = open(sample()).expect("valid");
        let mut p = Painter::new();
        s.build(&mut p, DVec2::new(1000.0, 2000.0), None, false, 0.0);
        // one triangle (3 vertices) and a river ribbon
        assert!(p.verts.len() >= 3 + 6);
        let l = s.labels();
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].name.as_str(), l[0].rank), ("Test", 100));
    }

    #[test]
    fn a_chunk_out_of_range_is_left_out() {
        let s = open(sample()).unwrap();
        let mut p = Painter::new();
        s.build(
            &mut p,
            DVec2::ZERO,
            Some((DVec2::new(1.0e7, 1.0e7), 500.0)),
            true,
            0.0,
        );
        assert!(p.verts.is_empty());
    }

    #[test]
    fn broken_files_are_refused_not_panicked_on() {
        let good = sample();
        assert!(open(Vec::new()).is_none());
        assert!(open(b"NOPE".to_vec()).is_none());
        for cut in [3, 10, 30, good.len() - 1] {
            // a truncated file either fails the header/index check or the chunk ends early
            let _ = open(good[..cut].to_vec()).map(|s| s.labels());
        }
        let mut bad = good.clone();
        bad[4] = 9; // another version
        assert!(open(bad).is_none());
    }

    #[test]
    fn the_player_has_a_colour_by_line() {
        let mut s = open(sample()).unwrap();
        assert!(s.player_style(None).is_none());
        s.player = Some(Player {
            free: Color::hex(0xffffff),
            line: Color::hex(0x5588c7),
            brt: Color::hex(0x1e7b34),
            brt_lines: vec!["110".into()],
        });
        assert_eq!(s.player_style(None).map(|p| p.1), Some(false));
        assert_eq!(
            s.player_style(Some(" 110 ")).map(|p| p.0),
            Some(Color::hex(0x1e7b34))
        );
        assert_eq!(
            s.player_style(Some("76")).map(|p| p.0),
            Some(Color::hex(0x5588c7))
        );
    }

    #[test]
    fn icon_sheet_never_exceeds_the_gpu_safe_size() {
        let (w, h, pos) = pack_icons(&[(900, 900); 4]);
        assert_eq!((w, h), (1803, 1803));
        assert_eq!(pos[3], Some((902, 902)));
        let (w, h, pos) = pack_icons(&[(1024, 1024), (1024, 1024)]);
        assert!(w <= 2048 && h <= 2048, "{w}x{h}");
        assert_eq!(pos, vec![Some((1, 1)), None], "the second does not fit and is left out");
        let (w, h, pos) = pack_icons(&[(10, 6), (4, 4)]);
        assert_eq!((w, h), (17, 8));
        assert_eq!(pos, vec![Some((1, 1)), Some((12, 1))]);
    }

    #[test]
    fn an_icon_is_a_quad_inside_its_place_on_the_sheet() {
        let mut s = open(sample()).unwrap();
        s.sheet = Some(IconSheet {
            w: 10,
            h: 6,
            rgba: vec![0; 240],
            rects: vec![([0.1, 0.2, 0.9, 0.8], 0.5)],
        });
        let m = Marker {
            at: DVec2::ZERO,
            icon: 0,
            width_px: 20.0,
        };
        let mut v = Vec::new();
        s.push_icon(&m, Vec2::new(100.0, 50.0), 2.0, &mut v);
        assert_eq!(v.len(), 6);
        // 40 px wide, 20 high, centred
        assert_eq!((v[0].pos[0], v[0].pos[1]), (80.0, 40.0));
        assert_eq!((v[2].pos[0], v[2].pos[1]), (120.0, 60.0));
        assert_eq!(v[0].uv, [0.1, 0.2]);
    }

    #[test]
    fn bus_lines_are_read_with_their_routes() {
        let text = "RLIN\t1\n# c\nORIGEM\t1000\t2000\nL\t110\tBRT\tSenador - Biblia\nR\t18.9\t35\t35\t0\tTodos\tBRT\tA - B\nP\t0 0 10 0 20 5\nS\t0 0 20 5\nL\t999\tALI\tSem rota\n";
        let lines = parse_lines(text, &|x, y| DVec2::new(x, y));
        assert_eq!(lines.len(), 1, "a line with no way is left out");
        let l = &lines[0];
        assert_eq!(
            (l.number.as_str(), l.group, l.routes.len()),
            ("110", GROUP_BRT, 1)
        );
        assert_eq!(l.routes[0].pts[2], DVec2::new(1020.0, 2005.0));
        assert_eq!(
            (
                l.routes[0].stops.len(),
                l.routes[0].minutes,
                l.routes[0].circular
            ),
            (2, 35, false)
        );
    }

    #[test]
    fn lines_carry_a_kind_of_vehicle_and_the_dates_they_run() {
        let text = "RLIN\t1\nORIGEM\t0\t0\nL\t1\tALI\tBus\nR\t1\t2\t3\t0\td\t\tA - B\nP\t0 0 5 5\n\
L\tT1\tALI\tTram\ttram\t0\t0\nR\t1\t2\t3\t0\td\t\tA - B\nP\t0 0 5 5\n\
L\tM1\tALI\tMetro\tmetro\t0\t0\nR\t1\t2\t3\t0\td\t\tA - B\nP\t0 0 5 5\n\
L\tR1\tALI\tTrain\trail\t0\t0\nR\t1\t2\t3\t0\td\t\tA - B\nP\t0 0 5 5\n\
L\t7\tALI\tOld\tbus\t0\t19991231\nR\t1\t2\t3\t0\td\t\tA - B\nP\t0 0 5 5\n\
L\t7\tALI\tNew\tbus\t20000101\t0\nR\t1\t2\t3\t0\td\t\tA - C\nP\t5 5 0 0\n";
        let lines = parse_lines(text, &|x, y| DVec2::new(x, y));
        let kinds: Vec<u8> = lines.iter().map(|l| l.modal).collect();
        assert_eq!(
            kinds,
            [MODAL_BUS, MODAL_TRAM, MODAL_METRO, MODAL_RAIL, MODAL_BUS, MODAL_BUS]
        );
        // a line with no kind or dates is a bus line open at both ends
        assert_eq!((lines[0].from, lines[0].to), (0, 0));
        assert_eq!((lines[4].from, lines[4].to), (0, 19991231));
        assert_eq!((lines[5].from, lines[5].to), (20000101, 0));
    }

    #[test]
    fn regions_say_where_a_point_is() {
        let text = "# c\nregion = Alfa | 5 | 5 | 8 | 2 | 0,0 10,0 10,10 0,10\nregion = Beta | 0 | 0 | 0 | 0 | 20,0 30,0 30,10\nregion = Ruim | 0 | 0 | 0 | 0 | 1,1\n";
        let r = parse_regions(text, &|x, y| DVec2::new(x, y));
        assert_eq!(
            r.len(),
            2,
            "an outline with fewer than three points is left out"
        );
        assert!(point_in_poly(&r[0].poly, DVec2::new(5.0, 5.0)));
        assert!(!point_in_poly(&r[0].poly, DVec2::new(15.0, 5.0)));
        assert_eq!((r[0].box_w, r[1].box_w), (8.0, 0.0));
    }

    #[test]
    fn the_nearest_named_street_of_a_chunk_is_found() {
        // no fills; one street of two points from (0,0) to (100,0), name index 7
        let mut b = Vec::new();
        b.extend(0u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend([LINE_STREET, 2]);
        b.extend(8.0f32.to_le_bytes());
        b.extend(7u32.to_le_bytes());
        b.extend(2u32.to_le_bytes());
        for v in [0.0f32, 0.0, 100.0, 0.0] {
            b.extend(v.to_le_bytes());
        }
        b.extend(0u32.to_le_bytes());
        assert!(chunk_has_streets(&b));
        let mut best = None;
        street_in_chunk(&b, DVec2::new(40.0, 12.0), &mut best).unwrap();
        let (d, name, speed, heavy) = best.unwrap();
        assert!((d - 12.0).abs() < 1e-6 && name == 7 && speed == 0 && heavy == 0);
    }

    #[test]
    fn a_street_with_limits_says_them() {
        // one street of two points, name index 3, limit 50 (the width), 30 for heavy vehicles (the flags)
        let mut b = Vec::new();
        b.extend(0u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend([LINE_STREET_LIMITS, 30]);
        b.extend(50.0f32.to_le_bytes());
        b.extend(3u32.to_le_bytes());
        b.extend(2u32.to_le_bytes());
        for v in [0.0f32, 0.0, 100.0, 0.0] {
            b.extend(v.to_le_bytes());
        }
        b.extend(0u32.to_le_bytes());
        assert!(chunk_has_streets(&b));
        let mut best = None;
        street_in_chunk(&b, DVec2::new(40.0, 5.0), &mut best).unwrap();
        let (d, name, speed, heavy) = best.unwrap();
        assert!((d - 5.0).abs() < 1e-6 && name == 3 && speed == 50 && heavy == 30);
    }

    /// A chunk body with two contour lines (heights 100 and 120, one index) as parallel horizontal lines.
    fn contour_chunk() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(0u32.to_le_bytes());
        b.extend(2u32.to_le_bytes());
        for (class, h, y) in [(LINE_CONTOUR, 100.0f32, 0.0f32), (LINE_CONTOUR_INDEX, 120.0, 100.0)] {
            b.extend([class, 0]);
            b.extend(h.to_le_bytes());
            b.extend(u32::MAX.to_le_bytes());
            b.extend(2u32.to_le_bytes());
            for v in [0.0f32, y, 200.0, y] {
                b.extend(v.to_le_bytes());
            }
        }
        b.extend(0u32.to_le_bytes());
        b
    }

    #[test]
    fn contour_lines_are_found_and_give_a_height_between_them() {
        let b = contour_chunk();
        assert!(chunk_has_contours(&b));
        assert!(!chunk_has_streets(&b));
        let mut found = Vec::new();
        contours_in_chunk(&b, DVec2::new(50.0, 25.0), &mut found).unwrap();
        found.sort_by(|x, y| x.0.total_cmp(&y.0));
        // 25 m from the 100 m line and 75 m from the 120 m one: a quarter of the way up
        assert_eq!(found.len(), 2);
        assert!((found[0].0 - 25.0).abs() < 1e-6 && found[0].1 == 100.0);
        let (d1, h1) = found[0];
        let (d2, h2) = found[1];
        let h = h1 + (h2 - h1) * (d1 / (d1 + d2)) as f32;
        assert!((h - 105.0).abs() < 1e-4);
    }

    #[test]
    fn heights_are_marked_along_a_line_and_not_too_close() {
        // a straight line of 100 m segments, 2 km long
        let pts: Vec<DVec2> = (0..=20).map(|i| DVec2::new(i as f64 * 100.0, 0.0)).collect();
        let idx = height_marks(&pts, 120, true);
        // first once 150 m are run (the segment 100-200 m), then 700 m of line further on (counted from where it was reached)
        assert_eq!(idx.iter().map(|m| m.at.x).collect::<Vec<_>>(), vec![150.0, 850.0, 1550.0]);
        assert!(idx.iter().all(|m| m.index && m.height == 120));
        let plain = height_marks(&pts, 100, false);
        assert_eq!(plain.iter().map(|m| m.at.x).collect::<Vec<_>>(), vec![50.0, 550.0, 1050.0, 1550.0]);
        // close up all of them, far away only the index ones, none nearer than the minimum
        let all: Vec<ContourMark> = plain.iter().chain(&idx).copied().collect();
        let scale = |p: DVec2| Vec2::new(p.x as f32 / 4.0, 0.0); // 4 m a pixel
        let close = marks_to_write(&all, 4.0, 1.0, scale, |_| true);
        assert!(close.iter().any(|(_, m)| !m.index), "close up the plain lines are marked too");
        for (i, (a, _)) in close.iter().enumerate() {
            assert!(close.iter().skip(i + 1).all(|(b, _)| (*a - *b).length() >= 64.0));
        }
        let far = marks_to_write(&all, 20.0, 1.0, |p| Vec2::new(p.x as f32 / 20.0, 0.0), |_| true);
        assert!(!far.is_empty() && far.iter().all(|(_, m)| m.index));
        assert!(marks_to_write(&all, 40.0, 1.0, scale, |_| true).is_empty());
        assert!(marks_to_write(&all, 4.0, 1.0, scale, |_| false).is_empty(), "outside the window");
    }

    #[test]
    fn contour_lines_get_fainter_as_the_view_widens() {
        assert_eq!((contour_strength(1.0), contour_strength(8.0), contour_strength(30.0)), (1.0, 0.5, 0.2));
    }

    #[test]
    fn colours_read_as_hex() {
        assert!(parse_color("#173341").is_some());
        assert!(parse_color("173341").is_none());
        assert!(parse_color("#12").is_none());
    }
}

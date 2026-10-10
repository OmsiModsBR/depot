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
                s.load_markers(&dir);
                s.load_regions(&dir);
                s.load_lines(&dir);
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
        let mut street = None;
        if self.has_streets {
            let a = self.world_to_author(pos) - self.origin;
            let (cx, cy) = (
                (a.x / self.chunk).floor() as i32,
                (a.y / self.chunk).floor() as i32,
            );
            let mut best: Option<(f64, u32)> = None;
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
                .filter(|b| b.0 <= 30.0)
                .map(|b| self.name(b.1))
                .filter(|n| !n.is_empty());
        }
        (region, street)
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
        // side by side, one transparent pixel between them
        let w: u32 = images.iter().map(|i| i.width + 2).sum();
        let h: u32 = images.iter().map(|i| i.height).max().unwrap_or(0) + 2;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let mut rects = Vec::with_capacity(images.len());
        let mut x0 = 1u32;
        for i in &images {
            for row in 0..i.height {
                let from = (row * i.width * 4) as usize;
                let to = (((row + 1) * w + x0) * 4) as usize;
                rgba[to..to + (i.width * 4) as usize]
                    .copy_from_slice(&i.rgba[from..from + (i.width * 4) as usize]);
            }
            rects.push((
                [
                    x0 as f32 / w as f32,
                    1.0 / h as f32,
                    (x0 + i.width) as f32 / w as f32,
                    (1 + i.height) as f32 / h as f32,
                ],
                i.height as f32 / i.width as f32,
            ));
            x0 += i.width + 2;
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
            if class == LINE_STREET {
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
    pub fn build(&self, p: &mut Painter, anchor: DVec2, near: Option<(DVec2, f64)>, light: bool) {
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
            for t in &tris[class as usize] {
                p.world_poly(&[rel(t[0]), rel(t[1]), rel(t[2])], color);
            }
        }
        // rivers under rail under borders; a river's width tells river from stream
        for want in [LINE_RIVER, LINE_RAIL, LINE_BORDER] {
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
            if class == LINE_STREET {
                return Some(true);
            }
            r.take(n.checked_mul(8)?)?;
        }
        Some(false)
    };
    scan(&mut r).unwrap_or(false)
}

/// The nearest street piece of a chunk to `p` (author-grid metres from the file's origin):
/// `best` is (distance, name index), kept when nearer than what it holds.
fn street_in_chunk(body: &[u8], p: DVec2, best: &mut Option<(f64, u32)>) -> Option<()> {
    let mut r = Rd { b: body, i: 0 };
    for _ in 0..r.u32()? {
        r.u8()?;
        let n = r.u32()? as usize;
        r.take(n.checked_mul(24)?)?;
    }
    for _ in 0..r.u32()? {
        let class = r.u8()?;
        r.u8()?;
        r.f32()?;
        let name = r.u32()?;
        let n = r.u32()? as usize;
        if class != LINE_STREET {
            r.take(n.checked_mul(8)?)?;
            continue;
        }
        let mut prev: Option<DVec2> = None;
        for _ in 0..n {
            let q = DVec2::new(r.f32()? as f64, r.f32()? as f64);
            if let Some(a) = prev {
                let d = dist_to_segment(p, a, q);
                if best.is_none_or(|b| d < b.0) {
                    *best = Some((d, name));
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
                out.push(LineInfo {
                    number: f[1].trim().to_string(),
                    group,
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
        s.build(&mut p, DVec2::new(1000.0, 2000.0), None, false);
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
        let (d, name) = best.unwrap();
        assert!((d - 12.0).abs() < 1e-6 && name == 7);
    }

    #[test]
    fn colours_read_as_hex() {
        assert!(parse_color("#173341").is_some());
        assert!(parse_color("173341").is_none());
        assert!(parse_color("#12").is_none());
    }
}

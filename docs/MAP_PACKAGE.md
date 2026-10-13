# Map packages: a pre-built surface, icons, regions and bus lines for the maps

A map author ships a **pre-built** picture of the map next to the map itself. openOMSI does
not compute, convert or triangulate anything: it opens the package, picks the chunks near the
camera and uploads them to the GPU. All the work (OSM, georeferencing, triangulation,
simplification, labels) is done once, by the author's tools.

Why a package and not "openOMSI reads OSM/GeoJSON": loading must cost a file read, never a
geometry pipeline; the game never needs to know where the data came from; any author (real
city, fictional map, other tools) produces the same files.

Fits the existing precedent `radio.cfg`: optional files in the map folder, read only by
openOMSI, ignored by OMSI 2, so a map made for both games loses nothing.

## 1. Fixed location and layout

One folder, with a fixed name and fixed file names, inside the map's own folder under
`Maps`. openOMSI looks only there; nothing is searched, nothing is configured.

```
<OMSI 2>/Maps/<Map>/
  global.cfg
  openomsi-realmap/                 <- the package (fixed name, lower case)
    manifest.cfg            required: its presence makes the package count
    surface.oms             the pre-built map surface and the named streets (section 4)
    markers.cfg             optional: icons on the map (section 8.1)
    regions.cfg             optional: municipalities, their names as a watermark, where the player is (8.3)
    lines.lin               optional: the bus lines, for the lines panel (8.4)
    icons/                  optional: the pictures markers.cfg names
    LICENSE.txt             optional: the licence of the data, for people (the game does not read it)
```

Rules:
- The folder is `openomsi-realmap` and the files carry the names above. A file that is missing is the
  feature that is off, never an error. No `manifest.cfg`: the package is ignored.
- The folder may be in the map's own folder or in the same place of another content root: a package for a map
  one does not own goes in the game's content folder (`Maps/<Map>/openomsi-realmap/`), as a mod does, so the OMSI 2
  installation is never written to. The root with the most priority wins.
- The package is read only by openOMSI. OMSI 2 ignores the folder, so a map made for both
  games loses nothing and the folder can ship inside the map's own download.
- Paths inside `manifest.cfg` are relative to the `openomsi-realmap` folder.
- Same precedent as `radio.cfg` (beside `global.cfg`), grouped in one folder because the
  package has several files.

## 2. Manifest `openomsi-realmap/manifest.cfg`

Same text style as `radio.cfg` (`key = value`, `#` comments, UTF-8).

```
format   = 1
name     = My Map
grid     = 300
credit   = (c) OpenStreetMap contributors (ODbL)
# optional look; any key left out takes the openOMSI theme
water    = #7fb2e5
green    = #9ccf8f
built    = #2a2a2a
```

- `format`: integer. A reader that does not know it ignores the package (never an error).
- `grid`: the author's tile size in metres (section 3). Required.
- The data files have fixed names (section 1), so the manifest does not name them.
- `credit`: shown on the city map and in the launcher. Required when the data needs attribution.
- `terrain_offset`: metres added to the height of every contour line when the game says it (default 0). The
  contour lines of a package are heights **above sea level** (section 8.3, "Terrain view"); this key is only for a
  package whose lines are in the editor's own heights.
- Colours are optional hints. The game has a **dark** and a **light** look; a colour key colours
  the dark map (`water = #rrggbb`) and its `light_` twin the light one (`light_water = #rrggbb`).
  Keys: `land water river green grass forest sport farm built rail rail_edge border contour`. A key left
  out takes the game's own colour for that look. Layers are **semantic**, so the game decides
  the final colour.

## 3. Coordinates

Positions are **metres of the author's tile grid**, the same numbers OMSI 2 shows for the map's
objects: `x = tile_column * grid + position_in_tile`, `y = tile_row * grid + position_in_tile`,
X east, Y north. The manifest's `grid` is the author's tile size in metres (300 for a plain map;
whatever the author measured for a map laid out otherwise, e.g. 585.795).

openOMSI turns each position into its own game metres with the grid it uses for the map
(`omsi_map::tile_local_to_world`): it splits the position into a tile and a place in the tile
by `grid`, then places that on its grid. This is why the package states `grid` instead of
openOMSI's absolute metres: a `[worldcoordinates]` map's grid is not 300 m and differs from
what the author measured, so absolute metres would be kilometres off far from the origin.
No latitude or longitude is stored; a map may be bent, shortened or fictional. The file stores
positions as `f32` offsets from `origin` (an `f64` pair) so large maps keep precision.

## 4. Binary file `surface.oms`

```
magic    "OMSF"
u32      version (= 1)
f64 x2   origin_x, origin_y         (game metres)
f32 x4   min_x, max_x, min_y, max_y (relative to origin)
f32      chunk_size                 (metres, e.g. 2000)
u32      n_strings ; n_strings x { u16 len, utf-8 bytes }     string table (names)
u32      n_chunks
n_chunks x chunk index { i32 cx, i32 cy, u64 offset, u32 byte_len }   (sorted by cx, cy)
... chunk payloads ...
```

Chunk `(cx, cy)` covers `[cx*chunk_size, (cx+1)*chunk_size)` on each axis, relative to
`origin`. A reader finds the chunks near the camera by index lookup and reads only those.
Whole-map layers (borders, labels) are repeated per chunk they touch, so no chunk needs
another one.

Chunk payload:

```
u32  n_fills   ; n_fills x  { u8 class, u32 n_tri, f32[6*n_tri] }          triangles, ready to draw
u32  n_lines   ; n_lines x  { u8 class, u8 flags, f32 width_m, u32 name,
                              u32 n_pts, f32[2*n_pts] }                     polylines
u32  n_labels  ; n_labels x { u8 kind, u8 rank, u32 name, f32 x, f32 y }    names and markers
```

- `class` for fills: `0 water, 1 green (parks, gardens), 2 built, 3 grass (grass, meadow, scrub), 4 forest (wood, forest), 5 sport (pitches, stadiums), 6 farm`.
- `class` for lines:

  | class | what | `width_m` | `flags` |
  |---|---|---|---|
  | 0 | river | 3 stream, 6 or more river | 0 |
  | 1 | rail | | 0 |
  | 2 | border | | 0 |
  | 3 | street | | its rank 0-5 |
  | 4 | street with limits | its speed limit, km/h (0 = none) | the limit of heavy vehicles, km/h (0 = the same, or none) |
  | 5 | contour line | its height above sea level, metres | 0 |
  | 6 | index contour line (every fifth), same | its height | 0 |

  A **street** (class 3 or 4) is not drawn: its points are only looked up to tell the player which street
  they are on and the street under the pointer (the nearest piece within 30 m, or within a reach that grows
  with the zoom for the pointer; `name` is its name). Other classes are skipped by a reader that does not know them.
  Roads are optional here: the game keeps drawing its own lanes from the tiles, so a package
  may leave road classes out. If present, they are decoration under the game's own roads.
- `flags` for lines: bit0 = bridge, bit1 = tunnel (draw dashed/under).
- `kind` for labels: `0 district, 1 terminal, 2 town, 3 landmark, 4 water`.
- `rank` 0-255: label importance (high = shown at far zoom). Authors decide; the game never
  derives it.
- Triangles are already triangulated and wound counter-clockwise; no holes to resolve.
- `name` is an index into the string table (`u32::MAX` = none).

## 5. What the reader does

1. At map load, in the same background step that reads the tiles: parse `manifest.cfg`,
   validate magic/version/index, keep the index in memory. Any failure logs one line and the
   map shows without a surface (today's look).
2. Each frame the map view needs a new area (the existing anchor/radius rule), it reads the
   chunks in range and uploads their vertices to one buffer drawn **under** the roads.
3. Nothing else: no geometry processing, no network access.

## 6. Versioning and room to grow

- Unknown `class`/`kind` values are skipped, so new classes are not a breaking change.
- New data goes in new sections with a bumped `version`; readers ignore versions they do not
  know. What the package may carry besides the surface is in section 8.

## 7. Making a package

openOMSI only reads. The files are made beforehand by the map author's own tools from whatever the
author has (OpenStreetMap, a GIS, a hand-made map): the format asks for nothing but the files of
section 1, so any source and any tool will do.

The upstream example packages (derived from OpenStreetMap data under the ODbL, one of them with line data from a third-party map author) are deliberately not included in Depot: their data licence and provenance have not been reviewed here. See the changelog entry for the origin of this feature.

## 8. What the package carries besides the surface

openOMSI already shows the trip's route, the next stops with times, the distance to the next
stop and early/late, so the package carries **no line or stop-sequence data**. Beside the
surface it may carry, all optional:

### 8.1 Icons on the map (`markers.cfg` and `icons/`)

Text file, `#` comments, one marker per line:

```
# icon | width in pixels at interface size 1 | x | y | name
marker = icons/terminal.png | 30 | 1200.5 | 840.0 | Central Terminal
marker = icons/depot.png | 54 | 1650.0 | 410.5 | Bus depot
```

- `icon`: a PNG (straight alpha, up to 1024 x 1024) under the package folder, no `..` in the
  path. Pictures are packed into one texture; a picture that cannot be read leaves out its lines.
- `x`, `y`: metres of the author's grid (section 3). `name` is for people reading the file.
- Drawn upright at their place on the navigator and the city map, always visible, no text.
- A map without the file shows none.

### 8.2 The player's marker (manifest keys)

```
player      = triangle
player_free = #ffffff     # no timetable
player_line = #5588c7     # on a normal line
player_brt  = #1e7b34     # on a line named in brt_lines
brt_lines   = 110, 111, 120
```

`player = triangle` replaces the white arrow by a triangle (tip ahead, short base behind): a
dark edge, a white ring while the player is on a line, and the colour of the kind of line
inside. Without the keys, the game's own arrow stays.

### 8.3 Regions (`regions.cfg`)

```
# region = name | label x | label y | label box width m | label box height m | polygon x,y x,y ...
region = Alphaville | 5200.0 | 3100.0 | 4800.0 | 520.0 | 1000.0,1000.0 9000.0,1000.0 9000.0,6000.0 1000.0,6000.0
```

Metres of the author's grid. The polygon says which municipality the player is in; the box is where
and how big the name fits, **computed by the author's tools** (the largest letters whose box fits
inside the municipality's visible part, one size cap for all). A box width of 0: no readable
room, no name. The city map draws each name as a big, faint watermark inside its box, only while
zoomed out (it gives way to street names when zoomed in). With the regions and the named streets
the game shows a **"municipality · street"** row under the next stop on the small map, and the
same two names in a chip on the city map. Without them, none of it appears.

The city map also shows the street under the pointer (any street the package names, also where the game has
no road) in a small box, with its limits when the package says them (class 4 of the street lines, section 4).
The limit of the street the player is on is shown as a **road sign** beside the "you are on" chip (and, smaller,
the limit of heavy vehicles when it is another). A generator takes the limits from `maxspeed` and `maxspeed:hgv`
of the real streets where they exist; a street without a limit is class 3 and shows none.

**Terrain view.** Classes 5 and 6 of the lines (section 4) are the contour lines of the real terrain, heights
above sea level. A package with them gets a **Terrain** button on the city map (and a setting, `nav_contours`).
In that view the map shows only what is of the terrain and of the game: water, rivers, the rails, the game's own
roads and the contour lines, with none of the built-up blocks, green areas or borders behind. The lines are faint
from afar, so that the streets stand out, and stronger up close; up close the height is written on the index
lines, in a faint colour. With the terrain view on, the height of the street the player is on and of the street
under the pointer are written beside their names (interpolated between the two nearest contour lines of
different heights). The generators take the heights from public elevation tiles for the area of the map.

### 8.4 Bus lines (`lines.lin`)

Tab separated, one record per line, `#` comments. Positions are relative to `ORIGEM` (metres of the
author's grid).

```
RLIN	1
ORIGEM	0.0	0.0
L	110	BRT	North - Central          line: number, kind (BRT, EXP, ALI), name
R	18.9	35	35	0	Every day	Articulated	North Terminal - Central Terminal     route: km, stops, minutes, circular 0/1, days, model, itinerary
P	1250.4 4428.6 1320.0 4330.8 ...                                       the way
S	1260.8 4409.6 1310.0 4341.5 ...                                       the stops
```

A line has one or two routes (`R`, `P`, `S` repeat). With the file, the city map gets a **Lines**
button: a panel with the data of the line in focus (length, stops, trip time, days, model, route 1
or 2), the list of the map's lines with filters (All, BRT, Express, Feeders) and a tick box each.
Ticked lines are drawn on the map, each in its own colour, with their stops; the vehicles running
a ticked line (read from the line they show) take its colour and a ring, and the card counts them.
The game's own route of the active trip is unchanged. Without the file, no button.

### 8.5 What is the map's own

Everything in the package is the map author's: its surface, icons, regions, lines and colours. The code in
openOMSI is generic and carries none of it.

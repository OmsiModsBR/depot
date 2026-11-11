//! The types of the scene built from a map: the scenery object and spline types, the
//! tiles as they are staged, placed and uploaded, the GPU caches, the world itself, the
//! light and night-map objects, the vehicles' render sets and the navigation map. Their
//! procedures are in `implementation/scene.rs`.

use crate::world::tiles::{MapIndex, Pose};
use glam::{DVec3, Mat4};
use hashbrown::HashMap;
use omsi_geometry::{MeshData, TileSurface};
use omsi_map::{GlobalCfg, Terrain};
use omsi_model::{MaterialDef, Model};
use omsi_render::{AlphaMode, MaterialExtra, MaterialId, MeshId, TextureId};
use omsi_scenery::{SceneryObject, Spline};
use omsi_sim::traffic::{Lane, TrafficLightController};
use omsi_texture::{Image, TextureCache, TextureData};
use parking_lot::{Mutex, RwLock};
use std::path::PathBuf;
use std::sync::Arc;

/// A loaded scenery object type: model meshes + material descriptions.
pub struct ObjectType {
    pub sco: SceneryObject,
    /// The `[sound]` config's file, found the first time it is needed.
    pub sound_path: std::sync::OnceLock<Option<PathBuf>>,
    pub model: Model,
    pub model_dir: PathBuf,
    /// LOD 0 meshes (mesh data, o3d materials, model material overrides).
    pub meshes: Vec<(MeshData, Vec<omsi_o3d::Material>, Vec<MaterialDef>)>,
    /// `[visible] var value` per mesh, parallel to `meshes`.
    pub mesh_visible: Vec<Option<(String, f32)>>,
    /// Model mesh definition index and pivot per loaded mesh (parallel to `meshes`).
    pub mesh_def_index: Vec<usize>,
    pub mesh_pivots: Vec<Mat4>,
    /// `[isshadow]` per loaded mesh: a flat shadow blob drawn on the ground.
    pub mesh_shadow: Vec<bool>,
    /// `[shadow]` per loaded mesh: the meshes OMSI casts (stencil) shadows from.
    pub mesh_casts: Vec<bool>,
    /// Compiled scripts when the object is scripted or animated.
    pub program: Option<Arc<omsi_script::Program>>,
    /// Further `[LOD]` levels: (min screen size, meshes), in model order after LOD 0.
    pub lower_lods: Vec<(
        f32,
        Vec<(MeshData, Vec<omsi_o3d::Material>, Vec<MaterialDef>)>,
    )>,
    /// Min screen size of LOD 0 (0 = always).
    pub lod0_min: f32,
    /// Number of `[CTC]` paint schemes the object offers.
    pub paint_scheme_count: usize,
    /// Runtime scenery texture groups (`[CTC]` and `[texchanges]`), each selected by its own
    /// script variable and supplying one or more material texture replacements per choice.
    pub dynamic_textures: Vec<DynamicTextureGroup>,
    /// `[terrainhole]` meshes of the model: where they lie the ground is taken away.
    pub holes: Vec<MeshData>,
    /// `[crossing_heightdeformation]`: the mesh a crossing presses the terrain into, so
    /// the junction plate and the roads that meet it sit on one surface.
    pub deform: Option<MeshData>,
    /// `[collision_mesh]`: what vehicles actually hit (often much plainer than the model).
    pub collision: Option<MeshData>,
    /// A plain object that is only paint at its foot ([`paint_at_foot`]).
    pub paint: bool,
    /// What of the type stops the outside camera (decided on first use).
    pub camera: std::sync::OnceLock<crate::output::BlockerShape>,
    /// The collision mesh as the vehicles meet it (built on first use).
    pub collision_shape: std::sync::OnceLock<Arc<omsi_sim::collision::MeshShape>>,
}

/// One scenery texture selector and its indexed replacement sets.
#[derive(Clone)]
pub struct DynamicTextureGroup {
    pub variable: String,
    /// Each replacement is (the material's default texture key, replacement file, folder).
    pub choices: Vec<Vec<(String, String, PathBuf)>>,
}

/// Where the glass that shows a mirror's picture is and how the picture lies on it, in the
/// bus's frame (x right, y forward, z up).
#[derive(Clone, Copy, Debug)]
pub struct MirrorGlass {
    pub centre: glam::Vec3,
    /// How the position moves with the texture's u and v.
    pub du: glam::Vec3,
    pub dv: glam::Vec3,
    /// How much of the picture the mesh uses.
    pub uv_area: f32,
}

/// A placed scenery object with a running script / animations.
pub struct ScriptedObject {
    pub ty: Arc<ObjectType>,
    pub pos: DVec3,
    pub xf: Mat4,
    /// Render instance per loaded mesh.
    pub instances: Vec<usize>,
    pub inst: omsi_sim::scenery::SceneryInstance,
    /// Traffic light program of this object (crossings) or of its parent (lamps).
    pub controller: Option<usize>,
    pub light_index: usize,
    /// A child of a crossing that names one of its lights without being one of our lamps
    /// (its own textures to choose, see `child_lamp`): (the crossing, the light). Its
    /// script reads that light's `TrafficLightPhase` and `TrafficLightApproach`, as
    /// OMSI hands them to any such child. The
    /// crossing's program is looked up when the script runs: it may stand on a tile
    /// loaded later.
    pub light_parent: Option<(i64, usize)>,
    pub map_id: i64,
    /// `[matl_change]` variants: (instance, slot, base, item, variable).
    pub variants: Vec<(usize, usize, MaterialId, MaterialId, String)>,
    /// `[sound]` config of the object, loaded when the listener comes near.
    pub sounds: Option<omsi_audio::SoundSet>,
    /// The tile it belongs to (it goes when the tile is unloaded).
    pub tile: (i32, i32),
    /// `[varparent]`: the object whose data this one shows (a departure display's stop).
    pub var_parent: Option<i64>,
    /// `[texttexture]`s drawn from the script's string variables: (texture, state).
    pub texts: Vec<(TextureId, omsi_sim::texttex::TextTextureState)>,
    /// The script asks for the buses due at its stop (`GetArrBus*`).
    pub arrivals: bool,
    /// `[htmltexture]` pages shown on the object: (script texture index, texture). The
    /// pages themselves are `inst.html_textures`.
    pub htmls: Vec<(usize, TextureId)>,
    /// Per instance, its slots faded by `[alphascale]` variables (`LampSlots::alpha`; empty
    /// when none is), and the alphas last given.
    pub alpha_slots: Vec<LampSlots>,
    pub alpha_last: Vec<Vec<f32>>,
}

/// Where a ray lands on a page (`[htmltexture]`) of a scenery object: see
/// [`World::html_object_hit`].
#[derive(Clone, Copy, Debug)]
pub struct PageHit {
    /// Distance (m) along the ray.
    pub t: f32,
    pub map_id: i64,
    /// The page's script texture index.
    pub page: usize,
    /// 0..1 across the page, `v` down from the top.
    pub u: f32,
    pub v: f32,
}

/// What the timetable tells the scenery: the time of day, and the buses due at the stops
/// whose departure displays are near (see [`World::timetable_boards`]).
#[derive(Default)]
pub struct StopBoards {
    /// The simulation clock the boards were made at (None before a timetable or clock ran:
    /// the scenery scripts then keep their own).
    pub clock: Option<omsi_sim::SimClock>,
    /// Per bus stop (map object id): the buses due, soonest first, as (line, terminus,
    /// expected arrival in seconds of the day).
    pub by_stop: HashMap<i64, Vec<(String, String, f64)>>,
    /// The stops whose displays asked in the last scenery update.
    pub wanted: Vec<i64>,
    /// The stop names the HTML pages asked departures for (`omsi.getDepartures`): trimmed,
    /// lower case.
    pub wanted_names: Vec<String>,
    /// Per stop name of `wanted_names`: the departures of the next two hours, soonest first,
    /// at most 20, as (line, destination, timestamp).
    pub departures: std::collections::HashMap<String, Vec<(String, String, f64)>>,
    /// Counts up whenever `departures` was made anew.
    pub departures_gen: u64,
}

/// The material slots of a lamp's mesh switched by its variables: `[alphascale] var`
/// fades a slot (a lens shown by its alpha rather than by a `[visible]` mesh, as the
/// Korean maps' signals do, #826) and `[matl_lightmap] tex var` lights it.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct LampSlots {
    pub count: usize,
    pub alpha: Vec<(usize, String)>,
    pub light: Vec<(usize, String)>,
}

/// A placed `[trafficlight]` object: its render instances follow the light state of
/// light `index` of the crossing `parent`.
#[derive(Clone)]
pub struct LightObject {
    pub parent: i64,
    pub index: usize,
    /// The map names no light for it (no string, or an empty one): a gate that is its own
    /// crossing, such as the Spandau depot's barrier (`Omnibushof_S_1`/`_S_2`, one arm for
    /// the way in and one for the way out, both moved by one script from one
    /// `TrafficLightPhase`). It is shown the most open state of its crossing's lights, so
    /// that the arms rise for whoever is let through, coming in or going out; on light 0
    /// alone the exit arm stayed down while the buses drove out through it.
    pub any_light: bool,
    /// (render instance, `[visible]` condition of that mesh)
    pub instances: Vec<(usize, Option<(String, f32)>)>,
    /// Per instance (parallel to `instances`) the material slots its lamp variables switch
    /// besides `[visible]`: see [`LampSlots`].
    pub slots: Vec<LampSlots>,
    /// Material switches kept with the lamp, updated alongside its visibility.
    pub variants: Vec<(usize, usize, MaterialId, MaterialId, String)>,
    pub pos: DVec3,
    /// The lamp's own script (`ampel1.osc` & co): it turns `TrafficLightPhase` into the
    /// `Red`/`Yellow`/`Green`/`Left`/`Light` variables its meshes and coronas show.
    /// Shared by the copies of the lamp list (the loaded tiles' lists are copied into
    /// `World::light_objects` whenever a tile comes or goes), so that it keeps its state.
    pub script: Option<Arc<Mutex<omsi_sim::scenery::SceneryInstance>>>,
    /// `[light_enh_2]` coronas switched by a lamp variable, and that variable.
    pub coronas: Vec<(omsi_render::Corona, String)>,
    /// Per corona the mesh its light belongs to and the light's place and direction in the
    /// model: an animated lamp's lights move with their mesh (see `model_light_sources`).
    pub corona_mesh: Vec<(usize, glam::Vec3, glam::Vec3)>,
    /// Current brightness of each corona (set with the lamp state every frame).
    pub lit: Vec<f32>,
    /// The object's rotation, and whether its script moves meshes of it: a level
    /// crossing's barrier is a `[trafficlight]` object whose arm turns with its light
    /// (`bue_schranke.osc`), and stood as a static model across the road.
    pub xf: Mat4,
    pub animated: bool,
    /// `[sound]` of the lamp (a crossing's bell, `bue_anlage1`), resolved, and its sounds
    /// once the listener is near (shared by the copies of the list, like `script`).
    pub sound: Option<PathBuf>,
    pub sounds: Arc<Mutex<Option<omsi_audio::SoundSet>>>,
    pub shown: Option<u64>,
    pub texts: Vec<(TextureId, omsi_sim::texttex::TextTextureState)>,
}

pub struct SplineType {
    pub def: Spline,
    pub dir: PathBuf,
    /// The `.surf` map of each of `def.textures` (see [`surf_map`]).
    pub surf: Vec<Option<Arc<omsi_geometry::HeightMap>>>,
}

#[derive(Debug, Default)]
pub struct LoadStats {
    pub tiles: usize,
    pub objects: usize,
    pub trees: usize,
    pub splines: usize,
    pub object_types: usize,
    pub spline_types: usize,
    pub textures: usize,
    /// Object records whose type is not in this installation (logged once per file).
    pub failed_objects: usize,
    /// Parking spaces the map leaves empty on purpose (a quarter of them).
    pub empty_spaces: usize,
    /// `[splineAttachement]` rows (and repeaters) that put objects on a spline.
    pub rows: usize,
    /// `[attachObj]` records, and those whose parent or attachment point is missing.
    pub attached: usize,
    pub unattached: usize,
    /// Objects stood on the ground (trees and invisible helpers included).
    pub objects_placed: usize,
    /// Ground points pulled onto `[spline_terrain_align]` roads, on how many tiles, and the
    /// biggest move (metres, where).
    pub ground_aligned: usize,
    pub ground_aligned_tiles: usize,
    pub ground_moved_most: Option<(f32, f64, f64)>,
    /// Tiles a crossing's `[crossing_heightdeformation]` changed, and crossings warped.
    pub ground_deformed_tiles: usize,
    pub crossings_warped: usize,
}

/// Where a staged object will stand once the ground under it is final.
#[derive(Clone)]
pub enum Placement {
    /// An `[object]` record: world x, y, the height above the terrain and its rotation.
    Ground {
        x: f64,
        y: f64,
        z: f64,
        rot: [f64; 3],
    },
    /// A pose known from the start: `[absheight]` objects, objects joined to splines by
    /// `[splinehelper]` (crossings, switches) and spline attachment rows.
    Pose(Pose),
    /// `[attachObj]`: attachment point `index` of object `parent`, turned by `rot`.
    Attached {
        parent: i64,
        index: usize,
        rot: [f64; 3],
    },
}

/// An object of a tile with its type, before it stands on the final ground.
pub struct StagedObject {
    pub ot: Arc<ObjectType>,
    pub id: i64,
    pub place: Placement,
    pub rules: Vec<omsi_map::MapRule>,
    /// The record's trailing lines: text strings, tree parameters, a lamp's light index,
    /// a bus stop's name.
    pub extra: Vec<String>,
    /// The crossing a traffic light lamp belongs to (`[varparent]`, else what it hangs on).
    pub lamp_parent: Option<i64>,
    /// A car put on a parking space (the traffic steers round it).
    pub parked: bool,
    /// The record is an `[object]` (its position goes into `object_positions`).
    pub map_object: bool,
    /// The instance of a spline-attachment row this object represents.
    pub instance: usize,
    /// What the collisions call this object: its map id, or for an object of a spline
    /// attachment row (which all share the row's id) a key of its own, so that one post of
    /// a row of `[crashmode_pole]` bollards falls alone.
    pub key: i64,
}

/// A spline of a staged tile as the rasters see it.
pub struct StagedSpline {
    /// Positions (relative to the tile origin) and triangles only.
    pub(crate) shape: MeshData,
    pub(crate) ty: Arc<SplineType>,
    /// World bounds (x0, y0, x1, y1).
    pub(crate) bounds: [f64; 4],
    /// It carries a road or a footway (a railway embankment or a bridge deck does not).
    pub(crate) drivable: bool,
    /// Every profile of it is blended (`[matl_alpha] 2`): a layer laid over the ground or a
    /// road, not a surface of its own (see `prepare_surfaces`).
    pub(crate) overlay: bool,
    /// It is ground (see `SPLINE_OVERHEAD`): it goes into the surface raster, cutting the
    /// terrain where that comes up through it. Overhead wires do not.
    pub(crate) cuts_terrain: bool,
    /// It stands clear of the ground all along (see `SPLINE_SHADOW_CLEARANCE`): it casts a
    /// sun shadow.
    pub(crate) casts_shadow: bool,
    /// Start point used by OMSI's far-to-near blend sort for spline surfaces.
    pub(crate) sort_origin: DVec3,
}

/// What stands on one parking space: nothing, or a car from the list with a paint scheme.
#[derive(Debug, PartialEq, Eq)]
pub struct ParkedPick {
    pub car: usize,
    pub scheme: usize,
}

/// A tile read and tessellated, its objects typed but not yet standing on the ground. Kept
/// (by [`World::prepare_tiles`]) while a loaded tile or one on its way depends on it.
pub struct StagedTile {
    pub tx: i32,
    pub ty: i32,
    pub origin: DVec3,
    pub path: PathBuf,
    /// The terrain as the tile file has it, before roads and crossings pulled it about.
    pub base_terrain: Terrain,
    /// `[spline_terrain_align]` splines: (index into `splines`, reach in metres).
    pub align: Vec<(usize, f32)>,
    /// The hole boundaries in world space, including the profile's authored height.
    pub hole_rims: Vec<Vec<DVec3>>,
    pub water: Option<[f32; 4]>,
    /// `[variable_terrainlightmap]`: the tile's light map is baked from the lamps around it
    /// (see [`bake_light_map`]), not read from its `.map.LM.bmp`.
    pub bakes_light_map: bool,
    pub splines: Vec<StagedSpline>,
    /// The whole spline meshes, in the order of `splines`, until the tile is placed.
    pub meshes: Mutex<Option<Vec<Arc<MeshData>>>>,
    /// The `[heightprofile]` surfaces of the tile's splines (local to `origin`) with their
    /// world bounds and `.surf` maps: what the wheels roll on.
    pub drive: Vec<(MeshData, [f64; 4], Option<omsi_geometry::SurfFaces>)>,
    /// Lanes, taken when the tile is loaded for the first time.
    pub lanes: Mutex<Vec<Lane>>,
    /// The street lanes' points of the tile's splines, kept for good (what an object's box
    /// is checked against: a road through it makes it no wall).
    pub street_points: Vec<DVec3>,
    pub objects: Vec<StagedObject>,
    /// The spline attachment rows `[attachObj]` records can hang on: (row id, where its first
    /// object stands, the row's own type - a car park row's, not its car's).
    pub anchors: Vec<(i64, Pose, Arc<ObjectType>)>,
    /// What reading the tile counted (missing types, empty car parks, rows, attachments).
    pub counts: LoadStats,
    pub resolved: std::sync::OnceLock<Arc<Resolved>>,
    /// The tile's overhead wire (None: it has none).
    pub wires: Option<omsi_trolley::TileWires>,
}

/// A staged tile's final ground and where its objects finally stand.
pub struct Resolved {
    pub(crate) terrain: Arc<Terrain>,
    /// Crossings warped onto the ground: object index → its own meshes.
    pub(crate) warped: HashMap<usize, Arc<Vec<MeshData>>>,
    /// By object index; None for an attachment without its parent or attachment point.
    pub(crate) poses: Vec<Option<Pose>>,
    pub(crate) unattached: usize,
    pub(crate) aligned_points: usize,
    pub(crate) biggest: Option<(f32, f64, f64)>,
    pub(crate) deformed: bool,
}

/// The tiles of a map and what each depends on.
pub struct TileLayout {
    pub paths: HashMap<(i32, i32), PathBuf>,
    /// Tile → the tiles whose splines, crossings or ground its own final ground, crossings
    /// and cut are made from: its neighbours, and tiles with splines reaching it from
    /// further away. The same for a whole-map load as for streaming.
    pub(crate) sources: HashMap<(i32, i32), Vec<(i32, i32)>>,
}

/// A placed scenery object, ready for the GPU.
pub struct PlacedObject {
    pub ot: Arc<ObjectType>,
    pub pos: DVec3,
    pub xf: Mat4,
    /// Traffic light lamp: (crossing id, light index).
    pub lamp: Option<(i64, usize, bool)>,
    pub map_id: i64,
    /// Collision key (see [`StagedObject::key`]).
    pub key: i64,
    pub controller: Option<usize>,
    pub strings: Vec<String>,
    pub warped: Option<Arc<Vec<MeshData>>>,
    /// `[varparent]` of the record.
    pub var_parent: Option<i64>,
    /// A car on a `[carpark_p]` space.
    pub parked: bool,
    /// A tile's own `[object]` record standing on the ground: the object editor may move it.
    pub editable: bool,
    pub script: Option<omsi_sim::scenery::SceneryInstance>,
}

/// A scenery object the object editor can take hold of (see [`World::edit_objects`]).
#[derive(Clone)]
pub struct EditObject {
    pub tile: (i32, i32),
    pub pos: DVec3,
    pub xf: Mat4,
    /// Collision key (its boxes carry it).
    pub key: i64,
    pub instances: Vec<usize>,
    /// Its `.sco`, for the editor's display.
    pub sco: std::path::PathBuf,
}

/// What the object editor did to one object: moved by `moved` (m), turned by `turned`
/// (degrees clockwise, as a map object's heading), or taken away.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ObjectEdit {
    pub moved: DVec3,
    pub turned: f64,
    pub deleted: bool,
}

/// A parked car of a loaded tile (see [`World::parked_objects`]).
#[derive(Clone)]
pub struct ParkedObject {
    pub tile: (i32, i32),
    pub pos: DVec3,
    pub heading: f64,
    /// Its `.sco` (the AI car of the same folder is what drives off in its place).
    pub sco: std::path::PathBuf,
    /// Its instances (every LOD).
    pub instances: Vec<usize>,
}

/// A tile ready for upload: everything computed, nothing on the GPU yet.
pub struct Prepared {
    pub tx: i32,
    pub ty: i32,
    pub terrain: Option<MeshData>,
    /// The terrain's exposed sides, drawn separately so the hole mask cannot cut them.
    pub hole_walls: MeshData,
    /// Ground painting: for every `[groundtex]` layer above the first that is painted on
    /// this tile, its index and the alpha mask the editor's brush left behind (as read;
    /// [`World::cut_terrain`] turns them into `paint`).
    pub paint_masks: Vec<(usize, Image)>,
    /// The painted layers ready for the GPU: index, mask (with the roads' cut taken out)
    /// and the painted fraction of the tile.
    pub paint: Vec<(usize, TextureData, f32)>,
    /// The same brush masks without the hole cut, for the exposed terrain sides.
    pub wall_paint: Vec<(usize, TextureData)>,
    /// `tile.map.water`: the height of the tile's water surface at its four corners.
    pub water: Option<[f32; 4]>,
    /// Spline meshes are local to the tile origin.
    /// Spline meshes (local to the tile origin), their type and whether they cast a shadow.
    pub splines: Vec<(Arc<MeshData>, Arc<SplineType>, bool, DVec3)>,
    /// Terrain-mapped spline faces pooled across types within spatial cells.
    pub ground_splines: Vec<Arc<MeshData>>,
    pub objects: Vec<PlacedObject>,
    /// (type, texture, position, height, width, heading)
    pub trees: Vec<(Arc<ObjectType>, String, DVec3, f64, f64, f64)>,
    pub origin: DVec3,
    pub light_map: Option<TextureData>,
    /// The cut the roads make into the ground (alpha 0 = cut), in tile space.
    pub cut: Option<TextureData>,
    /// Textures prepared on the worker, by file (shared by the tiles of a batch).
    pub images: Arc<HashMap<PathBuf, Arc<TextureData>>>,
}

/// A prepared tile on its way to the GPU (see [`World::begin_upload`]).
pub struct PendingUpload {
    pub prepared: Prepared,
    pub(crate) textures: Vec<PathBuf>,
    pub(crate) types: Vec<Arc<ObjectType>>,
    pub(crate) tg: TileGpu,
    pub(crate) placing: Placing,
}

/// How far [`World::place_step`] got with a tile, and what it made so far.
#[derive(Default)]
pub(crate) struct Placing {
    /// 0: the ground, 1: splines, 2: trees, 3: objects, 4: done.
    pub(crate) phase: u8,
    /// The next spline or tree of the phase.
    pub(crate) next: usize,
    pub(crate) ground_next: usize,
    pub(crate) night_slots: Vec<(usize, usize, MaterialId, MaterialId)>,
    pub(crate) night_modes: Vec<NightMode>,
    pub(crate) light_objects: Vec<LightObject>,
    pub(crate) poles: Vec<i64>,
    pub(crate) splines: usize,
    pub(crate) trees: usize,
    pub(crate) objects: usize,
    /// Seconds per phase (OMSI_PROFILE).
    pub(crate) secs: [f64; 4],
    /// `[terrainmapping]` uses the first [groundtex], without the roads' cut (which
    /// would punch holes into a traffic island). Painted terrain layers belong to the
    /// ground itself and must not be projected onto a spline verge or object.
    pub(crate) terrain_mapping_mat: Option<MaterialId>,
}

/// What a loaded tile added to the world, so that unloading it can take it away again.
#[derive(Default)]
pub struct TileState {
    pub bus_stops: Vec<(i64, DVec3, f64, String)>,
    /// The tile's waiting places (see `World::waiting_places`).
    pub waiting_places: Vec<(i64, DVec3, f64, f32)>,
    pub obstacles: Vec<omsi_sim::collision::Obb>,
    /// The boxes of the tile's parked cars (also among `obstacles`), for the pedestrians.
    pub parked_boxes: Vec<omsi_sim::collision::Obb>,
    /// Objects that collide with their `[collision_mesh]`.
    pub mesh_obstacles: Vec<omsi_sim::collision::MeshObstacle>,
    pub coronas: Vec<StaticCorona>,
    pub lights: Vec<omsi_render::PointLight>,
    pub light_objects: Vec<LightObject>,
    pub night_slots: Vec<(usize, usize, MaterialId, MaterialId)>,
    /// The tile's objects whose night textures follow a `[NightMapMode]` timetable.
    pub night_modes: Vec<NightMode>,
    /// Collision keys of the tile's `[crashmode_pole]` posts (in `World::poles`).
    pub poles: Vec<i64>,
    /// The tile's objects that stop the outside camera.
    pub blockers: Vec<crate::output::Blocker>,
    /// The boxes of the tile's `[petrolstation]` objects (see `World::petrol_stations`).
    pub petrol_stations: Vec<omsi_sim::collision::Obb>,
    /// Parked cars the tile placed (counted in `World::parked_live`).
    pub parked_count: usize,
    /// The tile's echoing places (see `World::reverb_zones`).
    pub reverb_zones: Vec<(omsi_sim::collision::Obb, f32, f32)>,
    pub gpu: TileGpu,
}

/// The GPU resources a tile holds: its own (terrain, splines, masks, text) and the shared
/// ones it uses (object and spline types, textures through them).
#[derive(Default)]
pub struct TileGpu {
    pub instances: Vec<usize>,
    pub meshes: Vec<MeshId>,
    pub textures: Vec<TextureId>,
    pub materials: Vec<MaterialId>,
    /// Shared textures the tile uses directly (ground layers).
    pub shared_textures: Vec<PathBuf>,
    pub types: Vec<usize>,
    pub spline_types: Vec<usize>,
    pub trees: Vec<String>,
    /// Sign texts the tile's objects show (shared, see `GpuCache::text_textures`).
    pub texts: Vec<String>,
}

pub(crate) struct TexEntry {
    pub(crate) id: TextureId,
    pub(crate) alpha: bool,
    /// Has an alpha channel, but every texel is fully opaque (see
    /// [`omsi_texture::gpu::TextureData::alpha_in_use`]).
    pub(crate) opaque_alpha: bool,
    pub(crate) users: usize,
    /// Texels of the uploaded image (mip levels not counted).
    pub(crate) texels: u64,
    /// Bytes on the GPU (all levels, as they are now).
    pub(crate) bytes: u64,
    pub(crate) format: omsi_texture::PixelFormat,
    /// Finest mip levels let go while the textures are over their budget.
    pub(crate) dropped: u32,
}

/// An object type on the GPU.
pub(crate) struct TypeGpu {
    /// Keeps the type (and so the pointer that keys this entry) alive while cached.
    pub(crate) ot: Arc<ObjectType>,
    pub(crate) meshes: Vec<(MeshId, Vec<MaterialId>)>,
    /// `[matl_change]` variants: (mesh index, slot, base, item, variable).
    pub(crate) variants: Vec<(usize, usize, MaterialId, MaterialId, String)>,
    /// Dynamic texture overrides, made only for combinations that placed scripts use.
    /// Rows align with LOD 0 meshes and material slots; each entry holds (base, item).
    pub(crate) dynamic_texture_variants:
        HashMap<Vec<usize>, Vec<Vec<Option<(MaterialId, MaterialId)>>>>,
    /// Lower LODs: (min size, max size, meshes).
    pub(crate) lods: Vec<(f32, f32, Vec<(MeshId, Vec<MaterialId>)>)>,
    pub(crate) materials: Vec<MaterialId>,
    pub(crate) textures: Vec<PathBuf>,
    pub(crate) users: usize,
    /// Some texture has a night copy in the `night` folder beside it (lit windows).
    pub(crate) auto_night: bool,
    /// The screen sizes from and up to which the first level (`meshes`) is drawn (see
    /// `lods`).
    pub(crate) lod0_lo: f32,
    pub(crate) lod0_max: f32,
    /// Material slots whose texture carries `[terrainmapping]`: (level: 0 the first,
    /// k the k-th of `lods`, mesh index in that level, slot).
    pub(crate) terrain_slots: Vec<(usize, usize, usize)>,
    /// The meshes without those slots, made once for all placements: ((level, mesh), id).
    pub(crate) terrain_rest: Vec<((usize, usize), MeshId)>,
}

pub(crate) struct SplineGpu {
    pub(crate) _st: Arc<SplineType>,
    pub(crate) materials: Vec<MaterialId>,
    pub(crate) textures: Vec<PathBuf>,
    pub(crate) users: usize,
    /// Texture slots with `[terrainmapping]` (the grass verges of Berlin-Spandau's
    /// `Splines/Ruede`): drawn with the ground of the tile, like such an object's slots.
    pub(crate) terrain: Vec<usize>,
}

pub(crate) struct TreeGpu {
    pub(crate) material: MaterialId,
    pub(crate) texture: Option<PathBuf>,
    pub(crate) users: usize,
}

/// Materials every tile shares (never freed).
pub struct GroundGpu {
    pub ground_id: Option<TextureId>,
    pub(crate) ground_mat: MaterialId,
    pub(crate) plain_terrain_mat: MaterialId,
    pub(crate) ground_detail: Option<(TextureId, f32)>,
    pub(crate) ground_repeats: f32,
    /// Whether the map's base ground layer (`ground_id`) carries `[moisture]`/`[puddles]`.
    pub(crate) ground_wet: f32,
    pub(crate) water_mat: MaterialId,
    pub(crate) tree_mesh: MeshId,
}

/// Freed scene slots, the lowest handed out first: new resources fill the front of the
/// scene's arrays, so that after a big unload their tail can be cut off (`compact_slots`).
#[derive(Default)]
pub(crate) struct FreeList(pub(crate) std::collections::BinaryHeap<std::cmp::Reverse<usize>>);

/// The GPU side of the loaded tiles: shared resources with their users, and the freed ids
/// that new resources take over.
#[derive(Default)]
pub struct GpuCache {
    pub(crate) textures: hashbrown::HashMap<PathBuf, TexEntry>,
    pub(crate) misses: hashbrown::HashSet<String>,
    pub(crate) types: HashMap<usize, TypeGpu>,
    pub(crate) splines: HashMap<usize, SplineGpu>,
    pub(crate) trees: HashMap<String, TreeGpu>,
    pub ground: Option<GroundGpu>,
    pub(crate) free_meshes: FreeList,
    pub(crate) free_textures: FreeList,
    pub(crate) free_materials: FreeList,
    /// Removed instances by material slot count.
    pub(crate) free_instances: HashMap<usize, FreeList>,
    /// Textures decoded on the thread that draws (not decoded ahead on the loader): how
    /// many, and the seconds they took (`OMSI_PROFILE`).
    pub(crate) sync_decodes: usize,
    pub(crate) sync_decode_secs: f64,
    /// Textures read here as RGBA to spare the frame, to be compressed on a worker.
    pub(crate) wants_upgrade: Vec<PathBuf>,
    /// Pictures read on the thread that draws are read the quick way (a window's frames;
    /// an offscreen load compresses them at once).
    pub(crate) fast_loads: bool,
    /// Textures that lost mip levels to the budget and are near again: read again whole.
    pub(crate) wants_restore: Vec<PathBuf>,
    /// Scenery sign texts (`[texttexture]`) by what they show: (texture, material, users).
    /// A street's name on twenty signs is one texture - every sign had its own, 270 MB
    /// around the Ahlheim main station.
    pub(crate) text_textures: HashMap<String, (TextureId, MaterialId, usize)>,
}

pub struct World {
    pub root: PathBuf,
    pub global: GlobalCfg,
    pub map_dir: PathBuf,
    /// Indexed parked car lists of the map, loaded when a parking space uses one.
    pub(crate) parklist: Mutex<HashMap<usize, Vec<String>>>,
    /// Render textures of the player's mirrors (`reflexionN.bmp`), by camera index.
    pub mirror_textures: Mutex<Vec<Option<TextureId>>>,
    /// Width / height of the glass of the player bus mirror N (from the mesh that shows its
    /// picture), 0 when not known: the shape of the panels that copy the mirrors to the screen.
    pub mirror_aspect: Mutex<Vec<f32>>,
    /// Where the glass of the player bus mirror N is and which way the picture's u and v run
    /// over it (middle, position per u, position per v; the bus's frame), from the mesh that
    /// shows it: how the panels that copy the mirrors turn the picture.
    pub mirror_glass: Mutex<Vec<Option<MirrorGlass>>>,
    pub(crate) object_types: Mutex<HashMap<String, Option<Arc<ObjectType>>>>,
    pub(crate) spline_types: Mutex<HashMap<String, Option<Arc<SplineType>>>>,
    pub textures: Arc<TextureCache>,
    /// World position (with terrain height) and rotation of every loaded map object by id.
    pub object_positions: Mutex<HashMap<i64, (DVec3, [f64; 3])>>,
    /// The objects whose id is used on more than one tile, by (tile, id) (see
    /// `MapIndex::duplicates`, `World::entry_point_place`).
    pub object_dups: Mutex<HashMap<((i32, i32), i64), (DVec3, [f64; 3])>>,
    /// Loaded terrains by tile coordinate.
    pub terrains: Arc<RwLock<HashMap<(i32, i32), Arc<Terrain>>>>,
    /// Surface rasters (roads) by tile coordinate.
    pub surfaces: Arc<RwLock<HashMap<(i32, i32), Arc<TileSurface>>>>,
    /// GPU resources per vehicle type (by .bus path) and paint scheme, shared by AI vehicles.
    pub(crate) vehicle_gpu: Mutex<HashMap<VehicleKey, VehicleSet>>,
    /// GPU textures of vehicles by file: one bus spawned in twenty adverts used to upload
    /// its whole texture set twenty times (200 ms a spawn, 1.5 fps on Spandau).
    /// (With the number of sets holding each.)
    pub(crate) vehicle_textures: Arc<Mutex<HashMap<PathBuf, (TextureId, usize)>>>,
    /// GPU meshes of vehicles by (bus file, mesh index), shared across paint schemes.
    pub(crate) vehicle_meshes: Arc<Mutex<HashMap<(PathBuf, usize), (MeshId, usize)>>>,
    /// Vehicle meshes and textures made on a worker, until their set is uploaded.
    pub(crate) vehicle_ready: Arc<Mutex<PreparedVehicles>>,
    /// Textures uploaded as RGBA to spare a frame, being compressed on the workers, and the
    /// compressed ones waiting to be swapped in.
    pub(crate) upgrades_pending: Mutex<hashbrown::HashSet<PathBuf>>,
    pub(crate) upgrades_done: Arc<Mutex<Vec<(PathBuf, Arc<TextureData>)>>>,
    /// Roller-blind pictures (`[matl_freetex]`) uploaded as RGBA, to be compressed.
    pub(crate) freetex_upgrades: Arc<Mutex<Vec<PathBuf>>>,
    /// OMSI's `[texmemlimit]`: bytes the scenery and vehicle textures may take on the GPU
    /// (0 = no limit), and when the budget was last looked at.
    pub(crate) texture_limit: std::sync::atomic::AtomicU64,
    pub(crate) budget_checked: Mutex<Option<std::time::Instant>>,
    /// Traffic-path lanes collected while building tiles.
    pub lanes: Mutex<Vec<omsi_sim::traffic::Lane>>,
    /// The tiles whose lanes and parked cars have been put into `lanes` and `parked_cars`.
    /// These two and `lanes` are only filled, and should only be taken, while the `lanes`
    /// lock is held: whoever takes them then has every parked car together with the lanes
    /// it stands beside, and knows which tiles those came from.
    pub lane_tiles: Mutex<Vec<(i32, i32)>>,
    /// Traffic light programs of placed crossings, and which map object owns each.
    pub traffic_lights: Mutex<Vec<TrafficLightController>>,
    pub controller_of_object: Mutex<HashMap<i64, usize>>,
    /// Placed traffic light lamps.
    pub light_objects: Mutex<Vec<LightObject>>,
    /// Placed objects with scripts / animations.
    pub scripted: Mutex<Vec<ScriptedObject>>,
    /// The clock and the departure boards the scenery scripts read.
    pub timetable_boards: Mutex<StopBoards>,
    /// The map's `Holidays.txt`, read when first asked.
    pub calendar: std::sync::OnceLock<omsi_map::Calendar>,
    /// The number plates of `registrations.txt` (the active chrono scenarios' first, the
    /// latest before, then the map's own: the original), read when first asked.
    pub registrations: std::sync::OnceLock<Vec<String>>,
    /// The clock the run starts at: a scenery object placed before the simulation's clock
    /// reaches the boards runs its `{init}` on it (it ran on 09:00 of 1989).
    pub start_clock: Mutex<omsi_sim::SimClock>,
    /// Obstacles for vehicle collisions (of the loaded tiles; replaced when they change).
    pub collision: Mutex<Arc<omsi_sim::collision::CollisionWorld>>,
    /// `[crashmode_pole]` objects of the loaded tiles by collision key: where they stand and
    /// their instances, so that a post a vehicle knocked over can be laid on the ground
    /// (see [`World::lay_down_pole`]). A tile's posts leave with it: its instances are
    /// handed to other objects.
    pub poles: Mutex<HashMap<i64, (DVec3, Mat4, Vec<usize>)>>,
    /// Posts knocked over in this run and the way they fell: a tile loaded again lays them
    /// down again.
    pub(crate) fallen_poles: Mutex<HashMap<i64, DVec3>>,
    /// The parked cars of the loaded tiles by collision key, so that one can pull out into
    /// the traffic (see [`World::depart_parked`]). They leave with their tile.
    pub parked_objects: Mutex<HashMap<i64, ParkedObject>>,
    /// The instances of the route arrows the map's author put up (`[helparrow]` objects) by
    /// tile: drawn only while OMSI 2's route arrows are on (see [`World::show_help_arrows`]).
    pub help_arrows: Mutex<HashMap<(i32, i32), Vec<usize>>>,
    /// Whether they are drawn now.
    pub(crate) help_arrows_shown: std::sync::atomic::AtomicBool,
    /// Parked cars that drove off in this run: their space stays empty when the tile comes
    /// back.
    pub(crate) departed: Mutex<std::collections::HashSet<i64>>,
    /// What a parked car that drove off left behind: its object and its boxes, for an AI
    /// car of the same kind that parks in the space again (`return_parked`).
    pub(crate) departed_objects: Mutex<
        std::collections::HashMap<
            i64,
            (
                ParkedObject,
                Vec<omsi_sim::collision::Obb>,
                Vec<omsi_sim::collision::Obb>,
            ),
        >,
    >,
    /// The loaded tiles' own objects by map id, for the object editor (`crate::world::editor`).
    pub edit_objects: Mutex<HashMap<i64, EditObject>>,
    /// What the object editor did this run, by map id: kept over tile reloads until saved.
    pub object_edits: Mutex<HashMap<i64, ObjectEdit>>,
    /// The ground as the editor's brush has left it, by tile (read instead of the file).
    pub terrain_edits: Mutex<HashMap<(i32, i32), Terrain>>,
    /// Placed `[busstop]` objects: (map id, world position, heading, name).
    pub bus_stops: Mutex<Vec<(i64, DVec3, f64, String)>>,
    /// Where people wait at the stops: the `[passpos]` points of placed objects with a
    /// `[passengercabin]` (the maps' `people_standing_*` markers and bus shelters) as
    /// (object id, world position, heading in degrees, seat height - 0 for a standing place).
    pub waiting_places: Mutex<Vec<(i64, DVec3, f64, f32)>>,
    /// Passenger cabins of waiting objects by file, read once.
    pub(crate) waiting_cabins: Mutex<HashMap<PathBuf, Option<Arc<omsi_vehicle::PassengerCabin>>>>,
    /// Counts the changes of the loaded tiles (see [`World::refresh_tile_lists`]): whoever
    /// keeps what it derived from the stops, the waiting places or the ground looks again.
    pub tiles_generation: std::sync::atomic::AtomicU64,
    /// Parked cars placed on `[carpark_p]` spaces: world position and heading (deg), for
    /// the traffic to steer round (see `lane_tiles`).
    pub parked_cars: Mutex<Vec<(DVec3, f64)>>,
    /// The boxes of the parked cars of the loaded tiles, which people walk round.
    pub parked_boxes: Mutex<Arc<Vec<omsi_sim::collision::Obb>>>,
    /// Placed objects with particle systems (chimney smoke, the fireworks, a memorial's
    /// flame) by tile.
    pub particle_objects: Mutex<HashMap<(i32, i32), Vec<ParticleObject>>>,
    /// The boxes of the loaded `[petrolstation]` objects (the depots' fuel and wash yards):
    /// OMSI lets the pump and the wash run only while the bus's box
    /// overlaps one, and sends the workshop's team out when the bus stands in none.
    pub petrol_stations: Mutex<Vec<omsi_sim::collision::Obb>>,
    /// Parked cars standing in the loaded tiles, and the options' `[AIMaxCountParked]`
    /// (the share of the spaces with a car, 0 to 100 percent).
    pub parked_live: std::sync::atomic::AtomicUsize,
    pub parked_max: i64,
    /// Chosen once per run: which cars stand on the parking spaces (`OMSI_SEED` repeats it).
    pub(crate) parked_seed: u64,
    /// Places that echo (`[triggerbox_new]` + `[triggerbox_setreverb]`: the railway bridges'
    /// underpasses): the box, the reverberation time (s) and the distance (m) over which it
    /// fades in at the box's sides.
    pub reverb_zones: Mutex<Vec<(omsi_sim::collision::Obb, f32, f32)>>,
    /// The loaded tiles' night light maps (`.map.LM.bmp`), for the light map atlas.
    pub light_maps: Mutex<HashMap<(i32, i32), Arc<omsi_texture::Image>>>,
    pub(crate) light_maps_generation: std::sync::atomic::AtomicU64,
    /// The atlas as last filled: centre tile and the generation of `light_maps`.
    pub(crate) light_map_atlas: Mutex<Option<((i32, i32), u64)>>,
    /// The map's `signalroutes.cfg` (unit `mc_fahrstrasse`): which track pieces each railway
    /// signal protects, its distant signal, the next signal and a speed limit.
    pub signal_routes: Vec<omsi_map::ailists::SignalRoute>,
    /// Chrono folders active on the sim date, in order, and the merged AI lists / date.
    /// The chrono scenarios in force on the sim date (changed at midnight: `set_date`).
    pub chrono_dirs: parking_lot::RwLock<Vec<PathBuf>>,
    pub ailists: omsi_map::AiLists,
    pub date: i32,
    /// Ticket pack (chrono folders may override the map's).
    pub ticket_pack: String,
    /// Fonts for text and script textures, shared by all vehicles.
    pub fonts: Arc<Mutex<omsi_sim::texttex::FontLibrary>>,
    /// Scenery material variants switched by `NightlightA`: (instance, slot, material on, off).
    pub night_slots: Mutex<Vec<(usize, usize, MaterialId, MaterialId)>>,
    /// Objects whose night textures follow a `[NightMapMode]` timetable (see `update_night_modes`).
    pub night_modes: Mutex<Vec<NightMode>>,
    /// Light coronas and point lights of placed scenery objects.
    pub static_coronas: Mutex<Vec<StaticCorona>>,
    pub static_lights: Mutex<Vec<omsi_render::PointLight>>,
    /// Every tile file of the map read once: splines for the spline attachment rows,
    /// objects for entry points and stops of tiles that are not loaded.
    pub(crate) index: Mutex<Option<Arc<MapIndex>>>,
    /// What each loaded tile added (see [`TileState`]).
    pub tile_state: Mutex<HashMap<(i32, i32), TileState>>,
    /// Tiles that have been loaded at least once: their lanes, light programs and parked
    /// cars are in the lists for good.
    pub(crate) seeded: Mutex<hashbrown::HashSet<(i32, i32)>>,
    /// The GPU side of the loaded tiles.
    pub gpu: Mutex<GpuCache>,
    /// Types this installation lacks, logged once each (file, what it is).
    pub(crate) missing: Mutex<hashbrown::HashMap<String, &'static str>>,
    /// Tiles read and typed that loaded tiles (or tiles on their way) depend on.
    pub(crate) staged: Mutex<HashMap<(i32, i32), Arc<StagedTile>>>,
    pub(crate) layout: Mutex<Option<Arc<TileLayout>>>,
    /// Scenery objects' sound configurations, read once per file.
    pub(crate) sound_cfgs: Mutex<HashMap<PathBuf, Option<Arc<omsi_vehicle::SoundCfg>>>>,
    /// The trolleybus content convention of the installation (None: no trolleybus content,
    /// and no wire is looked for).
    pub(crate) trolley: Option<Arc<omsi_trolley::Convention>>,
    /// The overhead wire of the loaded tiles (empty on a map without wire).
    pub wires: Mutex<omsi_trolley::WireNetwork>,
}

/// A placed object's particle systems (`[smoke]`, `[particle_emitter]`).
pub struct ParticleObject {
    pub map_id: i64,
    pub pos: DVec3,
    pub rot: Mat4,
    pub set: omsi_sim::particles::ParticleSet,
}

/// A `[light_enh]`/`[light_enh_2]` of a placed scenery object.
#[derive(Debug, Clone)]
pub struct StaticCorona {
    pub corona: omsi_render::Corona,
    /// What switches it: a constant, the night flag, or an object variable (treated as on).
    pub switch: LightSwitch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LightSwitch {
    Constant(f32),
    Night,
    Variable(String),
}

/// A `[maplight]` as a light map bakes it: where it stands (world x, y), its height over its
/// object, colour and core radius.
#[derive(Debug, Clone, Copy)]
pub struct BakeLamp {
    pub x: f64,
    pub y: f64,
    pub height: f32,
    pub color: [f32; 3],
    pub radius: f32,
}

/// An object whose night textures are lit by a `[NightMapMode]` timetable.
#[derive(Debug, Clone, Copy)]
pub struct NightMode {
    pub inst: usize,
    /// This object's in-use window and darkness threshold (see [`InUse`]).
    pub use_: InUse,
    pub slots: usize,
}

/// When a building with a `[NightMapMode]` is in use and when its windows are lit, as
/// OMSI decides once per object and every frame: in use
/// between `on` and `off` (seconds of the day; mode 2 homes 5.5-9.5 h until 22-24 h, mode 3
/// offices 6-8 h until 17-19 h on working days that are no holiday, mode 4 schools 6-8 h until
/// 14-16 h on school days, any other mode all day); lit while in use and the daylight under
/// `threshold` (0.6 for mode 0, else 0.3-0.75).
#[derive(Clone, Copy, Debug)]
pub struct InUse {
    pub mode: i32,
    pub on: f64,
    pub off: f64,
    pub threshold: f32,
}

/// The day as the in-use rules ask about it.
#[derive(Clone, Copy, Debug, Default)]
pub struct DayKind {
    pub workday: bool,
    pub holiday: bool,
    pub school_holiday: bool,
}

/// GPU-side representation of a vehicle instance: one render instance per mesh.
pub struct VehicleRender {
    pub instances: Vec<usize>,
    /// Materials made for this vehicle alone (its text and script texture slots).
    pub own_materials: Vec<MaterialId>,
    /// The shared set it is drawn with (None for the player's own).
    pub set: Option<VehicleKey>,
    /// `[matl_change]` / `[texchanges]` slots whose material a variable switches.
    pub variants: Vec<VariantSlot>,
    /// GPU texture per `[texttexture]` index.
    pub text_textures: Vec<Option<TextureId>>,
    /// GPU texture per `[scripttexture]` index.
    pub script_textures: Vec<Option<TextureId>>,
    /// `script_textures` belong to the vehicle this part is coupled to (`[scriptshare]`):
    /// they are not this render's to give back.
    pub shared_script: bool,
    /// The script textures (cockpit and passenger displays, 1024×512 pictures for a C2) wait
    /// with their upload this frame: the vehicle is far and it is not its half second.
    pub displays_far: bool,
    /// The half second a far vehicle's displays were last uploaded in.
    pub display_tick: u64,
    /// `[smoothskin]` meshes drawn from a copy of their own (the player's articulated bus's
    /// bellows): (mesh index, the copy, the bone transforms it was last shaped for).
    pub skinned: Vec<(usize, MeshId, Vec<Mat4>)>,
    /// An AI vehicle out of sight: its instances are hidden and not updated (see
    /// `Traffic::sync`).
    pub hidden: bool,
    /// The renderer's slots for the vehicle's `[interiorlight]` lamps (first, count), taken
    /// the first time the vehicle is synced (see `sync_interior_lamps`).
    pub interior_lamps: std::cell::Cell<Option<(u32, u32)>>,
    /// The runs of lamp slots and which of the vehicle's lamps each holds.
    pub interior_blocks: std::cell::OnceCell<Vec<(u32, Vec<usize>)>>,
}

/// A material slot whose texture is generated per vehicle (text or script texture).
#[derive(Debug, Clone)]
pub struct DynSlot {
    pub mesh: usize,
    pub slot: usize,
    pub text: Option<usize>,
    pub script: Option<usize>,
    pub script_trans: Option<usize>,
    pub tex: Option<TextureId>,
    pub alpha: AlphaMode,
    pub transmap: Option<(TextureId, bool)>,
    pub night: Option<TextureId>,
    pub lightmap: Option<TextureId>,
    pub envmap: Option<(TextureId, f32)>,
    /// `[matl_texadress_*]`: how the slot's textures read outside [0, 1].
    pub address: omsi_render::TexAddressing,
    /// Depth handling, reflection mask and specular term of the slot.
    pub extra: MaterialExtra,
    /// Diffuse and emissive colour of the slot's D3D material (see `d3d_material`); a
    /// script texture is drawn unlit and keeps its own colours.
    pub color: [f32; 4],
    pub emissive: [f32; 3],
}

/// A material variant switched by a variable.
#[derive(Clone)]
pub struct VariantSlot {
    pub mesh: usize,
    pub slot: usize,
    pub base: MaterialId,
    pub item: MaterialId,
    /// Items 2, 3, ... of the first `[matl_change]`.
    pub more: Vec<MaterialId>,
    /// `[matl_change]` variable: at 1 (rounded) the item variant shows.
    pub var: String,
    /// The variables of the slot's further `[matl_change]`s: the item shows while any of
    /// them is on as well (OMSI: each record picks its item by its own
    /// variable, and one at 0 leaves the material to the others).
    pub more_vars: Vec<String>,
    /// `[texchanges]`: the (base, item) pair of every entry of the master, in order.
    pub entries: Vec<(MaterialId, MaterialId)>,
    /// `[texchanges]` variable: its integer value picks the entry.
    pub tex_var: String,
    /// Free textures for the plain material and, independently, its switched item.
    pub free: Vec<FreeTex>,
    /// How to build a material of this slot for a texture loaded later.
    pub spec: SlotSpec,
    /// The textures `base`/`item` and each entry were made with (made again per vehicle
    /// when the slot shows the vehicle's own pictures, see `SlotSpec::per_vehicle`).
    pub base_tex: Option<TextureId>,
    pub entry_tex: Vec<Option<TextureId>>,
    /// Several `[matl_lightmap]`s on the slot (see `MultiLight`).
    pub lights: Option<MultiLight>,
}

/// A material slot with several `[matl_lightmap]`s, each a texture and a variable (the
/// LiAZ 5292's saloon: the cab lamp, saloon circuit 1 and circuit 2). OMSI keeps them
/// all; drawn with only the last one, the
/// saloon stayed unlit whenever circuit 2 was off. The slot's light map is the sum of the
/// maps switched on, made the first time that combination shows and shared by every
/// vehicle of the kind; the slot is as bright as its brightest variable.
#[derive(Clone)]
pub struct MultiLight {
    /// The maps' files and variables, in the order the model lists them.
    pub maps: Vec<(PathBuf, String)>,
    /// The set's own materials (the last map), shown while none is on.
    pub plain: (MaterialId, MaterialId),
    /// Materials made per switched-on combination (bit k: map k).
    pub cache: HashMap<u32, (MaterialId, MaterialId)>,
    pub current: u32,
    /// Composite maps are shared like the vehicles' pictures (see `FreeTex::shared`).
    pub shared: Arc<Mutex<HashMap<PathBuf, (TextureId, usize)>>>,
    pub held: Vec<PathBuf>,
}

#[derive(Clone)]
pub struct FreeTex {
    pub var: String,
    /// The original named texture can be used by several stages (a display commonly
    /// names the same black texture as its diffuse and its switched night map).
    pub key: Option<TextureId>,
    pub diffuse: bool,
    /// A declaration inside `[matl_item]` must not change the unpowered material.
    pub item_only: bool,
    /// Where the file name is looked up (the vehicle's texture folders).
    pub dirs: Vec<PathBuf>,
    pub textures: Arc<omsi_texture::TextureCache>,
    /// File name (lower case) → the materials already built for it.
    pub cache: HashMap<String, (MaterialId, MaterialId)>,
    /// The name currently applied.
    pub current: Option<String>,
    /// The world's shared vehicle textures (a picture is one texture for every vehicle
    /// showing it; every bus had its own copy, half a gigabyte of destination pictures on
    /// Ahlheim), the pictures this vehicle holds, and where pictures uploaded as RGBA are
    /// sent to be compressed.
    pub shared: Arc<Mutex<HashMap<PathBuf, (TextureId, usize)>>>,
    pub held: Vec<PathBuf>,
    pub wants_upgrade: Arc<Mutex<Vec<PathBuf>>>,
}

/// How one material slot is built, so that the same description can be applied to every
/// texture a `[texchanges]` master or a `[matl_freetex]` string switches between.
#[derive(Clone)]
pub struct SlotSpec {
    pub base: Look,
    /// The `[matl_item]` half.
    pub item: Option<Look>,
    /// The first `[matl_change]`'s items after its first (shown at 2, 3, ...).
    pub more: Vec<Look>,
}

/// How one half of a material slot (the plain material or its `[matl_item]`) is drawn.
#[derive(Clone)]
pub struct Look {
    pub alpha: AlphaMode,
    pub color: [f32; 4],
    pub emissive: [f32; 3],
    pub unlit: bool,
    /// A picture of the vehicle's own in place of the diffuse texture (see `DynTex`).
    pub diffuse: Option<TextureId>,
    pub transmap: Option<(TextureId, bool)>,
    pub night: Option<TextureId>,
    pub lightmap: Option<TextureId>,
    pub envmap: Option<(TextureId, f32)>,
    pub extra: MaterialExtra,
    pub dyn_tex: DynTex,
}

/// The pictures of its own vehicle a half of a slot shows: `[useTextTexture]`,
/// `[useScriptTexture]` and a script texture as the transparency map
/// (`[matl_transmap] \S:n`), and whether it is addressed without repeating.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DynTex {
    pub(crate) text: Option<usize>,
    pub(crate) script: Option<usize>,
    pub(crate) script_trans: Option<usize>,
    pub(crate) address: omsi_render::TexAddressing,
}

/// A vehicle type with a paint scheme, as its GPU set is known by.
pub type VehicleKey = (PathBuf, Option<usize>);

/// The GPU side of a vehicle type in one paint scheme, shared by its AI copies: meshes and
/// materials, the slots every copy fills itself, and what the set holds of the shared
/// textures and meshes (given back when the set is trimmed).
#[derive(Clone)]
pub struct VehicleSet {
    pub(crate) meshes: Vec<(MeshId, Vec<MaterialId>)>,
    pub(crate) dyn_slots: Vec<DynSlot>,
    pub(crate) variants: Vec<VariantSlot>,
    pub(crate) textures: Vec<PathBuf>,
    pub(crate) mesh_keys: Vec<(PathBuf, usize)>,
    pub(crate) materials: Vec<MaterialId>,
    /// Vehicles drawn with it now, and since when nobody is.
    pub(crate) users: usize,
    pub(crate) idle_since: Option<std::time::Instant>,
}

/// What reads vehicle sets ahead on a worker thread: their textures and meshes, made on the
/// GPU right there (the device takes calls from any thread) and waiting in `ready` until the
/// set is uploaded - which then only puts them into the scene and makes the materials. A
/// C2's set took 60 ms on the thread that draws when it had to read and upload it all.
#[derive(Clone)]
pub struct VehiclePrefetch {
    pub(crate) root: PathBuf,
    pub(crate) textures: Arc<TextureCache>,
    pub(crate) on_gpu: Arc<Mutex<HashMap<PathBuf, (TextureId, usize)>>>,
    pub(crate) meshes_on_gpu: Arc<Mutex<HashMap<(PathBuf, usize), (MeshId, usize)>>>,
    pub(crate) ready: Arc<Mutex<PreparedVehicles>>,
    pub(crate) gpu: (wgpu::Device, wgpu::Queue),
    pub(crate) mesh_pages: bool,
}

/// Vehicle meshes and textures made on a worker, by (bus file, mesh) and by file.
#[derive(Default)]
pub(crate) struct PreparedVehicles {
    pub(crate) meshes: HashMap<(PathBuf, usize), omsi_render::PreparedMesh>,
    pub(crate) textures:
        HashMap<PathBuf, (omsi_render::PreparedTexture, omsi_texture::PixelFormat)>,
}

/// The whole map for the navigator (see [`World::navigation_map`]).
pub struct NavigationMap {
    pub lanes: Vec<Lane>,
    /// Asphalt footprints used to corroborate editor-only driving paths. They are evidence
    /// for roads, not streets to draw: a paved yard or median has no road centre line.
    pub road_surfaces: Vec<(Vec<DVec3>, f32)>,
    /// Every placed object's position by id (bus stops beyond the loaded tiles).
    pub places: HashMap<i64, DVec3>,
    /// Street name signs: where, the object's heading and the name on it.
    pub signs: Vec<(DVec3, f64, String)>,
}

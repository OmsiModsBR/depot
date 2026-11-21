//! What a picture draws, as data: the frame plan (every draw after culling, level of detail and
//! the alpha order, with the picture's camera, light and sky) and the sink that receives it.
//! It is the contract with the Vanilla renderer, which draws the plan on another device.
//!
//! Data only: ids, matrices and lists. The renderer makes the plan (`renderer::mirror`).

use crate::passes::{
    Batch, PIPE_ALPHA_TEST, PIPE_BLEND, PIPE_BLEND_NO_WRITE, PIPE_OPAQUE, PIPE_SURFACE_DEPTH,
    PIPE_TERRAIN_PAINT,
};
use crate::scene::{MaterialId, MeshId, TextureId};

/// How a batch is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanKind {
    Opaque,
    AlphaTest,
    Blend,
    /// Blended without writing depth.
    BlendNoWrite,
    /// A road surface that writes depth only where its alpha passes.
    SurfaceDepth,
    /// A painted ground layer over the terrain.
    TerrainPaint,
}

/// One draw: indices `first..first + count` of mesh `mesh` (counted in the mesh's own index
/// list), with material `material`, once per entry of `FramePlan::list[instances]`.
#[derive(Debug, Clone)]
pub struct PlanBatch {
    pub kind: PlanKind,
    /// Back faces are culled (the visible side arrives clockwise, as in Direct3D).
    pub cull: bool,
    /// A road surface (the shader pulls it towards the eye).
    pub surface: bool,
    pub mesh: MeshId,
    pub first: u32,
    pub count: u32,
    pub material: MaterialId,
    pub instances: std::ops::Range<u32>,
}

impl PlanBatch {
    pub(crate) fn from_batch(b: &Batch) -> Self {
        let kind = match b.pipe / 4 {
            PIPE_OPAQUE => PlanKind::Opaque,
            PIPE_ALPHA_TEST => PlanKind::AlphaTest,
            PIPE_BLEND => PlanKind::Blend,
            PIPE_BLEND_NO_WRITE => PlanKind::BlendNoWrite,
            PIPE_SURFACE_DEPTH => PlanKind::SurfaceDepth,
            PIPE_TERRAIN_PAINT => PlanKind::TerrainPaint,
            k => unreachable!("pipeline kind {k}"),
        };
        Self {
            kind,
            cull: b.pipe & 2 != 0,
            surface: b.pipe & 1 != 0,
            mesh: b.mesh as MeshId,
            first: b.first,
            count: b.count,
            material: b.material as MaterialId,
            instances: b.instances.clone(),
        }
    }
}

/// What a picture (the window's, or a render texture's) draws this frame, in the order this renderer draws it.
///
/// Per-draw data: entry `e = list[i]` of an instance `i` of a batch has its model matrix
/// (column-major, relative to `render_origin`) at `Scene::slot_models()[e]` and two parameter
/// vectors at `Scene::slot_params()[2 e]` and `[2 e + 1]` (alpha, visible, uv offset; light,
/// night, interior lamp code, surface code).
#[derive(Debug, Clone)]
pub struct FramePlan {
    pub width: u32,
    pub height: u32,
    /// The render texture this picture is drawn into (a mirror's), `None` for the window's.
    /// A texture's picture has no overlays and no HUD.
    pub target: Option<TextureId>,
    pub render_origin: glam::DVec3,
    /// Column-major, from render-origin-relative world to clip space (wgpu depth 0..1).
    pub view_proj: [[f32; 4]; 4],
    /// xyz the camera relative to the render origin.
    pub cam_pos: [f32; 4],
    /// The light model as this renderer's shaders read it (`CameraUniform`).
    pub sun_dir: [f32; 4],
    pub sun_color: [f32; 4],
    /// The weather's ambient light (xyz) and the snow (w). Unlike the camera's, without the
    /// moonlight Vanilla+ adds: the renderer that takes the plan draws OMSI 2's nights.
    pub ambient: [f32; 4],
    pub fog: [f32; 4],
    pub sky_color: [f32; 4],
    /// The camera's right and up axes (xyz), which lay out the sphere maps of `[matl_envmap]`.
    pub cam_right: [f32; 4],
    pub cam_up: [f32; 4],
    /// The render origin's map position modulo the procedural patterns' period (xy, metres),
    /// for patterns laid on the map that must not jump when the origin moves.
    pub world_origin: [f32; 4],
    /// How wet the outdoor surfaces are (0 dry .. 1 soaked): what a road with `[moisture]`
    /// darkens, shines and gathers puddles by.
    pub wetness: f32,
    /// The rain reflection a second renderer lays on the puddles of the window's picture:
    /// the texture the mirrored camera's picture was drawn into this frame and that camera's
    /// view-projection (column-major, relative to `render_origin`). `None` in the pictures
    /// drawn into textures and when no reflection was drawn.
    pub rain_reflection: Option<(TextureId, [[f32; 4]; 4])>,
    /// Where the lamps' grid lies: x, y its south-west corner (render-origin relative), z a
    /// cell's side, w the cells per side (see [`crate::Scene::frame_lights`]).
    pub light_grid: [f32; 4],
    /// Where the tile light map atlas lies: x, y its south-west corner (render-origin
    /// relative), z its side, w 1 when it is set.
    pub light_map: [f32; 4],
    /// The sky's colour the picture is cleared to, as the lighting gives it (w 1).
    pub clear: [f32; 4],
    /// How hard it rains (0..1; none while it snows): what a second renderer gathers
    /// raindrops on glass by.
    pub rain: f32,
    /// The sky dome's light: x the sun's azimuth (radians, clockwise from north), yzw the
    /// weights of the day, twilight and night pictures ([`crate::Scene::sky_textures`]).
    pub sky: [f32; 4],
    /// The cloud layer: x the cover (0 none .. 1 closed), yz the drift with the wind in
    /// fractions of the picture's tile, w the layer's base height in metres.
    pub clouds: [f32; 4],
    /// Whether the glow around the sun is wanted (the `sun_glow` setting).
    pub sun_glow: bool,
    /// xyz towards the moon (world), w 1: where the classic sky draws the moon's picture
    /// ([`crate::Scene::sky_bodies`]).
    pub moon: [f32; 4],
    /// The starry sky: the rows of the rotation from a world direction into the stars'
    /// frame, w of the first how bright the stars are, w of the second the star map's cells
    /// across a face ([`crate::Scene::sky_stars`]; 0: no map).
    pub stars: [[f32; 4]; 3],
    /// Map lamps one spot of the ground takes into its light (0 = all).
    pub lamp_cap: u32,
    /// The whole camera uniform this renderer's shaders read, vector by vector, for a
    /// second renderer that ports them with the same layout.
    pub camera: Vec<[f32; 4]>,
    /// Indexes [`crate::Scene::slot_models`] (and two each into `slot_params`); a batch's
    /// `instances` is a range of this list.
    pub list: Vec<u32>,
    /// The main pass, then the vehicle the camera is in (drawn after it).
    pub main: Vec<PlanBatch>,
    pub cab: Vec<PlanBatch>,
    /// HUD pictures drawn over the scene: (texture, rect in pixels x0, y0, x1, y1).
    pub overlays: Vec<(TextureId, [f32; 4])>,
}

/// Receives each window picture's plan with the scene it was made from.
pub type PlanSink = Box<dyn FnMut(&FramePlan, &crate::scene::Scene) + Send>;

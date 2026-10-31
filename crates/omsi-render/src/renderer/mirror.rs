//! A second renderer's view of the frame: the asset mirror (every mesh and texture as it goes
//! up, in CPU form) and the frame plan (what a picture draws, after culling, level of detail
//! and the alpha order). With both, another GPU backend can draw the scene this renderer keeps
//! the books for, while this one runs on a device that draws nothing (wgpu's noop backend).
//!
//! Nothing here changes what this renderer draws: without a mirror or a sink set, the hooks
//! are a branch on `None`.

use crate::passes::{
    Batch, PIPE_ALPHA_TEST, PIPE_BLEND, PIPE_BLEND_NO_WRITE, PIPE_OPAQUE, PIPE_SURFACE_DEPTH,
    PIPE_TERRAIN_PAINT,
};
use crate::scene::{MaterialId, MeshId, TextureId, Vertex};
use std::cell::RefCell;

/// A renderer's asset mirror. Counted in [`crate::assets`] while it is set, so prepared
/// assets keep their CPU copies; dropping the renderer stops it.
#[derive(Default)]
pub(crate) struct MirrorSlot(RefCell<Option<AssetMirror>>);

impl MirrorSlot {
    pub(crate) fn set(&self, mirror: Option<AssetMirror>) {
        let mut slot = self.0.borrow_mut();
        match (slot.is_some(), mirror.is_some()) {
            (false, true) => crate::assets::mirror_started(),
            (true, false) => crate::assets::mirror_stopped(),
            _ => {}
        }
        *slot = mirror;
    }
}

impl Drop for MirrorSlot {
    fn drop(&mut self) {
        if self.0.get_mut().is_some() {
            crate::assets::mirror_stopped();
        }
    }
}

/// A texture's pixels as they reached this renderer.
#[derive(Debug, Clone, Copy)]
pub enum MirrorTexture<'a> {
    /// RGBA8, straight alpha, sRGB colours; `mipmaps`: a mip chain is wanted.
    Image {
        image: &'a omsi_texture::Image,
        mipmaps: bool,
    },
    /// RGBA8 or BC1-3 blocks with their levels (`omsi_texture::gpu`).
    Data(&'a omsi_texture::TextureData),
}

/// What happened to the scene's assets. Ids are this renderer's, so the mirror can keep its
/// resources in the same slots.
#[derive(Clone, Copy)]
pub enum MirrorEvent<'a> {
    /// A mesh in slot `id` (new, or put in place of what was there).
    Mesh {
        id: MeshId,
        vertices: &'a [Vertex],
        indices: &'a [u32],
    },
    /// New vertices for mesh `id` (same count or fewer), e.g. a skinned human's pose.
    MeshVertices {
        id: MeshId,
        vertices: &'a [Vertex],
    },
    FreeMesh(MeshId),
    /// Mesh `from` moved into slot `into` (`Renderer::recycle_mesh`).
    MoveMesh {
        from: MeshId,
        into: MeshId,
    },
    /// A texture in slot `id` (new, replaced or rewritten).
    Texture {
        id: TextureId,
        texture: MirrorTexture<'a>,
    },
    FreeTexture(TextureId),
    MoveTexture {
        from: TextureId,
        into: TextureId,
    },
    /// A tile's light map put into its place of the tile light map atlas
    /// (`Renderer::set_light_map_tile`): `slot` column from the west, row from the north, of
    /// [`crate::LM_ATLAS_TILES`] a side; `rgba` [`crate::LM_TILE_PX`] square, sRGB, black
    /// where the tile has none.
    LightMapTile {
        slot: (u32, u32),
        rgba: &'a [u8],
    },
    /// Picture `id` of the light coronas (`Corona::texture`, `Renderer::set_corona_texture`):
    /// RGBA8, straight alpha, sRGB colours; its brightness is the glow's shape.
    CoronaTexture {
        id: u16,
        image: &'a omsi_texture::Image,
    },
    /// The picture of a smoke particle (`Renderer::set_smoke_texture`): RGBA8, straight
    /// alpha, sRGB colours; its alpha is the particle's mask.
    SmokeTexture {
        image: &'a omsi_texture::Image,
    },
}

/// Receives the asset events; see [`MirrorEvent`].
pub type AssetMirror = Box<dyn FnMut(MirrorEvent<'_>) + Send>;

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
    pub ambient: [f32; 4],
    pub fog: [f32; 4],
    pub sky_color: [f32; 4],
    /// The camera's right and up axes (xyz), which lay out the sphere maps of `[matl_envmap]`.
    pub cam_right: [f32; 4],
    pub cam_up: [f32; 4],
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

/// The plan of the picture `f` with the lists `lists`.
pub(crate) fn plan_of(
    f: &super::frame::FrameContext<'_>,
    lists: &super::frame::DrawLists,
    light_map: [f32; 4],
) -> FramePlan {
    let cu = &f.cu;
    FramePlan {
        width: f.width,
        height: f.height,
        target: f.exclude_texture,
        render_origin: f.ro,
        view_proj: cu.view_proj,
        cam_pos: cu.cam_pos,
        sun_dir: cu.sun_dir,
        sun_color: cu.sun_color,
        ambient: cu.ambient,
        fog: cu.fog,
        sky_color: cu.sky_color,
        cam_right: cu.cam_right,
        cam_up: cu.cam_up,
        light_grid: cu.light_grid,
        light_map,
        clear: f.lighting.sky_color.extend(1.0).to_array(),
        rain: (f.lighting.rain - f.lighting.snowfall).clamp(0.0, 1.0),
        sky: cu.sky,
        clouds: [
            f.lighting.cloud_density,
            f.lighting.cloud_offset[0],
            f.lighting.cloud_offset[1],
            f.lighting.cloud_height,
        ],
        camera: bytemuck::cast_slice(std::slice::from_ref(cu)).to_vec(),
        list: lists.list.clone(),
        main: lists
            .main_batches
            .iter()
            .map(PlanBatch::from_batch)
            .collect(),
        cab: lists
            .cab_batches
            .iter()
            .map(PlanBatch::from_batch)
            .collect(),
        overlays: f.overlays.clone(),
    }
}

impl crate::scene::Scene {
    /// The sky dome's day, twilight and night pictures (`envir.cfg`'s `[sky_textures]`), once
    /// the game has set them.
    pub fn sky_textures(&self) -> Option<[TextureId; 3]> {
        self.sky_textures
    }

    /// The picture of the weather's cloud type for a renderer that draws the clouds itself
    /// (white, the clouds in its alpha), once the game has set it.
    pub fn sky_cloud_picture(&self) -> Option<TextureId> {
        self.sky_cloud_picture
    }

    /// The lights of the last frame as this renderer's shaders read them, four vectors each:
    /// xyz the position (render-origin relative) and the radius (0: a light only the
    /// enhanced path draws), rgb the colour and the intensity, the spot's direction and the
    /// cosine of its outer cone (below -1.5 a point light), then the cosine of the inner cone,
    /// the core radius, a headlamp's beam and the radius. The vehicles' interior lamps come
    /// first, at the indices their instances name.
    pub fn frame_lights(&self) -> std::borrow::Cow<'_, [[f32; 4]]> {
        match bytemuck::try_cast_slice(&self.last_lights) {
            Ok(v) => std::borrow::Cow::Borrowed(v),
            Err(_) => std::borrow::Cow::Owned(bytemuck::pod_collect_to_vec(&self.last_lights)),
        }
    }

    /// The last frame's lamp grid: per cell (row by row from the south-west, see
    /// [`FramePlan::light_grid`]) a fixed number of light indices, `u32::MAX` for none.
    pub fn frame_light_grid(&self) -> &[u32] {
        &self.last_grid
    }

    /// The last frame's light coronas (and the fog cones and halos of lights), the sprites
    /// the default renderer draws additively over the scene, 20 floats each in the order of
    /// its `corona.wgsl` vertex inputs: position (render-origin relative) and size; colour
    /// and brightness; direction and the outer cone's cosine; up axis and rotating mode;
    /// the inner cone's cosine, the z offset (or the fog's visibility), the parameter bits
    /// and the kind (0 a glow, 1 a cone in fog, 2 a halo). Only kept while an asset mirror is
    /// set. Sorted by picture; see [`Scene::frame_corona_runs`].
    pub fn frame_coronas(&self) -> &[[f32; 20]] {
        &self.last_coronas
    }

    /// The runs of [`Scene::frame_coronas`] by picture: (picture, first sprite, sprites,
    /// drawn after the vehicle the camera is in). Picture 0 is the standard glow, any other
    /// one that reached the mirror as [`MirrorEvent::CoronaTexture`].
    pub fn frame_corona_runs(&self) -> &[(u16, u32, u32, bool)] {
        &self.corona_runs
    }

    /// The last frame's smoke particles, farthest first (they are blended over each other):
    /// the sprites the default renderer draws with its smoke shader, in the same 20 floats as
    /// [`Scene::frame_coronas`] with the kind (the last) 3: position and size; colour and
    /// alpha; the cosine and sine of the particle's spin; the ground it fades into and the
    /// z offset towards the eye. Only OMSI's `[smoke]` and `[particle_emitter]` particles: the
    /// game's own tyre spray is left out. Empty in a mirror's picture when the options leave
    /// the smoke out of mirrors. Only kept while an asset mirror is set.
    pub fn frame_smoke(&self) -> &[[f32; 20]] {
        &self.last_smoke
    }
}

impl super::Renderer {
    /// Where the tile light map atlas lies relative to the render origin `ro` (x, y its
    /// south-west corner, z its side, w 1 when it is set).
    pub(crate) fn light_map_place(&self, ro: glam::DVec3) -> [f32; 4] {
        let (lx, ly, side) = self.light_map.lm_place.get();
        [
            (lx - ro.x) as f32,
            (ly - ro.y) as f32,
            side as f32,
            if side > 0.0 { 1.0 } else { 0.0 },
        ]
    }

    /// Hands `event` to the asset mirror, if there is one.
    pub(crate) fn mirror(&self, event: MirrorEvent<'_>) {
        if let Some(m) = self.asset_mirror.0.borrow_mut().as_mut() {
            m(event);
        }
    }

    pub(crate) fn mirroring(&self) -> bool {
        self.asset_mirror.0.borrow().is_some()
    }
}

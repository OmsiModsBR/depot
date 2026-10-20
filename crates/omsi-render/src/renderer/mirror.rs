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

/// What the window's picture draws this frame, in the order this renderer draws it.
///
/// Per-draw data: entry `e = list[i]` of an instance `i` of a batch has its model matrix
/// (column-major, relative to `render_origin`) at `Scene::slot_models()[e]` and two parameter
/// vectors at `Scene::slot_params()[2 e]` and `[2 e + 1]` (alpha, visible, uv offset; light,
/// night, interior lamp code, surface code).
#[derive(Debug, Clone)]
pub struct FramePlan {
    pub width: u32,
    pub height: u32,
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
    /// The sky's colour the picture is cleared to, as the lighting gives it (w 1).
    pub clear: [f32; 4],
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
) -> FramePlan {
    let cu = &f.cu;
    FramePlan {
        width: f.width,
        height: f.height,
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
        clear: f.lighting.sky_color.extend(1.0).to_array(),
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

impl super::Renderer {
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

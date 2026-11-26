//! A second renderer's view of the frame: the asset mirror (every mesh and texture as it goes
//! up, in CPU form) and the making of the frame plan (`interface::FramePlan`: what a picture
//! draws, after culling, level of detail and the alpha order). With both, another GPU backend can draw the scene this renderer keeps
//! the books for, while this one runs on a device that draws nothing (wgpu's noop backend).
//!
//! Nothing here changes what this renderer draws: without a mirror or a sink set, the hooks
//! are a branch on `None`.

use crate::interface::{FramePlan, PlanBatch};
use crate::scene::{MeshId, TextureId, Vertex};
use std::cell::RefCell;

/// A renderer's asset mirror. Counted in [`crate::assets`] while it is set, so prepared
/// assets keep their CPU copies; dropping the renderer stops it.
#[derive(Default)]
pub struct MirrorSlot(RefCell<Option<AssetMirror>>);

impl MirrorSlot {
    pub fn set(&self, mirror: Option<AssetMirror>) {
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

/// The plan of the picture `f` with the lists `lists`.
pub fn plan_of(
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
        // (the weather's own ambient light: the moonlight Vanilla+ adds is not OMSI 2's)
        ambient: f
            .lighting
            .ambient
            .extend(f.lighting.snow.clamp(0.0, 1.0))
            .to_array(),
        fog: cu.fog,
        sky_color: cu.sky_color,
        cam_right: cu.cam_right,
        cam_up: cu.cam_up,
        world_origin: cu.world_origin,
        wetness: cu.shadow[3],
        rain_reflection: None,
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
        sun_glow: f.lighting.sun_glow,
        moon: cu.moon,
        stars: cu.stars,
        lamp_cap: f.lighting.lamp_cap,
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

    /// The sun's and the moon's pictures the classic sky draws (`Texture\sonne.bmp`,
    /// `Texture\mond.bmp`), once the game has set them.
    pub fn sky_bodies(&self) -> (Option<TextureId>, Option<TextureId>) {
        (self.sky_sun, self.sky_moon)
    }

    /// The star map the skies draw the stars from (`omsi_content::StarMap`) and its cells
    /// across a face, once the game has set it.
    pub fn sky_stars(&self) -> Option<(TextureId, u32)> {
        self.sky_stars.map(|t| (t, self.star_cells))
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
    pub fn light_map_place(&self, ro: glam::DVec3) -> [f32; 4] {
        let (lx, ly, side) = self.light_map.lm_place.get();
        [
            (lx - ro.x) as f32,
            (ly - ro.y) as f32,
            side as f32,
            if side > 0.0 { 1.0 } else { 0.0 },
        ]
    }

    /// Hands `event` to the asset mirror, if there is one.
    pub fn mirror(&self, event: MirrorEvent<'_>) {
        if let Some(m) = self.asset_mirror.0.borrow_mut().as_mut() {
            m(event);
        }
    }

    pub fn mirroring(&self) -> bool {
        self.asset_mirror.0.borrow().is_some()
    }
}

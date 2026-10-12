//! Render targets of one size: the enhanced path's HDR chain, the reflection probe and
//! sky cube, and the ambient-occlusion textures.

use super::*;

/// High-range colour targets of the enhanced path for one size: the multisampled one the
/// scene is drawn into, the single-sampled one it resolves into, the glow's levels, the
/// tone-mapped picture FXAA reads, and the bind groups of the passes between them.
pub(crate) struct HdrTargets {
    pub(crate) msaa_view: Option<wgpu::TextureView>,
    pub(crate) view: wgpu::TextureView,
    /// The screen mask (`MASK_FORMAT`), multisampled and resolved like the picture.
    pub(crate) mask_msaa: Option<wgpu::TextureView>,
    pub(crate) mask: wgpu::TextureView,
    /// Enhanced+: the reflections' surfaces (`GBUF_FORMAT`, `AUX_FORMAT`): (multisampled,
    /// resolved) each.
    pub(crate) gbuf: Option<[(Option<wgpu::TextureView>, wgpu::TextureView); 2]>,
    /// Glow levels at 1/2, 1/4, ... of the size, and the upsampled sums per level.
    pub(crate) down: Vec<wgpu::TextureView>,
    pub(crate) up: Vec<wgpu::TextureView>,
    pub(crate) ldr: wgpu::TextureView,
    /// down[i] reads the picture (i = 0) or down[i - 1]; up[i] reads up[i + 1] (or the
    /// last down level) and down[i].
    pub(crate) down_bg: Vec<wgpu::BindGroup>,
    pub(crate) up_bg: Vec<wgpu::BindGroup>,
    pub(crate) meter_bg: wgpu::BindGroup,
    /// Tone mapping with the adapted exposure in `adapt[k]`.
    pub(crate) tonemap_bg: [wgpu::BindGroup; 2],
    pub(crate) fxaa_bg: wgpu::BindGroup,
    /// Classic shading is presented directly, without the Enhanced tone curve.
    pub(crate) classic_bg: wgpu::BindGroup,
    /// Allocated only when wet roads need scene reflections in the main view.
    pub(crate) puddles: Option<puddles::Targets>,
}

/// The enhanced path's reflection probe: a cube map of the sky around the camera with a
/// GGX-blurred mip chain, drawn now and then.
pub(crate) struct Probe {
    pub(crate) view: wgpu::TextureView,
    /// Per mip level, per face: the view the level is drawn into.
    pub(crate) faces: Vec<Vec<wgpu::TextureView>>,
    /// Per mip level, the two passes' bind groups (faces 0-2 and 3-5).
    pub(crate) bind_groups: Vec<[wgpu::BindGroup; 2]>,
    pub(crate) sky_pipeline: wgpu::RenderPipeline,
    pub(crate) filter_pipeline: wgpu::RenderPipeline,
    /// Frames since it was drawn, and the sky table scale it was drawn with.
    pub(crate) age: u32,
    pub(crate) scale: f32,
    /// The enhanced sky itself at `SKY_CUBE_SIZE` a face (clouds and all), drawn a face a
    /// frame; the dome reads it (camera binding 17). The volumetric clouds cost 6-18 ms a
    /// frame drawn for every pixel of a 1440p sky; a face a frame is well under one.
    pub(crate) cube_view: wgpu::TextureView,
    pub(crate) cube_faces: Vec<wgpu::TextureView>,
    pub(crate) cube_bind_groups: Vec<wgpu::BindGroup>,
    pub(crate) cube_pipeline: wgpu::RenderPipeline,
    pub(crate) cube_next: u32,
    pub(crate) cube_filled: bool,
    /// Redraws so far (picks the round of the next).
    pub(crate) cube_round: u32,
    /// Window frames since the last face was redrawn (see SKY_CUBE_EVERY).
    pub(crate) cube_wait: u32,
    /// Where every face of the cube is drawn from (world). A face drawn from wherever the
    /// camera stood at the time left six faces from six places: flying up towards the
    /// clouds, the faces disagreed at their seams and the blended history dragged each one
    /// behind the camera, so the clouds shook and lagged. The cube keeps one eye; the dome
    /// takes the parallax out, and the eye moves on (the whole cube redrawn) once the
    /// camera has gone far enough for the correction to show.
    pub(crate) cube_eye: Option<DVec3>,
    pub(crate) cube_recapture: bool,
}

/// Face size of the enhanced sky cube (see `Probe::cube_view`): about as many texels per
/// degree as a 1600-pixel-wide picture has pixels at half its size (at 512 the clouds'
/// edges stood in blocks of three or four pixels).
pub(crate) const SKY_CUBE_SIZE: u32 = 1024;

/// Redraw rounds of a sky cube face: each starts the clouds' steps elsewhere, and the
/// rounds are averaged (all of them at once for a picture on its own or a new sky).
pub(crate) const SKY_CUBE_ROUNDS: u32 = 8;

/// The old picture's share when a face is redrawn in the window.
pub(crate) const SKY_CUBE_HISTORY: f64 = 0.8;

/// A face is redrawn every this many window frames: the clouds drift by a pixel of the
/// cube in seconds, and a face a frame (a 32-step march of 1024x1024 texels) cost 2.9 ms of
/// every enhanced frame on an M4.
pub(crate) const SKY_CUBE_EVERY: u32 = 4;

pub(crate) const PROBE_SIZE: u32 = 64;

pub(crate) const PROBE_MIPS: u32 = 6;

/// Glow levels of the enhanced post path.
pub(crate) const GLOW_LEVELS: usize = 6;

/// The textures of the ambient-occlusion pass for one target size.
pub(crate) struct AoTargets {
    pub(crate) size: (u32, u32),
    pub(crate) depth_view: wgpu::TextureView,
    pub(crate) ao_view: wgpu::TextureView,
    pub(crate) blur_view: wgpu::TextureView,
    pub(crate) ssao_bg: wgpu::BindGroup,
    pub(crate) blur_bg: wgpu::BindGroup,
    /// The lamps' light in the fog (`fog_lamps.wgsl`): worked out at half size into
    /// `fog_view`, then added onto the picture (bind groups of the two passes).
    pub(crate) fog_view: wgpu::TextureView,
    pub(crate) fog_bg: Option<[wgpu::BindGroup; 2]>,
}

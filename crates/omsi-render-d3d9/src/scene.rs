//! The game's scene as this renderer draws it: meshes and textures kept under the ids the
//! game gives them, and a frame made of batches over a per-draw list, with the camera and the
//! light. Nothing here names a graphics API; the game maps its own frame plan onto these
//! types, and the Direct3D 9Ex device executes them (`D3d9Device::draw_scene`).
//!
//! The layouts follow the default renderer's scene shader, so the two draw from the same
//! values: a vertex is position, normal and uv; a material is its uniform of
//! [`MATERIAL_VECTORS`] vectors; each draw has a model matrix and two parameter vectors.

use std::ops::Range;

use crate::backend::VERTEX_FLOATS;
use crate::{BackendError, Result};

/// The vectors of a material's uniform, in order: color, params, extra, params2, emissive,
/// specular, bump, pbr, flags, ambient, sway (the default renderer's `MaterialParams`).
pub const MATERIAL_VECTORS: usize = 11;

/// Floats of one draw's instance record: the model matrix's four columns, then its two
/// parameter vectors.
pub const INSTANCE_FLOATS: usize = 24;

/// Where the alpha-tested materials cut: a texel is kept when its alpha is above this. The
/// same cut as the default renderer's scene shader (a texel of alpha 128 is thrown away), so
/// both renderers agree. PROVISIONAL, like [`crate::backend::AlphaMode::Test`]'s reference:
/// a parity item still to be observed in the original.
pub const ALPHA_CUT: f32 = 128.5 / 255.0;

/// A texture's pixel format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexFormat {
    /// 8-bit RGBA, 4 bytes a texel, colour as stored in the file (gamma-encoded).
    Rgba8,
    /// DXT1: 8 bytes per 4x4 block.
    Bc1,
    /// DXT3: 16 bytes per block.
    Bc2,
    /// DXT5: 16 bytes per block.
    Bc3,
}

impl TexFormat {
    pub fn is_compressed(self) -> bool {
        self != TexFormat::Rgba8
    }

    /// Bytes of one level of `w` x `h` texels.
    pub fn level_bytes(self, w: u32, h: u32) -> usize {
        let blocks = |n: u32| n.div_ceil(4) as usize;
        match self {
            TexFormat::Rgba8 => w as usize * h as usize * 4,
            TexFormat::Bc1 => blocks(w) * blocks(h) * 8,
            TexFormat::Bc2 | TexFormat::Bc3 => blocks(w) * blocks(h) * 16,
        }
    }

    /// Bytes of one row of texels (of blocks, when compressed) and the number of rows.
    pub fn rows(self, w: u32, h: u32) -> (usize, usize) {
        match self {
            TexFormat::Rgba8 => (w as usize * 4, h as usize),
            _ => (self.level_bytes(w, 4), h.div_ceil(4) as usize),
        }
    }
}

/// Whether a DXT1 block has a transparent texel: three-colour mode with index 3 in use.
fn bc1_block_is_clear(block: &[u8]) -> bool {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    c0 <= c1 && (0..16).any(|i| (bits >> (2 * i)) & 3 == 3)
}

/// Whether a DXT5 block has a texel whose interpolated alpha is below 255.
fn bc3_block_is_clear(block: &[u8]) -> bool {
    let (a0, a1) = (u32::from(block[0]), u32::from(block[1]));
    let bits = block[2..8]
        .iter()
        .enumerate()
        .fold(0u64, |bits, (i, &b)| bits | u64::from(b) << (8 * i));
    let value = |i: u32| match (i, a0 > a1) {
        (0, _) => a0,
        (1, _) => a1,
        (i, true) => ((8 - i) * a0 + (i - 1) * a1) / 7,
        (6, false) => 0,
        (7, false) => 255,
        (i, false) => ((6 - i) * a0 + (i - 1) * a1) / 5,
    };
    (0..16).any(|t| value(((bits >> (3 * t)) & 7) as u32) < 255)
}

/// The size of level `level` of a `w` x `h` texture.
pub fn level_size(w: u32, h: u32, level: usize) -> (u32, u32) {
    ((w >> level).max(1), (h >> level).max(1))
}

/// How many levels a full mip chain of `w` x `h` has (down to 1 x 1).
pub fn mip_count(w: u32, h: u32) -> usize {
    (32 - w.max(h).max(1).leading_zeros()) as usize
}

/// A texture as the game hands it over: level 0 first.
#[derive(Debug, Clone, Copy)]
pub struct SceneTexture<'a> {
    pub width: u32,
    pub height: u32,
    pub format: TexFormat,
    pub levels: &'a [Vec<u8>],
    /// A single RGBA level gets its mip chain made here ([`mip_chain`]).
    pub make_mips: bool,
}

/// The surface code (the second instance vector's w) of a vehicle's shadow blob.
pub const BLOB_SURFACE_CODE: f32 = 2.0;

impl SceneTexture<'_> {
    /// Whether the finest level has a texel that is less than fully opaque. A shadow blob
    /// whose texture has none is a solid quad: it is drawn black all over the road.
    pub fn alpha_in_use(&self) -> bool {
        let Some(top) = self.levels.first() else {
            return false;
        };
        match self.format {
            TexFormat::Rgba8 => top.chunks_exact(4).any(|p| p[3] < 255),
            TexFormat::Bc1 => top.chunks_exact(8).any(bc1_block_is_clear),
            TexFormat::Bc2 => top
                .chunks_exact(16)
                .any(|b| b[..8].iter().any(|&a| a != 0xff)),
            TexFormat::Bc3 => top.chunks_exact(16).any(bc3_block_is_clear),
        }
    }

    /// The checks before creating the texture: every level has the size its format and
    /// dimensions give, and the texture fits the device.
    pub fn validate(&self, max_size: u32, non_pow2: bool) -> Result<()> {
        if self.width == 0 || self.height == 0 || self.levels.is_empty() {
            return Err(BackendError::InvalidArgument("empty texture".into()));
        }
        if self.levels.len() > mip_count(self.width, self.height) {
            return Err(BackendError::InvalidArgument(
                "more levels than the size has".into(),
            ));
        }
        for (i, level) in self.levels.iter().enumerate() {
            let (w, h) = level_size(self.width, self.height, i);
            if level.len() != self.format.level_bytes(w, h) {
                return Err(BackendError::InvalidArgument(format!(
                    "level {i} has {} bytes, {w}x{h} {:?} needs {}",
                    level.len(),
                    self.format,
                    self.format.level_bytes(w, h)
                )));
            }
        }
        if self.width > max_size || self.height > max_size {
            return Err(BackendError::Unsupported(format!(
                "texture larger than {max_size}"
            )));
        }
        if !non_pow2 && !(self.width.is_power_of_two() && self.height.is_power_of_two()) {
            return Err(BackendError::Unsupported(
                "texture size is not a power of two".into(),
            ));
        }
        Ok(())
    }

    /// Whether [`mip_chain`] applies to this texture.
    pub fn wants_mips(&self) -> bool {
        self.make_mips && self.format == TexFormat::Rgba8 && self.levels.len() == 1
    }

    /// This texture brought within `max_size` texels a side, as its first level's size and
    /// its levels: a mip chain leaves its top levels out, a single RGBA image is halved until
    /// it fits. `None` when it fits already or cannot be made smaller (a compressed texture
    /// without the levels below). The levels must be valid ([`SceneTexture::validate`]).
    pub fn fitted(&self, max_size: u32) -> Option<(u32, u32, Vec<Vec<u8>>)> {
        let fits = |(w, h): (u32, u32)| w.max(h) <= max_size;
        if fits((self.width, self.height)) {
            return None;
        }
        let first = (1..mip_count(self.width, self.height))
            .find(|&i| fits(level_size(self.width, self.height, i)))?;
        let (w, h) = level_size(self.width, self.height, first);
        if first < self.levels.len() {
            return Some((w, h, self.levels[first..].to_vec()));
        }
        if self.format != TexFormat::Rgba8 || self.levels.len() != 1 {
            return None;
        }
        let mut chain = mip_chain(self.width, self.height, &self.levels[0]);
        Some((w, h, vec![chain.swap_remove(first - 1)]))
    }
}

/// The levels below a `w` x `h` RGBA image, each the 2 x 2 box average of the one above
/// (an odd edge repeats its last texel), down to 1 x 1. The colour is weighted by alpha, as
/// the default renderer's mip generator does it.
pub fn mip_chain(w: u32, h: u32, rgba: &[u8]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let (mut pw, mut ph) = (w as usize, h as usize);
    while pw > 1 || ph > 1 {
        let prev: &[u8] = out.last().map_or(rgba, |v| v.as_slice());
        let (nw, nh) = ((pw / 2).max(1), (ph / 2).max(1));
        let mut next = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            let (y0, y1) = ((y * 2).min(ph - 1), (y * 2 + 1).min(ph - 1));
            for x in 0..nw {
                let (x0, x1) = ((x * 2).min(pw - 1), (x * 2 + 1).min(pw - 1));
                let at = |xx: usize, yy: usize, c: usize| prev[(yy * pw + xx) * 4 + c] as u32;
                let texels = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)];
                let alpha: u32 = texels.iter().map(|&(xx, yy)| at(xx, yy, 3)).sum();
                let o = (y * nw + x) * 4;
                for c in 0..3 {
                    // weighted by alpha: the colour stored in transparent texels must not
                    // bleed into the silhouette (the default renderer's mip.wgsl does this)
                    let weighted: u32 = texels
                        .iter()
                        .map(|&(xx, yy)| at(xx, yy, c) * at(xx, yy, 3))
                        .sum();
                    next[o + c] = match (weighted + alpha / 2).checked_div(alpha) {
                        Some(v) => v as u8,
                        // all four transparent: the plain average (alpha is often only a mask)
                        None => {
                            let sum: u32 = texels.iter().map(|&(xx, yy)| at(xx, yy, c)).sum();
                            ((sum + 2) / 4) as u8
                        }
                    };
                }
                next[o + 3] = ((alpha + 2) / 4) as u8;
            }
        }
        out.push(next);
        (pw, ph) = (nw, nh);
    }
    out
}

/// The checks before creating a mesh: 8 floats per vertex, whole triangles, indices in range.
pub fn validate_mesh(vertices: &[f32], indices: &[u32], index32: bool) -> Result<()> {
    if vertices.is_empty() || !vertices.len().is_multiple_of(VERTEX_FLOATS) {
        return Err(BackendError::InvalidArgument(
            "vertex data is not 8 floats per vertex".into(),
        ));
    }
    let count = (vertices.len() / VERTEX_FLOATS) as u32;
    if indices.is_empty() || !indices.len().is_multiple_of(3) || indices.iter().any(|&i| i >= count)
    {
        return Err(BackendError::InvalidArgument("indices".into()));
    }
    if !index32 && count > 0x1_0000 {
        return Err(BackendError::Unsupported("32-bit indices".into()));
    }
    Ok(())
}

/// How a batch is drawn (the default renderer's pipeline kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawKind {
    Opaque,
    /// Cut at [`ALPHA_CUT`].
    AlphaTest,
    /// Blended, writing depth.
    Blend,
    /// Blended without depth write (`[matl_noZwrite]`).
    BlendNoWrite,
    /// Depth only, of the parts that are fully opaque.
    SurfaceDepth,
    /// Painted ground layers: blended over the ground, without depth write.
    TerrainPaint,
}

impl DrawKind {
    pub fn blends(self) -> bool {
        matches!(
            self,
            DrawKind::Blend | DrawKind::BlendNoWrite | DrawKind::TerrainPaint
        )
    }

    pub fn writes_depth(self) -> bool {
        !matches!(self, DrawKind::BlendNoWrite | DrawKind::TerrainPaint)
    }

    pub fn writes_color(self) -> bool {
        self != DrawKind::SurfaceDepth
    }
}

/// Draws of one mesh range with one material, for a run of the frame's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneBatch {
    pub kind: DrawKind,
    /// Back faces (counter-clockwise on screen) are culled.
    pub cull: bool,
    /// A road surface: drawn with the surface depth bias.
    pub surface: bool,
    pub mesh: usize,
    /// The index range, in the mesh's own indices.
    pub first: u32,
    pub count: u32,
    pub material: usize,
    /// The run of [`SceneFrame::list`] drawn by this batch, one instance per entry.
    pub instances: Range<u32>,
}

/// How a material's textures are addressed (`[matl_texadress_*]`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TexAddress {
    /// Repeating.
    #[default]
    Wrap,
    /// Every other repeat mirrored.
    Mirror,
    /// Clamped at the edge (and `[matl_texadress_border]`, whose colour the shader puts
    /// outside).
    Clamp,
    /// Mirrored once about 0, then clamped (the shader takes the absolute value).
    MirrorOnce,
}

/// What a material is drawn with. A map the uniform says it has but that is not uploaded
/// reads as nothing: black for the night and light maps and the sphere map, white for the
/// diffuse texture, the transmap and the reflection mask, flat for the bump map.
#[derive(Debug, Clone, Copy, Default)]
pub struct SceneMaterial<'a> {
    /// The diffuse texture.
    pub texture: Option<usize>,
    /// `[matl_transmap]` (its alpha replaces the diffuse alpha when the uniform says so).
    pub transmap: Option<usize>,
    /// `[matl_nightmap]`: added at night (or always, for a switched one).
    pub nightmap: Option<usize>,
    /// `[matl_lightmap]` (the terrain's detail texture on a terrain material).
    pub lightmap: Option<usize>,
    /// `[matl_envmap]`: the sphere map reflected on it, by the factor in the uniform.
    pub envmap: Option<usize>,
    /// `[matl_envmap_mask]`: its alpha masks the reflection.
    pub env_mask: Option<usize>,
    /// `[matl_bumpmap]`: a height map (alpha) whose slope moves the sphere-map lookup.
    pub bump: Option<usize>,
    /// `[matl_raindropmap]`: the areas of the pane where raindrops can gather (see
    /// [`crate::glass_rain`]).
    pub raindrop_map: Option<usize>,
    pub address: TexAddress,
    /// [`MATERIAL_VECTORS`] vectors.
    pub uniform: &'a [[f32; 4]],
}

/// The camera and the light, as the default renderer's camera uniform carries them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SceneLight {
    /// xyz the camera, relative to the render origin.
    pub cam_pos: [f32; 4],
    /// xyz towards the sun, w its intensity.
    pub sun_dir: [f32; 4],
    /// rgb the sun's light, w the night factor.
    pub sun_color: [f32; 4],
    /// rgb the ambient light.
    pub ambient: [f32; 4],
    /// rgb the fog's colour, w its density.
    pub fog: [f32; 4],
    /// rgb the light from above, w 1 for the vanilla picture.
    pub sky_color: [f32; 4],
    /// The camera's right and up axes (xyz), which lay the sphere maps out.
    pub cam_right: [f32; 4],
    pub cam_up: [f32; 4],
}

/// The `[matl_envmap]` factor x 255 x its light (each channel at most 1), truncated, and the
/// three put together as one packed colour without saturation: a factor over 1 spills into
/// the next channel as Direct3D's texture factor does (a factor of 10 at full light reads
/// almost white). The scene shader works it out the same way per pixel (`texture_factor`),
/// the light there taking the lamps in; this is its reference.
pub fn env_factor(factor: f32, light: [f32; 3]) -> [f32; 3] {
    let m = light.map(|l| (l.clamp(0.0, 1.0) * factor.max(0.0) * 255.0) as u32);
    let packed = (0xffu32 << 24) | m[0].wrapping_shl(16) | m[1].wrapping_shl(8) | m[2];
    [packed >> 16, packed >> 8, packed].map(|c| (c & 0xff) as f32 / 255.0)
}

/// The `[matl_texadress_border]` colour of a material's flags vector (uniform vector 8: y 1
/// for a border, z the packed rgb, w the alpha), unpacked as the scene shader takes it;
/// zero for a material without a border.
pub fn border_rgba(flags: [f32; 4]) -> [f32; 4] {
    if !(0.5..1.5).contains(&flags[1]) {
        return [0.0; 4];
    }
    let p = (flags[2] + 0.5).floor().clamp(0.0, 16_777_215.0) as u32;
    let [r, g, b] = [p >> 16, p >> 8, p].map(|c| (c & 0xff) as f32 / 255.0);
    [r, g, b, flags[3]]
}

/// What the scene shader takes of a material's `[matl_envmap]` bits (uniform vector 3, w):
/// x whether it has a mask, y whether a transmap is declared, as 1 or 0.
pub fn env_bits(params2_w: f32) -> [f32; 2] {
    let bits = (params2_w + 0.5).floor().max(0.0) as u32;
    [(bits & 1) as f32, ((bits >> 1) & 1) as f32]
}

/// A light of [`SceneLamps::lights`] (its four vectors) as the scene shader of the D3D9
/// device reads it, with what does not depend on the pixel worked out once:
/// * xyz the position, w 1 for a point light that lights every surface (dir.x 1);
/// * rgb the colour times the intensity, w 1 / the radius (0 for a light passed by);
/// * the spot's direction and outer cone, as given;
/// * the inner cone; the interior lamp's core radius squared (at least 1 cm); the radius
///   of full light squared (an eighth of the range; 0 for a light passed by); the interior
///   lamp's reach squared (0 when its intensity is not above 0, which lights nothing).
pub fn pack_light(light: &[[f32; 4]]) -> [[f32; 4]; 4] {
    let [pos, color, dir, extra] = [light[0], light[1], light[2], light[3]];
    let radius = pos[3];
    let every_surface = dir[3] < -1.5 && dir[0] > 0.5;
    let w = color[3];
    let (inv_radius, full) = if radius > 0.0 {
        (1.0 / radius, (radius * 0.125).powi(2))
    } else {
        (0.0, 0.0)
    };
    let core = extra[1].max(0.01);
    let reach = if w > 0.0 { extra[3] * extra[3] } else { 0.0 };
    [
        [pos[0], pos[1], pos[2], f32::from(u8::from(every_surface))],
        [color[0] * w, color[1] * w, color[2] * w, inv_radius],
        dir,
        [extra[0], core * core, full, reach],
    ]
}

/// The lamps of a picture, as the default renderer hands them to its shaders: the lights,
/// and the grid of cells round the camera that names the lights reaching each cell.
#[derive(Debug, Clone, Copy, Default)]
pub struct SceneLamps<'a> {
    /// Four vectors a light: xyz the position (render-origin relative), w the radius (0: a
    /// light this picture passes by); rgb the colour, w the intensity; the spot's direction
    /// and the cosine of its outer cone (below -1.5 a point light; x 1 on a point light that
    /// lights every surface); the cosine of the inner cone, the core radius, the beam and the
    /// radius the vehicles' interior lamps reach. The interior lamps come first, at the
    /// indices the instances' lamp codes name.
    pub lights: &'a [[f32; 4]],
    /// Per cell, row by row from the south-west, the same number of light indices,
    /// `u32::MAX` for an empty place (the rest of the cell's places are empty too).
    pub grid: &'a [u32],
    /// x, y the grid's south-west corner (render-origin relative), z a cell's side, w the
    /// cells per side.
    pub place: [f32; 4],
    /// Where the tile light map atlas ([`crate::d3d9::D3d9Device::set_tile_light_map`]) lies:
    /// x, y its south-west corner (render-origin relative), z its side, w 1 when it is set.
    pub tile_map: [f32; 4],
}

impl SceneLamps<'_> {
    /// The lights in `lights`.
    pub fn count(&self) -> usize {
        self.lights.len() / 4
    }

    /// The cells per side and the places per cell, when there is a grid.
    pub fn grid_shape(&self) -> Option<(usize, usize)> {
        let side = self.place[3].max(0.0) as usize;
        let cells = side.checked_mul(side)?;
        (cells > 0 && self.place[2] > 0.0 && !self.grid.is_empty())
            .then(|| (side, self.grid.len() / cells))
    }

    /// The lights come four vectors each and the grid has as many places in every cell.
    pub fn validate(&self) -> Result<()> {
        if !self.lights.len().is_multiple_of(4) {
            return Err(BackendError::InvalidArgument(format!(
                "{} light vectors, not four a light",
                self.lights.len()
            )));
        }
        if let Some((side, places)) = self.grid_shape() {
            if places == 0 || side * side * places != self.grid.len() {
                return Err(BackendError::InvalidArgument(format!(
                    "a grid of {} places for {side} x {side} cells",
                    self.grid.len()
                )));
            }
        }
        Ok(())
    }

    /// The grid as the shader reads it: each place's light index, -1 where it is empty or
    /// names a light past the first `kept`.
    pub fn grid_texels(&self, kept: usize, out: &mut Vec<f32>) {
        out.clear();
        out.extend(
            self.grid
                .iter()
                .map(|&i| if (i as usize) < kept { i as f32 } else { -1.0 }),
        );
    }
}

/// Floats a corona sprite takes: position and size; colour and brightness; direction and
/// the outer cone's cosine; the up axis and rotating mode; the inner cone's cosine, the z
/// offset, the parameter bits and the kind (0 a glow, 1 a cone in fog, 2 a halo).
pub const CORONA_FLOATS: usize = 20;

/// The light coronas of a picture, as the default renderer hands them to its corona shader
/// (`Scene::frame_coronas` and `Scene::frame_corona_runs` of `omsi-render`): camera-facing
/// sprites drawn additively over the scene, grouped in runs by picture.
#[derive(Debug, Clone, Copy, Default)]
pub struct SceneCoronas<'a> {
    pub sprites: &'a [[f32; CORONA_FLOATS]],
    /// (picture, first sprite, sprites, drawn after the vehicle the camera is in). Picture 0
    /// is the standard glow; any other was put up with `set_corona_texture`.
    pub runs: &'a [(u16, u32, u32, bool)],
}

impl SceneCoronas<'_> {
    /// Checks that every run lies in the sprite list.
    pub fn validate(&self) -> Result<()> {
        let len = self.sprites.len() as u64;
        match self
            .runs
            .iter()
            .find(|r| u64::from(r.1) + u64::from(r.2) > len)
        {
            Some(r) => Err(BackendError::InvalidArgument(format!(
                "corona run {}+{} outside a list of {len}",
                r.1, r.2
            ))),
            None => Ok(()),
        }
    }
}

/// One picture of the scene.
#[derive(Debug, Clone, Copy)]
pub struct SceneFrame<'a> {
    /// Column-major, from render-origin-relative world to clip space. Depth is reversed: the
    /// near plane is 1 and the far plane 0.
    pub view_proj: [[f32; 4]; 4],
    pub light: SceneLight,
    /// The per-draw entry of every drawn instance; indexes `models` (and two each `params`).
    pub list: &'a [u32],
    pub models: &'a [[[f32; 4]; 4]],
    /// x alpha multiplier, y visible, zw uv offset; then the second vector (w the surface code).
    pub params: &'a [[f32; 4]],
    /// The main pass, then the vehicle the camera is in, drawn after it.
    pub main: &'a [SceneBatch],
    pub cab: &'a [SceneBatch],
    /// The road surfaces' depth bias (the default renderer's, negative pulls towards the eye).
    pub surface_bias: i32,
    pub lamps: SceneLamps<'a>,
    /// The light coronas drawn over the scene.
    pub coronas: SceneCoronas<'a>,
    /// The rain and the wiper, for the raindrops on glass.
    pub rain: crate::glass_rain::SceneRain,
}

impl SceneFrame<'_> {
    /// Checks that every batch's run lies in the list and every entry has its model and
    /// parameters.
    pub fn validate(&self) -> Result<()> {
        let len = self.list.len() as u32;
        if let Some(b) = self
            .main
            .iter()
            .chain(self.cab)
            .find(|b| b.instances.start > b.instances.end || b.instances.end > len)
        {
            return Err(BackendError::InvalidArgument(format!(
                "batch instances {:?} outside a list of {len}",
                b.instances
            )));
        }
        if let Some(&e) = self
            .list
            .iter()
            .find(|&&e| e as usize >= self.models.len() || e as usize * 2 + 1 >= self.params.len())
        {
            return Err(BackendError::InvalidArgument(format!(
                "list entry {e} without its model or parameters"
            )));
        }
        self.coronas.validate()?;
        self.lamps.validate()
    }

    /// Appends one instance record ([`INSTANCE_FLOATS`]) per list entry, in list order: a
    /// batch's instances are then the records of its run.
    pub fn instance_records(&self, out: &mut Vec<f32>) {
        out.reserve(self.list.len() * INSTANCE_FLOATS);
        for &e in self.list {
            let e = e as usize;
            for column in &self.models[e] {
                out.extend_from_slice(column);
            }
            out.extend_from_slice(&self.params[e * 2]);
            out.extend_from_slice(&self.params[e * 2 + 1]);
        }
    }
}

/// The surface depth bias as Direct3D 9 takes it on a 24-bit depth buffer: the constant part
/// in depth units and the slope factor. The sign is turned with the reversed depth, as the
/// default renderer turns it.
pub fn surface_depth_bias(bias: i32) -> (f32, f32) {
    if bias == 0 {
        return (0.0, 0.0);
    }
    (-bias as f32 / 16_777_216.0, -(bias.signum() as f32) * 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_texture_too_large_is_brought_within_the_limit() {
        // a chain leaves its top levels out
        let chain: Vec<Vec<u8>> = (0..mip_count(8, 4))
            .map(|i| {
                let (w, h) = level_size(8, 4, i);
                vec![i as u8; TexFormat::Bc1.level_bytes(w, h)]
            })
            .collect();
        let mut t = SceneTexture {
            width: 8,
            height: 4,
            format: TexFormat::Bc1,
            levels: &chain,
            make_mips: false,
        };
        assert!(t.fitted(8).is_none());
        let (w, h, levels) = t.fitted(2).unwrap();
        assert_eq!((w, h, levels.len()), (2, 1, 2));
        assert_eq!(levels[0][0], 2);
        // a single compressed level cannot be made smaller
        t.levels = &chain[..1];
        assert!(t.fitted(4).is_none());
        // a single RGBA image is halved: white and black columns to grey
        let image = vec![[[255, 255, 255, 255], [0, 0, 0, 255]].concat().repeat(2)];
        let t = SceneTexture {
            width: 2,
            height: 2,
            format: TexFormat::Rgba8,
            levels: &image,
            make_mips: true,
        };
        let (w, h, levels) = t.fitted(1).unwrap();
        assert_eq!((w, h), (1, 1));
        assert_eq!(levels, vec![vec![128, 128, 128, 255]]);
    }

    #[test]
    fn level_sizes_follow_the_format() {
        assert_eq!(TexFormat::Rgba8.level_bytes(3, 2), 24);
        assert_eq!(TexFormat::Bc1.level_bytes(8, 8), 32);
        assert_eq!(TexFormat::Bc1.level_bytes(1, 1), 8);
        assert_eq!(TexFormat::Bc3.level_bytes(6, 4), 32);
        assert_eq!(TexFormat::Bc3.rows(8, 8), (32, 2));
        assert_eq!(TexFormat::Rgba8.rows(3, 2), (12, 2));
        assert_eq!(mip_count(1, 1), 1);
        assert_eq!(mip_count(256, 64), 9);
        assert_eq!(level_size(256, 64, 7), (2, 1));
    }

    #[test]
    fn textures_are_checked_level_by_level() {
        let levels = vec![vec![0u8; 64], vec![0u8; 16], vec![0u8; 4]];
        let t = SceneTexture {
            width: 4,
            height: 4,
            format: TexFormat::Rgba8,
            levels: &levels,
            make_mips: false,
        };
        t.validate(2048, true).unwrap();
        let short = vec![vec![0u8; 64], vec![0u8; 15]];
        let bad = SceneTexture {
            levels: &short,
            ..t
        };
        assert!(matches!(
            bad.validate(2048, true),
            Err(BackendError::InvalidArgument(_))
        ));
        let one = vec![vec![0u8; 3 * 4 * 4]];
        let npot = SceneTexture {
            width: 3,
            levels: &one,
            ..t
        };
        assert!(matches!(
            npot.validate(2048, false),
            Err(BackendError::Unsupported(_))
        ));
        npot.validate(2048, true).unwrap();
    }

    #[test]
    fn the_mip_chain_averages_down_to_one_texel() {
        // 2x2: black, white, white, black -> one mid grey; then nothing more.
        let rgba = [
            [0, 0, 0, 255],
            [255, 255, 255, 255],
            [255, 255, 255, 255],
            [0, 0, 0, 255],
        ]
        .concat();
        let chain = mip_chain(2, 2, &rgba);
        assert_eq!(chain, vec![vec![128, 128, 128, 255]]);
        // 3x1 -> 1x1 averages the first two texels (the odd one is left at this level).
        let row = [[0, 0, 0, 0], [100, 100, 100, 100], [200, 200, 200, 200]].concat();
        let chain = mip_chain(3, 1, &row);
        assert_eq!(chain.len(), 1);
        // (the colour is weighted by alpha: the transparent texel's black does not count)
        assert_eq!(chain[0], vec![100, 100, 100, 50]);
        // the chain of 8x2 has 8x2 -> 4x1 -> 2x1 -> 1x1 below level 0
        let chain = mip_chain(8, 2, &[7u8; 8 * 2 * 4]);
        let sizes: Vec<usize> = chain.iter().map(|l| l.len() / 4).collect();
        assert_eq!(sizes, vec![4, 2, 1]);
        assert!(chain.iter().flatten().all(|&c| c == 7));
    }

    #[test]
    fn the_mip_chain_keeps_transparent_colour_out_of_the_silhouette() {
        // a green leaf texel beside three blue fully transparent ones stays green
        let rgba = [
            [0, 200, 0, 255],
            [0, 0, 255, 0],
            [0, 0, 255, 0],
            [0, 0, 255, 0],
        ]
        .concat();
        assert_eq!(mip_chain(2, 2, &rgba), vec![vec![0, 200, 0, 64]]);
        // all four transparent: the plain average, so a mask in alpha does not blacken colour
        let rgba = [[40, 80, 120, 0]; 4].concat();
        assert_eq!(mip_chain(2, 2, &rgba), vec![vec![40, 80, 120, 0]]);
    }

    #[test]
    fn meshes_are_checked() {
        let v = vec![0.0f32; 3 * VERTEX_FLOATS];
        validate_mesh(&v, &[0, 1, 2], false).unwrap();
        assert!(validate_mesh(&v, &[0, 1, 3], false).is_err());
        assert!(validate_mesh(&v, &[0, 1], false).is_err());
        assert!(validate_mesh(&v[..7], &[0, 0, 0], false).is_err());
    }

    fn batch(instances: Range<u32>) -> SceneBatch {
        SceneBatch {
            kind: DrawKind::Opaque,
            cull: false,
            surface: false,
            mesh: 0,
            first: 0,
            count: 3,
            material: 0,
            instances,
        }
    }

    #[test]
    fn the_reflection_factor_spills_as_a_packed_colour() {
        let k = env_factor(0.5, [1.0, 0.5, 0.0]);
        assert_eq!(k, [127.0 / 255.0, 63.0 / 255.0, 0.0]);
        // a factor of 10 at full light: 2550 a channel, OR-ed into its neighbours
        let k = env_factor(10.0, [1.0; 3]);
        let p = (0xffu32 << 24) | (2550 << 16) | (2550 << 8) | 2550;
        assert_eq!(
            k,
            [(p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff].map(|c| c as f32 / 255.0)
        );
    }

    #[test]
    fn a_texture_has_alpha_when_a_texel_is_not_opaque() {
        let tex = |format, level: Vec<u8>| {
            let levels = [level];
            SceneTexture {
                width: 4,
                height: 4,
                format,
                levels: &levels,
                make_mips: false,
            }
            .alpha_in_use()
        };
        let px = |a: u8| [[0, 0, 0, 255].repeat(15), vec![0, 0, 0, a]].concat();
        assert!(!tex(TexFormat::Rgba8, px(255)));
        assert!(tex(TexFormat::Rgba8, px(254)));
        // DXT1: black all over, opaque; the same with one texel on the transparent index 3
        let solid = [0u8, 0, 0, 0, 0, 0, 0, 0];
        assert!(!tex(TexFormat::Bc1, solid.to_vec()));
        assert!(tex(TexFormat::Bc1, vec![0, 0, 0, 0, 0, 0, 0, 0xC0]));
        // (index 3 in four-colour mode is a colour)
        assert!(!tex(TexFormat::Bc1, vec![1, 0, 0, 0, 0, 0, 0, 0xC0]));
        assert!(!tex(TexFormat::Bc2, [vec![0xFF; 8], vec![0; 8]].concat()));
        assert!(tex(TexFormat::Bc2, [vec![0xFE; 8], vec![0; 8]].concat()));
        // DXT5: both end alphas 255, every index in use giving 255 (index 7 of the six-step
        // mode is 255 as well); one end below 255 and index 0 or 1 used is clear
        assert!(!tex(
            TexFormat::Bc3,
            [vec![255, 255, 0, 0, 0, 0, 0, 0], vec![0; 8]].concat()
        ));
        assert!(tex(
            TexFormat::Bc3,
            [vec![0, 255, 0, 0, 0, 0, 0, 0], vec![0; 8]].concat()
        ));
    }

    #[test]
    fn instance_records_follow_the_list() {
        let models = [[[1.0, 0.0, 0.0, 0.0]; 4], [[2.0, 0.0, 0.0, 0.0]; 4]];
        let params = [[10.0; 4], [11.0; 4], [20.0; 4], [21.0; 4]];
        let main = [batch(0..2)];
        let frame = SceneFrame {
            view_proj: [[0.0; 4]; 4],
            light: SceneLight::default(),
            list: &[1, 0],
            models: &models,
            params: &params,
            main: &main,
            cab: &[],
            surface_bias: -24,
            lamps: SceneLamps::default(),
            coronas: SceneCoronas::default(),
            rain: Default::default(),
        };
        frame.validate().unwrap();
        let mut out = Vec::new();
        frame.instance_records(&mut out);
        assert_eq!(out.len(), 2 * INSTANCE_FLOATS);
        assert_eq!(out[0], 2.0);
        assert_eq!(out[16], 20.0);
        assert_eq!(out[20], 21.0);
        assert_eq!(out[INSTANCE_FLOATS], 1.0);
        assert_eq!(out[INSTANCE_FLOATS + 16], 10.0);

        let outside = [batch(1..3)];
        assert!(SceneFrame {
            main: &outside,
            ..frame
        }
        .validate()
        .is_err());
        assert!(SceneFrame {
            list: &[2],
            main: &[],
            ..frame
        }
        .validate()
        .is_err());
    }

    #[test]
    fn a_corona_run_must_lie_in_the_sprite_list() {
        let sprites = [[0.0; CORONA_FLOATS]; 3];
        let ok = SceneCoronas {
            sprites: &sprites,
            runs: &[(0, 0, 2, false), (4, 2, 1, true)],
        };
        assert!(ok.validate().is_ok());
        let bad = SceneCoronas {
            sprites: &sprites,
            runs: &[(0, 2, 2, false)],
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn lights_are_packed_for_the_shader() {
        let spot = [
            [1.0, 2.0, 3.0, 16.0],
            [1.0, 0.5, 0.25, 2.0],
            [0.0, 0.0, -1.0, 0.7],
            [0.9, 0.5, 0.0, 4.0],
        ];
        assert_eq!(
            pack_light(&spot),
            [
                [1.0, 2.0, 3.0, 0.0],
                [2.0, 1.0, 0.5, 1.0 / 16.0],
                [0.0, 0.0, -1.0, 0.7],
                [0.9, 0.25, 4.0, 16.0],
            ]
        );
        // a vehicle's point light lights every surface; one passed by (radius 0) nothing,
        // an interior lamp without intensity reaches nothing
        let point = [
            [0.0; 4],
            [1.0, 1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0, -2.0],
            [0.0, 0.0, 0.0, 3.0],
        ];
        let packed = pack_light(&point);
        assert_eq!(packed[0][3], 1.0);
        assert_eq!((packed[1][3], packed[3][2], packed[3][3]), (0.0, 0.0, 0.0));
        assert!((packed[3][1] - 1e-4).abs() < 1e-9, "{}", packed[3][1]);
    }

    #[test]
    fn borders_and_env_bits_are_unpacked_for_the_shader() {
        assert_eq!(
            border_rgba([0.0, 1.0, (0x40_80_ff) as f32, 0.5]),
            [64.0 / 255.0, 128.0 / 255.0, 1.0, 0.5]
        );
        assert_eq!(border_rgba([0.0, 0.0, 12345.0, 1.0]), [0.0; 4]);
        assert_eq!(border_rgba([0.0, 2.0, 12345.0, 1.0]), [0.0; 4]);
        assert_eq!(env_bits(0.0), [0.0, 0.0]);
        assert_eq!(env_bits(1.0), [1.0, 0.0]);
        assert_eq!(env_bits(2.0), [0.0, 1.0]);
        assert_eq!(env_bits(3.0), [1.0, 1.0]);
    }

    #[test]
    fn lamp_grids_are_checked_and_read_as_indices() {
        let lights = [[0.0; 4]; 8];
        let grid = [0, 1, u32::MAX, 5, u32::MAX, u32::MAX, 1, 0];
        let lamps = SceneLamps {
            lights: &lights,
            grid: &grid,
            place: [0.0, 0.0, 25.0, 2.0],
            ..Default::default()
        };
        lamps.validate().unwrap();
        assert_eq!(lamps.count(), 2);
        assert_eq!(lamps.grid_shape(), Some((2, 2)));
        let mut out = Vec::new();
        lamps.grid_texels(2, &mut out);
        assert_eq!(out, [0.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 0.0]);
        assert!(SceneLamps {
            grid: &grid[..7],
            ..lamps
        }
        .validate()
        .is_err());
        assert!(SceneLamps {
            lights: &lights[..7],
            ..lamps
        }
        .validate()
        .is_err());
        // no grid: no map lamps, the interior lamps still
        let none = SceneLamps {
            place: [0.0; 4],
            ..lamps
        };
        assert_eq!(none.grid_shape(), None);
        none.validate().unwrap();
    }

    #[test]
    fn kinds_decide_blend_and_depth_write() {
        assert!(!DrawKind::Opaque.blends() && DrawKind::Opaque.writes_depth());
        assert!(DrawKind::Blend.blends() && DrawKind::Blend.writes_depth());
        assert!(DrawKind::BlendNoWrite.blends() && !DrawKind::BlendNoWrite.writes_depth());
        assert!(DrawKind::TerrainPaint.blends() && !DrawKind::TerrainPaint.writes_depth());
        assert!(!DrawKind::SurfaceDepth.writes_color());
    }

    #[test]
    fn the_surface_bias_pulls_towards_the_eye_in_reversed_depth() {
        let (constant, slope) = surface_depth_bias(-24);
        assert!(constant > 0.0 && slope > 0.0);
        assert_eq!(constant, 24.0 / 16_777_216.0);
        assert_eq!(surface_depth_bias(0), (0.0, 0.0));
    }
}

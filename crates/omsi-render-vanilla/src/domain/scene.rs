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

/// The sRGB curve's decode of a stored byte, in linear light.
fn srgb_to_linear(byte: u8) -> f32 {
    let c = byte as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The sRGB curve's encode of a linear value, as a stored byte.
fn linear_to_srgb(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let e = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0 + 0.5) as u8
}

/// The levels below a `w` x `h` RGBA image, each the 2 x 2 box average of the one above
/// (an odd edge repeats its last texel), down to 1 x 1. The colour is averaged in linear
/// light, as the default renderer's mip generator does it on its sRGB view (averaged as
/// stored, a fine pattern of light and dark - cobbles, grass - came out darker the further
/// off it lay), and weighted by alpha, as that generator weights it.
pub fn mip_chain(w: u32, h: u32, rgba: &[u8]) -> Vec<Vec<u8>> {
    let table: Vec<f32> = (0..=255u8).map(srgb_to_linear).collect();
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
                let at = |xx: usize, yy: usize, c: usize| prev[(yy * pw + xx) * 4 + c];
                let texels = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)];
                let alpha: u32 = texels.iter().map(|&(xx, yy)| at(xx, yy, 3) as u32).sum();
                let o = (y * nw + x) * 4;
                for c in 0..3 {
                    // weighted by alpha: the colour stored in transparent texels must not
                    // bleed into the silhouette (the default renderer's mip.wgsl does this);
                    // all four transparent: the plain average (alpha is often only a mask)
                    let (sum, weight) =
                        texels.iter().fold((0.0f32, 0.0f32), |(s, k), &(xx, yy)| {
                            let a = if alpha == 0 {
                                1.0
                            } else {
                                at(xx, yy, 3) as f32
                            };
                            (s + table[at(xx, yy, c) as usize] * a, k + a)
                        });
                    next[o + c] = linear_to_srgb(sum / weight);
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
    /// x how wet the outdoor surfaces are (0 dry .. 1 soaked), zw the render origin's map
    /// position modulo 1000 m, where the puddles' pattern is laid.
    pub weather: [f32; 4],
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
    /// At most this many map lamps a cell names are kept (0 = all of them: the "Fewer night
    /// lights" option), the first of its places; the vehicles' own lights are always kept.
    pub cell_cap: usize,
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
    /// names a light past the first `kept`. With a `cell_cap`, a cell keeps its first
    /// `cell_cap` map lamps and every vehicle light (a spot, or a point that lights every
    /// surface), in their order and packed to the front, the rest of the places empty:
    /// the shader stops at the first empty place.
    pub fn grid_texels(&self, kept: usize, out: &mut Vec<f32>) {
        out.clear();
        let valid = |i: u32| (i as usize) < kept;
        let Some((_, places)) = self.grid_shape().filter(|_| self.cell_cap > 0) else {
            out.extend(
                self.grid
                    .iter()
                    .map(|&i| if valid(i) { i as f32 } else { -1.0 }),
            );
            return;
        };
        let vehicle = |i: u32| {
            let dir = self.lights[i as usize * 4 + 2];
            // (a spot - a vehicle's headlight; the map's lamps are all points, -2 - or the older
            // headlight stand-in, a point that lights every surface)
            dir[3] > -1.5 || dir[0] > 0.5
        };
        for cell in self.grid.chunks(places) {
            let mut maps = 0;
            let before = out.len();
            for &i in cell.iter().take_while(|&&i| valid(i)) {
                if vehicle(i) {
                    out.push(i as f32);
                } else if maps < self.cell_cap {
                    maps += 1;
                    out.push(i as f32);
                }
            }
            out.resize(before + cell.len(), -1.0);
        }
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

/// The sky dome behind a picture: the three pictures of `envir.cfg`'s `[sky_textures]` and
/// the light that blends them. Without the pictures the picture keeps its clear colour.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SceneSky {
    /// The day, twilight and night pictures, as scene texture ids.
    pub textures: Option<[usize; 3]>,
    /// x the sun's azimuth (radians, clockwise from north), yzw the weights of the day,
    /// twilight and night pictures.
    pub params: [f32; 4],
    /// The weather's clouds over the dome.
    pub clouds: SceneClouds,
    /// Leaves the night's stars out (the Stars option off; the default draws them).
    pub no_stars: bool,
    /// Leaves the cloud layer out of the sky's picture (the Cloud picture option off).
    pub no_cloud_layer: bool,
    /// The sun's and the moon's pictures (`Texture\sonne.bmp`, `Texture\mond.bmp`) as scene
    /// texture ids: the sun's is drawn where the sun stands, the moon's on a ball lit by the
    /// sun (its phase). One that is not there is left out.
    pub bodies: [Option<usize>; 2],
    /// xyz towards the moon (world).
    pub moon_dir: [f32; 4],
    /// The star map the stars are drawn from (`omsi_content::StarMap`) as a scene texture
    /// id; None leaves the stars out.
    pub stars: Option<usize>,
    /// The rows of the rotation from a world direction into the stars' frame; w of the first
    /// how bright the stars are (0 none), w of the second the star map's cells across a face.
    pub star_frame: [[f32; 4]; 3],
}

/// The weather's clouds: one layer of the weather's cloud type, drawn over the sky dome.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SceneClouds {
    /// The cloud type's picture as a scene texture id (white, the clouds in its alpha); none
    /// draws no clouds.
    pub texture: Option<usize>,
    /// x the cover (0 none .. 1 closed; 0 draws no clouds), yz the drift with the wind in
    /// fractions of the picture's tile, w the layer's base height in metres.
    pub params: [f32; 4],
}

/// The smoke and particles of a picture, as the default renderer hands them to its smoke
/// shader (`Scene::frame_smoke` of `omsi-render`): camera-facing squares of the game's smoke
/// picture, alpha-blended over the scene, farthest first. Only OMSI's own `[smoke]` and
/// `[particle_emitter]` particles: the game's tyre spray on a wet road is not among them.
#[derive(Debug, Clone, Copy, Default)]
pub struct SceneSmoke<'a> {
    /// [`CORONA_FLOATS`] floats each, the last (the kind) 3: position and size; colour and
    /// alpha; the cosine and sine of the spin; the ground it fades into and the z offset.
    pub sprites: &'a [[f32; CORONA_FLOATS]],
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
    /// The sky dome drawn first, behind everything.
    pub sky: SceneSky,
    /// The smoke and particles drawn over the world's batches.
    pub smoke: SceneSmoke<'a>,
    /// The street seen from under the road, for the puddles to mirror: the scene texture it
    /// was drawn into and the camera's view-projection it was drawn with (as `view_proj`).
    /// None when the puddles mirror the sky alone.
    pub rain_reflection: Option<(usize, [[f32; 4]; 4])>,
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

    /// Writes one instance record ([`INSTANCE_FLOATS`]) per list entry into `out`, in list
    /// order: a batch's instances are then the records of its run. `out` holds the records
    /// of the whole list (the device writes them straight into its locked buffer).
    pub fn write_instance_records(&self, out: &mut [f32]) {
        for (&e, record) in self.list.iter().zip(out.chunks_exact_mut(INSTANCE_FLOATS)) {
            let e = e as usize;
            for (column, dst) in self.models[e].iter().zip(record.chunks_exact_mut(4)) {
                dst.copy_from_slice(column);
            }
            record[16..20].copy_from_slice(&self.params[e * 2]);
            record[20..24].copy_from_slice(&self.params[e * 2 + 1]);
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

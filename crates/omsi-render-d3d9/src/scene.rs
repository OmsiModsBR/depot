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

impl SceneTexture<'_> {
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
/// (an odd edge repeats its last texel), down to 1 x 1.
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
                for c in 0..4 {
                    let at = |xx: usize, yy: usize| prev[(yy * pw + xx) * 4 + c] as u32;
                    let sum = at(x0, y0) + at(x1, y0) + at(x0, y1) + at(x1, y1);
                    next[(y * nw + x) * 4 + c] = ((sum + 2) / 4) as u8;
                }
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

/// What a material is drawn with.
#[derive(Debug, Clone, Copy)]
pub struct SceneMaterial<'a> {
    /// The diffuse texture (white when none or not uploaded).
    pub texture: Option<usize>,
    /// `[matl_transmap]` (its alpha replaces the diffuse alpha when the uniform says so).
    pub transmap: Option<usize>,
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
        Ok(())
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
        assert_eq!(chain[0], vec![50, 50, 50, 50]);
        // the chain of 8x2 has 8x2 -> 4x1 -> 2x1 -> 1x1 below level 0
        let chain = mip_chain(8, 2, &[7u8; 8 * 2 * 4]);
        let sizes: Vec<usize> = chain.iter().map(|l| l.len() / 4).collect();
        assert_eq!(sizes, vec![4, 2, 1]);
        assert!(chain.iter().flatten().all(|&c| c == 7));
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

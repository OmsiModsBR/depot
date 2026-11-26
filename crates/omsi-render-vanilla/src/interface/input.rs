//! The renderer interface. It covers only what the Vanilla profile needs: create a texture or
//! a mesh, draw a batch with its state in a pass, and recover from a lost device. Nothing here
//! names a graphics API, so the wgpu renderer can satisfy it too.

use crate::{BackendError, Caps, Result};

/// A texture of a backend. Carries the device generation it was made in, so a handle from
/// before [`RenderBackend::recover`] is rejected instead of naming a new resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

/// A mesh of a backend; see [`TextureHandle`] for the generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshHandle {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

/// Checks a handle's parts against a backend's current generation and resource count.
pub(crate) fn handle_index(
    index: u32,
    generation: u32,
    current: u32,
    count: usize,
) -> Result<usize> {
    if generation != current {
        return Err(BackendError::InvalidArgument(
            "handle from before the device was recovered".into(),
        ));
    }
    let i = index as usize;
    if i >= count {
        return Err(BackendError::InvalidArgument("unknown handle".into()));
    }
    Ok(i)
}

/// The alpha handling of a material, as the content declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    Opaque,
    /// Hard cut-off against a reference value.
    Test,
    /// Blended; drawn back to front in the alpha pass.
    Blend,
}

#[derive(Debug, Clone)]
pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct MeshDesc {
    /// Interleaved position(3) normal(3) uv(2), as `f32`.
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
}

/// Floats per vertex in [`MeshDesc::vertices`].
pub const VERTEX_FLOATS: usize = 8;

impl TextureDesc {
    /// The checks every backend applies before creating the texture.
    pub fn validate(&self, caps: &Caps) -> Result<()> {
        if self.width == 0
            || self.height == 0
            || self.rgba.len() != (self.width as usize * self.height as usize * 4)
        {
            return Err(BackendError::InvalidArgument("texture data size".into()));
        }
        let max = caps.max_texture_size;
        if self.width > max || self.height > max {
            return Err(BackendError::Unsupported(format!(
                "texture larger than {max}"
            )));
        }
        if !caps.non_pow2_textures
            && !(self.width.is_power_of_two() && self.height.is_power_of_two())
        {
            return Err(BackendError::Unsupported(
                "texture size is not a power of two".into(),
            ));
        }
        Ok(())
    }
}

impl MeshDesc {
    pub fn vertex_count(&self) -> u32 {
        (self.vertices.len() / VERTEX_FLOATS) as u32
    }

    /// The checks every backend applies before creating the mesh.
    pub fn validate(&self, caps: &Caps) -> Result<()> {
        crate::scene::validate_mesh(&self.vertices, &self.indices, caps.index32)
    }
}

#[derive(Debug, Clone)]
pub struct DrawBatch {
    pub mesh: MeshHandle,
    pub texture: Option<TextureHandle>,
    pub alpha: AlphaMode,
    /// Column-major model-view-projection.
    pub mvp: [f32; 16],
    /// Multiplies the texture's colour and alpha (RGBA).
    pub tint: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct FrameDesc {
    pub width: u32,
    pub height: u32,
    pub clear_rgba: [f32; 4],
}

pub trait RenderBackend {
    fn caps(&self) -> &Caps;
    fn create_texture(&mut self, desc: &TextureDesc) -> Result<TextureHandle>;
    fn create_mesh(&mut self, desc: &MeshDesc) -> Result<MeshHandle>;
    fn begin_frame(&mut self, frame: &FrameDesc) -> Result<()>;
    /// Opaque and test batches first, then blended ones; the backend does not reorder.
    fn draw(&mut self, batch: &DrawBatch) -> Result<()>;
    /// Ends the frame and returns it as RGBA8 (`width * height * 4`) when it was off-screen.
    fn end_frame(&mut self) -> Result<Vec<u8>>;
    /// Rebuilds the device after [`BackendError::DeviceLost`]. Resources must be re-created by
    /// the caller afterwards; handles from before the loss are invalid.
    fn recover(&mut self) -> Result<()>;
}

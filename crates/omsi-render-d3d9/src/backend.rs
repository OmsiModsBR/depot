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
        if self.vertices.is_empty() || !self.vertices.len().is_multiple_of(VERTEX_FLOATS) {
            return Err(BackendError::InvalidArgument(
                "vertex data is not 8 floats per vertex".into(),
            ));
        }
        let count = self.vertex_count();
        if self.indices.is_empty()
            || !self.indices.len().is_multiple_of(3)
            || self.indices.iter().any(|&i| i >= count)
        {
            return Err(BackendError::InvalidArgument("indices".into()));
        }
        if !caps.index32 && count > 0x1_0000 {
            return Err(BackendError::Unsupported("32-bit indices".into()));
        }
        Ok(())
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

/// A backend with no GPU: validates arguments and records what it was asked to do. It exists
/// to exercise the interface (and code written against it) on every platform.
#[derive(Debug)]
pub struct NullBackend {
    caps: Caps,
    textures: u32,
    meshes: u32,
    generation: u32,
    in_frame: Option<FrameDesc>,
    pub batches_drawn: usize,
    pub lost: bool,
}

impl NullBackend {
    pub fn new(caps: Caps) -> Self {
        Self {
            caps,
            textures: 0,
            meshes: 0,
            generation: 0,
            in_frame: None,
            batches_drawn: 0,
            lost: false,
        }
    }

    fn check_alive(&self) -> Result<()> {
        if self.lost {
            Err(BackendError::DeviceLost)
        } else {
            Ok(())
        }
    }
}

impl RenderBackend for NullBackend {
    fn caps(&self) -> &Caps {
        &self.caps
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<TextureHandle> {
        self.check_alive()?;
        desc.validate(&self.caps)?;
        self.textures += 1;
        Ok(TextureHandle {
            index: self.textures - 1,
            generation: self.generation,
        })
    }

    fn create_mesh(&mut self, desc: &MeshDesc) -> Result<MeshHandle> {
        self.check_alive()?;
        desc.validate(&self.caps)?;
        self.meshes += 1;
        Ok(MeshHandle {
            index: self.meshes - 1,
            generation: self.generation,
        })
    }

    fn begin_frame(&mut self, frame: &FrameDesc) -> Result<()> {
        self.check_alive()?;
        if self.in_frame.is_some() {
            return Err(BackendError::InvalidArgument("frame already begun".into()));
        }
        let max = self.caps.max_texture_size;
        if frame.width == 0 || frame.height == 0 || frame.width.max(frame.height) > max {
            return Err(BackendError::InvalidArgument("target size".into()));
        }
        self.in_frame = Some(frame.clone());
        Ok(())
    }

    fn draw(&mut self, batch: &DrawBatch) -> Result<()> {
        self.check_alive()?;
        if self.in_frame.is_none() {
            return Err(BackendError::InvalidArgument("draw outside a frame".into()));
        }
        handle_index(
            batch.mesh.index,
            batch.mesh.generation,
            self.generation,
            self.meshes as usize,
        )?;
        if let Some(t) = batch.texture {
            handle_index(
                t.index,
                t.generation,
                self.generation,
                self.textures as usize,
            )?;
        }
        self.batches_drawn += 1;
        Ok(())
    }

    fn end_frame(&mut self) -> Result<Vec<u8>> {
        self.check_alive()?;
        let f = self
            .in_frame
            .take()
            .ok_or_else(|| BackendError::InvalidArgument("no frame".into()))?;
        let px: Vec<u8> = f
            .clear_rgba
            .iter()
            .map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
            .collect();
        Ok(px
            .iter()
            .copied()
            .cycle()
            .take(f.width as usize * f.height as usize * 4)
            .collect())
    }

    fn recover(&mut self) -> Result<()> {
        self.lost = false;
        self.generation += 1;
        self.textures = 0;
        self.meshes = 0;
        self.in_frame = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShaderModel;

    fn backend() -> NullBackend {
        NullBackend::new(Caps {
            adapter_name: "null".into(),
            shader_model: ShaderModel::Sm3,
            max_texture_size: 2048,
            non_pow2_textures: true,
            index32: false,
            max_render_targets: 1,
        })
    }

    fn tri() -> MeshDesc {
        MeshDesc {
            vertices: vec![0.0; 24],
            indices: vec![0, 1, 2],
        }
    }

    #[test]
    fn frame_roundtrip_returns_clear_color() {
        let mut b = backend();
        b.begin_frame(&FrameDesc {
            width: 2,
            height: 1,
            clear_rgba: [1.0, 0.0, 0.0, 1.0],
        })
        .unwrap();
        let px = b.end_frame().unwrap();
        assert_eq!(px, vec![255, 0, 0, 255, 255, 0, 0, 255]);
    }

    #[test]
    fn draw_validates_handles_and_frame() {
        let mut b = backend();
        let m = b.create_mesh(&tri()).unwrap();
        let batch = DrawBatch {
            mesh: m,
            texture: None,
            alpha: AlphaMode::Opaque,
            mvp: [0.0; 16],
            tint: [1.0; 4],
        };
        assert!(b.draw(&batch).is_err(), "draw outside a frame");
        b.begin_frame(&FrameDesc {
            width: 1,
            height: 1,
            clear_rgba: [0.0; 4],
        })
        .unwrap();
        b.draw(&batch).unwrap();
        let bad = DrawBatch {
            mesh: MeshHandle { index: 9, ..m },
            ..batch
        };
        assert!(b.draw(&bad).is_err());
        assert_eq!(b.batches_drawn, 1);
    }

    #[test]
    fn rejects_bad_resources() {
        let mut b = backend();
        assert!(b
            .create_texture(&TextureDesc {
                width: 2,
                height: 2,
                rgba: vec![0; 3]
            })
            .is_err());
        assert!(b
            .create_texture(&TextureDesc {
                width: 4096,
                height: 1,
                rgba: vec![0; 4096 * 4]
            })
            .is_err());
        assert!(b
            .create_mesh(&MeshDesc {
                vertices: vec![0.0; 24],
                indices: vec![0, 1, 7]
            })
            .is_err());
    }

    #[test]
    fn device_loss_blocks_until_recovered() {
        let mut b = backend();
        b.lost = true;
        assert!(matches!(
            b.create_mesh(&tri()),
            Err(BackendError::DeviceLost)
        ));
        b.recover().unwrap();
        assert!(b.create_mesh(&tri()).is_ok());
    }

    #[test]
    fn handles_from_before_recovery_are_rejected() {
        let mut b = backend();
        let old = b.create_mesh(&tri()).unwrap();
        b.recover().unwrap();
        let new = b.create_mesh(&tri()).unwrap();
        assert_eq!(old.index, new.index, "same slot, different generation");
        b.begin_frame(&FrameDesc {
            width: 1,
            height: 1,
            clear_rgba: [0.0; 4],
        })
        .unwrap();
        let batch = |mesh| DrawBatch {
            mesh,
            texture: None,
            alpha: AlphaMode::Opaque,
            mvp: [0.0; 16],
            tint: [1.0; 4],
        };
        assert!(b.draw(&batch(old)).is_err());
        b.draw(&batch(new)).unwrap();
    }

    #[test]
    fn frame_size_is_checked() {
        let mut b = backend();
        assert!(b
            .begin_frame(&FrameDesc {
                width: 0,
                height: 1,
                clear_rgba: [0.0; 4]
            })
            .is_err());
        assert!(b
            .begin_frame(&FrameDesc {
                width: 4096,
                height: 1,
                clear_rgba: [0.0; 4]
            })
            .is_err());
    }
}

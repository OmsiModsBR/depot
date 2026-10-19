//! The renderer interface. It covers only what the Vanilla profile needs: create a texture or
//! a mesh, draw a batch with its state in a pass, and recover from a lost device. Nothing here
//! names a graphics API, so the wgpu renderer can satisfy it too.

use crate::{BackendError, Caps, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshHandle(pub u32);

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

#[derive(Debug, Clone)]
pub struct DrawBatch {
    pub mesh: MeshHandle,
    pub texture: Option<TextureHandle>,
    pub alpha: AlphaMode,
    /// Column-major model-view-projection.
    pub mvp: [f32; 16],
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
        if desc.width == 0
            || desc.height == 0
            || desc.rgba.len() != (desc.width * desc.height * 4) as usize
        {
            return Err(BackendError::InvalidArgument("texture data size".into()));
        }
        let max = self.caps.max_texture_size;
        if desc.width > max || desc.height > max {
            return Err(BackendError::Unsupported(format!(
                "texture larger than {max}"
            )));
        }
        self.textures += 1;
        Ok(TextureHandle(self.textures - 1))
    }

    fn create_mesh(&mut self, desc: &MeshDesc) -> Result<MeshHandle> {
        self.check_alive()?;
        if desc.vertices.is_empty() || !desc.vertices.len().is_multiple_of(8) {
            return Err(BackendError::InvalidArgument(
                "vertex data is not 8 floats per vertex".into(),
            ));
        }
        let count = (desc.vertices.len() / 8) as u32;
        if desc.indices.is_empty()
            || !desc.indices.len().is_multiple_of(3)
            || desc.indices.iter().any(|&i| i >= count)
        {
            return Err(BackendError::InvalidArgument("indices".into()));
        }
        if !self.caps.index32 && count > 65_536 {
            return Err(BackendError::Unsupported("32-bit indices".into()));
        }
        self.meshes += 1;
        Ok(MeshHandle(self.meshes - 1))
    }

    fn begin_frame(&mut self, frame: &FrameDesc) -> Result<()> {
        self.check_alive()?;
        if self.in_frame.is_some() {
            return Err(BackendError::InvalidArgument("frame already begun".into()));
        }
        self.in_frame = Some(frame.clone());
        Ok(())
    }

    fn draw(&mut self, batch: &DrawBatch) -> Result<()> {
        self.check_alive()?;
        if self.in_frame.is_none() {
            return Err(BackendError::InvalidArgument("draw outside a frame".into()));
        }
        if batch.mesh.0 >= self.meshes || batch.texture.is_some_and(|t| t.0 >= self.textures) {
            return Err(BackendError::InvalidArgument("unknown handle".into()));
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
            .take((f.width * f.height * 4) as usize)
            .collect())
    }

    fn recover(&mut self) -> Result<()> {
        self.lost = false;
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
            mesh: MeshHandle(9),
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
}

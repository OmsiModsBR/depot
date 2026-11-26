//! A backend with no GPU, for testing the interface (and code written against it) on every
//! platform.

use crate::backend::{
    handle_index, DrawBatch, FrameDesc, MeshDesc, MeshHandle, RenderBackend, TextureDesc,
    TextureHandle,
};
use crate::{BackendError, Caps, Result};

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
    use crate::backend::AlphaMode;
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

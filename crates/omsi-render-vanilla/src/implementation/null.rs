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

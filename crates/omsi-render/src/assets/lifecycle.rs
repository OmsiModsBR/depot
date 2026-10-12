//! The life cycle of the scene's resources: freeing meshes, textures and materials again
//! and recycling their slots.

use super::material::{AlphaMode, TexAddressing};
use crate::renderer::Renderer;
use crate::scene::Scene;
use crate::scene::Vertex;

/// The placeholders freed scene slots share: an empty vertex and index buffer and a plain
/// material bind group. Making new ones for every freed slot (two buffers per mesh, a
/// buffer and a bind group per material) was most of a tile unload's time.
pub(crate) struct Freed {
    pub(crate) vertex_buf: wgpu::Buffer,
    pub(crate) index_buf: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) buf: wgpu::Buffer,
}

/// Letting go of scene resources again (tile streaming). Ids stay stable: a freed mesh,
/// texture or material keeps its slot with a tiny placeholder, and a resource added later
/// can take the slot over (`recycle_*`), so a long drive across a big map does not grow the
/// scene without bound. The contract of `recycle_*`: call it right after the `add_*` that
/// produced `new`, before anything refers to `new`.
impl Renderer {
    /// The shared placeholders of freed slots.
    pub(crate) fn freed(&self, scene: &mut Scene) -> &Freed {
        if self.assets.freed.get().is_none() {
            // a plain material, built as any other and taken out of the scene again (without
            // using up an addressing request meant for the next real material)
            let address = self.address_next.replace(TexAddressing::Wrap);
            let plain = self.add_material(scene, None, AlphaMode::Opaque, [1.0; 4], false);
            self.address_next.set(address);
            let m = scene.materials.swap_remove(plain);
            let (bind_group, buf) = (m.bind_group, m.buf);
            let vertex_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("freed mesh"),
                size: std::mem::size_of::<Vertex>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let index_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("freed mesh"),
                size: 4,
                usage: wgpu::BufferUsages::INDEX,
                mapped_at_creation: false,
            });
            let _ = self.assets.freed.set(Freed {
                vertex_buf,
                index_buf,
                bind_group,
                buf,
            });
        }
        self.assets.freed.get().unwrap()
    }
}

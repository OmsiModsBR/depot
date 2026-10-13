//! The ids the scene names its assets by, and the vertex a mesh is made of.

pub type MeshId = usize;

pub type TextureId = usize;

pub type MaterialId = usize;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

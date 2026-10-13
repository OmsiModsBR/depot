//! The bind group layouts the scene's shaders share: the camera's, the materials' and the
//! sky's.

/// The bind group layouts the scene's shaders share.
pub(crate) struct Layouts {
    pub(crate) camera_layout: wgpu::BindGroupLayout,
    pub(crate) material_layout: wgpu::BindGroupLayout,
    pub(crate) sky_layout: wgpu::BindGroupLayout,
}

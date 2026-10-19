//! The bind group layouts the scene's shaders share: the camera's, the materials' and the
//! sky's.

use super::camera::camera_layout_entries;
use crate::assets::material_layout_entries;
use crate::gpu::{array_path, sampler_entry, sixteen_texture_units, texture_entry};

/// The bind group layouts the scene's shaders share.
pub(crate) struct Layouts {
    pub(crate) camera_layout: wgpu::BindGroupLayout,
    pub(crate) material_layout: wgpu::BindGroupLayout,
    pub(crate) sky_layout: wgpu::BindGroupLayout,
}

impl Layouts {
    /// The camera's, the materials' and the sky's bind group layouts.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let camera_entries = camera_layout_entries(array_path(), sixteen_texture_units());
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera"),
            entries: &camera_entries,
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
            entries: &material_layout_entries(),
        });
        // sky dome
        let sky_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                texture_entry(2),
                sampler_entry(3),
                texture_entry(4),
                sampler_entry(5),
                texture_entry(6),
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                sampler_entry(8),
            ],
        });
        Self {
            camera_layout,
            material_layout,
            sky_layout,
        }
    }
}

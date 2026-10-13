//! The camera and the camera uniform the shaders read.

use crate::gpu::{array_layout_entry_on, ArrayPath};
use glam::{DVec3, Mat4, Vec3};

#[derive(Default)]
pub(crate) struct ViewCulling {
    pub(crate) drawn: Vec<u64>,
    pub(crate) sizes: hashbrown::HashMap<[u64; 4], f32>,
    pub(crate) scratch: hashbrown::HashMap<[u64; 4], f32>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CameraUniform {
    pub(crate) view_proj: [[f32; 4]; 4],
    pub(crate) cam_pos: [f32; 4],
    /// World-space render origin modulo 1000 m (the procedural patterns' period), added back
    /// for procedural patterns that must not jump when the floating origin moves to the next
    /// 100 m cell.
    pub(crate) world_origin: [f32; 4],
    pub(crate) sun_dir: [f32; 4],
    pub(crate) ambient: [f32; 4],
    pub(crate) fog: [f32; 4],
    pub(crate) sun_color: [f32; 4],
    pub(crate) sky_color: [f32; 4],
    pub(crate) light_grid: [f32; 4],
    pub(crate) sky: [f32; 4],
    pub(crate) cam_right: [f32; 4],
    pub(crate) cam_up: [f32; 4],
    pub(crate) clouds: [f32; 4],
    pub(crate) light_view_proj: [[f32; 4]; 4],
    pub(crate) light_view_proj_far: [[f32; 4]; 4],
    pub(crate) shadow: [f32; 4],
    /// x enhanced graphics (0/1), y seconds since start, z sun ndc x, w sun ndc y
    pub(crate) post: [f32; 4],
    /// The player's vehicle as a box the weather does not reach into: origin (render-origin
    /// relative) and sin(heading); cos(heading) and the half extents; the box centre offset
    /// and whether there is one.
    pub(crate) inside_a: [f32; 4],
    pub(crate) inside_b: [f32; 4],
    pub(crate) inside_c: [f32; 4],
    /// x procedural detail texturing (0/1), y soft shadow penumbra (0/1), z aerial
    /// perspective strength, w the close shadow cascade's half range
    pub(crate) flags: [f32; 4],
    /// The close shadow cascade (right half of the near map).
    pub(crate) light_view_proj_close: [[f32; 4]; 4],
    /// The player's vehicle's velocity (m/s, world) and 1: the airstream the rain on its
    /// glass meets (see `Lighting::glass_wind`).
    pub(crate) wind: [f32; 4],
    /// Enhanced: the street lamps' shadow maps (`LAMP_SHADOWS` tiles under the far map),
    /// and the light indices they belong to (-1: none).
    pub(crate) lamp_view_proj: [[[f32; 4]; 4]; 4],
    pub(crate) lamp_shadow: [f32; 4],
    /// Windy trees: xy the weather's wind (m/s, world; 0 with the setting off), zw how far
    /// the air has carried the gusts since the start (m, modulo the shaders' PATTERN_PERIOD).
    pub(crate) tree_wind: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// World position (f64: maps span millions of metres).
    pub position: DVec3,
    /// Degrees, 0 = looking north (+y), clockwise positive (OMSI heading).
    pub yaw: f32,
    /// Degrees, positive = looking up.
    pub pitch: f32,
    /// Degrees about the view direction (0 = the horizon level; see [`Camera::up`]). A
    /// camera fixed to a vehicle - a mirror's - leans with its body.
    pub roll: f32,
    pub fov_deg: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        Vec3::new(sy * cp, cy * cp, sp)
    }
    pub fn right(&self) -> Vec3 {
        let f = self.forward();
        let r0 = Vec3::new(f.y, -f.x, 0.0).normalize_or_zero();
        if self.roll == 0.0 {
            return r0;
        }
        f.cross(self.up()).normalize_or(r0)
    }
    /// The picture's up: world up for a level camera, turned about the view direction by
    /// `roll` (positive: the top leans to the right).
    pub fn up(&self) -> Vec3 {
        let f = self.forward();
        let r0 = Vec3::new(f.y, -f.x, 0.0).normalize_or_zero();
        if self.roll == 0.0 || r0 == Vec3::ZERO {
            return Vec3::Z;
        }
        let u0 = r0.cross(f);
        let (s, c) = self.roll.to_radians().sin_cos();
        (u0 * c + r0 * s).normalize_or(Vec3::Z)
    }
    /// View-projection relative to a render origin (the camera itself when `origin` is its
    /// position), so that GPU maths stays in small numbers.
    pub fn view_proj(&self, aspect: f32, origin: DVec3) -> Mat4 {
        let view = Mat4::look_to_rh(
            (self.position - origin).as_vec3(),
            self.forward(),
            self.up(),
        );
        // Reversed Z (near and far swapped): the depth buffer then spends its float
        // precision where the scene is far away instead of where it is close, which is what
        // stops distant roads, kerbs and painted ground from flickering against each other
        // - with a plain 0..1 depth the resolution at a kilometre is a good quarter of a
        // metre, less than the gap between a road surface and the ground under it.
        let proj = Mat4::perspective_rh(self.fov_deg.to_radians(), aspect, self.far, self.near);
        proj * view
    }

    /// Ray through a normalized device coordinate, relative to `origin`.
    pub fn ray(&self, ndc_x: f32, ndc_y: f32, aspect: f32, origin: DVec3) -> (Vec3, Vec3) {
        let inv = self.view_proj(aspect, origin).inverse();
        // ndc z = 0 is the far plane with reversed Z: the longest baseline for the ray
        let p = inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
        let o = (self.position - origin).as_vec3();
        (o, (p - o).normalize_or_zero())
    }
}

/// The camera group's textures only the enhanced path reads: the reflection probe, the sky
/// table and the sky cube.
pub(crate) const ENHANCED_CAMERA_TEXTURES: [u32; 3] = [12, 14, 17];

/// The camera group's entries on a device whose arrays take `path`, without the enhanced
/// path's textures where it has `sixteen` texture units (see `sixteen_texture_units`).
pub(crate) fn camera_layout_entries(
    path: ArrayPath,
    sixteen: bool,
) -> Vec<wgpu::BindGroupLayoutEntry> {
    let mut camera_entries = vec![
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        array_layout_entry_on(path, 1, wgpu::ShaderStages::VERTEX, true),
        array_layout_entry_on(path, 2, wgpu::ShaderStages::VERTEX, true),
        wgpu::BindGroupLayoutEntry {
            binding: 5,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 6,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 7,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 8,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 9,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
        array_layout_entry_on(path, 10, wgpu::ShaderStages::VERTEX, false),
        // enhanced: its lighting, the reflection probe, a clamped linear sampler, the sky table
        wgpu::BindGroupLayoutEntry {
            binding: 11,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 12,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::Cube,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 13,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 14,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 17,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::Cube,
                multisampled: false,
            },
            count: None,
        },
        // the tile light maps around the camera, and where they lie
        wgpu::BindGroupLayoutEntry {
            binding: 18,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 19,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
    ];
    // the point lights and their grid (see `ArrayPath::NoStorage`)
    if path != ArrayPath::NoStorage {
        camera_entries.push(array_layout_entry_on(
            path,
            3,
            wgpu::ShaderStages::FRAGMENT,
            true,
        ));
        camera_entries.push(array_layout_entry_on(
            path,
            4,
            wgpu::ShaderStages::FRAGMENT,
            false,
        ));
    }
    if sixteen {
        camera_entries.retain(|e| !ENHANCED_CAMERA_TEXTURES.contains(&e.binding));
    }
    camera_entries
}

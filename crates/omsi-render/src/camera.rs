//! The camera, the vertex layout and the camera uniform the shaders read.

use super::*;

#[derive(Default)]
pub(crate) struct ViewCulling {
    pub(crate) drawn: Vec<u64>,
    pub(crate) sizes: hashbrown::HashMap<[u64; 4], f32>,
    pub(crate) scratch: hashbrown::HashMap<[u64; 4], f32>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
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
